import { expect, test } from '@playwright/test';
import { abtasten, netz, oberflaeche, schriftPfad, verdeckt, vieleck, zug, type Blick, type Gelaende, type Punkt } from '../src/gelaende';
import { projiziere, zweiZuEins } from '../src/pick';

/** Ein Gelände aus einer Funktion je Zelle; `undefined` heisst ohne Wert. */
const gelaende = (wert: (i: number, j: number) => number | undefined, max = 400): Gelaende => ({ c: 4, grund: 63, zelle: wert, max });
const SE: Blick = { p: zweiZuEins(16), k: 0 };
const SW: Blick = { p: zweiZuEins(16), k: 1 };
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

test('abtasten: Anfang, Schnitte mit den Linien durch die Mitten der Zellen, je ein Punkt dazwischen, Ende', () => {
  expect(abtasten([[0, 0], [10, 0]], false, 4)).toEqual([[0, 0], [1, 0], [2, 0], [4, 0], [6, 0], [8, 0], [10, 0]]);
  // Geschlossen läuft der Zug zurück zum Anfang.
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
  // In der Mitte der Zelle (i, j) bei ((i + 0,5)·4, (j + 0,5)·4) genau ihr Wert + 1, dazwischen linear.
  expect(oberflaeche(linear, 2, 2)).toBeCloseTo(11, 9);
  expect(oberflaeche(linear, 4, 2)).toBeCloseTo(12, 9);
  expect(oberflaeche(linear, 7, 9)).toBeCloseTo(10 + 2 * (7 / 4 - 0.5) - (9 / 4 - 0.5) + 1, 9);
  const loch = gelaende((i, j) => (i === 0 && j === 0 ? undefined : 10 + i * i));
  // Der Mittelwert der 24 Nachbarn: 10 + Summe der i² über 5 × 5, durch 24.
  const soll = 10 + (5 * (4 + 1 + 0 + 1 + 4)) / 24;
  expect(oberflaeche(loch, 2, 2)).toBeCloseTo(soll + 1, 9);
  expect(oberflaeche(gelaende(() => undefined), 2, 2)).toBe(64);
});

test('verdeckt: Gelände mehr als eine Zelle vor dem Punkt und über dem Strahl verdeckt ihn, dahinter nicht; von oben nie', () => {
  // Eine Mauer bei i = j = 5 (vor dem Punkt aus se: x und z grösser), sonst eben auf 0.
  const mauer = gelaende((i, j) => (i === 5 && j === 5 ? 100 : 0), 101);
  expect(verdeckt(mauer, 2, 1, 2, SE)).toBe(true);
  // Hinter dem Punkt verdeckt sie nicht.
  expect(verdeckt(mauer, 30, 1, 30, SE)).toBe(false);
  // Aus sw liegt „vor“ in der Welt bei kleinerem x und grösserem z.
  expect(verdeckt(mauer, 2, 1, 2, SW)).toBe(false);
  expect(verdeckt(mauer, 30, 1, 14, SW)).toBe(true);
  // Ein Hang zur Kamera verdeckt nur, wenn er steiler steigt als der Strahl (2:1: 1 Block je Block in x und z).
  const sanft = gelaende((i, j) => 1.5 * (i + j), 400);
  const steil = gelaende((i, j) => 3 * (i + j), 400);
  expect(verdeckt(sanft, 50, oberflaeche(sanft, 50, 50), 50, SE)).toBe(false);
  expect(verdeckt(steil, 50, oberflaeche(steil, 50, 50), 50, SE)).toBe(true);
  expect(verdeckt(mauer, 2, 1, 2, OBEN)).toBe(false);
});

test('netz: auf ebenem Gelände so gross wie die Fläche mal der Projektion, mit Loch, wenige Ringe; von oben die Ringe selbst', () => {
  const eben = gelaende(() => 20, 21);
  const quadrat: Punkt[] = [[1, 1], [41, 1], [41, 41], [1, 41]];
  const loch: Punkt[] = [[11, 11], [21, 11], [21, 21], [11, 21]];
  const ringe = netz([{ aussen: quadrat, loecher: [loch] }], eben, SE, 0.25);
  // 2:1, scale 16: ein Block bedeckt 2 · h · a = 2 · 8 · 4 Pixel.
  expect(flaeche(ringe, 1)).toBeCloseTo((1600 - 100) * 64, -3);
  // Die Felder ganz drinnen fassen sich je Reihe zusammen und werden auf ebenem Grund zu Vierecken.
  expect(ringe.length).toBeLessThan(120);
  expect(ringe.filter((r) => r.length === 4).length).toBeGreaterThan(5);
  const oben = netz([{ aussen: quadrat, loecher: [loch] }], eben, OBEN, 0.25);
  expect(oben).toEqual([quadrat.map(([x, z]) => [x * 16, z * 16]), loch.map(([x, z]) => [x * 16, z * 16])]);
});

test('netz: ein Feld, dessen Mitte verdeckt ist, fällt weg; vor dem Wall bleibt alles', () => {
  // Ein hoher Wall quer über die Welt, wo i + j = 6; aus se liegt vorn, wo x + z gross ist.
  const wall = gelaende((i, j) => (i + j === 6 ? 120 : 0), 121);
  const flach = gelaende(() => 0, 1);
  const quadrat = (x: number, z: number, a: number): Punkt[] => [[x, z], [x + a, z], [x + a, z + a], [x, z + a]];
  // Ganz hinter dem Wall (x + z bis 16): verdeckt.
  const hinten = [{ aussen: quadrat(0, 0, 8), loecher: [] }];
  expect(flaeche(netz(hinten, flach, SE, 0.25), 1)).toBeGreaterThan(1000);
  expect(netz(hinten, wall, SE, 0.25)).toEqual([]);
  // Davor (x + z ab 48), wo der Wall die Höhen nicht mehr berührt: wie auf flachem Grund.
  const vorn = [{ aussen: quadrat(24, 24, 8), loecher: [] }];
  expect(flaeche(netz(vorn, wall, SE, 0.25), 1)).toBe(flaeche(netz(vorn, flach, SE, 0.25), 1));
});

test('zug: von oben eben und nie verdeckt; im iso auf dem Gelände abgetastet, verdeckt hinter der Mauer', () => {
  const mauer = gelaende((i, j) => (i === 5 && j === 5 ? 100 : 0), 101);
  const oben = zug([[0, 0], [10, 0]], false, mauer, OBEN);
  expect(oben).toEqual({ punkte: [[0, 0], [160, 0]], verdeckt: [false, false] });
  const iso = zug([[2, 2], [2, 40]], false, mauer, SE);
  expect(iso.punkte.length).toBeGreaterThan(10);
  expect(iso.verdeckt[0]).toBe(true);
  expect(iso.verdeckt.at(-1)).toBe(false);
});

test('schriftPfad: die Höhen gleitend über 32 Blöcke gemittelt; eine Spitze hebt die Schrift nur wenig, 40 Blöcke weiter nichts', () => {
  const spitze = gelaende((i, j) => (i === 10 && j === 0 ? 100 : 0), 101);
  const flach = gelaende(() => 0, 1);
  const pfad: Punkt[] = [[0, 2], [120, 2]];
  const [mit, ohne] = [schriftPfad(pfad, spitze, SE), schriftPfad(pfad, flach, SE)];
  expect(mit.length).toBe(ohne.length);
  const i = ohne.findIndex(([x]) => x === projiziere(42, 0, 2, zweiZuEins(16))[0]);
  const fern = ohne.findIndex(([x]) => x === projiziere(90, 0, 2, zweiZuEins(16))[0]);
  expect(i).toBeGreaterThan(0);
  // Am Gipfel stünde die Oberfläche 100 Blöcke höher, also 800 Pixel; gemittelt nur ein Bruchteil davon.
  const hub = ohne[i]![1] - mit[i]![1];
  expect(hub).toBeGreaterThan(0);
  expect(hub).toBeLessThan(800 / 4);
  expect(mit[fern]).toEqual(ohne[fern]);
  // Das Mittel liegt mittig um jeden Punkt: 8 Blöcke vor und hinter der Spitze hebt es gleich.
  const bei = (x: number) => ohne.findIndex(([px]) => px === projiziere(x, 0, 2, zweiZuEins(16))[0]);
  const [vor, nach] = [bei(34), bei(50)];
  expect(ohne[vor]![1] - mit[vor]![1]).toBeGreaterThan(0);
  expect(ohne[vor]![1] - mit[vor]![1]).toBeCloseTo(ohne[nach]![1] - mit[nach]![1], 6);
  // Von oben eben, ohne Abtasten.
  expect(schriftPfad(pfad, spitze, OBEN)).toEqual([[0, 32], [1920, 32]]);
});
