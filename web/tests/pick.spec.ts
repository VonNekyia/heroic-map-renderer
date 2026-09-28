import { expect, test } from '@playwright/test';
import { pick, region, strahl, umriss, type Block } from '../src/pick';

type Punkt = [number, number];

/** Die Projektion des Renderers, docs/renderer/kamera.md, „Projektion“. */
function projiziere(x: number, y: number, z: number, scale: number): Punkt {
  return [((x - z) * scale) / 2, ((x + z) * scale) / 4 - (y * scale) / 2];
}

/** Der Umriss eines Würfels, im Uhrzeigersinn auf dem Bildschirm. */
function sechseck([x, y, z]: Block, scale: number): Punkt[] {
  return [
    projiziere(x, y + 1, z, scale),
    projiziere(x + 1, y + 1, z, scale),
    projiziere(x + 1, y, z, scale),
    projiziere(x + 1, y, z + 1, scale),
    projiziere(x, y, z + 1, scale),
    projiziere(x, y + 1, z + 1, scale),
  ];
}

/** Liegt p echt innen? Auf einer Kante zählt nicht, dort liegt keine Pixelmitte. */
function innen([px, py]: Punkt, ecken: Punkt[]): boolean {
  return ecken.every(([ax, ay], i) => {
    const [bx, by] = ecken[(i + 1) % ecken.length]!;
    return (bx - ax) * (py - ay) - (by - ay) * (px - ax) > 0;
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

for (const scale of [12, 16, 32]) {
  test(`jeder Bildpunkt zeigt den Block, den das Zeichnen dort hinterlässt, scale ${scale}`, () => {
    const hoehen = gelaende();
    const hoehe = (x: number, z: number) => hoehen.get(`${x},${z}`);

    // Wie beim Zeichnen: je Pixelmitte der Würfel mit der grössten Tiefe
    // x + y + z, dessen Umriss sie enthält.
    const sichtbar = new Map<string, { tiefe: number; block: Block }>();
    for (const [spalte, h] of hoehen) {
      const [x, z] = spalte.split(',').map(Number) as [number, number];
      for (let y = 0; y <= h; y++) {
        const block: Block = [x, y, z];
        const ecken = sechseck(block, scale);
        const [ox, oy] = ecken[0]!;
        for (let u = ox - scale / 2; u < ox + scale / 2; u++) {
          for (let v = oy; v < oy + scale; v++) {
            if (!innen([u + 0.5, v + 0.5], ecken)) continue;
            const alt = sichtbar.get(`${u},${v}`);
            if (!alt || alt.tiefe < x + y + z) sichtbar.set(`${u},${v}`, { tiefe: x + y + z, block });
          }
        }
      }
    }

    // Jeder Pixel im Rechteck um das Gelände, auch die leeren, getroffen
    // nahe einer Ecke des Pixels: zählen muss seine Mitte.
    const falsch: string[] = [];
    let getroffen = 0;
    for (let u = -7 * scale; u < 7 * scale; u++) {
      for (let v = -7 * scale; v < 4 * scale; v++) {
        const erwartet = sichtbar.get(`${u},${v}`)?.block;
        const block = pick(strahl(u + 0.97, v + 0.04, scale, 0, 5), hoehe);
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

test('der Umriss ist das Sechseck des Würfels und die vordere Ecke', () => {
  const block: Block = [-3, 70, 12];
  const vorn = projiziere(-2, 71, 13, 32);
  expect(umriss(block, 32)).toEqual([
    [...sechseck(block, 32), projiziere(-3, 71, 12, 32)],
    [projiziere(-3, 71, 13, 32), vorn, projiziere(-2, 71, 12, 32)],
    [vorn, projiziere(-2, 70, 13, 32)],
  ]);
});

test('Spalten finden ihre Region, auch negative', () => {
  expect(region(0, 0)).toEqual({ rx: 0, rz: 0, i: 0 });
  expect(region(511, 1)).toEqual({ rx: 0, rz: 0, i: 512 + 511 });
  expect(region(512, -1)).toEqual({ rx: 1, rz: -1, i: 511 * 512 });
  expect(region(-513, -512)).toEqual({ rx: -2, rz: -1, i: 511 });
});
