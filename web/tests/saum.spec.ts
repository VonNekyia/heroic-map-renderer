/**
 * Die Grundschicht am Rand der Welt: Sie zeigt nur Pixel, die voll decken.
 * Siehe docs/entscheidungen/0141-grundschicht-nur-deckend.md.
 */
import { expect, test, type Page } from './test';
import { png } from './ebenen-welt';
import { projiziere, zweiZuEins } from '../src/pick';

/** Die Farbe der Welt in der Szene und der Grund der Karte, `--karte-grund`. */
const WELT = [60, 120, 200];
const GRUND = [18, 22, 28];
/** Die feinste Stufe der Szene: 2048 × 2048 Pixel, 8 × 8 Kacheln. */
const FEIN = 3;
const SEITE = 256 * 2 ** FEIN;

/**
 * Ein Pixel der feinsten Stufe mitten in der Welt mit Alpha 128: Drei Stufen
 * gröber hat es Alpha 254, der härteste Fall für die Schwelle.
 */
const FAST = { x: 990, y: 980, alpha: 128 };

/**
 * Alpha je Stufe wie `renderer/src/render/pyramid.rs`: auf der feinsten
 * 255 in der Welt und 0 ausserhalb, bei `FAST` 128, darüber der Mittelwert
 * von 2 × 2, aufgerundet.
 */
function stufen(drinnen: (x: number, y: number) => boolean): Uint8Array[] {
  const fein = new Uint8Array(SEITE * SEITE);
  for (let y = 0; y < SEITE; y++) for (let x = 0; x < SEITE; x++) fein[y * SEITE + x] = drinnen(x, y) ? 255 : 0;
  fein[FAST.y * SEITE + FAST.x] = FAST.alpha;
  const alle = [fein];
  for (let s = SEITE / 2; s >= 256; s /= 2) {
    const vorher = alle[0]!;
    const neu = new Uint8Array(s * s);
    for (let y = 0; y < s; y++) {
      for (let x = 0; x < s; x++) {
        const i = 2 * y * 2 * s + 2 * x;
        neu[y * s + x] = Math.ceil((vorher[i]! + vorher[i + 1]! + vorher[i + 2 * s]! + vorher[i + 2 * s + 1]!) / 4);
      }
    }
    alle.unshift(neu);
  }
  return alle;
}

/**
 * Eine Szene unter `/tiles-saum`: eine Welt in einer Farbe, deren Rand
 * nicht auf dem Raster der groben Pixel liegt. Schräg wie der Rand im iso
 * oder gerade wie in 2D.
 */
async function szene(page: Page, drinnen: (x: number, y: number) => boolean, halten = false): Promise<void> {
  const alpha = stufen(drinnen);
  expect(alpha[0]![(FAST.y >> FEIN) * 256 + (FAST.x >> FEIN)], 'Alpha 254 in der Grundschicht').toBe(254);
  await page.route('**/tiles-saum/map.json', (route) =>
    route.fulfill({ json: { tileSize: 256, scale: 16, minZoom: 0, maxZoom: FEIN, tiles: '{z}/{x}/{y}.png', bounds: [0, 0, SEITE, SEITE] } }),
  );
  await page.route(/\/tiles-saum\/\d+\/\d+\/\d+\.png$/, async (route) => {
    const [z, kx, ky] = /(\d+)\/(\d+)\/(\d+)\.png$/.exec(route.request().url())!.slice(1).map(Number) as [number, number, number];
    // Die feinen Kacheln hängen, solange der Test sie hält.
    if (halten && z === FEIN) return;
    const a = alpha[z]!;
    const s = 256 * 2 ** z;
    await route.fulfill({ contentType: 'image/png', body: png(256, 256, (x, y) => [...WELT, a[(ky * 256 + y) * s + kx * 256 + x]!]) });
  });
  // Der Block, dessen Ecke dem Pixel (1000, 1000) der feinsten Stufe am nächsten liegt; dort verläuft der Rand.
  let [bx, bz, best] = [0, 0, Infinity];
  for (let x = 0; x <= 300; x++) {
    for (let z = -100; z <= 200; z++) {
      const [px, py] = projiziere(x, 0, z, zweiZuEins(16));
      const d = Math.hypot(px - 1000, py - 1000);
      if (d < best) [bx, bz, best] = [x, z, d];
    }
  }
  await page.goto(`/?tiles=/tiles-saum&at=${bx},0,${bz}&zoom=0`);
}

/** Die Pixel eines Ausschnitts mitten im Fenster, fern der Knöpfe, als `r,g,b` und Anzahl. */
async function farben(page: Page): Promise<{ bild: Buffer; zahl: Map<string, number> }> {
  const bild = await page.screenshot({ clip: { x: 340, y: 160, width: 600, height: 400 }, animations: 'disabled' });
  const liste = await page.evaluate(async (b64) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    // createImageBitmap aus einem Blob lädt keine Adresse; die Content-Security-Policy der Seite bleibt unberührt.
    const bmp = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
    const ctx = new OffscreenCanvas(bmp.width, bmp.height).getContext('2d')!;
    ctx.drawImage(bmp, 0, 0);
    const d = ctx.getImageData(0, 0, bmp.width, bmp.height).data;
    const zahl: Record<string, number> = {};
    for (let o = 0; o < d.length; o += 4) {
      const k = `${d[o]},${d[o + 1]},${d[o + 2]}`;
      zahl[k] = (zahl[k] ?? 0) + 1;
    }
    return zahl;
  }, bild.toString('base64'));
  return { bild, zahl: new Map(Object.entries(liste)) };
}

/** Bis alle Kacheln geladen und eingeblendet sind und kein Zoom läuft. */
async function ruhig(page: Page): Promise<void> {
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.grund .leaflet-tile-loaded').first()).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          !document.querySelector('.leaflet-zoom-anim') &&
          [...document.querySelectorAll('.leaflet-tile')].every((i) => i.classList.contains('leaflet-tile-loaded') && getComputedStyle(i).opacity === '1'),
      ),
    )
    .toBe(true);
}

const RAENDER: [string, (x: number, y: number) => boolean][] = [
  ['schräg wie im iso', (x, y) => x + 2 * y < 3001],
  ['gerade wie in 2D', (x, y) => x < 1003 && y < 1003],
];

for (const [art, drinnen] of RAENDER) {
  test(`nach 0141 ist das Bild am Rand der Welt nach dem Laden gleich dem ohne Grundschicht, ${art}`, async ({ page }) => {
    await szene(page, drinnen);
    await ruhig(page);
    const mit = await farben(page);
    // Im Ausschnitt liegen Welt und Grund, also der Rand.
    expect(mit.zahl.get(WELT.join()) ?? 0).toBeGreaterThan(10_000);
    expect(mit.zahl.get(GRUND.join()) ?? 0).toBeGreaterThan(10_000);
    await page.locator('.grund').evaluate((e) => void (e.style.visibility = 'hidden'));
    const ohne = await farben(page);
    expect(mit.bild.equals(ohne.bild), 'die Grundschicht ragt unter den feinen Kacheln hervor').toBe(true);
  });

  test(`nach 0141 zeigt die Grundschicht beim Laden mitten in der Welt ihre Farbe und am Rand keinen Saum, ${art}`, async ({ page }) => {
    await szene(page, drinnen, true);
    await expect(page.locator('.grund .leaflet-tile-loaded').first()).toBeVisible();
    await expect(page.locator('.kacheln img.leaflet-tile-loaded')).toHaveCount(0);
    await expect
      .poll(() =>
        page.evaluate(
          () => !document.querySelector('.leaflet-zoom-anim') && [...document.querySelectorAll('.grund .leaflet-tile')].every((i) => getComputedStyle(i).opacity === '1'),
        ),
      )
      .toBe(true);
    const { zahl } = await farben(page);
    // Nur die Welt, unverändert, und der Grund der Karte: Ein halb gedecktes Pixel mischte beide.
    expect([...zahl.keys()].sort()).toEqual([GRUND.join(), WELT.join()].sort());
    expect(zahl.get(WELT.join())!).toBeGreaterThan(10_000);
  });
}

test('nach 0141 bleibt das Bild am Rand der Welt gleich, auch wenn Zurücklesen Zufall liefert wie mit Schutz vor Fingerabdrücken', async ({ page }) => {
  await page.addInitScript(() => {
    CanvasRenderingContext2D.prototype.getImageData = function (_x: number, _y: number, b: number, h: number) {
      const d = new ImageData(b, h);
      for (let i = 0; i < d.data.length; i++) d.data[i] = Math.floor(Math.random() * 256);
      return d;
    };
  });
  await szene(page, RAENDER[0]![1]);
  await ruhig(page);
  const mit = await farben(page);
  await page.locator('.grund').evaluate((e) => void (e.style.visibility = 'hidden'));
  expect(mit.bild.equals((await farben(page)).bild), 'die Grundschicht zeigt, was das Zurücklesen liefert').toBe(true);
});
