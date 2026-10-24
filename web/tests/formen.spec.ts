import { expect, test, type Locator, type Page } from '@playwright/test';
import { kreisSeiten } from '../src/gelaende';
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

/** Punkte von Flächen, Rändern, Linien und Wänden einer Ebene, wie die Karte sie mit `?punkte` an das Pane der Formen schreibt. */
const punkteDerEbene = (page: Page) => page.evaluate(() => Number(document.querySelector<HTMLElement>('.leaflet-pane[data-punkte]')?.dataset.punkte ?? Number.NaN));
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

/** Wo die Pfade auf dem Schirm bei x liegen: y ihres Punkts, der x am nächsten kommt, abgetastet alle 0,5 Pixel des Pfads. */
const yBei = (pfade: Locator, x: number) =>
  pfade.evaluateAll(
    (l, x) => {
      let best: [number, number] = [Infinity, 0];
      for (const p of l as SVGPathElement[]) {
        const m = p.getScreenCTM()!;
        for (let s = 0; s <= p.getTotalLength(); s += 0.5) {
          const q = p.getPointAtLength(s).matrixTransform(m);
          if (Math.abs(q.x - x) < best[0]) best = [Math.abs(q.x - x), q.y];
        }
      }
      return best[1];
    },
    x,
  );

/** Länge eines Pfads aus seinem `d`, in Pixeln. */
const laenge = (d: string) => {
  const z = (d.match(/-?\d+(\.\d+)?/g) ?? []).map(Number);
  let s = 0;
  for (let i = 2; i + 1 < z.length; i += 2) s += Math.hypot(z[i]! - z[i - 2]!, z[i + 1]! - z[i - 1]!);
  return s;
};

for (const hoehe of [10, 40]) {
  test(`eine Region liegt im iso auf dem Gelände, auf Höhe ${hoehe} um deren Pixel höher; die Fläche ohne Füllung und ohne Nebel, Rand in der Farbe ohne Alpha`, async ({ page }) => {
    await welt(page, staedte([GEBIET], { hoehe: () => hoehe }));
    // Die Mitte der Region in der Mitte des Fensters, damit Leaflet nichts abschneidet.
    await page.goto(`${DEMO}&at=32,${hoehe},-16`);
    const flaeche = page.locator('path[data-fill="#40E53F55"]');
    await expect(flaeche).toHaveCount(1);
    nah(await rahmen(flaeche), await sollRahmen(page, GEBIET.polygons[0]!.outer, hoehe + 1));
    // Die Fläche selbst füllt im iso nicht, sie ist nur Ziel für Name und Tafel.
    await expect(flaeche).toHaveAttribute('fill-opacity', '0');
    // Kein Nebel, kein Muster: im iso nur Rand und Wand.
    await expect(page.locator('pattern, path.nebel-band, path.nebel-umriss')).toHaveCount(0);
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

test('nach 0111 vereinfacht Leaflet einen Strich nicht noch einmal: eine Zickzacklinie mit 61 Ecken 0,6 px neben der Sehne behält im Pfad jeden Punkt', async ({ page }) => {
  // Wie weit die Ecke (8, −20 + d) neben der Sehne von (0, −20) nach (16, −20) liegt, auf Zoom −3, in Pixeln je Block d.
  const zoom = -3;
  const p = (x: number, z: number) => projiziere(x, 11, z, zweiZuEins(16));
  const [a, b, c] = [p(0, -20), p(8, -19), p(16, -20)];
  const jeBlock = (Math.abs((c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])) / Math.hypot(c[0] - a[0], c[1] - a[1])) * 2 ** zoom;
  // Über ε = 0,5 px, unter dem 1 px, mit dem Leaflet sonst vereinfacht. Leaflet rundet die Punkte vorher auf ganze Pixel;
  // mit vielen Ecken fallen darum immer einige auf höchstens 1 px und gingen verloren.
  const d = 0.6 / jeBlock;
  const zickzack = { id: 'zickzack', type: 'line', points: Array.from({ length: 61 }, (_, k) => [8 * k - 200, -20 + (k % 2) * d]), stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([zickzack], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&punkte&at=48,10,-20&zoom=${zoom}`);
  const pfad = page.locator('path[stroke="#123456"]');
  await expect(pfad).toHaveCount(1);
  await expect.poll(() => punkteDerEbene(page)).toBeGreaterThan(0);
  const gerechnet = await punkteDerEbene(page);
  // Jede Ecke bleibt nach ε; im Pfad steht je Punkt ein Paar Zahlen.
  expect(gerechnet).toBeGreaterThanOrEqual(61);
  const imPfad = ((await pfad.getAttribute('d'))!.match(/-?\d+(\.\d+)?/g)!.length) / 2;
  expect(imPfad).toBe(gerechnet);
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

test('Leaflet vereinfacht eine Fläche nicht: auf jeder Stufe hat ihr Pfad die Punkte, die die Karte nach 0111 rechnet', async ({ page }) => {
  const f = (i: number, j: number) => Math.round(6 * Math.sin(i / 2) + 4 * Math.cos(j / 3));
  await welt(page, staedte([rechteck('rau', 17, -31, 47, -1, { fill: '#ABCDEFFF', stroke: { width: 0 } })], { hoehe: f, mehr: { minZoom: -6 } }));
  for (const zoom of [0, -4]) {
    await page.goto(`${DEMO}&punkte&at=32,5,-16&zoom=${zoom}`);
    const pfad = page.locator('path[data-fill="#ABCDEFFF"]');
    await expect(pfad).toHaveCount(1);
    // Die Ebene hat nur diese Fläche, ohne Rand und so ohne Wand: Alle gerechneten Punkte sind ihre.
    expect((await pfad.getAttribute('d'))!.match(/[ML]/g)!.length, `Zoom ${zoom}`).toBe(await punkteDerEbene(page));
  }
});

test('von oben liegt eine Region eben, ein Kreis bleibt rund', async ({ page }) => {
  const kreis = { id: 'kreis', type: 'circle', center: [60, 60], radius: 60, fill: '#2040E0AA' };
  await welt(page, staedte([kreis], { mehr: VON_OBEN, hoehe: (i, j) => 4 * (i + j) }));
  // Zwei Stufen unter der feinsten: ein Block ist 4 Pixel breit.
  await page.goto(`${DEMO}&at=60,0,60&zoom=-2`);
  const flaeche = page.locator('path[data-fill="#2040E0AA"]');
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
  await expect(page.locator('path[data-fill="#0000FF80"]')).toHaveCount(1);
  await expect(page.locator('path[data-fill="#FF000080"]')).toHaveCount(0);
  await expect(page.locator('path[tabindex]')).toHaveCount(0);
  expect((await page.locator('path[data-fill="#0000FF80"]').boundingBox())!.width).toBeGreaterThan(50);
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  nah(await rahmen(page.locator('path[data-fill="#40E53F55"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 11));
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  nah(await rahmen(page.locator('path[data-fill="#40E53F55"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 31));
});

test('mit groundCell 1 folgt eine Linie dem Boden je Block: ein Plateau von 20 Blöcken hebt sie auf seine Höhe', async ({ page }) => {
  const linie = { id: 'plateau', type: 'line', points: [[0, -20], [52, -20]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([linie], { hoehe: () => 10, boden: (i) => (i >= 16 && i < 36 ? 30 : 10), bodenZelle: 1 }));
  await page.goto(`${DEMO}&at=26,15,-20`);
  const laeufe = page.locator('path[stroke="#123456"]');
  await expect(laeufe.first()).toBeAttached();
  // Mitten auf dem Plateau, bei x 26, liegt sie auch geglättet auf seiner Oberseite, 31: ihr Punkt dort auf dem Schirm.
  const [sx, sy] = await aufDemSchirm(page, ...projiziere(26, 31, -20, zweiZuEins(16)));
  const dort = await yBei(laeufe, sx!);
  expect(Math.abs(dort - sy!)).toBeLessThanOrEqual(1);
});

test('mit groundCell 1 nimmt eine Region ohne Datei in ground die gröberen Zellen aus heights', async ({ page }) => {
  // GEBIET liegt in der Region (0, −1); deren ground fehlt, heights hat dort je 4 × 4 die 30.
  await welt(page, staedte([GEBIET], { hoehe: () => 30, boden: (i) => (i >= 0 ? undefined : 5), bodenZelle: 1 }));
  await page.goto(`${DEMO}&at=32,30,-16`);
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  nah(await rahmen(page.locator('path[data-fill="#40E53F55"]')), await sollRahmen(page, GEBIET.polygons[0]!.outer, 31));
});

test('Flächen und Ränder holen ground nur für den Ausschnitt; bräuchte der mehr als 64 Regionen ground je Block, liegen sie mit Meldung auf heights', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  const boden: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/ground/')) boden.push(r.url());
  });
  // Rund 10 × 10 Regionen: über dem Budget für ground, darunter für heights.
  await welt(page, staedte([rechteck('weit', -5000, -5000, -100, -100, { fill: '#556677FF' })], { hoehe: () => 5, boden: () => 1, bodenZelle: 1, mehr: { minZoom: -6 } }));
  // Nah nach 0111: nur die Regionen, die im Ausschnitt liegen können, weit unter den rund 100 der Ebene.
  await page.goto(`${DEMO}&at=-300,5,-300`);
  await expect(page.locator('path[data-fill="#556677FF"]')).toHaveCount(1);
  await expect.poll(() => boden.length).toBeGreaterThan(0);
  expect(new Set(boden).size).toBeLessThan(20);
  expect(meldungen.some((m) => m.includes('Regionen ground, mehr als 64'))).toBe(false);
  // Weit herausgezoomt deckt der Ausschnitt die ganze Ebene.
  await page.goto(`${DEMO}&at=-2550,5,-2550&zoom=-4`);
  await expect.poll(() => meldungen.some((m) => m.includes('Regionen ground, mehr als 64'))).toBe(true);
  await expect(page.locator('path[data-fill="#556677FF"]')).toHaveCount(1);
});

test('unter Kronen bleibt ein Rand auf dem Boden ganz zu sehen; hinter einem Hang des Bodens dünn, gestrichelt und blass', async ({ page }) => {
  // Ein Wall wo i + j = 10, wie oben; einmal nur in heights, wie eine Reihe Kronen, einmal im Boden.
  const hinten = rechteck('hinten', 8, 8, 16, 16, { fill: '#FF000080' });
  const wall = (i: number, j: number) => (i + j === 10 ? 150 : 0);
  const stile = () =>
    page.locator('path[stroke="#FF0000"]').evaluateAll((l) => l.map((p) => [p.getAttribute('stroke-opacity'), p.getAttribute('stroke-dasharray')].join()));
  await welt(page, staedte([hinten], { hoehe: wall, boden: () => 0 }));
  await page.goto(`${DEMO}&at=40,0,40`);
  await expect(page.locator('path[data-fill="#FF000080"]')).toHaveCount(1);
  expect(new Set(await stile())).toEqual(new Set(['1,']));
  await page.unrouteAll({ behavior: 'ignoreErrors' });
  await welt(page, staedte([hinten], { hoehe: wall, boden: wall }));
  await page.goto(`${DEMO}&at=40,0,40`);
  await expect(page.locator('path[stroke="#FF0000"]').first()).toBeAttached();
  await expect(page.locator('path[data-fill="#FF000080"]')).toHaveCount(0);
  expect(new Set(await stile())).toEqual(new Set(['0.4,3 4']));
});

test('hinter einem Haus bleibt eine Linie voll gezeichnet, nicht gestrichelt und nicht blass', async ({ page }) => {
  // Ein Haus von 6 × 4 Blöcken, 7 hoch, vor der Linie bei z −20; aus se liegt vorn, wo z gross ist.
  const linie = { id: 'weg', type: 'line', points: [[0, -20], [44, -20]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([linie], { hoehe: () => 10, boden: (i, j) => (i >= 19 && i < 25 && j >= -17 && j < -13 ? 17 : 10), bodenZelle: 1 }));
  await page.goto(`${DEMO}&at=22,12,-18`);
  const laeufe = page.locator('path[stroke="#123456"]');
  await expect(laeufe.first()).toBeAttached();
  const stile = await laeufe.evaluateAll((l) => l.map((p) => [p.getAttribute('stroke-dasharray'), p.getAttribute('stroke-opacity')].join()));
  expect(new Set(stile)).toEqual(new Set([',1']));
});

test('ein Haus von 6 Blöcken am Weg hebt die Linie nicht: entlang des Wegs geglättet, über ihm liegt sie auf dem Boden daneben', async ({ page }) => {
  const linie = { id: 'weg', type: 'line', points: [[0, -20], [60, -20]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([linie], { hoehe: () => 10, boden: (i) => (i >= 27 && i < 33 ? 17 : 10), bodenZelle: 1 }));
  await page.goto(`${DEMO}&at=30,12,-20`);
  const laeufe = page.locator('path[stroke="#123456"]');
  await expect(laeufe.first()).toBeAttached();
  // Ihr Punkt bei x 30, mitten über dem Haus, liegt auf dem Boden neben ihm, 11, nicht auf dem Dach, 18.
  const [sx, sy] = await aufDemSchirm(page, ...projiziere(30, 11, -20, zweiZuEins(16)));
  const dort = await yBei(laeufe, sx!);
  expect(Math.abs(dort - sy!)).toBeLessThanOrEqual(1);
});

test('im iso steht am Rand einer Fläche eine Wand, 6 Blöcke hoch, unten 0,6 deckend, nach oben bis 0, auf Zoom 0 in 6 Bändern; nicht an Linien, nicht ohne Rand, nicht um einen Kreis ohne Füllung, nicht von oben', async ({ page }) => {
  const ohneRand = rechteck('ohne', 60, -32, 80, 0, { fill: '#AA00AA55', stroke: { width: 0 } });
  const nurUmriss = { id: 'umkreis', type: 'circle', center: [32, -60], radius: 10, stroke: { color: '#00AAAA', width: 2 } };
  const linie = { id: 'weg', type: 'line', points: [[20, -40], [44, -40]], stroke: { color: '#123456', width: 2 } };
  await welt(page, staedte([{ ...GEBIET, stroke: { color: '#40E53FCC' } }, ohneRand, nurUmriss, linie], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16`);
  const baender = page.locator('path[fill="#40E53F"]');
  // 48 px hoch, je Band mindestens 8 px; siehe wandBaender.
  await expect(baender).toHaveCount(6);
  // Von unten nach oben, nach der Unterkante jedes Bands: die Deckkraft fällt.
  const vonUnten = (await baender.evaluateAll((l) => l.map((p) => [p.getBoundingClientRect().bottom, Number(p.getAttribute('fill-opacity'))] as const))).sort((a, b) => b[0] - a[0]);
  vonUnten.forEach(([, d], k) => expect(d).toBeCloseTo(0.6 * (1 - (k + 0.5) / 6), 5));
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  await expect(page.locator('path[fill="#40E53F"]')).toHaveCount(0);
});

test('von oben füllt eine Fläche in ihrer Farbe, im iso nicht', async ({ page }) => {
  await welt(page, staedte([GEBIET], { mehr: VON_OBEN }));
  await page.goto(`${DEMO}&at=32,0,-16`);
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveAttribute('fill', '#40E53F55');
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveAttribute('fill-opacity', '1');
  await page.unrouteAll({ behavior: 'ignoreErrors' });
  await welt(page, staedte([GEBIET], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16`);
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveAttribute('fill-opacity', '0');
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

test('nach einem Mausklick auf eine Fläche mit Tafel oder nur mit Namen steht kein Fokusring um sie; ohne Tafel behält die Karte die Tastatur', async ({ page }) => {
  const mitTafel = { ...GEBIET, name: 'Mit Tafel', panel: { blocks: [{ type: 'title', text: 'T' }] } };
  const nurName = rechteck('nur', 60, -32, 92, 0, { fill: '#3F40E555', name: 'Nur Name' });
  await welt(page, staedte([mitTafel, nurName]));
  await page.goto(DEMO);
  for (const [farbe, x, z] of [['#40E53F55', 20, -28], ['#3F40E555', 64, -28]] as const) {
    const flaeche = page.locator(`path[data-fill="${farbe}"]`);
    await expect(flaeche).toHaveCount(1);
    // Ein Ring läge um den Rahmen der Fläche; verglichen wird der Streifen an ihrer linken oberen Ecke.
    const b = (await flaeche.boundingBox())!;
    const ecke = { x: Math.max(0, b.x - 6), y: Math.max(0, b.y - 6), width: 24, height: 24 };
    const vorher = await page.screenshot({ clip: ecke });
    const [px, py] = await aufDemSchirm(page, ...projiziere(x, 1, z, zweiZuEins(16)));
    await page.mouse.click(px!, py!);
    await page.waitForTimeout(200);
    expect(await page.screenshot({ clip: ecke }), farbe).toEqual(vorher);
    const aktiv = await page.evaluate(() => {
      const a = document.activeElement!;
      return { pfad: a.tagName === 'path', tabindex: a.getAttribute('tabindex'), outline: getComputedStyle(a).outlineStyle };
    });
    if (aktiv.pfad) expect(aktiv.outline, farbe).toBe('none');
    // Ohne Tafel ist die Fläche kein Ziel; der Fokus bleibt bei der Karte, die Pfeiltasten verschieben sie weiter.
    if (farbe === '#3F40E555') expect(aktiv, 'nur mit Namen').toEqual(expect.objectContaining({ pfad: false }));
    await page.keyboard.press('Escape');
    if (farbe === '#40E53F55') {
      // Escape gibt den Fokus aus der Tafel zurück, für die Tastatur sichtbar: der Umriss als Strich, kein Kasten.
      await expect(flaeche).toBeFocused();
      expect(await flaeche.evaluate((p) => [p.matches(':focus-visible'), getComputedStyle(p).outlineStyle, getComputedStyle(p).strokeWidth])).toEqual([true, 'none', '2px']);
    }
    await page.mouse.click(600, 650);
  }
});

test('eine Fläche nennt beim Zeigen ihren Namen als Text und hält beim Klick ihre Tafel; eine Nadel liegt über ihr', async ({ page }) => {
  const gebiet = { ...GEBIET, name: '<b>Gebiet</b>', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet, HAFEN]));
  await page.goto(DEMO);
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  const punkt = (x: number, z: number) => aufDemSchirm(page, ...projiziere(x, 1, z, zweiZuEins(16)));
  // Mitten in der Fläche, so liegt auch der Schliessknopf über ihr.
  const [[x1, y1], [x2, y2], [x3, y3]] = await Promise.all([punkt(32, -16), punkt(34, -14), punkt(60, 10)]);
  await page.mouse.move(x1!, y1!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  const knopf = page.locator('.tafel .leaflet-popup-close-button');
  const ueber = await knopf.evaluate((k) => {
    const r = k.getBoundingClientRect();
    return document.elementsFromPoint(r.x + r.width / 2, r.y + r.height / 2).some((e) => e.getAttribute('data-fill') === '#40E53F55');
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
  // Erst zu, sonst bliebe die Tafel von eben mit ihrem Knopf über der Fläche offen.
  await page.mouse.move(x3!, y3!);
  await expect(page.locator('.tafel')).toHaveCount(0);
  const [ex, ey] = await punkt(20, -28);
  await page.mouse.move(ex!, ey!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  const daneben = await knopf.evaluate((k) => {
    const r = k.getBoundingClientRect();
    return !document.elementsFromPoint(r.x + r.width / 2, r.y + r.height / 2).some((e) => e.getAttribute('data-fill') === '#40E53F55');
  });
  expect(daneben).toBe(true);
  await knopf.click();
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
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
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
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

test('nach einem Klick auf eine Fläche mit Tafel kein Rahmen; im Fokus der Tastatur ihr Umriss als Strich, kein Kasten', async ({ page }) => {
  const gebiet = { ...GEBIET, name: 'Gebiet', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet]));
  await page.goto(DEMO);
  const ziel = page.locator('path[tabindex="0"]');
  const stil = () => ziel.evaluate((e) => ({ outline: getComputedStyle(e).outlineStyle, strich: getComputedStyle(e).stroke }));
  await ziel.click();
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  await expect(ziel).toBeFocused();
  expect(await stil()).toEqual({ outline: 'none', strich: 'none' });
  // Mit der Tastatur: erst weg, dann zurück, so zählt der Fokus als sichtbar.
  await page.keyboard.press('Escape');
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Tab');
  await expect(ziel).toBeFocused();
  expect(await stil()).toEqual({ outline: 'none', strich: 'rgb(0, 0, 0)' });
});

/** Die Punkte im Pfad eines SVG-Elements: jedes M und L. */
const punkteIn = (pfad: Locator) => pfad.evaluate((p) => p.getAttribute('d')?.match(/[ML]/g)?.length ?? 0);

test('ohne Gelände im iso: ein gefüllter Kreis hat die Seiten nach 0111, ohne Netz und ohne Wand', async ({ page }) => {
  // Radius 100: auf Zoom −2, der tiefsten des Demobaums, rund 224 px, ganz im Bild, so schneidet Leaflet nichts ab.
  const kreis = { id: 'gross', type: 'circle', center: [32, -16], radius: 100, fill: '#AA5500AA', stroke: { color: '#AA5500' } };
  await welt(page, staedte([kreis], { mehr: { heights: null } }));
  await page.goto(`${DEMO}&at=32,0,-16&zoom=-2`);
  const flaeche = page.locator('path[data-fill="#AA5500AA"]');
  await expect(flaeche).toHaveCount(1);
  // Die Fläche vereinfacht Leaflet nicht (smoothFactor 0), so zählt der Pfad die Ecken; den Rand vereinfacht es auf dem Schirm.
  expect(await punkteIn(flaeche)).toBe(kreisSeiten(100 * Math.hypot(8, 4) * 2 ** -2, 0.5));
  await expect(page.locator('path[fill="#AA5500"]')).toHaveCount(0);
});

/** Gelände mit kleinen Buckeln, damit die Ränder etwas abzutasten haben. */
const buckel = (i: number, j: number) => 10 + ((i * 7 + j * 3) % 5);

test('nach 0111: Formen weit ausserhalb des Ausschnitts ändern die Punkte nicht; Herauszoomen erhöht sie nie', async ({ page }) => {
  const nah = { ...GEBIET, stroke: { color: '#40E53FCC' } };
  const fern = Array.from({ length: 30 }, (_, i) => rechteck(`fern-${i}`, 5000 + i * 300, 5000, 5200 + i * 300, 5200, { fill: '#123456FF' }));
  await welt(page, staedte([nah], { hoehe: buckel }));
  await page.goto(`${DEMO}&punkte&at=32,10,-16&zoom=0`);
  await expect.poll(() => punkteDerEbene(page)).toBeGreaterThan(0);
  const allein = await punkteDerEbene(page);
  await page.unrouteAll({ behavior: 'ignoreErrors' });
  await welt(page, staedte([nah, ...fern], { hoehe: buckel }));
  await page.reload();
  await expect.poll(() => punkteDerEbene(page)).toBe(allein);
  let vorher = Infinity;
  for (const zoom of [1, 0, -1, -2]) {
    await page.goto(`${DEMO}&punkte&at=32,10,-16&zoom=${zoom}`);
    await expect.poll(() => punkteDerEbene(page), { message: `Zoom ${zoom}` }).toBeGreaterThan(0);
    const n = await punkteDerEbene(page);
    expect(n, `Zoom ${zoom}`).toBeLessThanOrEqual(vorher);
    vorher = n;
  }
});

test('nach 0111: ein grosser Kreis hat als Rand und als Fläche höchstens 2 · Umfang auf dem Schirm / ε Punkte, gleich wie gross in Blöcken; neu gerechnet nach dem Verschieben', async ({ page }) => {
  // Einmal nur Rand, einmal nur Fläche, je ohne Wand; auf Zoom −3 rund 110 px Radius.
  const arten = { rand: { stroke: { color: '#5500AA' } }, flaeche: { fill: '#5500AA55' } };
  for (const [art, mehr] of Object.entries(arten)) {
    for (const [r, zoom] of [[100, -3], [800, -6]] as const) {
      await page.unrouteAll({ behavior: 'ignoreErrors' });
      await welt(page, staedte([{ id: 'k', type: 'circle', center: [32, -16], radius: r, ...mehr }], { hoehe: buckel, mehr: { maxZoom: 6 } }));
      await page.goto(`${DEMO}&punkte&at=32,10,-16&zoom=${zoom}`);
      await expect.poll(() => punkteDerEbene(page)).toBeGreaterThan(0);
      const n = await punkteDerEbene(page);
      // Der Umfang auf dem Schirm, über 1000 Punkte des Kreises auf Höhe 10 gerechnet.
      const faktor = 2 ** zoom;
      const ring = Array.from({ length: 1000 }, (_, i) => projiziere(32 + r * Math.cos((2 * Math.PI * i) / 1000), 11, -16 + r * Math.sin((2 * Math.PI * i) / 1000), zweiZuEins(16)));
      const umfang = ring.reduce((summe, p, i) => summe + Math.hypot(p[0] - ring[(i + 1) % 1000]![0], p[1] - ring[(i + 1) % 1000]![1]), 0) * faktor;
      expect(n, `${art}, Radius ${r}`).toBeLessThanOrEqual((2 * umfang) / 0.5 + 16);
    }
  }
  // Verschoben aus dem gerechneten Ausschnitt: neu gerechnet, die Punkte folgen.
  const vorher = await punkteDerEbene(page);
  await page.mouse.move(640, 360);
  await page.mouse.down();
  await page.mouse.move(-600, 360, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => punkteDerEbene(page)).not.toBe(vorher);
});

/** Zieht die Karte viermal um 1100 px nach links oder rechts, wie mit der Maus. */
async function ziehe(page: Page, richtung: 1 | -1): Promise<void> {
  for (let i = 0; i < 4; i++) {
    await page.mouse.move(640 - 550 * richtung, 360);
    await page.mouse.down();
    await page.mouse.move(640 + 550 * richtung, 360, { steps: 6 });
    await page.mouse.up();
  }
}

test('nach 0111: eine Fläche, die aus dem Ausschnitt fällt, ist kein Ziel mehr; zurück liegt sie wieder unter ihrem Rand und unter der späteren Fläche', async ({ page }) => {
  // Die spätere ist so gross, dass sie im Ausschnitt bleibt.
  const gross = rechteck('gross', -800, -800, 800, 800, { fill: '#77777755' });
  await welt(page, staedte([{ ...GEBIET, name: 'Gebiet', stroke: { color: '#40E53FCC' } }, gross], { hoehe: () => 10 }));
  await page.goto(`${DEMO}&at=32,10,-16&zoom=0`);
  const flaeche = page.locator('path[data-fill="#40E53F55"]');
  await expect(flaeche).toHaveCount(1);
  // 4400 px auf Zoom 0, rund 550 Blöcke in x − z: weit über den Ausschnitt hinaus.
  await ziehe(page, -1);
  await expect(flaeche).toHaveCount(0);
  await expect(page.locator('path[data-fill="#77777755"]')).toHaveCount(1);
  await ziehe(page, 1);
  await expect(flaeche).toHaveCount(1);
  const folgt = (a: string, b: string) =>
    page.evaluate(([a, b]) => (document.querySelector(a!)!.compareDocumentPosition(document.querySelector(b!)!) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0, [a, b]);
  expect(await folgt('path[data-fill="#40E53F55"]', 'path[stroke="#40E53FCC"]')).toBe(true);
  expect(await folgt('path[data-fill="#40E53F55"]', 'path[data-fill="#77777755"]')).toBe(true);
});

test('nach 0111: eine gehaltene Tafel einer Fläche bleibt offen, wenn die Karte die Fläche beim Zoomen neu rechnet', async ({ page }) => {
  const gebiet = { ...GEBIET, name: 'Gebiet', panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
  await welt(page, staedte([gebiet], { hoehe: buckel, mehr: { minZoom: -2 } }));
  await page.goto(`${DEMO}&punkte&at=32,0,-16&zoom=0`);
  await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
  const [x, y] = await aufDemSchirm(page, ...projiziere(20, 1, -28, zweiZuEins(16)));
  await page.mouse.click(x!, y!);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  const vorher = await punkteDerEbene(page);
  await page.locator('.leaflet-control-zoom-out').click();
  // Neu gerechnet: Auf Zoom −1 ist die Fläche kleiner und hat andere Punkte.
  await expect.poll(() => punkteDerEbene(page)).not.toBe(vorher);
  await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
});

test('die Wand hat je Band mindestens 8 px, höchstens 12 Bänder, unter 1 px keine; beim Zoomen neu gerechnet, unter dem Rand', async ({ page }) => {
  // maxZoom 6: auf Zoom −6 ist die Wand 6 · 8 / 64 = 0,75 px hoch, auf −5 1,5, auf −1 24, auf 0 48, auf 1 96.
  await welt(page, staedte([{ ...GEBIET, stroke: { color: '#40E53FCC' } }], { hoehe: () => 10, mehr: { maxZoom: 6 } }));
  await page.goto(`${DEMO}&at=32,10,-16&zoom=-6`);
  await expect(page.locator('path[stroke="#40E53FCC"]').first()).toBeAttached();
  await expect(page.locator('path[fill="#40E53F"]')).toHaveCount(0);
  await page.locator('.leaflet-control-zoom-in').click();
  await expect(page.locator('path[fill="#40E53F"]')).toHaveCount(1);
  for (const [zoom, n] of [[-1, 3], [1, 12], [0, 6]] as const) {
    await page.goto(`${DEMO}&at=32,10,-16&zoom=${zoom}`);
    await expect(page.locator('path[fill="#40E53F"]'), `Zoom ${zoom}`).toHaveCount(n);
  }
  // Die Ränder liegen über der Wand: im SVG nach ihr.
  const ueber = await page.evaluate(() => {
    const wand = document.querySelector('path[fill="#40E53F"]')!;
    const rand = document.querySelector('path[stroke="#40E53FCC"]')!;
    return (wand.compareDocumentPosition(rand) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  });
  expect(ueber).toBe(true);
  // Herauszoomen rechnet neu: 3 Bänder.
  await page.locator('.leaflet-control-zoom-out').click();
  await expect(page.locator('path[fill="#40E53F"]')).toHaveCount(3);
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
  await expect(page.locator('path[data-fill="#FF0000FF"]')).toHaveCount(1);
  const [x, y] = await aufDemSchirm(page, ...projiziere(32, 1, -16, zweiZuEins(16)));
  // Zu sehen ist die oberste, zu treffen die oberste mit Namen.
  const z = (farbe: string) => page.locator(`path[data-fill="${farbe}"]`).evaluate((p) => Number(getComputedStyle(p.closest('.leaflet-pane')!).zIndex));
  expect(await z('#FF0000FF')).toBeGreaterThan(await z('#00FF00FF'));
  expect(await z('#00FF00FF')).toBeGreaterThan(await z('#0000FFFF'));
  expect(await z('#FF0000FF')).toBeLessThan(510);
  expect(await page.evaluate(([x, y]) => document.elementFromPoint(x!, y!)?.getAttribute('data-fill'), [x, y])).toBe('#00FF00FF');
});

test('bräuchte der Ausschnitt einer Ebene mehr als 1024 Regionen Höhen, liegt sie mit Meldung auf seaLevel und erscheint trotzdem', async ({ page }) => {
  const meldungen: string[] = [];
  page.on('console', (m) => meldungen.push(m.text()));
  let hoehen = 0;
  // Der Demobaum hat kein area: Ein gefüllter Kreis mit 50 000 Blöcken Radius deckt Tausende Regionen. Auf Zoom −6 reicht der
  // Ausschnitt über rund 20 000 Blöcke, also über mehr als 1024 davon; der Rand liegt weiter draussen.
  await welt(page, staedte([{ id: 'riesig', type: 'circle', center: [0, 0], radius: 50_000, fill: '#0F0F0F55', stroke: { color: '#0F0F0F', width: 1 } }], { mehr: { minZoom: -6 } }));
  page.on('request', (r) => {
    if (r.url().includes('/heights/')) hoehen++;
  });
  await page.goto(`${DEMO}&at=0,0,0&zoom=-6`);
  await expect(page.locator('path[data-fill="#0F0F0F55"]')).toHaveCount(1);
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
  await welt(page, { ...staedte(objekte, { mehr: { area: [-512, -512, 0, 0], minZoom: -6 }, hoehe: () => 5 }), liste: () => [{ ...STAEDTE, visible: false }] });
  // Der Blick weit weg, so holt die Karte selbst keine dieser Regionen; die Ebene erst nach dem Laden.
  await page.goto(`${DEMO}&at=-1500,5,-1500`);
  await page.waitForLoadState('networkidle');
  anfragen.length = 0;
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input').check();
  // Alles liegt weit ausserhalb des Ausschnitts: nach 0111 kein Pfad und keine Höhen.
  await expect(page.locator('.ebenen input')).toBeChecked();
  await page.waitForLoadState('networkidle');
  await expect(page.locator('path[data-fill="#222222FF"]')).toHaveCount(0);
  await expect(page.locator('path[stroke="#333333"]')).toHaveCount(0);
  expect(anfragen).toEqual([]);
  // Weit herausgezoomt sind Flächen und Linie da, mit Höhen nur aus area, die Region (−1, −1) genau einmal;
  // die Mitte der Ansicht holt selbst nur Regionen um (−1500, −1500).
  await page.goto(`${DEMO}&at=-1500,5,-1500&zoom=-6`);
  await expect(page.locator('path[data-fill="#222222FF"]')).toHaveCount(1);
  await expect(page.locator('path[stroke="#333333"]').first()).toBeAttached();
  expect(anfragen.filter((a) => !/^-\d+\.-\d+\.bin$/.test(a))).toEqual([]);
  expect(anfragen.filter((a) => a === '-1.-1.bin')).toHaveLength(1);
});

test('eine Linie, die eine Region nur an der Ecke streift, lädt auch deren Höhen', async ({ page }) => {
  const anfragen: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/heights/')) anfragen.push(new URL(r.url()).pathname.split('/').pop()!);
  });
  // Von (400, 600) nach (600, 400): rund 34 Blöcke um (500, 500) liegen in der Region (0, 0).
  const ecke = { id: 'ecke', type: 'line', points: [[400, 600], [600, 400]], stroke: { color: '#444444' } };
  await welt(page, { ...staedte([ecke], { mehr: { minZoom: -6 } }), liste: () => [{ ...STAEDTE, visible: false }] });
  // Weit weg und weit herausgezoomt: Die Linie liegt im Ausschnitt, die Mitte der Ansicht holt selbst nur Regionen um (−1000, −1000).
  await page.goto(`${DEMO}&at=-1000,0,-1000&zoom=-6`);
  await page.waitForLoadState('networkidle');
  anfragen.length = 0;
  await page.locator('.ebenen summary').click();
  await page.locator('.ebenen input').check();
  await expect(page.locator('path[stroke="#444444"]').first()).toBeAttached();
  expect(anfragen).toContain('0.0.bin');
});

test.describe('auf dem Touchscreen', () => {
  test.use({ hasTouch: true });

  test('Tippen öffnet die Tafel einer Fläche', async ({ page }) => {
    const gebiet = { ...GEBIET, panel: { blocks: [{ type: 'title', text: 'Gebietstafel' }] } };
    await welt(page, staedte([gebiet]));
    await page.goto(DEMO);
    await expect(page.locator('path[data-fill="#40E53F55"]')).toHaveCount(1);
    const [x, y] = await aufDemSchirm(page, ...projiziere(20, 1, -28, zweiZuEins(16)));
    await page.touchscreen.tap(x!, y!);
    await expect(page.locator('.tafel .tafel-titel')).toHaveText('Gebietstafel');
  });

  test('der Umriss beim Tippen liegt über den Flächen der Ebenen und unter ihren Nadeln', async ({ page }) => {
    await welt(page, staedte([{ ...GEBIET, fill: '#40E53FFF' }]));
    await page.goto(DEMO);
    const flaeche = page.locator('path[data-fill="#40E53FFF"]');
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
  await expect(page.locator('path[data-fill="#0000FFFF"]')).toHaveCount(1);
  await expect(page.locator('path[data-fill="#FF0000FF"], path[data-fill="#00FF00FF"]')).toHaveCount(0);
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
  await expect(page.locator('path[data-fill="#40E53FFF"]')).toHaveCount(1);
  const [gleich, danach] = await page.evaluate(() => {
    const s = document.querySelector('svg.ebene-schrift')!;
    const f = document.querySelector('path[data-fill="#40E53FFF"]')!;
    return [s.closest('.leaflet-pane') === f.closest('.leaflet-pane'), Boolean(f.compareDocumentPosition(s) & Node.DOCUMENT_POSITION_FOLLOWING)];
  });
  expect(gleich).toBe(true);
  expect(danach).toBe(true);
});
