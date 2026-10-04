import { expect, test, type Page } from '@playwright/test';
import type { Rechteck } from 'heroic-map-renderer/skin-api';
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync } from 'node:zlib';
import { kamera, projiziere } from '../../../tests/kamera';
import { type Brett, lage } from '../brett';
import { type Blick, gesamtmitte, gesamtstufe, grenzen, GRUND } from '../tablett';

// Der Skin mit gerenderten Bildern: der Platzhalter aus werkzeug/brett.py für
// 2:1 in fixtures/brett, gebaut wie ein Skin von aussen, mit brett/. Die
// Seite kommt aus dem Build, ohne Server.

const SKIN = fileURLToPath(new URL('..', import.meta.url));
const WEB = fileURLToPath(new URL('../../..', import.meta.url));
const BRETT = fileURLToPath(new URL('fixtures/brett', import.meta.url));
const INDEX = JSON.parse(readFileSync(join(BRETT, 'brett.json'), 'utf8')) as Brett;

const URSPRUNG = 'http://brett.test';
const DEMO = `${URSPRUNG}/?tiles=/tiles-demo`;
const QUADRAT = { seaLevel: 0, area: [-64, -64, 64, 64] as Rechteck };

let ort = '';
let dist = '';

test.beforeAll(() => {
  test.setTimeout(120_000);
  ort = mkdtempSync(join(tmpdir(), 'brett-'));
  const ordner = join(ort, 'tablett');
  cpSync(SKIN, ordner, { recursive: true, filter: (quelle) => !quelle.startsWith(join(SKIN, 'tests')) });
  cpSync(BRETT, join(ordner, 'brett'), { recursive: true });
  dist = join(ort, 'dist');
  execFileSync(
    process.execPath,
    [join(WEB, 'node_modules', 'vite', 'bin', 'vite.js'), 'build', '--outDir', dist, '--emptyOutDir', '--logLevel', 'error'],
    { cwd: WEB, env: { ...process.env, SKIN: ordner }, stdio: 'pipe' },
  );
});

test.afterAll(() => rmSync(ort, { recursive: true, force: true }));

/** Liefert die Seite aus dem Build, map.json mit `mehr` und ebenen Höhen, die Bilder des Platzhalters unter /platzhalter/. */
async function welt(page: Page, mehr: object): Promise<void> {
  await page.route(`${URSPRUNG}/**`, async (route) => {
    const { pathname } = new URL(route.request().url());
    if (pathname === '/tiles-demo/map.json') {
      const info = JSON.parse(readFileSync(join(dist, 'tiles-demo', 'map.json'), 'utf8')) as object;
      const hoehen = { heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
      return route.fulfill({ json: { ...info, ...hoehen, ...mehr } });
    }
    if (pathname.startsWith('/tiles-demo/heights/')) return route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) });
    const datei = pathname.startsWith('/platzhalter/')
      ? join(BRETT, pathname.slice('/platzhalter/'.length))
      : join(dist, pathname === '/' ? 'index.html' : decodeURIComponent(pathname));
    return existsSync(datei) ? route.fulfill({ path: datei }) : route.fulfill({ status: 404 });
  });
}

const zeichnungen = (page: Page) => page.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length);
const gezeichnet = async (page: Page) => (await zeichnungen(page)) > 0;

/**
 * Vergleicht eine Leinwand mit ihrem Bild. `x`, `y` ist die erwartete linke
 * obere Ecke des Bilds im Fenster, `f` ein Pixel des Bilds in Pixeln des
 * Fensters. Die Leinwand malt die Ecke auf ganze Pixel des Geräts; gesucht
 * wird die, bei der die meisten Stichproben die Farbe ihres Pixels im Bild
 * haben. Dazu zählt es die Pixel der Leinwand in einer Farbe, die das Bild
 * nicht hat: Glättung mischt welche.
 */
function vergleiche(page: Page, ebene: 'fern' | 'nah', datei: string, x: number, y: number, f: number) {
  const grund = [1, 3, 5].map((i) => Number.parseInt(GRUND.slice(i, i + 2), 16));
  return page.evaluate(
    async ({ ebene, datei, x, y, f, grund }) => {
      const leinwand = document.querySelector<HTMLCanvasElement>(`.leaflet-tablett-${ebene}-pane canvas`)!;
      const r = leinwand.getBoundingClientRect();
      const ist = leinwand.getContext('2d')!.getImageData(0, 0, leinwand.width, leinwand.height).data;
      const bild = await createImageBitmap(await (await fetch(`/platzhalter/${datei}`)).blob());
      const kopie = new OffscreenCanvas(bild.width, bild.height).getContext('2d')!;
      kopie.drawImage(bild, 0, 0);
      const soll = kopie.getImageData(0, 0, bild.width, bild.height).data;
      // Wo das Bild nichts hat, zeigt fern den Grund, nah nichts.
      const leer = ebene === 'fern' ? ((grund[0]! << 24) | (grund[1]! << 16) | (grund[2]! << 8) | 255) >>> 0 : 0;
      const farbe = (d: Uint8ClampedArray, i: number) => (d[i + 3] === 0 ? leer : ((d[i]! << 24) | (d[i + 1]! << 16) | (d[i + 2]! << 8) | d[i + 3]!) >>> 0);
      const farben = new Set<number>([leer]);
      for (let i = 0; i < soll.length; i += 4) farben.add(farbe(soll, i));
      let fremd = 0;
      for (let i = 0; i < ist.length; i += 4) if (!farben.has(farbe(ist, i))) fremd++;
      const dpr = devicePixelRatio;
      const [ex, ey, fd] = [(x - r.left) * dpr, (y - r.top) * dpr, f * dpr];
      let beste = { anteil: -1, dx: 0, dy: 0 };
      for (let dy = -2; dy <= 2; dy++) {
        for (let dx = -2; dx <= 2; dx++) {
          const [x0, y0] = [Math.round(ex) + dx, Math.round(ey) + dy];
          let [treffer, proben] = [0, 0];
          for (let py = 0; py < leinwand.height; py += 7) {
            const sy = Math.floor((py + 0.5 - y0) / fd);
            if (sy < 0 || sy >= bild.height) continue;
            for (let px = 0; px < leinwand.width; px += 7) {
              const sx = Math.floor((px + 0.5 - x0) / fd);
              if (sx < 0 || sx >= bild.width) continue;
              proben++;
              if (farbe(ist, 4 * (py * leinwand.width + px)) === farbe(soll, 4 * (sy * bild.width + sx))) treffer++;
            }
          }
          if (treffer / proben > beste.anteil) beste = { anteil: treffer / proben, dx: x0 - ex, dy: y0 - ey };
        }
      }
      return { ...beste, fremd, dpr, f: fd };
    },
    { ebene, datei, x, y, f, grund },
  );
}

// Je Gerät ein Fenster: bei 1 ohne ganzes n in der Gesamtansicht, bei 2 und
// 1,5 mit n = 3 und n = 2.
for (const [dpr, breite, hoehe, ganz] of [
  [1, 1280, 720, false],
  [2, 1060, 596, true],
  [1.5, 1000, 563, true],
] as const) {
  test.describe(`devicePixelRatio ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr, viewport: { width: breite, height: hoehe } });

    test('gerendert liegt das Brett Pixel für Pixel auf der Karte, ohne Mischfarben, in der Gesamtansicht und eine Stufe tiefer', async ({ page }) => {
      await welt(page, { ...QUADRAT, projection: kamera('2:1', 16), direction: 'se' });
      await page.goto(DEMO);
      await expect.poll(() => gezeichnet(page)).toBe(true);
      const p = kamera('2:1', 16);
      const blick: Blick = { projektion: p, k: 0, projiziere: (x, y, z) => projiziere(x, y, z, p) };
      const karte = (await page.locator('#map').boundingBox())!;
      const bild = INDEX['2:1 se']!;
      const { links, oben, mass } = lage(QUADRAT.area, QUADRAT.seaLevel, blick, bild);
      const fit = gesamtstufe(grenzen(QUADRAT.area, QUADRAT.seaLevel, blick), 2, karte.width, karte.height, mass * dpr);
      const n = mass * dpr * 2 ** (fit - 2);
      expect(Math.abs(n - Math.round(n)) < 1e-9, `n = ${n}`).toBe(ganz);
      const [mx, my] = gesamtmitte(QUADRAT.area, QUADRAT.seaLevel, blick, karte.width * 2 ** (2 - fit), karte.height * 2 ** (2 - fit));
      for (const tiefer of [false, true]) {
        if (tiefer) {
          await page.locator('.leaflet-control-zoom-in').click();
          await expect.poll(() => page.evaluate(() => performance.getEntriesByName('tablett: zeichnen').length)).toBeGreaterThan(1);
        }
        // Die Adresse zählt die Stufe ab der feinsten.
        const stufe = 2 + Number(new URL(page.url()).searchParams.get('zoom'));
        expect(tiefer ? stufe : fit).toBeCloseTo(tiefer ? Math.round(fit + 1) : stufe, 9);
        // Die Mitte der Gesamtansicht bleibt beim Zoomen in der Mitte des Fensters.
        const s = 2 ** (stufe - 2);
        const [x, y] = [karte.x + karte.width / 2 + (links - mx) * s, karte.y + karte.height / 2 + (oben - my) * s];
        expect(mass * s * dpr).toBeGreaterThan(1);
        for (const [ebene, datei] of [
          ['fern', bild.fern],
          ['nah', bild.nah],
        ] as const) {
          const v = await vergleiche(page, ebene, datei, x, y, mass * s);
          const was = `${ebene}, Stufe ${stufe}: ${JSON.stringify(v)}`;
          expect(v.fremd, was).toBe(0);
          expect(v.anteil, was).toBeGreaterThan(0.999);
          // Auf ein Pixel des Fensters, so genau rundet Leaflet die Mitte.
          expect(Math.abs(v.dx), was).toBeLessThanOrEqual(dpr);
          expect(Math.abs(v.dy), was).toBeLessThanOrEqual(dpr);
        }
      }
    });
  });
}

test('ändert sich devicePixelRatio ohne neue Grösse des Fensters, folgen Gesamtstufe und Leinwände', async ({ page }) => {
  // Die Anfragen des Skins nach der Auflösung: Kopflos meldet Chromium ihr
  // change nicht, wenn CDP nur deviceScaleFactor ändert. Der Test schickt es
  // wie der Browser beim Wechsel des Monitors.
  await page.addInitScript(() => {
    const echt = window.matchMedia.bind(window);
    const anfragen: MediaQueryList[] = [];
    window.matchMedia = (frage: string) => {
      const anfrage = echt(frage);
      if (frage.includes('resolution')) anfragen.push(anfrage);
      return anfrage;
    };
    Object.assign(window, { anfragen });
  });
  // 1060 × 596: Bei 1 liegt die Gesamtansicht nach 0067, bei 2 deckt ein
  // Pixel des Bilds 3 Pixel, auf einer anderen Stufe.
  await page.setViewportSize({ width: 1060, height: 596 });
  await welt(page, { ...QUADRAT, projection: kamera('2:1', 16), direction: 'se' });
  await page.goto(DEMO);
  await expect.poll(() => gezeichnet(page)).toBe(true);
  const p = kamera('2:1', 16);
  const blick: Blick = { projektion: p, k: 0, projiziere: (x, y, z) => projiziere(x, y, z, p) };
  const { mass } = lage(QUADRAT.area, QUADRAT.seaLevel, blick, INDEX['2:1 se']!);
  const rahmen = grenzen(QUADRAT.area, QUADRAT.seaLevel, blick);
  const [bei1, bei2] = [gesamtstufe(rahmen, 2, 1060, 596, mass), gesamtstufe(rahmen, 2, 1060, 596, 2 * mass)];
  expect(bei2).not.toBeCloseTo(bei1, 3);
  const stufe = () => page.evaluate(() => 2 + Number(new URL(location.href).searchParams.get('zoom')));
  expect(await stufe()).toBeCloseTo(bei1, 9);
  const anfragen = () => page.evaluate(() => (window as unknown as { anfragen: MediaQueryList[] }).anfragen.map((a) => a.media));
  expect(await anfragen()).toEqual(['(resolution: 1dppx)']);
  await page.evaluate(() => addEventListener('resize', () => Object.assign(window, { groesse: true })));
  const vorher = await zeichnungen(page);
  // Nur das Verhältnis ändert sich, wie beim Wechsel des Monitors.
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1060, height: 596, deviceScaleFactor: 2, mobile: false });
  await page.evaluate(() => (window as unknown as { anfragen: MediaQueryList[] }).anfragen.at(-1)!.dispatchEvent(new Event('change')));
  await expect.poll(stufe).toBeCloseTo(bei2, 9);
  await expect.poll(() => zeichnungen(page)).toBeGreaterThan(vorher);
  const leinwand = page.locator('.leaflet-tablett-fern-pane canvas');
  await expect.poll(() => leinwand.evaluate((c: HTMLCanvasElement) => [c.width, c.height])).toEqual([3180, 1788]);
  // Kein resize, und die nächste Anfrage gilt dem neuen Verhältnis.
  expect(await page.evaluate(() => 'groesse' in window)).toBe(false);
  expect(await anfragen()).toEqual(['(resolution: 1dppx)', '(resolution: 2dppx)']);
});

test('fehlt die Kamera in brett.json, bleibt das Tablett aus, und die Konsole sagt es', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  await welt(page, { ...QUADRAT, projection: kamera('8:5', 16), direction: 'se' });
  await page.goto(DEMO);
  await expect.poll(() => meldungen.some((m) => m.includes('kein Bild für 8:5 se in brett/'))).toBe(true);
  await expect(page.locator('.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('canvas.tablett')).toHaveCount(0);
  await expect(page.locator('#map')).not.toHaveClass(/skin-tablett/);
});
