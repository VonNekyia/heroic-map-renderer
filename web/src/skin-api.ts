/**
 * Die Schnittstelle zwischen der Grundkarte und einem Skin, nur Typen und
 * die Version. Ein Skin importiert sie als `heroic-map-renderer/skin-api`,
 * nur mit `import type`; sonst aus der Grundkarte nichts als Leaflet. Wer
 * hier etwas ändert, hebt `VERSION`. Siehe docs/frontend.md, „Skins“.
 */
import type L from 'leaflet';

/** Die Version dieser Schnittstelle; ein Skin vergleicht sie mit `Kontext.version`. */
export const VERSION = 2;

/**
 * Die CSS-Variablen der UI, die ein Skin in seinem Stylesheet am Container
 * setzt; ihre Vorgaben und was sie färben, stehen in `web/src/style.css`.
 * Bilder hängt er an die Klassen der UI: `leaflet-bar` mit den Knöpfen für
 * Zoom und ganze Karte, `kompass`, `baeume`, `leiste` mit den Koordinaten
 * und `stand`.
 */
export type UiVariable =
  | '--karte-grund'
  | '--ui-grund'
  | '--ui-schrift'
  | '--ui-knopf'
  | '--ui-knopf-darueber'
  | '--ui-knopf-aus'
  | '--ui-schrift-aus'
  | '--ui-trenner'
  | '--ui-rand'
  | '--ui-radius'
  | '--ui-markiert'
  | '--ui-falsch'
  | '--ui-falsch-grund'
  | '--ui-schrift-zahlen'
  | '--ui-schrift-text'
  | '--ui-abstand';

/**
 * Die Zahlen der Projektion in Pixeln der feinsten Stufe, wie `projection`
 * in `map.json`: `u` ist h, `v` ist a, `y` ist b.
 */
export interface Projektion {
  azimuth: 'diagonal' | 'north';
  u: number;
  v: number;
  y: number;
}

/** Ein Rechteck der Welt in Blöcken, `[x0, z0, x1, z1]`; x1 und z1 sind Kanten dahinter. */
export type Rechteck = [x0: number, z0: number, x1: number, z1: number];

/** Ein Rechteck im Bild in Pixeln der feinsten Stufe, `[links, oben, rechts, unten]`. */
export type Grenzen = [links: number, oben: number, rechts: number, unten: number];

/**
 * Was die Grundkarte einem Skin reicht. Punkte der Karte sind Pixel der
 * feinsten Stufe; `karte.unproject([x, y], maxZoom)` macht daraus einen
 * Ort für Leaflet.
 */
export interface Kontext {
  /** `VERSION` der Grundkarte. */
  version: number;
  karte: L.Map;
  /** Der Container der Karte; ein Skin setzt hier seine Klasse. */
  container: HTMLElement;
  projektion: Projektion;
  /** Vierteldrehungen des Blicks; ein Punkt dreht je Schritt mit (x, z) → (z, −x). */
  k: number;
  /** Der Bildpunkt eines Punkts im Blick, in Pixeln der feinsten Stufe. */
  projiziere: (x: number, y: number, z: number) => [number, number];
  /** Die feinste Stufe mit Kacheln. */
  maxZoom: number;
  /** Aus `map.json`, ungeprüft. */
  area?: Rechteck;
  seaLevel?: number;
  /** Aus `map.json`, ohne Angabe −64. */
  minY: number;
  /** Die feinste ganze Stufe, auf der `grenzen` in ein Fenster dieser Grösse passen. */
  fitZoom: (grenzen: Grenzen, breite: number, hoehe: number) => number;
  /**
   * Texte aus der Build-Konfiguration: `titel` aus `SITE_TITLE`, dazu je
   * `SKIN_TEXT_<NAME>` ein Eintrag `<name>` in Kleinbuchstaben.
   */
  texte: Readonly<Record<string, string>>;
}

/** Was ein Skin zurückgibt: die Grenzen, die als ganze Karte gelten. */
export interface Antwort {
  ganzeKarte: Grenzen;
}

/** Ein Skin, der Default-Export seines Moduls. Ohne Antwort bleibt die Karte, wie sie ist. */
export type Skin = (kontext: Kontext) => Antwort | undefined;
