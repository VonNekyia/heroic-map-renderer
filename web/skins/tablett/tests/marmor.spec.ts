import { expect, test, type Page } from '@playwright/test';
import { deflateSync } from 'node:zlib';
import { MARMOR_PIXEL } from '../bilder';

// Der Skin, wie er ausgeliefert wird: vorerst nur der Marmor. Siehe
// docs/entscheidungen/0079-tablett-vertagt-nur-marmor.md.

/** Der Demobaum der Grundkarte: 2:1, scale 16, feinste Stufe 2. */
const DEMO = '/?tiles=/tiles-demo';

/**
 * Eine Welt, die kein Quadrat ist: Der Marmor braucht keins. So klein, dass
 * ein Block des Marmors in einem kleinen Fenster erst unter, dann über einem
 * Pixel liegt.
 */
const RECHTECK = { seaLevel: 0, area: [-16, -16, 16, 0] };

/** Der Demobaum mit diesen Feldern in map.json und ebenen Höhen auf Y 0. */
async function welt(page: Page, mehr: object = RECHTECK): Promise<void> {
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

/** Wie oft der Skin gezeichnet hat. */
const zeichnungen = (page: Page) => page.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length);

/**
 * Merkt sich je Muster auf einer Leinwand des Skins, wie viele Pixel der
 * Leinwand ein Pixel des Bilds mindestens deckt und ob es geglättet liegt.
 */
function zaehle(): void {
  const muster: { texel: number; glatt: boolean }[] = [];
  Object.assign(window, { muster });
  const proto = CanvasPattern.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
  const setze = proto.setTransform!;
  proto.setTransform = function (this: CanvasPattern & { texel?: number }, ...argumente: unknown[]) {
    const m = argumente[0] as DOMMatrix2DInit;
    this.texel = Math.min(Math.hypot(m.a!, m.b!), Math.hypot(m.c!, m.d!));
    return setze.apply(this, argumente);
  };
  const ctx = CanvasRenderingContext2D.prototype as unknown as Record<string, (...a: unknown[]) => unknown>;
  const fuelle = ctx.fillRect!;
  ctx.fillRect = function (this: CanvasRenderingContext2D, ...argumente: unknown[]) {
    if (this.canvas.classList.contains('tablett') && this.fillStyle instanceof CanvasPattern) {
      muster.push({ texel: (this.fillStyle as CanvasPattern & { texel: number }).texel, glatt: this.imageSmoothingEnabled });
    }
    return fuelle.apply(this, argumente);
  };
}

/**
 * Die ferne Leinwand: ob sie das Fenster deckt und unter den Kacheln liegt,
 * und wie viele ihrer Stichproben keine Farbe aus marmor.webp haben.
 */
const leinwand = (page: Page) =>
  page.evaluate(async () => {
    const adresse = performance.getEntriesByType('resource').find((e) => /\/marmor-[^/]*\.webp$/.test(e.name))!.name;
    const bild = await createImageBitmap(await (await fetch(adresse)).blob());
    const kopie = new OffscreenCanvas(bild.width, bild.height).getContext('2d')!;
    kopie.drawImage(bild, 0, 0);
    const farben = new Set<number>();
    const m = kopie.getImageData(0, 0, bild.width, bild.height).data;
    for (let i = 0; i < m.length; i += 4) farben.add((m[i]! << 16) | (m[i + 1]! << 8) | m[i + 2]!);
    const c = document.querySelector<HTMLCanvasElement>('.leaflet-tablett-fern-pane canvas')!;
    const { data } = c.getContext('2d')!.getImageData(0, 0, c.width, c.height);
    let fremd = 0;
    let proben = 0;
    for (let i = 0; i < data.length; i += 4 * 97) {
      proben++;
      if (data[i + 3] !== 255 || !farben.has((data[i]! << 16) | (data[i + 1]! << 8) | data[i + 2]!)) fremd++;
    }
    const r = c.getBoundingClientRect();
    const kacheln = Number(getComputedStyle(document.querySelector('.leaflet-tile-pane')!).zIndex);
    return {
      deckt: r.left <= 0 && r.top <= 0 && r.right >= innerWidth && r.bottom >= innerHeight,
      unter: Number(getComputedStyle(c.parentElement!.closest('.leaflet-pane')!).zIndex) < kacheln,
      fremd,
      proben,
    };
  });

test('um die Kacheln liegt der Marmor, auf jeder Stufe, ohne Glättung, solange ein Block ein Pixel deckt, auch ohne quadratisches area', async ({ page }) => {
  await page.addInitScript(zaehle);
  // Ein kleines Fenster: In der Gesamtansicht fällt ein Block unter ein Pixel.
  await page.setViewportSize({ width: 400, height: 300 });
  await welt(page);
  await page.goto(DEMO);
  await expect.poll(() => zeichnungen(page)).toBeGreaterThan(0);
  const rein = page.locator('.leaflet-control-zoom-in');
  const pane = page.locator('.leaflet-map-pane');
  for (;;) {
    const muster = await page.evaluate(() => (window as unknown as { muster: { texel: number; glatt: boolean }[] }).muster);
    const { texel, glatt } = muster.at(-1)!;
    const block = MARMOR_PIXEL * texel;
    expect(glatt, `Block von ${block.toFixed(2)} px`).toBe(block < 1);
    const l = await leinwand(page);
    expect(l.deckt).toBe(true);
    expect(l.unter).toBe(true);
    // Ohne Glättung nur Farben des Marmors.
    if (block >= 1) expect(l.fremd, `Block von ${block.toFixed(2)} px`).toBe(0);
    if ((await rein.getAttribute('class'))!.includes('leaflet-disabled')) break;
    const vorher = await zeichnungen(page);
    await rein.click();
    await expect(pane).not.toHaveClass(/leaflet-zoom-anim/);
    await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
  }
  const muster = await page.evaluate(() => (window as unknown as { muster: { texel: number }[] }).muster);
  // Beide Seiten der Schwelle kamen vor.
  expect(muster.some(({ texel }) => MARMOR_PIXEL * texel < 1)).toBe(true);
  expect(muster.some(({ texel }) => MARMOR_PIXEL * texel >= 1)).toBe(true);
});

test('vom Tablett liegt nichts: keine nahe Ebene, nur marmor.webp geladen, nach der ersten Kachel, keine Meldung', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  await welt(page);
  await page.goto(DEMO);
  await expect.poll(() => zeichnungen(page)).toBeGreaterThan(0);
  await expect(page.locator('#map')).toHaveClass(/skin-tablett/);
  await expect(page.locator('.leaflet-tablett-nah-pane')).toHaveCount(0);
  await expect(page.locator('canvas.tablett')).toHaveCount(1);
  const anfragen = await page.evaluate(() =>
    performance.getEntriesByType('resource').map((e) => ({ pfad: new URL(e.name).pathname, start: e.startTime, ende: (e as PerformanceResourceTiming).responseEnd })),
  );
  const bilder = anfragen.filter((a) => /^\/assets\/[^/]+\.webp$/.test(a.pfad));
  expect(bilder).toHaveLength(1);
  expect(bilder[0]!.pfad).toMatch(/^\/assets\/marmor-[^/]+\.webp$/);
  const kacheln = anfragen.filter((a) => /^\/tiles-demo\/\d+\/\d+\/\d+\.webp$/.test(a.pfad));
  expect(bilder[0]!.start).toBeGreaterThanOrEqual(Math.min(...kacheln.map((a) => a.ende)));
  expect(meldungen.filter((m) => m.startsWith('Tablett:'))).toEqual([]);
});

/** Kontrast nach WCAG 2.x zweier Farben aus 0 bis 255. */
function kontrast(a: number[], b: number[]): number {
  const hell = (k: number[]) =>
    k.map((x) => x / 255).map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4)).reduce((s, x, i) => s + x * [0.2126, 0.7152, 0.0722][i]!, 0);
  const [h, d] = [hell(a), hell(b)].sort((x, y) => y - x);
  return (h! + 0.05) / (d! + 0.05);
}
const kanaele = (farbe: string) => farbe.match(/[\d.]+/g)!.slice(0, 3).map(Number);

test('die UI aus Pergament, Holz und Messing hält den Kontrast nach WCAG AA, und kein Control deckt ein anderes, von Telefonen bis 4K', async ({ page }) => {
  await welt(page);
  for (const [breite, hoehe] of [
    [360, 740],
    [390, 844],
    [1280, 720],
    [1920, 1080],
    [3840, 2160],
  ] as const) {
    await page.setViewportSize({ width: breite, height: hoehe });
    await page.goto(DEMO);
    await expect(page.locator('#map')).toHaveClass(/skin-tablett/);
    await page.mouse.move(breite / 2, hoehe / 2);
    await expect(page.locator('.koordinaten')).toContainText('X');
    // Gesperrte Knöpfe nimmt WCAG aus.
    for (const selector of ['.stand', '.lizenzen', '.koordinaten', '.kompass', '.leaflet-bar a:not(.leaflet-disabled)']) {
      const [vorn, grund] = await page
        .locator(selector)
        .first()
        .evaluate((e) => [getComputedStyle(e).color, getComputedStyle(e).backgroundColor]);
      expect(kontrast(kanaele(vorn), kanaele(grund)), `${breite} × ${hoehe}, ${selector}: ${vorn} auf ${grund}`).toBeGreaterThanOrEqual(4.5);
    }
    const ui = await page.locator('.leaflet-control').evaluateAll((elemente: HTMLElement[]) =>
      elemente.map((e) => ({ name: e.className, ...e.getBoundingClientRect().toJSON() }) as { name: string } & DOMRect),
    );
    expect(ui.length).toBeGreaterThanOrEqual(4);
    for (const [i, a] of ui.entries()) {
      expect(a.left >= 0 && a.top >= 0 && a.right <= breite && a.bottom <= hoehe, `${breite} × ${hoehe}: ${a.name} im Fenster`).toBe(true);
      for (const b of ui.slice(i + 1)) {
        const deckt = a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
        expect(deckt, `${breite} × ${hoehe}: ${a.name} über ${b.name}`).toBe(false);
      }
    }
  }
});
