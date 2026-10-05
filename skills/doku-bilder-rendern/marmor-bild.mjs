// Das Bild docs/bilder/marmor.webp: die Karte mit dem Skin Tablett, vorerst
// nur der Marmor, an einem Kachelbaum der Testwelt; links die Gesamtansicht,
// rechts eine Stufe hinein. Aus web/, nach dem Build mit dem Skin:
//   SKIN=./skins/tablett npx vite build --outDir dist-skin
//   node ../skills/doku-bilder-rendern/marmor-bild.mjs <kachelwurzel> ../docs/bilder/marmor.webp
import { existsSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';

// Playwright aus web/node_modules.
const { chromium } = createRequire(resolve('package.json'))('playwright');

const [wurzel, ziel] = process.argv.slice(2).map((p) => resolve(p));
const DIST = resolve('dist-skin');
const [B, H] = [960, 600];

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: B, height: H } });
await page.route('http://bild.test/**', (route) => {
  const pfad = decodeURIComponent(new URL(route.request().url()).pathname);
  const datei = pfad.startsWith('/tiles/') ? join(wurzel, pfad.slice('/tiles/'.length)) : join(DIST, pfad === '/' ? 'index.html' : pfad);
  return existsSync(datei) ? route.fulfill({ path: datei }) : route.fulfill({ status: 404 });
});

/** Wartet, bis alle Kacheln da sind und der Skin für diese Ansicht gezeichnet hat. */
async function fertig(zeichnungen) {
  await page.waitForFunction((n) => performance.getEntriesByName('tablett: zeichnen').length > n, zeichnungen, { timeout: 60_000 });
  await page.waitForFunction(() => [...document.querySelectorAll('img.leaflet-tile')].every((k) => k.complete), null, { timeout: 60_000 });
  await page.evaluate(() => Promise.all(document.getAnimations().map((a) => a.finished)));
  await page.waitForTimeout(500);
}

await page.goto('http://bild.test/');
await fertig(0);
const ganz = await page.screenshot();
const n = await page.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length);
await page.locator('.leaflet-control-zoom-in').click();
await fertig(n);
const hinein = await page.screenshot();

// Beide nebeneinander, mit 8 px Abstand, als WebP aus dem Browser.
const bild = await page.evaluate(
  async ({ fotos, B, H }) => {
    const leinwand = new OffscreenCanvas(2 * B + 8, H);
    const ctx = leinwand.getContext('2d');
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0, 0, leinwand.width, H);
    for (const [i, foto] of fotos.entries()) {
      const b = await createImageBitmap(new Blob([Uint8Array.from(atob(foto), (z) => z.charCodeAt(0))], { type: 'image/png' }));
      ctx.drawImage(b, i * (B + 8), 0);
    }
    const blob = await leinwand.convertToBlob({ type: 'image/webp', quality: 0.9 });
    return [...new Uint8Array(await blob.arrayBuffer())];
  },
  { fotos: [ganz, hinein].map((f) => f.toString('base64')), B, H },
);
writeFileSync(ziel, Buffer.from(bild));
await browser.close();
