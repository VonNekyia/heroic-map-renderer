/**
 * Der Server des Renderers unter api/, im Speicher nachgebaut nach
 * docs/benutzung/server.md, „Aktionsebenen“ und „Erkennung per IP“: Sitzung,
 * Erkennung über die IP, Tausch über IPv4, Knöpfe, eine Aktionsebene und die
 * Sicht. Für aktionsebenen.spec.ts und mitspieler.spec.ts.
 */
import type { Page, Request } from '@playwright/test';

export const ROUTE = { id: 'route-a', type: 'pin', at: [20.5, -30.5], name: 'Meine Route', size: 'medium', color: '#3F7FE5' };
export const SCHIFF = { ebene: 'beispiel:staedte', objekt: 'stadt-17', knopf: 'proficisci:schiff', label: { de: 'Schiff', en: 'Ship' } };
export const TOKEN = 'gut.token';
export const TAUSCH = 'tausch.token';
export const UUID = '069a79f4-44e9-4726-a5be-fca90e38aaf5';

export interface Server {
  /** Eine Sitzung, oder erkannt über die IP. */
  angemeldet: boolean;
  /** `ip`: über die IP oder eine lesende Sitzung erkannt, dann nur lesen. */
  ueber: 'sitzung' | 'ip';
  /** Ob das Schiff für den Spieler an ist. */
  schiff: boolean;
  /** Der Name aus `ich.json`; `null`: Die Datei fehlt, der Server sagt nur die UUID. */
  name: string | null;
  /** Nach so vielen ms setzt das Plugin den Klick um; ohne Angabe nie. */
  umsetzen?: number;
  /** Köpfe an `204` und `401` von `api/ich`, etwa `x-heroic-ip: 6`. */
  kopf?: Record<string, string>;
  /** Ob `api/erkennen` den Browser erkennt und das Token gibt. */
  erkenntUeberV4: boolean;
  /** Was `api/sicht.json` liefert; ohne Angabe fehlt die Datei. */
  sicht?: object;
  /** Alle Anfragen unter api/, mit Methode, Pfad, Kopf und Körper. */
  anfragen: { methode: string; pfad: string; url: string; kopf: Record<string, string>; koerper: string | null }[];
}

export async function server(page: Page, s: Partial<Server> = {}): Promise<Server> {
  const z: Server = { angemeldet: false, ueber: 'sitzung', schiff: false, name: 'Steve', erkenntUeberV4: false, anfragen: [], ...s };
  const json = (wert: object) => ({ status: 200, contentType: 'application/json', body: JSON.stringify(wert) });
  await page.route('**/api/**', async (route) => {
    const r: Request = route.request();
    const pfad = new URL(r.url()).pathname.replace(/^\/api\//, '');
    z.anfragen.push({ methode: r.method(), pfad, url: r.url(), kopf: await r.allHeaders(), koerper: r.postData() });
    if (r.method() === 'POST') {
      // Wie der Server: ohne beide Köpfe kein POST.
      if (r.headers()['x-heroic-map'] !== '1' || r.headers()['content-type'] !== 'application/json') return route.fulfill({ status: 403 });
      if (pfad === 'erkennen') return route.fulfill(z.erkenntUeberV4 ? json({ token: TAUSCH }) : { status: 204 });
      if (pfad === 'anmelden') {
        const bearer = r.headers()['authorization'];
        if (bearer === `Bearer ${TAUSCH}` && z.erkenntUeberV4) [z.angemeldet, z.ueber] = [true, 'ip'];
        else if (bearer === `Bearer ${TOKEN}`) [z.angemeldet, z.ueber] = [true, 'sitzung'];
        else return route.fulfill({ status: 401 });
        return route.fulfill({ status: 204 });
      }
      if (pfad === 'abmelden') {
        if (z.ueber === 'sitzung') z.angemeldet = false;
        return route.fulfill({ status: 204 });
      }
      if (!z.angemeldet) return route.fulfill({ status: 401 });
      if (pfad === 'aktion') {
        if (z.ueber === 'ip') return route.fulfill({ status: 403 });
        if (z.umsetzen !== undefined) setTimeout(() => (z.schiff = !z.schiff), z.umsetzen);
        return route.fulfill({ status: 202 });
      }
      return route.fulfill({ status: 404 });
    }
    if (pfad === 'ich') {
      if (!z.angemeldet) return route.fulfill({ status: 204, headers: z.kopf ?? {} });
      return route.fulfill(json(z.name === null ? { uuid: UUID, ueber: z.ueber } : { name: z.name, uuid: UUID, ueber: z.ueber }));
    }
    if (!z.angemeldet) return route.fulfill({ status: 401 });
    if (pfad === 'knoepfe.json') return route.fulfill(json({ knoepfe: [{ ...SCHIFF, on: z.schiff }] }));
    if (pfad === 'ebenen/beispiel/routen.json') return route.fulfill(json({ objects: z.schiff ? [ROUTE] : [] }));
    if (pfad === 'sicht.json') return route.fulfill(json(z.sicht ?? { spieler: [] }));
    return route.fulfill({ status: 404 });
  });
  return z;
}
