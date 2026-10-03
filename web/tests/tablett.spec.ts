import { expect, test } from '@playwright/test';
import { projiziere, RICHTUNGEN, type Projektion } from '../src/pick';
import { imBlick, tablett, type Flaeche, type Rechteck } from '../src/tablett';
import { eintraege, kamera } from './kamera';

type Punkt = [number, number];

const paare = eintraege.filter((e) => e.eben === undefined && e.wand === undefined);

/** Jede Kamera aus den Einträgen des Renderers mit einem ihrer scales. */
const KAMERAS = [...new Map(paare.map((e) => [e.camera, e.scale])).entries()];

/** Liegt der Punkt in der Fläche? `rand` > 0 schrumpft sie, < 0 dehnt sie, in Teilen der Kanten. */
function deckt({ o, a, b }: Flaeche, [px, py]: Punkt, rand = 0): boolean {
  const det = a[0] * b[1] - a[1] * b[0];
  if (det === 0) return false;
  const [dx, dy] = [px - o[0], py - o[1]];
  const s = (dx * b[1] - dy * b[0]) / det;
  const t = (a[0] * dy - a[1] * dx) / det;
  return [s, t].every((w) => w > rand && w < 1 - rand);
}

test('die Oberkante liegt auf den Ecken von area in Höhe seaLevel, auf das Pixel', () => {
  expect(new Set(paare.map((e) => e.camera)).size).toBeGreaterThan(5);
  for (const { camera, direction, scale, block, pixel } of paare) {
    const p = kamera(camera, scale);
    const k = RICHTUNGEN[p.azimuth].indexOf(direction);
    // Eine Welt aus einem Block: Die Ecke vorn links oben im Blick ist das
    // Pixel des Renderers für diesen Block.
    const [bx, by, bz] = block;
    const rand = tablett([bx, bz, bx + 1, bz + 1], by, by - 128, p, k).filter((f) => f.art === 'rand');
    // Ein Stück daneben: Nur in Richtung der Welt liegt kein Rand.
    const [dx, dz] = [projiziere(1, 0, 0, p), projiziere(0, 0, 1, p)];
    const e = 1 / 64;
    for (const sx of [-1, 1]) {
      for (const sz of [-1, 1]) {
        const punkt: Punkt = [
          pixel[0] + e * (sx * dx[0] + sz * dz[0]),
          pixel[1] + e * (sx * dx[1] + sz * dz[1]),
        ];
        const name = `${camera} ${direction}, scale ${scale}, Block ${String(block)}, ${sx} ${sz}`;
        expect(rand.some((f) => deckt(f, punkt)), name).toBe(sx < 0 || sz < 0);
      }
    }
  }
});

/** Die Seiten eines Blocks, die die Kamera sieht, je als Ecke und zwei Kanten. */
function seiten([x, y, z]: [number, number, number], p: Projektion) {
  const liste: [number, number, number][][] = [[[x, y + 1, z], [1, 0, 0], [0, 0, 1]]];
  if (p.y > 0) liste.push([[x, y, z + 1], [1, 0, 0], [0, 1, 0]]);
  if (p.y > 0 && p.azimuth === 'diagonal') liste.push([[x + 1, y, z], [0, 0, 1], [0, 1, 0]]);
  return liste;
}

/** Mitte und vier Punkte nahe den Ecken einer Seite, im Bild. */
function proben([o, a, b]: [number, number, number][], p: Projektion): Punkt[] {
  const bei = (s: number, t: number) =>
    projiziere(o![0] + s * a![0] + t * b![0], o![1] + s * a![1] + t * b![1], o![2] + s * a![2] + t * b![2], p);
  return [bei(0.5, 0.5), bei(0.1, 0.1), bei(0.9, 0.1), bei(0.1, 0.9), bei(0.9, 0.9)];
}

const AREA: Rechteck = [-40, -24, 40, 56];
const MEER = 63;
const MIN_Y = -64;

test('was vor den Kacheln liegt, deckt kein Gelände über dem Wasserspiegel, an keinem Rand', () => {
  // Gesammelt statt je Punkt geprüft: Es sind Hunderttausende.
  const gedeckt: string[] = [];
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const nah = tablett(AREA, MEER, MIN_Y, p, k).filter((f) => f.nah);
      expect(nah.length).toBeGreaterThan(0);
      const [x0, z0, x1, z1] = imBlick(AREA, k);
      // Ein Hügel aus drei Blöcken über dem Wasser an jedem Rand.
      const bloecke: [number, number, number][] = [];
      for (let y = MEER; y < MEER + 3; y++) {
        for (let x = x0; x < x1; x++) bloecke.push([x, y, z0], [x, y, z1 - 1]);
        for (let z = z0; z < z1; z++) bloecke.push([x0, y, z], [x1 - 1, y, z]);
      }
      for (const block of bloecke) {
        for (const seite of seiten(block, p)) {
          for (const punkt of proben(seite, p)) {
            if (nah.some((f) => deckt(f, punkt))) gedeckt.push(`${camera} k=${k}, Block ${String(block)}`);
          }
        }
      }
    }
  }
  expect(gedeckt.slice(0, 10)).toEqual([]);
});

/** Eine schmale Welt: Hier reicht der Tisch nur über den Schnitt, weil er es muss. */
const SCHMAL: Rechteck = [-12, -4, 12, 20];

test('der Schnitt der Welt zur Kamera liegt ganz unter Wand und Tisch', () => {
  const offen: string[] = [];
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    // Von oben zeigt die Welt keinen Schnitt.
    if (p.y === 0) continue;
    for (let k = 0; k < 4; k++) {
      const nah = tablett(SCHMAL, MEER, MIN_Y, p, k).filter((f) => f.nah);
      const [x0, z0, x1, z1] = imBlick(SCHMAL, k);
      const punkte: [number, number, number][] = [];
      for (let y = MIN_Y + 0.5; y < MEER; y += 4) {
        for (let x = x0 + 0.5; x < x1; x += 2) punkte.push([x, y, z1]);
        if (p.azimuth === 'diagonal') for (let z = z0 + 0.5; z < z1; z += 2) punkte.push([x1, y, z]);
      }
      for (const punkt of punkte) {
        const bild = projiziere(...punkt, p);
        if (!nah.some((f) => deckt(f, bild, -1e-9))) offen.push(`${camera} k=${k}, ${String(punkt)}`);
      }
    }
  }
  expect(offen.slice(0, 10)).toEqual([]);
});
