import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { pick, projiziere, region, strahl, umriss, type Block, type Projektion } from '../src/pick';

type Punkt = [number, number];

/**
 * Die Zahlen einer Kamera bei einem scale, wie sie der Renderer in
 * `map.json` schreibt: docs/renderer/kamera.md, „Projektion“. Nur der Test
 * kennt Kameras beim Namen; das Frontend liest die Zahlen.
 */
function kamera(name: string, scale: number): Projektion {
  if (name === 'top-north') return { azimuth: 'north', u: scale, v: scale, y: 0 };
  if (name === 'north-45') return { azimuth: 'north', u: scale, v: scale, y: scale };
  const [w, h] = name === 'top' ? [1, 1] : (name.split(':').map(Number) as [number, number]);
  const y = name === 'top' ? 0 : scale / 2;
  const p: Projektion = { azimuth: 'diagonal', u: scale / 2, v: (scale * h) / (2 * w), y };
  // Nur gültige Paare, wie beim Renderer: jede Ecke auf ganzen Pixeln.
  if (scale % 2 !== 0 || !Number.isInteger(p.v)) throw new Error(`${name} bei ${scale} ungültig`);
  return p;
}

/**
 * Die Einträge des Renderers, aktuell gehalten von einem seiner Tests.
 * Genordete Kameras rechnet das Frontend erst mit seiner PR zu #67; die
 * entfernt diesen Filter.
 */
const eintraege = (
  JSON.parse(
    readFileSync(new URL('../../renderer/tests/fixtures/projektion.json', import.meta.url), 'utf8'),
  ) as {
    camera: string;
    direction: string;
    scale: number;
    block: Block;
    pixel: Punkt;
    eben?: 0;
    wand?: 'south' | 'east';
  }[]
).filter((e) => e.camera !== 'top-north' && e.camera !== 'north-45');

test('die Projektion rechnet wie der Renderer, jede Kamera', () => {
  const paare = eintraege.filter((e) => e.eben === undefined && e.wand === undefined);
  expect(new Set(paare.map((e) => e.camera)).size).toBeGreaterThan(5);
  for (const { camera, direction, scale, block, pixel } of paare) {
    const p = kamera(camera, scale);
    expect(direction).toBe(p.azimuth === 'north' ? 's' : 'se');
    const name = `${camera}, scale ${scale}, Block ${String(block)}`;
    expect(projiziere(...block, p), name).toEqual(pixel);
  }
});

test('eine Pixelmitte auf einer Blockkante bekommt der Block, den der Renderer zeichnet', () => {
  const kanten = eintraege.filter((e) => e.eben !== undefined || e.wand !== undefined);
  expect(kanten.filter((e) => e.eben !== undefined).length).toBeGreaterThanOrEqual(6);
  // Auf der Südseite gewinnt die obere Fläche, auf der Ostseite die untere.
  expect(new Set(kanten.map((e) => e.wand))).toEqual(new Set([undefined, 'south', 'east']));
  for (const { camera, scale, block, pixel, wand } of kanten) {
    // Ebener Boden mit Oberseiten bei y = 0, oder eine Säule aus (4, 0, 4)
    // und (4, 1, 4) im Leeren.
    const hoehe = wand ? (x: number, z: number) => (x === 4 && z === 4 ? 1 : undefined) : () => -1;
    const bloecke = strahl(pixel[0], pixel[1], kamera(camera, scale), -64, 319);
    const name = `${camera}, scale ${scale}, ${wand ?? 'eben'}, Pixel ${String(pixel)}`;
    expect(pick(bloecke, hoehe), name).toEqual(block);
  }
});

/** Der Umriss eines Würfels, im Uhrzeigersinn auf dem Bildschirm; von oben die Oberseite. */
function umrandung([x, y, z]: Block, p: Projektion): Punkt[] {
  const ecke = (dx: number, dy: number, dz: number) => projiziere(x + dx, y + dy, z + dz, p);
  if (p.y === 0) return [ecke(0, 1, 0), ecke(1, 1, 0), ecke(1, 1, 1), ecke(0, 1, 1)];
  if (p.azimuth === 'north') return [ecke(0, 1, 0), ecke(1, 1, 0), ecke(1, 0, 1), ecke(0, 0, 1)];
  return [ecke(0, 1, 0), ecke(1, 1, 0), ecke(1, 0, 0), ecke(1, 0, 1), ecke(0, 0, 1), ecke(0, 1, 1)];
}

/**
 * Liegt p im Umriss? Auf einer Kante zählt er, wenn die Kante im Umlauf
 * nach oben führt: So bekommt ihn der Block mit dem grösseren u = x − z,
 * wie nach der Füllregel des Renderers.
 */
function innen([px, py]: Punkt, ecken: Punkt[]): boolean {
  return ecken.every(([ax, ay], i) => {
    const [bx, by] = ecken[(i + 1) % ecken.length]!;
    const kreuz = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
    return kreuz > 0 || (kreuz === 0 && by < ay);
  });
}

/** Ein kleines Gelände mit Löchern, 0 bis 5 hoch, auch bei negativen Koordinaten. */
function gelaende(): Map<string, number> {
  const hoehen = new Map<string, number>();
  for (let x = -6; x < 6; x++) {
    for (let z = -6; z < 6; z++) {
      const h = ((Math.imul(x, 73856093) ^ Math.imul(z, 19349663)) >>> 0) % 7;
      if (h < 6) hoehen.set(`${x},${z}`, h);
    }
  }
  return hoehen;
}

const KAMERAS: [string, number][] = [
  ['2:1', 4],
  ['2:1', 12],
  ['2:1', 16],
  ['2:1', 24],
  ['2:1', 32],
  ['2:1', 48],
  ['8:5', 32],
  ['4:3', 32],
  ['1:1', 6],
  ['1:1', 32],
  ['5:3', 30],
  ['top', 32],
  ['top', 6],
  ['top', 4],
  ['top-north', 6],
  ['top-north', 7],
  ['top-north', 16],
  ['top-north', 48],
  ['north-45', 6],
  ['north-45', 7],
  ['north-45', 12],
  ['north-45', 24],
  ['north-45', 32],
  ['north-45', 48],
];

for (const [name, scale] of KAMERAS) {
  test(`jeder Bildpunkt zeigt den Block, den das Zeichnen dort hinterlässt, ${name} bei ${scale}`, () => {
    const p = kamera(name, scale);
    const hoehen = gelaende();
    const hoehe = (x: number, z: number) => hoehen.get(`${x},${z}`);

    // Wie beim Zeichnen: Würfel in der Reihenfolge (y, v, u), ein späterer
    // übermalt einen früheren, je Pixelmitte im Umriss.
    const wuerfel: Block[] = [];
    for (const [spalte, h] of hoehen) {
      const [x, z] = spalte.split(',').map(Number) as [number, number];
      for (let y = 0; y <= h; y++) wuerfel.push([x, y, z]);
    }
    wuerfel.sort((a, b) => a[1] - b[1] || a[0] + a[2] - b[0] - b[2] || a[0] - a[2] - b[0] + b[2]);
    const sichtbar = new Map<string, Block>();
    for (const block of wuerfel) {
      const ecken = umrandung(block, p);
      const xs = ecken.map(([x]) => x);
      const ys = ecken.map(([, y]) => y);
      for (let u = Math.floor(Math.min(...xs)); u < Math.max(...xs); u++) {
        for (let v = Math.floor(Math.min(...ys)); v < Math.max(...ys); v++) {
          if (innen([u + 0.5, v + 0.5], ecken)) sichtbar.set(`${u},${v}`, block);
        }
      }
    }

    // Jeder Pixel im Rechteck um das Gelände, auch die leeren, getroffen
    // nahe einer Ecke des Pixels: zählen muss seine Mitte.
    const falsch: string[] = [];
    let getroffen = 0;
    // Das Rechteck um die Ecken des Geländes, ein Pixel Rand.
    const ecken = [-6, 6].flatMap((x) =>
      [0, 6].flatMap((y) => [-6, 6].map((z) => projiziere(x, y, z, p))),
    );
    const [u0, u1] = [Math.min(...ecken.map(([u]) => u)), Math.max(...ecken.map(([u]) => u))];
    const [v0, v1] = [Math.min(...ecken.map(([, v]) => v)), Math.max(...ecken.map(([, v]) => v))];
    for (let u = u0 - 1; u <= u1; u++) {
      for (let v = v0 - 1; v <= v1; v++) {
        const erwartet = sichtbar.get(`${u},${v}`);
        const block = pick(strahl(u + 0.97, v + 0.04, p, 0, 5), hoehe);
        if (block) getroffen++;
        if (String(block) !== String(erwartet)) {
          falsch.push(`${u},${v}: ${String(block)} statt ${String(erwartet)}`);
        }
      }
    }
    expect(falsch.slice(0, 10)).toEqual([]);
    expect(getroffen).toBe(sichtbar.size);
    expect(getroffen).toBeGreaterThan(1000);
  });
}

test('ein Strahl geht bei 2:1 drei Würfel je Schicht, bei 1:1 und north-45 zwei, von oben einen', () => {
  expect(strahl(5, 7, kamera('2:1', 32), -64, 319)).toHaveLength(384 * 3);
  expect(strahl(5, 7, kamera('1:1', 32), -64, 319)).toHaveLength(384 * 2);
  expect(strahl(5, 7, kamera('north-45', 32), -64, 319)).toHaveLength(384 * 2);
  expect(strahl(5, 7, kamera('top', 32), -64, 319)).toHaveLength(384);
  expect(strahl(5, 7, kamera('top-north', 32), -64, 319)).toHaveLength(384);
});

test('der Umriss: Sechseck und vordere Ecke, von oben die Oberseite, genordet ein Rechteck', () => {
  const block: Block = [-3, 70, 12];
  const p = kamera('4:3', 32);
  const ecke = (x: number, y: number, z: number) => projiziere(x, y, z, p);
  const vorn = ecke(-2, 71, 13);
  expect(umriss(block, p)).toEqual([
    [...umrandung(block, p), ecke(-3, 71, 12)],
    [ecke(-3, 71, 13), vorn, ecke(-2, 71, 12)],
    [vorn, ecke(-2, 70, 13)],
  ]);
  const oben = kamera('top', 32);
  expect(umriss(block, oben)).toEqual([[...umrandung(block, oben), projiziere(-3, 71, 12, oben)]]);
  // Genordet: Oberseite und Südseite als Rechteck, dazwischen ihre Kante.
  const norden = kamera('north-45', 16);
  const n = (x: number, y: number, z: number) => projiziere(x, y, z, norden);
  expect(umriss(block, norden)).toEqual([
    [n(-3, 71, 12), n(-2, 71, 12), n(-2, 70, 13), n(-3, 70, 13), n(-3, 71, 12)],
    [n(-3, 71, 13), n(-2, 71, 13)],
  ]);
});

test('Spalten finden Region und Zelle, auch negative', () => {
  // Je Spalte eine Zelle.
  expect(region(0, 0, 1)).toEqual({ rx: 0, rz: 0, i: 0 });
  expect(region(511, 1, 1)).toEqual({ rx: 0, rz: 0, i: 512 + 511 });
  expect(region(512, -1, 1)).toEqual({ rx: 1, rz: -1, i: 511 * 512 });
  expect(region(-513, -512, 1)).toEqual({ rx: -2, rz: -1, i: 511 });
  // 4 × 4 Spalten je Zelle, 128 × 128 Zellen je Region.
  expect(region(3, 3, 4)).toEqual({ rx: 0, rz: 0, i: 0 });
  expect(region(4, 511, 4)).toEqual({ rx: 0, rz: 0, i: 127 * 128 + 1 });
  expect(region(512, -1, 4)).toEqual({ rx: 1, rz: -1, i: 127 * 128 });
  expect(region(-513, -512, 4)).toEqual({ rx: -2, rz: -1, i: 127 });
  expect(region(-1, -4, 4)).toEqual({ rx: -1, rz: -1, i: 127 * 128 + 127 });
  expect(region(-1, -5, 4)).toEqual({ rx: -1, rz: -1, i: 126 * 128 + 127 });
});
