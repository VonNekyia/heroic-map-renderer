/**
 * Der Worker des Tabletts: Er rechnet die Flächen mit Textur und den Marmor
 * und malt beide Ebenen abseits des Hauptthreads, damit das Laden nicht
 * stockt. Siehe docs/tablett.md, „Texturen“.
 */
import { KACHEL, type Licht, marmorKachel, type MitTextur, type Pixel, rechne } from './stoffe';
import type { Flaeche, Teil } from './tablett';
import { ebenen, marmor } from './zeichnen';

/** Was der Skin schickt; `s` und `versatz` wie bei `rechne`, `leinwand` ist Breite und Höhe der Ebenen. */
export interface Auftrag {
  nummer: number;
  teile: Teil[];
  s: number;
  versatz: [number, number];
  leinwand: [number, number];
  licht: Licht;
  /** v/u der Projektion, für die Kachel Marmor. */
  streckung: number;
}

/** Was zurückkommt: die Ebenen fern und nah, dazu die Zeit im Worker in ms. */
export interface Ergebnis {
  nummer: number;
  bilder: ImageBitmap[];
  dauer: number;
}

/** Der Grund des Marmors; die Projektion und mit ihr die Streckung bleiben für die Seite gleich. */
let grund: Uint8ClampedArray<ArrayBuffer> | undefined;

self.onmessage = ({ data }: MessageEvent<Auftrag>) => {
  const beginn = performance.now();
  const { teile, s, versatz, leinwand, licht } = data;
  grund ??= marmorKachel(KACHEL, data.streckung);
  const platte = marmor(grund, ...leinwand, data.streckung);
  const pixel = new Map<Flaeche, Pixel>();
  for (const teil of teile) {
    if (teil.form === 'flaeche' && teil.textur) pixel.set(teil, rechne(teil as MitTextur, s, versatz, licht, leinwand));
  }
  const [fern, nah] = [new OffscreenCanvas(...leinwand), new OffscreenCanvas(...leinwand)];
  ebenen(fern.getContext('2d')!, nah.getContext('2d')!, teile, s, versatz, pixel, platte);
  const bilder = [fern.transferToImageBitmap(), nah.transferToImageBitmap()];
  const ergebnis: Ergebnis = { nummer: data.nummer, bilder, dauer: performance.now() - beginn };
  self.postMessage(ergebnis, { transfer: bilder });
};
