import { expect, test } from '@playwright/test';
import type { Rechteck } from 'heroic-map-renderer/skin-api';
import { readFileSync } from 'node:fs';
import { kamera, projiziere, RICHTUNGEN } from '../../../tests/kamera';
import { type Brett, type Brettbild, kameraName, lage } from '../brett';
import { type Blick, gesamtmitte, gesamtstufe, grenzen } from '../tablett';

type Punkt = [number, number];

/** Die Kameras, die werkzeug/brett.py rendert, je mit einem gültigen scale. */
const KAMERAS = (
  [
    ['2:1', 16],
    ['16:9', 32],
    ['8:5', 16],
    ['4:3', 16],
    ['1:1', 16],
    ['top', 16],
    ['top-north', 16],
    ['north-45', 16],
  ] as const
).map(([name, scale]) => ({ name, p: kamera(name, scale) }));

const blick = (p: Blick['projektion'], k: number): Blick => ({ projektion: p, k, projiziere: (x, y, z) => projiziere(x, y, z, p) });

/**
 * brett.json des Platzhalters aller 32 Blicke, ohne die Bilder: Grösse und
 * Mitte hängen nur an der Kamera, nicht an der Szene.
 */
const ALLE = JSON.parse(readFileSync(new URL('fixtures/brett-alle.json', import.meta.url), 'utf8')) as Brett;

/** Fenster von Telefonen hochkant (9:20) bis zu Monitoren in 21:9 (2560 × 1080, 3440 × 1440) und 4K. */
const FENSTER = [
  [360, 800],
  [390, 844],
  [412, 915],
  [768, 1024],
  [1024, 768],
  [1280, 720],
  [1366, 768],
  [1440, 900],
  [1536, 864],
  [1920, 1080],
  [2560, 1080],
  [2560, 1440],
  [3440, 1440],
  [3840, 2160],
] as const;

const DPR = [1, 1.25, 1.5, 2, 3];

/**
 * Der Bildpunkt eines Punkts im Blick in Pixeln des gerenderten Bilds, ab
 * der Mitte der Karte, in Kanten: wie `bild` in werkzeug/brett.py, aus dem
 * Namen der Kamera. Schräg bleiben die Wände so hoch wie u, von oben
 * verschwinden sie.
 */
function imBild(name: string, u: number, [x, y, z]: [number, number, number]): Punkt {
  if (name === 'top-north' || name === 'north-45') return [x * u, z * u - y * (name === 'north-45' ? u : 0)];
  const [w, h] = name === 'top' ? [1, 1] : name.split(':').map(Number);
  return [(x - z) * u, (x + z) * ((u * h!) / w!) - y * (name === 'top' ? 0 : u)];
}

test('der Name einer Kamera ist der von --camera und --direction, wie in brett.json', () => {
  for (const { name, p } of KAMERAS) {
    for (let k = 0; k < 4; k++) expect(kameraName(p, k)).toBe(`${name} ${RICHTUNGEN[p.azimuth][k]}`);
  }
  // Gekürzt wie beim Renderer, und eine Kamera ohne Bilder bekommt trotzdem einen Namen.
  expect(kameraName(kamera('3:2', 48), 2)).toBe('3:2 nw');
});

test('ein gerendertes Bild liegt auf der Karte: Ecken, Mitte und ein Punkt darüber treffen ihren Bildpunkt', () => {
  // Eine Welt, deren Mitte nicht im Ursprung liegt, und ein Bild mit beliebiger Mitte.
  const area: Rechteck = [-96, 32, 160, 288];
  const meer = 62;
  for (const { name, p } of KAMERAS) {
    for (let k = 0; k < 4; k++) {
      for (const u of [322.075, 455.48]) {
        const bild: Brettbild = { fern: '', nah: '', groesse: [2000, 1500], mitte: [1003, 761], u };
        const b = blick(p, k);
        const { links, oben, mass } = lage(area, meer, b, bild);
        // Die Punkte in Weltkoordinaten; im Blick dreht sich der Ort, nicht das Bild.
        const [x0, z0, x1, z1] = area;
        const kante = x1 - x0;
        const [cx, cz] = [(x0 + x1) / 2, (z0 + z1) / 2];
        for (const [x, y, z] of [
          [x0, 0, z0],
          [x1, 0, z0],
          [x0, 0, z1],
          [x1, 0, z1],
          [cx, 0, cz],
          [x1, 0.3 * kante, z1],
        ] as [number, number, number][]) {
          let [bx, bz] = [x - cx, z - cz];
          for (let i = 0; i < k; i++) [bx, bz] = [bz, -bx];
          const [ix, iy] = imBild(name, u, [bx / kante, y / kante, bz / kante]);
          const [sx, sy] = [links + (bild.mitte[0] + ix) * mass, oben + (bild.mitte[1] + iy) * mass];
          let [wx, wz] = [x, z];
          for (let i = 0; i < k; i++) [wx, wz] = [wz, -wx];
          const [ex, ey] = b.projiziere(wx, y + meer, wz);
          expect(Math.abs(sx - ex), `${name} ${k} ${u}: (${x}, ${y}, ${z})`).toBeLessThan(1e-6);
          expect(Math.abs(sy - ey), `${name} ${k} ${u}: (${x}, ${y}, ${z})`).toBeLessThan(1e-6);
        }
      }
    }
  }
});

test('jedes Bild deckt die Gesamtansicht in Fenstern von 9:20 bis 21:9, in jeder Kamera und Richtung', () => {
  const area: Rechteck = [-512, -512, 512, 512];
  const [meer, maxZoom] = [62, 6];
  for (const { p } of KAMERAS) {
    for (let k = 0; k < 4; k++) {
      const name = kameraName(p, k);
      const bild = ALLE[name]!;
      const b = blick(p, k);
      const { links, oben, mass } = lage(area, meer, b, bild);
      const [rechts, unten] = [links + bild.groesse[0] * mass, oben + bild.groesse[1] * mass];
      for (const [breite, hoehe] of FENSTER) {
        for (const dpr of DPR) {
          const f = 2 ** (maxZoom - gesamtstufe(grenzen(area, meer, b), maxZoom, breite, hoehe, mass * dpr));
          const [mx, my] = gesamtmitte(area, meer, b, breite * f, hoehe * f);
          const was = `${name}, ${breite} × ${hoehe}, devicePixelRatio ${dpr}`;
          expect(mx - (breite * f) / 2, was).toBeGreaterThanOrEqual(links);
          expect(my - (hoehe * f) / 2, was).toBeGreaterThanOrEqual(oben);
          expect(mx + (breite * f) / 2, was).toBeLessThanOrEqual(rechts);
          expect(my + (hoehe * f) / 2, was).toBeLessThanOrEqual(unten);
        }
      }
    }
  }
});

test('unter 71 % nimmt die Gesamtansicht kein ganzes n, auch nicht auf der ganzen Stufe, auf die Leaflet einpasst', () => {
  // Der Rahmen füllt auf Stufe 6 das 2^0,6-fache des Fensters, auf Stufe 5
  // also 66 %. Dort deckt ein Pixel des Bilds genau 3 Pixel; jedes andere n
  // liegt auf keiner erlaubten Stufe.
  const breite = 100 * 2 ** -0.4;
  expect(gesamtstufe([0, 0, 100, 100], 6, breite, breite, 6)).toBe(5.5);
  expect(gesamtstufe([0, 0, 100, 100], 6, breite, breite)).toBe(5.5);
});

test('die Gesamtansicht nimmt ein ganzes n, wo eine erlaubte Stufe zwischen 71 und 100 % eines hat, die nächste an 92,5 %; sonst die Stufe nach 0067', () => {
  const zaehler = { ganz: 0, krumm: 0 };
  for (const name of ['8:5', '2:1'] as const) {
    const p = kamera(name, 32);
    const b = blick(p, 0);
    for (const kante of [128, 1024, 4096, 25600]) {
      const area: Rechteck = [-kante / 2, -kante / 2, kante / 2, kante / 2];
      const maxZoom = Math.ceil(Math.log2((2 * kante * p.u) / 256));
      const rahmen = grenzen(area, 62, b);
      const { mass } = lage(area, 62, b, ALLE[`${name} se`]!);
      for (const [breite, hoehe] of FENSTER) {
        for (const dpr of DPR) {
          const kunst = mass * dpr;
          const fit = gesamtstufe(rahmen, maxZoom, breite, hoehe, kunst);
          // Die Regel ausgeschrieben: Stufen der ganzen n, die Leaflet die
          // Kacheln nur verkleinern lassen, nicht unter der ganzen Stufe, auf
          // die es einpasst, 71 bis 100 % gefüllt.
          const voll = maxZoom + Math.log2(Math.min(breite / (rahmen[2] - rahmen[0]), hoehe / (rahmen[3] - rahmen[1])));
          const ganz = Math.floor(Math.round(voll * 100) / 100);
          const ziel = voll + Math.log2(0.925);
          const stufen: number[] = [];
          for (let n = 1; n <= kunst * 2 ** (Math.max(voll, ganz) - maxZoom) + 1; n++) {
            let z = maxZoom + Math.log2(n / kunst);
            if (Math.abs(z - Math.round(z)) < 1e-9) z = Math.round(z);
            const bruch = z - Math.floor(z);
            const leaflet = Number.isInteger(z) || (bruch >= 0.5 && z < maxZoom);
            if (leaflet && z >= ganz && z <= Math.max(voll, ganz) && 2 ** (z - voll) >= 0.71) stufen.push(z);
          }
          const was = `${name}, ${kante} Blöcke, ${breite} × ${hoehe}, devicePixelRatio ${dpr}`;
          if (stufen.length > 0) {
            const n = kunst * 2 ** (fit - maxZoom);
            expect(Math.abs(n - Math.round(n)), was).toBeLessThan(1e-9);
            expect(Math.abs(fit - ziel), was).toBeLessThanOrEqual(Math.min(...stufen.map((z) => Math.abs(z - ziel))) + 1e-12);
            zaehler.ganz++;
          } else {
            expect(fit, was).toBe(gesamtstufe(rahmen, maxZoom, breite, hoehe));
            zaehler.krumm++;
          }
        }
      }
    }
  }
  // Beide Fälle kommen vor.
  expect(zaehler.ganz).toBeGreaterThan(0);
  expect(zaehler.krumm).toBeGreaterThan(0);
});
