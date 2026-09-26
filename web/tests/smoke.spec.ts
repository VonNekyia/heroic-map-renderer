import { expect, test, type Locator, type Page } from '@playwright/test';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
const DEMO = '/?tiles=/tiles-demo';

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
