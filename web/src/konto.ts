/**
 * Anmeldung und Sitzung am Server des Renderers, unter `api/` neben der
 * Seite: Anmelde-Token aus dem Fragment, Abmelden, wer angemeldet ist, die
 * Knöpfe und der Klick. Die Sitzung trägt ein Cookie mit HttpOnly; die Seite
 * sieht es nie. Siehe docs/frontend.md, „Aktionsebenen“.
 */
import { istObjekt, istText } from './pruefen';

/** Ein Knopf an einem Objekt, mit dem Zustand für den angemeldeten Spieler. */
export interface Knopf {
  ebene: string;
  objekt: string;
  knopf: string;
  label: { de?: string; en?: string };
  on: boolean;
}

/** Wer angemeldet ist. */
export interface Spieler {
  name: string;
  uuid: string;
}

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

export async function ich(): Promise<Antwort<Spieler>> {
  const wert = await hole('ich', 4096);
  if (wert === 'abgemeldet' || wert === 'fehlt') return wert;
  return istObjekt(wert) && istText(wert.name, 64) && istText(wert.uuid, 36) ? { name: wert.name, uuid: wert.uuid } : 'fehlt';
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

/** Ein Objekt einer Ebene als Schlüssel; JSON, damit kein Zeichen der ID zwei Paare verwechselt. */
export const objektSchluessel = (ebene: string, objekt: string): string => JSON.stringify([ebene, objekt]);
