import { expect, test, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { deflateSync } from 'node:zlib';
import { projiziere, zweiZuEins } from '../src/pick';
import { aufDemSchirm, BILDER, DEMO, fuss, HAFEN, KREISE, LEER, pixelAufPixel, png, STAEDTE, staedte, welt } from './ebenen-welt';

test('ohne layers.json gibt es keine Liste der Ebenen, und die Karte fragt bis zum Neuladen nicht nach', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  let anfragen = 0;
  await page.route('**/tiles-demo/layers.json', (route) => {
    anfragen++;
    return route.fulfill({ status: 404 });
  });
  await page.clock.install();
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect.poll(() => anfragen).toBe(1);
  await page.clock.runFor(61_000);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await page.waitForTimeout(300);
  expect(anfragen).toBe(1);
  await expect(page.locator('.ebenen')).toHaveCount(0);
  expect(meldungen.filter((m) => m.includes('Ebenen ohne Höhen'))).toEqual([]);
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

/** Drei Ebenen nach order 3, 2, 1, je mit einer Nadel ihres Namens, dazu die in `mehr`. */
const reihe = (mehr: { id: string; order: number }[] = []) =>
  [...[{ id: 'a', order: 3 }, { id: 'b', order: 2 }, { id: 'c', order: 1 }], ...mehr].map(({ id, order }) => ({ ...STAEDTE, id: `beispiel:${id}`, name: { de: id.toUpperCase() }, order }));
const nadelIn = (name: string) => ({ objects: [{ ...HAFEN, id: name, name: name.toUpperCase(), at: [20.5 + name.charCodeAt(0) - 97, -30.5] }] });
/** Der z-index des Panes, in dem die Nadel einer Ebene liegt. */
const stapel = (page: Page, name: string) => page.locator(`.nadel-icon[title="${name}"]`).evaluate((e) => Number(getComputedStyle(e.closest('.leaflet-pane')!).zIndex));
const SCHLUESSEL = 'ebenen-reihe:/tiles-demo';

test('nach 0113 stellt jeder die Reihe der Ebenen mit ▲ und ▼ für sich um; Liste und Panes folgen, nach dem Neuladen bleibt sie; „Reihe des Servers“ stellt order wieder her; die Reihe einer anderen Wurzel gilt nicht', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('ebenen-reihe:/tiles-andere', JSON.stringify(['beispiel:c', 'beispiel:b', 'beispiel:a'])));
  await welt(page, { liste: () => reihe(), datei: nadelIn });
  await page.goto(DEMO);
  const liste = page.locator('.ebenen');
  await liste.locator('summary').click();
  await expect(liste.locator('label')).toHaveText([/A/, /B/, /C/]);
  await expect(liste.locator('.ebenen-vorgabe')).toHaveCount(0);
  const pfeil = (name: string, wohin: string) => liste.getByRole('button', { name: `${name} ${wohin}` });
  await expect(pfeil('A', 'nach oben')).toBeDisabled();
  await expect(pfeil('C', 'nach unten')).toBeDisabled();
  await expect(page.locator('.nadel-icon')).toHaveCount(3);
  expect(await stapel(page, 'A')).toBeGreaterThan(await stapel(page, 'B'));
  // A eins nach unten: Liste und Panes folgen, der Fokus bleibt auf dem Pfeil.
  await pfeil('A', 'nach unten').click();
  await expect(liste.locator('label')).toHaveText([/B/, /A/, /C/]);
  await expect(pfeil('A', 'nach unten')).toBeFocused();
  expect(await stapel(page, 'B')).toBeGreaterThan(await stapel(page, 'A'));
  expect(await stapel(page, 'A')).toBeGreaterThan(await stapel(page, 'C'));
  // Gemerkt unter der eigenen Wurzel.
  expect(JSON.parse((await page.evaluate((s) => localStorage.getItem(s), SCHLUESSEL))!)).toEqual(['beispiel:b', 'beispiel:a', 'beispiel:c']);
  await page.reload();
  await page.locator('.ebenen summary').click();
  await expect(liste.locator('label')).toHaveText([/B/, /A/, /C/]);
  // Per Tastatur ganz nach unten: Dort ist ▼ gesperrt, der Fokus geht auf ▲ derselben Ebene.
  await pfeil('A', 'nach unten').focus();
  await page.keyboard.press('Enter');
  await expect(liste.locator('label')).toHaveText([/B/, /C/, /A/]);
  await expect(pfeil('A', 'nach unten')).toBeDisabled();
  await expect(pfeil('A', 'nach oben')).toBeFocused();
  // Die Reihe des Servers: wieder nach order, der Speicher leer, der Knopf weg.
  await liste.locator('.ebenen-vorgabe').click();
  await expect(liste.locator('label')).toHaveText([/A/, /B/, /C/]);
  await expect(liste.locator('.ebenen-vorgabe')).toHaveCount(0);
  expect(await page.evaluate((s) => localStorage.getItem(s), SCHLUESSEL)).toBeNull();
  expect(await stapel(page, 'A')).toBeGreaterThan(await stapel(page, 'B'));
});

test('nach 0113 kommt eine neue Ebene in eine umgestellte Reihe direkt über die oberste mit order nicht höher, sonst ganz unten; eine verschwundene fällt weg; zurückgeschrieben wird nichts', async ({ page }) => {
  const gemerkt = JSON.stringify(['beispiel:c', 'beispiel:a', 'beispiel:b']);
  await page.addInitScript(([s, g]) => localStorage.setItem(s!, g!), [SCHLUESSEL, gemerkt]);
  // B ist weg; N hat order 2, darüber steht C mit 1, obwohl C in der Reihe des Servers unten wäre; L hat die kleinste.
  const liste = reihe([{ id: 'n', order: 2 }, { id: 'l', order: 0 }]).filter((e) => e.id !== 'beispiel:b');
  await welt(page, { liste: () => liste, datei: nadelIn });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen label')).toHaveText([/N/, /C/, /A/, /L/]);
  expect(await page.evaluate((s) => localStorage.getItem(s), SCHLUESSEL)).toBe(gemerkt);
});

test('nach 0113 gibt ein kaputter Eintrag im Speicher die Reihe des Servers', async ({ page }) => {
  await page.addInitScript((s) => localStorage.setItem(s, '{"a":1}'), SCHLUESSEL);
  await welt(page, { liste: () => reihe(), datei: nadelIn });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen label')).toHaveText([/A/, /B/, /C/]);
  await expect(page.locator('.ebenen-vorgabe')).toHaveCount(0);
});

test('nach 0113 geht das Umstellen ohne localStorage bis zum Neuladen', async ({ page }) => {
  await page.addInitScript(() => {
    for (const art of ['getItem', 'setItem', 'removeItem'] as const) {
      Storage.prototype[art] = () => {
        throw new Error('kein Speicher');
      };
    }
  });
  await welt(page, { liste: () => reihe(), datei: nadelIn });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await page.getByRole('button', { name: 'C nach oben' }).click();
  await expect(page.locator('.ebenen label')).toHaveText([/A/, /C/, /B/]);
  await page.reload();
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen label')).toHaveText([/A/, /B/, /C/]);
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

for (const [richtung, drehe] of [
  ['se', (x: number, z: number): [number, number] => [x, z]],
  ['sw', (x: number, z: number): [number, number] => [z, -x]],
] as const) {
  test(`aus ${richtung}: ohne y steht die Nadel auf der Oberfläche: bilinear zwischen den Zellen, eine leere Zelle aus ihren Nachbarn, ohne Höhen auf seaLevel`, async ({ page }) => {
    // Gefälle in x und z, eine leere Zelle unter den vier um die Nadel. Gekrümmt in x,
    // damit der Mittelwert der Nachbarn vom Umkreis abhängt.
    const hoehe = (i: number, j: number) => (i === 9 && j === -4 ? LEER : 10 + 2 * i * i + 3 * j);
    // Westlich von x = 0 fehlen die Regionen. Nach 0128 hat nur ein Ort im Fenster mit Rand einen Marker: „Ohne“ liegt
    // darum nah, mit allen Zellen seiner Rechnung samt Nachbarn in der fehlenden Region.
    const mitLuecke = (i: number, j: number) => (i < 0 ? undefined : hoehe(i, j));
    const zwischen = { ...HAFEN, id: 'zwischen', name: 'Zwischen', at: [35.5, -14.5] };
    const ohne = { ...HAFEN, id: 'ohne', name: 'Ohne', at: [-12.5, -14.5] };
    // Auf ihrer Höhe stehen beide über der Welt, wo keine Kachel liegt. Gemessen wird darum gegen eine Nadel mit y am
    // Ort von Zwischen; dass eine Nadel mit y richtig steht, prüft der Test davor gegen die Kacheln.
    const bezug = { ...HAFEN, id: 'bezug', name: 'Bezug', at: [35.5, -14.5], y: 40 };
    await welt(page, staedte([zwischen, ohne, bezug], { mehr: { seaLevel: 40, direction: richtung }, hoehe: mitLuecke }));
    // Nach 0128 hat nur ein Ort im Fenster mit Rand einen Marker: die Mitte zwischen ihnen, das Fenster hoch genug.
    await page.setViewportSize({ width: 1280, height: 1600 });
    await page.goto(`${DEMO}&at=12,90,-15`);
    await expect(page.locator('.nadel-icon')).toHaveCount(3);
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
    const [rx, rz] = drehe(35.5, -14.5);
    const r = projiziere(rx, 41, rz, zweiZuEins(16));
    const bei = await fuss(page, 'Bezug');
    for (const [name, x, y, z] of [
      ['Zwischen', 35.5, h, -14.5],
      // Keine Höhen dort, kein Nachbar: seaLevel + 1.
      ['Ohne', -12.5, 41, -14.5],
    ] as const) {
      const [bx, bz] = drehe(x, z);
      const p = projiziere(bx, y, bz, zweiZuEins(16));
      const soll = [bei[0]! + p[0] - r[0], bei[1]! + p[1] - r[1]];
      const ist = await fuss(page, name);
      expect(Math.abs(ist[0]! - soll[0]!), `${name} x`).toBeLessThanOrEqual(1);
      expect(Math.abs(ist[1]! - soll[1]!), `${name} y`).toBeLessThanOrEqual(1);
    }
  });
}

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

/** Ein Gesicht 8 × 8 und sein Name, der SHA-256 seiner Bytes, wie 0114. */
const GESICHT = { png: png(8, 8), name: createHash('sha256').update(png(8, 8)).digest('hex') };
/** Liefert unter koepfe/ je Name die Bytes oder einen Status; zählt die Anfragen. */
async function koepfe(page: Page, je: Record<string, Buffer | number>): Promise<string[]> {
  const geholt: string[] = [];
  await page.route('**/tiles-demo/koepfe/**', (route) => {
    const name = new URL(route.request().url()).pathname.split('/').pop()!.replace(/\.png$/, '');
    geholt.push(name);
    const antwort = je[name];
    return Buffer.isBuffer(antwort) ? route.fulfill({ body: antwort, contentType: 'image/png' }) : route.fulfill({ status: antwort ?? 404 });
  });
  return geholt;
}

test('nach 0118 zeigt eine Nadel mit head das Gesicht, die Mitte auf dem Fuss, den Namen darunter, die Tafel darüber; fehlt der Kopf, ist er kaputt oder head kein Name, die Nadel', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const [fehlt, kaputt] = ['0'.repeat(64), '1'.repeat(64)];
  const mit = (name: string, at: [number, number], head: string) => ({ ...HAFEN, id: name, name, at, y: 5, head });
  await welt(page, staedte([mit('Kopf', [30.5, -20.5], GESICHT.name), mit('Fehlt', [40.5, -10.5], fehlt), mit('Kaputt', [25.5, -25.5], kaputt), mit('Gross', [20.5, -30.5], GESICHT.name.toUpperCase())]));
  const geholt = await koepfe(page, { [GESICHT.name]: GESICHT.png, [kaputt]: Buffer.from('kein png') });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(4);
  // Das Gesicht: ein Pixel 2 Pixel des Geräts bei devicePixelRatio 1, ohne Glätten.
  const leinwand = page.locator('.nadel-icon[title="Kopf"] canvas');
  expect(await leinwand.evaluate((c: HTMLCanvasElement) => [c.width, c.height, c.getBoundingClientRect().width, c.getBoundingClientRect().height])).toEqual([16, 16, 16, 16]);
  const ist = await leinwand.evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(0, 0, 16, 16).data]);
  const soll: number[] = [];
  for (let py = 0; py < 16; py++) for (let px = 0; px < 16; px++) soll.push(((px >> 1) * 7) % 256, ((py >> 1) * 3) % 256, 128, 255);
  expect(ist).toEqual(soll);
  // Die Mitte des Gesichts auf dem Fuss, genau; der Fuss auf der Oberseite des Blocks.
  const icon = page.locator('.nadel-icon[title="Kopf"]');
  expect(await icon.evaluate((e) => [(e as HTMLElement).style.marginLeft, (e as HTMLElement).style.marginTop])).toEqual(['-8px', '-8px']);
  const sollFuss = await aufDemSchirm(page, ...projiziere(30.5, 6, -20.5, zweiZuEins(16)));
  const kasten = (await icon.boundingBox())!;
  expect(Math.abs(kasten.x + 8 - sollFuss[0]!)).toBeLessThanOrEqual(1);
  expect(Math.abs(kasten.y + 8 - sollFuss[1]!)).toBeLessThanOrEqual(1);
  // Der Name gerade und mittig unter dem Gesicht.
  const name = (await icon.locator('.nadel-name').boundingBox())!;
  expect(Math.abs(name.y - (kasten.y + kasten.height))).toBeLessThanOrEqual(1);
  expect(Math.abs(name.x + name.width / 2 - (kasten.x + 8))).toBeLessThanOrEqual(1);
  // Die übrigen bleiben Nadeln in ihrer Grösse, mit Namen.
  for (const n of ['Fehlt', 'Kaputt', 'Gross']) {
    expect(await page.locator(`.nadel-icon[title="${n}"] canvas`).evaluate((c: HTMLCanvasElement) => [c.width, c.height]), n).toEqual([23, 33]);
    await expect(page.locator(`.nadel-icon[title="${n}"] .nadel-name`), n).toHaveText(n);
  }
  // Die Tafel beim Zeigen, ihre Spitze 7 px unter der Oberkante des Gesichts.
  await icon.hover();
  const spitze = page.locator('.leaflet-popup-tip-container');
  await expect(spitze).toBeVisible();
  await expect(page.locator('.tafel')).toContainText('Hafenstadt');
  const s = (await spitze.boundingBox())!;
  expect(Math.abs(s.y + s.height - (kasten.y + 7))).toBeLessThanOrEqual(1);
  // Je Name eine Anfrage; den in Grossbuchstaben fragt die Karte nie.
  expect([...geholt].sort()).toEqual([GESICHT.name, fehlt, kaputt].sort());
  expect(meldungen.filter((m) => m.includes('ohne Gesicht')).length).toBe(2);
});

test('nach 0119 bleiben Nadeln, Köpfe und Banner auf jeder Stufe gleich gross; unter 16 Blöcken je Pixel wird nur der Name stufenlos kleiner und blendet weich aus', async ({ page }) => {
  const dorf = { ...HAFEN, id: 'dorf', name: 'Dorf', size: 'small', at: [30.5, -20.5], panel: undefined };
  const fahne = { id: 'fahne', type: 'banner', at: [40.5, -10.5], image: 'images/fahne.png', name: 'Fahne' };
  const kopf = { ...HAFEN, id: 'kopf', name: 'Kopf', head: GESICHT.name, at: [25.5, -25.5], panel: undefined };
  // Wie der Baum des Users: von oben genordet, 4 Pixel je Block. Auf Zoom −6 ist ein Block 1/16 Pixel breit.
  const vonOben = { camera: 'top-north', scale: 4, projection: { azimuth: 'north', u: 4, v: 4, y: 0 }, minZoom: -10 };
  await welt(page, staedte([HAFEN, dorf, fahne, kopf], { mehr: vonOben, bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]) }));
  await koepfe(page, { [GESICHT.name]: GESICHT.png });
  const groessen = ['Dorf 9 × 15', 'Fahne 22 × 40', 'Hafenstadt 23 × 33', 'Kopf 16 × 16'];
  const namen = page.locator('.nadel-name, .nadel-bogen');
  // Je Zoom: der Faktor des Namens, seine Deckkraft und ob er sichtbar ist.
  for (const [zoom, teil, deck] of [
    [0, 1, 1],
    [-6, 1, 1],
    // Ein Block 1/32 Pixel: der Name halb so gross, 8 px, noch ganz deckend.
    [-7, 0.5, 1],
    // 4 px liegt unter 5: weg, aber das Icon bleibt.
    [-8, 0.25, 0],
    [-10, 0.0625, 0],
  ] as const) {
    await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
    await expect(page.locator('.nadel-icon')).toHaveCount(4);
    await expect(page.locator('.nadel-name'), `Zoom ${zoom}`).toHaveCount(3);
    await expect(page.locator('.nadel-bogen'), `Zoom ${zoom}`).toHaveCount(1);
    const gemessen = await page.locator('.nadel-icon').evaluateAll((l) =>
      l.map((e) => `${e.getAttribute('title')} ${e.getBoundingClientRect().width} × ${e.getBoundingClientRect().height}`).sort(),
    );
    expect(gemessen, `Zoom ${zoom}`).toEqual(groessen);
    // Erst nach dem Übergang von CSS messen.
    await page.waitForTimeout(300);
    const stil = await namen.evaluateAll((l) => l.map((e) => [getComputedStyle(e).transform, Number(getComputedStyle(e).opacity), getComputedStyle(e).visibility]));
    for (const [transform, opacity, sichtbar] of stil) {
      expect(transform, `Zoom ${zoom}`).toMatch(new RegExp(`^matrix\\(${teil}, 0, 0, ${teil}, `));
      expect(opacity, `Zoom ${zoom}`).toBe(deck);
      expect(sichtbar, `Zoom ${zoom}`).toBe(deck > 0 ? 'visible' : 'hidden');
    }
    if (zoom === -7) {
      // Die gerade Schrift 8 px hoch, um ihre Oberkante; der Bogen um den Fuss.
      expect(await page.locator('.nadel-name').first().evaluate((e) => e.getBoundingClientRect().height)).toBeCloseTo(8, 0);
      expect(await page.locator('.nadel-bogen').evaluate((e) => getComputedStyle(e).transformOrigin)).toBe('0px 0px');
    }
  }
  // Ein Zoom von −7 auf −8 baut kein Icon neu, und der Name blendet über mehrere Bilder aus, sichtbar bis zuletzt.
  await page.goto(`${DEMO}&at=35,0,-15&zoom=-7`);
  const fahneIcon = page.locator('.nadel-icon[title="Fahne"]');
  await expect(fahneIcon).toHaveCount(1);
  await fahneIcon.evaluate((e) => ((e as HTMLElement & { probe?: number }).probe = 1));
  await page.waitForTimeout(300);
  const verlauf = await page.evaluate(async () => {
    const name = document.querySelector('.nadel-icon[title="Dorf"] .nadel-name')!;
    (document.querySelector('.leaflet-control-zoom-out') as HTMLElement).click();
    const werte: [number, string][] = [];
    const ende = performance.now() + 450;
    while (performance.now() < ende) {
      await new Promise(requestAnimationFrame);
      werte.push([Number(getComputedStyle(name).opacity), getComputedStyle(name).visibility]);
    }
    return werte;
  });
  expect(verlauf.some(([o, v]) => o > 0.05 && o < 0.95 && v === 'visible'), JSON.stringify(verlauf)).toBe(true);
  expect(verlauf.at(-1)).toEqual([0, 'hidden']);
  expect(await fahneIcon.evaluate((e) => (e as HTMLElement & { probe?: number }).probe)).toBe(1);
  // Die Tafel steht über dem Icon wie auf jeder Stufe: ihre Spitze 7 px unter seiner Oberkante.
  const hafen = page.locator('.nadel-icon[title="Hafenstadt"]');
  await hafen.hover();
  const spitze = page.locator('.leaflet-popup-tip-container');
  await expect(spitze).toBeVisible();
  const [oben, unten] = [(await hafen.boundingBox())!.y, (await spitze.boundingBox())!];
  expect(Math.abs(unten.y + unten.height - (oben + 7))).toBeLessThanOrEqual(1);
  await page.goto(`${DEMO}&at=35,0,-15&zoom=0`);
  // Die Variablen stehen am Pane der Nadeln, nicht am Container; ein Zoom, der sie nicht ändert, schreibt nichts.
  await expect(page.locator('.nadel-icon')).toHaveCount(4);
  const lage = () =>
    page.evaluate(() => {
      const pane = document.querySelector('.nadel-icon')!.closest<HTMLElement>('.leaflet-pane')!;
      return [document.querySelector<HTMLElement>('#map')!.style.getPropertyValue('--name-teil'), pane.style.getPropertyValue('--name-teil')];
    });
  expect(await lage()).toEqual(['', '1']);
  await page.evaluate(() => {
    const pane = document.querySelector('.nadel-icon')!.closest('.leaflet-pane')!;
    const w = window as unknown as { geschrieben: number };
    w.geschrieben = 0;
    new MutationObserver((l) => (w.geschrieben += l.length)).observe(pane, { attributes: true, attributeFilter: ['style', 'class'] });
  });
  await page.locator('.leaflet-control-zoom-out').click();
  await page.waitForTimeout(400);
  expect(await page.evaluate(() => (window as unknown as { geschrieben: number }).geschrieben)).toBe(0);
  await page.goto(`${DEMO}&at=35,0,-15&zoom=0`);
  // Die Namen wie die Kartenschrift, 16 Pixel, ohne Kasten, auch ohne Kartenschrift in der Ebene.
  const stil = await page.locator('.nadel-name').first().evaluate((e) => {
    const s = getComputedStyle(e);
    return [s.fontFamily, s.fontSize, s.color, s.backgroundColor, s.webkitTextStrokeWidth, s.webkitTextStrokeColor, s.paintOrder.split(' ')[0]];
  });
  expect(stil).toEqual(['Kartenschrift, serif', '16px', 'rgb(43, 43, 43)', 'rgba(0, 0, 0, 0)', '4px', 'rgb(242, 232, 208)', 'stroke']);
  // check() sagt auch für eine nie geladene Schrift ja; es zählt die geladene FontFace.
  await expect.poll(() => page.evaluate(() => [...document.fonts].some((f) => f.family === 'Kartenschrift' && f.status === 'loaded'))).toBe(true);
});

test('der Name eines Banners steht im Bogen: Radius 2 · Höhe, bis 120° offen, dann flacher, der tiefste Punkt 0,75 · s unter dem Fuss, Sperrung 2 px; der einer Nadel bleibt gerade', async ({ page }) => {
  const fahne = (id: string, name: string, at: [number, number]) => ({ id, type: 'banner', at, image: 'images/fahne.png', name });
  const nadel = { ...HAFEN, id: 'nadel', name: 'Wegpunkt', panel: undefined };
  await welt(page, staedte([fahne('kurz', 'Hafenstadt', [30.5, -20.5]), fahne('lang', 'Neu-Hafenstadt am Westmeer', [40.5, -10.5]), nadel], {
    bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]),
  }));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const bogen = (name: string) =>
    page.locator(`.nadel-icon[title="${name}"] .nadel-bogen`).evaluate((svg: SVGSVGElement) => {
      const pfad = svg.querySelector('path')!;
      const text = svg.querySelector('text')!;
      const mitte = pfad.getPointAtLength(pfad.getTotalLength() / 2);
      const r = Number(/A ([\d.]+)/.exec(pfad.getAttribute('d')!)![1]);
      // Die gerenderte Länge zählt die Sperrung auch nach dem letzten Zeichen.
      const l = text.getComputedTextLength() - 2;
      return { r, mitte: [mitte.x, mitte.y], l, sperrung: text.getAttribute('letter-spacing'), lage: [svg.style.left, svg.style.top] };
    });
  const kurz = await bogen('Hafenstadt');
  // Höhe 40, also r = 80; der tiefste Punkt am Fuss in x und 12 = 0,75 · 16 darunter.
  expect(kurz.r).toBe(80);
  expect(Math.abs(kurz.mitte[0]!)).toBeLessThan(0.01);
  expect(Math.abs(kurz.mitte[1]! - 12)).toBeLessThan(0.01);
  expect(kurz.sperrung).toBe('2');
  // Der Ursprung des SVG am Fuss: ⌊22 / 2⌋ = 11, Höhe 40.
  expect(kurz.lage).toEqual(['11px', '40px']);
  expect(kurz.l / kurz.r).toBeLessThan((2 * Math.PI) / 3);
  // Der lange Name öffnet bis 120°, darüber wächst r.
  const lang = await bogen('Neu-Hafenstadt am Westmeer');
  expect(lang.r).toBeGreaterThan(80);
  expect(Math.abs(lang.l / lang.r - (2 * Math.PI) / 3)).toBeLessThan(0.05);
  // Die Nadel: gerade, kein Bogen.
  await expect(page.locator('.nadel-icon[title="Wegpunkt"] .nadel-name')).toHaveText('Wegpunkt');
  await expect(page.locator('.nadel-icon[title="Wegpunkt"] .nadel-bogen')).toHaveCount(0);
});

/** Ein Satz von Sprites wie aus `--banners`: in 2:1 23 × 51 mit dem Fuss bei (12, 49) und der Unterkante um atan(1/2) gedreht. */
const SATZ = { foot: [12, 49], angle: 26.56505117707799 };
const spriteWelt = (objekte: object[], baum: { path: string; camera: string; look?: string }, satz: string, sprites: Record<string, Buffer>, satzJson: object = SATZ) =>
  staedte(objekte, {
    baum,
    bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]),
    weitere: (pfad) => {
      const rest = pfad.startsWith(`beispiel/banner/staedte/${satz}/`) ? pfad.slice(`beispiel/banner/staedte/${satz}/`.length) : undefined;
      if (rest === 'satz.json') return satzJson;
      return rest === undefined ? undefined : sprites[rest];
    },
  });

test('mit design zeigt die Karte das Sprite ihres Satzes, mit capital das aus krone/; Fuss und Winkel aus satz.json, der Bogen des Namens dreht mit', async ({ page }) => {
  const stadt = { id: 'stadt', type: 'banner', at: [30.5, -20.5], design: 'nordreich', image: 'images/fahne.png', name: 'Hafenstadt' };
  const haupt = { ...stadt, id: 'haupt', at: [40.5, -10.5], capital: true, name: 'Hauptstadt' };
  const anfragen = await welt(page, spriteWelt([stadt, haupt], { path: '2x1-se', camera: '2:1' }, '2x1-se', { 'nordreich.png': png(23, 51), 'krone/nordreich.png': png(23, 51) }));
  await page.goto(`${DEMO}&tree=2x1-se&at=35,0,-15`);
  const icon = (name: string) => page.locator(`.nadel-icon[title="${name}"]`);
  await expect(icon('Hauptstadt').locator('img')).toHaveJSProperty('naturalWidth', 23);
  for (const name of ['Hafenstadt', 'Hauptstadt']) {
    // Die linke obere Ecke auf Ort − foot, das SVG des Namens am Fuss, um den Winkel gedreht.
    expect(await icon(name).evaluate((e) => [(e as HTMLElement).style.marginLeft, (e as HTMLElement).style.marginTop]), name).toEqual(['-12px', '-49px']);
    const bogen = icon(name).locator('.nadel-bogen');
    expect(await bogen.evaluate((svg) => [(svg as SVGSVGElement).style.left, (svg as SVGSVGElement).style.top])).toEqual(['12px', '49px']);
    await expect(bogen.locator('g')).toHaveAttribute('transform', 'rotate(26.56505117707799)');
  }
  const geholt = anfragen.filter((a) => a.includes('/banner/') || a.includes('images/'));
  expect(geholt.map((a) => a.replace(/\?.*$/, '')).sort()).toEqual([
    'beispiel/banner/staedte/2x1-se/krone/nordreich.png',
    'beispiel/banner/staedte/2x1-se/nordreich.png',
    'beispiel/banner/staedte/2x1-se/satz.json',
  ]);
});

for (const designs of [true, false]) {
  test(`mit designs: ${designs} in layers.json geht satz.json ${designs ? 'schon mit der Datei der Ebene los' : 'erst nach ihr los'}`, async ({ page }) => {
    const stadt = { id: 'stadt', type: 'banner', at: [30.5, -20.5], design: 'nordreich', name: 'Hafenstadt' };
    const anfragen = await welt(page, {
      ...spriteWelt([stadt], { path: '2x1-se', camera: '2:1' }, '2x1-se', { 'nordreich.png': png(23, 51) }),
      liste: () => [{ ...STAEDTE, ...(designs ? { designs: true } : {}) }],
    });
    // Die Datei der Ebene hängt, bis der Test sie freigibt; später angemeldet, geht vor.
    let frei = () => {};
    const gehalten = new Promise<void>((r) => (frei = r));
    let angefragt = false;
    await page.route('**/tiles-demo/layers/beispiel/staedte.json', async (route) => {
      angefragt = true;
      await gehalten;
      await route.fallback();
    });
    const satz = () => anfragen.some((a) => a.startsWith('beispiel/banner/staedte/2x1-se/satz.json'));
    await page.goto(`${DEMO}&tree=2x1-se&at=35,0,-15`);
    await expect.poll(() => angefragt).toBe(true);
    if (designs) await expect.poll(satz).toBe(true);
    else {
      await page.waitForTimeout(500);
      expect(satz()).toBe(false);
    }
    frei();
    // Danach wie immer: das Sprite des Satzes, satz.json genau einmal.
    await expect(page.locator('.nadel-icon[title="Hafenstadt"] img')).toHaveJSProperty('naturalWidth', 23);
    expect(anfragen.filter((a) => a.startsWith('beispiel/banner/staedte/2x1-se/satz.json'))).toHaveLength(1);
  });
}

test('ein Baum von oben nimmt den Satz oben; fehlt das Sprite, zeigt die Karte image, ohne image übergeht sie das Banner mit Meldung', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const mitBild = { id: 'mit', type: 'banner', at: [30.5, -20.5], design: 'fehlt', image: 'images/fahne.png', name: 'Mit Bild' };
  const ohneBild = { id: 'ohne', type: 'banner', at: [40.5, -10.5], design: 'fehlt', name: 'Ohne Bild' };
  const da = { id: 'da', type: 'banner', at: [35.5, -14.5], design: 'nordreich', name: 'Da' };
  // Von oben 20 × 46 mit dem Fuss bei (10, 45), ohne Winkel.
  const anfragen = await welt(page, spriteWelt([mitBild, ohneBild, da], { path: 'top-north-s', camera: 'top-north' }, 'oben', { 'nordreich.png': png(20, 46) }, { foot: [10, 45], angle: 0 }));
  await page.goto(`${DEMO}&tree=top-north-s`);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  // Das Bild mit seinem Fuss unten mittig, das Sprite mit dem aus satz.json.
  await expect(page.locator('.nadel-icon[title="Mit Bild"] img')).toHaveJSProperty('naturalWidth', 22);
  await expect(page.locator('.nadel-icon[title="Da"] img')).toHaveJSProperty('naturalWidth', 20);
  expect(anfragen.some((a) => a.startsWith('beispiel/banner/staedte/oben/satz.json'))).toBe(true);
  await expect.poll(() => meldungen.some((m) => m.includes('Banner ohne') && m.includes('übergangen'))).toBe(true);
  expect(meldungen.some((m) => m.includes('Banner mit') && m.includes('nimmt image'))).toBe(true);
});

test('ohne trees.json gibt es keinen Satz: Die Karte zeigt image und fragt unter banner/ nichts an', async ({ page }) => {
  const stadt = { id: 'stadt', type: 'banner', at: [30.5, -20.5], design: 'nordreich', image: 'images/fahne.png', name: 'Hafenstadt' };
  const anfragen = await welt(page, staedte([stadt], { bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]) }));
  await page.goto(`${DEMO}&at=35,0,-15`);
  await expect(page.locator('.nadel-icon[title="Hafenstadt"] img')).toHaveJSProperty('naturalWidth', 22);
  expect(anfragen.filter((a) => a.includes('/banner/'))).toEqual([]);
  await expect(page.locator('.nadel-icon[title="Hafenstadt"] .nadel-bogen g')).toHaveAttribute('transform', 'rotate(0)');
});

test('ein Banner bis 32 × 64 steht Pixel auf Pixel, der Fuss bei ⌊Breite / 2⌋ auf seinem Block; breiter, höher, ausserhalb von images/ oder in anderem Format übergeht die Karte mit Meldung und holt nur die erlaubten Bilder', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const groessen: Record<string, [number, number]> = { 'gross.png': [32, 64], 'schmal.png': [21, 40], 'breit.png': [33, 64], 'hoch.png': [32, 65] };
  const fahne = (name: string, image: string, at: [number, number]) => ({ id: name.toLowerCase(), type: 'banner', at, y: 5, image, name });
  const anfragen = await welt(page, staedte([
    fahne('Gross', 'images/gross.png', [20.5, -30.5]),
    fahne('Schmal', 'images/schmal.png', [30.5, -20.5]),
    fahne('Breit', 'images/breit.png', [35.5, -14.5]),
    fahne('Hoch', 'images/hoch.png', [40.5, -10.5]),
    fahne('Gif', 'images/gross.gif', [25.5, -25.5]),
    fahne('Draussen', '../gross.png', [25.5, -20.5]),
  ], {
    bild: (n) => {
      const g = groessen[n];
      return g && png(...g);
    },
  }));
  // Beide ganz im Fenster, für den Screenshot.
  await page.goto(`${DEMO}&at=25,5,-25`);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  for (const [name, [b, h], [x, z]] of [
    ['Gross', [32, 64], [20.5, -30.5]],
    ['Schmal', [21, 40], [30.5, -20.5]],
  ] as const) {
    const bild = page.locator(`.nadel-icon[title="${name}"] img`);
    expect(await bild.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight, i.getBoundingClientRect().width, i.getBoundingClientRect().height]), name).toEqual([b, h, b, h]);
    // Auf dem Schirm, ohne den Namen im Bogen am Fuss: die oberen h − 8 Zeilen.
    const { fehler, geprueft } = await pixelAufPixel(page, bild, 1, 8);
    expect(fehler, name).toEqual([]);
    expect(geprueft, name).toBeGreaterThanOrEqual((b - 2) * (h - 10));
    // Der Anker genau, ohne die Toleranz von 1 px unten: ⌊Breite / 2⌋, die Unterkante.
    expect(await page.locator(`.nadel-icon[title="${name}"]`).evaluate((e) => [(e as HTMLElement).style.marginLeft, (e as HTMLElement).style.marginTop]), name).toEqual([`-${Math.floor(b / 2)}px`, `-${h}px`]);
    // fuss misst bei ⌊Breite / 2⌋ von links; y + 1 ist die Oberseite des Blocks.
    const sollFuss = await aufDemSchirm(page, ...projiziere(x, 6, z, zweiZuEins(16)));
    const istFuss = await fuss(page, name);
    expect(Math.abs(istFuss[0]! - sollFuss[0]!), `${name} x`).toBeLessThanOrEqual(1);
    expect(Math.abs(istFuss[1]! - sollFuss[1]!), `${name} y`).toBeLessThanOrEqual(1);
  }
  await expect.poll(() => ['Banner breit: 33 × 64', 'Banner hoch: 32 × 65'].every((t) => meldungen.some((m) => m.includes(t) && m.includes('höchstens 32 × 64')))).toBe(true);
  for (const bild of ['images/gross.gif', '../gross.png']) expect(meldungen.some((m) => m.includes(`„${bild}“`)), bild).toBe(true);
  expect(anfragen.filter((a) => a !== 'beispiel/staedte.json').sort()).toEqual(['beispiel/images/breit.png', 'beispiel/images/gross.png', 'beispiel/images/hoch.png', 'beispiel/images/schmal.png']);
});

test('eine Nadel mit Namen lädt die Kartenschrift, auch ohne Banner und Kartenschrift in der Ebene; nach 0130 als WOFF2', async ({ page }) => {
  const schriften: { url: string; typ: string | undefined }[] = [];
  page.on('response', (r) => {
    if (/\.(woff2|ttf|otf|woff)$/.test(new URL(r.url()).pathname)) schriften.push({ url: new URL(r.url()).pathname, typ: r.headers()['content-type'] });
  });
  await welt(page, staedte([HAFEN]));
  await page.goto(DEMO);
  await expect(page.locator('.nadel-name')).toHaveText('Hafenstadt');
  await expect.poll(() => page.evaluate(() => [...document.fonts].some((f) => f.family === 'Kartenschrift' && f.status === 'loaded'))).toBe(true);
  // Genau eine Schrift, die WOFF2 aus dem Build, mit ihrem Typ.
  expect(schriften).toHaveLength(1);
  expect(schriften[0]!.url).toMatch(/\/IMFeENsc28P-[^/]+\.woff2$/);
  expect(schriften[0]!.typ).toBe('font/woff2');
});

test('Banner teilen sich ein Bild: die Karte holt es einmal', async ({ page }) => {
  const fahne = (id: string, at: [number, number]) => ({ id, type: 'banner', at, image: 'images/nation.png', name: id });
  const anfragen = await welt(page, staedte([fahne('A', [20.5, -30.5]), fahne('B', [30.5, -20.5]), fahne('C', [40.5, -10.5])], { bild: (n) => (n === 'nation.png' ? png(22, 40) : undefined) }));
  await page.goto(DEMO);
  // Drei Bilder mit derselben Adresse, keine Leinwand: Der Browser hält es einmal.
  await expect(page.locator('.nadel-icon img')).toHaveCount(3);
  await expect(page.locator('.nadel-icon canvas')).toHaveCount(0);
  expect(anfragen.filter((a) => a.includes('images/'))).toEqual(['beispiel/images/nation.png']);
});

test('Ebenen liegen nach order übereinander, in einer Ebene die spätere oben, gleich wo auf dem Schirm', async ({ page }) => {
  // Je zwei Nadeln fast am selben Ort; die untere liegt einige Pixel tiefer auf dem Schirm.
  const oben = { ...STAEDTE, id: 'beispiel:oben', name: { de: 'Oben' }, order: 2 };
  const unten = { ...STAEDTE, id: 'beispiel:unten', name: { de: 'Unten' }, order: 1 };
  let liste = [unten, oben];
  await welt(page, {
    liste: () => liste,
    datei: (name) =>
      ({
        oben: { objects: [{ ...HAFEN, id: 'o', name: 'O', at: [35.5, -14.5] }] },
        unten: { objects: [{ ...HAFEN, id: 'u', name: 'U', at: [35.5, -13.5] }] },
        staedte: { objects: [{ ...HAFEN, id: 'a', name: 'A', at: [20.5, -30] }, { ...HAFEN, id: 'b', name: 'B', at: [20.5, -30.5] }] },
      })[name],
  });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  const oberste = async (name: string) => {
    const [x, y] = await fuss(page, name);
    return page.evaluate(([x, y]) => document.elementFromPoint(x!, y! - 12)?.closest('.nadel-icon')?.getAttribute('title'), [x, y]);
  };
  // Die Ebene mit der höheren order liegt oben, obwohl die andere tiefer steht. Gewartet, bis Lage und Reihe stehen.
  await expect.poll(() => oberste('O')).toBe('O');
  liste = [STAEDTE];
  await page.reload();
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  // In der Ebene liegt B, die spätere, oben, obwohl A tiefer steht.
  await expect.poll(() => oberste('B')).toBe('B');
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

test('dauert das Laden einer Ebene länger als bis zum nächsten Nachfragen, startet es nicht neu, und die Ebene erscheint', async ({ page }) => {
  await welt(page, staedte([HAFEN]));
  // Die erste Anfrage hängt, bis der Test sie freigibt; jede weitere hängt für immer.
  let anfragen = 0;
  let frei: (() => void) | undefined;
  await page.route('**/tiles-demo/layers/beispiel/staedte.json', async (route) => {
    anfragen++;
    const erste = anfragen === 1;
    await new Promise<void>((weiter) => {
      if (erste) frei = weiter;
    });
    await route.fulfill({ json: { ...STAEDTE, objects: [HAFEN] } });
  });
  await page.goto(DEMO);
  await expect.poll(() => anfragen).toBe(1);
  await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
  await page.waitForTimeout(500);
  frei!();
  await expect(page.locator('.nadel-icon[title="Hafenstadt"]')).toHaveCount(1);
  expect(anfragen).toBe(1);
});

test('an, aus, an: es steht genau eine Gruppe, die des letzten Einschaltens, auch wenn das erste Laden zuletzt fertig wird', async ({ page }) => {
  let freigeben = () => {};
  const frei = new Promise<void>((los) => (freigeben = los));
  let anfrage = 0;
  await welt(page, { liste: () => [KREISE], datei: () => undefined });
  await page.route('**/tiles-demo/layers/beispiel/stadtinfos.json', async (route) => {
    // Die erste Anfrage wartet, bis die zweite steht.
    const name = ++anfrage === 1 ? 'Alt' : 'Neu';
    if (name === 'Alt') await frei;
    await route.fulfill({ json: { objects: [{ ...HAFEN, name }] } });
  });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  const box = page.locator('.ebenen input[data-id="beispiel:stadtinfos"]');
  await box.check();
  await box.uncheck();
  await box.check();
  await expect(page.locator('.nadel-icon[title="Neu"]')).toHaveCount(1);
  freigeben();
  await page.waitForTimeout(800);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  await expect(page.locator('.nadel-icon[title="Neu"]')).toHaveCount(1);
});

test('fällt eine Ebene während ihres Ladens aus layers.json, zeigt die Karte ihre Nadeln nicht', async ({ page }) => {
  await page.clock.install();
  let liste: object[] = [STAEDTE];
  let freigeben = () => {};
  const frei = new Promise<void>((los) => (freigeben = los));
  await welt(page, { liste: () => liste, datei: () => undefined });
  await page.route('**/tiles-demo/layers/beispiel/staedte.json', async (route) => {
    await frei;
    await route.fulfill({ json: { objects: [HAFEN] } });
  });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen label')).toHaveCount(1);
  liste = [];
  await page.clock.runFor(31_000);
  await expect(page.locator('.ebenen label')).toHaveCount(0);
  freigeben();
  await page.waitForTimeout(800);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
});

test('sagt Content-Length mehr als die Grenze, liest die Karte die Datei nicht und sagt es', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  await page.route('**/tiles-demo/layers.json', (route) =>
    route.fulfill({ status: 200, headers: { 'content-type': 'application/json', 'content-length': String(64 * 1024 + 1) }, body: JSON.stringify({ layers: [STAEDTE] }) }),
  );
  await page.goto(DEMO);
  await expect.poll(() => meldungen.some((m) => m.includes('grösser als 65536 Byte'))).toBe(true);
  await expect(page.locator('.ebenen')).toHaveCount(0);
});

test('ändert der Betreiber visible und der Betrachter hat nie gewählt, folgen die Nadeln', async ({ page }) => {
  await page.clock.install();
  let sichtbar = false;
  await welt(page, { liste: () => [{ ...KREISE, visible: sichtbar }], datei: () => ({ objects: [HAFEN] }) });
  await page.goto(DEMO);
  await expect(page.locator('.ebenen')).toHaveCount(1);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  sichtbar = true;
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  sichtbar = false;
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
});

test('bekommt eine gezeigte Ebene permission, weicht sie, und die Karte holt diese version nicht noch einmal', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  await page.clock.install();
  let version = 'a';
  const anfragen = await welt(page, {
    liste: () => [{ ...STAEDTE, version }],
    datei: () => (version === 'a' ? { objects: [HAFEN] } : { permission: 'stadt.geheim', objects: [HAFEN] }),
  });
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(1);
  version = 'b';
  await page.clock.runFor(31_000);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  await page.clock.runFor(62_000);
  await page.waitForTimeout(300);
  expect(anfragen.filter((a) => a === 'beispiel/staedte.json')).toHaveLength(2);
  expect(meldungen.filter((m) => m.includes('permission'))).toHaveLength(1);
});

test('ein Klick auf eine Nadel öffnet die Tafel, ohne den Fokus hineinzuziehen', async ({ page }) => {
  await welt(page, staedte([HAFEN]));
  await page.goto(DEMO);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('✪ Hafenstadt');
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => document.activeElement?.closest('.tafel') !== null)).toBe(false);
});

test('ein Titel unter 3:1 auf dem dunklen Grund wird im selben Farbton aufgehellt, in Schritten von 10 % mit Weiss; ein heller bleibt', async ({ page }) => {
  const titel = (id: string, at: [number, number], color: string) => ({ ...HAFEN, id, name: id, at, panel: { blocks: [{ type: 'title', text: id, color }] } });
  await welt(page, staedte([titel('Blau', [30.5, -20.5], '#2B3A55'), titel('Gruen', [35.5, -14.5], '#40E53F'), titel('Schwarz', [40.5, -10.5], '#000000CC')]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const farbe = async (id: string) => {
    await page.locator(`.nadel-icon[title="${id}"]`).click();
    const f = await page.locator('.tafel .tafel-titel', { hasText: id }).evaluate((e) => getComputedStyle(e).color);
    await page.keyboard.press('Escape');
    return f;
  };
  // Nachgerechnet in docs/benutzung/ebenen.md, „Aussehen“: #2B3A55 nach zwei Schritten #556177 (3,04:1), #000000 nach vier #666666, Alpha bleibt.
  expect(await farbe('Blau')).toBe('rgb(85, 97, 119)');
  expect(await farbe('Gruen')).toBe('rgb(64, 229, 63)');
  expect(await farbe('Schwarz')).toBe('rgba(102, 102, 102, 0.8)');
});

/** Wie viele Tafeln offen sind; eine schliessende blendet mit Deckkraft 0 aus. */
const offeneTafeln = (page: Page) => page.locator('.tafel').evaluateAll((l) => l.filter((e) => (e as HTMLElement).style.opacity !== '0').length);

test('beim Zeigen erscheint die Tafel nach 50 ms Ruhe, über der Nadel, und schliesst 300 ms nach dem Verlassen; dazwischen kann der Zeiger in die Tafel; kurz darüber öffnet nichts; ein Klick hält sie', async ({ page }) => {
  await page.clock.install();
  await welt(page, staedte([HAFEN]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  await expect(nadel).toHaveCount(1);
  const weg = () => page.mouse.move(400, 650);
  // Ohne Pause liefe die Uhr des Tests mit der echten Zeit.
  await page.clock.pauseAt(Date.now() + 1000);
  await nadel.hover();
  await page.clock.runFor(49);
  expect(await offeneTafeln(page)).toBe(0);
  await page.clock.runFor(1);
  expect(await offeneTafeln(page)).toBe(1);
  // Über dem Icon, nicht darauf: Die Spitze der Tafel endet an seiner Oberkante.
  const [icon, tafel] = await Promise.all([nadel.boundingBox(), page.locator('.tafel').boundingBox()]);
  expect(tafel!.y + tafel!.height).toBeLessThanOrEqual(icon!.y + 1);
  await page.locator('.tafel .tafel-titel').hover();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(1);
  await weg();
  await page.clock.runFor(299);
  expect(await offeneTafeln(page)).toBe(1);
  await page.clock.runFor(1);
  expect(await offeneTafeln(page)).toBe(0);
  await page.clock.runFor(1000);
  // Kurz darüber und weiter.
  await nadel.hover();
  await page.clock.runFor(40);
  await weg();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(0);
  // Zurück binnen 300 ms: Sie bleibt offen.
  await nadel.hover();
  await page.clock.runFor(50);
  expect(await offeneTafeln(page)).toBe(1);
  await weg();
  await page.clock.runFor(200);
  await nadel.hover();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(1);
  await weg();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(0);
  // Gehalten bleibt sie, auch ohne Zeiger.
  await nadel.click();
  expect(await offeneTafeln(page)).toBe(1);
  await weg();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(1);
});

test('eine gehaltene Tafel übersteht das Zeigen auf ein anderes Ziel, per Klick wie per Tastatur; ein Klick auf das andere Ziel wechselt', async ({ page }) => {
  await page.clock.install();
  const zweite = { ...HAFEN, id: 'zweite', name: 'Zweite', at: [44.5, -20.5], panel: { blocks: [{ type: 'title', text: 'Zweite Tafel' }] } };
  await welt(page, staedte([HAFEN, zweite]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const [a, b] = [page.locator('.nadel-icon[title="Hafenstadt"]'), page.locator('.nadel-icon[title="Zweite"]')];
  await expect(b).toHaveCount(1);
  await page.clock.pauseAt(Date.now() + 1000);
  const titel = () => page.locator('.tafel').evaluateAll((l) => l.filter((e) => (e as HTMLElement).style.opacity !== '0').map((e) => e.querySelector('.tafel-titel')?.textContent));
  await a.click();
  await b.hover();
  await page.clock.runFor(1000);
  expect(await titel()).toEqual(['✪ Hafenstadt']);
  // Per Tastatur: Der Fokus bleibt in der Tafel.
  await page.keyboard.press('Escape');
  await page.clock.runFor(1000);
  await a.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.tafel .leaflet-popup-content')).toBeFocused();
  // Erst weg, damit das Zeigen auf B wirklich ankommt.
  await page.mouse.move(0, 0);
  await b.hover();
  await page.clock.runFor(1000);
  expect(await titel()).toEqual(['✪ Hafenstadt']);
  await expect(page.locator('.tafel .leaflet-popup-content')).toBeFocused();
  // Ein Klick wechselt.
  await b.click();
  await page.clock.runFor(1000);
  expect(await titel()).toEqual(['Zweite Tafel']);
});

test('wird die Ebene ausgeschaltet, schliesst ihre offene Tafel', async ({ page }) => {
  await welt(page, staedte([HAFEN]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  await expect(page.locator('.tafel')).toHaveCount(1);
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input[data-id="beispiel:staedte"]').uncheck();
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  await expect(page.locator('.tafel')).toHaveCount(0);
});

test('nur eine gehaltene Tafel verschiebt die Karte; beim Zeigen bleiben Karte und festgehaltener Block', async ({ page }) => {
  // Niedrig, so passt die Tafel über der Nadel nicht ins Fenster.
  await page.setViewportSize({ width: 800, height: 300 });
  await welt(page, staedte([HAFEN]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  await expect(nadel).toHaveCount(1);
  const anzeige = page.locator('.koordinaten');
  const box = (await nadel.boundingBox())!;
  await page.mouse.click(box.x - 120, box.y + box.height + 20);
  await expect(anzeige).toHaveClass(/gehalten/);
  await nadel.hover();
  await expect(page.locator('.tafel')).toHaveCount(1);
  await page.waitForTimeout(500);
  expect((await nadel.boundingBox())!.y).toBe(box.y);
  await expect(anzeige).toHaveClass(/gehalten/);
  await nadel.click();
  await expect.poll(async () => (await nadel.boundingBox())!.y).toBeGreaterThan(box.y);
});

test('ein Banner mit Tafel: Die Tafel steht um seine Höhe versetzt über ihm; Enter öffnet sie mit dem Fokus darin, auch wenn sie beim Zeigen schon offen ist', async ({ page }) => {
  const fahne = { id: 'fahne', type: 'banner', at: [35.5, -14.5], image: 'images/fahne.png', name: 'Fahne', panel: { blocks: [{ type: 'title', text: 'Bannertafel' }] } };
  await welt(page, staedte([fahne], { bild: (n) => (n === 'fahne.png' ? png(22, 40) : undefined) }));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const banner = page.locator('.nadel-icon[title="Fahne"]');
  await expect(banner).toHaveAttribute('tabindex', '0');
  await banner.hover();
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Bannertafel');
  const [icon, tafel] = await Promise.all([banner.boundingBox(), page.locator('.tafel').boundingBox()]);
  expect(icon!.height).toBe(40);
  expect(tafel!.y + tafel!.height).toBeLessThanOrEqual(icon!.y + 1);
  // Beim Zeigen offen, der Fokus ausserhalb; Enter zieht ihn hinein.
  await banner.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.tafel .leaflet-popup-content')).toBeFocused();
});

for (const [dpr, k] of [
  [1, 1],
  [1.25, 1],
  [1.5, 2],
  [2, 2],
  [3, 3],
] as const) {
  test.describe(`bei devicePixelRatio ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr });

    test(`nach 0129 ist ein Pixel des Banners ${k} Pixel des Geräts breit, jedes gleich, auf dem Schirm nicht verschmiert`, async ({ page }) => {
      // Ohne Namen: Der Bogen am Fuss läge über den unteren Zeilen.
      const fahne = { id: 'fahne', type: 'banner', at: [35.5, -14.5], image: 'images/fahne.png' };
      await welt(page, staedte([fahne], { bild: (n) => (n === 'fahne.png' ? png(21, 40) : undefined) }));
      await page.goto(`${DEMO}&at=35,0,-15`);
      const bild = page.locator('.nadel-icon img');
      await expect(bild).toHaveCount(1);
      await expect(bild).toHaveJSProperty('complete', true);
      const [w, h, cssW, cssH] = await bild.evaluate((i: HTMLImageElement) => [i.naturalWidth, i.naturalHeight, i.getBoundingClientRect().width, i.getBoundingClientRect().height]);
      expect([w, h]).toEqual([21, 40]);
      // k Pixel des Geräts je Pixel des Bilds; das Layout rechnet in 1/64 Pixel.
      expect(Math.abs(cssW * dpr - 21 * k)).toBeLessThan(1 / 32);
      expect(Math.abs(cssH * dpr - 40 * k)).toBeLessThan(1 / 32);
      expect(await page.locator('.nadel-icon').evaluate((e) => Number.parseFloat((e as HTMLElement).style.marginLeft))).toBeCloseTo((-10 * k) / dpr, 3);
      // Jedes Pixel des Geräts ganz im Bild trägt genau eine Farbe der Quelle, die nächste: kein Glätten, auch bei 1,25 und 1,5.
      const { fehler, geprueft } = await pixelAufPixel(page, bild, k);
      expect(fehler).toEqual([]);
      // Am Rand kann der Browser je eine Spalte und Zeile anschneiden; das Innere zählt.
      expect(geprueft).toBeGreaterThanOrEqual((21 - 3) * (40 - 3));
    });
  });
}

test('ändert sich devicePixelRatio ohne resize, zeichnet die Karte die Icons neu', async ({ page }) => {
  // CDP ändert nur deviceScaleFactor und meldet der Anfrage nach der Auflösung
  // kein change; der Test schickt es wie der Browser beim Wechsel des Monitors.
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
  const fahne = { id: 'fahne', type: 'banner', at: [35.5, -14.5], image: 'images/fahne.png', name: 'Fahne' };
  await welt(page, staedte([fahne], { bild: (n) => (n === 'fahne.png' ? png(22, 40) : undefined) }));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const bild = page.locator('.nadel-icon[title="Fahne"] img');
  const breite = () => bild.evaluate((i) => i.getBoundingClientRect().width);
  await expect.poll(breite).toBe(22);
  // Bei 1,5 ist k = 2: 44 Pixel des Geräts, also 44 / 1,5 Pixel des Schirms.
  const cdp = await page.context().newCDPSession(page);
  const { width, height } = page.viewportSize()!;
  await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1.5, mobile: false });
  await page.evaluate(() => (window as unknown as { anfragen: MediaQueryList[] }).anfragen.at(-1)!.dispatchEvent(new Event('change')));
  await expect.poll(async () => Math.abs((await breite()) - 44 / 1.5)).toBeLessThan(1 / 32);
});

test('Escape und ein Klick daneben schliessen zuerst nur die Tafel: Der festgehaltene Block bleibt, ein neuer wird nicht festgehalten', async ({ page }) => {
  await welt(page, staedte([HAFEN]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  const anzeige = page.locator('.koordinaten');
  await expect(nadel).toHaveCount(1);
  // Gelände links unter der Nadel, nicht unter der Tafel, die über ihr erscheint.
  const box = (await nadel.boundingBox())!;
  const daneben = () => page.mouse.click(box.x - 120, box.y + box.height + 40);
  // Einen Block festhalten, dann die Tafel öffnen.
  await daneben();
  await expect(anzeige).toHaveClass(/gehalten/);
  const block = await anzeige.textContent();
  await nadel.click();
  await expect(page.locator('.tafel')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(page.locator('.tafel')).toHaveCount(0);
  await expect(anzeige).toHaveClass(/gehalten/);
  // Der zweite Druck wirkt auf die Karte.
  await page.keyboard.press('Escape');
  await expect(anzeige).not.toHaveClass(/gehalten/);
  // Ein Klick daneben: schliesst nur die Tafel, hält keinen Block.
  await nadel.click();
  await expect(page.locator('.tafel')).toHaveCount(1);
  await daneben();
  await expect(page.locator('.tafel')).toHaveCount(0);
  await expect(anzeige).not.toHaveClass(/gehalten/);
  await daneben();
  await expect(anzeige).toHaveClass(/gehalten/);
  expect(await anzeige.textContent()).toBe(block);
});

/**
 * Breite, Höhe und `image-rendering` der Bilder in der Tafel, sobald alle
 * geladen sind. `complete` kommt vor dem Ereignis `load`, das Grösse und
 * Klasse setzt: Tests fragen darum mit `expect.poll`.
 */
const tafelBilder = async (page: Page) => {
  const bilder = page.locator('.tafel img');
  await expect.poll(() => bilder.evaluateAll((l) => l.length > 0 && l.every((i) => (i as HTMLImageElement).complete))).toBe(true);
  return bilder.evaluateAll((l) => l.map((i) => [i.getBoundingClientRect().width, i.getBoundingClientRect().height, getComputedStyle(i).imageRendering]));
};

/** Ein Baustein `image`. */
const tafelBild = (image: string, width: number, height: number) => ({ type: 'image', image, width, height });

test('ein Bild der Tafel breiter als 320 Pixel wird geglättet verkleinert, im Verhältnis von width und height; vergrössert um ganze k = round(Faktor) ohne Glättung', async ({ page }) => {
  // burg_16 ist 16 × 16, burg_9 ist 9 × 9.
  const tafel = { ...HAFEN, panel: { blocks: [
    tafelBild('images/flach.png', 512, 64),
    tafelBild('images/quadrat.png', 512, 512),
    // Die Datei quadratisch, die Angabe flach: Es gilt die Angabe.
    tafelBild('images/quadrat-gross.png', 512, 64),
    tafelBild('images/burg_9.png', 9, 9),
    tafelBild('images/burg_16.png', 32, 32),
    tafelBild('images/burg_16.png', 24, 24),
    tafelBild('images/burg_16.png', 20, 20),
  ] } };
  const dateien: Record<string, Buffer> = { 'flach.png': png(512, 64), 'quadrat.png': png(512, 512), 'quadrat-gross.png': png(640, 640) };
  await welt(page, staedte([tafel], { bild: (n) => dateien[n] ?? BILDER[n] }));
  await page.goto(`${DEMO}&at=35,0,-15`);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  await expect.poll(() => tafelBilder(page)).toEqual([
    [320, 40, 'auto'],
    [320, 320, 'auto'],
    [320, 40, 'auto'],
    [9, 9, 'pixelated'],
    [32, 32, 'pixelated'],
    // 1,5 rundet auf 2, 1,25 auf 1.
    [32, 32, 'pixelated'],
    [16, 16, 'pixelated'],
  ]);
});

test.describe('bei devicePixelRatio 1.5', () => {
  test.use({ deviceScaleFactor: 1.5 });

  test('ein Bild der Tafel ist k = round(Faktor) Pixel des Geräts je Pixel der Datei, höchstens 320 Pixel breit', async ({ page }) => {
    const tafel = { ...HAFEN, panel: { blocks: [tafelBild('images/burg_16.png', 16, 16), tafelBild('images/burg_16.png', 320, 320)] } };
    await welt(page, staedte([tafel]));
    await page.goto(`${DEMO}&at=35,0,-15`);
    await page.locator('.nadel-icon[title="Hafenstadt"]').click();
    await expect.poll(async () => (await tafelBilder(page)).map((b) => b[2])).toEqual(['pixelated', 'pixelated']);
    const [klein, gross] = await tafelBilder(page);
    // 16 · 1,5 / 16 = 1,5 → k = 2: 32 Pixel des Geräts; 320 · 1,5 / 16 = 30 → 480 Pixel des Geräts.
    expect(Math.abs((klein![0] as number) * 1.5 - 32)).toBeLessThan(1 / 32);
    expect(klein![2]).toBe('pixelated');
    expect(Math.abs((gross![0] as number) - 320)).toBeLessThan(1 / 32);
    expect(gross![2]).toBe('pixelated');
  });
});

test.describe('auf dem Touchscreen', () => {
  test.use({ hasTouch: true });

  test('Tippen öffnet die Tafel einer Nadel und hält sie; Tippen daneben schliesst sie', async ({ page }) => {
    await welt(page, staedte([HAFEN]));
    await page.goto(`${DEMO}&at=35,0,-15`);
    await page.locator('.nadel-icon[title="Hafenstadt"]').tap();
    await expect(page.locator('.tafel .tafel-titel')).toHaveText('✪ Hafenstadt');
    await page.waitForTimeout(600);
    expect(await offeneTafeln(page)).toBe(1);
    await page.touchscreen.tap(400, 650);
    await expect(page.locator('.tafel')).toHaveCount(0);
  });
});

test('die Tafel zeigt Bausteine als Text und Bilder vom eigenen Server, nie Markup, dunkel wie im Mod; zu hoch, scrollt sie; keine Verletzung der Content-Security-Policy', async ({ page }) => {
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
  // Dunkel wie im Mod: Grund, Schrift, Rahmen aussen und innen.
  const stil = await page.locator('.tafel .leaflet-popup-content-wrapper').evaluate((e) => {
    const s = getComputedStyle(e);
    return [s.backgroundColor, s.color, s.borderTopColor, s.borderTopWidth, s.boxShadow];
  });
  expect(stil).toEqual(['rgba(16, 16, 20, 0.88)', 'rgb(217, 217, 217)', 'rgb(0, 0, 0)', '1px', 'rgb(58, 58, 68) 0px 0px 0px 1px inset']);
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
  // Auch mit dem Fokus auf dem Schliessknopf.
  await page.keyboard.press('Enter');
  await expect(tafel).toBeFocused();
  await page.locator('.tafel .leaflet-popup-close-button').focus();
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

test('Kennungen und Bilder gegen die Regel aus „Kennung“ übergeht die Karte mit Meldung und holt die Bilder nicht; unbekannte Objekte übergeht sie', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const schlecht = ['images/Burg_16.png', 'images/burg_9.gif', `images/${'b'.repeat(65)}.png`, 'images/.burg_9.png', 'images/burg..png', 'images/nul.png', 'images/com1.x.png', '../geheim.png', 'images/../x.png'];
  const tafel = { blocks: [{ type: 'image', image: 'images/lpt9.png', width: 16, height: 16 }, { type: 'title', text: 'Tafel' }] };
  const anfragen = await welt(page, {
    liste: () => [STAEDTE, ...['beispiel:con', 'beispiel:ende.', 'nul.x:ebene', 'beispiel:.vorn'].map((id) => ({ ...STAEDTE, id }))],
    datei: (name) =>
      name === 'staedte'
        ? { objects: [
            ...schlecht.map((s, i) => ({ ...HAFEN, id: `s${i}`, name: `S${i}`, at: [20.5 + 4 * i, -30.5], symbol: { large: s }, panel: tafel })),
            { id: 'neu', type: 'hologram', at: [1, 1] },
            { ...HAFEN, id: 'ohne-at', at: 'hier' },
          ] }
        : { objects: [HAFEN] },
  });
  await page.goto(DEMO);
  await page.locator('.ebenen summary').click();
  await expect(page.locator('.ebenen label')).toHaveCount(1);
  await expect(page.locator('.nadel-icon')).toHaveCount(schlecht.length);
  await page.locator('.nadel-icon[title="S0"]').click();
  await expect(page.locator('.tafel-titel')).toHaveText('Tafel');
  await expect(page.locator('.tafel img')).toHaveCount(0);
  await page.waitForTimeout(300);
  expect(anfragen.filter((a) => a.includes('images/') || !a.startsWith('beispiel/staedte'))).toEqual([]);
  for (const s of [...schlecht, 'images/lpt9.png', 'beispiel:con', 'beispiel:ende.', 'nul.x:ebene', 'beispiel:.vorn']) {
    expect(meldungen.some((m) => m.includes(`„${s}“`)), s).toBe(true);
  }
});

test('was über die Grenzen geht oder nicht auf die Webkarte gehört, übergeht die Karte und sagt es in der Konsole', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  // Nadeln und Banner zählen zusammen.
  const viele = Array.from({ length: 1001 }, (_, i) => ({ id: `n${i}`, type: i % 2 ? 'banner' : 'pin', image: 'images/burg_16.png', at: [35.5 + (i % 30), -14.5 - Math.floor(i / 30)] }));
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
  await expect.poll(() => meldungen.some((m) => m.includes('1002 Nadeln und Banner'))).toBe(true);
  expect(meldungen.some((m) => m.includes('geheim.json') && m.includes('permission'))).toBe(true);
  await page.locator('.nadel-icon[title="Tafel"]').click();
  await expect(page.locator('.tafel-reihe')).toHaveCount(1);
  await expect(page.locator('.tafel img')).toHaveCount(0);
  expect(meldungen.some((m) => m.includes('max 21'))).toBe(true);
  expect(meldungen.some((m) => m.includes('width und height'))).toBe(true);
});

/** Von oben genordet, 4 Pixel je Block; Zoom 0 zeigt 320 × 180 Blöcke, der Rahmen nach 0128 reicht 240 Blöcke von der Mitte. */
const WEIT = { camera: 'top-north', scale: 4, projection: { azimuth: 'north', u: 4, v: 4, y: 0 }, minZoom: -10, bounds: [-4096, -4096, 4096, 4096] };
const NAH = { ...HAFEN, id: 'nah', name: 'Nah', at: [0.5, 0.5], panel: undefined };
/** 400 Blöcke östlich: auf Zoom 0 ausserhalb des Rahmens, auf −1 darin, aber nicht im Fenster, auf −2 im Fenster. */
const FERN = { ...HAFEN, id: 'fern', name: 'Fern', at: [400.5, 0.5] };
const fern = (page: Page) => page.locator('.nadel-icon[title="Fern"]');
const imFenster = (page: Page) => fern(page).evaluate((e) => e.getBoundingClientRect().left < innerWidth);
/** Zoomt mit den Knöpfen um eine Stufe und wartet, bis die Adresse sie nennt, also nach `moveend`. */
async function stufe(page: Page, knopf: 'in' | 'out'): Promise<void> {
  const vorher = Number(new URL(page.url()).searchParams.get('zoom'));
  await page.locator(`.leaflet-control-zoom-${knopf}`).click();
  await expect.poll(() => Number(new URL(page.url()).searchParams.get('zoom'))).toBe(vorher + (knopf === 'in' ? 1 : -1));
}

test('nach 0128 hat ein Ort nur im Fenster mit Rand einen Marker: Hinauszoomen baut ihn, Hineinzoomen hängt ihn aus', async ({ page }) => {
  await welt(page, staedte([NAH, FERN], { mehr: WEIT }));
  await page.goto(`${DEMO}&at=0,0,0&zoom=0`);
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
  await expect(fern(page)).toHaveCount(0);
  // Auf −1 im Rahmen, aber noch rechts ausserhalb des Fensters: Der Rand baut ihn vorab.
  await stufe(page, 'out');
  await expect(fern(page)).toHaveCount(1);
  expect(await imFenster(page)).toBe(false);
  await stufe(page, 'out');
  await expect.poll(() => imFenster(page)).toBe(true);
  // Zurück auf 0: wieder ausgehängt, der nahe bleibt.
  await stufe(page, 'in');
  await stufe(page, 'in');
  await expect(fern(page)).toHaveCount(0);
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
});

test('nach 0128 kommt ein Ort schon beim Ziehen dazu, nicht erst beim Loslassen', async ({ page }) => {
  await welt(page, staedte([NAH, FERN], { mehr: WEIT }));
  await page.goto(`${DEMO}&at=0,0,0&zoom=0`);
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
  await expect(fern(page)).toHaveCount(0);
  // 800 px nach links sind 200 Blöcke: Fern liegt dann 200 Blöcke rechts der Mitte, im Rahmen.
  await page.mouse.move(1000, 360);
  await page.mouse.down();
  await page.mouse.move(200, 360, { steps: 20 });
  await expect(fern(page)).toHaveCount(1);
  await page.mouse.up();
});

test('nach 0128 bleibt ein Marker mit offener Tafel oder mit dem Fokus, auch ausserhalb des Rahmens', async ({ page }) => {
  await welt(page, staedte([NAH, FERN], { mehr: WEIT }));
  await page.goto(`${DEMO}&at=0,0,0&zoom=-2`);
  await expect.poll(() => imFenster(page)).toBe(true);
  // Mit gehaltener Tafel zurück auf 0: Er liegt weit ausserhalb, bleibt aber, und die Tafel bleibt offen.
  await fern(page).click();
  await expect(page.locator('.leaflet-popup')).toBeVisible();
  await stufe(page, 'in');
  await stufe(page, 'in');
  await expect(fern(page)).toHaveCount(1);
  expect(await imFenster(page)).toBe(false);
  await expect(page.locator('.leaflet-popup')).toBeVisible();
  // Geschlossen und einmal verschoben: Er weicht.
  await page.keyboard.press('Escape');
  await expect(page.locator('.leaflet-popup')).toHaveCount(0);
  await page.mouse.move(640, 360);
  await page.mouse.down();
  await page.mouse.move(600, 360, { steps: 4 });
  await page.mouse.up();
  await expect(fern(page)).toHaveCount(0);
  // Mit dem Fokus: hinaus, fokussieren, mit dem Rad um die Mitte hinein.
  await page.goto(`${DEMO}&at=0,0,0&zoom=-2`);
  await expect.poll(() => imFenster(page)).toBe(true);
  await fern(page).focus();
  await page.mouse.move(640, 360);
  for (let i = 0; i < 4; i++) {
    await page.mouse.wheel(0, -120);
    await page.waitForTimeout(300);
  }
  await expect.poll(() => imFenster(page)).toBe(false);
  await expect(fern(page)).toHaveCount(1);
  expect(await fern(page).evaluate((e) => e === document.activeElement)).toBe(true);
});

/** Die Städte mit einer Zoomgrenze in layers.json. */
const mitGrenze = (objekte: object[], grenze: number, mehr: object) => ({
  ...staedte(objekte, { mehr }),
  liste: () => [{ ...STAEDTE, maxBlocksPerPixel: grenze }],
});
const datei = (anfragen: string[]) => anfragen.filter((a) => a === 'beispiel/staedte.json').length;

test('nach 0131 holt die Karte eine Ebene jenseits ihrer Zoomgrenze nicht, genau auf der Grenze zeigt sie sie; hinaus hängt sie aus, ohne neue Anfrage', async ({ page }) => {
  // Von oben 4 Pixel je Block: Zoom −2 ist genau 1 Block je Pixel, −3 sind 2.
  const anfragen = await welt(page, mitGrenze([NAH], 1, WEIT));
  await page.goto(`${DEMO}&at=0,0,0&zoom=-3`);
  await expect(page.locator('.ebenen-zeile.ausser-bereich')).toHaveCount(1);
  await expect(page.locator('.ebenen-zeile .ebenen-bereich')).toHaveText('(beim Hineinzoomen)');
  await page.waitForTimeout(500);
  expect(datei(anfragen)).toBe(0);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  // Genau auf der Grenze: sichtbar.
  await stufe(page, 'in');
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
  await expect(page.locator('.ebenen-zeile.ausser-bereich')).toHaveCount(0);
  expect(datei(anfragen)).toBe(1);
  // Hinaus hängt sie aus, hinein wieder ein, ohne die Datei noch einmal zu holen.
  await stufe(page, 'out');
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  await stufe(page, 'in');
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
  expect(datei(anfragen)).toBe(1);
});

test('nach 0131 gilt die Zoomgrenze in Blöcken, nicht in Stufen: im iso mit scale 16 dieselbe Grenze an anderer Stufe', async ({ page }) => {
  // 2:1 mit scale 16: Ein Block ist auf der feinsten Stufe √(8² + 4²) ≈ 8,9 Pixel breit; Zoom −3 sind 0,89 Blöcke je Pixel, −4 sind 1,79.
  const anfragen = await welt(page, mitGrenze([{ ...NAH, at: [35.5, -14.5] }], 1, { bounds: [-65536, -65536, 65536, 65536] }));
  await page.goto(`${DEMO}&at=35,0,-15&zoom=-3`);
  await expect(page.locator('.nadel-icon[title="Nah"]')).toHaveCount(1);
  await stufe(page, 'out');
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
  await page.goto(`${DEMO}&at=35,0,-15&zoom=-4`);
  await expect(page.locator('.ebenen-zeile.ausser-bereich')).toHaveCount(1);
  await page.waitForTimeout(500);
  expect(datei(anfragen)).toBe(1);
});

test('nach 0131 schliesst eine offene Tafel, wenn ihre Ebene beim Hinauszoomen über die Grenze fällt', async ({ page }) => {
  await welt(page, mitGrenze([{ ...HAFEN, at: [0.5, 0.5] }], 1, WEIT));
  await page.goto(`${DEMO}&at=0,0,0&zoom=-2`);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  await expect(page.locator('.leaflet-popup')).toBeVisible();
  await stufe(page, 'out');
  await expect(page.locator('.leaflet-popup')).toHaveCount(0);
  await expect(page.locator('.nadel-icon')).toHaveCount(0);
});
