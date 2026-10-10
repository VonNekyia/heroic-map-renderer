import { expect, test, type Page } from '@playwright/test';
import { deflateSync } from 'node:zlib';
import { projiziere, zweiZuEins } from '../src/pick';
import { aufDemSchirm, BILDER, DEMO, fuss, HAFEN, KREISE, LEER, png, STAEDTE, staedte, welt } from './ebenen-welt';

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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
    const zwischen = { ...HAFEN, id: 'zwischen', name: 'Zwischen', at: [35.5, -14.5] };
    const ohne = { ...HAFEN, id: 'ohne', name: 'Ohne', at: [600.5, -14.5] };
    await welt(page, staedte([zwischen, ohne], { mehr: { seaLevel: 40, direction: richtung }, hoehe }));
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
      const [bx, bz] = drehe(x, z);
      const soll = await aufDemSchirm(page, ...projiziere(bx, y, bz, zweiZuEins(16)));
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

test('Nadeln und Banner bleiben beim Hinauszoomen gleich gross, jede mit ihrem Namen, Städte wie Dörfer', async ({ page }) => {
  const dorf = { ...HAFEN, id: 'dorf', name: 'Dorf', size: 'small', at: [30.5, -20.5], panel: undefined };
  const fahne = { id: 'fahne', type: 'banner', at: [40.5, -10.5], image: 'images/fahne.png', name: 'Fahne' };
  // scale 4: Auf Stufe −8 ist ein Block 1/64 Pixel breit; früher war dort jede Nadel aus.
  await welt(page, staedte([HAFEN, dorf, fahne], { mehr: { scale: 4, minZoom: -6 }, bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]) }));
  for (const zoom of [0, -8]) {
    await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
    await expect(page.locator('.nadel-name')).toHaveCount(3);
    const groessen = await page.locator('.nadel-icon').evaluateAll((l) =>
      l.map((e) => `${e.getAttribute('title')} ${e.getBoundingClientRect().width} × ${e.getBoundingClientRect().height}`).sort(),
    );
    expect(groessen, `Zoom ${zoom}`).toEqual(['Dorf 9 × 15', 'Fahne 22 × 40', 'Hafenstadt 23 × 33']);
  }
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
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon')).toHaveCount(2);
  for (const [name, [b, h], [x, z]] of [
    ['Gross', [32, 64], [20.5, -30.5]],
    ['Schmal', [21, 40], [30.5, -20.5]],
  ] as const) {
    const leinwand = page.locator(`.nadel-icon[title="${name}"] canvas`);
    expect(await leinwand.evaluate((c: HTMLCanvasElement) => [c.width, c.height, c.getBoundingClientRect().width, c.getBoundingClientRect().height]), name).toEqual([b, h, b, h]);
    const ist = await leinwand.evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data]);
    const soll: number[] = [];
    for (let py = 0; py < h; py++) for (let px = 0; px < b; px++) soll.push((px * 7) % 256, (py * 3) % 256, 128, 255);
    expect(ist, name).toEqual(soll);
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

test('Banner teilen sich ein Bild: die Karte holt es einmal', async ({ page }) => {
  const fahne = (id: string, at: [number, number]) => ({ id, type: 'banner', at, image: 'images/nation.png', name: id });
  const anfragen = await welt(page, staedte([fahne('A', [20.5, -30.5]), fahne('B', [30.5, -20.5]), fahne('C', [40.5, -10.5])], { bild: (n) => (n === 'nation.png' ? png(22, 40) : undefined) }));
  await page.goto(DEMO);
  await expect(page.locator('.nadel-icon canvas')).toHaveCount(3);
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

/** Wie viele Tafeln offen sind; eine schliessende blendet mit Deckkraft 0 aus. */
const offeneTafeln = (page: Page) => page.locator('.tafel').evaluateAll((l) => l.filter((e) => (e as HTMLElement).style.opacity !== '0').length);

test('beim Zeigen erscheint die Tafel nach 150 ms Ruhe, über der Nadel, und schliesst 300 ms nach dem Verlassen; dazwischen kann der Zeiger in die Tafel; kurz darüber öffnet nichts; ein Klick hält sie', async ({ page }) => {
  await page.clock.install();
  await welt(page, staedte([HAFEN]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  await expect(nadel).toHaveCount(1);
  const weg = () => page.mouse.move(400, 650);
  // Ohne Pause liefe die Uhr des Tests mit der echten Zeit.
  await page.clock.pauseAt(Date.now() + 1000);
  await nadel.hover();
  await page.clock.runFor(149);
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
  await page.clock.runFor(100);
  await weg();
  await page.clock.runFor(1000);
  expect(await offeneTafeln(page)).toBe(0);
  // Zurück binnen 300 ms: Sie bleibt offen.
  await nadel.hover();
  await page.clock.runFor(150);
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
  [1.25, 1],
  [1.5, 2],
  [2, 2],
] as const) {
  test.describe(`bei devicePixelRatio ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr });

    test(`ein Pixel des Banners ist ${k} Pixel des Geräts breit, jedes gleich`, async ({ page }) => {
      const fahne = { id: 'fahne', type: 'banner', at: [35.5, -14.5], image: 'images/fahne.png', name: 'Fahne' };
      await welt(page, staedte([fahne], { bild: (n) => (n === 'fahne.png' ? png(21, 40) : undefined) }));
      await page.goto(`${DEMO}&at=35,0,-15`);
      const leinwand = page.locator('.nadel-icon[title="Fahne"] canvas');
      await expect(leinwand).toHaveCount(1);
      const [w, h, cssW, cssH] = await leinwand.evaluate((c: HTMLCanvasElement) => [c.width, c.height, c.getBoundingClientRect().width, c.getBoundingClientRect().height]);
      expect([w, h]).toEqual([21 * k, 40 * k]);
      // So gross auf dem Schirm wie die Leinwand in Pixeln des Geräts; das Layout rechnet in 1/64 Pixel.
      expect(Math.abs(cssW * dpr - 21 * k)).toBeLessThan(1 / 32);
      expect(Math.abs(cssH * dpr - 40 * k)).toBeLessThan(1 / 32);
      expect(await page.locator('.nadel-icon[title="Fahne"]').evaluate((e) => Number.parseFloat((e as HTMLElement).style.marginLeft))).toBeCloseTo((-10 * k) / dpr, 3);
      const ist = await leinwand.evaluate((c: HTMLCanvasElement) => [...c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data]);
      const soll: number[] = [];
      for (let py = 0; py < 40 * k; py++) for (let px = 0; px < 21 * k; px++) soll.push((Math.floor(px / k) * 7) % 256, (Math.floor(py / k) * 3) % 256, 128, 255);
      expect(ist).toEqual(soll);
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
  const leinwand = page.locator('.nadel-icon[title="Fahne"] canvas');
  await expect(leinwand).toHaveJSProperty('width', 22);
  const cdp = await page.context().newCDPSession(page);
  const { width, height } = page.viewportSize()!;
  await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 2, mobile: false });
  await page.evaluate(() => (window as unknown as { anfragen: MediaQueryList[] }).anfragen.at(-1)!.dispatchEvent(new Event('change')));
  await expect(leinwand).toHaveJSProperty('width', 44);
  expect(await leinwand.evaluate((c) => c.getBoundingClientRect().width)).toBe(22);
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

test('ein Bild der Tafel breiter als 320 Pixel wird mit gleichem Seitenverhältnis verkleinert, ein schmales nie vergrössert; ohne Glättung nur bei ganzem Faktor', async ({ page }) => {
  const bild = (image: string, width: number, height: number) => ({ type: 'image', image, width, height });
  // Die Dateien: burg_16 ist 16 × 16, burg_9 ist 9 × 9.
  const breit = { ...HAFEN, panel: { blocks: [bild('images/burg_16.png', 512, 64), bild('images/burg_9.png', 9, 9), bild('images/burg_16.png', 32, 32), bild('images/burg_16.png', 24, 24)] } };
  await welt(page, staedte([breit]));
  await page.goto(`${DEMO}&at=35,0,-15`);
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  const bilder = page.locator('.tafel img');
  await expect(bilder).toHaveCount(4);
  await expect.poll(() => bilder.evaluateAll((l) => l.every((i) => (i as HTMLImageElement).complete))).toBe(true);
  const masse = await bilder.evaluateAll((l) => l.map((i) => [i.getBoundingClientRect().width, i.getBoundingClientRect().height, getComputedStyle(i).imageRendering]));
  expect(masse).toEqual([
    // 320 / 16 = 20, aber 40 / 16 = 2,5: kein ganzer Faktor.
    [320, 40, 'auto'],
    [9, 9, 'pixelated'],
    [32, 32, 'pixelated'],
    [24, 24, 'auto'],
  ]);
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
