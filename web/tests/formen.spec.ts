import { expect, test, type Locator, type Page } from '@playwright/test';
import { projiziere, zweiZuEins } from '../src/pick';
import { aufDemSchirm, BILDER, DEMO, fuss, HAFEN, png, STAEDTE, staedte, welt } from './ebenen-welt';

/** Eine Region aus einem Rechteck der Welt. */
const rechteck = (id: string, x0: number, z0: number, x1: number, z1: number, mehr: object = {}) => ({
  id,
  type: 'region',
  polygons: [{ outer: [[x0, z0], [x1, z0], [x1, z1], [x0, z1]] }],
  ...mehr,
});
const GEBIET = rechteck('gebiet', 16, -32, 48, 0, { fill: '#40E53F55' });
const VON_OBEN = { camera: 'top', projection: { azimuth: 'north' as const, u: 16, v: 16, y: 0 } };

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

test('von oben steht jede Art dort, wo die Projektion sie hinlegt, gleich welche Höhe: Linie, Kartenschrift, Nadel und Banner', async ({ page }) => {
  // Ein Hang unter allem: Von oben zählt keine Höhe, P = (16 · x, 16 · z).
  const linie = { id: 'linie', type: 'line', points: [[10, 12], [30, 12], [30, 24]], stroke: { color: '#123456', width: 2 } };
  const schrift = { id: 'meer', type: 'label', text: 'Meer', path: [[12, 40], [38, 40]], size: 2 };
  const nadel = { ...HAFEN, id: 'nadel', name: 'Nadel', at: [20.5, 30.5], panel: undefined };
  const banner = { id: 'banner', type: 'banner', at: [35.5, 30.5], image: 'images/fahne.png', name: 'Banner' };
  await welt(page, staedte([linie, schrift, nadel, banner], {
    mehr: VON_OBEN,
    hoehe: (i, j) => 4 * (i + j),
    bild: (n) => (n === 'fahne.png' ? png(22, 40) : BILDER[n]),
  }));
  await page.goto(`${DEMO}&at=25,0,28`);
  const p = (x: number, z: number) => aufDemSchirm(page, ...projiziere(x, 0, z, VON_OBEN.projection));
  // Die Linie: ihr Rahmen über die drei Punkte.
  const pfad = page.locator('path[stroke="#123456"]');
  await expect(pfad).toHaveCount(1);
  nah(await rahmen(pfad), [...(await p(10, 12)), ...(await p(30, 24))]);
  // Die Kartenschrift: ihr Pfad von Punkt zu Punkt, waagrecht.
  const lage = await page.locator('svg.ebene-schrift[data-id="meer"]').evaluate((svg: SVGSVGElement) => {
    const weg = svg.querySelector('path')!;
    const kasten = svg.getBoundingClientRect();
    const [a, b] = [weg.getPointAtLength(0), weg.getPointAtLength(weg.getTotalLength())];
    return [kasten.left + a.x, kasten.top + a.y, kasten.left + b.x, kasten.top + b.y];
  });
  nah(lage, [...(await p(12, 40)), ...(await p(38, 40))]);
  // Nadel und Banner mit dem Fuss auf ihrem Ort.
  nah(await fuss(page, 'Nadel'), await p(20.5, 30.5));
  nah(await fuss(page, 'Banner'), await p(35.5, 30.5));
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

test('mit ground liegen Fläche, Rand und Linie im iso auf dem Boden, Nadel und Kartenschrift auf der Oberfläche', async ({ page }) => {
  // Kronen auf 40, der Boden darunter auf 10.
  const linie = { id: 'weg', type: 'line', points: [[20, -40], [44, -40]], stroke: { color: '#123456', width: 2 } };
  const nadel = { ...HAFEN, id: 'nadel', name: 'Nadel', at: [24.5, -8.5], panel: undefined };
  const schrift = { id: 'wald', type: 'label', text: 'Wald', path: [[18, -20], [46, -20]], size: 2 };
  await welt(page, staedte([{ ...GEBIET, stroke: { color: '#40E53F' } }, linie, nadel, schrift], { hoehe: () => 40, boden: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16`);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  nah(await rahmen(page.locator('path[fill="#40E53F55"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 11));
  nah(await rahmen(page.locator('path[stroke="#40E53F"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 11));
  nah(await rahmen(page.locator('path[stroke="#123456"]')), await sollRahmen(page, linie.points, 11));
  nah(await fuss(page, 'Nadel'), await aufDemSchirm(page, ...projiziere(24.5, 41, -8.5, zweiZuEins(16))));
  const anfang = await page.locator('svg.ebene-schrift[data-id="wald"]').evaluate((svg: SVGSVGElement) => {
    const a = svg.querySelector('path')!.getPointAtLength(0);
    const kasten = svg.getBoundingClientRect();
    return [kasten.left + a.x, kasten.top + a.y];
  });
  nah(anfang, await aufDemSchirm(page, ...projiziere(18, 41, -20, zweiZuEins(16))));
});

test('fehlt einer Region die Datei in ground, liegen Formen dort auf heights', async ({ page }) => {
  // GEBIET liegt in der Region (0, −1); deren ground fehlt.
  await welt(page, staedte([GEBIET], { hoehe: () => 30, boden: (i) => (i >= 0 ? undefined : 5) }));
  await page.goto(`${DEMO}&at=32,30,-16`);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  nah(await rahmen(page.locator('path[fill="#40E53F55"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 31));
});

test('unter Kronen bleibt ein Rand auf dem Boden ganz zu sehen; hinter einem Hang des Bodens dünn, gestrichelt und blass', async ({ page }) => {
  // Ein Wall wo i + j = 10, wie oben; einmal nur in heights, wie eine Reihe Kronen, einmal im Boden.
  const hinten = rechteck('hinten', 8, 8, 16, 16, { fill: '#FF000080' });
  const wall = (i: number, j: number) => (i + j === 10 ? 150 : 0);
  const stile = () =>
    page.locator('path[stroke="#FF0000"]').evaluateAll((l) => l.map((p) => [p.getAttribute('stroke-opacity'), p.getAttribute('stroke-dasharray')].join()));
  await welt(page, staedte([hinten], { hoehe: wall, boden: () => 0 }));
  await page.goto(`${DEMO}&at=40,0,40`);
  await expect(page.locator('path[fill="#FF000080"]')).toHaveCount(1);
  expect(new Set(await stile())).toEqual(new Set(['1,']));
  await page.unrouteAll({ behavior: 'ignoreErrors' });
  await welt(page, staedte([hinten], { hoehe: wall, boden: wall }));
  await page.goto(`${DEMO}&at=40,0,40`);
  await expect(page.locator('path[stroke="#FF0000"]').first()).toBeAttached();
  await expect(page.locator('path[fill="#FF000080"]')).toHaveCount(0);
  expect(new Set(await stile())).toEqual(new Set(['0.4,3 4']));
});

test('im iso steht am Rand einer Fläche eine Wand, 6 Blöcke hoch, unten 0,6 deckend, nach oben bis 0; nicht an Linien, nicht ohne Rand, nicht um einen Kreis ohne Füllung, nicht von oben', async ({ page }) => {
  const ohneRand = rechteck('ohne', 60, -32, 80, 0, { fill: '#AA00AA55', stroke: { width: 0 } });
  const nurUmriss = { id: 'umkreis', type: 'circle', center: [32, -60], radius: 10, stroke: { color: '#00AAAA', width: 2 } };
  const linie = { id: 'weg', type: 'line', points: [[20, -40], [44, -40]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([{ ...GEBIET, stroke: { color: '#40E53FCC' } }, ohneRand, nurUmriss, linie], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16`);
  const baender = page.locator('path[fill="#40E53F"]');
  await expect(baender).toHaveCount(12);
  // Von unten nach oben, nach der Unterkante jedes Bands: die Deckkraft fällt.
  const vonUnten = (await baender.evaluateAll((l) => l.map((p) => [p.getBoundingClientRect().bottom, Number(p.getAttribute('fill-opacity'))] as const))).sort((a, b) => b[0] - a[0]);
  vonUnten.forEach(([, d], k) => expect(d).toBeCloseTo(0.6 * (1 - (k + 0.5) / 12), 5));
  for (const b of await baender.all()) {
    await expect(b).toHaveAttribute('stroke', 'none');
    // Gleich herum gelaufen, siehe wand in gelaende.ts: nur mit nonzero ohne Loch, wo sich Vorder- und Rückseite decken.
    await expect(b).toHaveAttribute('fill-rule', 'nonzero');
  }
  // Zusammen reichen die Bänder vom Rand bis 6 Blöcke darüber.
  const rand = await sollRahmen(page, GEBIET.polygons[0]!.outer, 11);
  const alle = await Promise.all((await baender.all()).map(rahmen));
  nah([Math.min(...alle.map((r) => r[0]!)), Math.min(...alle.map((r) => r[1]!)), Math.max(...alle.map((r) => r[2]!)), Math.max(...alle.map((r) => r[3]!))], [rand[0]!, rand[1]! - 6 * zweiZuEins(16).y, rand[2]!, rand[3]!]);
  await expect(page.locator('path[fill="#AA00AA"]')).toHaveCount(0);
  await expect(page.locator('path[fill="#123456"]')).toHaveCount(0);
  await expect(page.locator('path[stroke="#00AAAA"]')).toHaveCount(1);
  await expect(page.locator('path[fill="#00AAAA"]')).toHaveCount(0);
  await page.unrouteAll({ behavior: 'ignoreErrors' });
  await welt(page, staedte([{ ...GEBIET, stroke: { color: '#40E53FCC' } }], { mehr: VON_OBEN }));
  await page.goto(`${DEMO}&at=32,0,-16`);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  await expect(page.locator('path[fill="#40E53F"]')).toHaveCount(0);
});

test('ground holt die Karte nur für Flächen, Ränder und Linien, nicht für Nadeln und Kartenschrift', async ({ page }) => {
  const boden: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/ground/')) boden.push(r.url());
  });
  const schrift = { id: 'wald', type: 'label', text: 'Wald', path: [[18, -20], [46, -20]], size: 2 };
  await welt(page, staedte([HAFEN, schrift], { hoehe: () => 40, boden: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16`);
  await expect(page.locator('svg.ebene-schrift[data-id="wald"]')).toBeAttached();
  await page.waitForLoadState('networkidle');
  expect(boden).toEqual([]);
});

test('eine Fläche nennt beim Zeigen ihren Namen als Text und hält beim Klick ihre Tafel; eine Nadel liegt über ihr', async ({ page }) => {
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

test('eine Fläche öffnet ihre Tafel beim Zeigen; von Hand geschlossen, öffnet erst ein neues Zeigen sie wieder, auch nach dem Weg vom Schliessknopf hinaus', async ({ page }) => {
  const gebiet = { ...GEBIET, name: 'Gebiet', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet]));
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  const punkt = (x: number, z: number) => aufDemSchirm(page, ...projiziere(x, 1, z, zweiZuEins(16)));
  // Mitten in der Fläche, so liegt auch der Schliessknopf über ihr.
  const [[x1, y1], [x2, y2], [x3, y3]] = await Promise.all([punkt(32, -16), punkt(34, -14), punkt(60, 10)]);
  await page.mouse.move(x1!, y1!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  const knopf = page.locator('.tafel .leaflet-popup-close-button');
  const ueber = await knopf.evaluate((k) => {
    const r = k.getBoundingClientRect();
    return document.elementsFromPoint(r.x + r.width / 2, r.y + r.height / 2).some((e) => e.getAttribute('fill') === '#40E53F55');
  });
  expect(ueber).toBe(true);
  const titel = (await page.locator('.tafel .tafel-titel').boundingBox())!;
  await knopf.click();
  // Durch die ausblendende Tafel: Sie zählt nicht als Ort ausserhalb.
  await page.mouse.move(titel.x + 2, titel.y + titel.height / 2);
  await expect(page.locator('.tafel')).toHaveCount(0);
  // Weiter in der Fläche: bleibt zu.
  await page.mouse.move(x2!, y2!, { steps: 5 });
  await page.waitForTimeout(600);
  await expect(page.locator('.tafel')).toHaveCount(0);
  // Hinaus und wieder hinein: öffnet.
  await page.mouse.move(x3!, y3!);
  await page.mouse.move(x2!, y2!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  // An der Ecke, wo der Schliessknopf neben der Fläche liegt: von ihm hinaus, dann hinein, öffnet auch.
  await page.mouse.move(x3!, y3!);
  const [ex, ey] = await punkt(20, -28);
  await page.mouse.move(ex!, ey!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  await page.locator('.tafel .leaflet-popup-close-button').click();
  await expect(page.locator('.tafel')).toHaveCount(0);
  await page.mouse.move(x3!, y3!);
  await page.mouse.move(x2!, y2!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
});

test('eine Fläche öffnet ihre Tafel erst nach 50 ms Ruhe, am Ort der Ruhe; jede Bewegung beginnt die Ruhe von vorn', async ({ page }) => {
  await page.clock.install();
  const gebiet = { ...GEBIET, name: 'Gebiet', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet]));
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  await page.clock.pauseAt(Date.now() + 1000);
  const offen = () => page.locator('.tafel').evaluateAll((l) => l.filter((e) => (e as HTMLElement).style.opacity !== '0').length);
  const punkt = (x: number, z: number) => aufDemSchirm(page, ...projiziere(x, 1, z, zweiZuEins(16)));
  const [[x1, y1], [x2, y2]] = await Promise.all([punkt(24, -24), punkt(36, -12)]);
  await page.mouse.move(x1!, y1!);
  for (let i = 1; i <= 4; i++) {
    await page.clock.runFor(40);
    await page.mouse.move(x1! + ((x2! - x1!) * i) / 4, y1! + ((y2! - y1!) * i) / 4);
  }
  expect(await offen()).toBe(0);
  await page.clock.runFor(49);
  expect(await offen()).toBe(0);
  await page.clock.runFor(1);
  expect(await offen()).toBe(1);
  // Die Spitze der Tafel zeigt auf den Ort der Ruhe, nicht auf den Eintritt.
  const tafel = (await page.locator('.tafel').boundingBox())!;
  expect(Math.abs(tafel.x + tafel.width / 2 - x2!)).toBeLessThanOrEqual(1);
});

test('ein Klick auf eine Region nur mit Namen schliesst eine offene Tafel und hält keinen Block; erst der nächste hält einen', async ({ page }) => {
  const benannt = { ...GEBIET, name: 'Gebiet' };
  await welt(page, staedte([benannt, HAFEN]));
  await page.goto(DEMO);
  await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
  const anzeige = page.locator('.koordinaten');
  await page.locator('.nadel-icon[title="Hafenstadt"]').click();
  await expect(page.locator('.tafel')).toHaveCount(1);
  // Unter der Nadel in der Region, nicht unter der Tafel über ihr.
  const [x, y] = await aufDemSchirm(page, ...projiziere(40, 1, -4, zweiZuEins(16)));
  await page.mouse.click(x!, y!);
  await expect(page.locator('.tafel')).toHaveCount(0);
  await expect(anzeige).not.toHaveClass(/gehalten/);
  await page.mouse.click(x!, y!);
  await expect(anzeige).toHaveClass(/gehalten/);
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

  test('Tippen öffnet die Tafel einer Fläche', async ({ page }) => {
    const gebiet = { ...GEBIET, panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
    await welt(page, staedte([gebiet]));
    await page.goto(DEMO);
    await expect(page.locator('path[fill="#40E53F55"]')).toHaveCount(1);
    const [x, y] = await aufDemSchirm(page, ...projiziere(20, 1, -28, zweiZuEins(16)));
    await page.touchscreen.tap(x!, y!);
    await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  });

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
  // check() sagt auch für eine nie geladene Schrift ja; es zählt die geladene FontFace.
  await expect.poll(() => page.evaluate(() => [...document.fonts].some((f) => f.family === 'Kartenschrift' && f.status === 'loaded'))).toBe(true);
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

test('Grenzfälle der Kartenschrift wie im Format: ohne Kontur bei {}, null oder keinem Objekt, size ≤ 0 oder kein Zahl wird 16, spacing auf 0 bis 1 gekappt, ein Feld mit falschem Typ nimmt die Vorgabe', async ({ page }) => {
  const zug = (id: string, mehr: object, i: number) => ({ id, type: 'label', text: 'Meer', path: [[-30 + 25 * (i % 6), -40 - 25 * Math.floor(i / 6)]], size: 4, ...mehr });
  const faelle: [string, object][] = [
    ['leer', { outline: {} }],
    ['null', { outline: null }],
    ['text', { outline: 'rot' }],
    ['breite-text', { outline: { width: '3' } }],
    ['farbe-zahl', { outline: { color: 5, width: 3 } }],
    ['groesse-null', { size: 0 }],
    ['groesse-minus', { size: -2 }],
    ['groesse-text', { size: '4' }],
    ['weit', { spacing: 5 }],
    ['eng', { spacing: -1 }],
    ['sperrung-text', { spacing: '0.3' }],
    ['farbe-falsch', { color: 123 }],
  ];
  await welt(page, staedte(faelle.map(([id, mehr], i) => zug(id, mehr, i)), { mehr: { minZoom: -6 } }));
  // Zwei Stufen unter der feinsten ist ein Block 4 Pixel breit: size 4 heisst 16 Pixel, size 16 heisst 64.
  await page.goto(`${DEMO}&at=30,0,-40&zoom=-2`);
  await expect(schrift(page, 'farbe-falsch')).toHaveCount(1);
  const text = (id: string) => schrift(page, id).locator('text');
  for (const id of ['leer', 'null', 'text', 'breite-text']) await expect(text(id), id).not.toHaveAttribute('stroke', /.*/);
  await expect(text('farbe-zahl')).toHaveAttribute('stroke', '#F2E8D0');
  for (const id of ['groesse-null', 'groesse-minus', 'groesse-text']) expect(Math.abs((await hHoehe(page, id)) - 64), id).toBeLessThanOrEqual(1);
  // Die Sperrung in Pixeln: Anteil mal Höhe der Grossbuchstaben, hier 16.
  await expect(text('weit')).toHaveAttribute('letter-spacing', '16');
  await expect(text('eng')).toHaveAttribute('letter-spacing', '0');
  await expect(text('sperrung-text')).toHaveAttribute('letter-spacing', '0');
  await expect(text('farbe-falsch')).toHaveAttribute('fill', '#2B2B2B');
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
