/**
 * Gerenderte Bilder des Bretts: je Kamera und Richtung eines unter und eines
 * über den Kacheln, aus Blender (werkzeug/brett.py). Hier steht, welches Bild
 * zu einer Kamera gehört und wo es über der Karte liegt. Ohne Leaflet, damit
 * die Tests es in Node laden. Siehe docs/tablett-gerendert.md, „Im Skin“.
 */
import type { Projektion, Rechteck } from 'heroic-map-renderer/skin-api';
import { type Blick, imBlick } from './tablett';

/**
 * Ein Eintrag in brett.json: die Bilder unter und über den Kacheln, ihre
 * Grösse, die Mitte der Karte am Wasserspiegel darin, auf einer Pixelecke,
 * und `u` der Projektion des Bilds in Pixeln je Kante der Karte.
 */
export interface Brettbild {
  fern: string;
  nah: string;
  groesse: [number, number];
  mitte: [number, number];
  u: number;
}

/** brett.json: die Bilder nach dem Namen ihrer Kamera. */
export type Brett = Record<string, Brettbild>;

const RICHTUNGEN = { diagonal: ['se', 'sw', 'nw', 'ne'], north: ['s', 'w', 'n', 'e'] } as const;

/**
 * Der Name der Kamera in brett.json wie `--camera` und `--direction` des
 * Renderers, etwa `8:5 se` oder `top-north s`. Schräg ist v/u = H/W, gekürzt.
 */
export function kameraName(p: Projektion, k: number): string {
  const richtung = RICHTUNGEN[p.azimuth][k];
  if (p.azimuth === 'north') return `${p.y === 0 ? 'top-north' : 'north-45'} ${richtung}`;
  if (p.y === 0) return `top ${richtung}`;
  for (let w = 1; w <= 64; w++) {
    const h = (w * p.v) / p.u;
    if (Math.abs(h - Math.round(h)) < 1e-9) return `${w}:${Math.round(h)} ${richtung}`;
  }
  return `? ${richtung}`;
}

/**
 * Wo ein Bild über der Karte liegt, in Pixeln der feinsten Stufe: seine linke
 * obere Ecke und wie viele davon ein Pixel des Bilds deckt. Das Bild ist die
 * Projektion der Karte, nur kleiner: v/u und y/u sind dieselben.
 */
export function lage(area: Rechteck, meer: number, { projektion: p, k, projiziere }: Blick, bild: Brettbild) {
  const [x0, z0, x1, z1] = imBlick(area, k);
  const [mx, my] = projiziere((x0 + x1) / 2, meer, (z0 + z1) / 2);
  const mass = ((x1 - x0) * p.u) / bild.u;
  return { links: mx - bild.mitte[0] * mass, oben: my - bild.mitte[1] * mass, mass };
}

/**
 * Malt ein gerendertes Bild auf eine Leinwand, ein Pixel des Bilds auf `f`
 * Pixel der Leinwand, seine linke obere Ecke bei (`x`, `y`), auf ganze Pixel
 * gerundet: Bei ganzem `f` trifft so jedes Pixel des Bilds ganze Pixel. Ab
 * f = 1 ungeglättet, darunter geglättet. Siehe docs/tablett-gerendert.md,
 * „Im Skin“.
 */
export function maleBild(ctx: CanvasRenderingContext2D, bild: ImageBitmap, f: number, x: number, y: number): void {
  const [x0, y0] = [Math.round(x), Math.round(y)];
  // Nur der Teil auf der Leinwand: Tief in der Karte wäre das ganze Bild
  // Millionen Pixel breit.
  const [sx, sy] = [Math.max(0, Math.floor(-x0 / f)), Math.max(0, Math.floor(-y0 / f))];
  const [ex, ey] = [Math.min(bild.width, Math.ceil((ctx.canvas.width - x0) / f)), Math.min(bild.height, Math.ceil((ctx.canvas.height - y0) / f))];
  if (ex <= sx || ey <= sy) return;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.imageSmoothingEnabled = f < 1;
  ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(bild, sx, sy, ex - sx, ey - sy, x0 + sx * f, y0 + sy * f, (ex - sx) * f, (ey - sy) * f);
}
