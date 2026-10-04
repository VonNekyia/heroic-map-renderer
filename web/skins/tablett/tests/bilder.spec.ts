import { expect, test } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import { ECKSTUECKE, GEGENSTAENDE, LILIEN, MASS, PFEILER, RAND, SEITEN } from '../bilder';

const ORDNER = new URL('../bilder/', import.meta.url);

/** Breite und Höhe eines WebP-Bilds, aus seinem Kopf: verlustbehaftet, verlustfrei oder erweitert. */
function groesse(datei: string): [number, number] {
  const d = readFileSync(new URL(datei, ORDNER));
  expect([d.toString('latin1', 0, 4), d.toString('latin1', 8, 12)], datei).toEqual(['RIFF', 'WEBP']);
  const art = d.toString('latin1', 12, 16);
  if (art === 'VP8X') return [1 + d.readUIntLE(24, 3), 1 + d.readUIntLE(27, 3)];
  if (art === 'VP8L') {
    const bits = d.readUInt32LE(21);
    return [1 + (bits & 0x3fff), 1 + ((bits >> 14) & 0x3fff)];
  }
  expect(art, datei).toBe('VP8 ');
  return [d.readUInt16LE(26) & 0x3fff, d.readUInt16LE(28) & 0x3fff];
}

test('jedes Bild in bilder/ nimmt der Skin, und jedes, das er nimmt, liegt dort', () => {
  const da = readdirSync(ORDNER).sort();
  const genommen = [
    ...Object.values(SEITEN).flatMap(({ band, wand }) => [band, ...(wand ? [wand] : [])]),
    ...Object.values(PFEILER),
    ...Object.values(ECKSTUECKE),
    ...Object.values(LILIEN).map((l) => l.bild),
    ...GEGENSTAENDE.map((g) => g.bild),
    'tisch',
  ].map((name) => `${name}.webp`);
  expect(da).toEqual([...new Set(genommen)].sort());
});

test('Streifen und Eckstücke haben das Seitenverhältnis ihrer Flächen, Lilien und Gegenstände die Grösse aus bilder.ts', () => {
  // Breite durch Höhe gegen die Fläche in w, wo eine Seite 1 / RAND lang ist;
  // auf ganze Pixel gerundet weicht es um unter 1 % ab.
  const passt = (datei: string, soll: number) => {
    const [b, h] = groesse(`${datei}.webp`);
    expect(Math.abs(b / h / soll - 1), datei).toBeLessThan(0.01);
  };
  for (const { band, wand } of Object.values(SEITEN)) {
    passt(band, 1 / RAND);
    if (wand) passt(wand, 1 / RAND / MASS.tiefe);
  }
  for (const bild of Object.values(PFEILER)) passt(bild, MASS.pfeiler / MASS.tiefe);
  for (const bild of Object.values(ECKSTUECKE)) passt(bild, 1);
  for (const { bild, groesse: soll } of [...Object.values(LILIEN), ...GEGENSTAENDE]) expect(groesse(`${bild}.webp`), bild).toEqual(soll);
});
