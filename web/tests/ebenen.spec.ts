import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { projiziere, zweiZuEins } from '../src/pick';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
const DEMO = '/?tiles=/tiles-demo';
const BURG_16 = readFileSync(new URL('fixtures/ebenen/burg_16.png', import.meta.url));
const BURG_9 = readFileSync(new URL('fixtures/ebenen/burg_9.png', import.meta.url));

const STAEDTE = { id: 'beispiel:staedte', name: { de: 'Städte', en: 'Towns' }, visible: true, order: 100, version: 'a' };
const KREISE = { id: 'beispiel:stadtinfos', name: { de: 'Stadtinfos', en: 'Town info' }, visible: false, order: 99, version: 'a' };

/** Eine Nadel mit Tafel, wie das Plugin für Städte sie schreibt. */
const HAFEN = {
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

/**
 * Der Demobaum mit Höhen eben auf Y 0 und diesen Ebenen. `mehr` geht in
 * map.json, `objekte` in die Datei der Städte; `anfragen` zählt, welche
 * Dateien der Ebenen die Seite holt.
 */
async function welt(page: Page, liste: object[], objekte: object[], mehr: object = {}): Promise<string[]> {
  const anfragen: string[] = [];
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    await route.fulfill({ response, json: { ...info, heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319, seaLevel: 0, ...mehr } });
  });
  await page.route('**/tiles-demo/heights/*.bin', (route) => route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) }));
  await page.route('**/tiles-demo/layers.json', (route) => route.fulfill({ json: { layers: liste } }));
  await page.route('**/tiles-demo/layers/**', (route) => {
    const pfad = new URL(route.request().url()).pathname;
    anfragen.push(pfad.replace('/tiles-demo/layers/', ''));
    if (pfad.endsWith('/staedte.json')) return route.fulfill({ json: { ...STAEDTE, objects: objekte } });
    if (pfad.endsWith('/stadtinfos.json')) return route.fulfill({ json: { ...KREISE, objects: [{ ...HAFEN, id: 'kreis-pin', at: [40.5, -10.5] }] } });
    if (pfad.endsWith('/images/burg_16.png')) return route.fulfill({ body: BURG_16, contentType: 'image/png' });
    if (pfad.endsWith('/images/burg_9.png')) return route.fulfill({ body: BURG_9, contentType: 'image/png' });
    return route.fulfill({ status: 404 });
  });
  return anfragen;
}

/** Wo der Fuss einer Nadel auf dem Schirm liegt: Mitte der Unterkante ihres Icons. */
const fuss = (page: Page, name: string) =>
  page.locator(`.nadel-icon[title="${name}"]`).evaluate((e) => {
    const r = e.getBoundingClientRect();
    return [r.left + Math.floor(r.width / 2), r.bottom];
  });

test('ohne layers.json gibt es keine Liste der Ebenen', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.ebenen')).toHaveCount(0);
});

test('die Liste nennt die Ebenen nach order, an oder aus nach visible; die Wahl bleibt nach dem Neuladen', async ({ page }) => {
  const anfragen = await welt(page, [KREISE, STAEDTE], [HAFEN]);
  await page.goto(DEMO);
  const liste = page.locator('.ebenen');
  await liste.locator('summary').click();
  await expect(liste.locator('label')).toHaveText([/Städte|Towns/, /Stadtinfos|Town info/]);
  await expect(liste.locator('input[data-id="beispiel:staedte"]')).toBeChecked();
  await expect(liste.locator('input[data-id="beispiel:stadtinfos"]')).not.toBeChecked();
  // Eine verborgene Ebene lädt erst beim Einschalten.
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  expect(anfragen).not.toContain('beispiel/stadtinfos.json');
  await liste.locator('input[data-id="beispiel:stadtinfos"]').check();
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  await liste.locator('input[data-id="beispiel:staedte"]').uncheck();
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  await page.reload();
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen input[data-id="beispiel:staedte"]')).not.toBeChecked();
  await expect(page.locator('.ebenen input[data-id="beispiel:stadtinfos"]')).toBeChecked();
});

for (const [richtung, drehe] of [
  ['se', (x: number, z: number): [number, number] => [x, z]],
  ['sw', (x: number, z: number): [number, number] => [z, -x]],
  ['nw', (x: number, z: number): [number, number] => [-x, -z]],
] as const) {
  test(`aus ${richtung} steht eine Nadel mit ihrer Spitze auf dem Gelände, mit y auf dessen Block, ohne y auf der Höhe aus map.json`, async ({ page }) => {
    const mitY = { ...HAFEN, id: 'mit-y', name: 'Mit Y', at: [20.5, -30.5], y: 5 };
    await welt(page, [STAEDTE], [HAFEN, mitY], { direction: richtung });
    await page.goto(DEMO);
    await expect(page.locator('.nadel-icon')).toHaveCount(2);
    const p = zweiZuEins(16);
    for (const [name, x, y, z] of [
      ['Hafenstadt', 35.5, 1, -14.5],
      ['Mit Y', 20.5, 6, -30.5],
    ] as const) {
      // Gegen eine eigene Drehung des Punkts: im Blick (x, z) → (z, −x), je Vierteldrehung.
      const [bx, bz] = drehe(x, z);
      const [px, py] = projiziere(bx, y, bz, p);
      const soll = await page.evaluate(([px, py]: [number, number]) => {
        // Die Kachel 2/0/0 liegt bei Pixel (0, 0) der feinsten Stufe; auf Stufe 2 deckt ein Pixel einen.
        const kachel = document.querySelector('img[src$="/2/0/0.webp"]')!.getBoundingClientRect();
        return [kachel.left + px, kachel.top + py];
      }, [px, py] as [number, number]);
      const ist = await fuss(page, name);
      expect(Math.abs(ist[0]! - soll[0]!), `${name} x`).toBeLessThanOrEqual(1);
      expect(Math.abs(ist[1]! - soll[1]!), `${name} y`).toBeLessThanOrEqual(1);
    }
  });
}

test('das Feld des Schilds trägt color, Rahmen und Symbol nicht; Pixel für Pixel wie im Mod', async ({ page }) => {
  await welt(page, [STAEDTE], [HAFEN]);
  await page.goto(DEMO);
  const leinwand = page.locator('.nadel canvas').first();
  await expect(leinwand).toHaveJSProperty('width', 23);
  const abweichend = await leinwand.evaluate(async (c: HTMLCanvasElement) => {
    const adresse = (muster: RegExp) => performance.getEntriesByType('resource').map((e) => e.name).find((n) => muster.test(n))!;
    const lade = async (url: string) => {
      const img = new Image();
      img.src = url;
      await img.decode();
      return img;
    };
    const [feld, rahmen, symbol] = await Promise.all([lade(adresse(/schild_gross-[^/]*\.png$/)), lade(adresse(/schild_gross_rahmen-[^/]*\.png$/)), lade(adresse(/burg_16\.png$/))]);
    const soll = new OffscreenCanvas(23, 33).getContext('2d')!;
    soll.drawImage(feld, 0, 0);
    const d = soll.getImageData(0, 0, 23, 33);
    const farbe = [0x40, 0xe5, 0x3f];
    for (let i = 0; i < d.data.length; i += 4) for (let k = 0; k < 3; k++) d.data[i + k] = Math.floor((d.data[i + k]! * farbe[k]!) / 255);
    soll.putImageData(d, 0, 0);
    soll.drawImage(rahmen, 0, 0);
    soll.drawImage(symbol, 3, 3);
    const a = soll.getImageData(0, 0, 23, 33).data;
    const b = c.getContext('2d')!.getImageData(0, 0, 23, 33).data;
    let n = 0;
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) n++;
    return n;
  });
  expect(abweichend).toBe(0);
});

test('beim Hinauszoomen wird eine Nadel kleiner und verschwindet zuletzt, Städte nach Dörfern; den Namen hat sie nur in ihrer Grundgrösse', async ({ page }) => {
  const dorf = { ...HAFEN, id: 'dorf', name: 'Dorf', size: 'small', at: [30.5, -20.5], panel: undefined };
  // scale 4: ein Block ist auf Stufe 2 vier Pixel breit, je Stufe tiefer halb so breit.
  await welt(page, [STAEDTE], [HAFEN, dorf], { scale: 4, minZoom: -6 });
  const breiten: Record<number, number[]> = {};
  const namen: Record<number, number> = {};
  for (const zoom of [0, -3, -4, -6, -8]) {
    await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
    await page.waitForTimeout(500);
    breiten[zoom] = await page.locator('.nadel canvas').evaluateAll((l) => l.map((c) => (c as HTMLCanvasElement).width).sort((a, b) => b - a));
    namen[zoom] = await page.locator('.nadel-name').count();
  }
  // Stufe 2 + zoom; Breite eines Blocks 4 · 2^zoom Pixel.
  expect(breiten[0]).toEqual([23, 9]); // 4 px: Grundgrösse
  expect(namen[0]).toBe(2);
  expect(breiten[-3]).toEqual([23, 9]); // 1/2 px: noch Grundgrösse
  expect(breiten[-4]).toEqual([15]); // 1/4 px: eine kleiner, das Dorf ist aus
  expect(namen[-4]).toBe(0);
  expect(breiten[-6]).toEqual([9]); // 1/16 px: zwei kleiner
  expect(breiten[-8]).toEqual([]); // 1/64 px: aus
});

test('die Tafel zeigt Bausteine als Text und Bilder vom eigenen Server, nie Markup; zu hoch, scrollt sie', async ({ page }) => {
  const lang = { ...HAFEN, id: 'lang', name: 'Lang', at: [44.5, -20.5], panel: { blocks: [{ type: 'lines', lines: Array.from({ length: 60 }, (_, i) => `Zeile ${i}`) }] } };
  await welt(page, [STAEDTE], [HAFEN, lang]);
  await page.setViewportSize({ width: 800, height: 400 });
  await page.goto(DEMO);
  await page.locator('.nadel-icon').first().click();
  const tafel = page.locator('.tafel .leaflet-popup-content');
  await expect(tafel.locator('.tafel-titel')).toHaveText('✪ Hafenstadt');
  await expect(tafel.locator('.tafel-titel')).toHaveCSS('color', 'rgb(64, 229, 63)');
  // Markup bleibt Text.
  await expect(tafel.locator('.tafel-zeilen')).toContainText('Nation: <b>Nordreich</b>');
  await expect(tafel.locator('b')).toHaveCount(0);
  // Bilder nur unter images/ vom eigenen Server, kein data:.
  const bilder = await tafel.locator('img').evaluateAll((l) => l.map((i) => (i as HTMLImageElement).src));
  expect(bilder).toHaveLength(1);
  expect(bilder[0]).toMatch(/\/tiles-demo\/layers\/beispiel\/images\/burg_16\.png$/);
  await expect(tafel.locator('.tafel-ueberschrift')).toHaveText('Statistiken');
  const punkte = tafel.locator('.tafel-punkt');
  await expect(punkte).toHaveCount(4);
  expect(await punkte.evaluateAll((l) => l.map((p) => getComputedStyle(p).opacity))).toEqual(['1', '1', '1', '0.25']);
  // Die lange Tafel scrollt.
  await page.locator('.leaflet-popup-close-button').click();
  await page.locator('.nadel-icon[title="Lang"]').click();
  // Die erste Tafel blendet noch aus; es zählt die mit den Zeilen.
  const lange = page.locator('.tafel .leaflet-popup-content', { hasText: 'Zeile 0' });
  await expect(lange).toContainText('Zeile 59');
  expect(await lange.evaluate((e) => e.scrollHeight > e.clientHeight && getComputedStyle(e).overflowY === 'auto')).toBe(true);
});

test('alle 30 Sekunden fragt die Karte layers.json nach und lädt eine Ebene neu, deren version sich geändert hat', async ({ page }) => {
  await page.clock.install();
  let liste = [STAEDTE];
  let objekte: object[] = [HAFEN];
  await welt(page, [], []);
  await page.route('**/tiles-demo/layers.json', (route) => route.fulfill({ json: { layers: liste } }));
  await page.route('**/tiles-demo/layers/beispiel/staedte.json', (route) => route.fulfill({ json: { ...STAEDTE, objects: objekte } }));
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  objekte = [HAFEN, { ...HAFEN, id: 'neu', name: 'Neu', at: [20.5, -30.5] }];
  // Ohne neue version bleibt es, wie es ist.
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  liste = [{ ...STAEDTE, version: 'b' }];
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
});

test('ein Symbol ausserhalb von images/ holt die Karte nicht, und unbekannte Objekte übergeht sie', async ({ page }) => {
  const anfragen = await welt(page, [STAEDTE], [
    { ...HAFEN, symbol: { large: '../geheim.png', medium: 'images/../x.png' } },
    { id: 'neu', type: 'hologram', at: [1, 1] },
    { ...HAFEN, id: 'ohne-at', at: 'hier' },
  ]);
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  await page.waitForTimeout(300);
  expect(anfragen.filter((a) => a.endsWith('.png'))).toEqual([]);
});
