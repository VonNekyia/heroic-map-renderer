/**
 * Welcher Block an einem Bildpunkt der Karte zu sehen ist.
 *
 * Die Projektion des Renderers wirft die Blickachse (b, 2a, b) auf einen
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
 * Die Zahlen der Projektion in Pixeln der feinsten Stufe, wie `projection`
 * in `map.json`: `u` ist h, `v` ist a, `y` ist b. Diagonal ist u = x − z
 * und v = x + z, genordet u = x und v = z.
 * Siehe docs/benutzung/map-json.md, „Kamera und Projektion“.
 */
export interface Projektion {
  azimuth: 'diagonal' | 'north';
  u: number;
  v: number;
  y: number;
}

/** Die Projektion von 2:1, für Bäume ohne `projection`. */
export function zweiZuEins(scale: number): Projektion {
  return { azimuth: 'diagonal', u: scale / 2, v: scale / 4, y: scale / 2 };
}

/** Der Bildpunkt der Ecke (x, y, z), wie `Projection::project_block`. */
export function projiziere(x: number, y: number, z: number, p: Projektion): [number, number] {
  if (p.azimuth === 'north') return [x * p.u, z * p.v - y * p.y];
  return [(x - z) * p.u, (x + z) * p.v - y * p.y];
}

/**
 * Die Würfel entlang des Strahls durch den Bildpunkt (px, py) der
 * feinsten Stufe, von vorn nach hinten, von Höhe `maxY` bis `minY`.
 *
 * Abgetastet wird die Mitte des Pixels wie im Renderer. Je Schicht geht
 * der Strahl von oben nach unten durch das Würfelgitter. Gerechnet wird
 * ganzzahlig, mal 4·h·a, damit eine Mitte genau auf einer Kante auch genau
 * dort liegt. Dort entscheidet die Füllregel des Renderers: Sie gibt den
 * Pixel der Fläche rechts der Kante. Das gleicht einer Mitte, die um ein
 * unendlich kleines Stück nach rechts rückt; x wächst dabei, z fällt.
 * Siehe docs/frontend.md, „Koordinaten“.
 */
export function strahl(
  px: number,
  py: number,
  p: Projektion,
  minY: number,
  maxY: number,
): Block[] {
  if (p.azimuth === 'north') return genordet(px, py, p, minY, maxY);
  const { u: h, v: a, y: b } = p;
  const i = 2 * Math.floor(px) + 1;
  const j = 2 * Math.floor(py) + 1;
  // Auf Höhe t liegt der Strahl bei x = (X0 + s·t)/n und z = (Z0 + s·t)/n.
  const n = 4 * h * a;
  const s = 2 * h * b;
  const X0 = a * i + h * j;
  const Z0 = h * j - a * i;
  const bloecke: Block[] = [];
  for (let y = maxY; y >= minY; y--) {
    let X = X0 + s * (y + 1);
    let Z = Z0 + s * (y + 1);
    let x = Math.floor(X / n);
    let z = Math.ceil(Z / n) - 1;
    bloecke.push([x, y, z]);
    // Bis zum Boden der Schicht fallen X und Z um s. Wer zuerst die
    // untere Grenze seines Würfels erreicht, wechselt; beide zugleich
    // nie, denn senkrechte Kanten liegen nie auf einer Pixelmitte. Mit der
    // gerückten Mitte liegt X knapp über, Z knapp unter dem gerechneten
    // Wert: Eine Grenze in z genau am Boden zählt noch, eine in x nicht.
    let rest = s;
    for (;;) {
      const dx = X - x * n;
      const dz = Z - z * n;
      const d = Math.min(dx, dz);
      if (dx < dz ? dx >= rest : dz > rest) break;
      X -= d;
      Z -= d;
      rest -= d;
      if (dx < dz) x--;
      else z--;
      bloecke.push([x, y, z]);
    }
  }
  return bloecke;
}

/**
 * Der Strahl genordet: x steht je Pixel fest, z wandert entlang der Achse
 * (0, a, b). Ganzzahlig mal 2·a gerechnet; eine Mitte liegt nie auf einer
 * Kante, denn Zähler und Grenzen sind einmal ungerade, einmal gerade.
 */
function genordet(
  px: number,
  py: number,
  { u: h, v: a, y: b }: Projektion,
  minY: number,
  maxY: number,
): Block[] {
  const x = Math.floor((2 * Math.floor(px) + 1) / (2 * h));
  // Auf Höhe t liegt der Strahl bei z = (j + s·t)/n.
  const j = 2 * Math.floor(py) + 1;
  const n = 2 * a;
  const s = 2 * b;
  const bloecke: Block[] = [];
  for (let y = maxY; y >= minY; y--) {
    let Z = j + s * (y + 1);
    let z = Math.floor(Z / n);
    bloecke.push([x, y, z]);
    // Bis zum Boden der Schicht fällt Z um s.
    let rest = s;
    while (Z - z * n < rest) {
      rest -= Z - z * n;
      Z = z * n;
      z--;
      bloecke.push([x, y, z]);
    }
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
 * Ecke. Von oben (b = 0) bleibt die Oberseite, diagonal als Raute,
 * genordet als Quadrat.
 */
export function umriss([x, y, z]: Block, p: Projektion): [number, number][][] {
  const ecke = (dx: number, dy: number, dz: number) => projiziere(x + dx, y + dy, z + dz, p);
  const oben = ecke(0, 1, 0);
  const rechtsOben = ecke(1, 1, 0);
  const vorn = ecke(1, 1, 1);
  const linksOben = ecke(0, 1, 1);
  if (p.y === 0) return [[oben, rechtsOben, vorn, linksOben, oben]];
  // Genordet zeigt die Kamera Oberseite und Südseite: das Rechteck und die
  // Kante zwischen beiden.
  if (p.azimuth === 'north') {
    return [
      [oben, rechtsOben, ecke(1, 0, 1), ecke(0, 0, 1), oben],
      [linksOben, vorn],
    ];
  }
  const rechtsUnten = ecke(1, 0, 0);
  const unten = ecke(1, 0, 1);
  const linksUnten = ecke(0, 0, 1);
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
