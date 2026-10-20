import { expect, test } from '@playwright/test';
import {
  abtasten,
  abtastenImRahmen,
  kreisSeiten,
  glaetten,
  kreisRing,
  netz,
  oberflaeche,
  punktImBlick,
  rechteck,
  ringImRechteck,
  schriftPfad,
  sichtbareFelder,
  verdeckt,
  vieleck,
  wand,
  zug,
  zugImRechteck,
  type Blick,
  type Gelaende,
  type Punkt,
  type Schirm,
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

/** Ein Gelände mit Hügeln quer zum Weg: bilinear in Zellen von 4, Höhen 40 bis 100. */
const huegel = gelaende((i, j) => 70 + 25 * Math.sin(i / 2.3) + 5 * Math.cos(j / 1.7));
/** Ein Schirm auf Zoom `z` unter der feinsten Stufe, mit einem grossen Rahmen und ε = 0,5 px. */
const schirm = (z: number, rahmen: [number, number, number, number] = [-1e6, -1e6, 1e6, 1e6]): Schirm => ({ faktor: 2 ** z, rahmen, eps: 0.5 });
/** Der grösste Abstand der Punkte `dicht` vom Linienzug `zug`. */
function abweichung(dicht: Punkt[], zug: Punkt[]): number {
  let max = 0;
  for (const p of dicht) {
    let best = Infinity;
    for (let i = 1; i < zug.length; i++) {
      const [a, b] = [zug[i - 1]!, zug[i]!];
      const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
      const l2 = dx * dx + dy * dy;
      const t = l2 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
      best = Math.min(best, Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy));
    }
    max = Math.max(max, best);
  }
  return max;
}
/** Die Länge eines Zugs. */
const laenge = (zug: Punkt[]) => zug.slice(1).reduce((summe, p, i) => summe + Math.hypot(p[0] - zug[i]![0], p[1] - zug[i]![1]), 0);

test('kreisSeiten: die Pfeilhöhe jeder Seite unter ε, mit den wenigsten Seiten, die das schaffen, mindestens 12', () => {
  for (const r of [0.3, 5, 50, 500, 5000, 50_000]) {
    const n = kreisSeiten(r, 0.5);
    expect(n).toBeGreaterThanOrEqual(12);
    expect(r * (1 - Math.cos(Math.PI / n))).toBeLessThanOrEqual(0.5);
    if (n > 12) expect(r * (1 - Math.cos(Math.PI / (n - 1)))).toBeGreaterThan(0.5);
  }
});

/** Ein Zug auf dem Schirm, mal `faktor`. */
const mal = (punkte: Punkt[], s: Schirm): Punkt[] => punkte.map(([a, b]) => [a * s.faktor, b * s.faktor]);

test('zug mit Schirm: auf dem Schirm höchstens ε vom Zug ohne Schirm, geglättet und an den Zellen; auf jedem Zoom; jeder Wechsel von verdeckt bleibt', () => {
  const weg: Punkt[] = [[3, 7], [383, 161], [203, 400]];
  const genau = zug(weg, false, huegel, SE);
  for (const z of [0, -1, -2, -3, -4, -5]) {
    const s = schirm(z);
    const grob = zug(weg, false, huegel, SE, s);
    expect(abweichung(mal(genau.punkte, s), mal(grob.punkte, s)), `Zoom ${z}`).toBeLessThanOrEqual(0.5 + 1e-9);
    // Jeder Wechsel bleibt an derselben Stelle: beide Punkte um ihn, nebeneinander.
    const wechsel = (zz: typeof genau) => zz.punkte.flatMap((p, i) => (i > 0 && zz.verdeckt[i] !== zz.verdeckt[i - 1] ? [[zz.punkte[i - 1], p]] : []));
    expect(wechsel(grob), `Zoom ${z}`).toEqual(wechsel(genau));
  }
});

test('zug mit Schirm: Herauszoomen gibt nie mehr Punkte; je Zug höchstens 2 · Länge in px / ε und die Ecken', () => {
  const weg: Punkt[] = [[3, 7], [383, 161], [203, 400], [3, 7]];
  let vorher = Infinity;
  for (const z of [0, -1, -2, -3, -4, -5, -6]) {
    const s = schirm(z);
    const punkte = zug(weg, false, huegel, SE, s).punkte;
    expect(punkte.length, `Zoom ${z}`).toBeLessThanOrEqual(vorher);
    vorher = punkte.length;
    expect(punkte.length, `Zoom ${z}`).toBeLessThanOrEqual((2 * laenge(mal(punkte, s))) / s.eps + weg.length);
  }
});

test('abtastenImRahmen: Strecken ausserhalb des Rahmens geben nur ihre Ecken, auf jedem Zoom; innen wie abtasten', () => {
  const draussen: Punkt[] = [[5000, 5000], [9000, 5000], [9000, 9000]];
  for (const z of [0, -3, -6]) expect(abtastenImRahmen(draussen, false, huegel, SE, schirm(z, [0, 0, 2000, 2000]))).toEqual(draussen);
  const innen: Punkt[] = [[3, 7], [83, 61]];
  expect(abtastenImRahmen(innen, false, huegel, SE, schirm(-2))).toEqual(abtasten(innen, false, 4, Infinity));
});

test('abtasten: Ein Zug über 512 Zellen schneidet an Linien im Abstand Länge / 512, so bleiben es höchstens etwa 4 · 512 Proben, gleich wie lang', () => {
  for (const laenge of [4000, 40_000, 400_000]) {
    const proben = abtasten([[0, 0], [laenge, laenge / 3]], false, 4);
    expect(proben.length).toBeGreaterThan(512);
    expect(proben.length).toBeLessThanOrEqual(4 * 512 + 2);
  }
  // Kürzer als 512 Zellen: unverändert an den Zellen, 25 Linien bei 2, 6, …, 98 geben 26 Stücke zu je 2 Punkten und das Ende.
  expect(abtasten([[0, 0], [100, 0]], false, 4)).toHaveLength(2 * 26 + 1);
});

test('kreisRing: mit Gelände Seiten von einer Zelle, höchstens 512; ohne Gelände ein 128-Eck, ein kleiner Kreis wie mit', () => {
  expect(kreisRing([0, 0], 10, 4, false)).toHaveLength(16);
  expect(kreisRing([0, 0], 2000, 4, false)).toHaveLength(512);
  expect(kreisRing([0, 0], 200_000, 4, false)).toHaveLength(512);
  expect(kreisRing([0, 0], 2000, 4, true)).toHaveLength(128);
  expect(kreisRing([0, 0], 10, 4, true)).toHaveLength(16);
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

test('zug: im iso auf dem Gelände, auf einer gleichmässigen Steigung jeder Punkt im Inneren auf seiner Höhe; verdeckt hinter der Mauer; von oben eben und nie verdeckt', () => {
  // Eine Steigung ändert das Glätten nicht, wo die Punkte gleich dicht liegen: abseits der Enden.
  const f = (i: number) => 2 * i;
  const H = bilinear(f);
  const g = gelaende(f, 101);
  const punkte: Punkt[] = [[0, 4], [60, 4]];
  const z = zug(punkte, false, g, SE);
  const abgetastet = abtasten(punkte, false, 4);
  const soll = abgetastet.map(([x, zz]) => projiziere(x, H(x, zz), zz, ZWEI));
  expect(z.punkte.length).toBe(soll.length);
  z.punkte.forEach((p, i) => {
    if (abgetastet[i]![0] < 14 || abgetastet[i]![0] > 46) return;
    expect(p[0]).toBeCloseTo(soll[i]![0], 9);
    expect(p[1]).toBeCloseTo(soll[i]![1], 9);
  });
  const mauer = gelaende((i, j) => (i === 5 && j === 5 ? 100 : 0), 101);
  const iso = zug([[2, 2], [2, 40]], false, mauer, SE);
  expect(iso.verdeckt[0]).toBe(true);
  expect(iso.verdeckt.at(-1)).toBe(false);
  expect(zug([[0, 0], [10, 0]], false, mauer, OBEN)).toEqual({ punkte: [[0, 0], [160, 0]], verdeckt: [false, false], gestrichelt: [false, false] });
});

test('zug: ein Haus verdeckt die Linie nur kurz, sie bleibt voll; ein langer Kamm verdeckt sie, sie wird gestrichelt', () => {
  // Ein Haus von einer Zelle, 4 Blöcke, 6 hoch, vor der Linie bei z 2; aus se liegt vorn, wo z gross ist.
  const haus = gelaende((i, j) => (i === 5 && j === 1 ? 6 : 0), 7);
  const z = zug([[0, 2], [40, 2]], false, haus, SE);
  expect(z.verdeckt.some((v) => v)).toBe(true);
  expect(z.gestrichelt.some((v) => v)).toBe(false);
  // Ein Kamm über die ganze Länge, 100 hoch: alles verdeckt, alles gestrichelt.
  const kamm = gelaende((_, j) => (j === 1 ? 100 : 0), 101);
  const k = zug([[0, 2], [40, 2]], false, kamm, SE);
  expect(k.gestrichelt.slice(1).every((v) => v)).toBe(true);
});

test('wand: Bänder übereinander, alle Ringe gleich herum, kein Loch, wo sich Vorder- und Rückseite decken; verdeckte Strecken fallen weg', () => {
  /** Umlaufzahl aller Ringe um (x, y); nonzero füllt, wo sie nicht 0 ist. */
  const umlauf = (ringe: Punkt[][], x: number, y: number) => {
    let w = 0;
    for (const r of ringe) {
      for (let i = 0; i < r.length; i++) {
        const [a, b] = [r[i]!, r[(i + 1) % r.length]!];
        const links = (b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1]);
        if (a[1] <= y && b[1] > y && links > 0) w++;
        if (a[1] > y && b[1] <= y && links < 0) w--;
      }
    }
    return w;
  };
  const vorzeichen = (r: Punkt[]) => Math.sign(r.reduce((s, a, i) => s + a[0] * r[(i + 1) % r.length]![1] - r[(i + 1) % r.length]![0] * a[1], 0));
  // Hin und auf derselben Strecke zurück, wie Vorder- und Rückseite einer Region im Bild: zwei Läufe.
  const hinUndHer = wand({ punkte: [[0, 100], [40, 120], [80, 100], [40, 120], [0, 100]], verdeckt: [false, false, false, false, false] }, 30, 3);
  expect(hinUndHer.map((b) => b.length)).toEqual([2, 2, 2]);
  const alle = hinUndHer.flat();
  expect(new Set(alle.map(vorzeichen))).toEqual(new Set([vorzeichen(alle[0]!)]));
  // Mitten in jedem Band, über beiden Läufen: gefüllt, nicht ausgespart.
  for (let k = 0; k < 3; k++) expect(Math.abs(umlauf(hinUndHer[k]!, 40, 120 - 10 * k - 5))).toBe(2);
  // Band k reicht von 10 · k bis 10 · (k + 1) Pixel über dem Rand.
  expect(Math.max(...hinUndHer[1]!.flat().map((p) => p[1]))).toBe(120 - 10);
  expect(Math.min(...hinUndHer[1]!.flat().map((p) => p[1]))).toBe(100 - 20);
  // Die Strecken zu den verdeckten Punkten fallen weg; vor und nach ihnen je ein Lauf.
  const mitLuecke = wand({ punkte: [[0, 0], [10, 0], [20, 0], [30, 0], [40, 0]], verdeckt: [false, false, true, false, false] }, 6, 1);
  expect(mitLuecke[0]!.map((r) => [...new Set(r.map((p) => p[0]))].sort((a, b) => a - b))).toEqual([[0, 10], [20, 30, 40]]);
  // Ganz verdeckt: keine Wand.
  expect(wand({ punkte: [[0, 0], [10, 0]], verdeckt: [true, true] }, 6, 2)).toEqual([[], []]);
});

test('glaetten: eine Steigung bleibt, ein Grat bis 8 Blöcke breit fällt heraus, ein Plateau von 20 bleibt in der Mitte; ein Ring läuft über den Anfang', () => {
  // Alle 0,5 Blöcke ein Punkt, wie abtasten bei Zellen von 1.
  const zug = (n: number): Punkt[] => Array.from({ length: n + 1 }, (_, k): Punkt => [k / 2, 0]);
  const punkte = zug(160);
  const im = (werte: number[], von: number, bis: number) => werte.filter((_, k) => k / 2 >= von && k / 2 <= bis);
  // Steigung: im Inneren unverändert.
  const steigung = punkte.map(([x]) => 3 * x);
  im(glaetten(punkte, steigung), 12, 68).forEach((h, k) => expect(h).toBeCloseTo(3 * (12 + k / 2), 9));
  // Ein Haus von 6 Blöcken bei x 40: weg, die Linie bleibt auf 10.
  const haus = punkte.map(([x]) => (x >= 37 && x < 43 ? 17 : 10));
  for (const h of im(glaetten(punkte, haus), 30, 50)) expect(h).toBeCloseTo(10, 9);
  // Ein Plateau von 20 Blöcken: in seiner Mitte bleibt seine Höhe.
  const plateau = punkte.map(([x]) => (x >= 30 && x < 50 ? 20 : 10));
  for (const h of im(glaetten(punkte, plateau), 38, 42)) expect(h).toBeCloseTo(20, 9);
  // Ein offener Zug: am Ende ein Fenster der Breite 0, der Wert bleibt.
  expect(glaetten(punkte, haus.map((h, k) => (k === 160 ? 99 : h))).at(-1)).toBe(99);
  // Ein Ring: erster und letzter Punkt gleich; ein Haus über dem Anfang fällt auch heraus.
  const ring: Punkt[] = Array.from({ length: 161 }, (_, k): Punkt => [20 * Math.cos((k / 160) * 2 * Math.PI), 20 * Math.sin((k / 160) * 2 * Math.PI)]);
  // Genau geschlossen, wie abtasten einen Ring schliesst; sin(2π) wäre nicht genau 0.
  ring[160] = ring[0]!;
  const amAnfang = ring.map((_, k) => (k < 4 || k > 156 ? 17 : 10));
  const glatt = glaetten(ring, amAnfang);
  for (const h of glatt) expect(h).toBeCloseTo(10, 9);
  expect(glatt.at(-1)).toBe(glatt[0]);
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
