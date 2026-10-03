/**
 * Was der Skin und das Skript in werkzeug/ teilen: die Masse des Tabletts
 * in w, das Licht und den Atlas, also welche Bilder es je Dichte gibt und wo
 * sie liegen. Nur Typen werden importiert, damit Node die Datei lädt.
 * Siehe docs/tablett.md, „Texturen“.
 */
import type { Projektion } from 'heroic-map-renderer/skin-api';

export type Vektor = [number, number, number];

/**
 * Die Breite w der Oberkante als Anteil der Kante der Welt, nach der
 * Vermessung der Vorlage. Siehe docs/tablett.md, „Masse“.
 */
export const RAND = 0.013;

/** Masse in w. */
export const MASS = {
  /** Vom Wasserspiegel bis zur Platte. */
  tiefe: 6.4,
  /** Die Pfeiler an den Ecken, im Quadrat. */
  pfeiler: 2.7,
  /** Die Holzkante des Tischs, 5 % der Kante der Welt. */
  tischkante: 0.05 / RAND,
  /** Nach so viel wiederholen sich Band, Wand und Holzkante des Tischs. */
  periode: 20,
};

/** Texel je w, für die es einen Atlas gibt. */
export const DICHTEN = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16] as const;

/**
 * Das Licht nach der Vermessung der Vorlage: von oben, leicht von links im
 * Bild, 77° über der Tischebene, fest im Blick, so dass es aus jeder
 * Richtung gleich aussieht. `oben` zählt entlang der Normalen der Platte,
 * `rechts` nach rechts im Bild. Siehe docs/tablett.md, „Licht und Schatten“.
 */
const LICHT = { oben: 0.975, rechts: -0.223 };
export const UMGEBUNG = 0.22;
export const DIFFUS = 0.8;

export const plus = (u: Vektor, v: Vektor): Vektor => [u[0] + v[0], u[1] + v[1], u[2] + v[2]];
export const mal = (s: number, v: Vektor): Vektor => [s * v[0], s * v[1], s * v[2]];
export const einheit = (v: Vektor): Vektor => mal(1 / Math.hypot(...v), v);

/**
 * Licht und Blick im Blick: `kamera` zeigt zur Kamera, `halb` liegt zwischen
 * Licht und Kamera, für das Glanzlicht. Siehe docs/tablett.md, „Licht und
 * Schatten“.
 */
export function lichtUndBlick(p: Projektion) {
  const genordet = p.azimuth === 'north';
  const kamera = einheit(genordet ? [0, p.v, p.y] : [p.y, 2 * p.v, p.y]);
  // Nach rechts im Bild, in der Welt waagrecht.
  const rechts: Vektor = genordet ? [1, 0, 0] : [Math.SQRT1_2, 0, -Math.SQRT1_2];
  const licht = plus([0, LICHT.oben, 0], mal(LICHT.rechts, rechts));
  return { kamera, licht, halb: einheit(plus(licht, kamera)), umgebung: UMGEBUNG, diffus: DIFFUS };
}

/**
 * Die Zeilen der Wand von oben, in w ab dem Wasserspiegel: bis hier die
 * obere Leiste, der Fries, die untere Leiste und die Fuge; darunter der
 * Sockel bis zur Platte. Siehe docs/tablett.md, „Masse“.
 */
export const WAND = { leiste: 0.6, fries: 3.28, leiste2: 3.63, fuge: 4.4 };

/** Gewünschte Länge eines Felds am Fries in w; je Seite liegen ganze Felder. */
export const FELD = 3.4;

/** So weit reicht der Rahmen eines Felds vom Stoss zweier Felder, in w. */
export const STOSS = 0.3;

/**
 * Der Stoss zweier Felder am Fries in Texeln: seine halbe Breite, die
 * erste Zeile der Wand, die er deckt, und die Zeile hinter der letzten. Er
 * reicht ein Texel über den Rahmen hinaus, denn auch dort kippt der Rahmen
 * das Licht. Was er an der Wand nicht ändert, ist in seinem Bild
 * durchsichtig.
 */
export function stoss(dichte: number): { halb: number; von: number; bis: number } {
  return {
    halb: Math.ceil(STOSS * dichte) + 1,
    von: Math.floor(WAND.leiste * dichte) - 1,
    bis: Math.ceil(WAND.fries * dichte),
  };
}

/** Eine Richtung im Blick, waagrecht. */
export type Richtung = '+x' | '-x' | '+z' | '-z';

/** Was ein Bild zeigt. Siehe docs/tablett.md, „Texturen“. */
export type Rolle = 'oben' | 'wand' | 'stoss' | 'pfeiler' | 'kappe' | 'tischkante';

/**
 * Ein Bild im Atlas: Rolle, Blick und Richtung der Fläche, Breite und Höhe
 * in w. Die Breite läuft entlang der Kante a der Fläche, die Höhe entlang b.
 */
export interface Bild {
  name: string;
  rolle: Rolle;
  azimuth: Projektion['azimuth'];
  /** Bei Bändern die Seite nach aussen, bei Wänden und Pfeilern die Normale. */
  richtung?: Richtung;
  breite: number;
  hoehe: number;
  /** Wiederholt sich entlang der Breite. */
  periodisch: boolean;
}

/** Der Name des Bilds einer Fläche. */
export const bildName = (azimuth: Projektion['azimuth'], rolle: Rolle, richtung?: Richtung) =>
  [azimuth, rolle, richtung].filter(Boolean).join(' ');

/**
 * Die Bilder, die ein Blick zeigen kann: diagonal sieht die Kamera die
 * Seiten nach +x und +z, genordet nur nach +z. Band, Wand und Holzkante
 * des Tischs wiederholen sich; die Stösse des Frieses legt der Skin
 * einzeln darauf.
 */
export const BILDER: Bild[] = (['diagonal', 'north'] as const).flatMap((azimuth) => {
  const vorn: Richtung[] = azimuth === 'diagonal' ? ['+x', '+z'] : ['+z'];
  const bild = (rolle: Rolle, richtung: Richtung | undefined, breite: number, hoehe: number, periodisch = false): Bild => ({
    name: bildName(azimuth, rolle, richtung),
    rolle,
    azimuth,
    richtung,
    breite,
    hoehe,
    periodisch,
  });
  return [
    ...(['+x', '-x', '+z', '-z'] as const).map((r) => bild('oben', r, MASS.periode, 1, true)),
    ...vorn.map((r) => bild('wand', r, MASS.periode, MASS.tiefe, true)),
    ...vorn.map((r) => bild('stoss', r, 2 * STOSS, WAND.fries - WAND.leiste)),
    ...vorn.map((r) => bild('pfeiler', r, MASS.pfeiler, MASS.tiefe)),
    bild('kappe', undefined, MASS.pfeiler, MASS.pfeiler),
    ...(['+x', '+z'] as const).map((r) => bild('tischkante', r, MASS.periode, MASS.tischkante, true)),
  ];
});

/**
 * Der Anschnitt eines Bilds in Texeln, entlang seiner Breite und Höhe: je
 * Seite ein Texel, eine Kopie der Kante, wo das Bild nicht weiterläuft. Er
 * deckt, was frei bliebe, wenn der Skin das Texelgitter auf die Pixel rückt.
 * Ein Bild, das sich wiederholt, läuft entlang seiner Breite weiter; der
 * Stoss ist ringsum durchsichtig. Siehe docs/tablett.md, „Ganze Pixel“.
 */
export function anschnitt(bild: Bild): [u: number, v: number] {
  if (bild.rolle === 'stoss') return [0, 0];
  return [bild.periodisch ? 0 : 1, 1];
}

/** Wie gross ein Bild bei einer Dichte ist, in ganzen Texeln, mit Anschnitt. */
export function groesse(bild: Bild, dichte: number): [breite: number, hoehe: number] {
  if (bild.rolle === 'stoss') {
    const { halb, von, bis } = stoss(dichte);
    return [2 * halb, bis - von];
  }
  const [au, av] = anschnitt(bild);
  return [Math.ceil(bild.breite * dichte) + 2 * au, Math.ceil(bild.hoehe * dichte) + 2 * av];
}

/** Wo ein Bild im Atlas liegt, in Texeln. */
export interface Bereich {
  x: number;
  y: number;
  breite: number;
  hoehe: number;
  periodisch: boolean;
}

/**
 * Der Atlas einer Dichte: Die Bilder liegen der Reihe nach in Zeilen, so
 * breit wie das breiteste.
 */
export function atlas(dichte: number): { breite: number; hoehe: number; bereiche: Map<string, Bereich> } {
  const breite = Math.max(...BILDER.map((bild) => groesse(bild, dichte)[0]));
  const bereiche = new Map<string, Bereich>();
  let [x, y, zeile] = [0, 0, 0];
  for (const bild of BILDER) {
    const [b, h] = groesse(bild, dichte);
    if (x + b > breite) [x, y, zeile] = [0, y + zeile, 0];
    bereiche.set(bild.name, { x, y, breite: b, hoehe: h, periodisch: bild.periodisch });
    x += b;
    zeile = Math.max(zeile, h);
  }
  return { breite, hoehe: y + zeile, bereiche };
}
