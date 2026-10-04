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
  // brett.json des Platzhalters aller 32 Blicke, ohne die Bilder: Grösse und
  // Mitte hängen nur an der Kamera, nicht an der Szene.
  const index = JSON.parse(readFileSync(new URL('fixtures/brett-alle.json', import.meta.url), 'utf8')) as Brett;
  const area: Rechteck = [-512, -512, 512, 512];
  const [meer, maxZoom] = [62, 6];
  const fenster = [
    [360, 800],
    [390, 844],
    [768, 1024],
    [1024, 768],
    [1280, 720],
    [1920, 1080],
    [2560, 1080],
    [3440, 1440],
  ];
  for (const { p } of KAMERAS) {
    for (let k = 0; k < 4; k++) {
      const name = kameraName(p, k);
      const bild = index[name]!;
      const b = blick(p, k);
      const { links, oben, mass } = lage(area, meer, b, bild);
      const [rechts, unten] = [links + bild.groesse[0] * mass, oben + bild.groesse[1] * mass];
      for (const [breite, hoehe] of fenster) {
        const f = 2 ** (maxZoom - gesamtstufe(grenzen(area, meer, b), maxZoom, breite!, hoehe!));
        const [mx, my] = gesamtmitte(area, meer, b, breite! * f, hoehe! * f);
        const was = `${name}, ${breite} × ${hoehe}`;
        expect(mx - (breite! * f) / 2, was).toBeGreaterThanOrEqual(links);
        expect(my - (hoehe! * f) / 2, was).toBeGreaterThanOrEqual(oben);
        expect(mx + (breite! * f) / 2, was).toBeLessThanOrEqual(rechts);
        expect(my + (hoehe! * f) / 2, was).toBeLessThanOrEqual(unten);
      }
    }
  }
});
