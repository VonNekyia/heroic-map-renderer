import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { inflateSync } from 'node:zlib';
import { atlas, DICHTEN } from '../atlas';

/**
 * Ein Bild aus bilder/, wie das Skript es schreibt: 8 Bit mit Palette, keine
 * Zeile gefiltert. Gibt Grösse, Palette und je Pixel die Stelle in ihr.
 */
function lies(datei: string) {
  const daten = readFileSync(new URL(`../bilder/${datei}`, import.meta.url));
  let [breite, hoehe] = [0, 0];
  let palette: [number, number, number][] = [];
  const idat: Buffer[] = [];
  for (let i = 8; i < daten.length; i += 12 + daten.readUInt32BE(i)) {
    const inhalt = daten.subarray(i + 8, i + 8 + daten.readUInt32BE(i));
    const typ = daten.toString('latin1', i + 4, i + 8);
    if (typ === 'IHDR') {
      [breite, hoehe] = [inhalt.readUInt32BE(0), inhalt.readUInt32BE(4)];
      expect([inhalt[8], inhalt[9]], `${datei}: 8 Bit mit Palette`).toEqual([8, 3]);
    }
    if (typ === 'PLTE') {
      palette = Array.from({ length: inhalt.length / 3 }, (_, k): [number, number, number] => [inhalt[3 * k]!, inhalt[3 * k + 1]!, inhalt[3 * k + 2]!]);
    }
    if (typ === 'IDAT') idat.push(inhalt);
  }
  const zeilen = inflateSync(Buffer.concat(idat));
  const stellen = new Uint8Array(breite * hoehe);
  for (let y = 0; y < hoehe; y++) {
    expect(zeilen[y * (breite + 1)], `${datei}, Zeile ${y} gefiltert`).toBe(0);
    stellen.set(zeilen.subarray(y * (breite + 1) + 1, (y + 1) * (breite + 1)), y * breite);
  }
  return { breite, hoehe, palette, stellen };
}

test('jeder Atlas hat die Grösse, die der Skin erwartet', () => {
  for (const dichte of DICHTEN) {
    const { breite, hoehe } = lies(`atlas-${dichte}.png`);
    const plan = atlas(dichte);
    expect([breite, hoehe], `atlas-${dichte}.png: neu erzeugen mit werkzeug/texturen.ts`).toEqual([plan.breite, plan.hoehe]);
  }
});

test('die Kachel Marmor hat keine Naht und wenige Adern, im Median dunkles Gold, hell nur an einzelnen Stellen', () => {
  const { breite: n, hoehe, palette, stellen } = lies('marmor.png');
  expect(hoehe).toBe(n);
  const hell = (stelle: number) => palette[stelle]!.reduce((a, b) => a + b);
  // Mittlerer Sprung der Helligkeit zwischen zwei Spalten oder Zeilen.
  const sprung = (ort: (k: number, i: number) => number, i: number, j: number) => {
    let summe = 0;
    for (let k = 0; k < n; k++) summe += Math.abs(hell(stellen[ort(k, i)]!) - hell(stellen[ort(k, j)]!));
    return summe / n;
  };
  const spalte = (k: number, i: number) => k * n + i;
  const zeile = (k: number, i: number) => i * n + k;
  // Über die Naht nicht mehr als zwischen Nachbarn im Innern.
  for (const ort of [spalte, zeile]) {
    const innen = [n / 4, n / 2, (3 * n) / 4].map((i) => sprung(ort, i - 1, i));
    expect(sprung(ort, n - 1, 0)).toBeLessThan(1.5 * Math.max(...innen));
  }
  // Adern sind gold, also viel wärmer als der dunkle, grünliche Grund. Unter
  // 2 % der Fläche, nach der Zahl ihrer Pixel im Median `#6e4628` wie die
  // Vorlage, die hellste Farbe unter 5 % davon.
  const zahl = palette.map(() => 0);
  for (const stelle of stellen) zahl[stelle]!++;
  const adern = [...palette.keys()].filter((k) => palette[k]![0] - palette[k]![2] > 35).sort((p, q) => hell(p) - hell(q));
  const alle = adern.reduce((summe, k) => summe + zahl[k]!, 0);
  expect(alle).toBeGreaterThan(0);
  expect(alle / (n * n)).toBeLessThan(0.02);
  let bis = 0;
  const median = palette[adern.find((k) => (bis += zahl[k]!) >= alle / 2)!]!;
  expect(`#${median.map((kanal) => kanal.toString(16).padStart(2, '0')).join('')}`).toBe('#6e4628');
  expect(zahl[adern.at(-1)!]! / alle).toBeLessThan(0.05);
});
