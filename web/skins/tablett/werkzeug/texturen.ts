/**
 * Erzeugt die Bilder des Tabletts in ../bilder/: je Dichte einen Atlas
 * aus Holz und Messing, dazu die Kachel Marmor, als PNG mit Palette. Läuft
 * einmal von Hand, nicht im Build:
 *
 *     node skins/tablett/werkzeug/texturen.ts
 *
 * aus web/. Siehe docs/tablett.md, „Texturen“.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { crc32, deflateSync } from 'node:zlib';
import { atlas, BILDER, DICHTEN } from '../atlas.ts';
import { HOLZ, KACHEL, male, marmor, MARMOR, type Rgb } from './stoffe.ts';

/**
 * Ein PNG mit 8 Bit je Pixel aus einer Palette, ohne Filter. Die ersten
 * `durchsichtig` Farben der Palette sind durchsichtig.
 */
function png(breite: number, hoehe: number, stellen: Uint8Array, palette: Rgb[], durchsichtig = 0): Buffer {
  const stueck = (typ: string, daten: Buffer): Buffer => {
    const kopf = Buffer.alloc(8);
    kopf.writeUInt32BE(daten.length, 0);
    kopf.write(typ, 4, 'latin1');
    const pruefsumme = Buffer.alloc(4);
    pruefsumme.writeUInt32BE(crc32(Buffer.concat([kopf.subarray(4), daten])), 0);
    return Buffer.concat([kopf, daten, pruefsumme]);
  };
  const kopf = Buffer.alloc(13);
  kopf.writeUInt32BE(breite, 0);
  kopf.writeUInt32BE(hoehe, 4);
  // Tiefe 8, Farbtyp 3: Palette; Kompression, Filter und Zeilenfolge wie üblich.
  kopf.set([8, 3, 0, 0, 0], 8);
  // Jede Zeile beginnt mit ihrem Filter, hier 0.
  const zeilen = Buffer.alloc((breite + 1) * hoehe);
  for (let y = 0; y < hoehe; y++) zeilen.set(stellen.subarray(y * breite, (y + 1) * breite), y * (breite + 1) + 1);
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    stueck('IHDR', kopf),
    stueck('PLTE', Buffer.from(palette.flat())),
    // Je Farbe der Palette die Deckkraft; was fehlt, deckt ganz.
    ...(durchsichtig > 0 ? [stueck('tRNS', Buffer.alloc(durchsichtig))] : []),
    stueck('IDAT', deflateSync(zeilen, { level: 9 })),
    stueck('IEND', Buffer.alloc(0)),
  ]);
}

const ZIEL = new URL('../bilder/', import.meta.url);
mkdirSync(ZIEL, { recursive: true });

/** Schreibt ein Bild und sagt, wie gross es ist. */
function schreibe(name: string, daten: Buffer): void {
  writeFileSync(new URL(name, ZIEL), daten);
  console.log(`${name.padEnd(14)} ${(daten.length / 1024).toFixed(1).padStart(6)} KB`);
}

const stelleIn = (palette: Rgb[]) => new Map(palette.map((f, i) => [f.join(), i]));

// Vorn eine Farbe für durchsichtig, dann die Rampen.
const PALETTE: Rgb[] = [[0, 0, 0], ...HOLZ];
const holz = stelleIn(PALETTE);
for (const dichte of DICHTEN) {
  const { breite, hoehe, bereiche } = atlas(dichte);
  // Was kein Bild deckt, bleibt durchsichtig; es wird nie gelegt.
  const stellen = new Uint8Array(breite * hoehe);
  for (const bild of BILDER) {
    const bereich = bereiche.get(bild.name)!;
    const farben = male(bild, dichte);
    for (let k = 0; k < bereich.hoehe; k++) {
      for (let i = 0; i < bereich.breite; i++) {
        const farbe = farben[k * bereich.breite + i];
        stellen[(bereich.y + k) * breite + bereich.x + i] = farbe ? holz.get(farbe.join())! : 0;
      }
    }
  }
  schreibe(`atlas-${dichte}.png`, png(breite, hoehe, stellen, PALETTE, 1));
}
schreibe('marmor.png', png(KACHEL, KACHEL, marmor(KACHEL), MARMOR));
