import { expect, test } from '@playwright/test';
import {
  abtasten,
  netz,
  oberflaeche,
  punktImBlick,
  rechteck,
  ringImRechteck,
  schriftPfad,
  sichtbareFelder,
  verdeckt,
  vieleck,
  zug,
  zugImRechteck,
  type Blick,
  type Gelaende,
  type Punkt,
} from '../src/gelaende';
import { projiziere, zweiZuEins, type Projektion } from '../src/pick';

/** Ein Gelände aus einer Funktion je Zelle; `undefined` heisst ohne Wert. */
const gelaende = (wert: (i: number, j: number) => number | undefined, max = 400, grund = 63): Gelaende => ({ c: 4, grund, zelle: wert, max });
const ZWEI: Projektion = zweiZuEins(16);
/** Genordet mit Neigung, wie north-45: a = b. */
const GENORDET: Projektion = { azimuth: 'north', u: 16, v: 8, y: 8 };
const SE: Blick = { p: ZWEI, k: 0 };
const OBEN: Blick = { p: { azimuth: 'north', u: 16, v: 16, y: 0 }, k: 0 };

/** Fläche gerade/ungerade über alle Ringe, abgetastet auf einem Gitter von `schritt` Pixeln. */
function flaeche(ringe: Punkt[][], schritt: number): number {
  const alle = ringe.flat();
  const [x0, x1] = [Math.min(...alle.map((p) => p[0])), Math.max(...alle.map((p) => p[0]))];
  const [y0, y1] = [Math.min(...alle.map((p) => p[1])), Math.max(...alle.map((p) => p[1]))];
  let treffer = 0;
  for (let y = y0 + schritt / 2; y < y1; y += schritt) {
    for (let x = x0 + schritt / 2; x < x1; x += schritt) {
      let drin = false;
      for (const r of ringe) {
        for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
          const [a, b] = [r[i]!, r[j]!];
          if (a[1] > y !== b[1] > y && x < ((b[0] - a[0]) * (y - a[1])) / (b[1] - a[1]) + a[0]) drin = !drin;
        }
      }
      if (drin) treffer++;
    }
  }
  return treffer * schritt * schritt;
}

/** Fläche eines Vielecks nach der Schnürsenkelformel. */
const schnuer = (r: Punkt[]) => Math.abs(r.reduce((s, a, i) => s + a[0] * r[(i + 1) % r.length]![1] - r[(i + 1) % r.length]![0] * a[1], 0)) / 2;

/** Ein Grat entlang z: der Wert jeder Zelle hängt nur an i. */
const grat = (steil: number) => (i: number) => steil * Math.abs(i - 5);
/** H unabhängig vom Code nachgerechnet: bilinear zwischen den Mitten, für Zellen ohne Lücken, + 1. */
const bilinear = (f: (i: number, j: number) => number) => (x: number, z: number) => {
  const [fx, fz] = [x / 4 - 0.5, z / 4 - 0.5];
  const [i, j] = [Math.floor(fx), Math.floor(fz)];
  const [tx, tz] = [fx - i, fz - j];
  return (f(i, j) * (1 - tx) + f(i + 1, j) * tx) * (1 - tz) + (f(i, j + 1) * (1 - tx) + f(i + 1, j + 1) * tx) * tz + 1;
};

test('abtasten: Anfang, Schnitte mit den Linien durch die Mitten der Zellen, je ein Punkt dazwischen, Ende', () => {
  expect(abtasten([[0, 0], [10, 0]], false, 4)).toEqual([[0, 0], [1, 0], [2, 0], [4, 0], [6, 0], [8, 0], [10, 0]]);
  const ring = abtasten([[0, 0], [4, 0], [4, 4]], true, 4);
  expect(ring[0]).toEqual([0, 0]);
  expect(ring.at(-1)).toEqual([0, 0]);
  expect(ring).toContainEqual([2, 2]);
});

test('vieleck: Seiten von höchstens einer Zelle, mindestens acht Ecken, alle auf dem Kreis', () => {
  for (const r of [0.5, 10, 2000]) {
    const ecken = vieleck([5, -7], r, 4);
    expect(ecken.length).toBeGreaterThanOrEqual(8);
    for (let i = 0; i < ecken.length; i++) {
      const [a, b] = [ecken[i]!, ecken[(i + 1) % ecken.length]!];
      expect(Math.hypot(a[0] - b[0], a[1] - b[1])).toBeLessThanOrEqual(4 + 1e-9);
      expect(Math.hypot(a[0] - 5, a[1] + 7)).toBeCloseTo(r, 9);
    }
  }
});

test('oberflaeche: bilinear zwischen den Mitten, eine Zelle ohne Wert aus den Nachbarn im Umkreis zwei, ohne Nachbarn grund', () => {
  const linear = gelaende((i, j) => 10 + 2 * i - j);
  expect(oberflaeche(linear, 2, 2)).toBeCloseTo(11, 9);
  expect(oberflaeche(linear, 4, 2)).toBeCloseTo(12, 9);
  expect(oberflaeche(linear, 7, 9)).toBeCloseTo(10 + 2 * (7 / 4 - 0.5) - (9 / 4 - 0.5) + 1, 9);
  const loch = gelaende((i, j) => (i === 0 && j === 0 ? undefined : 10 + i * i));
  // Der Mittelwert der 24 Nachbarn: 10 + Summe der i² über 5 × 5, durch 24.
  expect(oberflaeche(loch, 2, 2)).toBeCloseTo(10 + (5 * (4 + 1 + 0 + 1 + 4)) / 24 + 1, 9);
  expect(oberflaeche(gelaende(() => undefined), 2, 2)).toBe(64);
});

test('verdeckt: erst Gelände mehr als eine Zelle vor dem Punkt zählt, diagonal ab der nächsten Mitte, genordet ab der übernächsten; jede Richtung', () => {
  // Der Punkt in der Mitte der Zelle (10, 10), auf Höhe 1; eine einzelne hohe Zelle k Zellen davor.
  const [x, z] = [42, 42];
  const turm = (i: number, j: number) => gelaende((a, b) => (a === i && b === j ? 100 : 0), 101);
  for (let k = 0; k < 4; k++) {
    // Diagonal: die Richtung zur Kamera im Blick (1, 1), in der Welt (1, 1) zurückgedreht.
    const [wx, wz] = punktImBlick(1, 1, (4 - k) % 4);
    const blick: Blick = { p: ZWEI, k };
    expect(verdeckt(turm(10 + wx, 10 + wz), x, 1, z, blick), `diagonal k=${k}, eins davor`).toBe(true);
    expect(verdeckt(turm(10 - wx, 10 - wz), x, 1, z, blick), `diagonal k=${k}, dahinter`).toBe(false);
    const [nx, nz] = punktImBlick(0, 1, (4 - k) % 4);
    const genordet: Blick = { p: GENORDET, k };
    expect(verdeckt(turm(10 + nx, 10 + nz), x, 1, z, genordet), `genordet k=${k}, eins davor`).toBe(false);
    expect(verdeckt(turm(10 + 2 * nx, 10 + 2 * nz), x, 1, z, genordet), `genordet k=${k}, zwei davor`).toBe(true);
    expect(verdeckt(turm(10 - 2 * nx, 10 - 2 * nz), x, 1, z, genordet), `genordet k=${k}, dahinter`).toBe(false);
  }
  // Ein Hang zur Kamera verdeckt nur, wenn er steiler steigt als der Strahl (2:1: 1 Block je Block in x und z).
  const sanft = gelaende((i, j) => 1.5 * (i + j));
  const steil = gelaende((i, j) => 3 * (i + j));
  expect(verdeckt(sanft, 50, oberflaeche(sanft, 50, 50), 50, SE)).toBe(false);
  expect(verdeckt(steil, 50, oberflaeche(steil, 50, 50), 50, SE)).toBe(true);
  // Eine fehlende Region auf dem Strahl liegt auf grund: hoch verdeckt sie, tief nicht.
  // max bleibt 64: So läuft die Schleife auch bei tiefem grund, und erst die Höhe entscheidet.
  const fehlt = (grund: number) => gelaende((i) => (i >= 12 ? undefined : 0), 64, grund);
  expect(verdeckt(fehlt(63), 42, 1, 42, SE)).toBe(true);
  expect(verdeckt(fehlt(-10), 42, 1, 42, SE)).toBe(false);
  expect(verdeckt(turm(11, 11), x, 1, z, OBEN)).toBe(false);
});

test('sichtbareFelder: ein Durchgang mit laufendem Maximum gibt an jeder Mitte eines Felds dasselbe wie verdeckt, in jeder Richtung', () => {
  const g = gelaende((i, j) => Math.round(20 * Math.sin(i / 2.3) * Math.cos(j / 3.1) + 10 * Math.sin((i + j) / 1.7)), 31);
  const [pa, qa, nx, nz] = [-6, -4, 14, 11];
  for (const p of [ZWEI, GENORDET]) {
    for (let k = 0; k < 4; k++) {
      const blick: Blick = { p, k };
      const sicht = sichtbareFelder(g, blick, pa, qa, nx, nz, () => true);
      let verdeckte = 0;
      for (let i = 0; i < nx * nz; i++) {
        const [x, z] = [(pa + Math.floor(i / nz) + 1) * 4, (qa + (i % nz) + 1) * 4];
        const soll = verdeckt(g, x, oberflaeche(g, x, z), z, blick) ? 0 : 1;
        expect(sicht[i], `${p.azimuth} k=${k}, Mitte ${x},${z}`).toBe(soll);
        verdeckte += 1 - soll;
      }
      expect(verdeckte, `${p.azimuth} k=${k}: das Gelände verdeckt etwas`).toBeGreaterThan(5);
    }
  }
  // Ein schmaler Turm: eine Zelle von 10 Blöcken, deren Mitte zwischen zwei Mitten von Feldern liegt.
  const turm = gelaende((i, j) => (i === 5 && j === 5 ? 10 : 0), 11);
  expect(verdeckt(turm, 16, 1, 16, SE)).toBe(true);
  expect(sichtbareFelder(turm, SE, 3, 3, 1, 1, () => true)[0]).toBe(0);
});

test('netz: auf ebenem Grund so gross wie die Fläche mal der Projektion, mit Loch; von oben die Ringe selbst', () => {
  const eben = gelaende(() => 20, 21);
  const quadrat: Punkt[] = [[1, 1], [41, 1], [41, 41], [1, 41]];
  const loch: Punkt[] = [[11, 11], [21, 11], [21, 21], [11, 21]];
  const ringe = netz([{ aussen: quadrat, loecher: [loch] }], eben, SE);
  // 2:1, scale 16: ein Block bedeckt 2 · h · a = 2 · 8 · 4 Pixel.
  expect(flaeche(ringe, 1)).toBeCloseTo((1600 - 100) * 64, -3);
  const oben = netz([{ aussen: quadrat, loecher: [loch] }], eben, OBEN);
  expect(oben).toEqual([quadrat.map(([x, z]) => [x * 16, z * 16]), loch.map(([x, z]) => [x * 16, z * 16])]);
});

test('netz: Felder ganz drinnen ergeben einen Umriss, die Punkte wachsen mit dem Umfang, nicht mit der Fläche', () => {
  // Auf den Linien durch die Mitten der Zellen: 10 × 10 Felder ganz drinnen, ein Loch von 3 × 3.
  const ringe = netz([{ aussen: [[2, 2], [42, 2], [42, 42], [2, 42]], loecher: [[[10, 10], [22, 10], [22, 22], [10, 22]]] }], gelaende(() => 20, 21), SE);
  // Umriss 40 Ecken, Loch 12; jedes Feld für sich wären 91 · 4.
  expect(ringe.flat().length).toBe(52);
  expect(flaeche(ringe, 1)).toBeCloseTo((1600 - 144) * 64, -3);
});

test('netz: auf unebenem Grund so gross wie die Felder, jedes für sich projiziert; ein Umriss ohne die Ecken dazwischen wäre falsch', () => {
  // Ein Grat nur in der hinteren Hälfte; nach vorn fällt das Gelände, verdeckt ist nichts.
  const f = (i: number, j: number) => (j < 5 ? grat(2)(i) : 0);
  const H = bilinear(f);
  const g = gelaende(f, 21);
  const [x0, x1] = [2, 42];
  const ringe = netz([{ aussen: [[x0, x0], [x1, x0], [x1, x1], [x0, x1]], loecher: [] }], g, SE);
  let soll = 0;
  for (let x = x0; x < x1; x += 4) {
    for (let z = x0; z < x1; z += 4) {
      const ecken: Punkt[] = [[x, z], [x + 4, z], [x + 4, z + 4], [x, z + 4]];
      soll += schnuer(ecken.map(([a, b]) => projiziere(a, H(a, b), b, ZWEI)));
    }
  }
  expect(Math.abs(flaeche(ringe, 1) - soll) / soll).toBeLessThan(0.01);
  const vierEcken = ([[x0, x0], [x1, x0], [x1, x1], [x0, x1]] as Punkt[]).map(([a, b]) => projiziere(a, H(a, b), b, ZWEI));
  expect(Math.abs(schnuer(vierEcken) - soll) / soll).toBeGreaterThan(0.02);
});

test('netz: ein Feld, dessen Mitte verdeckt ist, fällt weg; vor dem Wall bleibt alles', () => {
  const wall = gelaende((i, j) => (i + j === 6 ? 120 : 0), 121);
  const flach = gelaende(() => 0, 1);
  const quadrat = (x: number, z: number, a: number): Punkt[] => [[x, z], [x + a, z], [x + a, z + a], [x, z + a]];
  const hinten = [{ aussen: quadrat(0, 0, 8), loecher: [] }];
  expect(flaeche(netz(hinten, flach, SE), 1)).toBeGreaterThan(1000);
  expect(netz(hinten, wall, SE)).toEqual([]);
  const vorn = [{ aussen: quadrat(24, 24, 8), loecher: [] }];
  expect(flaeche(netz(vorn, wall, SE), 1)).toBe(flaeche(netz(vorn, flach, SE), 1));
});

test('zug: im iso auf dem Gelände, jeder Punkt auf seiner Höhe auf einem Grat; verdeckt hinter der Mauer; von oben eben und nie verdeckt', () => {
  const f = (i: number) => grat(2)(i);
  const H = bilinear(f);
  const g = gelaende(f, 21);
  const punkte: Punkt[] = [[0, 4], [40, 4]];
  const z = zug(punkte, false, g, SE);
  const soll = abtasten(punkte, false, 4).map(([x, zz]) => projiziere(x, H(x, zz), zz, ZWEI));
  expect(z.punkte.length).toBe(soll.length);
  z.punkte.forEach((p, i) => {
    expect(p[0]).toBeCloseTo(soll[i]![0], 9);
    expect(p[1]).toBeCloseTo(soll[i]![1], 9);
  });
  const mauer = gelaende((i, j) => (i === 5 && j === 5 ? 100 : 0), 101);
  const iso = zug([[2, 2], [2, 40]], false, mauer, SE);
  expect(iso.verdeckt[0]).toBe(true);
  expect(iso.verdeckt.at(-1)).toBe(false);
  expect(zug([[0, 0], [10, 0]], false, mauer, OBEN)).toEqual({ punkte: [[0, 0], [160, 0]], verdeckt: [false, false] });
});

test('ringImRechteck: eine Fläche, beschnitten auf das Rechteck', () => {
  const ring = ringImRechteck([[-10, -10], [10, -10], [10, 10], [-10, 10]], [0, 0, 5, 5]);
  expect(rechteck(ring)).toEqual([0, 0, 5, 5]);
  expect(schnuer(ring)).toBe(25);
  expect(ringImRechteck([[20, 20], [30, 20], [30, 30]], [0, 0, 5, 5])).toEqual([]);
});

test('zugImRechteck: nur die Stücke im Rechteck, ein Ring ganz drinnen bleibt ganz', () => {
  expect(zugImRechteck([[-10, 0], [10, 0], [10, 20]], false, [0, -5, 5, 5])).toEqual([[[0, 0], [5, 0]]]);
  expect(zugImRechteck([[1, 1], [2, 1], [2, 2]], true, [0, 0, 5, 5])).toEqual([[[1, 1], [2, 1], [2, 2], [1, 1]]]);
  // Hinaus und wieder hinein: zwei Stücke.
  expect(zugImRechteck([[1, 1], [9, 1], [9, 2], [1, 2]], false, [0, 0, 5, 5])).toEqual([[[1, 1], [5, 1]], [[5, 2], [1, 2]]]);
});

test('schriftPfad: die Höhen gleitend über genau 32 Blöcke gemittelt, mittig um jeden Punkt; von oben eben', () => {
  const g = gelaende((i, j) => (i === 10 && j === 0 ? 100 : 0), 101);
  const pfad: Punkt[] = [[0, 2], [120, 2]];
  const ist = schriftPfad(pfad, g, SE);
  // Unabhängig nachgerechnet: dieselben Punkte, je der Mittelwert von H über alle Punkte höchstens 16 Blöcke entfernt.
  const punkte = abtasten(pfad, false, 4);
  const H = punkte.map(([x, z]) => oberflaeche(g, x, z));
  const soll = punkte.map(([x, z]) => {
    const nah = punkte.map((p, j) => [p, j] as const).filter(([p]) => Math.abs(p[0] - x) <= 16);
    return projiziere(x, nah.reduce((s, [, j]) => s + H[j]!, 0) / nah.length, z, ZWEI);
  });
  expect(ist.length).toBe(soll.length);
  ist.forEach((p, i) => expect(p[1]).toBeCloseTo(soll[i]![1], 9));
  expect(schriftPfad(pfad, g, OBEN)).toEqual([[0, 32], [1920, 32]]);
});
