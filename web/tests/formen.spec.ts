import { expect, test, type Locator, type Page } from '@playwright/test';
import { projiziere, zweiZuEins } from '../src/pick';
import { aufDemSchirm, DEMO, HAFEN, STAEDTE, staedte, welt } from './ebenen-welt';

/** Eine Region aus einem Rechteck der Welt. */
const rechteck = (id: string, x0: number, z0: number, x1: number, z1: number, mehr: object = {}) => ({
  id,
  type: 'region',
  polygons: [{ outer: [[x0, z0], [x1, z0], [x1, z1], [x0, z1]] }],
  ...mehr,
});
const GEBIET = rechteck('gebiet', 16, -32, 48, 0, { fill: '#40E53F55' });
const VON_OBEN = { camera: 'top', projection: { azimuth: 'north', u: 16, v: 16, y: 0 } };

/** Die Ecken des Rahmens um ein Element auf dem Schirm. */
const rahmen = async (l: Locator) => {
  const b = (await l.boundingBox())!;
  return [b.x, b.y, b.x + b.width, b.y + b.height];
};

/** Der Rahmen um Punkte der Welt auf Höhe y, nach der Projektion von 2:1, scale 16, auf Stufe 2. */
const sollRahmen = async (page: Page, punkte: number[][], y: number) => {
  const auf = await Promise.all(punkte.map(([x, z]) => aufDemSchirm(page, ...projiziere(x!, y, z!, zweiZuEins(16)))));
  return [Math.min(...auf.map((p) => p[0]!)), Math.min(...auf.map((p) => p[1]!)), Math.max(...auf.map((p) => p[0]!)), Math.max(...auf.map((p) => p[1]!))];
};

const nah = (ist: number[], soll: number[], toleranz = 1) => ist.forEach((w, i) => expect(Math.abs(w - soll[i]!), `${i}: ${w} statt ${soll[i]}`).toBeLessThanOrEqual(toleranz));

/** Länge eines Pfads aus seinem `d`, in Pixeln. */
const laenge = (d: string) => {
  const z = (d.match(/-?\d+(\.\d+)?/g) ?? []).map(Number);
  let s = 0;
  for (let i = 2; i + 1 < z.length; i += 2) s += Math.hypot(z[i]! - z[i - 2]!, z[i + 1]! - z[i - 1]!);
  return s;
};

for (const hoehe of [10, 40]) {
  test(`eine Region liegt im iso auf dem Gelände, auf Höhe ${hoehe} um deren Pixel höher; Füllung in ihrer Farbe, Rand in der Farbe ohne Alpha`, async ({ page }) => {
    await welt(page, staedte([GEBIET], { hoehe: () => hoehe }));
    // Die Mitte der Region in der Mitte des Fensters, damit Leaflet nichts abschneidet.
    await page.goto(`${DEMO}&at=32,${hoehe},-16`);
    const flaeche = page.locator('path[fill="#40E53F55"]');
    await expect(flaeche).toHaveCount(1);
    nah(await rahmen(flaeche), await sollRahmen(page, GEBIET.polygons[0]!.outer, hoehe + 1));
    await expect(flaeche).toHaveAttribute('fill-opacity', '1');
    const raender = page.locator('path[stroke="#40E53F"]');
    await expect(raender).toHaveCount(1);
    await expect(raender).toHaveAttribute('stroke-width', '2');
    await expect(raender).toHaveAttribute('fill', 'none');
    // Der Rand liegt auf derselben Höhe wie die Fläche.
    nah(await rahmen(raender), await sollRahmen(page, GEBIET.polygons[0]!.outer, hoehe + 1));
  });
}

test('eine Linie über unebenem Grund liegt auf ihm: ihr Pfad reicht so weit, wie die Projektion ihre Punkte auf ihre Höhe legt', async ({ page }) => {
  // Ein Grat quer zur Linie, sanft genug, dass nichts verdeckt ist.
  const f = (i: number) => 2 * Math.abs(i - 5);
  const linie = { id: 'grat', type: 'line', points: [[0, -20], [40, -20]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([linie], { hoehe: (i) => f(i) }));
  await page.goto(`${DEMO}&at=20,5,-20`);
  const pfad = page.locator('path[stroke="#123456"]');
  await expect(pfad).toHaveCount(1);
  // Unabhängig nachgerechnet: H linear zwischen den Mitten der Zellen, + 1; Punkte an jeder Mitte und dazwischen.
  const H = (x: number) => {
    const fx = x / 4 - 0.5;
    const i = Math.floor(fx);
    return f(i) * (1 - (fx - i)) + f(i + 1) * (fx - i) + 1;
  };
  const xs = [0, 1, 2, ...Array.from({ length: 9 }, (_, k) => [4 + 4 * k, 6 + 4 * k]).flat(), 39, 40];
  const auf = await Promise.all(xs.map((x) => aufDemSchirm(page, ...projiziere(x, H(x), -20, zweiZuEins(16)))));
  nah(await rahmen(pfad), [Math.min(...auf.map((p) => p[0]!)), Math.min(...auf.map((p) => p[1]!)), Math.max(...auf.map((p) => p[0]!)), Math.max(...auf.map((p) => p[1]!))]);
});

test('eine gestrichelte Linie wird nicht am Rand des Renderers geschnitten, so springen ihre Striche beim Verschieben nicht', async ({ page }) => {
  const lang = { id: 'lang', type: 'line', points: [[-400, 4], [400, 4]], stroke: { color: '#654321', width: 2, style: 'dashed' } };
  await welt(page, staedte([lang]));
  await page.goto(`${DEMO}&at=0,0,4`);
  const pfad = page.locator('path[stroke="#654321"]');
  await expect(pfad).toHaveCount(1);
  // 800 Blöcke, je Block 8 Pixel nach rechts und 4 nach unten: weit über das Fenster hinaus.
  expect(laenge((await pfad.getAttribute('d'))!)).toBeCloseTo(800 * Math.hypot(8, 4), -1);
});

test('Leaflet vereinfacht eine Fläche nicht: auf jeder Stufe dieselben Punkte', async ({ page }) => {
  const f = (i: number, j: number) => Math.round(6 * Math.sin(i / 2) + 4 * Math.cos(j / 3));
  await welt(page, staedte([rechteck('rau', 17, -31, 47, -1, { fill: '#ABCDEFFF' })], { hoehe: f, mehr: { minZoom: -6 } }));
  const punkte = async (zoom: number) => {
    await page.goto(`${DEMO}&at=32,5,-16&zoom=${zoom}`);
    await expect(page.locator('path[fill="#ABCDEFFF"]')).toHaveCount(1);
    return (await page.locator('path[fill="#ABCDEFFF"]').getAttribute('d'))!.match(/[ML]/g)!.length;
  };
  const fein = await punkte(0);
  expect(fein).toBeGreaterThan(50);
  expect(await punkte(-4)).toBe(fein);
});

test('von oben liegt eine Region eben, ein Kreis bleibt rund', async ({ page }) => {
  const kreis = { id: 'kreis', type: 'circle', center: [60, 60], radius: 60, fill: '#2040E0AA' };
  await welt(page, staedte([kreis], { mehr: VON_OBEN, hoehe: (i, j) => 4 * (i + j) }));
  // Zwei Stufen unter der feinsten: ein Block ist 4 Pixel breit.
  await page.goto(`${DEMO}&at=60,0,60&zoom=-2`);
  const flaeche = page.locator('path[fill="#2040E0AA"]');
  await expect(flaeche).toHaveCount(1);
  const b = (await flaeche.boundingBox())!;
  expect(Math.abs(b.width - 480)).toBeLessThanOrEqual(1);
  expect(Math.abs(b.height - 480)).toBeLessThanOrEqual(1);
});

test('hinter einem Wall füllt die Karte nichts und zeichnet den Rand dünn, gestrichelt und blass; davor alles', async ({ page }) => {
  // Ein Wall quer über die Welt, wo i + j = 10; aus se liegt vorn, wo x + z gross ist.
  // Die hintere hat eine Tafel: Ganz verdeckt ist sie trotzdem kein Ziel für Tab.
  const hinten = rechteck('hinten', 8, 8, 16, 16, { fill: '#FF000080', panel: { blocks: [{ type: 'title', text: 'Hinten' }] } });
  const vorn = rechteck('vorn', 60, 60, 68, 68, { fill: '#0000FF80' });
  await welt(page, staedte([hinten, vorn], { hoehe: (i, j) => (i + j === 10 ? 150 : 0) }));
  await page.goto(`${DEMO}&at=40,0,40`);
  await expect(page.locator('path[fill="#0000FF80"]')).toHaveCount(1);
  await expect(page.locator('path[fill="#FF000080"]')).toHaveCount(0);
  await expect(page.locator('path[tabindex]')).toHaveCount(0);
  expect((await page.locator('path[fill="#0000FF80"]').boundingBox())!.width).toBeGreaterThan(50);
  const stile = (farbe: string) =>
    page.locator(`path[stroke="${farbe}"]`).evaluateAll((l) => l.map((p) => [p.getAttribute('stroke-opacity'), p.getAttribute('stroke-dasharray'), p.getAttribute('stroke-width')]));
  expect(new Set((await stile('#FF0000')).map(String))).toEqual(new Set([['0.4', '3 4', '1'].join()]));
  expect(new Set((await stile('#0000FF')).map(String))).toEqual(new Set([['1', null, '2'].join()]));
});

test('eine gestrichelte Linie zählt ihre Striche über verdeckte Stücke hinweg, auch nach einem Zoom', async ({ page }) => {
  const route = { id: 'route', type: 'line', points: [[0, 4], [80, 4]], stroke: { color: '#3A6EA5', width: 3, style: 'dashed', dash: [10, 8] } };
  await welt(page, staedte([route], { hoehe: (i, j) => (i + j === 10 ? 150 : 0) }));
  await page.goto(`${DEMO}&at=40,0,4`);
  const laeufe = page.locator('path[stroke="#3A6EA5"]');
  await expect(laeufe).toHaveCount(2);
  const lesen = () => laeufe.evaluateAll((l) => l.map((p) => [p.getAttribute('d')!, p.getAttribute('stroke-dasharray'), p.getAttribute('stroke-dashoffset')] as const));
  const [hinten, davor] = await lesen();
  // Hinter dem Wall verdeckt, ab dem Wall sichtbar; die Striche gehen weiter, wo der verdeckte Lauf endet.
  expect(hinten![1]).toBe('3 4');
  expect(davor![1]).toBe('10 8');
  expect(Number(davor![2])).toBeGreaterThan(10);
  expect(Number(davor![2])).toBeCloseTo(laenge(hinten![0]), 0);
  // Eine Stufe tiefer ist alles halb so lang.
  await page.goto(`${DEMO}&at=40,0,4&zoom=-1`);
  await expect(laeufe).toHaveCount(2);
  const [hinten1, davor1] = await lesen();
  expect(Number(davor1![2])).toBeCloseTo(laenge(hinten1![0]), 0);
  expect(Number(davor1![2])).toBeCloseTo(Number(davor![2]) / 2, 0);
});

test('eine Fläche nennt beim Zeigen ihren Namen als Text und öffnet beim Klick ihre Tafel; eine Nadel liegt über ihr', async ({ page }) => {
  const gebiet = { ...GEBIET, name: '<b>Gebiet</b>', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet, HAFEN]));
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  const [x, y] = await aufDemSchirm(page, ...projiziere(20, 1, -28, zweiZuEins(16)));
  await page.mouse.move(x!, y!);
  const name = page.locator('.leaflet-tooltip.ebene-name');
  await expect(name).toHaveText('<b>Gebiet</b>');
  await expect(name.locator('b')).toHaveCount(0);
  await page.mouse.click(x!, y!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  await page.locator('.leaflet-popup-close-button').click();
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  // Die erste Tafel blendet noch aus; es zählt die der Nadel.
  await expect(page.locator('.tafel .tafel-titel', { hasText: 'Hafenstadt' })).toHaveText('✪ Hafenstadt');
  await expect(page.locator('.tafel .tafel-titel', { hasText: 'Gebietstafel' })).toHaveCount(0);
});

test('eine Fläche mit Tafel erreicht die Tastatur: Enter öffnet die Tafel mit dem Fokus darin, Escape gibt ihn der Fläche zurück', async ({ page }) => {
  const gebiet = { ...GEBIET, name: 'Gebiet', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet]));
  await page.goto(DEMO);
  const ziel = page.locator('path[tabindex="0"]');
  await expect(ziel).toHaveAttribute('role', 'button');
  await expect(ziel).toHaveAttribute('aria-label', 'Gebiet');
  await ziel.focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.tafel .leaflet-popup-content')).toBeFocused();
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  await page.keyboard.press('Escape');
  await expect(page.locator('.tafel')).toHaveCount(0);
  await expect(ziel).toBeFocused();
});

test('Flächen liegen nach order übereinander; eine ohne Namen und Tafel lässt Klicks zu den Flächen darunter durch', async ({ page }) => {
  const oben = { ...STAEDTE, id: 'beispiel:oben', name: { de: 'Oben' }, order: 2 };
  const mitte = { ...STAEDTE, id: 'beispiel:mitte', name: { de: 'Mitte' }, order: 1 };
  const unten = { ...STAEDTE, id: 'beispiel:unten', name: { de: 'Unten' }, order: 0 };
  await welt(page, {
    liste: () => [unten, oben, mitte],
    datei: (n) =>
      ({
        oben: { objects: [rechteck('o', 16, -32, 48, 0, { fill: '#FF0000FF' })] },
        mitte: { objects: [rechteck('m', 16, -32, 48, 0, { fill: '#00FF00FF', name: 'Mitte' })] },
        unten: { objects: [rechteck('u', 16, -32, 48, 0, { fill: '#0000FFFF', name: 'Unten' })] },
      })[n],
  });
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#FF0000FF"]')).toHaveCount(1);
  const [x, y] = await aufDemSchirm(page, ...projiziere(32, 1, -16, zweiZuEins(16)));
  // Zu sehen ist die oberste, zu treffen die oberste mit Namen.
  const z = (farbe: string) => page.locator(`path[fill="${farbe}"]`).evaluate((p) => Number(getComputedStyle(p.closest('.leaflet-pane')!).zIndex));
  expect(await z('#FF0000FF')).toBeGreaterThan(await z('#00FF00FF'));
  expect(await z('#00FF00FF')).toBeGreaterThan(await z('#0000FFFF'));
  expect(await z('#FF0000FF')).toBeLessThan(510);
  expect(await page.evaluate(([x, y]) => document.elementFromPoint(x!, y!)?.getAttribute('fill'), [x, y])).toBe('#00FF00FF');
});

test('bräuchte eine Ebene mehr als 1024 Regionen Höhen, liegt sie mit Meldung auf seaLevel und erscheint trotzdem', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  let hoehen = 0;
  // Der Demobaum hat kein area: Ein Kreis mit 50 000 Blöcken Radius streift Tausende Regionen.
  await welt(page, staedte([{ id: 'riesig', type: 'circle', center: [0, 0], radius: 50_000, stroke: { color: '#0F0F0F', width: 1 } }]));
  page.on('request', (r) => {
    if (r.url().includes('/heights/')) hoehen++;
  });
  await page.goto(DEMO);
  await expect(page.locator('path[stroke="#0F0F0F"]')).toHaveCount(1);
  expect(meldungen.some((m) => m.includes('mehr als 1024'))).toBe(true);
  // Höhen holt nur die Koordinatenanzeige für ihre Umgebung, nicht die Ebene.
  expect(hoehen).toBeLessThan(50);
});

test('Höhen lädt die Karte nur innerhalb von area, jede Region einmal, auch für zwei Formen und eine Linie darin', async ({ page }) => {
  const anfragen: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/heights/')) anfragen.push(new URL(r.url()).pathname.split('/').pop()!);
  });
  // area deckt nur die Region (−1, −1); der Streifen zur Kamera und die Linie reichen darüber hinaus.
  const objekte = [
    rechteck('a', -100, -100, -60, -60, { fill: '#111111FF' }),
    rechteck('b', -50, -50, -20, -20, { fill: '#222222FF' }),
    { id: 'l', type: 'line', points: [[-200, -30], [300, -30]], stroke: { color: '#333333' } },
  ];
  await welt(page, { ...staedte(objekte, { mehr: { area: [-512, -512, 0, 0] }, hoehe: () => 5 }), liste: () => [{ ...STAEDTE, visible: false }] });
  // Der Blick weit weg, so holt die Karte selbst keine dieser Regionen; die Ebene erst nach dem Laden.
  await page.goto(`${DEMO}&at=-1500,5,-1500`);
  await page.waitForLoadState('networkidle');
  anfragen.length = 0;
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input').check();
  await expect(page.locator('path[fill="#222222FF"]')).toHaveCount(1);
  await expect(page.locator('path[stroke="#333333"]')).toHaveCount(1);
  // Genau die eine Region in area, genau einmal.
  expect(anfragen).toEqual(['-1.-1.bin']);
});

test('eine Linie, die eine Region nur an der Ecke streift, lädt auch deren Höhen', async ({ page }) => {
  const anfragen: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/heights/')) anfragen.push(new URL(r.url()).pathname.split('/').pop()!);
  });
  // Von (400, 600) nach (600, 400): rund 34 Blöcke um (500, 500) liegen in der Region (0, 0).
  const ecke = { id: 'ecke', type: 'line', points: [[400, 600], [600, 400]], stroke: { color: '#444444' } };
  await welt(page, { ...staedte([ecke]), liste: () => [{ ...STAEDTE, visible: false }] });
  // Der Blick weit weg, so holt die Karte selbst die Region (0, 0) nicht; die Ebene erst nach dem Laden.
  await page.goto(`${DEMO}&at=-1500,0,-1500`);
  await page.waitForLoadState('networkidle');
  anfragen.length = 0;
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input').check();
  await expect(page.locator('path[stroke="#444444"]')).toHaveCount(1);
  expect(anfragen).toContain('0.0.bin');
});

test.describe('auf dem Touchscreen', () => {
  test.use({ hasTouch: true });

  test('der Umriss beim Tippen liegt über den Flächen der Ebenen und unter ihren Nadeln', async ({ page }) => {
    await welt(page, staedte([{ ...GEBIET, fill: '#40E53FFF' }]));
    await page.goto(DEMO);
    const flaeche = page.locator('path[fill="#40E53FFF"]');
    await expect(flaeche).toHaveCount(1);
    const [x, y] = await aufDemSchirm(page, ...projiziere(32, 1, -16, zweiZuEins(16)));
    await page.touchscreen.tap(x!, y!);
    const umriss = page.locator('.leaflet-shadow-pane path');
    await expect(umriss).toHaveAttribute('d', /M/);
    const z = (l: Locator) => l.evaluate((e) => Number(getComputedStyle(e.closest('.leaflet-pane')!).zIndex));
    expect(await z(umriss)).toBeGreaterThan(await z(flaeche));
    expect(await z(umriss)).toBeLessThan(510);
  });
});

test('was über die Grenzen geht, übergeht die Karte mit Meldung', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const viele = Array.from({ length: 10_001 }, (_, i) => [i % 100, Math.floor(i / 100)]);
  await welt(page, staedte([
    { id: 'gross', type: 'circle', center: [0, 0], radius: 100_001, fill: '#FF0000FF' },
    { id: 'lang', type: 'line', points: viele },
    rechteck('loecher', 0, 0, 200, 200, { fill: '#00FF00FF', polygons: [{ outer: [[0, 0], [200, 0], [200, 200], [0, 200]], holes: Array.from({ length: 101 }, (_, i) => [[i, 1], [i + 0.5, 1], [i + 0.5, 2]]) }] }),
    rechteck('gut', 16, -32, 48, 0, { fill: '#0000FFFF' }),
  ]));
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#0000FFFF"]')).toHaveCount(1);
  await expect(page.locator('path[fill="#FF0000FF"], path[fill="#00FF00FF"]')).toHaveCount(0);
  for (const m of ['Radius 100001', 'mehr als 10000 Punkte', 'mehr als 100 Löcher']) expect(meldungen.some((x) => x.includes(m)), m).toBe(true);
});

const WESTMEER = {
  id: 'meer',
  type: 'label',
  text: 'Westmeer',
  path: [[-30, -30], [30, -24], [90, -30]],
  size: 4,
  spacing: 0.3,
  color: '#2B3A55',
  outline: { color: '#F2E8D0CC', width: 3 },
};

/** Die Kartenschrift mit dieser `id`. */
const schrift = (page: Page, id: string) => page.locator(`svg.ebene-schrift[data-id="${id}"]`);

/** Wie hoch ein „H“ in der Schrift der Karte bei der Schriftgrösse eines Schriftzugs steht, gemessen auf einer Leinwand. */
const hHoehe = (page: Page, id: string) =>
  schrift(page, id).evaluate(async (svg) => {
    await document.fonts.ready;
    const ctx = document.createElement('canvas').getContext('2d')!;
    ctx.font = `${svg.querySelector('text')!.getAttribute('font-size')}px Kartenschrift`;
    return ctx.measureText('H').actualBoundingBoxAscent;
  });

test('die Kartenschrift steht in der Schrift der Karte, ohne Verletzung der Content-Security-Policy, Grossbuchstaben size Blöcke hoch; unter 8 Pixeln aus, über 96 gedeckelt; Text bleibt Text', async ({ page }) => {
  await page.addInitScript(() => {
    const verletzt: string[] = [];
    Object.assign(window, { verletzt });
    document.addEventListener('securitypolicyviolation', (e) => verletzt.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  const riese = { ...WESTMEER, id: 'riese', text: 'Riese', size: 40, path: [[10, -60]] };
  const markup = { ...WESTMEER, id: 'markup', text: '<i>Meer</i>', path: [[60, -60]] };
  await welt(page, staedte([WESTMEER, riese, markup], { mehr: { minZoom: -6 } }));
  await page.goto(`${DEMO}&at=30,0,-30`);
  await expect(schrift(page, 'meer').locator('textPath')).toHaveText('Westmeer');
  expect(await page.evaluate(async () => (await document.fonts.ready).check('16px Kartenschrift'))).toBe(true);
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual([]);
  // Auf der feinsten Stufe ist ein Block 16 Pixel breit: 4 Blöcke heissen 64 Pixel, 40 Blöcke gedeckelt auf 96.
  expect(Math.abs((await hHoehe(page, 'meer')) - 64)).toBeLessThanOrEqual(1);
  expect(Math.abs((await hHoehe(page, 'riese')) - 96)).toBeLessThanOrEqual(1);
  const text = schrift(page, 'meer').locator('text');
  await expect(text).toHaveAttribute('fill', '#2B3A55');
  await expect(text).toHaveAttribute('letter-spacing', String(0.3 * 64));
  await expect(text).toHaveAttribute('stroke', '#F2E8D0CC');
  await expect(text).toHaveAttribute('stroke-width', '6');
  await expect(text).toHaveAttribute('paint-order', 'stroke');
  // Die Führung der Schrift ist unsichtbar.
  await expect(schrift(page, 'meer').locator('path')).toHaveAttribute('fill', 'none');
  await expect(schrift(page, 'meer').locator('path')).not.toHaveAttribute('stroke', /.*/);
  await expect(schrift(page, 'markup').locator('textPath')).toHaveText('<i>Meer</i>');
  await expect(schrift(page, 'markup').locator('i')).toHaveCount(0);
  // Drei Stufen tiefer: 4 Blöcke sind 8 Pixel, noch zu sehen; vier Stufen tiefer 4 Pixel, aus.
  await page.goto(`${DEMO}&at=30,0,-30&zoom=-3`);
  await expect(schrift(page, 'meer')).toBeVisible();
  expect(Math.abs((await hHoehe(page, 'meer')) - 8)).toBeLessThanOrEqual(1);
  await page.goto(`${DEMO}&at=30,0,-30&zoom=-4`);
  // Erst steht die Ebene, dann zählt, dass die kleine Schrift aus ist; ein fehlendes Element wäre auch „hidden“.
  await expect(schrift(page, 'riese')).toBeVisible();
  await expect(schrift(page, 'meer')).toHaveCSS('display', 'none');
});

test('die Kartenschrift liegt auf ihrem Pfad im iso über dem Gelände; nach links kehrt sie um, zu kurz geht der Pfad weiter, ein Punkt heisst waagrecht', async ({ page }) => {
  const links = { ...WESTMEER, id: 'links', path: [[90, -40], [-30, -40]] };
  const kurz = { ...WESTMEER, id: 'kurz', text: 'Ein langer Name', path: [[30, -10], [31, -10]] };
  const punkt = { ...WESTMEER, id: 'punkt', text: 'Insel', path: [[30, -50]] };
  await welt(page, staedte([WESTMEER, links, kurz, punkt], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&at=30,10,-30`);
  await expect(schrift(page, 'punkt')).toHaveCount(1);
  /** Anfang, Ende und Länge des gezeichneten Pfads auf dem Schirm, dazu die Länge des Texts. */
  const lage = (id: string) =>
    schrift(page, id).evaluate(async (svg: SVGSVGElement) => {
      await document.fonts.ready;
      const pfad = svg.querySelector('path')!;
      const kasten = svg.getBoundingClientRect();
      const n = pfad.getTotalLength();
      const [a, b] = [pfad.getPointAtLength(0), pfad.getPointAtLength(n)];
      return { a: [kasten.left + a.x, kasten.top + a.y], b: [kasten.left + b.x, kasten.top + b.y], n, text: svg.querySelector('textPath')!.getComputedTextLength() };
    });
  // Der Pfad beginnt und endet, wo die Projektion seine Punkte auf Höhe 11 hinlegt.
  const meer = await lage('meer');
  nah(meer.a, await aufDemSchirm(page, ...projiziere(-30, 11, -30, zweiZuEins(16))));
  nah(meer.b, await aufDemSchirm(page, ...projiziere(90, 11, -30, zweiZuEins(16))));
  // Nach links gezeichnet, läuft er umgekehrt.
  const l = await lage('links');
  expect(l.a[0]).toBeLessThan(l.b[0]!);
  nah(l.a, await aufDemSchirm(page, ...projiziere(-30, 11, -40, zweiZuEins(16))));
  // Zu kurz: Er geht an beiden Enden weiter, mindestens so lang wie der Text.
  const k = await lage('kurz');
  expect(k.n).toBeGreaterThanOrEqual(k.text);
  // Ein Punkt: waagrecht, mittig um ihn.
  const p = await lage('punkt');
  expect(Math.abs(p.a[1]! - p.b[1]!)).toBeLessThanOrEqual(0.5);
  expect(p.n).toBeGreaterThanOrEqual(p.text);
  const mitte = await aufDemSchirm(page, ...projiziere(30, 11, -50, zweiZuEins(16)));
  nah([(p.a[0]! + p.b[0]!) / 2, p.a[1]!], mitte);
  // Mittig auf dem Pfad: die Grundlinie eine halbe Höhe der Grossbuchstaben, 64 / 2 Pixel, darunter.
  const grundlinie = await schrift(page, 'punkt').evaluate((svg: SVGSVGElement) => {
    const text = svg.querySelector('text')!;
    return text.getStartPositionOfChar(0).y - svg.querySelector('path')!.getPointAtLength(0).y;
  });
  expect(grundlinie).toBeCloseTo(32, 0);
});

test('die Kartenschrift liegt im Pane ihrer Ebene, über deren Flächen', async ({ page }) => {
  await welt(page, staedte([{ ...GEBIET, fill: '#40E53FFF' }, { ...WESTMEER, path: [[16, -16], [48, -16]] }]));
  await page.goto(`${DEMO}&at=32,0,-16`);
  await expect(schrift(page, 'meer')).toHaveCount(1);
  await expect(page.locator('path[fill="#40E53FFF"]')).toHaveCount(1);
  const [gleich, danach] = await page.evaluate(() => {
    const s = document.querySelector('svg.ebene-schrift')!;
    const f = document.querySelector('path[fill="#40E53FFF"]')!;
    return [s.closest('.leaflet-pane') === f.closest('.leaflet-pane'), Boolean(f.compareDocumentPosition(s) & Node.DOCUMENT_POSITION_FOLLOWING)];
  });
  expect(gleich).toBe(true);
  expect(danach).toBe(true);
});
