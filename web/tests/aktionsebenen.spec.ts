import { expect, test, type Page, type Request } from '@playwright/test';
import { DEMO, HAFEN, STAEDTE, welt } from './ebenen-welt';

/** Eine Aktionsebene: Ihre Datei kommt je Spieler aus api/. */
const ROUTEN = { id: 'beispiel:routen', name: { de: 'Meine Routen' }, visible: true, order: 50, version: 'a', player: true };
const ROUTE = { id: 'route-a', type: 'pin', at: [20.5, -30.5], name: 'Meine Route', size: 'medium', color: '#3F7FE5' };
const SCHIFF = { ebene: 'beispiel:staedte', objekt: 'stadt-17', knopf: 'proficisci:schiff', label: { de: 'Schiff', en: 'Ship' } };
const TOKEN = 'gut.token';

/** Der Server des Renderers unter api/, mit einer Sitzung im Speicher statt im Cookie. */
interface Server {
  angemeldet: boolean;
  /** Ob das Schiff für den Spieler an ist. */
  schiff: boolean;
  /** Der Name aus `ich.json`; `null`: Die Datei fehlt, der Server sagt nur die UUID. */
  name: string | null;
  /** Nach so vielen ms setzt das Plugin den Klick um; ohne Angabe nie. */
  umsetzen?: number;
  /** Alle Anfragen unter api/, mit Methode, Pfad, Kopf und Körper. */
  anfragen: { methode: string; pfad: string; url: string; kopf: Record<string, string>; koerper: string | null }[];
}

async function server(page: Page, s: Partial<Server> = {}): Promise<Server> {
  const z: Server = { angemeldet: false, schiff: false, name: 'Steve', anfragen: [], ...s };
  const json = (wert: object) => ({ status: 200, contentType: 'application/json', body: JSON.stringify(wert) });
  await page.route('**/api/**', async (route) => {
    const r: Request = route.request();
    const pfad = new URL(r.url()).pathname.replace(/^\/api\//, '');
    z.anfragen.push({ methode: r.method(), pfad, url: r.url(), kopf: await r.allHeaders(), koerper: r.postData() });
    if (r.method() === 'POST') {
      // Wie der Server: ohne beide Köpfe kein POST.
      if (r.headers()['x-heroic-map'] !== '1' || r.headers()['content-type'] !== 'application/json') return route.fulfill({ status: 403 });
      if (pfad === 'anmelden') {
        if (r.headers()['authorization'] !== `Bearer ${TOKEN}`) return route.fulfill({ status: 401 });
        z.angemeldet = true;
        return route.fulfill({ status: 204 });
      }
      if (pfad === 'abmelden') {
        z.angemeldet = false;
        return route.fulfill({ status: 204 });
      }
      if (!z.angemeldet) return route.fulfill({ status: 401 });
      if (pfad === 'aktion') {
        if (z.umsetzen !== undefined) setTimeout(() => (z.schiff = !z.schiff), z.umsetzen);
        return route.fulfill({ status: 202 });
      }
      return route.fulfill({ status: 404 });
    }
    if (!z.angemeldet) return route.fulfill({ status: 401 });
    const uuid = '069a79f4-44e9-4726-a5be-fca90e38aaf5';
    if (pfad === 'ich') return route.fulfill(json(z.name === null ? { uuid } : { name: z.name, uuid }));
    if (pfad === 'knoepfe.json') return route.fulfill(json({ knoepfe: [{ ...SCHIFF, on: z.schiff }] }));
    if (pfad === 'ebenen/beispiel/routen.json') return route.fulfill(json({ objects: z.schiff ? [ROUTE] : [] }));
    return route.fulfill({ status: 404 });
  });
  return z;
}

const nurStaedte = { liste: () => [STAEDTE], datei: (name: string) => (name === 'staedte' ? { objects: [HAFEN] } : undefined) };
const mitRouten = { ...nurStaedte, liste: () => [STAEDTE, ROUTEN] };

/** Dieser Browser war zuletzt angemeldet: So fragt die Seite auch ohne Aktionsebene nach Knöpfen. */
const warAngemeldet = (page: Page) => page.addInitScript(() => localStorage.setItem('angemeldet:/', '1'));

/** Öffnet die Tafel der Hafenstadt und gibt ihren Knopf zurück. */
async function schiffKnopf(page: Page) {
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const knopf = page.locator('.tafel .tafel-knopf');
  await expect(knopf).toHaveText(/^(Schiff|Ship)$/);
  return knopf;
}

test('ein Gast ohne Aktionsebene fragt api/ nie; mit Aktionsebene einmal nach der Sitzung, sieht den Hinweis zum Anmelden und die Ebene nicht', async ({ page }) => {
  const ohne = await server(page);
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon[title="Hafenstadt"]')).toHaveCount(1);
  await page.waitForTimeout(300);
  expect(ohne.anfragen).toEqual([]);
  await expect(page.locator('.ebenen-konto')).toBeHidden();

  await page.unrouteAll();
  const mit = await server(page);
  await welt(page, mitRouten);
  await page.reload();
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen: im Spiel /karte anmelden');
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Städte|Towns/]);
  expect(mit.anfragen.map((a) => a.pfad)).toEqual(['ich']);
});

test('ohne Server des Renderers, etwa wenn ein statischer Server die index.html liefert, zeigt die Seite keinen Hinweis zum Anmelden', async ({ page }) => {
  await welt(page, mitRouten);
  await page.route('**/api/**', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '<!doctype html>' }));
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-liste label')).toHaveCount(1);
  await expect(page.locator('.ebenen-konto')).toBeHidden();
});

test('der Anmeldelink: Das Token verlässt sofort die Adresse, geht nur im Kopf Authorization an api/anmelden; danach Name, Aktionsebene, Abmelden', async ({ page }) => {
  const z = await server(page);
  await welt(page, mitRouten);
  // Was die Seite in die Adresse schreibt, von Anfang an.
  await page.addInitScript(() => {
    const echt = history.replaceState.bind(history);
    (window as unknown as { adressen: string[] }).adressen = [];
    history.replaceState = (zustand, titel, url) => {
      (window as unknown as { adressen: string[] }).adressen.push(String(url));
      echt(zustand, titel, url);
    };
  });
  // Jede Anfrage der Seite, mit Adresse ohne Fragment, wie der Server sie sieht.
  const alle: string[] = [];
  page.on('request', (r) => alle.push(r.url()));
  await page.goto(`${DEMO}#anmelden=${TOKEN}`);
  await expect(page.locator('.ebenen-konto')).toContainText('Angemeldet als Steve');
  // Die Liste öffnet sich nach dem Anmelden von selbst.
  await expect(page.locator('.ebenen')).toHaveAttribute('open', '');
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Städte|Towns/, /Meine Routen/]);
  expect(page.url()).not.toContain('anmelden');
  const adressen = await page.evaluate(() => (window as unknown as { adressen: string[] }).adressen);
  expect(adressen.join(' ')).not.toContain(TOKEN);
  const anmelden = z.anfragen.find((a) => a.pfad === 'anmelden')!;
  expect(anmelden.kopf['authorization']).toBe(`Bearer ${TOKEN}`);
  expect(z.anfragen.every((a) => !a.url.includes(TOKEN) && !a.koerper?.includes(TOKEN))).toBe(true);
  expect(alle.filter((u) => u.includes(TOKEN))).toEqual([]);

  await page.locator('.ebenen-abmelden').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen: im Spiel /karte anmelden');
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Städte|Towns/]);
  expect(z.anfragen.some((a) => a.methode === 'POST' && a.pfad === 'abmelden')).toBe(true);
});

test('ein abgelaufener oder benutzter Anmeldelink sagt es, und die Seite bleibt abgemeldet', async ({ page }) => {
  await server(page);
  await welt(page, mitRouten);
  await page.goto(`${DEMO}#anmelden=alt.token`);
  await expect(page.locator('.ebenen-meldung')).toHaveText(/abgelaufen oder schon benutzt/);
  await expect(page.locator('.ebenen-liste label')).toHaveCount(1);
  expect(page.url()).not.toContain('anmelden');
});

test('ein Anmeldelink in einem Tab, in dem die Karte schon offen ist, meldet ohne Neuladen an; ohne ich.json beim Server ohne Namen', async ({ page }) => {
  await server(page, { name: null });
  await welt(page, mitRouten);
  await page.goto(DEMO);
  await expect(page.locator('.ebenen-konto')).toContainText('Anmelden');
  await page.evaluate((t) => (location.hash = `anmelden=${t}`), TOKEN);
  await expect(page.locator('.ebenen-konto span')).toHaveText('Angemeldet');
  await expect(page.locator('.ebenen-liste label')).toHaveCount(2);
  expect(page.url()).not.toContain('anmelden');
});

test('ein Knopf in der Tafel: aria-pressed nach dem Zustand, beim Klick gesperrt, bis der neue Zustand kommt; dann zeichnet die Aktionsebene neu; ein Doppelklick schickt einen POST', async ({ page }) => {
  const z = await server(page, { angemeldet: true, umsetzen: 500 });
  await welt(page, mitRouten);
  await page.goto(DEMO);
  const knopf = await schiffKnopf(page);
  await expect(knopf).toHaveAttribute('aria-pressed', 'false');
  await expect(page.locator('.nadel-icon[title="Meine Route"]')).toHaveCount(0);
  await knopf.dblclick();
  await expect(knopf).toHaveAttribute('aria-busy', 'true');
  await expect(knopf).toHaveAttribute('aria-pressed', 'true');
  await expect(knopf).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('.tafel-hinweis')).toHaveText('');
  await expect(page.locator('.nadel-icon[title="Meine Route"]')).toHaveCount(1);
  const klicks = z.anfragen.filter((a) => a.pfad === 'aktion');
  expect(klicks).toHaveLength(1);
  expect(JSON.parse(klicks[0]!.koerper!)).toEqual({ ebene: SCHIFF.ebene, objekt: SCHIFF.objekt, knopf: SCHIFF.knopf });
  // Der Fokus bleibt auf dem Knopf, auch wenn sein Zustand wechselt.
  await expect(knopf).toBeFocused();
});

test('kommt nach einem Klick 3 s kein neuer Zustand, wird der Knopf mit dem alten frei und sagt es', async ({ page }) => {
  const z = await server(page, { angemeldet: true });
  await warAngemeldet(page);
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  const knopf = await schiffKnopf(page);
  const start = Date.now();
  await knopf.click();
  await expect(knopf).toHaveAttribute('aria-busy', 'true');
  await expect(knopf).toHaveAttribute('aria-busy', 'false', { timeout: 6000 });
  expect(Date.now() - start).toBeGreaterThanOrEqual(2900);
  await expect(knopf).toHaveAttribute('aria-pressed', 'false');
  await expect(page.locator('.tafel-hinweis')).toHaveText('noch keine Antwort');
  // Nach 0,3, 0,8, 1,5 und 3 s gefragt, dazu einmal beim Laden.
  expect(z.anfragen.filter((a) => a.pfad === 'knoepfe.json')).toHaveLength(5);
});

test('ein Objekt ohne Tafel bekommt mit einem Knopf eine nur mit dem Knopf; abgemeldet ist sie wieder weg', async ({ page }) => {
  const z = await server(page, { angemeldet: true });
  const ohne = { ...HAFEN, panel: undefined };
  await warAngemeldet(page);
  await welt(page, { ...nurStaedte, datei: (name: string) => (name === 'staedte' ? { objects: [ohne] } : undefined) });
  await page.goto(DEMO);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  await expect(nadel).toHaveAttribute('tabindex', '0');
  await nadel.click();
  await expect(page.locator('.tafel .tafel-knopf')).toHaveText(/^(Schiff|Ship)$/);
  await expect(page.locator('.tafel .tafel-titel')).toHaveCount(0);
  // Im Spiel /karte abmelden: Die nächste Frage bekommt 401.
  z.angemeldet = false;
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await expect(page.locator('.ebenen-konto')).toHaveText('');
  await expect(page.locator('.nadel-icon[title="Hafenstadt"]')).not.toHaveAttribute('tabindex', /.*/);
});
