/**
 * Welcher Block an einem Bildpunkt der Karte zu sehen ist.
 *
 * Die Projektion des Renderers wirft die Blickachse (1, 1, 1) auf einen
 * Punkt: Ein Bildpunkt kann jeden Würfel entlang seines Strahls zeigen.
 * Welcher es ist, entscheidet die Höhe der Spalten, durch die der Strahl
 * geht. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/frontend.md, „Koordinaten“.
 */

/** Ein Block in den Koordinaten des Spiels. */
export type Block = readonly [x: number, y: number, z: number];

/** Kantenlänge einer Region in Blöcken, wie im Spiel. */
export const REGION = 512;

/**
 * Die Würfel entlang des Strahls durch den Bildpunkt (u, v) der feinsten
 * Stufe, von vorn nach hinten, von Höhe `maxY` bis `minY`.
 *
 * Abgetastet wird die Mitte des Pixels wie im Renderer. Der Strahl ist
 * `(x0 + t, t, z0 + t)`; in jeder Schicht kreuzt er je einmal eine
 * x- und eine z-Grenze, trifft also drei Würfel. Die Nachkommaanteile von
 * x0 und z0 sind dort nie 0 und nie gleich, eine Kante trifft er nie.
 */
export function strahl(u: number, v: number, scale: number, minY: number, maxY: number): Block[] {
  const mu = Math.floor(u) + 0.5;
  const mv = Math.floor(v) + 0.5;
  const x0 = (mu + 2 * mv) / scale;
  const z0 = (2 * mv - mu) / scale;
  const fx = Math.floor(x0);
  const fz = Math.floor(z0);
  const xZuerst = x0 - fx > z0 - fz;
  const bloecke: Block[] = [];
  for (let y = maxY; y >= minY; y--) {
    const x = fx + y;
    const z = fz + y;
    bloecke.push([x + 1, y, z + 1], xZuerst ? [x + 1, y, z] : [x, y, z + 1], [x, y, z]);
  }
  return bloecke;
}

/**
 * Der vorderste Würfel des Strahls, dessen Spalte bis zu ihm hinauf
 * gefüllt ist. `hoehe` gibt das Y des obersten gezeichneten Blocks einer
 * Spalte, `undefined` für eine leere.
 */
export function pick(
  bloecke: readonly Block[],
  hoehe: (x: number, z: number) => number | undefined,
): Block | undefined {
  return bloecke.find(([x, y, z]) => y <= (hoehe(x, z) ?? -Infinity));
}

/**
 * Die sichtbaren Kanten eines Würfels in Pixeln der feinsten Stufe, wie
 * der Auswahlrahmen im Spiel: der Umriss und die drei Kanten der vorderen
 * Ecke.
 */
export function umriss([x, y, z]: Block, scale: number): [number, number][][] {
  const h = scale / 2;
  const q = scale / 4;
  // Die hintere obere Ecke (x, y+1, z) ist der oberste Punkt.
  const sx = (x - z) * h;
  const sy = (x + z) * q - (y + 1) * h;
  const oben: [number, number] = [sx, sy];
  const rechtsOben: [number, number] = [sx + h, sy + q];
  const rechtsUnten: [number, number] = [sx + h, sy + 3 * q];
  const unten: [number, number] = [sx, sy + scale];
  const linksUnten: [number, number] = [sx - h, sy + 3 * q];
  const linksOben: [number, number] = [sx - h, sy + q];
  const vorn: [number, number] = [sx, sy + h];
  return [
    [oben, rechtsOben, rechtsUnten, unten, linksUnten, linksOben, oben],
    [linksOben, vorn, rechtsOben],
    [vorn, unten],
  ];
}

/**
 * Die Region einer Spalte und der Platz ihrer Zelle in der Höhenkarte der
 * Region. Eine Zelle fasst `zelle` × `zelle` Spalten zusammen.
 */
export function region(
  x: number,
  z: number,
  zelle: number,
): { rx: number; rz: number; i: number } {
  const n = REGION / zelle;
  const cx = Math.floor(x / zelle);
  const cz = Math.floor(z / zelle);
  const rx = Math.floor(cx / n);
  const rz = Math.floor(cz / n);
  return { rx, rz, i: (cz - rz * n) * n + (cx - rx * n) };
}
