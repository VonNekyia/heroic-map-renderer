import { expect, test, type Locator, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { server, TAUSCH } from './api-server';
import { DEMO, HAFEN, png, STAEDTE, welt } from './ebenen-welt';

const nurStaedte = { liste: () => [STAEDTE], datei: (name: string) => (name === 'staedte' ? { objects: [HAFEN] } : undefined) };
const ALEX = { uuid: '5f3c2b6e-0d4a-4c1e-9b7a-2e5f6a1d3c90', name: 'Alex<b>!</b>', x: 20, z: -30 };
/** Eine Sicht des Plugins, mit `jetzt` in Sekunden. */
const sichtMit = (jetzt: number, alex = ALEX) => ({ jetzt, ich: { x: 35, z: -15 }, spieler: [alex] });

test('über die IP erkannt: „Erkannt als … (über die IP)“, kein Abmelden, Knöpfe gesperrt mit Hinweis, ein Klick schickt nichts', async ({ page }) => {
  const z = await server(page, { angemeldet: true, ueber: 'ip' });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto span')).toHaveText('Erkannt als Steve (über die IP)');
  await expect(page.locator('.ebenen-konto .ebenen-klein')).toHaveText('Nicht du? Im Spiel /karte anmelden');
  await expect(page.locator('.ebenen-abmelden')).toHaveCount(0);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const knopf = page.locator('.tafel .tafel-knopf');
  await expect(knopf).toHaveAttribute('aria-disabled', 'true');
  await expect(page.locator('.tafel-hinweis')).toHaveText('Zum Schalten im Spiel /karte anmelden');
  // Wie ein Mensch: aria-disabled hält den Klick nicht auf, die Seite muss es selbst tun.
  await knopf.click({ force: true });
  await page.waitForTimeout(300);
  expect(z.anfragen.filter((a) => a.pfad === 'aktion')).toEqual([]);
});

test('sagt der Server beim Klick 403, steht derselbe Hinweis am Knopf', async ({ page }) => {
  const z = await server(page, { angemeldet: true });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const knopf = page.locator('.tafel .tafel-knopf');
  await expect(knopf).toHaveAttribute('aria-disabled', 'false');
  // Das Plugin hat ihn inzwischen auf nur lesen gesetzt, etwa nach /karte abmelden auf einem anderen Gerät.
  z.ueber = 'ip';
  await knopf.click();
  await expect(page.locator('.tafel-hinweis')).toHaveText('Zum Schalten im Spiel /karte anmelden');
});

test('ein Gast über IPv6 ohne zweiten Namen liest, warum die Karte ihn nicht erkennt', async ({ page }) => {
  await server(page, { kopf: { 'x-heroic-ip': '6' } });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Über IPv6 erkennt die Karte dich nicht. Für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
});

test('über IPv6 mit X-Heroic-Erkennen: Tausch über IPv4 ohne Cookie, dann eine lesende Sitzung; erkennt der Server dort nicht, bleibt der Hinweis', async ({ page }) => {
  const origin = 'http://127.0.0.1:' + new URL(test.info().project.use.baseURL!).port;
  // Ein Cookie der Seite: Der Tausch muss ohne es gehen.
  await page.context().addCookies([{ name: 'probe', value: '1', url: origin }]);
  const z = await server(page, { kopf: { 'x-heroic-erkennen': `${origin}/api/erkennen` }, erkenntUeberV4: true });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto span')).toHaveText('Erkannt als Steve (über die IP)');
  const tausch = z.anfragen.find((a) => a.pfad === 'erkennen')!;
  expect(tausch.methode).toBe('POST');
  expect(tausch.koerper).toBe('{}');
  expect(tausch.kopf['cookie']).toBeUndefined();
  expect(z.anfragen.find((a) => a.pfad === 'anmelden')!.kopf['authorization']).toBe(`Bearer ${TAUSCH}`);

  await page.unrouteAll();
  await server(page, { kopf: { 'x-heroic-erkennen': `${origin}/api/erkennen` }, erkenntUeberV4: false });
  await welt(page, nurStaedte);
  await page.reload();
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
});

test('X-Heroic-Erkennen geht nur an http(s)://…/api/erkennen; eine andere Adresse fragt die Karte nicht', async ({ page }) => {
  const origin = 'http://127.0.0.1:' + new URL(test.info().project.use.baseURL!).port;
  const z = await server(page, { kopf: { 'x-heroic-erkennen': `${origin}/api/anmelden` }, erkenntUeberV4: true });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
  expect(z.anfragen.map((a) => a.pfad)).toEqual(['ich']);
});

test('Mitspieler: erkannt alle 2 s aus api/sicht.json, der Betrachter hervorgehoben, Namen als Text; eine Figur wandert, statt neu zu entstehen', async ({ page }) => {
  const z = await server(page, { angemeldet: true, ueber: 'ip', sicht: sichtMit(1000) });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-liste input[data-id="heroicmap:mitspieler"]')).toBeChecked();
  const figuren = page.locator('.mitspieler');
  await expect(figuren).toHaveCount(2);
  await expect(page.locator('.mitspieler.ich .mitspieler-name')).toHaveText('Steve');
  const alex = page.locator('.mitspieler:not(.ich)');
  // Der Name nur als Text, auch mit Zeichen wie Markup.
  await expect(alex.locator('.mitspieler-name')).toHaveText('Alex<b>!</b>');
  expect(await alex.locator('b').count()).toBe(0);
  const vorher = await alex.boundingBox();
  await alex.evaluate((e) => ((e as HTMLElement & { probe?: number }).probe = 1));
  // Nur x ändert sich: Im Blick aus Südost wandert die Figur dann nach rechts unten.
  z.sicht = sichtMit(1001, { ...ALEX, x: 40 });
  await expect.poll(async () => (await alex.boundingBox())!.x, { timeout: 4000 }).not.toBe(vorher!.x);
  // Dasselbe Element, nur verschoben.
  expect(await alex.evaluate((e) => (e as HTMLElement & { probe?: number }).probe)).toBe(1);
  expect(z.anfragen.filter((a) => a.pfad === 'sicht.json').length).toBeGreaterThanOrEqual(2);
});

test('Mitspieler: ändert sich jetzt 15 s nicht, verschwinden sie; aus in der Liste fragt die Karte nicht mehr', async ({ page }) => {
  await page.clock.install();
  const z = await server(page, { angemeldet: true, ueber: 'ip', sicht: sichtMit(1000) });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await expect(page.locator('.mitspieler')).toHaveCount(2);
  await page.clock.runFor(10_000);
  await expect(page.locator('.mitspieler')).toHaveCount(2);
  await page.clock.runFor(8000);
  await expect(page.locator('.mitspieler')).toHaveCount(0);
  // Ein neues jetzt bringt sie zurück.
  z.sicht = sichtMit(1020);
  await page.clock.runFor(2000);
  await expect(page.locator('.mitspieler')).toHaveCount(2);
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen-liste input[data-id="heroicmap:mitspieler"]').uncheck();
  await expect(page.locator('.mitspieler')).toHaveCount(0);
  const zahl = z.anfragen.filter((a) => a.pfad === 'sicht.json').length;
  await page.clock.runFor(6000);
  expect(z.anfragen.filter((a) => a.pfad === 'sicht.json')).toHaveLength(zahl);
});

test('Mitspieler auch ohne layers.json; ein Gast fragt api/sicht.json nie', async ({ page }) => {
  await page.route('**/tiles-demo/layers.json', (r) => r.fulfill({ status: 404 }));
  await server(page, { angemeldet: true, ueber: 'ip', sicht: sichtMit(1000) });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Mitspieler/]);
  await expect(page.locator('.mitspieler')).toHaveCount(2);

  await page.unrouteAll();
  await page.route('**/tiles-demo/layers.json', (r) => r.fulfill({ status: 404 }));
  const gast = await server(page);
  await page.reload();
  await page.waitForTimeout(2500);
  expect(gast.anfragen.map((a) => a.pfad)).toEqual(['ich']);
  await expect(page.locator('.mitspieler')).toHaveCount(0);
});

/** Der Name eines Kopfs: der SHA-256 seiner Bytes. */
const hash = (daten: Buffer) => createHash('sha256').update(daten).digest('hex');
const GESICHT = png(8, 8);
/** Liefert unter koepfe/ je Hash die Bytes oder einen Status; zählt die Anfragen je Pfad. */
async function koepfe(page: Page, je: Record<string, Buffer | number>): Promise<string[]> {
  const geholt: string[] = [];
  await page.route('**/tiles-demo/koepfe/**', (route) => {
    const name = new URL(route.request().url()).pathname.split('/').pop()!.replace(/\.png$/, '');
    geholt.push(name);
    const antwort = je[name];
    return Buffer.isBuffer(antwort) ? route.fulfill({ body: antwort, contentType: 'image/png' }) : route.fulfill({ status: antwort ?? 404 });
  });
  return geholt;
}
/** Die Farbe von Pixel (x, y) des Gesichts, in der Mitte seines Felds auf der Leinwand. */
const pixelIm = (canvas: Locator, x: number, y: number) =>
  canvas.evaluate((c: HTMLCanvasElement, [x, y]) => {
    const k = c.width / 8;
    return [...c.getContext('2d')!.getImageData(x! * k + k / 2, y! * k + k / 2, 1, 1).data];
  }, [x, y]);

for (const [dpr, k] of [[1, 2], [1.25, 2], [2, 4]] as const) {
  test.describe(`bei devicePixelRatio ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr });

    test(`nach 0114 zeigt ein Mitspieler mit kopf sein Gesicht Pixel für Pixel, je Pixel ${k} Pixel des Geräts; der Betrachter mit gelbem Rahmen; ein neuer kopf zeichnet neu`, async ({ page }) => {
      await page.addInitScript(() => document.addEventListener('securitypolicyviolation', (e) => ((window as unknown as { csp: string[] }).csp ??= []).push(e.violatedDirective)));
      const kopf = hash(GESICHT);
      const anders = 'f'.repeat(64);
      await koepfe(page, { [kopf]: GESICHT, [anders]: GESICHT });
      const z = await server(page, { angemeldet: true, ueber: 'ip', sicht: { jetzt: 1000, ich: { x: 35, z: -15, kopf }, spieler: [{ ...ALEX, kopf }] } });
      await welt(page, nurStaedte);
      await page.goto(DEMO);
      const alex = page.locator('.mitspieler:not(.ich) canvas.mitspieler-kopf');
      await expect(alex).toHaveCount(1);
      await expect(page.locator('.mitspieler-punkt')).toHaveCount(0);
      const [breit, hoch, aufDemSchirm] = await alex.evaluate((c: HTMLCanvasElement) => [c.width, c.height, c.getBoundingClientRect().width]);
      expect([breit, hoch]).toEqual([8 * k, 8 * k]);
      // Das Layout rundet auf 1/64 px.
      expect(Math.abs(aufDemSchirm - (8 * k) / dpr)).toBeLessThanOrEqual(1 / 32);
      for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) expect(await pixelIm(alex, x, y), `${x}, ${y}`).toEqual([(x * 7) % 256, (y * 3) % 256, 128, 255]);
      const ich = page.locator('.mitspieler.ich canvas.mitspieler-kopf');
      expect(await ich.evaluate((c) => [getComputedStyle(c).outlineStyle, getComputedStyle(c).outlineWidth, getComputedStyle(c).outlineColor])).toEqual(['solid', '1px', 'rgb(255, 216, 74)']);
      await expect(page.locator('.mitspieler:not(.ich) .mitspieler-name')).toHaveText('Alex<b>!</b>');
      // Ein anderer Skin: dieselbe Figur, ein neues Gesicht.
      await alex.evaluate((c) => ((c as HTMLCanvasElement & { probe?: number }).probe = 1));
      z.sicht = { jetzt: 1001, ich: { x: 35, z: -15, kopf }, spieler: [{ ...ALEX, kopf: anders }] };
      await expect.poll(() => page.locator('.mitspieler:not(.ich) canvas.mitspieler-kopf').evaluate((c) => (c as HTMLCanvasElement & { probe?: number }).probe ?? 0), { timeout: 5000 }).toBe(0);
      expect(await page.evaluate(() => (window as unknown as { csp?: string[] }).csp ?? [])).toEqual([]);
    });
  });
}

test('nach 0114 bleibt ein Mitspieler ein Punkt, wenn kopf nicht 64 Zeichen hex hat (ohne Anfrage), 404 gibt, zu gross ist oder nicht 8 × 8; ein Kopf wird einmal geholt', async ({ page }) => {
  const gut = hash(GESICHT);
  const fehlt = '0'.repeat(64);
  const gross = '1'.repeat(64);
  const sechzehn = '2'.repeat(64);
  const geholt = await koepfe(page, { [gut]: GESICHT, [gross]: Buffer.concat([GESICHT, Buffer.alloc(17 * 1024)]), [sechzehn]: png(16, 16) });
  const mit = (name: string, x: number, kopf: string) => ({ uuid: `${name.padEnd(8, '0')}-0000-4000-8000-000000000000`.slice(0, 36), name, x, z: -30, kopf });
  const spieler = [mit('Gross', 20, gross), mit('Falsch', 24, 'A'.repeat(64)), mit('Kurz', 28, 'a'.repeat(63)), mit('Fehlt', 32, fehlt), mit('Sechzehn', 36, sechzehn), mit('Gut', 40, gut)];
  await server(page, { angemeldet: true, ueber: 'ip', sicht: { jetzt: 1000, spieler } });
  await welt(page, nurStaedte);
  await page.goto(DEMO);
  await expect(page.locator('.mitspieler')).toHaveCount(6);
  const figur = (name: string) => page.locator('.mitspieler', { has: page.locator('.mitspieler-name', { hasText: new RegExp(`^${name}$`) }) });
  await expect(figur('Gut').locator('canvas.mitspieler-kopf')).toHaveCount(1);
  for (const name of ['Gross', 'Falsch', 'Kurz', 'Fehlt', 'Sechzehn']) await expect(figur(name).locator('.mitspieler-punkt'), name).toHaveCount(1);
  // Über mehrere Abrufe von sicht.json bleibt es je Kopf bei einer Anfrage; die falschen fragt die Karte nie.
  await page.waitForTimeout(4500);
  expect([...geholt].sort()).toEqual([gut, fehlt, gross, sechzehn].sort());
});
