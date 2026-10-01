import { expect, test, type Locator, type Page } from '@playwright/test';
import { deflateSync } from 'node:zlib';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
const DEMO = '/?tiles=/tiles-demo';

/**
 * Höhen für den Demobaum, je 4 × 4 Spalten: eben auf Y 0, dazu eine Säule
 * bis Y 5 in der Zelle der Spalten 32 bis 35 und -16 bis -13, in einer
 * negativen Region. Ein falscher Platz in der Höhenkarte fiele so auf.
 */
async function welt(page: Page, mehr: object = {}): Promise<void> {
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    const hoehen = { heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    await route.fulfill({ response, json: { ...info, ...hoehen, ...mehr } });
  });
  await page.route('**/tiles-demo/heights/*.bin', async (route) => {
    const karte = new Int16Array(128 * 128);
    if (route.request().url().endsWith('/0.-1.bin')) karte[(-4 + 128) * 128 + 8] = 5;
    await route.fulfill({ body: deflateSync(Buffer.from(karte.buffer)) });
  });
}

/**
 * Die Mitte eines Pixels der feinsten Stufe auf dem Bildschirm. Der
 * Demobaum passt auf Stufe 2 ins Fenster, ein Pixel der Kachel ist dann
 * einer des Bildschirms.
 */
async function bildschirm(page: Page, u: number, v: number): Promise<[number, number]> {
  const kachel = await page.locator('img[src$="/2/0/0.webp"]').boundingBox();
  if (!kachel) throw new Error('Kachel 2/0/0 nicht zu sehen');
  return [kachel.x + u + 0.5, kachel.y + v + 0.5];
}

/** Die Zoomstufen, aus denen die sichtbaren Kacheln stammen. */
async function tileZooms(page: Page): Promise<number[]> {
  const quellen = await page.locator('img.leaflet-tile-loaded').evaluateAll((bilder) =>
    bilder.map((bild) => (bild as HTMLImageElement).src),
  );
  const stufen = quellen
    .map((src) => /\/tiles-demo\/(\d+)\//.exec(src)?.[1])
    .filter((z): z is string => z !== undefined)
    .map(Number);
  return [...new Set(stufen)].sort((a, b) => a - b);
}

/**
 * Um welchen Faktor Leaflet die vorhandenen Kacheln vergrössert.
 *
 * Jenseits der feinsten gerenderten Stufe gibt es keine neuen Kacheln
 * mehr; Leaflet skaliert dann den Kachelcontainer.
 */
async function tileStretch(page: Page): Promise<number> {
  const transform = await page
    .locator('.leaflet-tile-container')
    .first()
    .evaluate((element) => getComputedStyle(element).transform);
  return Number(/^matrix\(([\d.]+)/.exec(transform)?.[1] ?? 1);
}

/**
 * Klickt einen Zoomknopf und wartet, bis Leaflet fertig ist.
 *
 * Ein zweiter Klick mitten in der Animation geht verloren. Leaflet hängt
 * für ihre Dauer `leaflet-zoom-anim` an die Kartenebene — erst kommt die
 * Klasse, dann ist sie wieder weg, und dann ist es sicher.
 */
async function zoomClick(page: Page, control: Locator): Promise<void> {
  const pane = page.locator('.leaflet-map-pane');
  await control.click();
  await expect(pane).toHaveClass(/leaflet-zoom-anim/);
  await expect(pane).not.toHaveClass(/leaflet-zoom-anim/);
}

test('die Karte laedt Kacheln, ohne zu meckern', async ({ page }) => {
  const fehler: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') fehler.push(m.text());
  });
  page.on('pageerror', (e) => fehler.push(e.message));

  await page.goto(DEMO);

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await tileZooms(page)).not.toEqual([]);
  expect(fehler).toEqual([]);
  // Ohne Höhen in map.json keine Koordinaten.
  await expect(page.locator('.koordinaten')).toHaveCount(0);
});

test('die Karte läuft unter strengen Headern', async ({ page }) => {
  await page.addInitScript(() => {
    const verletzt: string[] = [];
    (window as unknown as { verletzt: string[] }).verletzt = verletzt;
    document.addEventListener('securitypolicyviolation', (e) =>
      verletzt.push(`${e.effectiveDirective} ${e.blockedURI}`),
    );
  });
  await welt(page);
  const antwort = await page.goto(DEMO);
  expect(antwort?.headers()['content-security-policy']).toContain("default-src 'self'");

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  // Koordinaten holen Höhen und entpacken sie; auch das muss erlaubt sein.
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual(
    [],
  );
});

test('die Koordinaten rechnen mit projection aus map.json', async ({ page }) => {
  // Eine Draufsicht über den Kacheln von 2:1: falsch fürs Auge, aber so
  // zeigt sich, dass projection gilt und nicht scale.
  await welt(page, { camera: 'top', projection: { azimuth: 'diagonal', u: 8, v: 8, y: 0 } });
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 27  Y 0  Z -23');
});

for (const mehr of [
  { projection: { azimuth: 'north', u: 16, v: 16, y: 0 } },
  { direction: 'sw' },
  { projection: { azimuth: 'diagonal', u: 8, v: 0, y: 8 } },
]) {
  test(`eine Kamera, die das Frontend nicht kennt, zeigt keine Koordinaten: ${JSON.stringify(mehr)}`, async ({
    page,
  }) => {
    const warnungen: string[] = [];
    page.on('console', (m) => {
      if (m.type() === 'warning') warnungen.push(m.text());
    });
    await welt(page, mehr);
    await page.goto(DEMO);

    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
    await expect(page.locator('.koordinaten')).toHaveCount(0);
    expect(warnungen.join(' ')).toContain('keine Koordinaten');
  });
}

test('unvollständige Höhen lassen die Karte stehen', async ({ page }) => {
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    // heights ohne heightsCell, etwa aus einem halben Stand.
    await route.fulfill({ response, json: { ...info, heights: 'heights/{x}.{z}.bin' } });
  });
  await page.goto(DEMO);

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.koordinaten')).toHaveCount(0);
});

test('die Maus zeigt Koordinaten des Blocks darunter, ohne Umriss', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  await expect(anzeige).toHaveText('X –  Y –  Z –');

  // Die Mitte der Oberseite von (35, 5, -15) und von (40, 0, 20).
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  await page.mouse.move(...(await bildschirm(page, 160, 236)));
  await expect(anzeige).toHaveText('X 40  Y 0  Z 20');
  // Den Block zeigt der Mauszeiger; ein Umriss hat keine Linie.
  await expect(page.locator('.leaflet-overlay-pane path')).not.toHaveAttribute('d', /M[^M]+M/);
});

test.describe('auf dem Touchscreen', () => {
  test.use({ hasTouch: true });

  test('ein Tippen zeigt den Block darunter', async ({ page }) => {
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
    // Ohne Zeiger zeigt der Umriss den Block: Sechseck und die drei Kanten
    // der vorderen Ecke.
    const linie = page.locator('.leaflet-overlay-pane path');
    await expect(linie).toHaveAttribute('d', /^M[^M]+M[^M]+M[^M]+$/);

    // Kommt danach die Maus, zeigt wieder ihr Zeiger den Block.
    await page.mouse.move(...(await bildschirm(page, 160, 236)));
    await expect(page.locator('.koordinaten')).toHaveText('X 40  Y 0  Z 20');
    await expect(linie).not.toHaveAttribute('d', /M[^M]+M/);
  });
});

test('zoomen wechselt die Kachelstufe, bis es keine feinere gibt', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  const hinein = page.locator('.leaflet-control-zoom-in');
  const heraus = page.locator('.leaflet-control-zoom-out');

  // Stufe für Stufe heraus und wieder hinein: die Kachelstufe folgt.
  await zoomClick(page, heraus);
  await expect.poll(() => tileZooms(page)).toEqual([1]);
  await zoomClick(page, heraus);
  await expect.poll(() => tileZooms(page)).toEqual([0]);
  await expect(heraus).toHaveClass(/leaflet-disabled/);

  await zoomClick(page, hinein);
  await expect.poll(() => tileZooms(page)).toEqual([1]);
  await zoomClick(page, hinein);
  await expect.poll(() => tileZooms(page)).toEqual([2]);

  // Über die feinste gerenderte Stufe hinaus gibt es keine Kacheln mehr.
  // Leaflet vergrössert dann die vorhandenen, statt ins Leere zu laden —
  // genau dafür ist maxNativeZoom da.
  await zoomClick(page, hinein);
  await expect.poll(() => tileStretch(page)).toBe(2);
  await zoomClick(page, hinein);
  await expect.poll(() => tileStretch(page)).toBe(4);
  expect(await tileZooms(page)).toEqual([2]);
  await expect(hinein).toHaveClass(/leaflet-disabled/);
});

test('passt Zoom 0 nicht ins Fenster, geht es weiter heraus', async ({ page }) => {
  // Zoom 0 des Demobaums ist 128 px gross. Einer gewachsenen Welt geht es
  // in jedem Fenster so: ihr Baum behält seine Stufen.
  await page.setViewportSize({ width: 100, height: 100 });
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();

  expect(await tileZooms(page)).toEqual([0]);
  await expect.poll(() => tileStretch(page)).toBe(0.5);
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);
});

test('ohne map.json sagt die Seite warum', async ({ page }) => {
  await page.goto('/?tiles=/gibt-es-nicht');
  await expect(page.locator('.error')).toContainText('map.json');
});
