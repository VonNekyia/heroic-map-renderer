import { expect, test, type Page } from '@playwright/test';
import type { Rechteck } from 'heroic-map-renderer/skin-api';
import { readdirSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { kamera, projiziere } from '../../../tests/kamera';
import { MARMOR_PIXEL, PERGAMENT, TISCH_RAND, VORLAGE } from '../bilder';
import { type Figur, type Flaeche, gesamtmitte, gesamtstufe, grenzen, GRUND, tablett } from '../tablett';

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
 * geglättet in hoher Güte gemalt werden; dazu je Muster, also je Marmor, wie
 * viele Pixel der Leinwand ein Pixel des Bilds mindestens deckt und ob es
 * geglättet liegt.
 */
function zaehle(): void {
  const zaehler = { n: 0, bilder: 0, glatt: 0, muster: [] as { texel: number; glatt: boolean }[] };
  Object.assign(window, { zaehler });
  const muster = CanvasPattern.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
  const setze = muster.setTransform!;
  muster.setTransform = function (this: CanvasPattern & { texel?: number }, ...argumente: unknown[]) {
    const m = argumente[0] as DOMMatrix2DInit | undefined;
    if (m) this.texel = Math.min(Math.hypot(m.a!, m.b!), Math.hypot(m.c!, m.d!));
    return setze.apply(this, argumente);
  };
  const proto = CanvasRenderingContext2D.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
  for (const name of ['fillRect', 'fill', 'stroke', 'drawImage', 'putImageData', 'clearRect', 'fillText']) {
    const original = proto[name]!;
    proto[name] = function (this: CanvasRenderingContext2D, ...argumente: unknown[]) {
      zaehler.n++;
      if (this.canvas.classList.contains('tablett') && name === 'drawImage' && argumente[0] instanceof ImageBitmap) {
        zaehler.bilder++;
        if (this.imageSmoothingEnabled && this.imageSmoothingQuality === 'high') zaehler.glatt++;
      }
      if (this.canvas.classList.contains('tablett') && name === 'fillRect' && this.fillStyle instanceof CanvasPattern) {
        zaehler.muster.push({ texel: (this.fillStyle as CanvasPattern & { texel: number }).texel, glatt: this.imageSmoothingEnabled });
      }
      return original.apply(this, argumente);
    };
  }
}
type Zaehler = { zaehler: { n: number; bilder: number; glatt: number; muster: { texel: number; glatt: boolean }[] } };
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

test('die Bilder kommen aus bilder/ und liegen geglättet, der Marmor als Muster; kommen sie beim Ziehen, malt der Skin erst danach', async ({ page }) => {
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
  // Bilder aus der Vorlage, jedes geglättet in hoher Güte; der Marmor als
  // Muster, wie scharf, prüft der nächste Test.
  const { bilder, glatt, muster } = await page.evaluate(() => (window as unknown as Zaehler).zaehler);
  expect(bilder).toBeGreaterThan(0);
  expect(glatt).toBe(bilder);
  expect(muster.length).toBeGreaterThan(0);
});

test('der Marmor liegt ohne Glättung, solange ein Block ein Pixel deckt, kleiner geglättet, auf jeder Stufe', async ({ page }) => {
  await page.addInitScript(zaehle);
  // Von der Gesamtansicht in einem Fenster, in dem ein Block unter ein Pixel
  // fällt, Stufe um Stufe bis ganz hinein.
  await page.setViewportSize({ width: 400, height: 300 });
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const rein = page.locator('.leaflet-control-zoom-in');
  while (!(await rein.getAttribute('class'))!.includes('leaflet-disabled')) {
    const vorher = await zeichnungen(page);
    await zoome(page, 'in');
    await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
  }
  const { muster } = await page.evaluate(() => (window as unknown as Zaehler).zaehler);
  const marmor = muster.map(({ texel, glatt }) => ({ block: MARMOR_PIXEL * texel, glatt }));
  // Beide Seiten der Schwelle kommen vor, auch Blöcke von wenigen Pixeln.
  expect(marmor.some(({ block }) => block < 1)).toBe(true);
  expect(marmor.some(({ block }) => block >= 1 && block < 4)).toBe(true);
  for (const { block, glatt } of marmor) expect(glatt, `Block von ${block.toFixed(2)} px`).toBe(block < 1);
});

for (const dpr of [1.25, 1.5]) {
  test.describe(`devicePixelRatio ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr });

    test('liegen die Leinwände zwischen zwei Pixeln des Geräts, zeigt der Bildschirm sie Pixel für Pixel, ohne zu glätten', async ({ page }) => {
      // 1101 × 701: Die Leinwände beginnen 275 px links und 175 px über dem
      // Fenster, bei 1,25 und 1,5 zwischen zwei Pixeln des Geräts.
      await page.setViewportSize({ width: 1101, height: 701 });
      await welt(page, QUADRAT);
      await page.goto(DEMO);
      await expect.poll(() => gezeichnet(page)).toBe(true);
      await page.evaluate(() => Promise.all(document.getAnimations().map((a) => a.finished)));
      // Oben rechts unter dem Kompass liegt nur ferner Tisch.
      const clip = { x: 1001, y: 60, width: 90, height: 60 };
      const foto = (await page.screenshot({ clip })).toString('base64');
      const v = await page.evaluate(
        async ({ foto, clip }) => {
          const d = devicePixelRatio;
          const bild = await createImageBitmap(new Blob([Uint8Array.from(atob(foto), (z) => z.charCodeAt(0))], { type: 'image/png' }));
          const kopie = new OffscreenCanvas(bild.width, bild.height).getContext('2d')!;
          kopie.drawImage(bild, 0, 0);
          const ist = kopie.getImageData(0, 0, bild.width, bild.height).data;
          const [fern, nah] = ['fern', 'nah'].map((n) => document.querySelector<HTMLCanvasElement>(`.leaflet-tablett-${n}-pane canvas`)!);
          const r = fern!.getBoundingClientRect();
          const [x0, y0] = [Math.round((clip.x - r.left) * d), Math.round((clip.y - r.top) * d)];
          const lies = (c: HTMLCanvasElement) => c.getContext('2d')!.getImageData(x0 - 2, y0 - 2, bild.width + 4, bild.height + 4).data;
          const [f, n] = [lies(fern!), lies(nah!)];
          const breite = bild.width + 4;
          // Der beste ganze Versatz bis 2 Pixel: wie viele Pixel des Fotos dort der fernen Leinwand gleichen.
          let beste = { gleich: -1, dx: 0, dy: 0 };
          for (let dy = -2; dy <= 2; dy++) {
            for (let dx = -2; dx <= 2; dx++) {
              let gleich = 0;
              for (let y = 0; y < bild.height; y++) {
                for (let x = 0; x < bild.width; x++) {
                  const [i, j] = [4 * (y * bild.width + x), 4 * ((y + 2 + dy) * breite + x + 2 + dx)];
                  if (ist[i] === f[j] && ist[i + 1] === f[j + 1] && ist[i + 2] === f[j + 2]) gleich++;
                }
              }
              if (gleich > beste.gleich) beste = { gleich, dx, dy };
            }
          }
          return { ...beste, pixel: bild.width * bild.height, nah: n.some((wert, i) => i % 4 === 3 && wert > 0), versatz: Math.abs(r.left * d - Math.round(r.left * d)) };
        },
        { foto, clip },
      );
      // Wirklich zwischen zwei Pixeln, und nah deckt hier nichts.
      expect(v.versatz).toBeGreaterThan(0.2);
      expect(v.nah).toBe(false);
      expect(v.gleich, JSON.stringify(v)).toBe(v.pixel);
    });
  });
}

test('die Bilder des Skins laden erst, wenn die erste Kachel da und gemalt ist, und das Tablett blendet ein', async ({ page }) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const anfragen = await page.evaluate(() =>
    performance.getEntriesByType('resource').map((e) => ({ pfad: new URL(e.name).pathname, start: e.startTime, ende: (e as PerformanceResourceTiming).responseEnd })),
  );
  const kacheln = anfragen.filter((a) => /^\/tiles-demo\/\d+\/\d+\/\d+\.webp$/.test(a.pfad));
  const bilder = anfragen.filter((a) => /^\/assets\/[^/]+\.webp$/.test(a.pfad));
  expect(kacheln.length).toBeGreaterThan(0);
  expect(bilder.length).toBe(readdirSync(new URL('../bilder/', import.meta.url)).length);
  const ersteKachel = Math.min(...kacheln.map((a) => a.ende));
  // Gemalt: Die Kachel ist das grösste Element, sobald sie zu sehen ist.
  // Begänne ein Bild davor, zählte Lighthouse es zum LCP.
  const gemalt = await page.evaluate(
    () =>
      new Promise<number>((fertig) => {
        new PerformanceObserver((liste) => {
          const kachel = liste.getEntries().find((e) => (e as LargestContentfulPaint).url.includes('/tiles-demo/'));
          if (kachel) fertig(kachel.startTime);
        }).observe({ type: 'largest-contentful-paint', buffered: true });
      }),
  );
  for (const { pfad, start } of bilder) {
    expect(start, pfad).toBeGreaterThanOrEqual(ersteKachel);
    expect(start, pfad).toBeGreaterThanOrEqual(gemalt);
  }
  await expect(page.locator('canvas.tablett').first()).toHaveCSS('animation-name', 'tablett-einblenden');
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

// Bei 300 × 200 liegt die Gesamtansicht unter Zoom 0, der kleinsten Stufe aus map.json.
for (const [breite, hoehe] of [[800, 600], [300, 200]] as const) {
  test(`in einem Fenster der Grösse 0 lädt die Karte, und mit der ersten Grösse, ${breite} × ${hoehe}, kommt die Gesamtansicht mit Kacheln`, async ({ page }) => {
    // Ein iframe der Grösse 0 ist ein Fenster der Grösse 0, wie ein Tab, der
    // verborgen aufgeht.
    await welt(page, QUADRAT);
    await page.route('**/leer.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: `<iframe src="${DEMO}" width="0" height="0" style="border: 0"></iframe>` }),
    );
    await page.goto('/leer.html');
    const fenster = page.locator('iframe');
    const karte = (await (await fenster.elementHandle()).contentFrame())!;
    // Die Karte steht, sobald die Adresse ihre Stufe nennt; bricht sie ab,
    // steht statt ihrer der Fehler im Fenster.
    const zustand = () =>
      karte.evaluate(() => document.querySelector('#map .error')?.textContent ?? (new URL(location.href).searchParams.has('zoom') ? 'steht' : 'lädt'));
    await expect.poll(zustand).toBe('steht');

    await fenster.evaluate((e: HTMLIFrameElement, [b, h]) => Object.assign(e, { width: String(b), height: String(h) }), [breite, hoehe]);
    await expect.poll(() => karte.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length)).toBeGreaterThan(0);
    const p = kamera('2:1', 16);
    const rahmen = grenzen(QUADRAT.area, QUADRAT.seaLevel, { projektion: p, k: 0, projiziere: (x, y, z) => projiziere(x, y, z, p) });
    const groesse = (await karte.locator('#map').boundingBox())!;
    const fit = gesamtstufe(rahmen, 2, groesse.width, groesse.height);
    await expect.poll(() => karte.evaluate(() => Number(new URL(location.href).searchParams.get('zoom')))).toBeCloseTo(fit - 2, 9);
    await expect.poll(() => karte.locator('img.leaflet-tile-loaded').count()).toBeGreaterThan(0);
    expect(await zustand()).toBe('steht');
  });
}

test('wird das Fenster kleiner, zeigt die Gesamtansicht Kacheln, auch unter der kleinsten Stufe aus map.json', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  await page.setViewportSize({ width: 300, height: 200 });
  const p = kamera('2:1', 16);
  const rahmen = grenzen(QUADRAT.area, QUADRAT.seaLevel, { projektion: p, k: 0, projiziere: (x, y, z) => projiziere(x, y, z, p) });
  const groesse = (await page.locator('#map').boundingBox())!;
  const fit = gesamtstufe(rahmen, 2, groesse.width, groesse.height);
  expect(fit).toBeLessThan(0);
  await expect.poll(() => Number(new URL(page.url()).searchParams.get('zoom'))).toBeCloseTo(fit - 2, 9);
  await expect.poll(() => page.locator('img.leaflet-tile-loaded').count()).toBeGreaterThan(0);
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

/** Die sieben Kameras der Vergleichsbilder, wie map.json sie nennt. */
const SIEBEN = [
  ['8:5', 'se'],
  ['2:1', 'se'],
  ['2:1', 'nw'],
  ['1:1', 'ne'],
  ['top', 'sw'],
  ['north-45', 's'],
  ['top-north', 'e'],
] as const;

/**
 * Wie viele Stellen von 3 × 3 Pixeln der fernen Leinwand im Fenster nur
 * Grund zeigen oder durchsichtig sind: Dort endete der Tisch. Einzelne
 * Pixel in der Farbe des Grunds kann auch das Bild haben.
 */
const loecher = (page: Page) =>
  page.locator('.leaflet-tablett-fern-pane canvas').evaluate((leinwand: HTMLCanvasElement, grund: number[]) => {
    const r = leinwand.getBoundingClientRect();
    const [sx, sy] = [leinwand.width / r.width, leinwand.height / r.height];
    const [x0, y0] = [Math.max(0, Math.ceil(-r.left * sx)), Math.max(0, Math.ceil(-r.top * sy))];
    const [x1, y1] = [Math.min(leinwand.width, Math.floor((innerWidth - r.left) * sx)), Math.min(leinwand.height, Math.floor((innerHeight - r.top) * sy))];
    const [w, h] = [x1 - x0, y1 - y0];
    const { data } = leinwand.getContext('2d')!.getImageData(x0, y0, w, h);
    const leer = (x: number, y: number) => {
      const i = 4 * (y * w + x);
      return data[i + 3]! < 255 || (data[i] === grund[0] && data[i + 1] === grund[1] && data[i + 2] === grund[2]);
    };
    let n = 0;
    for (let y = 1; y < h - 1; y += 3) {
      for (let x = 1; x < w - 1; x += 3) {
        let alle = true;
        for (let dy = -1; dy <= 1 && alle; dy++) for (let dx = -1; dx <= 1 && alle; dx++) alle = leer(x + dx, y + dy);
        if (alle) n++;
      }
    }
    return n;
  }, [1, 3, 5].map((i) => Number.parseInt(GRUND.slice(i, i + 2), 16)));

test('in jeder Kamera endet der Tisch nirgends, in der Gesamtansicht und hineingezoomt an den Rändern von maxBounds', async ({ page }) => {
  // Sieben Kameras nacheinander, je mit zwei Wegen an den Rand.
  test.setTimeout(120_000);
  for (const [camera, direction] of SIEBEN) {
    await page.unrouteAll();
    await welt(page, { ...QUADRAT, projection: kamera(camera, 16), direction });
    await page.goto(DEMO);
    await expect.poll(() => gezeichnet(page)).toBe(true);
    expect(await loecher(page), `${camera} ${direction}, Gesamtansicht`).toBe(0);
    await zoome(page, 'in');
    // Bis an die Grenzen, oben links und unten rechts; dort zeichnet der Skin neu.
    const karte = (await page.locator('#map').boundingBox())!;
    const [mx, my] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
    for (const richtung of [1, -1]) {
      const vorher = await zeichnungen(page);
      for (let i = 0; i < 6; i++) {
        await page.mouse.move(mx, my);
        await page.mouse.down();
        await page.mouse.move(mx + 400 * richtung, my + 300 * richtung, { steps: 8 });
        await page.evaluate(() => new Promise((fertig) => void requestAnimationFrame(fertig)));
        await page.mouse.up();
      }
      await steht(page);
      await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
      expect(await loecher(page), `${camera} ${direction}, am Rand ${richtung}`).toBe(0);
    }
  }
});

test('auf den Rücken zweier Bücher steht der Text aus SKIN_TEXT_BUCH1 und SKIN_TEXT_BUCH2', async ({ page }) => {
  await page.addInitScript(() => {
    const texte: string[] = [];
    Object.assign(window, { texte });
    const proto = CanvasRenderingContext2D.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
    const original = proto.fillText!;
    proto.fillText = function (this: CanvasRenderingContext2D, ...argumente: unknown[]) {
      if (this.canvas.classList.contains('tablett')) texte.push(String(argumente[0]));
      return original.apply(this, argumente);
    };
  });
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  // Die Werte setzt playwright.config.ts beim Build mit dem Skin.
  const texte = await page.evaluate(() => (window as unknown as { texte: string[] }).texte);
  expect([...new Set(texte)].sort()).toEqual(['Probe Eins', 'Probe Zwei']);
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

/** Die relative Helligkeit einer Farbe nach WCAG 2. */
function helligkeit([r, g, b]: number[]): number {
  const linear = (c: number) => (c / 255 <= 0.04045 ? c / 255 / 12.92 : ((c / 255 + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear(r!) + 0.7152 * linear(g!) + 0.0722 * linear(b!);
}

/** Der Kontrast zweier Farben nach WCAG 2, von 1 bis 21. */
function kontrast(a: number[], b: number[]): number {
  const [hell, dunkel] = [helligkeit(a), helligkeit(b)].sort((x, y) => y - x);
  return (hell! + 0.05) / (dunkel! + 0.05);
}

/** Die Kanäle einer Farbe aus `getComputedStyle`. */
const kanaele = (farbe: string) => farbe.match(/[\d.]+/g)!.slice(0, 3).map(Number);

/** Ein Wert aus `getComputedStyle` des ersten Elements. */
const stil = (page: Page, selector: string, eigenschaft: string) =>
  page.locator(selector).first().evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), eigenschaft);

test('die UI auf Pergament und Holz hält den Kontrast nach WCAG AA, auch der Umschalter; gesperrt bleibt ein Knopf aus Holz', async ({
  page,
}) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('#map')).toHaveClass(/skin-tablett/);
  await expect(page.locator('.stand')).toBeVisible();
  // Den Umschalter gibt es nur mit zwei Bäumen; seine Regeln hängen an der Klasse.
  await page.locator('#map').evaluate((karte) => karte.append(Object.assign(document.createElement('select'), { className: 'baeume' })));
  const karte = (await page.locator('#map').boundingBox())!;
  await page.mouse.move(karte.x + karte.width / 2, karte.y + karte.height / 2);
  await expect(page.locator('.koordinaten')).toContainText('X');
  for (const selector of ['.stand', '.lizenzen', '.koordinaten', '.kompass', '.leaflet-control-zoom-in', '.baeume']) {
    const [vorn, grund] = [await stil(page, selector, 'color'), await stil(page, selector, 'background-color')];
    expect(kontrast(kanaele(vorn), kanaele(grund)), `${selector}: ${vorn} auf ${grund}`).toBeGreaterThanOrEqual(4.5);
  }
  // In der Gesamtansicht ist − gesperrt: Holz wie die anderen, nicht das Grau von Leaflet.
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);
  expect(await stil(page, '.leaflet-control-zoom-out', 'background-color')).toBe(await stil(page, '.leaflet-control-zoom-in', 'background-color'));
  // Der Rand aus Messing ist ein Verlauf, eckig wie die Knöpfe darin.
  expect(await stil(page, '.leaflet-bar', 'border-image-source')).toMatch(/^linear-gradient\(/);
  expect(await stil(page, '.leaflet-control-zoom-in', 'border-top-left-radius')).toBe('0px');
});

test('per Tastatur liegt der Fokus innen: Messing auf Holz, Tinte auf Pergament', async ({ page }) => {
  await welt(page, QUADRAT);
  await page.goto(DEMO);
  await expect(page.locator('#map')).toHaveClass(/skin-tablett/);
  for (const selector of ['.leaflet-control-zoom-in', '.kopieren']) {
    const ziel = page.locator(selector);
    for (let i = 0; i < 20 && !(await ziel.evaluate((e) => e === document.activeElement)); i++) await page.keyboard.press('Tab');
    await expect(ziel).toBeFocused();
    const rand = ['outline-style', 'outline-width', 'outline-offset'];
    expect(await Promise.all(rand.map((name) => stil(page, selector, name)))).toEqual(['solid', '2px', '-2px']);
    const [farbe, grund] = [await stil(page, selector, 'outline-color'), await stil(page, selector, 'background-color')];
    expect(kontrast(kanaele(farbe), kanaele(grund)), `${selector}: ${farbe} auf ${grund}`).toBeGreaterThanOrEqual(3);
  }
});

test('in der Gesamtansicht deckt die UI keinen Gegenstand und keine Lilie, in 8:5 von Telefonen bis 4K', async ({ page }) => {
  const p = kamera('8:5', 16);
  const blick = { projektion: p, k: 0, projiziere: (x: number, y: number, z: number) => projiziere(x, y, z, p) };
  await welt(page, { ...QUADRAT, projection: p, direction: 'se' });
  for (const [breite, hoehe] of [
    [1491, 1055],
    [1680, 1050],
    [1920, 1080],
    [1280, 720],
    [3840, 2160],
    [390, 844],
  ] as const) {
    await page.setViewportSize({ width: breite, height: hoehe });
    await page.goto(DEMO);
    await expect.poll(() => gezeichnet(page)).toBe(true);
    // Mit Koordinaten ist die Leiste so breit wie im Gebrauch.
    await page.mouse.move(breite / 2, hoehe / 2);
    await expect(page.locator('.koordinaten')).toContainText('X');
    // Gegenstände und Lilien, wie der Skin sie in der Gesamtansicht legt.
    const fit = gesamtstufe(grenzen(QUADRAT.area, QUADRAT.seaLevel, blick), 2, breite, hoehe);
    const s = 2 ** (fit - 2);
    const [mx, my] = gesamtmitte(QUADRAT.area, QUADRAT.seaLevel, blick, breite / s, hoehe / s);
    const imFenster = (px: number, py: number) => [(px - mx) * s + breite / 2, (py - my) * s + hoehe / 2];
    const teile = tablett(QUADRAT.area, QUADRAT.seaLevel, -64, blick);
    const dinge = teile
      .filter((t): t is Figur => t.form === 'figur')
      .map(({ bild, fuss, anker, mass, groesse }) => {
        const [l, o] = imFenster(fuss[0] - mass * anker[0], fuss[1] - mass * anker[1]);
        return { bild, l: l!, o: o!, r: l! + mass * groesse[0] * s, u: o! + mass * groesse[1] * s };
      });
    // Das Pergament liegt flach im Bild des Tischs, dort, wo dieses es zeigt.
    const tisch = teile.find((t): t is Flaeche => t.form === 'flaeche' && t.bild === 'tisch')!;
    const imTisch = (px: number, py: number) => {
      const [u, v] = [(px + TISCH_RAND) / (VORLAGE[0] + 2 * TISCH_RAND), (py + TISCH_RAND) / (VORLAGE[1] + 2 * TISCH_RAND)];
      return imFenster(tisch.o[0] + u * tisch.a[0] + v * tisch.b[0], tisch.o[1] + u * tisch.a[1] + v * tisch.b[1]);
    };
    const [[pl, po], [pr, pu]] = [imTisch(PERGAMENT[0], PERGAMENT[1]), imTisch(PERGAMENT[2], PERGAMENT[3])];
    dinge.push({ bild: 'pergament', l: pl!, o: po!, r: pr!, u: pu! });
    const ui = await page.locator('.leaflet-control').evaluateAll((elemente: HTMLElement[]) =>
      elemente.map((e) => {
        const { left, top, right, bottom } = e.getBoundingClientRect();
        return { name: e.className, left, top, right, bottom };
      }),
    );
    expect(ui.length).toBeGreaterThanOrEqual(4);
    for (const [i, { name, left, top, right, bottom }] of ui.entries()) {
      for (const d of dinge) {
        const deckt = left < d.r && right > d.l && top < d.u && bottom > d.o;
        expect(deckt, `${breite} × ${hoehe}: ${name} über ${d.bild}`).toBe(false);
      }
      // Auch kein Control über einem anderen: `weiche` sieht die anderen Ecken nicht.
      for (const b of ui.slice(i + 1)) {
        const deckt = left < b.right && right > b.left && top < b.bottom && bottom > b.top;
        expect(deckt, `${breite} × ${hoehe}: ${name} über ${b.name}`).toBe(false);
      }
    }
  }
});

test('im Bezugsrahmen liegt der Zoom links auf dem Marmor zwischen Pergament und Holzrand', async ({ page }) => {
  // 8:5 aus se im Fenster der Vorlage, mit einer Welt, deren Rahmen es zu
  // 92,5 % füllt: Dort ist ein Pixel der Vorlage ein Pixel des Fensters,
  // siehe den Test zum Bezugsrahmen in tablett.spec.ts. Kacheln gibt es für
  // diese Welt nicht; ihre Ebene meldet trotzdem `load`.
  const weit = 1e6;
  await welt(page, { seaLevel: 0, area: [-3200, -3200, 3200, 3200], maxZoom: 11, bounds: [-weit, -weit, weit, weit], projection: kamera('8:5', 16), direction: 'se' });
  await page.setViewportSize({ width: VORLAGE[0], height: VORLAGE[1] });
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const zoom = (await page.locator('.leaflet-control-zoom').boundingBox())!;
  // Das Pergament endet bei y = 428; der Holzrand des Tischs beginnt am
  // linken Rand bei y = 533 und fällt nach rechts um 0,67 px je Pixel,
  // gemessen in der Vorlage.
  expect(zoom.y).toBeGreaterThanOrEqual(PERGAMENT[3]);
  expect(zoom.y + zoom.height).toBeLessThanOrEqual(533 + 0.67 * zoom.x);
});
