import { expect, test, type Page } from '@playwright/test';
import { readdirSync, readFileSync } from 'node:fs';
import { ECKSTUECKE, GEGENSTAENDE, LILIEN, MARMOR, MASS, PFEILER, RAND, SEITEN, TISCH_RAND, VORLAGE } from '../bilder';

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
    'marmor',
  ].map((name) => `${name}.webp`);
  expect(da).toEqual([...new Set(genommen)].sort());
});

test('Streifen und Eckstücke haben das Seitenverhältnis ihrer Flächen, Lilien, Gegenstände, Tisch und Marmor die Grösse aus bilder.ts', () => {
  expect(groesse('tisch.webp')).toEqual([VORLAGE[0] + 2 * TISCH_RAND, VORLAGE[1] + 2 * TISCH_RAND]);
  expect(groesse('marmor.webp')).toEqual([MARMOR, MARMOR]);
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

/**
 * Ein Bild aus bilder/, im Browser gelesen: je Block von 8 × 8 Pixeln das
 * Mittel von Rot, Grün und Deckkraft, dazu die grösste Deckkraft an der
 * Kante des Bilds und die kleinste mindestens `innen` Pixel davon entfernt.
 */
async function lies(page: Page, datei: string, innen = 0) {
  return page.evaluate(
    async ({ b64, innen }) => {
      const bild = await createImageBitmap(new Blob([Uint8Array.from(atob(b64), (z) => z.charCodeAt(0))]));
      const { width: w, height: h } = bild;
      const ctx = new OffscreenCanvas(w, h).getContext('2d')!;
      ctx.drawImage(bild, 0, 0);
      const { data } = ctx.getImageData(0, 0, w, h);
      const bloecke: { rot: number; gruen: number; deckt: number }[] = [];
      for (let by = 0; by + 8 <= h; by += 8) {
        for (let bx = 0; bx + 8 <= w; bx += 8) {
          let [rot, gruen, deckt] = [0, 0, 0];
          for (let y = by; y < by + 8; y++) {
            for (let x = bx; x < bx + 8; x++) {
              const i = 4 * (y * w + x);
              rot += data[i]!;
              gruen += data[i + 1]!;
              deckt += data[i + 3]!;
            }
          }
          bloecke.push({ rot: rot / 64, gruen: gruen / 64, deckt: deckt / 64 });
        }
      }
      let [kante, drin] = [0, 255];
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          const deckt = data[4 * (y * w + x) + 3]!;
          if (x === 0 || y === 0 || x === w - 1 || y === h - 1) kante = Math.max(kante, deckt);
          if (x >= innen && y >= innen && x < w - innen && y < h - innen) drin = Math.min(drin, deckt);
        }
      }
      return { bloecke, kante, drin };
    },
    { b64: readFileSync(new URL(datei, ORDNER)).toString('base64'), innen },
  );
}

/**
 * Holz: im Mittel eines Blocks von 8 × 8 Pixeln Rot mindestens 1,6-mal so
 * stark wie Grün und nicht im tiefen Schatten. Marmor bleibt darunter, auch
 * mit seinen Goldadern, denn sie sind dünn; im Marmor der Vorlage höchstens
 * 1,47.
 */
const holz = ({ rot, gruen, deckt }: { rot: number; gruen: number; deckt: number }) => deckt > 250 && rot >= 1.6 * gruen && rot >= 40;

test('jenseits der Vorlage liegt nur Marmor ohne die Farbe des Holzes, und der Tisch läuft über ihren Rand hinaus bis auf nichts aus', async ({ page }) => {
  const marmor = await lies(page, 'marmor.webp');
  expect(marmor.bloecke.filter(holz)).toEqual([]);
  // Die Regel erkennt Holz: Der Tisch zeigt den Holzrand der Vorlage.
  const tisch = await lies(page, 'tisch.webp', TISCH_RAND);
  expect(tisch.bloecke.filter(holz).length).toBeGreaterThan(1000);
  // Die Vorlage deckt ganz; ihr Rand läuft bis zur Kante des Bilds aus.
  expect(tisch.drin).toBe(255);
  expect(tisch.kante).toBe(0);
});
