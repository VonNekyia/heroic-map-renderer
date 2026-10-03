import { expect, test, type Page } from '@playwright/test';
import { deflateSync } from 'node:zlib';

/** Der Demobaum der Grundkarte. */
const DEMO = '/?tiles=/tiles-demo';

/** Ein Tablett um die Spalten von -64 bis 63, mit der Oberkante auf Y 0. */
const QUADRAT = { seaLevel: 0, area: [-64, -64, 64, 64] };

/** Der Demobaum mit diesen Feldern in map.json und ebenen Höhen auf Y 0. */
async function welt(page: Page, mehr: object): Promise<void> {
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    const hoehen = { heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    await route.fulfill({ response, json: { ...info, ...hoehen, ...mehr } });
  });
  await page.route('**/tiles-demo/heights/*.bin', (route) =>
    route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) }),
  );
}

/** Zoomt per Knopf und wartet, bis Leaflet fertig ist. */
async function zoome(page: Page, knopf: 'in' | 'out'): Promise<void> {
  const pane = page.locator('.leaflet-map-pane');
  await page.locator(`.leaflet-control-zoom-${knopf}`).click();
  await expect(pane).toHaveClass(/leaflet-zoom-anim/);
  await expect(pane).not.toHaveClass(/leaflet-zoom-anim/);
}

/** Beide Ebenen: über den Kacheln, Deckkraft, sichtbar, Klicks, ob gemalt. */
const stand = (page: Page) =>
  page.evaluate(() => {
    const kacheln = Number(getComputedStyle(document.querySelector('.leaflet-tile-pane')!).zIndex);
    const ebene = (name: string) => {
      const pane = document.querySelector(`.leaflet-${name}-pane`)!;
      const leinwand = pane.querySelector('canvas')!;
      const { data } = leinwand.getContext('2d')!.getImageData(0, 0, leinwand.width, leinwand.height);
      return {
        ueber: Number(getComputedStyle(pane).zIndex) > kacheln,
        deckkraft: Number(getComputedStyle(leinwand).opacity),
        sichtbar: getComputedStyle(leinwand).visibility === 'visible',
        klicks: getComputedStyle(leinwand).pointerEvents,
        gemalt: data.some((wert, i) => i % 4 === 3 && wert > 0),
      };
    };
    return { fern: ebene('tablett-fern'), nah: ebene('tablett-nah') };
  });

test('Rahmen und Tisch liegen um die Kacheln, fangen keine Klicks ab und blenden bis fitZoom + 1 aus', async ({
  page,
}) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('canvas.tablett')).toHaveCount(2);
  const ganz = { deckkraft: 1, sichtbar: true, klicks: 'none', gemalt: true };
  await expect.poll(() => stand(page)).toEqual({ fern: { ueber: false, ...ganz }, nah: { ueber: true, ...ganz } });
  await expect(page.locator('#map')).toHaveClass(/skin-tablett/);

  // Die Koordinaten gehen durch beide Ebenen hindurch.
  const karte = (await page.locator('#map').boundingBox())!;
  await page.mouse.move(karte.x + karte.width / 2, karte.y + karte.height / 2);
  await expect(page.locator('.koordinaten')).toHaveText(/^X -?\d+ {2}Y 0 {2}Z -?\d+$/);

  // fitZoom ist die kleinste Stufe.
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);

  // Eine Stufe hinein sind beide ausgeblendet und verborgen, zurück wieder da.
  await zoome(page, 'in');
  const weg = { deckkraft: 0, sichtbar: false, klicks: 'none', gemalt: true };
  await expect.poll(() => stand(page)).toEqual({ fern: { ueber: false, ...weg }, nah: { ueber: true, ...weg } });
  await zoome(page, 'out');
  await expect.poll(() => stand(page)).toEqual({ fern: { ueber: false, ...ganz }, nah: { ueber: true, ...ganz } });
});

test('beim Ziehen und Zoomen zeichnet der Skin nichts, nur bei einer neuen Fenstergrösse', async ({ page }) => {
  // Zählt jeden Aufruf, der auf eine Leinwand malt.
  await page.addInitScript(() => {
    const zaehler = { n: 0 };
    Object.assign(window, { zaehler });
    const proto = CanvasRenderingContext2D.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
    for (const name of ['fillRect', 'fill', 'stroke', 'drawImage', 'putImageData', 'clearRect', 'fillText']) {
      const original = proto[name]!;
      proto[name] = function (this: unknown, ...argumente: unknown[]) {
        zaehler.n++;
        return original.apply(this, argumente);
      };
    }
  });
  const gemalt = () => page.evaluate(() => (window as unknown as { zaehler: { n: number } }).zaehler.n);
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect.poll(gemalt).toBeGreaterThan(0);
  const vorher = await gemalt();

  const karte = (await page.locator('#map').boundingBox())!;
  const [mx, my] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  await page.mouse.move(mx, my);
  await page.mouse.down();
  for (let i = 1; i <= 10; i++) await page.mouse.move(mx + 15 * i, my + 8 * i);
  await page.mouse.up();
  await zoome(page, 'in');
  await zoome(page, 'out');
  expect(await gemalt()).toBe(vorher);

  // Gegenprobe: Eine neue Fenstergrösse zeichnet neu.
  await page.setViewportSize({ width: 900, height: 700 });
  await expect.poll(gemalt).toBeGreaterThan(vorher);
});

test('ein area, das kein Quadrat ist, zeichnet kein Tablett und sagt es in der Konsole', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (meldung) => meldungen.push(meldung.text()));
  await welt(page, { seaLevel: 0, area: [-64, -64, 64, 32] });
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('canvas.tablett')).toHaveCount(0);
  expect(meldungen).toContain('Tablett: area ist kein Quadrat, das Tablett bleibt aus.');
  await expect(page.locator('.leaflet-control-zoom-out')).not.toHaveClass(/leaflet-disabled/);
});
