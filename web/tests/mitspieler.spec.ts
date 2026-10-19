import { expect, test } from '@playwright/test';
import { server, TAUSCH } from './api-server';
import { DEMO, HAFEN, STAEDTE, welt } from './ebenen-welt';

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
