import { expect, test } from '@playwright/test';
import { adern, KACHEL, marmorKachel, type MitTextur, rechne, rgb, rgba, STOFFE, texel, type Texel } from '../stoffe';
import { lichtUndBlick } from '../tablett';

/** Summe der Kanäle einer Farbe als RGBA. */
const hell = (farbe: number) => (farbe & 255) + ((farbe >> 8) & 255) + ((farbe >> 16) & 255);

test('die Kachel Marmor hat keine Naht', () => {
  const n = KACHEL;
  const pixel = new Uint32Array(marmorKachel(n, 0.625).buffer);
  // Mittlerer Sprung der Helligkeit zwischen zwei Spalten oder Zeilen.
  const sprung = (stelle: (k: number, i: number) => number, i: number, j: number) => {
    let summe = 0;
    for (let k = 0; k < n; k++) summe += Math.abs(hell(pixel[stelle(k, i)]!) - hell(pixel[stelle(k, j)]!));
    return summe / n;
  };
  const spalte = (k: number, i: number) => k * n + i;
  const zeile = (k: number, i: number) => i * n + k;
  // Über die Naht nicht mehr als zwischen Nachbarn im Innern.
  for (const stelle of [spalte, zeile]) {
    const innen = [n / 4, n / 2, (3 * n) / 4].map((i) => sprung(stelle, i - 1, i));
    expect(sprung(stelle, n - 1, 0)).toBeLessThan(1.5 * Math.max(...innen));
  }
});

test('die Adern im Marmor sind fein und selten, für dieselbe Grösse immer dieselben', () => {
  for (const [breite, hoehe, streckung] of [
    [1280, 800, 0.625],
    [1920, 1080, 0.625],
    [1491, 1055, 0.5],
  ] as const) {
    const alle = adern(breite, hoehe, streckung);
    expect(adern(breite, hoehe, streckung)).toEqual(alle);
    let flaeche = 0;
    for (const ader of alle) {
      for (let i = 1; i < ader.length; i++) {
        const [p, q] = [ader[i - 1]!, ader[i]!];
        flaeche += (Math.hypot(q.x - p.x, q.y - p.y) * (p.breite + q.breite)) / 2;
      }
    }
    const name = `${breite} × ${hoehe}`;
    expect(Math.max(...alle.flat().map((p) => p.breite)), name).toBeLessThanOrEqual(2);
    expect(flaeche / (breite * hoehe), name).toBeLessThan(0.02);
    expect(alle.length, name).toBeGreaterThanOrEqual(2);
  }
});

test('eine Fläche mit Textur hat nur Farben aus den Rampen ihrer Stoffe, ohne Glätten', () => {
  const licht = lichtUndBlick({ azimuth: 'diagonal', u: 8, v: 5, y: 8 });
  // Ein Fries von 120 × 30 Pixeln, schräg wie im Bild, mit Feldern und
  // Messingnägeln an ihren Stössen.
  const fries: MitTextur = {
    o: [0, 0],
    a: [120, 40],
    b: [0, 30],
    n: [0, 0, 1],
    textur: { rolle: 'fries', a3: [1, 0, 0], b3: [0, -1, 0], la: 10, lb: 2.68 },
  };
  const { stellen, farben } = rechne(fries, 1, [0, 0], licht, [120, 70]);
  expect(stellen.length).toBeGreaterThan(3000);
  const rampe = (stoff: keyof typeof STOFFE) => new Set(STOFFE[stoff].rampe.map((hex) => rgba(rgb(hex))));
  const [holz, messing] = [rampe('wandholz'), rampe('messing')];
  expect([...farben].filter((f) => !holz.has(f) && !messing.has(f))).toEqual([]);
  expect([...farben].some((f) => messing.has(f))).toBe(true);
});

test('die Maserung läuft längs der Fläche, in ruhigen Linien', () => {
  const t: Texel = { m: 0, stoff: 'oberholz' };
  const m = (u: number, v: number) => texel('schraege', u, v, 100, 4, 0, t).m;
  let [laengs, quer] = [0, 0];
  for (let i = 0; i < 4000; i++) {
    const [u, v] = [(i % 100) * 0.97, Math.floor(i / 100) * 0.1];
    laengs += Math.abs(m(u + 0.05, v) - m(u, v));
    quer += Math.abs(m(u, v + 0.05) - m(u, v));
  }
  expect(laengs).toBeLessThan(quer / 5);
});
