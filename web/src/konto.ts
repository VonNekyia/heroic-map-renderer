/**
 * Anmeldung und Sitzung am Server des Renderers, unter `api/` neben der
 * Seite: Anmelde-Token aus dem Fragment, Abmelden, wer angemeldet oder über
 * die IP erkannt ist, der Tausch über IPv4, die Knöpfe, der Klick und die
 * Mitspieler. Die Sitzung trägt ein Cookie mit HttpOnly; die Seite sieht es
 * nie. Siehe docs/frontend.md, „Mitspieler“.
 */
import { istObjekt, istText, istZahl } from './pruefen';

/** Ein Knopf an einem Objekt, mit dem Zustand für den angemeldeten Spieler. */
export interface Knopf {
  ebene: string;
  objekt: string;
  knopf: string;
  label: { de?: string; en?: string };
  on: boolean;
}

/** Wer angemeldet ist; ohne `ich.json` beim Server nur mit `uuid`. */
export interface Spieler {
  name?: string;
  uuid: string;
  /** Erkannt über die IP oder mit einer lesenden Sitzung: sehen ja, klicken nein. */
  nurLesen: boolean;
}

/**
 * Was `api/ich` sagt: erkannt, ein Gast oder kein Server des Renderers. Ein
 * Gast über IPv6 bekommt dazu, wo der Server ihn über IPv4 erkennen kann,
 * oder dass es nicht geht. Siehe docs/benutzung/server.md, „Erkennung per IP“.
 */
export type Erkennung =
  | { art: 'erkannt'; spieler: Spieler }
  /** `erkennen`: die Adresse für den Tausch über IPv4; `ipv6`: über IPv6 ohne zweiten Namen, erkennen geht nicht. */
  | { art: 'gast'; erkennen?: string; ipv6: boolean }
  | { art: 'fehlt' };

/**
 * Was `api/` antwortet: Wert, `abgemeldet` bei 401, `fehlt` ohne Server des
 * Renderers. Ein statischer Server liefert auf unbekannte Pfade oft die
 * index.html; sie ist kein JSON und heisst darum auch `fehlt`.
 */
type Antwort<T> = T | 'abgemeldet' | 'fehlt';

/** Gegen fremde Seiten: Beides erzwingt einen Preflight, den der Server nie mit CORS beantwortet. */
const KOPF = { 'Content-Type': 'application/json', 'X-Heroic-Map': '1' };

/** Das Anmelde-Token: zwei Teile base64url, wie in docs/plugin.md, „Token“. */
const TOKEN = /^[A-Za-z0-9_-]{1,128}\.[A-Za-z0-9_-]{1,128}$/;

/**
 * Nimmt das Anmelde-Token aus `#anmelden=…` und löscht das Fragment aus der
 * Adresse, auch ein kaputtes. Vor jeder Anfrage der Seite: Danach steht es
 * weder in der Adresse noch im Verlauf.
 */
export function nimmToken(): string | undefined {
  if (!location.hash.startsWith('#anmelden=')) return undefined;
  const token = location.hash.slice('#anmelden='.length);
  history.replaceState(history.state, '', location.pathname + location.search);
  return TOKEN.test(token) ? token : undefined;
}

async function frage(pfad: string, init: RequestInit = {}): Promise<Response | 'fehlt'> {
  try {
    return await fetch(`api/${pfad}`, { cache: 'no-store', credentials: 'same-origin', ...init });
  } catch {
    return 'fehlt';
  }
}

/** Holt JSON bis `max` Byte. */
async function hole(pfad: string, max: number): Promise<Antwort<unknown>> {
  const antwort = await frage(pfad);
  if (antwort === 'fehlt') return 'fehlt';
  if (antwort.status === 401) return 'abgemeldet';
  if (!antwort.ok) return 'fehlt';
  const text = await antwort.text();
  if (new TextEncoder().encode(text).length > max) {
    console.warn(`api/${pfad}: grösser als ${max} Byte, übergangen`);
    return 'fehlt';
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return 'fehlt';
  }
}

/** Ein `POST` unter `api/`; gibt den Status, 0 ohne Antwort. */
async function schicke(pfad: string, koerper: object, kopf: Record<string, string> = {}): Promise<number> {
  const antwort = await frage(pfad, { method: 'POST', headers: { ...KOPF, ...kopf }, body: JSON.stringify(koerper) });
  return antwort === 'fehlt' ? 0 : antwort.status;
}

/** Löst das Anmelde-Token ein; 204 heisst angemeldet, das Cookie setzt der Server. */
export const anmelden = (token: string): Promise<number> => schicke('anmelden', {}, { Authorization: `Bearer ${token}` });

export const abmelden = (): Promise<number> => schicke('abmelden', {});

export async function ich(): Promise<Erkennung> {
  const antwort = await frage('ich');
  if (antwort === 'fehlt') return { art: 'fehlt' };
  // 204 ohne Erkennung, 401 nach einer abgelaufenen Sitzung, deren Cookie der Server schon gelöscht hat.
  if (antwort.status === 204 || antwort.status === 401) {
    const erkennen = antwort.headers.get('x-heroic-erkennen') ?? undefined;
    return { art: 'gast', erkennen, ipv6: antwort.headers.get('x-heroic-ip') === '6' };
  }
  if (!antwort.ok) return { art: 'fehlt' };
  let wert: unknown;
  try {
    wert = JSON.parse(await antwort.text());
  } catch {
    return { art: 'fehlt' };
  }
  if (!istObjekt(wert) || !istText(wert.uuid, 36)) return { art: 'fehlt' };
  return { art: 'erkannt', spieler: { name: istText(wert.name, 64) ? wert.name : undefined, uuid: wert.uuid, nurLesen: wert.ueber === 'ip' } };
}

/**
 * Der Tausch über IPv4: `POST` an die Adresse aus `X-Heroic-Erkennen`, ohne
 * Cookie. Gibt das Token, das `anmelden` gegen eine lesende Sitzung tauscht,
 * oder nichts, wenn der Server den Browser auch dort nicht erkennt. Nur eine
 * Adresse `http(s)://…/api/erkennen`; die CSP erlaubt sie, wenn der Server
 * den zweiten Namen kennt.
 */
export async function erkennen(adresse: string): Promise<string | undefined> {
  let url: URL;
  try {
    url = new URL(adresse);
  } catch {
    return undefined;
  }
  if (!/^https?:$/.test(url.protocol) || url.pathname !== '/api/erkennen' || url.search || url.hash) return undefined;
  try {
    const antwort = await fetch(url, { method: 'POST', credentials: 'omit', cache: 'no-store', headers: KOPF, body: '{}' });
    if (antwort.status !== 200) return undefined;
    const wert: unknown = await antwort.json();
    return istObjekt(wert) && typeof wert.token === 'string' && TOKEN.test(wert.token) ? wert.token : undefined;
  } catch {
    return undefined;
  }
}

/** Höchstens so viele Knöpfe je Objekt; siehe docs/frontend.md, „Aktionsebenen“. */
const JE_OBJEKT = 4;

/** Die Knöpfe des Spielers; was nicht taugt, fällt weg. */
export async function knoepfe(gilt: (kennung: string) => boolean): Promise<Antwort<Knopf[]>> {
  const wert = await hole('knoepfe.json', 256 * 1024);
  if (wert === 'abgemeldet' || wert === 'fehlt') return wert;
  const roh = istObjekt(wert) && Array.isArray(wert.knoepfe) ? wert.knoepfe : [];
  const zahl = new Map<string, number>();
  const aus: Knopf[] = [];
  for (const k of roh) {
    if (!istObjekt(k) || typeof k.ebene !== 'string' || !gilt(k.ebene) || !istText(k.objekt, 64)) continue;
    if (typeof k.knopf !== 'string' || !gilt(k.knopf) || typeof k.on !== 'boolean' || !istObjekt(k.label)) continue;
    const de = istText(k.label.de, 32) ? k.label.de : undefined;
    const en = istText(k.label.en, 32) ? k.label.en : undefined;
    if (!de && !en) continue;
    const schluessel = objektSchluessel(k.ebene, k.objekt);
    const n = (zahl.get(schluessel) ?? 0) + 1;
    zahl.set(schluessel, n);
    if (n > JE_OBJEKT) continue;
    aus.push({ ebene: k.ebene, objekt: k.objekt, knopf: k.knopf, label: { de, en }, on: k.on });
  }
  return aus;
}

/** Die Inhalte einer Aktionsebene für den Spieler, bis `max` Byte. */
export const ebene = (mod: string, name: string, max: number): Promise<Antwort<unknown>> => hole(`ebenen/${mod}/${name}.json`, max);

/** Der Klick: 202 heisst angenommen, den neuen Zustand kennt erst das Plugin. */
export const aktion = (k: Knopf): Promise<number> => schicke('aktion', { ebene: k.ebene, objekt: k.objekt, knopf: k.knopf });

/** Ein Mitspieler auf ganzen Blöcken; `kopf` ist der SHA-256 seines Gesichts unter `koepfe/`, siehe 0114. */
export interface Mitspieler {
  uuid: string;
  name: string;
  x: number;
  z: number;
  kopf?: string;
}

/** Was `api/sicht.json` sagt: wann das Plugin schrieb, wo der Spieler steht, wenn online, und wen er sieht. */
export interface Sicht {
  jetzt?: number;
  ich?: { x: number; z: number; kopf?: string };
  spieler: Mitspieler[];
}

/** Höchstens so viele Mitspieler, so gross; siehe docs/plugin.md, „`Web:` und `sicht.json`“. */
const SICHT = { spieler: 256, bytes: 64 * 1024 };

/** Die Sicht des Spielers; was nicht taugt, fällt weg. */
export async function sicht(): Promise<Antwort<Sicht>> {
  const wert = await hole('sicht.json', SICHT.bytes);
  if (wert === 'abgemeldet' || wert === 'fehlt') return wert;
  if (!istObjekt(wert)) return { spieler: [] };
  // Ein Kopf nur mit genau 64 Zeichen hex, sonst baut die Karte keine Adresse daraus.
  const lage = (o: unknown) =>
    istObjekt(o) && istZahl(o.x) && istZahl(o.z) ? { x: o.x, z: o.z, ...(typeof o.kopf === 'string' && /^[0-9a-f]{64}$/.test(o.kopf) ? { kopf: o.kopf } : {}) } : undefined;
  const roh = Array.isArray(wert.spieler) ? wert.spieler.slice(0, SICHT.spieler) : [];
  const spieler: Mitspieler[] = [];
  for (const s of roh) {
    const l = lage(s);
    if (l && istObjekt(s) && istText(s.uuid, 36) && istText(s.name, 16)) spieler.push({ uuid: s.uuid, name: s.name, ...l });
  }
  return { jetzt: istZahl(wert.jetzt) ? wert.jetzt : undefined, ich: lage(wert.ich), spieler };
}

/** Ein Objekt einer Ebene als Schlüssel; JSON, damit kein Zeichen der ID zwei Paare verwechselt. */
export const objektSchluessel = (ebene: string, objekt: string): string => JSON.stringify([ebene, objekt]);
