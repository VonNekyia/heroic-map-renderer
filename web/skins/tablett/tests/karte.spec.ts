import { expect, test, type Page } from '@playwright/test';
import type { Rechteck } from 'heroic-map-renderer/skin-api';
import { deflateSync } from 'node:zlib';
import { kamera, projiziere } from '../../../tests/kamera';
import { gesamtstufe, grenzen } from '../tablett';

/** Der Demobaum der Grundkarte: 2:1, scale 16, feinste Stufe 2. */
const DEMO = '/?tiles=/tiles-demo';

/** Ein Tablett um die Spalten von -64 bis 63, mit der Oberkante auf Y 0. */
const QUADRAT = { seaLevel: 0, area: [-64, -64, 64, 64] as Rechteck };

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

/**
 * Zählt jeden Aufruf, der auf eine Leinwand malt, in `window.zaehler`: alle;
 * auf den Leinwänden des Tabletts die Bilder aus bilder/ und wie viele davon
 * geglättet in hoher Güte gemalt werden.
 */
function zaehle(): void {
  const zaehler = { n: 0, bilder: 0, glatt: 0 };
  Object.assign(window, { zaehler });
  const proto = CanvasRenderingContext2D.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
  for (const name of ['fillRect', 'fill', 'stroke', 'drawImage', 'putImageData', 'clearRect', 'fillText']) {
    const original = proto[name]!;
    proto[name] = function (this: CanvasRenderingContext2D, ...argumente: unknown[]) {
      zaehler.n++;
      if (this.canvas.classList.contains('tablett') && name === 'drawImage' && argumente[0] instanceof ImageBitmap) {
        zaehler.bilder++;
        if (this.imageSmoothingEnabled && this.imageSmoothingQuality === 'high') zaehler.glatt++;
      }
      return original.apply(this, argumente);
    };
  }
}
type Zaehler = { zaehler: { n: number; bilder: number; glatt: number } };
const gemalt = (page: Page) => page.evaluate(() => (window as unknown as Zaehler).zaehler.n);

/** Wie oft der Skin gezeichnet hat. */
const zeichnungen = (page: Page) => page.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length);

/** Hat der Skin schon gezeichnet? */
const gezeichnet = async (page: Page) => (await zeichnungen(page)) > 0;

/**
 * Wartet, bis die Karte steht: Adresse und Lage der Ebene 300 ms gleich. Die
 * Adresse allein reicht nicht: Während Leaflet an der Grenze noch
 * nachschiebt, bleibt sie oft gleich, und ein Zug hinein verpufft.
 */
async function steht(page: Page): Promise<void> {
  let vorher = '';
  await expect
    .poll(
      async () => {
        const jetzt = `${page.url()} ${await page.locator('.leaflet-map-pane').getAttribute('style')}`;
        const gleich = jetzt === vorher;
        vorher = jetzt;
        return gleich;
      },
      { intervals: [300] },
    )
    .toBe(true);
}

/**
 * Zieht die Karte langsam um (dx, dy) Pixel, um die Mitte des Fensters,
 * ohne Schwung, und wartet, bis sie steht.
 */
async function ziehe(page: Page, dx: number, dy: number): Promise<void> {
  const karte = (await page.locator('#map').boundingBox())!;
  const [mx, my] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  await page.mouse.move(mx - dx / 2, my - dy / 2);
  await page.mouse.down();
  await page.mouse.move(mx + dx / 2, my + dy / 2, { steps: 10 });
  // Nach einer Pause läuft die Karte nicht mit Schwung weiter.
  await page.waitForTimeout(150);
  await page.mouse.up();
  await steht(page);
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

/** Zoomt hinein, bis es nicht weiter geht, und wartet je Stufe auf das neue Bild. */
async function ganzHinein(page: Page): Promise<void> {
  const hinein = page.locator('.leaflet-control-zoom-in');
  while (!(await hinein.getAttribute('class'))!.includes('leaflet-disabled')) {
    const vorher = await zeichnungen(page);
    await zoome(page, 'in');
    await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
  }
}

test('Rahmen und Tisch liegen um die Kacheln, fangen keine Klicks ab und bleiben bis zur feinsten Stufe sichtbar', async ({
  page,
}) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  // Die Ebenen kommen mit dem ersten Bild, wenn die Bilder geladen sind.
  await expect.poll(() => gezeichnet(page)).toBe(true);
  await expect(page.locator('canvas.tablett')).toHaveCount(2);
  const da = { deckkraft: 1, sichtbar: true, klicks: 'none' };
  const ganz = { fern: { ueber: false, ...da, gemalt: true }, nah: { ueber: true, ...da, gemalt: true } };
  await expect.poll(() => stand(page)).toEqual(ganz);
  await expect(page.locator('#map')).toHaveClass(/skin-tablett/);

  // Die Koordinaten gehen durch beide Ebenen hindurch.
  const karte = (await page.locator('#map').boundingBox())!;
  await page.mouse.move(karte.x + karte.width / 2, karte.y + karte.height / 2);
  await expect(page.locator('.koordinaten')).toHaveText(/^X -?\d+ {2}Y 0 {2}Z -?\d+$/);

  // Die Gesamtansicht ist die kleinste Stufe; in diesem Fenster ist sie ganz.
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);
  await expect(page.locator('#map')).not.toHaveClass(/tablett-gebrochen/);

  // Eine Stufe hinein und auf der feinsten sind beide sichtbar und decken
  // voll. In der Mitte der Welt liegt dort nichts Nahes im Bild.
  const vorher = await zeichnungen(page);
  await zoome(page, 'in');
  await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
  await expect.poll(() => stand(page)).toEqual(ganz);
  await ganzHinein(page);
  await expect.poll(() => stand(page)).toEqual({ ...ganz, nah: expect.objectContaining({ ueber: true, ...da }) });
  // An der nahen Ecke, unten im Bild, deckt dort auch der nahe Rahmen.
  for (let i = 0; i < 6; i++) await ziehe(page, 0, -600);
  await expect.poll(() => stand(page)).toEqual(ganz);
});

test('die Leinwand ist das Fenster mit Überstand, auf der feinsten Stufe so gross wie in der Gesamtansicht', async ({ page }) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const groesse = () =>
    page.locator('.leaflet-tablett-fern-pane canvas').evaluate((leinwand: HTMLCanvasElement) => [leinwand.width, leinwand.height]);
  const karte = (await page.locator('#map').boundingBox())!;
  // Je Seite ein Viertel des Fensters darüber.
  expect(await groesse()).toEqual([Math.round(karte.width * 1.5), Math.round(karte.height * 1.5)]);
  const gesamt = await groesse();
  await ganzHinein(page);
  expect(await groesse()).toEqual(gesamt);
});

test('der Skin zeichnet nach jedem Zoom und nach einem Zug über den Überstand hinaus neu, nach einem kurzen Zug nicht', async ({
  page,
}) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect.poll(() => gezeichnet(page)).toBe(true);

  // Nach einem Zoom einmal.
  let vorher = await zeichnungen(page);
  await zoome(page, 'in');
  await expect.poll(() => zeichnungen(page)).toBe(vorher + 1);

  // Ein Zug, der im Überstand von einem Viertel des Fensters bleibt, zeichnet
  // nichts; einer darüber hinaus zeichnet neu.
  const breite = (await page.locator('#map').boundingBox())!.width;
  vorher = await zeichnungen(page);
  await ziehe(page, breite / 8, 0);
  expect(await zeichnungen(page)).toBe(vorher);
  await ziehe(page, -breite / 2, 0);
  await expect.poll(() => zeichnungen(page)).toBe(vorher + 1);

  // Eine neue Fenstergrösse zeichnet neu.
  vorher = await zeichnungen(page);
  await page.setViewportSize({ width: 900, height: 700 });
  await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
});

test('die Bilder kommen aus bilder/ und liegen geglättet; kommen sie beim Ziehen, malt der Skin erst danach', async ({ page }) => {
  await page.addInitScript(zaehle);
  // Hält das Bild des Tischs zurück, bis der Test es freigibt.
  let freigeben = () => {};
  const frei = new Promise<void>((los) => (freigeben = los));
  await page.route('**/tisch-*.webp', async (route) => {
    await frei;
    await route.continue();
  });
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await gezeichnet(page)).toBe(false);
  const vorher = await gemalt(page);

  // Das Bild kommt, während die Karte gezogen wird: Gemalt wird erst danach.
  const karte = (await page.locator('#map').boundingBox())!;
  const [mx, my] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  await page.mouse.move(mx, my);
  await page.mouse.down();
  for (let i = 1; i <= 5; i++) await page.mouse.move(mx + 15 * i, my + 8 * i);
  const tisch = page.waitForResponse('**/tisch-*.webp');
  freigeben();
  expect((await tisch).ok()).toBe(true);
  // Zeit, das Bild zu lesen.
  await page.waitForTimeout(500);
  expect(await gemalt(page)).toBe(vorher);
  expect(await gezeichnet(page)).toBe(false);
  await page.mouse.up();
  await expect.poll(() => gezeichnet(page)).toBe(true);
  expect(await gemalt(page)).toBeGreaterThan(vorher);
  // Bilder aus der Vorlage, jedes geglättet in hoher Güte.
  const { bilder, glatt } = await page.evaluate(() => (window as unknown as Zaehler).zaehler);
  expect(bilder).toBeGreaterThan(0);
  expect(glatt).toBe(bilder);
});

test('in der Gesamtansicht füllt das Tablett 92,5 % des Fensters wie in der Vorlage, auch zwischen zwei Stufen', async ({ page }) => {
  // In diesem Fenster füllte es auf der ganzen Stufe darunter 61 %.
  await page.setViewportSize({ width: 1790, height: 1000 });
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const p = kamera('2:1', 16);
  const rahmen = grenzen(QUADRAT.area, QUADRAT.seaLevel, { projektion: p, k: 0, projiziere: (x, y, z) => projiziere(x, y, z, p) });
  const groesse = (await page.locator('#map').boundingBox())!;
  const fit = gesamtstufe(rahmen, 2, groesse.width, groesse.height);
  const fuellung = Math.max(((rahmen[2] - rahmen[0]) * 2 ** (fit - 2)) / groesse.width, ((rahmen[3] - rahmen[1]) * 2 ** (fit - 2)) / groesse.height);
  expect(fuellung).toBeCloseTo(0.925, 9);
  expect(fit - Math.floor(fit)).toBeGreaterThanOrEqual(0.5);
  // Die Adresse zählt die Stufe ab der feinsten.
  await expect.poll(() => Number(new URL(page.url()).searchParams.get('zoom'))).toBeCloseTo(fit - 2, 9);
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);

  // Die Leinwände liegen Pixel auf Pixel; die Kacheln der Stufe darüber
  // verkleinert Leaflet, geglättet.
  const lage = await page.locator('.leaflet-tablett-fern-pane canvas').evaluate((e: HTMLCanvasElement) => {
    const { width, height } = e.getBoundingClientRect();
    return [e.width - width, e.height - height];
  });
  expect(Math.abs(lage[0]!) + Math.abs(lage[1]!)).toBeLessThan(1e-3);
  await expect(page.locator('#map')).toHaveClass(/tablett-gebrochen/);
  const glatt = () => page.locator('img.leaflet-tile').first().evaluate((e) => getComputedStyle(e).imageRendering);
  expect(await glatt()).toBe('auto');

  // Hinein überspringt Leaflet die nächste ganze Stufe. Heraus liegt sie
  // dazwischen, ganz und pixelig; dann wieder die Gesamtansicht. Das
  // Tablett bleibt auf jeder Stufe.
  const da = { deckkraft: 1, sichtbar: true, klicks: 'none' };
  const sichtbar = { fern: { ueber: false, ...da, gemalt: true }, nah: expect.objectContaining({ ueber: true, ...da }) };
  await zoome(page, 'in');
  await expect.poll(() => stand(page)).toEqual(sichtbar);
  await zoome(page, 'out');
  await expect(page.locator('#map')).not.toHaveClass(/tablett-gebrochen/);
  expect(await glatt()).toBe('pixelated');
  await expect.poll(() => stand(page)).toEqual(sichtbar);
  await zoome(page, 'out');
  await expect(page.locator('#map')).toHaveClass(/tablett-gebrochen/);
  await expect.poll(() => stand(page)).toEqual({ fern: { ueber: false, ...da, gemalt: true }, nah: { ueber: true, ...da, gemalt: true } });
});

test('hineingezoomt lässt sich die Karte bis über jede Ecke von area ziehen', async ({ page }) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await zoome(page, 'in');
  await zoome(page, 'in');
  const karte = (await page.locator('#map').boundingBox())!;
  const [mx, my] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  // Der Block an einem Punkt am Rand des Fensters, als x − z und x + z: von
  // Südost liegen sie im Bild waagrecht und senkrecht. x + z gilt auf Y 0,
  // entlang des Blicks zurückgerechnet: In 2:1 rückt ein Block Höhe das
  // Bild so weit wie zwei Blöcke x + z.
  const block = async (x: number, y: number) => {
    await page.mouse.move(x, y);
    const [, bx, by, bz] = (await page.locator('.koordinaten').textContent())!.match(/X (-?\d+) {2}Y (-?\d+) {2}Z (-?\d+)/)!.map(Number);
    return { quer: bx! - bz!, laengs: bx! + bz! - 2 * by! };
  };
  // Ziehen nach rechts zeigt den linken Rand, und so fort. Zwölf Züge
  // reichen von einem Ende der Grenzen bis zum anderen.
  for (const [dx, dy, x, y, pruefe] of [
    [1, 0, karte.x + 2, my, ({ quer }: { quer: number }) => quer <= -128],
    [-1, 0, karte.x + karte.width - 3, my, ({ quer }: { quer: number }) => quer >= 128],
    [0, 1, mx, karte.y + 2, ({ laengs }: { laengs: number }) => laengs <= -128],
    [0, -1, mx, karte.y + karte.height - 3, ({ laengs }: { laengs: number }) => laengs >= 128],
  ] as const) {
    for (let i = 0; i < 12; i++) {
      await page.mouse.move(mx, my);
      await page.mouse.down();
      await page.mouse.move(mx + 400 * dx, my + 300 * dy, { steps: 8 });
      // Leaflet setzt die Karte erst im nächsten Bild; ohne es verfiele der Rest des Zugs.
      await page.evaluate(() => new Promise((fertig) => void requestAnimationFrame(fertig)));
      await page.mouse.up();
    }
    await steht(page);
    const am = await block(x, y);
    expect(pruefe(am), JSON.stringify(am)).toBe(true);
  }
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
