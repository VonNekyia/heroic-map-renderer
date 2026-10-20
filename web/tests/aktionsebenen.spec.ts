import { expect, test, type Page } from '@playwright/test';
import { SCHIFF, server, TOKEN } from './api-server';
import { DEMO, HAFEN, STAEDTE, welt } from './ebenen-welt';

/** Eine Aktionsebene: Ihre Datei kommt je Spieler aus api/. */
const ROUTEN = { id: 'beispiel:routen', name: { de: 'Meine Routen' }, visible: true, order: 50, version: 'a', player: true };

const nurStaedte = { liste: () => [STAEDTE], datei: (name: string) => (name === 'staedte' ? { objects: [HAFEN] } : undefined) };
const mitRouten = { ...nurStaedte, liste: () => [STAEDTE, ROUTEN] };

/** Öffnet die Tafel der Hafenstadt und gibt ihren Knopf zurück. */
async function schiffKnopf(page: Page) {
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const knopf = page.locator('.tafel .tafel-knopf');
  await expect(knopf).toHaveText(/^(Schiff|Ship)$/);
  return knopf;
}

test('ein Gast fragt beim Laden einmal api/ich, bekommt 204, sieht den Hinweis zum Anmelden und keine Aktionsebene, und fragt sonst nichts unter api/', async ({ page }) => {
  for (const welt_ of [nurStaedte, mitRouten]) {
    await page.unrouteAll();
    const z = await server(page);
    await welt(page, welt_);
    await page.goto(DEMO);
    await page.locator('.ebenen summary').click();
    await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
    await expect(page.locator('.ebenen-liste label')).toHaveText([/Städte|Towns/]);
    await page.waitForTimeout(2500);
    expect(z.anfragen.map((a) => a.pfad)).toEqual(['ich']);
  }
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
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Mitspieler/, /Städte|Towns/, /Meine Routen/]);
  expect(page.url()).not.toContain('anmelden');
  const adressen = await page.evaluate(() => (window as unknown as { adressen: string[] }).adressen);
  expect(adressen.join(' ')).not.toContain(TOKEN);
  const anmelden = z.anfragen.find((a) => a.pfad === 'anmelden')!;
  expect(anmelden.kopf['authorization']).toBe(`Bearer ${TOKEN}`);
  expect(z.anfragen.every((a) => !a.url.includes(TOKEN) && !a.koerper?.includes(TOKEN))).toBe(true);
  expect(alle.filter((u) => u.includes(TOKEN))).toEqual([]);

  await page.locator('.ebenen-abmelden').click();
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
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
  await expect(page.locator('.ebenen-liste label')).toHaveCount(3);
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
  await expect(page.locator('.ebenen-konto')).toHaveText('Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden');
  await expect(page.locator('.nadel-icon[title="Hafenstadt"]')).not.toHaveAttribute('tabindex', /.*/);
});

test('nach 0113 bleibt eine Aktionsebene, die ein Gast nicht sieht, beim Umstellen an ihrem Platz in der gemerkten Reihe; angemeldet steht sie, wo sie war', async ({ page }) => {
  await server(page);
  const ebene = (id: string, order: number) => ({ ...STAEDTE, id: `beispiel:${id}`, name: { de: id.toUpperCase() }, order });
  await welt(page, { liste: () => [ebene('a', 3), { ...ROUTEN, order: 2 }, ebene('c', 1)], datei: () => ({ objects: [] }) });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen-liste label')).toHaveText([/A/, /C/]);
  // C tauscht mit A, der nächsten sichtbaren; die Routen dazwischen bleiben in der Mitte.
  await page.getByRole('button', { name: 'C nach oben' }).click();
  await expect(page.locator('.ebenen-liste label')).toHaveText([/C/, /A/]);
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('ebenen-reihe:/tiles-demo')!) as unknown)).toEqual(['beispiel:c', 'beispiel:routen', 'beispiel:a']);
  await page.goto(`${DEMO}#anmelden=${TOKEN}`);
  await expect(page.locator('.ebenen-konto')).toContainText('Angemeldet als Steve');
  await expect(page.locator('.ebenen-liste label')).toHaveText([/Mitspieler/, /C/, /Meine Routen/, /A/]);
});
