import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { projiziere, zweiZuEins } from '../src/pick';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
const DEMO = '/?tiles=/tiles-demo';
const BILDER: Record<string, Buffer> = {
  'burg_16.png': readFileSync(new URL('fixtures/ebenen/burg_16.png', import.meta.url)),
  'burg_9.png': readFileSync(new URL('fixtures/ebenen/burg_9.png', import.meta.url)),
  'rot_16.png': readFileSync(new URL('fixtures/ebenen/rot_16.png', import.meta.url)),
};
const LEER = -32768;

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

interface Welt {
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
async function welt(page: Page, w: Welt): Promise<string[]> {
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
const staedte = (objekte: object[], mehr: Partial<Welt> = {}): Welt => ({
  liste: () => [STAEDTE],
  datei: (name) => (name === 'staedte' ? { ...STAEDTE, objects: objekte } : undefined),
  ...mehr,
});

/** Wo die Spitze einer Nadel auf dem Schirm liegt: Mitte der Unterkante ihres Icons. */
const fuss = (page: Page, name: string) =>
  page.locator(`.nadel-icon[title="${name}"]`).evaluate((e) => {
    const r = e.getBoundingClientRect();
    return [r.left + Math.floor(r.width / 2), r.bottom];
  });

/** Wo ein Punkt der feinsten Stufe auf dem Schirm liegt: Die Kachel 2/0/0 beginnt bei (0, 0), auf Stufe 2 deckt ein Pixel einen. */
const aufDemSchirm = (page: Page, px: number, py: number) =>
  page.evaluate(([px, py]: [number, number]) => {
    const kachel = document.querySelector('img[src$="/2/0/0.webp"]')!.getBoundingClientRect();
    return [kachel.left + px, kachel.top + py];
  }, [px, py] as [number, number]);

test('ohne layers.json gibt es keine Liste der Ebenen', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.ebenen')).toHaveCount(0);
});

test('die Liste nennt die Ebenen nach order, bei Gleichstand nach id, an oder aus nach visible; die Wahl bleibt nach dem Neuladen', async ({ page }) => {
  const gleich = { ...STAEDTE, id: 'beispiel:aaa', name: { de: 'Erste' } };
  const anfragen = await welt(page, {
    liste: () => [KREISE, STAEDTE, gleich],
    datei: (name) => ({ staedte: { objects: [HAFEN] }, stadtinfos: { objects: [{ ...HAFEN, id: 'k', name: 'Kreis', at: [40.5, -10.5] }] }, aaa: { objects: [] } })[name],
  });
  await page.goto(DEMO);
  const liste = page.locator('.ebenen');
  await liste.locator('summary').click();
  await expect(liste.locator('label')).toHaveText([/Erste/, /Städte|Towns/, /Stadtinfos|Town info/]);
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
    await welt(page, staedte([HAFEN, mitY], { mehr: { direction: richtung } }));
    await page.goto(DEMO);
    await expect(page.locator('.nadel-icon')).toHaveCount(2);
    for (const [name, x, y, z] of [
      ['Hafenstadt', 35.5, 1, -14.5],
      ['Mit Y', 20.5, 6, -30.5],
    ] as const) {
      // Gegen eine eigene Drehung des Punkts: im Blick (x, z) → (z, −x), je Vierteldrehung.
      const [bx, bz] = drehe(x, z);
      const soll = await aufDemSchirm(page, ...projiziere(bx, y, bz, zweiZuEins(16)));
      const ist = await fuss(page, name);
      expect(Math.abs(ist[0]! - soll[0]!), `${name} x`).toBeLessThanOrEqual(1);
      expect(Math.abs(ist[1]! - soll[1]!), `${name} y`).toBeLessThanOrEqual(1);
    }
  });
}

test('ohne y steht die Nadel auf der Oberfläche: bilinear zwischen den Zellen, eine leere Zelle aus ihren Nachbarn, ohne Höhen auf seaLevel', async ({ page }) => {
  // Gefälle in x und z, eine leere Zelle unter den vier um die Nadel.
  const hoehe = (i: number, j: number) => (i === 9 && j === -4 ? LEER : 10 + i + 3 * j);
  const zwischen = { ...HAFEN, id: 'zwischen', name: 'Zwischen', at: [35.5, -14.5] };
  const ohne = { ...HAFEN, id: 'ohne', name: 'Ohne', at: [600.5, -14.5] };
  await welt(page, staedte([zwischen, ohne], { mehr: { seaLevel: 40 }, hoehe }));
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  // Die Regel aus ebenen.md, hier unabhängig nachgebaut.
  const zelle = (i: number, j: number) => {
    const eigen = hoehe(i, j);
    if (eigen !== LEER) return eigen;
    const n: number[] = [];
    for (let di = -2; di <= 2; di++) for (let dj = -2; dj <= 2; dj++) if (hoehe(i + di, j + dj) !== LEER) n.push(hoehe(i + di, j + dj));
    return n.reduce((a, b) => a + b, 0) / n.length;
  };
  const [fx, fz] = [35.5 / 4 - 0.5, -14.5 / 4 - 0.5];
  const [i, j, tx, tz] = [Math.floor(fx), Math.floor(fz), fx - Math.floor(fx), fz - Math.floor(fz)];
  expect([i, j, i + 1, j + 1]).toEqual([8, -5, 9, -4]);
  const h = (zelle(i, j) * (1 - tx) + zelle(i + 1, j) * tx) * (1 - tz) + (zelle(i, j + 1) * (1 - tx) + zelle(i + 1, j + 1) * tx) * tz + 1;
  for (const [name, x, y, z] of [
    ['Zwischen', 35.5, h, -14.5],
    // Keine Höhen dort, kein Nachbar: seaLevel + 1.
    ['Ohne', 600.5, 41, -14.5],
  ] as const) {
    const soll = await aufDemSchirm(page, ...projiziere(x, y, z, zweiZuEins(16)));
    const ist = await fuss(page, name);
    expect(Math.abs(ist[0]! - soll[0]!), `${name} x`).toBeLessThanOrEqual(1);
    expect(Math.abs(ist[1]! - soll[1]!), `${name} y`).toBeLessThanOrEqual(1);
  }
});

/** Das Schild, wie es nach ebenen.md sein muss, aus den Bildern des Designers, mit oder ohne Symbol. */
const schildSoll = (page: Page, groesse: string, b: number, h: number, farbe: number[], symbol: string | undefined) =>
  page.evaluate(
    async ({ groesse, b, h, farbe, symbol }) => {
      const adresse = (muster: RegExp) => performance.getEntriesByType('resource').map((e) => e.name).find((n) => muster.test(n))!;
      const lade = async (url: string) => {
        const img = new Image();
        img.src = url;
        await img.decode();
        return img;
      };
      const [feld, rahmen] = await Promise.all([lade(adresse(new RegExp(`schild_${groesse}-[^/]*\\.png$`))), lade(adresse(new RegExp(`schild_${groesse}_rahmen-[^/]*\\.png$`)))]);
      const soll = new OffscreenCanvas(b, h).getContext('2d')!;
      soll.drawImage(feld, 0, 0);
      const d = soll.getImageData(0, 0, b, h);
      for (let i = 0; i < d.data.length; i += 4) for (let k = 0; k < 3; k++) d.data[i + k] = Math.floor((d.data[i + k]! * farbe[k]!) / 255);
      soll.putImageData(d, 0, 0);
      if (symbol) {
        const s = await lade(adresse(new RegExp(symbol)));
        soll.drawImage(s, Math.floor((b - s.width) / 2), 3);
      }
      soll.drawImage(rahmen, 0, 0);
      return [...soll.getImageData(0, 0, b, h).data];
    },
    { groesse, b, h, farbe, symbol },
  );

test('das Feld des Schilds trägt color, abgeschnitten, das Symbol liegt unter dem Rahmen; Pixel für Pixel in allen drei Grössen', async ({ page }) => {
  await welt(page, staedte([
    { ...HAFEN, id: 'g', name: 'G', size: 'large', at: [30.5, -20.5] },
    { ...HAFEN, id: 'm', name: 'M', size: 'medium', at: [35.5, -14.5] },
    { ...HAFEN, id: 'k', name: 'K', size: 'small', at: [40.5, -10.5] },
  ]));
  await page.goto(DEMO);
  for (const [name, groesse, b, h, symbol] of [
    ['G', 'gross', 23, 33, 'burg_16\\.png'],
    ['M', 'mittel', 15, 23, 'burg_9\\.png'],
    ['K', 'klein', 9, 15, undefined],
  ] as const) {
    const leinwand = page.locator(`.nadel-icon[title="${name}"] canvas`);
    await expect(leinwand).toHaveJSProperty('width', b);
    const ist = await leinwand.evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data]);
    expect(ist, name).toEqual(await schildSoll(page, groesse, b, h, [0x40, 0xe5, 0x3f], symbol));
  }
});

test('ein Symbol in falscher Grösse bleibt weg, das Schild leer, und die Konsole sagt es', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  await welt(page, staedte([{ ...HAFEN, size: 'medium', symbol: { medium: 'images/burg_16.png' } }]));
  await page.goto(DEMO);
  const leinwand = page.locator('.nadel canvas');
  await expect(leinwand).toHaveJSProperty('width', 15);
  await expect.poll(() => meldungen.some((m) => m.includes('burg_16.png') && m.includes('statt 9 × 9'))).toBe(true);
  const ist = await leinwand.evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data]);
  expect(ist).toEqual(await schildSoll(page, 'mittel', 15, 23, [0x40, 0xe5, 0x3f], undefined));
});

test('beim Hinauszoomen wird eine Nadel kleiner und verschwindet zuletzt, Städte nach Dörfern, mit den Schwellen ½, ⅛ und 1/32; den Namen hat sie nur in ihrer Grundgrösse', async ({ page }) => {
  const dorf = { ...HAFEN, id: 'dorf', name: 'Dorf', size: 'small', at: [30.5, -20.5], panel: undefined };
  // scale 4: ein Block ist auf Stufe 2 vier Pixel breit, je Stufe tiefer halb so breit.
  await welt(page, staedte([HAFEN, dorf], { mehr: { scale: 4, minZoom: -6 } }));
  const breiten: Record<number, number[]> = {};
  const namen: Record<number, number> = {};
  for (const zoom of [0, -3, -4, -5, -6, -7, -8]) {
    await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
    await page.waitForTimeout(500);
    breiten[zoom] = await page.locator('.nadel canvas').evaluateAll((l) => l.map((c) => (c as HTMLCanvasElement).width).sort((a, b) => b - a));
    namen[zoom] = await page.locator('.nadel-name').count();
  }
  // Stufe 2 + zoom; Breite eines Blocks 4 · 2^zoom Pixel.
  expect(breiten[0]).toEqual([23, 9]); // 4 px: Grundgrösse
  expect(namen[0]).toBe(2);
  expect(breiten[-3]).toEqual([23, 9]); // genau ½ px: noch Grundgrösse
  expect(breiten[-4]).toEqual([15]); // ¼ px: eine kleiner, das Dorf ist aus
  expect(namen[-4]).toBe(0);
  expect(breiten[-5]).toEqual([15]); // genau ⅛ px: noch eine kleiner
  expect(breiten[-6]).toEqual([9]); // 1/16 px: zwei kleiner
  expect(breiten[-7]).toEqual([9]); // genau 1/32 px: noch zwei kleiner
  expect(breiten[-8]).toEqual([]); // 1/64 px: aus
});

test('Ebenen liegen nach order übereinander, in einer Ebene die spätere oben, gleich wo auf dem Schirm', async ({ page }) => {
  // Je zwei Nadeln fast am selben Ort; die untere liegt 2 px tiefer auf dem Schirm.
  const oben = { ...STAEDTE, id: 'beispiel:oben', name: { de: 'Oben' }, order: 5 };
  const unten = { ...STAEDTE, id: 'beispiel:unten', name: { de: 'Unten' }, order: 1 };
  let liste = [unten, oben];
  await welt(page, {
    liste: () => liste,
    datei: (name) =>
      ({
        oben: { objects: [{ ...HAFEN, id: 'o', name: 'O', at: [35.5, -14.5] }] },
        unten: { objects: [{ ...HAFEN, id: 'u', name: 'U', at: [35.5, -14] }] },
        staedte: { objects: [{ ...HAFEN, id: 'a', name: 'A', at: [20.5, -30] }, { ...HAFEN, id: 'b', name: 'B', at: [20.5, -30.5] }] },
      })[name],
  });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  const oberste = async (name: string) => {
    const [x, y] = await fuss(page, name);
    return page.evaluate(([x, y]) => document.elementFromPoint(x!, y! - 12)?.closest('.nadel-icon')?.getAttribute('title'), [x, y]);
  };
  // Die Ebene mit der höheren order liegt oben, obwohl die andere tiefer steht.
  expect(await oberste('O')).toBe('O');
  liste = [STAEDTE];
  await page.reload();
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  // In der Ebene liegt B, die spätere, oben, obwohl A tiefer steht.
  expect(await oberste('B')).toBe('B');
});

test('wird eine Ebene ausgeschaltet, während sie lädt, zeigt sie danach nichts', async ({ page }) => {
  let freigeben = () => {};
  const frei = new Promise<void>((los) => (freigeben = los));
  await welt(page, { liste: () => [KREISE], datei: () => ({ objects: [HAFEN] }) });
  await page.route('**/tiles-demo/layers/beispiel/stadtinfos.json', async (route) => {
    await frei;
    await route.fulfill({ json: { objects: [HAFEN] } });
  });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  const box = page.locator('.ebenen input[data-id="beispiel:stadtinfos"]');
  await box.check();
  await box.uncheck();
  await box.check();
  await box.uncheck();
  freigeben();
  await page.waitForTimeout(800);
  await expect(page.locator('.leaflet-marker-icon')).toHaveCount(0);
});

test('wird eine Ebene ausgeschaltet, während die Höhen für ihre Nadeln laden, zeigt sie danach nichts', async ({ page }) => {
  let freigeben = () => {};
  const frei = new Promise<void>((los) => (freigeben = los));
  await welt(page, { liste: () => [KREISE], datei: () => ({ objects: [HAFEN] }) });
  await page.route('**/tiles-demo/heights/*.bin', async (route) => {
    await frei;
    await route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) });
  });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  const box = page.locator('.ebenen input[data-id="beispiel:stadtinfos"]');
  await box.check();
  // Die Datei ist da, die Nadeln warten auf ihre Höhen.
  await page.waitForTimeout(300);
  await box.uncheck();
  freigeben();
  await page.waitForTimeout(800);
  await expect(page.locator('.leaflet-marker-icon')).toHaveCount(0);
});

test('die Tafel zeigt Bausteine als Text und Bilder vom eigenen Server, nie Markup, in den Farben der UI; zu hoch, scrollt sie; keine Verletzung der Content-Security-Policy', async ({ page }) => {
  await page.addInitScript(() => {
    const verletzt: string[] = [];
    Object.assign(window, { verletzt });
    document.addEventListener('securitypolicyviolation', (e) => verletzt.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  const lang = { ...HAFEN, id: 'lang', name: 'Lang', at: [44.5, -20.5], panel: { blocks: [{ type: 'lines', lines: Array.from({ length: 60 }, (_, i) => `Zeile ${i}`) }] } };
  await welt(page, staedte([HAFEN, lang]));
  await page.setViewportSize({ width: 800, height: 400 });
  await page.goto(DEMO);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const tafel = page.locator('.tafel .leaflet-popup-content');
  await expect(tafel.locator('.tafel-titel')).toHaveText('✪ Hafenstadt');
  await expect(tafel.locator('.tafel-titel')).toHaveCSS('color', 'rgb(64, 229, 63)');
  // Markup bleibt Text.
  await expect(tafel.locator('.tafel-zeilen')).toContainText('Nation: <b>Nordreich</b>');
  await expect(tafel.locator('b')).toHaveCount(0);
  // Bilder nur unter images/ vom eigenen Server, mit der version der Ebene, kein data:.
  const bilder = await tafel.locator('img').evaluateAll((l) => l.map((i) => (i as HTMLImageElement).src));
  expect(bilder).toHaveLength(1);
  expect(bilder[0]).toMatch(/\/tiles-demo\/layers\/beispiel\/images\/burg_16\.png\?v=a$/);
  await expect(tafel.locator('.tafel-ueberschrift')).toHaveText('Statistiken');
  const punkte = tafel.locator('.tafel-punkt');
  await expect(punkte).toHaveCount(4);
  expect(await punkte.evaluateAll((l) => l.map((p) => getComputedStyle(p).opacity))).toEqual(['1', '1', '1', '0.25']);
  // Grund und Schrift aus den Variablen der UI.
  const [grund, schrift, uiGrund, uiSchrift] = await page.locator('.tafel .leaflet-popup-content-wrapper').evaluate((e) => {
    const probe = document.createElement('div');
    probe.style.background = 'var(--ui-grund)';
    probe.style.color = 'var(--ui-schrift)';
    document.querySelector('#map')!.append(probe);
    const werte = [getComputedStyle(e).backgroundColor, getComputedStyle(e).color, getComputedStyle(probe).backgroundColor, getComputedStyle(probe).color];
    probe.remove();
    return werte;
  });
  expect([grund, schrift]).toEqual([uiGrund, uiSchrift]);
  // Die lange Tafel scrollt.
  await page.locator('.leaflet-popup-close-button').click();
  await page.locator('.nadel-icon[title="Lang"]').click();
  // Die erste Tafel blendet noch aus; es zählt die mit den Zeilen.
  const lange = page.locator('.tafel .leaflet-popup-content', { hasText: 'Zeile 0' });
  await expect(lange).toContainText('Zeile 59');
  expect(await lange.evaluate((e) => e.scrollHeight > e.clientHeight && getComputedStyle(e).overflowY === 'auto')).toBe(true);
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual([]);
});

test('per Tastatur: eine Nadel mit Tafel ist ein Ziel, Enter öffnet sie mit dem Fokus darin, Escape schliesst und gibt ihn zurück; eine Nadel ohne Tafel ist kein Ziel', async ({ page }) => {
  const ohne = { ...HAFEN, id: 'ohne', name: 'Ohne', at: [20.5, -30.5], panel: undefined };
  await welt(page, staedte([HAFEN, ohne]));
  await page.goto(DEMO);
  const mit = page.locator('.nadel-icon[title="Hafenstadt"]');
  await expect(mit).toHaveAttribute('tabindex', '0');
  await expect(page.locator('.nadel-icon[title="Ohne"]')).not.toHaveAttribute('tabindex', /.*/);
  await mit.focus();
  await page.keyboard.press('Enter');
  const tafel = page.locator('.tafel .leaflet-popup-content');
  await expect(tafel).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('.tafel')).toHaveCount(0);
  await expect(mit).toBeFocused();
});

test('alle 30 Sekunden und beim Zurückkehren auf den Tab fragt die Karte layers.json mit no-cache nach; neu lädt nur eine Ebene mit neuer version, samt neuem Bild unter gleichem Namen; der Fokus in der Liste bleibt', async ({ page }) => {
  await page.addInitScript(() => {
    const modi: string[] = [];
    Object.assign(window, { modi });
    const holen = window.fetch.bind(window);
    window.fetch = (eingabe, init) => {
      if (String(eingabe instanceof Request ? eingabe.url : eingabe).endsWith('layers.json')) modi.push(String(init?.cache));
      return holen(eingabe, init);
    };
  });
  await page.clock.install();
  let version = 'a';
  let objekte: object[] = [HAFEN];
  let bild = 'burg_16.png';
  await welt(page, {
    liste: () => [{ ...STAEDTE, version }],
    datei: () => ({ objects: objekte }),
    bild: (name) => BILDER[name === 'burg_16.png' ? bild : name],
  });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  const pixel = () => page.locator('.nadel canvas').first().evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(11, 10, 1, 1).data].join());
  const vorher = await pixel();
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input').focus();
  // Ohne neue version bleibt es, wie es ist; der Fokus auch.
  objekte = [HAFEN, { ...HAFEN, id: 'neu', name: 'Neu', at: [20.5, -30.5] }];
  bild = 'rot_16.png';
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  await expect(page.locator('.ebenen input')).toBeFocused();
  // Mit neuer version: neu geladen, mit dem neuen Bild unter gleichem Namen.
  version = 'b';
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  await expect.poll(pixel).not.toBe(vorher);
  await expect(page.locator('.ebenen input')).toBeFocused();
  // Zurück auf den Tab: gleich nachfragen.
  const modi = () => page.evaluate(() => (window as unknown as { modi: string[] }).modi);
  const zahl = (await modi()).length;
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await expect.poll(async () => (await modi()).length).toBe(zahl + 1);
  expect(new Set(await modi())).toEqual(new Set(['no-cache']));
});

test('ein Symbol ausserhalb von images/ holt die Karte nicht, und unbekannte Objekte übergeht sie', async ({ page }) => {
  const anfragen = await welt(page, staedte([
    { ...HAFEN, symbol: { large: '../geheim.png', medium: 'images/../x.png' } },
    { id: 'neu', type: 'hologram', at: [1, 1] },
    { ...HAFEN, id: 'ohne-at', at: 'hier' },
  ]));
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  await page.waitForTimeout(300);
  expect(anfragen.filter((a) => a.endsWith('.png'))).toEqual([]);
});

test('was über die Grenzen geht oder nicht auf die Webkarte gehört, übergeht die Karte und sagt es in der Konsole', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const viele = Array.from({ length: 1001 }, (_, i) => ({ id: `n${i}`, type: 'pin', at: [35.5 + (i % 30), -14.5 - Math.floor(i / 30)] }));
  const geheim = { ...STAEDTE, id: 'beispiel:geheim', name: { de: 'Geheim' }, order: 3 };
  const tafel = { ...HAFEN, id: 'tafel', name: 'Tafel', at: [20.5, -30.5], panel: { blocks: [
    { type: 'rating', rows: [{ label: 'Zu viel', value: 3, max: 21, color: '#E5C33F' }, { label: 'Gut', value: 1, max: 2 }] },
    { type: 'image', image: 'images/burg_16.png', width: 600, height: 16 },
  ] } };
  await welt(page, {
    liste: () => [STAEDTE, geheim],
    datei: (name) => (name === 'geheim' ? { permission: 'stadt.geheim', objects: [HAFEN] } : { objects: [tafel, ...viele] }),
  });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1000);
  await expect.poll(() => meldungen.some((m) => m.includes('1002 Nadeln'))).toBe(true);
  expect(meldungen.some((m) => m.includes('geheim.json') && m.includes('permission'))).toBe(true);
  await page.locator('.nadel-icon[title="Tafel"]').click();
  await expect(page.locator('.tafel-reihe')).toHaveCount(1);
  await expect(page.locator('.tafel img')).toHaveCount(0);
  expect(meldungen.some((m) => m.includes('max 21'))).toBe(true);
  expect(meldungen.some((m) => m.includes('width und height'))).toBe(true);
});
