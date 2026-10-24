/**
 * Keine Prüfung, eine Messung: 1000 Banner und 1000 Nadeln auf zwei Ebenen,
 * bei dpr 1 und 3, CPU 1× und 4×, je Laden, Ziehen, Rad und Pinch, hinein und
 * hinaus getrennt. Gemessen werden Frames, lange Aufgaben, JS-Heap,
 * DOM-Knoten und die Pixel der Leinwände der Icons im DOM. Kacheln gibt es
 * keine, Höhen stehen am Objekt: Es zählt nur, was die Orte kosten. Läuft nur
 * mit `STRESS=1`; `AUS` nennt eine Datei, an die jede Zeile als JSON
 * angehängt wird, `VERSUCH` hängt `&versuch=…` an die Adresse, `LAEUFE`
 * setzt die Läufe je Zelle, Vorgabe 3.
 * Siehe docs/messungen/2026-10-24-viele-nadeln-und-banner.md.
 */
import { test, type Browser, type CDPSession, type Page } from '@playwright/test';
import { appendFileSync } from 'node:fs';
import { BILDER, DEMO, png, STAEDTE, welt } from './ebenen-welt';

test.skip(!process.env.STRESS, 'nur mit STRESS=1, siehe oben');

const NADELN = { id: 'beispiel:nadeln', name: { de: 'Nadeln', en: 'Pins' }, visible: true, order: 90, version: 'a' };
/** Ein Satz wie aus `--banners` in 2:1, wie in ebenen.spec.ts. */
const SATZ = { foot: [12, 49], angle: 26.56505117707799 };
const SPRITE = png(23, 51);

/** Je Variante: Abstand im Raster in Blöcken, ob Banner einen Namen tragen, Zoom des Starts. */
const VARIANTEN = {
  dicht: { abstand: 4, namen: true, zoom: -4 },
  weit: { abstand: 40, namen: true, zoom: -4 },
  'weit-ohne-namen': { abstand: 40, namen: false, zoom: -4 },
};

/** 2000 Orte auf einem Raster 45 × 45 um (0, 0), abwechselnd Banner und Nadel. */
function orte(abstand: number, namen: boolean): { banner: object[]; nadeln: object[] } {
  const banner: object[] = [];
  const nadeln: object[] = [];
  for (let n = 0; n < 2000; n++) {
    const [i, j] = [(n % 45) - 22, Math.floor(n / 45) - 22];
    const at = [i * abstand + 0.5, j * abstand + 0.5];
    const name = `Ort ${n}`;
    const panel = { blocks: [{ type: 'title', text: name }] };
    if (n % 2 === 0) banner.push({ id: `b${n}`, type: 'banner', at, y: 64, design: 'nordreich', ...(namen ? { name } : {}), panel });
    else nadeln.push({ id: `n${n}`, type: 'pin', at, y: 64, name, size: 'large', symbol: { large: 'images/burg_16.png', medium: 'images/burg_9.png' }, panel });
  }
  return { banner, nadeln };
}

/** Frames per rAF und lange Aufgaben von Beginn der Seite an, ohne `document.body`. */
const BEOBACHTER = () => {
  const w = window as unknown as { frames_: number[]; lang_: [number, number][] };
  w.frames_ = [];
  w.lang_ = [];
  const takt = (t: number) => {
    w.frames_.push(t);
    requestAnimationFrame(takt);
  };
  requestAnimationFrame(takt);
  new PerformanceObserver((l) => {
    for (const e of l.getEntries()) w.lang_.push([e.startTime, e.duration]);
  }).observe({ type: 'longtask', buffered: true });
};

interface Abschnitt {
  frames: number;
  p50: number;
  p95: number;
  ueber50: number;
  lang_ms: number;
  laengste_ms: number;
}

/** Frames und lange Aufgaben zwischen zwei Zeitpunkten der Seite. */
const abschnitt = (page: Page, von: number, bis: number): Promise<Abschnitt> =>
  page.evaluate(
    ([von, bis]) => {
      const w = window as unknown as { frames_: number[]; lang_: [number, number][] };
      const t = w.frames_.filter((f) => f >= von && f <= bis);
      const d = t.slice(1).map((f, i) => f - t[i]!).sort((a, b) => a - b);
      const q = (p: number) => (d.length ? Math.round(d[Math.min(d.length - 1, Math.floor(p * d.length))]! * 10) / 10 : 0);
      const lang = w.lang_.filter(([s]) => s >= von && s <= bis).map(([, l]) => l);
      return {
        frames: d.length,
        p50: q(0.5),
        p95: q(0.95),
        ueber50: d.length ? Math.round((d.filter((x) => x > 50).length / d.length) * 1000) / 10 : 0,
        lang_ms: Math.round(lang.reduce((s, l) => s + l, 0)),
        laengste_ms: Math.round(Math.max(0, ...lang)),
      };
    },
    [von, bis] as [number, number],
  );

const jetzt = (page: Page) => page.evaluate(() => performance.now());

/** Misst, was `tu` auslöst, bis 600 ms danach. */
async function miss(page: Page, tu: () => Promise<unknown>): Promise<Abschnitt> {
  const von = await jetzt(page);
  await tu();
  await page.waitForTimeout(600);
  return abschnitt(page, von, await jetzt(page));
}

async function lauf(browser: Browser, variante: keyof typeof VARIANTEN, dpr: number, cpu: number): Promise<object> {
  const v = VARIANTEN[variante];
  // Ein eigener Kontext je Lauf, mit dpr und Touch; die Basis der Seite aus dem Projekt.
  const { baseURL } = test.info().project.use;
  const context = await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr, hasTouch: true });
  const page = await context.newPage();
  await page.addInitScript(BEOBACHTER);
  const { banner, nadeln } = orte(v.abstand, v.namen);
  await welt(page, {
    liste: () => [STAEDTE, NADELN],
    datei: (name) => (name === 'staedte' ? { ...STAEDTE, objects: banner } : name === 'nadeln' ? { ...NADELN, objects: nadeln } : undefined),
    baum: { path: '2x1-se', camera: '2:1' },
    mehr: { bounds: [-65536, -65536, 65536, 65536] },
    bild: (n) => BILDER[n],
    weitere: (pfad) => {
      const rest = pfad.startsWith('beispiel/banner/staedte/2x1-se/') ? pfad.slice('beispiel/banner/staedte/2x1-se/'.length) : undefined;
      return rest === 'satz.json' ? SATZ : rest === 'nordreich.png' ? SPRITE : undefined;
    },
  });
  // Später angemeldet, geht vor: keine Kacheln, keine Höhen.
  await page.route('**/*.webp', (r) => r.fulfill({ status: 404 }));
  await page.route('**/heights/*.bin', (r) => r.fulfill({ status: 404 }));
  const cdp: CDPSession = await context.newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: cpu });
  const start = Date.now();
  await page.goto(`${DEMO}&tree=2x1-se&at=0,64,0&zoom=${v.zoom}${process.env.VERSUCH ? `&versuch=${process.env.VERSUCH}` : ''}`);
  // Bis die Zahl der Icons 1 s lang steht: Nach 0128 hängen nur die im Fenster mit Rand.
  let [zahl, seit] = [-1, Date.now()];
  for (;;) {
    const n = await page.locator('.nadel-icon').count();
    if (n !== zahl) [zahl, seit] = [n, Date.now()];
    else if (zahl > 0 && Date.now() - seit > 1000) break;
    await page.waitForTimeout(100);
  }
  const laden_ms = seit - start;
  const laden = await abschnitt(page, 0, await jetzt(page));
  await page.waitForTimeout(1000);
  const b = { x: 640, y: 400 };
  // Mit `SPUR`: eine Chrome-Spur nur des Ziehens, je Lauf eine Datei.
  if (process.env.SPUR) await browser.startTracing(page, { categories: ['devtools.timeline', 'disabled-by-default-devtools.timeline'] });
  const ziehen = await miss(page, async () => {
    await page.mouse.move(b.x, b.y);
    await page.mouse.down();
    for (let i = 1; i <= 60; i++) {
      await page.mouse.move(b.x + i * 8, b.y + i * 3);
      await page.waitForTimeout(33);
    }
    await page.mouse.up();
  });
  if (process.env.SPUR) appendFileSync(`${process.env.SPUR}-${variante}-${dpr}-${cpu}-${Date.now()}.json`, await browser.stopTracing());
  // Hinein und hinaus getrennt: Hinaus kommen viele Orte auf einmal ins Fenster.
  const rasten = (d: number) => async () => {
    for (let i = 0; i < 3; i++) {
      await page.mouse.wheel(0, d);
      await page.waitForTimeout(500);
    }
  };
  const radRein = await miss(page, rasten(-120));
  const radRaus = await miss(page, rasten(120));
  const pinch = (scaleFactor: number) => () => cdp.send('Input.synthesizePinchGesture', { x: b.x, y: b.y, scaleFactor, relativeSpeed: 600, gestureSourceType: 'touch' });
  const pinchRein = await miss(page, pinch(2));
  const pinchRaus = await miss(page, pinch(0.5));
  const zustand = await page.evaluate(() => {
    const icons = [...document.querySelectorAll('.nadel-icon')];
    const leinwand = [...document.querySelectorAll<HTMLCanvasElement>('.nadel-icon canvas')].reduce((s, c) => s + c.width * c.height * 4, 0);
    const imAusschnitt = icons.filter((e) => {
      const r = e.getBoundingClientRect();
      return r.right > 0 && r.bottom > 0 && r.left < innerWidth && r.top < innerHeight;
    }).length;
    return { leinwand_mib: Math.round((leinwand / 2 ** 20) * 10) / 10, imAusschnitt, imDom: icons.length };
  });
  await cdp.send('HeapProfiler.collectGarbage');
  const { usedSize } = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number };
  const { nodes } = (await cdp.send('Memory.getDOMCounters')) as { nodes: number };
  await context.close();
  return { stand: process.env.STAND ?? '', variante, dpr, cpu, laden_ms, laden, ziehen, radRein, radRaus, pinchRein, pinchRaus, ...zustand, heap_mb: Math.round(usedSize / 1e5) / 10, dom_knoten: nodes };
}

for (const variante of Object.keys(VARIANTEN) as (keyof typeof VARIANTEN)[]) {
  for (const dpr of [1, 3]) {
    for (const cpu of [1, 4]) {
      test(`${variante} dpr ${dpr} CPU ${cpu}×`, async ({ browser }) => {
        test.setTimeout(1_200_000);
        for (let n = 1; n <= Number(process.env.LAEUFE ?? 3); n++) {
          const zeile = JSON.stringify({ n, ...(await lauf(browser, variante, dpr, cpu)) });
          console.log(zeile);
          if (process.env.AUS) appendFileSync(process.env.AUS, `${zeile}\n`);
        }
      });
    }
  }
}
