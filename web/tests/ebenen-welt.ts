/** Eine Welt für die Tests der Ebenen: der Demobaum mit Höhen und Ebenen, die der Test liefert. */
import type { Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { crc32, deflateSync } from 'node:zlib';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
export const DEMO = '/?tiles=/tiles-demo';
export const BILDER: Record<string, Buffer> = {
  'burg_16.png': readFileSync(new URL('fixtures/ebenen/burg_16.png', import.meta.url)),
  'burg_9.png': readFileSync(new URL('fixtures/ebenen/burg_9.png', import.meta.url)),
  'rot_16.png': readFileSync(new URL('fixtures/ebenen/rot_16.png', import.meta.url)),
};
export const LEER = -32768;

/** Ein PNG b × h, RGBA, jedes Pixel anders gefärbt: (x · 7, y · 3, 128, 255). */
export function png(b: number, h: number): Buffer {
  const teil = (art: string, daten: Buffer) => {
    const laenge = Buffer.alloc(4);
    laenge.writeUInt32BE(daten.length);
    const rest = Buffer.concat([Buffer.from(art, 'latin1'), daten]);
    const pruef = Buffer.alloc(4);
    pruef.writeUInt32BE(crc32(rest));
    return Buffer.concat([laenge, rest, pruef]);
  };
  const kopf = Buffer.alloc(13);
  kopf.writeUInt32BE(b, 0);
  kopf.writeUInt32BE(h, 4);
  kopf.set([8, 6, 0, 0, 0], 8);
  const zeilen = Buffer.alloc(h * (1 + 4 * b));
  for (let y = 0; y < h; y++) for (let x = 0; x < b; x++) zeilen.set([(x * 7) % 256, (y * 3) % 256, 128, 255], y * (1 + 4 * b) + 1 + 4 * x);
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), teil('IHDR', kopf), teil('IDAT', deflateSync(zeilen)), teil('IEND', Buffer.alloc(0))]);
}

export const STAEDTE = { id: 'beispiel:staedte', name: { de: 'Städte', en: 'Towns' }, visible: true, order: 100, version: 'a' };
export const KREISE = { id: 'beispiel:stadtinfos', name: { de: 'Stadtinfos', en: 'Town info' }, visible: false, order: 99, version: 'a' };

/** Eine Nadel mit Tafel, wie das Plugin für Städte sie schreibt. */
export const HAFEN = {
  id: 'stadt-17',
  type: 'pin',
  at: [35.5, -14.5],
  name: 'Hafenstadt',
  size: 'large',
  symbol: { large: 'images/burg_16.png', medium: 'images/burg_9.png' },
  color: '#40E53F',
  panel: {
    blocks: [
      { type: 'columns', columns: [
        [{ type: 'title', text: '✪ Hafenstadt', color: '#40E53F' }, { type: 'lines', lines: ['Nation: <b>Nordreich</b>', 'Level: 3'] }],
        [{ type: 'image', image: 'images/burg_16.png', width: 32, height: 32 }],
      ] },
      { type: 'section', heading: { text: 'Statistiken' }, blocks: [
        { type: 'rating', rows: [{ label: 'Bergbau', value: 3, max: 4, color: '#E5C33F' }] },
      ] },
      { type: 'image', image: 'data:image/png;base64,AAAA', width: 8, height: 8 },
      { type: 'video', src: 'x' },
    ],
  },
};

export interface Welt {
  /** Ebenen in `layers.json`, als Funktion für Änderungen während des Tests. */
  liste: () => object[];
  /** Die Datei einer Ebene nach ihrem Namen, etwa `staedte`. */
  datei: (name: string) => object | undefined;
  /** Felder für map.json. */
  mehr?: object;
  /** Höhe einer Zelle nach ihrem Index in der Welt; ohne Angabe eben auf 0. Regionen mit x ≥ 512 fehlen. */
  hoehe?: (i: number, j: number) => number;
  /** Ein Bild unter `images/`, nach Name. */
  bild?: (name: string) => Buffer | undefined;
}

/** Der Demobaum mit Höhen und Ebenen, die der Test liefert; gibt die Pfade der geholten Dateien der Ebenen zurück. */
export async function welt(page: Page, w: Welt): Promise<string[]> {
  const anfragen: string[] = [];
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    await route.fulfill({ response, json: { ...info, heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319, seaLevel: 0, ...w.mehr } });
  });
  await page.route('**/tiles-demo/heights/*.bin', (route) => {
    const [rx, rz] = /\/(-?\d+)\.(-?\d+)\.bin$/.exec(route.request().url())!.slice(1).map(Number) as [number, number];
    if (rx >= 1) return route.fulfill({ status: 404 });
    const karte = new Int16Array(128 * 128);
    if (w.hoehe) for (let lj = 0; lj < 128; lj++) for (let li = 0; li < 128; li++) karte[lj * 128 + li] = w.hoehe(rx * 128 + li, rz * 128 + lj);
    return route.fulfill({ body: deflateSync(Buffer.from(karte.buffer)) });
  });
  await page.route('**/tiles-demo/layers.json', (route) => route.fulfill({ json: { layers: w.liste() } }));
  await page.route('**/tiles-demo/layers/**', (route) => {
    const pfad = new URL(route.request().url()).pathname;
    anfragen.push(pfad.replace('/tiles-demo/layers/', ''));
    const datei = /\/beispiel\/([^/]+)\.json$/.exec(pfad)?.[1];
    if (datei) {
      const inhalt = w.datei(datei);
      return inhalt ? route.fulfill({ json: inhalt }) : route.fulfill({ status: 404 });
    }
    const name = /\/images\/([^/]+)$/.exec(pfad)?.[1];
    const bild = name && (w.bild ?? ((n: string) => BILDER[n]))(name);
    return bild ? route.fulfill({ body: bild, contentType: 'image/png' }) : route.fulfill({ status: 404 });
  });
  return anfragen;
}

/** Eine Welt mit den Städten und diesen Nadeln. */
export const staedte = (objekte: object[], mehr: Partial<Welt> = {}): Welt => ({
  liste: () => [STAEDTE],
  datei: (name) => (name === 'staedte' ? { ...STAEDTE, objects: objekte } : undefined),
  ...mehr,
});

/** Wo die Spitze einer Nadel auf dem Schirm liegt: Mitte der Unterkante ihres Icons. */
export const fuss = (page: Page, name: string) =>
  page.locator(`.nadel-icon[title="${name}"]`).evaluate((e) => {
    const r = e.getBoundingClientRect();
    return [r.left + Math.floor(r.width / 2), r.bottom];
  });

/** Wo ein Punkt der feinsten Stufe auf dem Schirm liegt: Die Kachel 2/0/0 beginnt bei (0, 0), auf Stufe 2 deckt ein Pixel einen. */
export const aufDemSchirm = (page: Page, px: number, py: number) =>
  page.evaluate(([px, py]: [number, number]) => {
    const kachel = document.querySelector('img[src$="/2/0/0.webp"]')!.getBoundingClientRect();
    return [kachel.left + px, kachel.top + py];
  }, [px, py] as [number, number]);
