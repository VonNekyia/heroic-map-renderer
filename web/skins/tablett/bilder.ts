/**
 * Was der Skin über die Bilder in bilder/ weiss: die Masse des Tabletts in
 * w, und je Bild, wohin es gehört. Die Bilder schneidet
 * werkzeug/ausschnitte.py aus der Vorlage; die Zahlen hier nennt es nach dem
 * Schneiden. Nur Typen werden importiert, damit Node die Datei lädt.
 * Siehe docs/tablett.md, „Bilder aus der Vorlage“.
 */
import type { Projektion } from 'heroic-map-renderer/skin-api';

export type Vektor = [number, number, number];

/**
 * Die Breite w der Oberkante als Anteil der Kante der Welt, gemessen an der
 * Vorlage. Siehe docs/tablett.md, „Masse“.
 */
export const RAND = 0.016;

/** Masse in w. */
export const MASS = {
  /** Vom Wasserspiegel bis zur Platte. */
  tiefe: 5.4,
  /** Die Pfeiler an den Ecken, im Quadrat. */
  pfeiler: 2.2,
  /** Die Eckstücke innen, im Quadrat. */
  eck: 4.5,
};

/** Die Breite der Karte in der Vorlage, in ihren Pixeln. Daran messen sich die Ausschnitte. */
export const BREITE_VORLAGE = 1288.3;

/** Die Grösse der Vorlage in Pixeln. */
export const VORLAGE: [number, number] = [1491, 1055];

/**
 * So viele Pixel reicht das Bild des Tischs rundum über die Vorlage hinaus;
 * darin laufen ihre Farben in den Marmor aus.
 */
export const TISCH_RAND = 24;

/** Die Seite des Marmors in Pixeln der Vorlage: Er wiederholt sich nahtlos über die ganze Ebene. */
export const MARMOR = 768;

/**
 * Die Kamera des Bezugsrahmens, 8:5, als Verhältnis von u, v und y. Darin
 * liegen Tisch und Gegenstände in der Gesamtansicht wie in der Vorlage.
 * Siehe docs/tablett.md, „Bilder aus der Vorlage“.
 */
export const BEZUG = { u: 8, v: 5, y: 8 };

/** Eine Richtung im Blick, waagrecht. */
export type Richtung = '+x' | '-x' | '+z' | '-z';

/**
 * Je Seite im Blick die Bilder ihres Bands und ihrer Wand, aus der Seite der
 * Vorlage, die dort liegt: vorn links (+z), vorn rechts (+x), hinten links
 * (−x), hinten rechts (−z). Wände haben nur die nahen Seiten; die anderen
 * zeigt keine Kamera.
 */
export const SEITEN: Record<Richtung, { band: string; wand?: string }> = {
  '+z': { band: 'band-vl', wand: 'wand-vl' },
  '+x': { band: 'band-vr', wand: 'wand-vr' },
  '-x': { band: 'band-hl' },
  '-z': { band: 'band-hr' },
};

/** Die Seiten eines Pfeilers, die eine Kamera sieht, aus dem vorderen Pfeiler der Vorlage. */
export const PFEILER: Partial<Record<Richtung, string>> = { '+z': 'pfeiler-links', '+x': 'pfeiler-rechts' };

/** Eine Ecke der Welt im Blick, diagonal benannt nach ihrer Lage im Bild. */
export type Ecke = 'hinten' | 'rechts' | 'vorn' | 'links';

/**
 * Je Ecke ihr Eckstück aus der Vorlage: Innen ist der Rahmen an den Ecken
 * rund und deckt die Ecke der Karte. Das Bild liegt auf dem Wasserspiegel,
 * MASS.eck von der Ecke nach innen, seine Breite entlang x, seine Höhe
 * entlang z. Siehe docs/tablett.md, „Die Ecken“.
 */
export const ECKSTUECKE: Record<Ecke, string> = {
  hinten: 'eck-hinten',
  rechts: 'eck-rechts',
  vorn: 'eck-vorn',
  links: 'eck-links',
};

/**
 * Ein freigestelltes Bild: seine Grösse in Pixeln und der Punkt darin, der
 * auf seinem Fuss in der Welt steht.
 */
export interface Ausschnitt {
  bild: string;
  groesse: [number, number];
  fuss: [number, number];
}

/**
 * Ein Feld für Text auf einem Bild: Ecke `o` unten links, `a` entlang der
 * Zeile, `b` nach oben, in Pixeln der Vorlage vom Fuss des Bilds aus.
 */
export interface Feld {
  o: [number, number];
  a: [number, number];
  b: [number, number];
}

/**
 * Ein Gegenstand auf dem Tisch: sein Bild, aufrecht, und wo sein Fuss in
 * der Vorlage steht, in ihren Pixeln. Der Skin stellt ihn dorthin auf die
 * Platte, wo er im Bezugsrahmen über diesem Punkt liegt. `schrift` nennt
 * Felder für Texte aus der Konfiguration des Builds, nach ihrem Namen in
 * `Kontext.texte`.
 */
export interface Gegenstand extends Ausschnitt {
  vorlage: [number, number];
  schrift?: Record<string, Feld>;
}

/**
 * Die Gegenstände der Vorlage: Bücher mit Messingsäule und Gänseblümchen,
 * die Kerze im Leuchter, das Kästchen, der Kompass auf dem grossen Buch mit
 * dem roten Tuch und die Armillarsphäre mit Gänseblümchen. Auf den Rücken
 * zweier Bücher steht Text aus `SKIN_TEXT_BUCH1` und `SKIN_TEXT_BUCH2`;
 * siehe docs/tablett.md, „Gegenstände“.
 */
export const GEGENSTAENDE: Gegenstand[] = [
  {
    bild: 'buecher',
    groesse: [506, 295],
    fuss: [238, 214],
    vorlage: [230, 206],
    schrift: {
      buch1: { o: [-150, 8], a: [64, -38.4], b: [1.2, -25] },
      buch2: { o: [-160, -124], a: [80, -48], b: [1, -20] },
    },
  },
  { bild: 'kerze', groesse: [126, 263], fuss: [61, 255], vorlage: [1417, 263] },
  { bild: 'kaestchen', groesse: [59, 140], fuss: [22, 132], vorlage: [1462, 626] },
  { bild: 'kompass', groesse: [244, 367], fuss: [25, 142], vorlage: [1280, 838] },
  { bild: 'sphaere', groesse: [342, 426], fuss: [138, 413], vorlage: [130, 1050] },
];

/**
 * Die Lilien, je Ecke ihre aus der Vorlage mit dem Deckel ihres Pfeilers;
 * ihr Fuss ist die Mitte des Deckels.
 */
export const LILIEN: Record<Ecke, Ausschnitt> = {
  hinten: { bild: 'lilie-hinten', groesse: [63, 52], fuss: [30.8, 29.5] },
  rechts: { bild: 'lilie-rechts', groesse: [57, 57], fuss: [34.4, 36.8] },
  vorn: { bild: 'lilie-vorn', groesse: [63, 74], fuss: [31.5, 53.8] },
  links: { bild: 'lilie-links', groesse: [59, 61], fuss: [22.7, 41.4] },
};

/**
 * Das Licht nach der Vermessung der Vorlage: von oben, leicht von links im
 * Bild, 77° über der Tischebene, fest im Blick. In den Bildern ist es
 * eingebacken; der Skin braucht es nur für Flächen ohne Bild. `oben` zählt
 * entlang der Normalen der Platte, `rechts` nach rechts im Bild.
 * Siehe docs/tablett.md, „Licht“.
 */
const LICHT = { oben: 0.975, rechts: -0.223 };
export const UMGEBUNG = 0.22;
export const DIFFUS = 0.8;

export const plus = (u: Vektor, v: Vektor): Vektor => [u[0] + v[0], u[1] + v[1], u[2] + v[2]];
export const mal = (s: number, v: Vektor): Vektor => [s * v[0], s * v[1], s * v[2]];
export const einheit = (v: Vektor): Vektor => mal(1 / Math.hypot(...v), v);

/** Licht und Blick im Blick: `kamera` zeigt zur Kamera. */
export function lichtUndBlick(p: Projektion) {
  const genordet = p.azimuth === 'north';
  const kamera = einheit(genordet ? [0, p.v, p.y] : [p.y, 2 * p.v, p.y]);
  // Nach rechts im Bild, in der Welt waagrecht.
  const rechts: Vektor = genordet ? [1, 0, 0] : [Math.SQRT1_2, 0, -Math.SQRT1_2];
  const licht = plus([0, LICHT.oben, 0], mal(LICHT.rechts, rechts));
  return { kamera, licht };
}
