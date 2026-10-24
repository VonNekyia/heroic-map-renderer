/**
 * Ebenen über den Kacheln: die Liste zum Umschalten, Nadeln als Wappenschild
 * und ihre Infotafel, Regionen, Kreise, Linien und Kartenschrift. Das Format
 * steht in docs/benutzung/ebenen.md, wie die Karte es zeigt in
 * docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import feldGross from './ebenen/schild_gross.png?url&no-inline';
import rahmenGross from './ebenen/schild_gross_rahmen.png?url&no-inline';
import feldKlein from './ebenen/schild_klein.png?url&no-inline';
import rahmenKlein from './ebenen/schild_klein_rahmen.png?url&no-inline';
import feldMittel from './ebenen/schild_mittel.png?url&no-inline';
import rahmenMittel from './ebenen/schild_mittel_rahmen.png?url&no-inline';
import { bereich, flaechenZiele, form, hatFlaeche, versetze, zeichneAnsicht, zuege, type Form, type Strich } from './formen';
import { BLOCK, bildpunkt, blockBreite, blockHoch, oberflaeche as hoeheAuf, rechteck, schriftPfad, weltRechteck, zurKamera, type Blick, type Gelaende, type Rechteck, type Schirm } from './gelaende';
import { FRISCH, HOEHEN_BYTES, LEER, regionBytes, type Hoehenkarten } from './hoehen';
import { REGION } from './pick';
import { abmelden, aktion, anmelden, ebene as spielerEbene, erkennen, ich, knoepfe as holeKnoepfeVomServer, nimmToken, objektSchluessel, sicht, type Knopf, type Spieler } from './konto';
import { farbe, istObjekt, istText, istZahl, punkt } from './pruefen';
import { KAPPE, ladeSchrift, Schrift, schriftzug, SVG, type Schriftzug } from './schrift';

/** Ein Eintrag in `layers.json`. */
interface Eintrag {
  id: string;
  name: { de?: string; en?: string };
  visible: boolean;
  order: number;
  version: string;
  /** Eine Aktionsebene: ihre Datei kommt je Spieler aus `api/`, nur angemeldet. */
  player: boolean;
}

type Groesse = 'large' | 'medium' | 'small';

/** Eine Nadel, wie die Ansicht sie braucht; Unbekanntes ist schon weg. */
interface Nadel {
  id: string;
  at: [number, number];
  y?: number;
  name?: string;
  size: Groesse;
  symbol: { large?: string; medium?: string };
  color: string;
  /** Der Name eines Kopfs unter `koepfe/`; taugt er, zeigt die Karte das Gesicht statt der Nadel. */
  head?: string;
  panel?: unknown[];
}

/** Ein Banner: ein Bild der Ebene, Pixel auf Pixel. Siehe docs/benutzung/ebenen.md, „Banner“. */
interface Banner {
  id: string;
  at: [number, number];
  y?: number;
  /** Ein Entwurf der Ebene; dann zeigt die Karte das Sprite ihres Satzes, `image` ist Ersatz. */
  design?: string;
  capital: boolean;
  image?: string;
  name?: string;
  panel?: unknown[];
}

/** Ein Satz von Sprites aus `satz.json`: Fuss im Sprite und Winkel der Unterkante in Grad. Siehe docs/benutzung/ebenen.md, „Sprites“. */
interface Satz {
  ordner: string;
  fuss: [number, number];
  winkel: number;
}

/** Was die Ebenen von der Karte brauchen. */
export interface Umgebung {
  map: L.Map;
  /** Die Wurzel mit `trees.json`, ohne `/` am Ende. */
  wurzel: string;
  blick: Blick;
  scale: number;
  maxZoom: number;
  /** Die Höhen; ohne sie liegt im iso alles auf `seaLevel`. */
  karten?: Hoehenkarten;
  /** Die Höhen ohne Laub aus `ground`, nur mit `karten`; Formen liegen darauf, sonst auf `karten`. */
  boden?: Hoehenkarten;
  /** Kante einer Zelle von `boden` in Blöcken, `groundCell`, sonst `heightsCell`. */
  bodenCell?: number;
  heightsCell?: number;
  seaLevel?: number;
  /** Die Bauhöhe aus `map.json`; sie begrenzt, wie weit Gelände eine Form verdecken kann. */
  minY?: number;
  maxY?: number;
  /** `area` aus `map.json`; Höhen gibt es nur darin. */
  area?: Rechteck;
  /** Der Satz der Banner-Sprites dieses Baums, siehe docs/benutzung/ebenen.md, „Sprites“; ohne `trees.json` keiner. */
  satz?: string;
  /** Das Anmelde-Token aus dem Fragment, schon aus der Adresse gelöscht. */
  token?: string;
}

/**
 * Die Nadel der Karte in ihren drei Grössen: Feld in Graustufen, Rahmen mit
 * Nadel, Breite und Höhe samt Nadel, Seite des Symbols. Die Bilder stammen
 * vom Designer (#219, Teil 6). Siehe docs/benutzung/ebenen.md, „Nadel“.
 */
const SCHILDE: Record<Groesse, { feld: string; rahmen: string; b: number; h: number; symbol: number }> = {
  large: { feld: feldGross, rahmen: rahmenGross, b: 23, h: 33, symbol: 16 },
  medium: { feld: feldMittel, rahmen: rahmenMittel, b: 15, h: 23, symbol: 9 },
  small: { feld: feldKlein, rahmen: rahmenKlein, b: 9, h: 15, symbol: 0 },
};
const GROESSEN: Groesse[] = ['large', 'medium', 'small'];

/** So oft fragt die Karte `layers.json` nach, in ms. */
const TAKT = 30_000;

/**
 * Der Fehler auf dem Schirm, den Ränder, Linien und Wand höchstens haben, in
 * Pixeln; und wie lange die Karte nach dem letzten Verschieben oder Zoomen
 * wartet, bevor sie sie neu rechnet, in ms. Siehe
 * docs/entscheidungen/0111-geometrie-nach-bildschirmfehler.md.
 */
const REGEL = { eps: 0.5, warten: 150 };

/** Mit `?punkte` in der Adresse schreibt die Karte die Zahl der Punkte an das Pane der Formen, für Tests und Messungen. */
const ZAEHLEN = new URLSearchParams(location.search).has('punkte');

/** So oft fragt die Karte nach den Mitspielern, und nach so langer Zeit ohne neues `jetzt` gelten sie als veraltet, in ms. */
const SICHT = { takt: 2000, alt: 15_000 };
/** Ein Kopf: höchstens so viele Byte, so viele Pixel je Seite; siehe 0114. */
const KOPF = { bytes: 16 * 1024, seite: 8 };

/** Grenzen aus docs/benutzung/ebenen.md, „Grenzen“. */
const GRENZEN = { liste: 64 * 1024, datei: 4 * 1024 * 1024, ebenen: 64, objekte: 10_000, nadeln: 1000, bausteine: 64, bild: 512, punkte: 20, banner: [32, 64] };

/**
 * Je geladener Karte von `seite` × `seite` Zellen das Höchste je Block von
 * `kante` Zellen und ob dort eine leer ist, einmal gerechnet; es fällt mit der
 * Karte weg. Für `Gelaende.hoch`.
 */
const kartenBloecke = new WeakMap<Int16Array, Map<number, { max: Int16Array; leer: Uint8Array }>>();
function bloeckeDer(karte: Int16Array, seite: number, kante: number): { max: Int16Array; leer: Uint8Array } {
  // Je Kante eigen: Eine Karte aus heights dient im Gelände aus ground mit kleineren Blöcken.
  let jeKante = kartenBloecke.get(karte);
  if (!jeKante) kartenBloecke.set(karte, (jeKante = new Map<number, { max: Int16Array; leer: Uint8Array }>()));
  let b = jeKante.get(kante);
  if (!b) {
    const n = seite / kante;
    b = { max: new Int16Array(n * n).fill(LEER), leer: new Uint8Array(n * n) };
    for (let j = 0; j < seite; j++) {
      for (let i = 0; i < seite; i++) {
        const v = karte[j * seite + i]!;
        const k = Math.floor(j / kante) * n + Math.floor(i / kante);
        if (v === LEER) b.leer[k] = 1;
        else if (v > b.max[k]!) b.max[k] = v;
      }
    }
    jeKante.set(kante, b);
  }
  return b;
}
/** Je geladener Karte ihre höchste Oberseite, einmal gerechnet; sie fällt mit der Karte weg. */
const kartenMax = new WeakMap<Int16Array, number>();
function hoechste(karte: Int16Array): number {
  let max = kartenMax.get(karte);
  if (max === undefined) {
    max = -Infinity;
    for (const v of karte) if (v !== LEER && v + 1 > max) max = v + 1;
    kartenMax.set(karte, max);
  }
  return max;
}

/**
 * Die Panes der Nadeln, 510 bis 573: über `shadowPane` (500), unter
 * `markerPane` (600), `tooltipPane` und Tafel. Darunter die Panes der Formen
 * und Schrift, 410 bis 473: So liegen die Nadeln aller Ebenen über allem
 * anderen.
 */
const PANE_GRUND = 510;
const FORM_GRUND = 410;

/** Die Tafel als Popup, für Nadeln und Flächen gleich. */
/** So breit ist der Inhalt der Tafel höchstens, in Pixeln; style.css verkleinert Bilder auf dasselbe Mass. */
const TAFEL_BREITE = 320;
const TAFEL: L.PopupOptions = { className: 'tafel', maxWidth: TAFEL_BREITE, minWidth: 120, autoPanPadding: [8, 8] };

/** Die Tafel beim Zeigen: erscheint nach so vielen ms Ruhe, schliesst so viele ms nach dem Verlassen. Siehe docs/benutzung/ebenen.md, „Infotafel“. */
const TAFEL_AUF = 50;
const TAFEL_ZU = 300;

/**
 * Ein Teil der Kennung oder der Name eines Bilds ohne Endung, nach
 * docs/benutzung/ebenen.md, „Kennung“: 1 bis 64 Zeichen, kein `.` vorn oder
 * hinten, vor dem ersten `.` kein Gerät von Windows.
 */
function teilGilt(teil: string): boolean {
  return /^[a-z0-9_-]([a-z0-9_.-]{0,62}[a-z0-9_-])?$/.test(teil) && !/^(con|prn|aux|nul|com[0-9]|lpt[0-9])$/.test(teil.split('.')[0]!);
}

/** Eine Kennung `modname:ebene`, beide Teile nach `teilGilt`. */
function kennungGilt(id: string): boolean {
  const teile = id.split(':');
  return teile.length === 2 && teile.every(teilGilt);
}

/** Ein Bild der Ebene, siehe docs/benutzung/ebenen.md, „Bilder“: nur unter `images/`, PNG oder WebP, der Name nach `teilGilt`. */
function bildGilt(pfad: string): boolean {
  const name = /^images\/(.+)\.(png|webp)$/.exec(pfad)?.[1];
  return name !== undefined && teilGilt(name);
}

function eintrag(wert: unknown): Eintrag | undefined {
  if (!istObjekt(wert) || typeof wert.id !== 'string' || !istText(wert.version, 256)) return undefined;
  if (!kennungGilt(wert.id)) {
    console.warn(`layers.json: Kennung „${wert.id}“ gegen die Regel aus „Kennung“, übergangen`);
    return undefined;
  }
  const name = istObjekt(wert.name) ? wert.name : {};
  const de = istText(name.de, 64) ? name.de : undefined;
  const en = istText(name.en, 64) ? name.en : undefined;
  if (!de && !en) return undefined;
  return {
    id: wert.id,
    name: { de, en },
    visible: wert.visible !== false,
    order: Number.isInteger(wert.order) ? (wert.order as number) : 0,
    version: wert.version,
    player: wert.player === true,
  };
}

function nadel(wert: unknown): Nadel | undefined {
  if (!istObjekt(wert) || wert.type !== 'pin' || !istText(wert.id, 64)) return undefined;
  const at = punkt(wert.at);
  if (!at) return undefined;
  const symbol = istObjekt(wert.symbol) ? wert.symbol : {};
  const bild = (pfad: unknown) => {
    if (typeof pfad !== 'string') return undefined;
    if (bildGilt(pfad)) return pfad;
    console.warn(`Nadel ${String(wert.id)}: Symbol „${pfad}“ gegen „Bilder“, das Schild bleibt leer`);
    return undefined;
  };
  return {
    id: wert.id,
    at,
    y: Number.isInteger(wert.y) ? (wert.y as number) : undefined,
    name: istText(wert.name, 64) ? wert.name : undefined,
    size: GROESSEN.includes(wert.size as Groesse) ? (wert.size as Groesse) : 'medium',
    symbol: { large: bild(symbol.large), medium: bild(symbol.medium) },
    color: farbe(wert.color) ?? '#D9443A',
    head: typeof wert.head === 'string' && /^[0-9a-f]{64}$/.test(wert.head) ? wert.head : undefined,
    panel: istObjekt(wert.panel) && Array.isArray(wert.panel.blocks) ? wert.panel.blocks : undefined,
  };
}

function banner(wert: unknown): Banner | undefined {
  if (!istObjekt(wert) || wert.type !== 'banner' || !istText(wert.id, 64)) return undefined;
  const at = punkt(wert.at);
  if (!at) return undefined;
  const design = typeof wert.design === 'string' ? wert.design : undefined;
  if (design !== undefined && !teilGilt(design)) {
    console.warn(`Banner ${wert.id}: Entwurf „${design}“ gegen „Kennung“, übergangen`);
    return undefined;
  }
  const image = typeof wert.image === 'string' ? wert.image : undefined;
  if ((image !== undefined && !bildGilt(image)) || (image === undefined && design === undefined)) {
    console.warn(`Banner ${wert.id}: Bild „${String(wert.image)}“ gegen „Bilder“, übergangen`);
    return undefined;
  }
  return {
    id: wert.id,
    at,
    y: Number.isInteger(wert.y) ? (wert.y as number) : undefined,
    design,
    capital: wert.capital === true,
    image,
    name: istText(wert.name, 64) ? wert.name : undefined,
    panel: istObjekt(wert.panel) && Array.isArray(wert.panel.blocks) ? wert.panel.blocks : undefined,
  };
}

/**
 * Der Name eines Banners im Bogen, in CSS-Pixeln: Schriftgrösse, Sperrung
 * zwischen den Zeichen, tiefster Punkt unter dem Fuss, grösste Öffnung.
 * Siehe docs/benutzung/ebenen.md, „Nadeln und Banner“.
 */
const NAME = { s: 16, sperrung: 0.125 * 16, tief: 0.75 * 16, oeffnung: (2 * Math.PI) / 3 };

/**
 * Nadeln und Banner haben ihre volle Grösse, solange ein Block auf dem
 * Schirm mindestens `grenze` CSS-Pixel breit ist, also bis 16 Blöcke je
 * Pixel; darunter je Stufe die Hälfte, bis ihr Name kleiner als `nameMin`
 * Pixel würde. Dann fehlt er, und kleiner werden sie nicht mehr. Siehe
 * docs/entscheidungen/0116-nadeln-und-banner-unter-16-bloecken-je-pixel.md.
 */
const GROESSE = { grenze: 1 / 16, nameMin: 6 };
/** Die kleinste Stufe: die erste, auf der der Name unter `nameMin` fiele. */
const STUFE_MAX = Math.ceil(Math.log2(NAME.s / GROESSE.nameMin));
let bogenNummer = 0;

/**
 * Ein SVG mit dem Ursprung am Fuss: der Name auf einem Kreisbogen darunter,
 * jedes Zeichen aufrecht zum Bogen. `h` ist die gezeichnete Höhe des Banners,
 * `winkel` der aus `satz.json` in Grad, mit `image` 0.
 */
function nameImBogen(name: string, h: number, winkel: number): SVGSVGElement {
  const { s, sperrung, tief, oeffnung } = NAME;
  const ctx = document.createElement('canvas').getContext('2d')!;
  ctx.font = `${s}px Kartenschrift, serif`;
  const l = ctx.measureText(name).width + sperrung * ([...name].length - 1);
  const r = Math.max(2 * h, l / oeffnung);
  // Der Pfad etwas länger als der Name, damit textPath an den Enden nichts abschneidet.
  const phi = Math.min(1.8 * Math.PI, l / r + 0.6);
  const [x, y] = [r * Math.sin(phi / 2), tief - r + r * Math.cos(phi / 2)];
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('class', 'nadel-bogen');
  const g = document.createElementNS(SVG, 'g');
  g.setAttribute('transform', `rotate(${winkel})`);
  const pfad = document.createElementNS(SVG, 'path');
  const id = `nadel-bogen-${bogenNummer++}`;
  pfad.id = id;
  pfad.setAttribute('d', `M ${-x} ${y} A ${r} ${r} 0 ${phi > Math.PI ? 1 : 0} 0 ${x} ${y}`);
  const text = document.createElementNS(SVG, 'text');
  text.setAttribute('letter-spacing', String(sperrung));
  const entlang = document.createElementNS(SVG, 'textPath');
  entlang.setAttribute('href', `#${id}`);
  entlang.setAttribute('startOffset', '50%');
  entlang.setAttribute('text-anchor', 'middle');
  // Die Sperrung nach dem letzten Zeichen zählt nicht; die Grundlinie eine halbe Höhe der Grossbuchstaben unter dem Bogen.
  entlang.setAttribute('dx', String(sperrung / 2));
  entlang.setAttribute('dy', String((KAPPE * s) / 2));
  entlang.textContent = name;
  text.append(entlang);
  g.append(pfad, text);
  svg.append(g);
  return svg;
}

/** Die Bilder des Schilds; die Symbole hält jede geladene Ebene selbst. */
const schildBilder = new Map<string, Promise<HTMLImageElement>>();

/** Ein Bild vom eigenen Server, einmal je Adresse und Cache; ein Fehlschlag bleibt nicht im Cache. */
function bild(adresse: string, bilder: Map<string, Promise<HTMLImageElement>>): Promise<HTMLImageElement> {
  let laden = bilder.get(adresse);
  if (!laden) {
    laden = new Promise((fertig, fehler) => {
      const element = new Image();
      element.onload = () => fertig(element);
      element.onerror = () => fehler(new Error(`${adresse} lässt sich nicht laden`));
      element.src = adresse;
    });
    laden.catch(() => bilder.delete(adresse));
    bilder.set(adresse, laden);
  }
  return laden;
}

/**
 * Zeichnet die Nadel: das Feld mal `color` je Kanal, abgeschnitten, dann das
 * Symbol mit der linken oberen Ecke bei (⌊(b − Seite) / 2⌋, 3), zuletzt
 * Rahmen und Nadel. Ein Symbol in falscher Grösse bleibt weg. Siehe
 * docs/benutzung/ebenen.md, „Nadel“.
 */
async function zeichneNadel(
  groesse: Groesse,
  farbe: string,
  symbol: string | undefined,
  symbole: Map<string, Promise<HTMLImageElement>>,
): Promise<HTMLCanvasElement> {
  const { feld, rahmen, b, h, symbol: seite } = SCHILDE[groesse];
  const leinwand = document.createElement('canvas');
  [leinwand.width, leinwand.height] = [b, h];
  const ctx = leinwand.getContext('2d')!;
  const [f, r, s] = await Promise.all([
    bild(feld, schildBilder),
    bild(rahmen, schildBilder),
    symbol ? bild(symbol, symbole).catch(() => undefined) : undefined,
  ]);
  ctx.drawImage(f, 0, 0);
  const daten = ctx.getImageData(0, 0, b, h);
  const kanal = [1, 3, 5].map((i) => Number.parseInt(farbe.slice(i, i + 2), 16));
  for (let i = 0; i < daten.data.length; i += 4) {
    for (let c = 0; c < 3; c++) daten.data[i + c] = Math.floor((daten.data[i + c]! * kanal[c]!) / 255);
  }
  ctx.putImageData(daten, 0, 0);
  if (s && (s.width !== seite || s.height !== seite)) {
    console.warn(`${symbol}: ${s.width} × ${s.height} statt ${seite} × ${seite}, das Schild bleibt leer`);
  } else if (s) {
    ctx.drawImage(s, Math.floor((b - seite) / 2), 3);
  }
  ctx.drawImage(r, 0, 0);
  return leinwand;
}

/** Der Grund der Tafel ohne Alpha; gegen ihn muss ein Titel lesbar sein. */
const TAFEL_GRUND = [16, 16, 20];

/** Relative Leuchtdichte nach WCAG 2.1 aus Kanälen 0 bis 255. */
function leuchtdichte(kanaele: readonly number[]): number {
  const [r, g, b] = kanaele.map((k) => {
    const s = k / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * Eine Titelfarbe, lesbar auf dem dunklen Grund: unter 3:1 gegen
 * `TAFEL_GRUND` je Schritt s um 10 % mit Weiss gemischt, bis sie 3:1
 * erreicht, in ganzen Zahlen ⌊(10·c + (255 − c)·s + 5) / 10⌋; Alpha bleibt.
 * Siehe docs/benutzung/ebenen.md, „Infotafel“, der Punkt Aussehen.
 */
function lesbar(farbe: string): string {
  const c = [1, 3, 5].map((i) => Number.parseInt(farbe.slice(i, i + 2), 16));
  const grund = leuchtdichte(TAFEL_GRUND);
  for (let s = 0; s < 10; s++) {
    const m = c.map((k) => Math.floor((10 * k + (255 - k) * s + 5) / 10));
    const l = leuchtdichte(m);
    if ((Math.max(l, grund) + 0.05) / (Math.min(l, grund) + 0.05) >= 3) {
      return `#${m.map((k) => k.toString(16).padStart(2, '0')).join('').toUpperCase()}${farbe.slice(7)}`;
    }
  }
  // Mit s = 10 ist sie Weiss, rund 17:1.
  return `#FFFFFF${farbe.slice(7)}`;
}

/**
 * Baut die Infotafel aus ihren Bausteinen, nur als Text und Bilder vom
 * eigenen Server, nie als Markup. Unbekannte Bausteine übergeht sie, was über
 * die Grenzen geht, nennt sie in der Konsole. `v` hängt an jeder Bildadresse,
 * damit ein neues Bild unter gleichem Namen ankommt. Siehe
 * docs/benutzung/ebenen.md, „Infotafel“.
 */
export function tafel(bausteine: unknown[], ordner: string, v: string): HTMLElement {
  let anzahl = 0;
  const groesse = (wert: unknown) => Number.isInteger(wert) && (wert as number) >= 1 && (wert as number) <= GRENZEN.bild;
  const bildElement = (b: Record<string, unknown>) => {
    if (typeof b.image !== 'string') return undefined;
    if (!bildGilt(b.image)) {
      console.warn(`${ordner}: Bild „${b.image}“ gegen „Bilder“, übergangen`);
      return undefined;
    }
    if (!groesse(b.width) || !groesse(b.height)) {
      console.warn(`${ordner}/${b.image}: width und height müssen ganze Zahlen von 1 bis ${GRENZEN.bild} sein`);
      return undefined;
    }
    const img = document.createElement('img');
    img.src = `${ordner}/${b.image}?v=${encodeURIComponent(v)}`;
    img.alt = typeof b.alt === 'string' ? b.alt : '';
    [img.width, img.height] = [b.width as number, b.height as number];
    // Verkleinert per max-width, im Verhältnis von width und height, nicht dem der Datei.
    const [breite, hoehe] = [b.width as number, b.height as number];
    img.style.aspectRatio = `${breite} / ${hoehe}`;
    // Vergrössert wie ein Banner: k = round(Faktor) Pixel des Geräts je Pixel
    // der Datei, ohne Glättung, höchstens 320 breit. Verkleinert geglättet,
    // sonst gingen Zeilen verloren. Siehe docs/frontend.md, „Ebenen“.
    img.addEventListener('load', () => {
      const [nb, nh] = [img.naturalWidth, img.naturalHeight];
      const gezeigt = Math.min(breite, TAFEL_BREITE);
      const [fb, fh] = [(gezeigt * devicePixelRatio) / nb, ((gezeigt * hoehe) / breite) * devicePixelRatio / nh];
      const kb = Math.min(Math.round(fb), Math.floor((TAFEL_BREITE * devicePixelRatio) / nb));
      if (fb < 1 || fh < 1 || kb < 1) return;
      [img.style.width, img.style.height] = [`${(nb * kb) / devicePixelRatio}px`, `${(nh * Math.round(fh)) / devicePixelRatio}px`];
      img.classList.add('ganz');
    });
    return img;
  };
  const baue = (liste: unknown[], tiefe: number): HTMLElement => {
    const teil = L.DomUtil.create('div', 'tafel-teil');
    for (const baustein of liste) {
      if (!istObjekt(baustein)) continue;
      if (++anzahl > GRENZEN.bausteine) {
        if (anzahl === GRENZEN.bausteine + 1) console.warn(`${ordner}: mehr als ${GRENZEN.bausteine} Bausteine in einer Tafel, der Rest fehlt`);
        break;
      }
      const element = ((): HTMLElement | undefined => {
        switch (baustein.type) {
          case 'title': {
            if (!istText(baustein.text, 64)) return undefined;
            const titel = L.DomUtil.create('div', 'tafel-titel');
            titel.textContent = baustein.text;
            const f = farbe(baustein.color);
            if (f) titel.style.color = lesbar(f);
            return titel;
          }
          case 'lines': {
            if (!Array.isArray(baustein.lines)) return undefined;
            const zeilen = L.DomUtil.create('div', 'tafel-zeilen');
            for (const zeile of baustein.lines) if (istText(zeile, 120)) L.DomUtil.create('div', '', zeilen).textContent = zeile;
            return zeilen;
          }
          case 'image': {
            const img = bildElement(baustein);
            if (img) img.className = `tafel-bild tafel-${['center', 'right'].includes(baustein.align as string) ? (baustein.align as string) : 'left'}`;
            return img;
          }
          case 'section': {
            if (tiefe > 0 || !istObjekt(baustein.heading) || !Array.isArray(baustein.blocks)) return undefined;
            const abschnitt = L.DomUtil.create('div', 'tafel-abschnitt');
            const kopf = bildElement(baustein.heading);
            if (kopf) abschnitt.append(kopf);
            else if (istText(baustein.heading.text, 64)) L.DomUtil.create('div', 'tafel-ueberschrift', abschnitt).textContent = baustein.heading.text;
            abschnitt.append(baue(baustein.blocks, tiefe + 1));
            return abschnitt;
          }
          case 'rating': {
            if (!Array.isArray(baustein.rows)) return undefined;
            const wertung = L.DomUtil.create('div', 'tafel-wertung');
            for (const reihe of baustein.rows) {
              if (!istObjekt(reihe) || !istText(reihe.label, 64) || !Number.isInteger(reihe.value) || !Number.isInteger(reihe.max)) continue;
              if ((reihe.max as number) < 1 || (reihe.max as number) > GRENZEN.punkte) {
                console.warn(`${ordner}: Wertung „${reihe.label}“ mit max ${String(reihe.max)}, erlaubt 1 bis ${GRENZEN.punkte}`);
                continue;
              }
              const zeile = L.DomUtil.create('div', 'tafel-reihe', wertung);
              L.DomUtil.create('span', 'tafel-label', zeile).textContent = reihe.label;
              const f = farbe(reihe.color) ?? '#888888';
              for (let i = 0; i < (reihe.max as number); i++) {
                const punkt = L.DomUtil.create('span', 'tafel-punkt', zeile);
                punkt.style.background = f;
                if (i >= (reihe.value as number)) punkt.style.opacity = '0.25';
              }
            }
            return wertung;
          }
          case 'columns': {
            if (tiefe > 0 || !Array.isArray(baustein.columns)) return undefined;
            const spalten = L.DomUtil.create('div', 'tafel-spalten');
            for (const spalte of baustein.columns.slice(0, 2)) if (Array.isArray(spalte)) spalten.append(baue(spalte, tiefe + 1));
            return spalten;
          }
          default:
            return undefined;
        }
      })();
      if (element) teil.append(element);
    }
    return teil;
  };
  return baue(bausteine, 0);
}

/**
 * Die Wahl des Betrachters, welche Ebene an ist, je Wurzel: im Speicher der
 * Seite, durchgeschrieben nach `localStorage`. Ohne ihn gilt sie bis zum
 * Neuladen.
 */
function gemerkt(wurzel: string): { lies: () => Readonly<Record<string, boolean>>; setze: (id: string, an: boolean) => void } {
  const schluessel = `ebenen:${wurzel}`;
  let wahl: Record<string, boolean> = {};
  try {
    const wert: unknown = JSON.parse(localStorage.getItem(schluessel) ?? '{}');
    if (istObjekt(wert)) wahl = wert as Record<string, boolean>;
  } catch {
    // Ohne Speicher beginnt die Wahl leer.
  }
  return {
    lies: () => wahl,
    setze: (id, an) => {
      wahl = { ...wahl, [id]: an };
      try {
        localStorage.setItem(schluessel, JSON.stringify(wahl));
      } catch {
        // Ohne Speicher gilt die Wahl bis zum Neuladen.
      }
    },
  };
}

/**
 * Die Reihe der Ebenen, wie der Betrachter sie umgestellt hat, je Wurzel:
 * ihre `id` von oben nach unten, im Speicher der Seite, durchgeschrieben nach
 * `localStorage`. `undefined`, solange niemand umgestellt hat; dann gilt die
 * Reihe des Servers. Siehe docs/entscheidungen/0113-reihenfolge-der-ebenen-je-betrachter.md.
 */
function gemerkteReihe(wurzel: string): { lies: () => readonly string[] | undefined; setze: (reihe: readonly string[] | undefined) => void } {
  const schluessel = `ebenen-reihe:${wurzel}`;
  let reihe: readonly string[] | undefined;
  try {
    const wert: unknown = JSON.parse(localStorage.getItem(schluessel) ?? 'null');
    if (Array.isArray(wert) && wert.every((id) => typeof id === 'string')) reihe = [...new Set(wert)];
  } catch {
    // Ohne Speicher oder mit kaputtem Eintrag gilt die Reihe des Servers.
  }
  return {
    lies: () => reihe,
    setze: (neu) => {
      reihe = neu && [...neu];
      try {
        if (neu) localStorage.setItem(schluessel, JSON.stringify(neu));
        else localStorage.removeItem(schluessel);
      } catch {
        // Ohne Speicher gilt die Reihe bis zum Neuladen.
      }
    },
  };
}

/**
 * Die Ebenen in der Reihe des Betrachters: die gemerkten, die es noch gibt,
 * in ihrer Reihenfolge; jede neue, in der Reihe des Servers, direkt über der
 * obersten schon eingereihten, deren `order` nicht höher ist, sonst ganz
 * unten. Ohne gemerkte Reihe die des Servers. Gewählt in 0113.
 */
function einreihen<T extends { id: string; order: number }>(nachServer: readonly T[], gemerkt: readonly string[] | undefined): T[] {
  if (!gemerkt) return [...nachServer];
  const je = new Map(nachServer.map((e) => [e.id, e]));
  const reihe = gemerkt.flatMap((id) => je.get(id) ?? []);
  for (const e of nachServer) {
    if (reihe.includes(e)) continue;
    const darunter = reihe.findIndex((x) => x.order <= e.order);
    if (darunter < 0) reihe.push(e);
    else reihe.splice(darunter, 0, e);
  }
  return reihe;
}

/** Fuss und Winkel aus `satz.json`, oder `undefined`, wenn sie nicht taugen. */
function satzDaten(wert: unknown): Omit<Satz, 'ordner'> | undefined {
  if (!istObjekt(wert) || !istZahl(wert.angle)) return undefined;
  const fuss = wert.foot;
  if (!Array.isArray(fuss) || fuss.length !== 2 || !fuss.every((k) => Number.isInteger(k) && (k as number) >= 0)) return undefined;
  return { fuss: fuss as [number, number], winkel: wert.angle };
}

/** Holt JSON vom Server bis `max` Byte; `undefined` für fehlend, eine HTML-Seite, zu gross oder kaputt. */
async function json(pfad: string, max: number): Promise<unknown> {
  const antwort = await fetch(pfad, FRISCH);
  if (!antwort.ok || antwort.headers.get('content-type')?.startsWith('text/html')) return undefined;
  const zuGross = () => console.warn(`${pfad}: grösser als ${max} Byte, übergangen`);
  // Erst die angesagte Länge, ohne Download; ohne sie oder gepackt zählt der gelesene Text.
  if (Number(antwort.headers.get('content-length')) > max) {
    void antwort.body?.cancel();
    return zuGross();
  }
  const text = await antwort.text();
  if (new TextEncoder().encode(text).length > max) return zuGross();
  try {
    return JSON.parse(text) as unknown;
  } catch {
    console.warn(`${pfad}: keine gültige JSON-Datei`);
    return undefined;
  }
}

/**
 * Die Ebenen der Karte: liest `layers.json` neben `trees.json`, zeigt die
 * Liste zum Umschalten und die Objekte der Ebenen, die an sind, und fragt
 * alle 30 Sekunden nach Änderungen. Fehlt `layers.json` beim Laden, geschieht
 * bis zum Neuladen nichts.
 */
export async function ebenen(umgebung: Umgebung): Promise<void> {
  const { map, wurzel, blick, scale, maxZoom, karten, heightsCell, seaLevel } = umgebung;
  const sprache = navigator.language.startsWith('de') ? 'de' : 'en';
  const wahl = gemerkt(wurzel);
  const reihe = gemerkteReihe(wurzel);
  const iso = blick.p.y > 0;
  const grund = seaLevel ?? 64;
  const c = heightsCell ?? 4;
  /** Ohne Höhen: eben auf `grund`. */
  const eben: Gelaende = { c, grund, zelle: () => undefined, max: grund + 1 };

  /** Die Oberseite des Geländes je Punkt, einmal gerechnet; siehe ebenen.md, „Die Oberfläche im iso“. */
  /** Die Oberseite des Geländes an einem Punkt, ohne Cache, etwa für Mitspieler, die laufen. */
  const oberflaecheJetzt = async (x: number, z: number): Promise<number> => {
    if (!iso || !karten || !heightsCell) return grund + 1;
    // Vier Zellen um den Punkt und für den Ersatz zwei weitere in jede Richtung.
    await karten.lade([-3 * c, 3 * c].flatMap((dx) => [-3 * c, 3 * c].map((dz) => [Math.floor(x + dx), 0, Math.floor(z + dz)] as [number, number, number])));
    return hoeheAuf({ ...eben, zelle: (i, j) => karten.hoehe(i * c, j * c) }, x, z);
  };
  const oberflaechen = new Map<string, Promise<number>>();
  const oberflaeche = (x: number, z: number): Promise<number> => {
    const schluessel = `${x},${z}`;
    let wert = oberflaechen.get(schluessel);
    if (!wert) {
      wert = oberflaecheJetzt(x, z);
      oberflaechen.set(schluessel, wert);
    }
    return wert;
  };

  /** Eine Region als Zahl: Ein Schlüssel aus Text kostete je Zugriff. */
  const schluessel = (rx: number, rz: number) => (rx + 65536) * 131072 + (rz + 65536);

  /**
   * Die Regionen, deren Höhen Formen und Schrift einer Ebene brauchen, nur
   * innerhalb von `area`: für eine Fläche ihr Rechteck, für Ränder und Linien
   * nur die Punkte entlang des Zugs; je samt dem Streifen zur Kamera, aus dem
   * Gelände sie verdecken kann, und drei Zellen für Mischen und Ersatz.
   */
  const regionenFuer = (formen: readonly Form[], schriften: readonly Schriftzug[], nurIn?: Rechteck): Set<number> => {
    const menge = new Set<number>();
    if (!iso || !karten || !heightsCell) return menge;
    const { wx, wz, steigung } = zurKamera(blick);
    const weit = ((umgebung.maxY ?? 319) - (umgebung.minY ?? -64)) / steigung;
    const nimm = ([x0, z0, x1, z1]: Rechteck): void => {
      let [ax, az] = [Math.min(x0, x0 + wx * weit) - 3 * c, Math.min(z0, z0 + wz * weit) - 3 * c];
      let [bx, bz] = [Math.max(x1, x1 + wx * weit) + 3 * c, Math.max(z1, z1 + wz * weit) + 3 * c];
      // Nur in area, und mit `nurIn` nur, was im Ausschnitt liegen kann.
      for (const r of [umgebung.area, nurIn]) if (r) [ax, az, bx, bz] = [Math.max(ax, r[0]), Math.max(az, r[1]), Math.min(bx, r[2]), Math.min(bz, r[3])];
      // Die obere Kante zählt nicht mit: area endet vor ihr.
      for (let rx = Math.floor(ax / REGION); rx < Math.ceil(bx / REGION); rx++) {
        for (let rz = Math.floor(az / REGION); rz < Math.ceil(bz / REGION); rz++) menge.add(schluessel(rx, rz));
      }
    };
    for (const f of formen) {
      if (hatFlaeche(f)) nimm(bereich(f));
      if (hatFlaeche(f) || f.rand.breite === 0) continue;
      // Entlang des Zugs höchstens alle 64 Blöcke ein Punkt, je mit dem halben
      // Schritt als Rand: So decken die Rechtecke den ganzen Zug, auch wo er
      // eine Region nur an der Ecke streift.
      for (const stueck of zuege(f, c, umgebung.area)) {
        for (let i = 1; i < stueck.length; i++) {
          const [a, b] = [stueck[i - 1]!, stueck[i]!];
          const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / 64));
          for (let t = 0; t <= n; t++) {
            const [x, z] = [a[0] + ((b[0] - a[0]) * t) / n, a[1] + ((b[1] - a[1]) * t) / n];
            nimm([x - 32, z - 32, x + 32, z + 32]);
          }
        }
      }
    }
    for (const s of schriften) {
      const [x0, z0, x1, z1] = rechteck(s.pfad);
      nimm([x0 - 16, z0 - 16, x1 + 16, z1 + 16]);
    }
    return menge;
  };

  /**
   * Die Höhen dieser Regionen, einmal geladen für alle Formen oder alle
   * Schriften einer Ebene; mit `aufDemBoden` aus `boden`, je Region ohne
   * Datei aus `karten`. Die Karten hält das Ergebnis selbst, so verdrängt der
   * Cache keine, solange es lebt.
   */
  const ladeGelaende = async (menge: ReadonlySet<number>, aufDemBoden: boolean): Promise<Gelaende> => {
    if (!karten || menge.size === 0) return eben;
    const boden = aufDemBoden ? umgebung.boden : undefined;
    // Die Zelle des Ergebnisses: die von ground, wenn es Formen auf dem Boden sind.
    const cg = boden ? (umgebung.bodenCell ?? c) : c;
    if (boden && menge.size * regionBytes(cg) > HOEHEN_BYTES) {
      console.warn(`${wurzel}: Ebene bräuchte ${menge.size} Regionen ground, mehr als ${HOEHEN_BYTES / regionBytes(cg)}; ihre Formen liegen auf heights`);
      return ladeGelaende(menge, false);
    }
    // Gegen eine Ebene ohne area mit riesigem Kreis.
    if (menge.size * regionBytes(c) > HOEHEN_BYTES) {
      console.warn(`${wurzel}: Ebene bräuchte ${menge.size} Regionen Höhen, mehr als ${HOEHEN_BYTES / regionBytes(c)}; sie liegt ganz auf seaLevel`);
      return eben;
    }
    /** Je Region die Karte und ihre Zellen je Kante; ohne ground die gröberen aus heights. */
    const regionen = new Map<number, { karte: Int16Array; n: number }>();
    let max = grund + 1;
    await Promise.all(
      [...menge].map(async (s) => {
        const [rx, rz] = [Math.floor(s / 131072) - 65536, (s % 131072) - 65536];
        const vomBoden = boden ? await boden.karte(rx, rz) : null;
        const karte = vomBoden ?? (await karten.karte(rx, rz));
        if (!karte) return;
        regionen.set(s, { karte, n: REGION / (vomBoden ? cg : c) });
        max = Math.max(max, hoechste(karte));
      }),
    );
    const n = REGION / cg;
    // Die zuletzt gefragte Region: Nachbarn liegen meist in derselben.
    let [letzte, eintrag]: [number, { karte: Int16Array; n: number } | undefined] = [Number.NaN, undefined];
    // Je Block von BLOCK Zellen dieses Geländes das Höchste seiner Region; ohne Region oder mit leerer Zelle zählt grund.
    const proRegion = n / BLOCK;
    const roh = (bi: number, bj: number): number => {
      const [rx, rz] = [Math.floor(bi / proRegion), Math.floor(bj / proRegion)];
      const e = regionen.get(schluessel(rx, rz));
      if (!e) return grund;
      const kante = (BLOCK * e.n) / n;
      const b = bloeckeDer(e.karte, e.n, kante);
      const k = (bj - rz * proRegion) * proRegion + (bi - rx * proRegion);
      return b.leer[k] || b.max[k] === LEER ? Math.max(grund, b.max[k]!) : b.max[k]!;
    };
    return {
      c: cg,
      grund,
      max,
      hoch: blockHoch(roh),
      zelle: (i, j) => {
        const [rx, rz] = [Math.floor(i / n), Math.floor(j / n)];
        const s = schluessel(rx, rz);
        if (s !== letzte) [letzte, eintrag] = [s, regionen.get(s)];
        if (!eintrag) return undefined;
        // Eine Region aus heights hat gröbere Zellen: die, in der diese liegt.
        const m = eintrag.n;
        const v = eintrag.karte[Math.floor(((j - rz * n) * m) / n) * m + Math.floor(((i - rx * n) * m) / n)];
        return v === undefined || v === LEER ? undefined : v;
      },
    };
  };

  interface Geladen {
    version: string;
    /** Was gezeichnet ist: `version`, bei einer Aktionsebene der Inhalt ihrer Datei. */
    stand: string;
    /** Nadeln und Banner. */
    gruppe: L.LayerGroup;
    /** Flächen, Wände, Ränder, Linien und Schrift, in dieser Reihenfolge. */
    formen: L.LayerGroup;
    /** Die Formen, aus denen die Karte Flächen, Ränder, Linien und Wände je Ansicht neu rechnet, und ihr Renderer. */
    quelle: readonly Form[];
    renderer: L.Renderer;
    /** Die Ziele der Flächen, je mit dem Index ihrer Form; ihre Ringe setzt jede Rechnung neu. */
    flaechen: { flaeche: L.Polygon; index: number }[];
    /** Wände, Ränder und Linien, wie für `schirm` gerechnet; jede neue Rechnung zählt `nummer` hoch. */
    waende: L.Layer[];
    striche: Strich[];
    schirm: Schirm;
    nummer: number;
  }
  const geladen = new Map<string, Geladen>();
  /** Je Ebene ein Zähler: Ein Laden, das ein späteres Umschalten überholt, verwirft sich selbst. */
  const auftrag = new Map<string, number>();
  /** Je Ebene die `version` mit `permission` oder `web: false`; die holt die Karte nicht noch einmal. */
  const abgewiesen = new Map<string, string>();
  /**
   * Je Ebene die `version`, die das Nachfragen gerade lädt. Dauert ein Laden
   * länger als der Takt, etwa auf einem ausgelasteten Rechner, startete
   * sonst jedes Nachfragen es neu und verwürfe das laufende: Die Ebene
   * erschiene nie.
   */
  const laeuft = new Map<string, string>();
  let eintraege: Eintrag[] = [];
  /** Alle Einträge in der Reihe des Servers, auch Aktionsebenen ohne Anmeldung; `eintraege` ordnet sie nach `reihe`. */
  let nachServer: Eintrag[] = [];
  /** Die Einträge aus `layers.json`, wie zuletzt gelesen, auch Aktionsebenen ohne Anmeldung. */
  let roh: unknown[] = [];

  // Anmeldung und Knöpfe, siehe docs/frontend.md, „Aktionsebenen“.
  let spieler: Spieler | undefined;
  /** Ob unter `api/` ein Server des Renderers antwortet; erst nach der ersten Frage bekannt. */
  let mitServer = false;
  /** Ein Gast über IPv6, den der Server ohne zweiten Namen nicht erkennen kann. */
  let ipv6 = false;
  /** Der Hinweis an gesperrten Knöpfen: über die IP erkannt, oder der Server sagt 403. */
  const NUR_LESEN = 'Zum Schalten im Spiel /karte anmelden';
  /** Die eingebaute Ansicht der Mitspieler, gemerkt wie eine Ebene. */
  const MITSPIELER = 'heroicmap:mitspieler';
  const mitspielerAn = (): boolean => spieler !== undefined && (wahl.lies()[MITSPIELER] ?? true);
  /** Was beim letzten Anmelden schiefging, für die Liste. */
  let meldung = '';
  /** Die Knöpfe je Objekt, nach `objektSchluessel`. */
  let knoepfe = new Map<string, Knopf[]>();
  /** Knöpfe, deren Klick noch läuft, nach `knopfSchluessel`. */
  const laufend = new Set<string>();
  /** Je Objekt ein Hinweis unter den Knöpfen, etwa „noch keine Antwort“. */
  const hinweise = new Map<string, string>();
  const knopfSchluessel = (k: Knopf) => JSON.stringify([k.ebene, k.objekt, k.knopf]);

  /** Die Tafel, die gerade offen ist, das Element ihres Ziels, das den Fokus zurückbekommt, und ob sie gehalten ist. */
  let offen: { popup: L.Popup; element: () => Element | undefined; gehalten: () => boolean } | undefined;

  /**
   * Die Tafel eines Ziels, siehe docs/benutzung/ebenen.md, „Infotafel“:
   * Ruht der Zeiger 50 ms darauf, erscheint sie dort; 300 ms nachdem er Ziel
   * und Tafel verlassen hat, schliesst sie. Ein Klick oder Tippen hält sie,
   * und solange eine gehaltene Tafel offen ist, öffnet Zeigen keine andere.
   * Gibt zurück, womit Tastatur sie öffnet, dann mit dem Fokus darin. `ort`
   * sagt, wo sie erscheint, ohne Ereignis für die Tastatur; `hoehe` hebt sie
   * über das Icon, damit sie es nicht deckt, gefragt bei jedem Öffnen: Das
   * Icon wird beim Zoomen kleiner.
   */
  const tafelAn = (
    ziel: L.Marker | L.Polygon,
    inhalt: () => HTMLElement,
    element: () => Element | undefined,
    ort: (ereignis?: L.LeafletMouseEvent) => L.LatLng,
    hoehe: () => number = () => 0,
  ): ((perTastatur: boolean) => void) => {
    // 7 ist der Versatz, den Leaflet sonst hat.
    const popup = L.popup({ ...TAFEL, offset: [0, 7 - hoehe()] }).setContent(inhalt);
    let gehalten = false;
    let perTastatur = false;
    let auf: ReturnType<typeof setTimeout> | undefined;
    let zu: ReturnType<typeof setTimeout> | undefined;
    // Ob der Zeiger auf dem Ziel oder in der Tafel ist.
    let darauf = false;
    let imKasten = false;
    // Von Hand unter dem Zeiger geschlossen: Erst ein neues Zeigen öffnet sie wieder.
    let ruhe = false;
    /** Wartet nach dem Schliessen in der Tafel auf das erste Element ausserhalb der Tafel. */
    let danach: ((e: MouseEvent) => void) | undefined;
    const vergiss = () => {
      if (danach) document.removeEventListener('mouseover', danach, true);
      danach = undefined;
    };
    const fokus = () => {
      const inhaltKasten = popup.getElement()?.querySelector<HTMLElement>('.leaflet-popup-content');
      if (!inhaltKasten) return;
      inhaltKasten.tabIndex = -1;
      // Ohne preventScroll dürfte der Browser den Container scrollen, um die Tafel zu zeigen.
      if (perTastatur) inhaltKasten.focus({ preventScroll: true });
    };
    const oeffne = (wo: L.LatLng) => {
      clearTimeout(auf);
      clearTimeout(zu);
      // Nur eine gehaltene Tafel verschiebt die Karte; beim Zeigen liesse das einen festgehaltenen Block los.
      popup.options.autoPan = gehalten;
      popup.options.offset = [0, 7 - hoehe()];
      popup.setLatLng(wo);
      if (!map.hasLayer(popup)) popup.openOn(map);
      else fokus();
    };
    /** Eine gehaltene Tafel eines anderen Ziels ist offen: Zeigen öffnet dann nichts. */
    const andereGehalten = () => offen !== undefined && offen.popup !== popup && offen.gehalten();
    // Bei jeder Bewegung von vorn: Erst Ruhe öffnet, und zwar am Ort der Ruhe.
    const plane = (ereignis: L.LeafletMouseEvent) => {
      clearTimeout(auf);
      if (ruhe || map.hasLayer(popup) || andereGehalten()) return;
      auf = setTimeout(() => {
        if (!andereGehalten()) oeffne(ort(ereignis));
      }, TAFEL_AUF);
    };
    const bald = () => {
      clearTimeout(auf);
      if (!gehalten) zu = setTimeout(() => map.closePopup(popup), TAFEL_ZU);
    };
    ziel.on('mouseover', (ereignis) => {
      darauf = true;
      clearTimeout(zu);
      plane(ereignis);
    });
    ziel.on('mousemove', plane);
    ziel.on('mouseout', () => {
      darauf = ruhe = false;
      bald();
    });
    // Sonst schlösse der Klick auf der Karte die Tafel gleich wieder (closeOnClick).
    ziel.on('preclick', L.DomEvent.stopPropagation);
    ziel.on('click', (ereignis) => {
      gehalten = true;
      perTastatur = false;
      oeffne(ort(ereignis));
    });
    // Ein Klick daneben schliesst nur die Tafel; ein Ziel mit Tafel zählt nicht als daneben.
    ziel.on('add', () => element()?.classList.add('tafel-ziel'));
    // Weicht das Ziel, mit seiner Ebene oder beim Neuladen, weicht die Tafel mit.
    ziel.on('remove', () => {
      clearTimeout(auf);
      darauf = false;
      map.closePopup(popup);
    });
    let angemeldet = false;
    popup.on('add', () => {
      vergiss();
      offen = { popup, element, gehalten: () => gehalten };
      const kasten = popup.getElement();
      if (kasten && !angemeldet) {
        angemeldet = true;
        kasten.addEventListener('mouseenter', () => {
          imKasten = true;
          clearTimeout(zu);
        });
        kasten.addEventListener('mouseleave', () => {
          imKasten = false;
          bald();
        });
      }
      fokus();
    });
    popup.on('remove', () => {
      // Von selbst schliesst sie nur, wenn der Zeiger weder auf dem Ziel noch in ihr ist.
      ruhe = darauf || imKasten;
      // Lag der Zeiger in der Tafel, entscheidet, wo er als Nächstes ist; die
      // ausblendende Tafel liegt noch 200 ms unter ihm und zählt nicht.
      if (ruhe && imKasten) {
        const kasten = popup.getElement();
        danach = (e) => {
          if (kasten?.contains(e.target as Node)) return;
          vergiss();
          ruhe = element()?.contains(e.target as Node) === true;
        };
        document.addEventListener('mouseover', danach, true);
      }
      imKasten = false;
      gehalten = perTastatur = false;
      clearTimeout(auf);
      clearTimeout(zu);
      if (offen?.popup === popup) offen = undefined;
    });
    return (tastatur) => {
      gehalten = true;
      perTastatur = tastatur;
      oeffne(ort());
    };
  };

  /** Eine Fläche mit Tafel: beim Zeigen wie jedes Ziel, dazu ein Ziel für Tab; Enter oder Leertaste öffnet die Tafel. */
  const bediene = (flaeche: L.Polygon, name: string | undefined, inhalt: () => HTMLElement): void => {
    const oeffne = tafelAn(flaeche, inhalt, () => flaeche.getElement(), (ereignis) => ereignis?.latlng ?? flaeche.getBounds().getCenter());
    flaeche.on('add', () => {
      const element = flaeche.getElement();
      if (!element) return;
      element.setAttribute('tabindex', '0');
      element.setAttribute('role', 'button');
      element.setAttribute('aria-label', name ?? 'Tafel');
      element.addEventListener('keydown', (ereignis) => {
        if ((ereignis as KeyboardEvent).key !== 'Enter' && (ereignis as KeyboardEvent).key !== ' ') return;
        ereignis.preventDefault();
        oeffne(true);
      });
    });
  };

  /** Die Stufe der Grösse jetzt, nach dem Massstab der Karte, höchstens `STUFE_MAX`. Siehe `GROESSE`. */
  const groessenStufe = (): number => {
    const px = blockBreite(blick) * faktor();
    return Math.min(STUFE_MAX, Math.max(0, Math.ceil(Math.log2(GROESSE.grenze / px) - 1e-9)));
  };

  /**
   * Das Icon einer Nadel oder eines Banners auf der Stufe `stufe`: das Bild
   * Pixel auf Pixel, der Fuss unten bei ⌊b / 2⌋ oder bei `ort.fuss`, der
   * Name darunter, mit `ort.winkel` im Bogen. Ein
   * Pixel des Bilds ist k = max(1, round(devicePixelRatio)) Pixel des Geräts
   * breit, so bleibt jedes gleich breit, auch bei 1,25 oder 1,5; je Stufe
   * die Hälfte, ohne Glätten. Der Name wird mit kleiner und fehlt unter
   * `GROESSE.nameMin`. Siehe docs/frontend.md, „Ebenen“.
   */
  const ortIcon = (
    bild: CanvasImageSource,
    b: number,
    h: number,
    name: string | undefined,
    ort: { fuss: [number, number]; winkel?: number } | undefined,
    stufe: number,
  ): L.DivIcon => {
    // Der Fuss als Punkt auf den Kanten der Pixel: bei der Nadel und beim Bild (⌊b / 2⌋, h), beim Sprite aus satz.json, beim Kopf die Mitte.
    const [fx, fy] = ort?.fuss ?? [Math.floor(b / 2), h];
    const k = Math.max(1, Math.round(devicePixelRatio));
    const s = k / devicePixelRatio;
    const teil = 2 ** -stufe;
    // Auf dem Gerät gerundet, so bleibt jedes Pixel ganz; das Bild ohne Glätten, je Stufe jedes zweite Pixel.
    const [gb, gh] = [Math.max(1, Math.round(b * k * teil)), Math.max(1, Math.round(h * k * teil))];
    const [sb, sh] = [gb / devicePixelRatio, gh / devicePixelRatio];
    const html = L.DomUtil.create('div', 'nadel');
    const kopie = document.createElement('canvas');
    [kopie.width, kopie.height] = [gb, gh];
    [kopie.style.width, kopie.style.height] = [`${sb}px`, `${sh}px`];
    const ctx = kopie.getContext('2d')!;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(bild, 0, 0, gb, gh);
    html.append(kopie);
    const [ax, ay] = [(fx * sb) / b, (fy * sh) / h];
    const mitName = name && NAME.s * teil >= GROESSE.nameMin;
    if (mitName && ort?.winkel !== undefined) {
      // Gemessen in der Kartenschrift: bannerIcon wartet, bis sie geladen ist. Der Bogen wie auf voller Grösse, mit kleiner.
      const svg = nameImBogen(name, h * s, ort.winkel);
      [svg.style.left, svg.style.top] = [`${ax}px`, `${ay}px`];
      if (teil < 1) [svg.style.transform, svg.style.transformOrigin] = [`scale(${teil})`, '0 0'];
      html.append(svg);
    } else if (mitName) {
      // Der Name steht in der Kartenschrift; bis sie geladen ist, in der Vorgabe von style.css.
      void ladeSchrift();
      const text = L.DomUtil.create('span', 'nadel-name', html);
      text.textContent = name;
      if (teil < 1) [text.style.transform, text.style.transformOrigin] = [`translateX(-50%) scale(${teil})`, '50% 0'];
    }
    return L.divIcon({ html, className: 'nadel-icon', iconSize: [sb, sh], iconAnchor: [ax, ay] });
  };

  /** Je Nadel und Banner, wie es sein Icon auf einer Stufe zeichnet; beim Zoomen über eine Stufe bekommt es ein neues. */
  const iconAuf = new WeakMap<L.Marker, (stufe: number) => L.DivIcon>();
  let stufeJetzt = -1;
  /** Zeichnet die Icons neu, wenn der Zoom die Stufe der Grösse ändert. */
  const groesseNachZoom = (): void => {
    const stufe = groessenStufe();
    if (stufe === stufeJetzt) return;
    stufeJetzt = stufe;
    for (const g of geladen.values()) {
      g.gruppe.eachLayer((m) => {
        const neu = iconAuf.get(m as L.Marker);
        if (!neu) return;
        // setIcon baut ein neues Element; die Klasse des Ziels einer Tafel kommt mit.
        const ziel = (m as L.Marker).getElement()?.classList.contains('tafel-ziel');
        (m as L.Marker).setIcon(neu(stufe));
        if (ziel) (m as L.Marker).getElement()?.classList.add('tafel-ziel');
      });
    }
  };

  /** Je Kopf das Gesicht, einmal geholt je Seite, für Mitspieler und Nadeln; `undefined`, wenn es nicht taugt. */
  const koepfe = new Map<string, Promise<ImageBitmap | undefined>>();
  /**
   * Holt das Gesicht `koepfe/<kopf>.png` aus der Wurzel, höchstens
   * `KOPF.bytes` und genau 8 × 8, ohne `blob:` oder `data:` für die CSP.
   * Siehe docs/entscheidungen/0114-koepfe-der-mitspieler.md.
   */
  const kopfBild = (kopf: string): Promise<ImageBitmap | undefined> => {
    let bild = koepfe.get(kopf);
    if (!bild) {
      const pfad = `${wurzel}/koepfe/${kopf}.png`;
      const zuGross = (n: number) => n > KOPF.bytes;
      const taugtNicht = (warum: string) => {
        console.warn(`${pfad}: ${warum}, ohne Gesicht`);
        return undefined;
      };
      bild = (async () => {
        // Ohne FRISCH: Der Name ist der Hash des Bildes, der Browser darf ihn behalten.
        const antwort = await fetch(pfad);
        if (!antwort.ok) return taugtNicht(`${antwort.status}`);
        // Erst die angesagte Länge, ohne Download, dann die gelesene.
        if (zuGross(Number(antwort.headers.get('content-length')))) {
          void antwort.body?.cancel();
          return taugtNicht(`grösser als ${KOPF.bytes} Byte`);
        }
        const daten = await antwort.blob();
        if (zuGross(daten.size)) return taugtNicht(`grösser als ${KOPF.bytes} Byte`);
        const b = await createImageBitmap(daten).catch(() => undefined);
        if (!b) return taugtNicht('kein Bild');
        return b.width === KOPF.seite && b.height === KOPF.seite ? b : taugtNicht(`${b.width} × ${b.height} statt ${KOPF.seite} × ${KOPF.seite}`);
      })().catch(() => taugtNicht('nicht zu holen'));
      koepfe.set(kopf, bild);
    }
    return bild;
  };

  /**
   * Das Icon einer Nadel in ihrer `size`, je Stufe; mit tauglichem `head`
   * das Gesicht, ein Pixel 2 Pixel der Nadel, die Mitte auf dem Fuss. Siehe
   * docs/benutzung/ebenen.md, „Nadel“. Icons und Symbole gelten je Laden einer Ebene.
   */
  const nadelIcon = async (
    n: Nadel,
    ordner: string,
    version: string,
    icons: Map<string, Promise<HTMLCanvasElement>>,
    symbole: Map<string, Promise<HTMLImageElement>>,
  ): Promise<(stufe: number) => L.DivIcon> => {
    const gesicht = n.head && (await kopfBild(n.head));
    if (gesicht) return (stufe) => ortIcon(gesicht, 2 * KOPF.seite, 2 * KOPF.seite, n.name, { fuss: [KOPF.seite, KOPF.seite] }, stufe);
    const pfad = n.size === 'small' ? undefined : n.symbol[n.size];
    const symbol = pfad && `${ordner}/${pfad}?v=${encodeURIComponent(version)}`;
    const schluessel = `${n.size} ${n.color} ${symbol ?? ''}`;
    let leinwand = icons.get(schluessel);
    if (!leinwand) icons.set(schluessel, (leinwand = zeichneNadel(n.size, n.color, symbol, symbole)));
    const { b, h } = SCHILDE[n.size];
    const fertig = await leinwand;
    return (stufe) => ortIcon(fertig, b, h, n.name, undefined, stufe);
  };

  /**
   * Das Icon eines Banners: mit `design` und Satz das Sprite des Satzes, mit
   * `capital` das aus `krone/`, Fuss und Winkel aus `satz.json`; fehlt es,
   * `image`. Ohne beides `undefined` mit Meldung. Siehe
   * docs/benutzung/ebenen.md, „Banner“ und „Sprites“.
   */
  const bannerIcon = async (
    banner: Banner,
    ordner: string,
    version: string,
    symbole: Map<string, Promise<HTMLImageElement>>,
    satz: Satz | undefined,
  ): Promise<((stufe: number) => L.DivIcon) | undefined> => {
    const [b, h] = GRENZEN.banner as [number, number];
    const v = encodeURIComponent(version);
    const passt = (geholt: HTMLImageElement | undefined) => geholt && geholt.naturalWidth <= b && geholt.naturalHeight <= h;
    if (banner.design && satz) {
      const adresse = `${satz.ordner}/${banner.capital ? 'krone/' : ''}${banner.design}.png?v=${v}`;
      const sprite = await bild(adresse, symbole).catch(() => undefined);
      const [fx, fy] = satz.fuss;
      if (sprite && passt(sprite) && fx <= sprite.naturalWidth && fy <= sprite.naturalHeight) {
        if (banner.name) await ladeSchrift();
        return (stufe) => ortIcon(sprite, sprite.naturalWidth, sprite.naturalHeight, banner.name, satz, stufe);
      }
      console.warn(`Banner ${banner.id}: Sprite ${adresse} fehlt, ist zu gross oder passt nicht zum Fuss aus satz.json${banner.image ? ', nimmt image' : ', übergangen'}`);
    }
    if (!banner.image) {
      if (!satz) console.warn(`Banner ${banner.id}: ohne Sprite dieses Baums und ohne image, übergangen`);
      return undefined;
    }
    const adresse = `${ordner}/${banner.image}?v=${v}`;
    const geholt = await bild(adresse, symbole).catch(() => undefined);
    if (!geholt || !passt(geholt)) {
      console.warn(`Banner ${banner.id}: ${geholt ? `${geholt.naturalWidth} × ${geholt.naturalHeight} statt höchstens ${b} × ${h}` : `${adresse} lässt sich nicht laden`}, übergangen`);
      return undefined;
    }
    if (banner.name) await ladeSchrift();
    // Mit image der Fuss unten mittig und der Name ohne Drehung.
    const fuss: [number, number] = [Math.floor(geholt.naturalWidth / 2), geholt.naturalHeight];
    return (stufe) => ortIcon(geholt, geholt.naturalWidth, geholt.naturalHeight, banner.name, { fuss, winkel: 0 }, stufe);
  };

  /**
   * Je Ebene ein Pane für die Nadeln und eines für Formen und Schrift, mit
   * eigenem SVG; die Namen zählen hoch, so kollidieren keine zwei Kennungen.
   * Klicks gehen durch das SVG zu tieferen Panes; nur Flächen mit Namen oder
   * Tafel fangen sie.
   */
  const panes = new Map<string, { nadeln: string; formen: string; renderer: L.Renderer }>();
  const pane = (id: string) => {
    let eintrag = panes.get(id);
    if (!eintrag) {
      const name = `ebene-${panes.size}`;
      map.createPane(name);
      map.createPane(`${name}-formen`);
      eintrag = { nadeln: name, formen: `${name}-formen`, renderer: L.svg({ pane: `${name}-formen` }) };
      panes.set(id, eintrag);
    }
    return eintrag;
  };
  /** Ordnet die Panes in der Reihe von `eintraege`: die erste oben. */
  const stapeln = (): void => {
    eintraege.forEach((e, i) => {
      const rang = eintraege.length - 1 - i;
      map.getPane(pane(e.id).nadeln)!.style.zIndex = String(PANE_GRUND + rang);
      map.getPane(pane(e.id).formen)!.style.zIndex = String(FORM_GRUND + rang);
    });
  };

  /** Ordnet die Einträge nach der Reihe des Betrachters, Aktionsebenen nur angemeldet, und zeigt Liste und Panes danach. */
  const ordne = (): void => {
    // Aktionsebenen nur angemeldet; siehe docs/frontend.md, „Aktionsebenen“.
    eintraege = einreihen(nachServer, reihe.lies()).filter((e) => !e.player || spieler);
    zeigeListe();
    stapeln();
  };
  /**
   * Tauscht eine Ebene mit der nächsten sichtbaren darüber oder darunter und
   * merkt sich die ganze Reihe, auch mit Aktionsebenen ohne Anmeldung, die
   * dabei an ihrem Platz bleiben.
   */
  const stelleUm = (id: string, schritt: -1 | 1): void => {
    const nachbar = eintraege[eintraege.findIndex((e) => e.id === id) + schritt];
    if (!nachbar) return;
    const alle = einreihen(nachServer, reihe.lies()).map((e) => e.id);
    const [a, b] = [alle.indexOf(id), alle.indexOf(nachbar.id)];
    [alle[a], alle[b]] = [alle[b]!, alle[a]!];
    reihe.setze(alle);
    gedrueckt = { id, schritt };
    ordne();
  };

  /** Nimmt eine geladene Ebene von der Karte. */
  const weg = (id: string): void => {
    const g = geladen.get(id);
    g?.gruppe.remove();
    g?.formen.remove();
    geladen.delete(id);
  };

  const an = (e: Eintrag): boolean => wahl.lies()[e.id] ?? e.visible;

  /** Pixel des Schirms je Pixel der feinsten Stufe. */
  const faktor = () => 2 ** (map.getZoom() - maxZoom);

  /**
   * Die Schrift einer Ebene, mit einmal geladenen Höhen der Oberfläche, wie
   * Nadeln und Banner. Eine eigene Funktion, die keine Closure zurücklässt:
   * So leben die Höhen nur in ihrem Aufruf und fallen nach dem Zeichnen weg.
   */
  const schriftLagenFuer = async (schriften: readonly Schriftzug[], formPane: string): Promise<Schrift[]> => {
    if (!schriften.length) return [];
    const oben = await ladeGelaende(regionenFuer([], schriften), false);
    return schriften.map((s) => new Schrift(s, schriftPfad(s.pfad, oben, blick), faktor, scale, formPane));
  };

  /** Das sichtbare Fenster in Pixeln der feinsten Stufe. */
  const fenster = (): Rechteck => {
    const b = map.getBounds();
    return [b.getWest(), Math.min(b.getSouth(), b.getNorth()), b.getEast(), Math.max(b.getSouth(), b.getNorth())];
  };
  /** Die Ansicht nach 0111: das Fenster plus ein halbes je Seite, so trägt ein Verschieben bis zum Rand ohne neue Rechnung. */
  const schirmJetzt = (): Schirm => {
    const [x0, y0, x1, y1] = fenster();
    const [w, h] = [(x1 - x0) / 2, (y1 - y0) / 2];
    return { faktor: faktor(), rahmen: [x0 - w, y0 - h, x1 + w, y1 + h], eps: REGEL.eps };
  };

  /** Was eine Rechnung für die Ansicht gibt. */
  type Ansicht = ReturnType<typeof zeichneAnsicht> & { schirm: Schirm };

  /**
   * Flächen, Ränder, Linien und Wände einer Ebene für die Ansicht jetzt, mit
   * den Höhen nur der Regionen, die im Ausschnitt liegen können. Eigene
   * Funktion aus demselben Grund wie `schriftLagenFuer`: Die Höhen leben nur
   * im Aufruf.
   */
  const ansichtJetzt = async (formen: readonly Form[], renderer: L.Renderer): Promise<Ansicht> => {
    const schirm = schirmJetzt();
    const welt = weltRechteck(schirm.rahmen, blick, umgebung.minY ?? -64, umgebung.maxY ?? 319);
    const boden = formen.some((f) => f.rand.breite > 0 || hatFlaeche(f)) ? await ladeGelaende(regionenFuer(formen, [], welt), true) : eben;
    return { ...zeichneAnsicht(formen, { renderer, blick, gelaende: boden, eben: boden === eben, area: umgebung.area }, schirm), schirm };
  };

  /** Schreibt mit `?punkte` die Punkte von Flächen, Rändern, Linien und Wänden an das Pane der Formen. */
  const zaehle = (id: string, punkte: number): void => {
    if (ZAEHLEN) map.getPane(pane(id).formen)!.dataset.punkte = String(punkte);
  };

  /**
   * Setzt eine Rechnung in die Ebene: Wände und Striche weg, die Ringe der
   * Flächen neu, dann Wände vor Strichen dazu, so liegen Ränder über der
   * Wand und alles über den Flächen. Eine Fläche ohne Ringe nimmt die Karte
   * weg, ganz verdeckt oder ausserhalb ist sie kein Ziel; kommt sie wieder,
   * rücken alle Flächen in ihrer Reihenfolge nach vorn, sonst läge sie über
   * den späteren.
   */
  const setze = (id: string, g: Geladen, neu: Ansicht): void => {
    for (const w of g.waende) g.formen.removeLayer(w);
    for (const s of g.striche) g.formen.removeLayer(s.linie);
    let wieder = false;
    for (const { flaeche, index } of g.flaechen) {
      const ringe = neu.flaechen[index]!;
      if (!ringe.length) {
        g.formen.removeLayer(flaeche);
        continue;
      }
      flaeche.setLatLngs(ringe);
      if (!g.formen.hasLayer(flaeche)) {
        g.formen.addLayer(flaeche);
        wieder = true;
      }
    }
    if (wieder) for (const { flaeche } of g.flaechen) if (g.formen.hasLayer(flaeche)) flaeche.bringToFront();
    for (const w of neu.waende) g.formen.addLayer(w);
    for (const s of neu.striche) g.formen.addLayer(s.linie);
    [g.waende, g.striche, g.schirm] = [neu.waende, neu.striche, neu.schirm];
    versetze(g.striche, faktor());
    zaehle(id, neu.punkte);
  };

  /** Rechnet eine Ebene neu, wenn sich der Zoom geändert hat oder das Fenster aus dem gerechneten Ausschnitt läuft. */
  const ansichtNeu = async (id: string, g: Geladen): Promise<void> => {
    const [x0, y0, x1, y1] = fenster();
    const [r0, r1, r2, r3] = g.schirm.rahmen;
    if (g.schirm.faktor === faktor() && x0 >= r0 && y0 >= r1 && x1 <= r2 && y1 <= r3) return;
    const nummer = ++g.nummer;
    const neu = await ansichtJetzt(g.quelle, g.renderer);
    // Ist die Ebene weg, neu geladen oder schon wieder neu gerechnet, gilt dieses Ergebnis nicht.
    if (geladen.get(id) !== g || g.nummer !== nummer) return;
    setze(id, g, neu);
  };
  let ansichtWarten: ReturnType<typeof setTimeout> | undefined;
  const ansichtGeaendert = (): void => {
    clearTimeout(ansichtWarten);
    ansichtWarten = setTimeout(() => {
      for (const [id, g] of geladen) void ansichtNeu(id, g).catch((fehler: unknown) => console.error(id, fehler));
    }, REGEL.warten);
  };

  /**
   * Lädt eine Ebene und zeichnet sie. Eine Aktionsebene kommt aus `api/` und
   * zeichnet nur neu, wenn sich ihr Inhalt geändert hat oder `erzwingen` gilt.
   */
  const ladeEbene = async (e: Eintrag, erzwingen = false): Promise<void> => {
    if (abgewiesen.get(e.id) === e.version) return;
    const [mod, name] = e.id.split(':') as [string, string];
    // Die Bilder auch einer Aktionsebene liegen öffentlich unter layers/.
    const ordner = `${wurzel}/layers/${mod}`;
    let datei: unknown;
    let stand = e.version;
    if (e.player) {
      const wert = await spielerEbene(mod, name, GRENZEN.datei);
      if (wert === 'abgemeldet') return abgemeldet();
      // Ohne Datei hat der Spieler in dieser Ebene noch nichts.
      datei = wert === 'fehlt' ? undefined : wert;
      stand = JSON.stringify(datei) ?? '';
      if (!erzwingen && geladen.get(e.id)?.stand === stand) return;
    }
    const nummer = (auftrag.get(e.id) ?? 0) + 1;
    auftrag.set(e.id, nummer);
    // Gilt, solange niemand umgeschaltet hat und die Ebene noch in der Liste steht und an ist.
    const gilt = () => {
      const jetzt = eintraege.find((x) => x.id === e.id);
      return auftrag.get(e.id) === nummer && jetzt !== undefined && an(jetzt);
    };
    if (!e.player) datei = await json(`${ordner}/${name}.json`, GRENZEN.datei);
    if (istObjekt(datei) && (datei.permission !== undefined || datei.web === false)) {
      if (!gilt()) return;
      console.warn(`${ordner}/${name}.json: Ebene mit permission oder web: false, übergangen`);
      abgewiesen.set(e.id, e.version);
      // Eine ältere version, die schon steht, weicht.
      weg(e.id);
      return;
    }
    let objekte = istObjekt(datei) && Array.isArray(datei.objects) ? datei.objects : [];
    if (objekte.length > GRENZEN.objekte) {
      console.warn(`${e.id}: ${objekte.length} Objekte, gezeigt die ersten ${GRENZEN.objekte}`);
      objekte = objekte.slice(0, GRENZEN.objekte);
    }
    const warne = (text: string) => console.warn(`${e.id}: ${text}`);
    const formen = objekte.map((o) => (istObjekt(o) ? form(o, warne) : undefined)).filter((f): f is Form => f !== undefined);
    const schriften = objekte.map((o) => (istObjekt(o) ? schriftzug(o) : undefined)).filter((s): s is Schriftzug => s !== undefined);
    // Nadeln und Banner in der Reihenfolge der Datei: In der Ebene liegt das spätere oben.
    let orte = objekte.map((o) => nadel(o) ?? banner(o)).filter((o): o is Nadel | Banner => o !== undefined);
    if (orte.length > GRENZEN.nadeln) {
      console.warn(`${e.id}: ${orte.length} Nadeln und Banner, gezeigt die ersten ${GRENZEN.nadeln}`);
      orte = orte.slice(0, GRENZEN.nadeln);
    }
    // Ein Objekt mit Knöpfen, aber ohne Tafel, bekommt eine nur mit den Knöpfen; Linien und Schrift haben keine.
    const mitKnopf = (id: string) => knoepfe.has(objektSchluessel(e.id, id));
    for (const o of [...formen, ...orte]) if (o.panel === undefined && !('linie' in o && o.linie) && mitKnopf(o.id)) o.panel = [];
    const tafelDerEbene = (bausteine: unknown[], objekt: string) => {
      const teil = tafel(bausteine, ordner, e.version);
      teil.append(knopfLeiste(e.id, objekt));
      return teil;
    };
    // Der Satz dieses Baums, einmal je Laden und nur, wenn ein Banner einen Entwurf nennt.
    let satz: Satz | undefined;
    if (umgebung.satz && orte.some((o) => 'design' in o && o.design)) {
      const satzOrdner = `${ordner}/banner/${name}/${umgebung.satz}`;
      const daten = satzDaten(await json(`${satzOrdner}/satz.json?v=${encodeURIComponent(e.version)}`, 4096));
      if (daten) satz = { ordner: satzOrdner, ...daten };
      else console.warn(`${satzOrdner}/satz.json fehlt oder taugt nicht, Banner mit image`);
    }
    const { renderer, formen: formPane } = pane(e.id);
    // Die Ziele der Flächen und die Schrift einmal je version, die Ansicht je Ausschnitt; hier vor dem Tausch.
    const flaechen = flaechenZiele(formen, { renderer, blick, tafel: tafelDerEbene, bediene });
    const [schriftLagen, ansicht] = await Promise.all([schriftLagenFuer(schriften, formPane), ansichtJetzt(formen, renderer)]);
    // Icons und Symbole gelten nur für dieses Laden und fallen danach weg.
    const icons = new Map<string, Promise<HTMLCanvasElement>>();
    const symbole = new Map<string, Promise<HTMLImageElement>>();
    const marker = await Promise.all(
      orte.map(async (o, index) => {
        const icon = 'symbol' in o ? await nadelIcon(o, ordner, e.version, icons, symbole) : await bannerIcon(o, ordner, e.version, symbole, satz);
        if (!icon) return undefined;
        const y = o.y !== undefined ? o.y + 1 : await oberflaeche(o.at[0], o.at[1]);
        const [px, py] = bildpunkt(o.at[0], iso ? y : 0, o.at[1], blick);
        // In der Ebene liegt das spätere oben, gleich wo auf dem Schirm.
        const m = L.marker(L.latLng(py, px), {
          icon: icon(groessenStufe()),
          pane: pane(e.id).nadeln,
          zIndexOffset: index * 100_000,
          interactive: o.panel !== undefined,
          keyboard: o.panel !== undefined,
          title: o.name ?? '',
          alt: o.name ?? '',
        });
        const panel = o.panel;
        if (panel) {
          // Vom Fuss bis zur Oberkante des Icons jetzt, auf seiner Stufe: Die Tafel steht darüber, auch beim Kopf mit dem Fuss in der Mitte.
          const hoehe = () => (m.options.icon!.options.iconAnchor as L.PointTuple)[1];
          const oeffne = tafelAn(m, () => tafelDerEbene(panel, o.id), () => m.getElement(), () => m.getLatLng(), hoehe);
          m.on('keypress', (ereignis) => {
            if (ereignis.originalEvent.key === 'Enter') oeffne(true);
          });
        }
        iconAuf.set(m, icon);
        return m;
      }),
    );
    // Hat jemand inzwischen umgeschaltet, gilt sein Auftrag; erst jetzt die alte Gruppe ersetzen.
    if (!gilt()) return;
    weg(e.id);
    // Erst Flächen, Wände, Ränder und Linien, in dieser Reihenfolge von `setze`; zuletzt die Schrift, je in einem eigenen svg über dem des Renderers.
    const formGruppe = L.layerGroup().addTo(map);
    const gruppe = L.layerGroup(marker.filter((m): m is L.Marker => m !== undefined)).addTo(map);
    const g: Geladen = { version: e.version, stand, gruppe, formen: formGruppe, quelle: formen, renderer, flaechen, waende: [], striche: [], schirm: ansicht.schirm, nummer: 0 };
    geladen.set(e.id, g);
    setze(e.id, g, ansicht);
    for (const s of schriftLagen) formGruppe.addLayer(s);
  };

  const entferne = (id: string): void => {
    auftrag.set(id, (auftrag.get(id) ?? 0) + 1);
    weg(id);
  };

  // Die Liste zum Umschalten, oben rechts unter Kompass und Umschalter.
  const control = new L.Control({ position: 'topright' });
  const kasten = L.DomUtil.create('details', 'ebenen');
  // Deutsch wie die übrige UI; die Namen der Ebenen folgen der Sprache des Browsers.
  L.DomUtil.create('summary', '', kasten).textContent = 'Ebenen';
  const inhalt = L.DomUtil.create('div', 'ebenen-liste', kasten);
  L.DomEvent.disableClickPropagation(kasten);
  L.DomEvent.disableScrollPropagation(kasten);
  control.onAdd = () => kasten;

  let gezeigteListe = '';
  /** Nach dem Umstellen: der Pfeil, der den Fokus wieder bekommt. */
  let gedrueckt: { id: string; schritt: -1 | 1 } | undefined;
  /** Baut die Liste nur, wenn sich Ebenen, Namen oder Reihenfolge ändern; so bleibt der Fokus. */
  const zeigeListe = (): void => {
    const stand = JSON.stringify([spieler !== undefined, reihe.lies() !== undefined, eintraege.map((e) => [e.id, e.name, e.order])]);
    if (stand !== gezeigteListe) {
      gezeigteListe = stand;
      inhalt.replaceChildren();
      // Die Mitspieler, eingebaut, nur erkannt; siehe docs/frontend.md, „Mitspieler“.
      if (spieler) {
        const zeile = L.DomUtil.create('label', '', inhalt);
        const box = L.DomUtil.create('input', '', zeile);
        box.type = 'checkbox';
        box.dataset.id = MITSPIELER;
        zeile.append(' Mitspieler');
        box.addEventListener('change', () => {
          wahl.setze(MITSPIELER, box.checked);
          if (box.checked) void holeSicht();
          else leereSicht();
        });
      }
      eintraege.forEach((e, i) => {
        const name = e.name[sprache] ?? e.name.de ?? e.name.en ?? e.id;
        const zeile = L.DomUtil.create('div', 'ebenen-zeile', inhalt);
        const etikett = L.DomUtil.create('label', '', zeile);
        const box = L.DomUtil.create('input', '', etikett);
        box.type = 'checkbox';
        box.dataset.id = e.id;
        etikett.append(` ${name}`);
        box.addEventListener('change', () => {
          wahl.setze(e.id, box.checked);
          if (box.checked) void ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler));
          else entferne(e.id);
        });
        // Umstellen wie im Mod, siehe docs/entscheidungen/0113-reihenfolge-der-ebenen-je-betrachter.md.
        for (const [schritt, zeichen, wohin] of [[-1, '▲', 'nach oben'], [1, '▼', 'nach unten']] as const) {
          const pfeil = L.DomUtil.create('button', 'ebenen-pfeil', zeile);
          pfeil.type = 'button';
          pfeil.textContent = zeichen;
          pfeil.dataset.id = e.id;
          pfeil.dataset.schritt = String(schritt);
          pfeil.setAttribute('aria-label', `${name} ${wohin}`);
          pfeil.disabled = i + schritt < 0 || i + schritt >= eintraege.length;
          pfeil.addEventListener('click', () => stelleUm(e.id, schritt));
        }
      });
      if (reihe.lies()) {
        const vorgabe = L.DomUtil.create('button', 'ebenen-vorgabe', inhalt);
        vorgabe.type = 'button';
        vorgabe.textContent = 'Reihe des Servers';
        vorgabe.addEventListener('click', () => {
          reihe.setze(undefined);
          ordne();
          inhalt.querySelector<HTMLElement>('.ebenen-pfeil:not(:disabled)')?.focus();
        });
      }
      if (gedrueckt) {
        const { id, schritt } = gedrueckt;
        gedrueckt = undefined;
        // Oben oder unten angekommen, ist der gedrückte Pfeil gesperrt; dann der andere derselben Ebene.
        const pfeile = [...inhalt.querySelectorAll<HTMLButtonElement>('.ebenen-pfeil')].filter((b) => b.dataset.id === id);
        (pfeile.find((b) => b.dataset.schritt === String(schritt) && !b.disabled) ?? pfeile.find((b) => !b.disabled))?.focus();
      }
    }
    for (const box of inhalt.querySelectorAll<HTMLInputElement>('input')) {
      const e = eintraege.find((x) => x.id === box.dataset.id);
      if (e) box.checked = an(e);
      else if (box.dataset.id === MITSPIELER) box.checked = mitspielerAn();
    }
  };

  // Unter der Liste, wer angemeldet ist, oder wie man sich anmeldet.
  const konto = L.DomUtil.create('div', 'ebenen-konto', kasten);
  let gezeigtesKonto = '';
  /** Baut die Zeile nur, wenn sich etwas ändert; so bleibt der Fokus auf „Abmelden“. */
  const zeigeKonto = (): void => {
    const stand = JSON.stringify([spieler?.uuid, spieler?.name, spieler?.nurLesen, mitServer, ipv6, meldung]);
    if (stand !== gezeigtesKonto) {
      gezeigtesKonto = stand;
      konto.replaceChildren();
      if (spieler?.nurLesen) {
        // Über die IP gibt es nichts abzumelden; klicken geht nur angemeldet.
        L.DomUtil.create('span', '', konto).textContent = spieler.name ? `Erkannt als ${spieler.name} (über die IP)` : 'Erkannt (über die IP)';
        L.DomUtil.create('div', 'ebenen-klein', konto).textContent = 'Nicht du? Im Spiel /karte anmelden';
      } else if (spieler) {
        L.DomUtil.create('span', '', konto).textContent = spieler.name ? `Angemeldet als ${spieler.name} ` : 'Angemeldet ';
        const knopf = L.DomUtil.create('button', 'ebenen-abmelden', konto);
        knopf.type = 'button';
        knopf.textContent = 'Abmelden';
        knopf.addEventListener('click', () => void abmelden().then(abgemeldet));
      } else if (mitServer) {
        konto.textContent = ipv6
          ? 'Über IPv6 erkennt die Karte dich nicht. Für eigene Ebenen und Mitspieler: im Spiel /karte anmelden'
          : 'Anmelden für eigene Ebenen und Mitspieler: im Spiel /karte anmelden';
      }
      if (meldung) {
        const zeile = L.DomUtil.create('div', 'ebenen-meldung', konto);
        zeile.setAttribute('role', 'status');
        zeile.textContent = meldung;
      }
      konto.hidden = konto.childNodes.length === 0;
    }
    if ((eintraege.length > 0 || !konto.hidden) && !control.getContainer()) control.addTo(map);
  };

  /** Die Knöpfe eines Objekts unter seiner Tafel, mit dem Hinweis darunter. */
  const knopfLeiste = (ebeneId: string, objekt: string): HTMLElement => {
    const leiste = L.DomUtil.create('div', 'tafel-knoepfe');
    leiste.dataset.objekt = objektSchluessel(ebeneId, objekt);
    fuelle(leiste);
    return leiste;
  };
  /**
   * Bringt eine Leiste auf den Stand von `knoepfe`. Vorhandene Knöpfe bleiben
   * Elemente, nur Text und Zustand ändern sich; so behält ein Knopf den Fokus.
   * Ein laufender Klick sperrt seinen Knopf mit `aria-disabled`, nicht mit
   * `disabled`, das den Fokus nähme.
   */
  const fuelle = (leiste: HTMLElement): void => {
    const schluessel = leiste.dataset.objekt!;
    const da = new Map([...leiste.querySelectorAll<HTMLButtonElement>('button')].map((b) => [b.dataset.knopf, b]));
    const neu: HTMLElement[] = (knoepfe.get(schluessel) ?? []).map((k) => {
      let b = da.get(k.knopf);
      if (!b) {
        b = L.DomUtil.create('button', 'tafel-knopf');
        b.type = 'button';
        b.dataset.knopf = k.knopf;
        const { ebene: ebeneId, objekt } = k;
        b.addEventListener('click', () => void klick(ebeneId, objekt, b!.dataset.knopf!));
      }
      b.textContent = k.label[sprache] ?? k.label.de ?? k.label.en ?? '';
      b.setAttribute('aria-pressed', String(k.on));
      const busy = laufend.has(knopfSchluessel(k));
      b.setAttribute('aria-busy', String(busy));
      b.setAttribute('aria-disabled', String(busy || spieler?.nurLesen === true));
      return b;
    });
    let hinweis = leiste.querySelector<HTMLElement>('.tafel-hinweis');
    if (!hinweis) {
      hinweis = L.DomUtil.create('div', 'tafel-hinweis');
      hinweis.setAttribute('role', 'status');
    }
    hinweis.textContent = hinweise.get(schluessel) ?? (spieler?.nurLesen && neu.length ? NUR_LESEN : '');
    neu.push(hinweis);
    leiste.hidden = neu.length === 1 && !hinweis.textContent;
    if (neu.length !== leiste.children.length || neu.some((b, i) => leiste.children[i] !== b)) leiste.replaceChildren(...neu);
  };
  const fuelleAlle = (): void => {
    for (const leiste of document.querySelectorAll<HTMLElement>('.tafel-knoepfe')) fuelle(leiste);
  };

  /**
   * Übernimmt die Knöpfe. Bekommt ein Objekt seine ersten Knöpfe oder
   * verliert es alle, zeichnet seine Ebene neu: Ein Objekt ohne Tafel hat
   * dann eine oder keine mehr.
   */
  const setzeKnoepfe = (liste: readonly Knopf[]): void => {
    const neu = new Map<string, Knopf[]>();
    for (const k of liste) {
      const s = objektSchluessel(k.ebene, k.objekt);
      neu.set(s, [...(neu.get(s) ?? []), k]);
    }
    const geaendert = new Set<string>();
    for (const s of new Set([...knoepfe.keys(), ...neu.keys()])) {
      if (knoepfe.has(s) !== neu.has(s)) geaendert.add((JSON.parse(s) as [string, string])[0]);
    }
    knoepfe = neu;
    fuelleAlle();
    for (const e of eintraege) if (geaendert.has(e.id) && geladen.has(e.id)) void ladeEbene(e, true).catch((fehler: unknown) => console.error(e.id, fehler));
  };
  const holeKnoepfe = async (): Promise<void> => {
    const antwort = await holeKnoepfeVomServer(kennungGilt);
    if (antwort === 'abgemeldet') abgemeldet();
    // Ohne Antwort bleiben die alten bis zur nächsten Frage.
    else if (antwort !== 'fehlt') setzeKnoepfe(antwort);
  };

  /** Ohne Sitzung: Aktionsebenen und Knöpfe fallen weg. */
  const abgemeldet = (): void => {
    if (!spieler) return;
    spieler = undefined;
    setzeKnoepfe([]);
    leereSicht();
    zeigeListe();
    zeigeKonto();
    void abgleichen(roh);
  };

  /**
   * Fragt, wer angemeldet oder über die IP erkannt ist, und holt dann die
   * Knöpfe. Kommt die Seite über IPv6 und kennt der Server einen Namen nur
   * mit IPv4, erkennt sie sich dort und tauscht das Token gegen eine
   * lesende Sitzung. Siehe docs/benutzung/server.md, „Erkennen über IPv4“.
   */
  const pruefe = async (): Promise<void> => {
    let antwort = await ich();
    if (antwort.art === 'gast' && antwort.erkennen) {
      const tausch = await erkennen(antwort.erkennen);
      if (tausch && (await anmelden(tausch)) === 204) antwort = await ich();
    }
    mitServer = antwort.art !== 'fehlt';
    ipv6 = antwort.art === 'gast' && antwort.ipv6;
    spieler = antwort.art === 'erkannt' ? antwort.spieler : undefined;
    if (spieler) await holeKnoepfe();
    else setzeKnoepfe([]);
    zeigeListe();
    zeigeKonto();
    if (spieler) void holeSicht();
    else leereSicht();
  };

  /** Löst ein Anmelde-Token ein; danach ist die Liste offen, mit „Angemeldet als …“. */
  const meldeAn = async (token: string): Promise<void> => {
    const status = await anmelden(token);
    meldung = status === 204 ? '' : status === 401 ? 'Anmeldelink abgelaufen oder schon benutzt; neu im Spiel: /karte anmelden' : 'Anmelden ging nicht';
    await pruefe();
    if (status === 204) kasten.open = true;
  };

  /** Nach 0,3, 0,8, 1,5 und 3 s fragt die Seite nach einem Klick nach dem neuen Zustand. */
  const NACH_KLICK = [300, 500, 700, 1500];

  /**
   * Ein Klick auf einen Knopf: Der Server nimmt ihn mit 202 an, den neuen
   * Zustand setzt erst das Plugin. Bis er da ist, ist der Knopf gesperrt;
   * kommt er in 3 s nicht, wird der Knopf mit dem alten Zustand frei, und
   * den Rest holt das Nachfragen alle 30 s.
   */
  const klick = async (ebeneId: string, objekt: string, knopfId: string): Promise<void> => {
    const schluessel = objektSchluessel(ebeneId, objekt);
    const k = knoepfe.get(schluessel)?.find((x) => x.knopf === knopfId);
    // Über die IP nur sehen: Der Knopf ist gesperrt, sein Hinweis sagt, wie man schaltet.
    if (!k || laufend.has(knopfSchluessel(k)) || spieler?.nurLesen) return;
    laufend.add(knopfSchluessel(k));
    hinweise.delete(schluessel);
    fuelleAlle();
    const status = await aktion(k);
    let hinweis = '';
    if (status === 202) {
      hinweis = 'noch keine Antwort';
      for (const ms of NACH_KLICK) {
        await new Promise((fertig) => setTimeout(fertig, ms));
        await holeKnoepfe();
        if (!spieler) break;
        // Auch ein Knopf, der weggefallen ist, hat geantwortet.
        if (knoepfe.get(schluessel)?.find((x) => x.knopf === knopfId)?.on !== k.on) {
          hinweis = '';
          break;
        }
      }
    } else if (status === 401) {
      abgemeldet();
    } else if (status === 404) {
      // Den Knopf gibt es nicht mehr.
      await holeKnoepfe();
    } else if (status === 403) {
      hinweis = NUR_LESEN;
    } else {
      hinweis = 'Klick nicht angenommen';
    }
    laufend.delete(knopfSchluessel(k));
    if (hinweis) hinweise.set(schluessel, hinweis);
    fuelleAlle();
    // Der Klick ändert womöglich Inhalte je Spieler, auch in anderen Ebenen.
    if (status === 202 && !hinweis) for (const e of eintraege) if (e.player && geladen.has(e.id)) void ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler));
  };

  // Die Mitspieler auf der Karte, siehe docs/frontend.md, „Mitspieler“.
  map.createPane('mitspieler').style.zIndex = '590';
  /** Je Spieler seine Figur und wofür ihr Icon gezeichnet ist; `ich:` vor der UUID für den Betrachter selbst. */
  const figuren = new Map<string, { m: L.Marker; fuer: string }>();
  /** Das zuletzt gesehene `jetzt` und wann es kam, nach der Uhr der Seite. */
  let zuletzt: { jetzt?: number; seit: number } = { seit: 0 };
  let sichtLaeuft = false;
  const leereSicht = (): void => {
    for (const { m } of figuren.values()) m.remove();
    figuren.clear();
  };
  /**
   * Eine Figur: das Gesicht, sonst ein Punkt, darunter der Name, nur als
   * Text. Ein Pixel des Gesichts ist `k` = 2 · max(1, round(devicePixelRatio))
   * Pixel des Geräts breit, wie bei den Bannern Pixel auf Pixel.
   */
  const figur = (name: string, selbst: boolean, gesicht: ImageBitmap | undefined, k: number): L.DivIcon => {
    const html = L.DomUtil.create('div', 'mitspieler-figur');
    let seite = 12;
    if (gesicht) {
      const leinwand = L.DomUtil.create('canvas', 'mitspieler-kopf', html);
      leinwand.width = leinwand.height = KOPF.seite * k;
      seite = (KOPF.seite * k) / devicePixelRatio;
      leinwand.style.width = leinwand.style.height = `${seite}px`;
      const ctx = leinwand.getContext('2d')!;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(gesicht, 0, 0, KOPF.seite * k, KOPF.seite * k);
    } else {
      L.DomUtil.create('span', 'mitspieler-punkt', html);
    }
    L.DomUtil.create('span', 'mitspieler-name', html).textContent = name;
    return L.divIcon({ html, className: selbst ? 'mitspieler ich' : 'mitspieler', iconSize: [seite, seite], iconAnchor: [seite / 2, seite / 2] });
  };
  /**
   * Holt die Sicht und stellt die Figuren: neue dazu, bekannte verschoben,
   * fehlende weg. Ändert sich `jetzt` 15 s lang nicht, nach der Uhr der Seite,
   * nie der des Browsers gegen die des Servers, gelten alle als veraltet.
   */
  const holeSicht = async (): Promise<void> => {
    if (!mitspielerAn() || sichtLaeuft) return;
    sichtLaeuft = true;
    try {
      const s = await sicht();
      if (s === 'abgemeldet') return abgemeldet();
      if (s !== 'fehlt' && s.jetzt !== zuletzt.jetzt) zuletzt = { jetzt: s.jetzt, seit: performance.now() };
      if (s === 'fehlt' || s.jetzt === undefined || performance.now() - zuletzt.seit > SICHT.alt) return leereSicht();
      const selbst = spieler;
      const alle = [...(s.ich && selbst ? [{ uuid: `ich:${selbst.uuid}`, name: selbst.name ?? 'Ich', ...s.ich, selbst: true }] : []), ...s.spieler.map((p) => ({ ...p, selbst: false }))];
      // Die Mitte des Blocks, auf der Oberfläche wie eine Nadel ohne y.
      const lagen = await Promise.all(alle.map(async ({ x, z }) => bildpunkt(x + 0.5, iso ? await oberflaecheJetzt(x + 0.5, z + 0.5) : 0, z + 0.5, blick)));
      const gesichter = await Promise.all(alle.map((p) => (p.kopf ? kopfBild(p.kopf) : Promise.resolve(undefined))));
      if (!mitspielerAn()) return;
      const k = 2 * Math.max(1, Math.round(devicePixelRatio));
      const da = new Set<string>();
      alle.forEach((p, i) => {
        const [px, py] = lagen[i]!;
        da.add(p.uuid);
        // Ein neues Icon nur mit anderem Gesicht, anderem Namen oder anderem devicePixelRatio.
        const fuer = JSON.stringify([gesichter[i] ? p.kopf : undefined, p.name, devicePixelRatio]);
        const f = figuren.get(p.uuid);
        if (f) {
          f.m.setLatLng(L.latLng(py, px));
          if (f.fuer !== fuer) f.m.setIcon(figur(p.name, p.selbst, gesichter[i], k));
          f.fuer = fuer;
        } else {
          const m = L.marker(L.latLng(py, px), { icon: figur(p.name, p.selbst, gesichter[i], k), pane: 'mitspieler', interactive: false, keyboard: false });
          figuren.set(p.uuid, { m: m.addTo(map), fuer });
        }
      });
      for (const [uuid, { m }] of figuren) {
        if (da.has(uuid)) continue;
        m.remove();
        figuren.delete(uuid);
      }
    } finally {
      sichtLaeuft = false;
    }
  };

  /** Liest die Liste; nimmt weg, was nicht mehr an ist, und lädt neu, was an ist und sich geändert hat. */
  const abgleichen = async (neu: unknown[]): Promise<void> => {
    roh = neu;
    if (neu.length > GRENZEN.ebenen) console.warn(`${wurzel}/layers.json: ${neu.length} Ebenen, gezeigt die ersten ${GRENZEN.ebenen}`);
    const alle = neu
      .slice(0, GRENZEN.ebenen)
      .map(eintrag)
      .filter((e): e is Eintrag => e !== undefined)
      .sort((a, b) => b.order - a.order || a.id.localeCompare(b.id));
    // Aktionsebenen nur angemeldet; siehe docs/frontend.md, „Aktionsebenen“.
    nachServer = alle;
    ordne();
    zeigeKonto();
    // Auch, wenn der Betreiber `visible` ändert und der Betrachter nie gewählt hat.
    for (const id of [...geladen.keys()]) if (!eintraege.some((e) => e.id === id && an(e))) entferne(id);
    // Eine Aktionsebene fragt jedes Mal; neu zeichnet sie nur mit neuem Inhalt.
    const zuLaden = eintraege.filter((e) => an(e) && (e.player || geladen.get(e.id)?.version !== e.version) && laeuft.get(e.id) !== e.version);
    await Promise.all(
      zuLaden.map((e) => {
        laeuft.set(e.id, e.version);
        return ladeEbene(e)
          .catch((fehler: unknown) => console.error(e.id, fehler))
          .finally(() => {
            if (laeuft.get(e.id) === e.version) laeuft.delete(e.id);
          });
      }),
    );
  };

  /** Die Ebenen aus `layers.json`, oder `undefined`, wenn sie fehlt oder kaputt ist. */
  const holeListe = async (): Promise<unknown[] | undefined> => {
    const liste = await json(`${wurzel}/layers.json`, GRENZEN.liste).catch(() => undefined);
    return istObjekt(liste) && Array.isArray(liste.layers) ? liste.layers : undefined;
  };
  const nachfragen = async (): Promise<void> => {
    const liste = await holeListe();
    // Erst die Knöpfe, dann die Ebenen: So zeichnet eine Ebene schon mit ihnen.
    if (spieler) await holeKnoepfe();
    if (liste) await abgleichen(liste);
  };

  const erste = await holeListe();
  // Bei jedem Laden, auch ohne layers.json: Erkannt über die IP sieht man
  // Mitspieler; ein Gast bekommt 204, keinen Fehler in der Konsole.
  if (umgebung.token) await meldeAn(umgebung.token);
  else await pruefe();
  // Ein Anmeldelink in einem Tab, in dem die Karte schon offen ist, lädt die Seite nicht neu.
  addEventListener('hashchange', () => {
    const token = nimmToken();
    if (token) void meldeAn(token).then(() => abgleichen(roh));
  });
  setInterval(() => {
    if (document.visibilityState === 'visible') void holeSicht();
  }, SICHT.takt);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void holeSicht();
  });
  // Ohne layers.json beim Laden fragt die Karte bis zum Neuladen nicht nach Ebenen.
  if (!erste) return;
  if (iso && !karten) console.warn(`${wurzel}: Ebenen ohne Höhen, im iso auf seaLevel`);
  // Erst anmelden, dann abgleichen: Ein Fehler beim ersten Abgleich hält das
  // Nachfragen nicht auf.
  map.on('zoomend', () => {
    for (const g of geladen.values()) versetze(g.striche, faktor());
  });
  map.on('zoomend moveend', ansichtGeaendert);
  map.on('zoomend', groesseNachZoom);
  setInterval(() => {
    if (document.visibilityState === 'visible') void nachfragen();
  }, TAKT);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void nachfragen();
  });
  // Escape und ein Klick daneben schliessen zuerst nur die Tafel, siehe
  // docs/benutzung/ebenen.md, „Infotafel“: Im Capture, so erreichen sie weder
  // Leaflet noch die Leiste. War der Fokus in der Tafel, geht er an ihr Ziel zurück.
  document.addEventListener(
    'keydown',
    (ereignis) => {
      const jetzt = offen;
      if (ereignis.key !== 'Escape' || !jetzt) return;
      ereignis.stopPropagation();
      const darin = jetzt.popup.getElement()?.contains(ereignis.target as Node);
      map.closePopup(jetzt.popup);
      if (darin) (jetzt.element() as HTMLElement | SVGElement | undefined)?.focus({ preventScroll: true });
    },
    true,
  );
  map.getContainer().addEventListener(
    'click',
    (ereignis) => {
      const jetzt = offen;
      // Ein Ziel mit Tafel öffnet seine eigene; nach einem Ziehen gilt der Klick nicht, wie bei Leaflet.
      const ziel = ereignis.target as Element;
      if (!jetzt || ziel.closest('.leaflet-popup, .tafel-ziel, .leaflet-control') || (map.dragging as unknown as { moved(): boolean }).moved()) return;
      ereignis.stopPropagation();
      map.closePopup(jetzt.popup);
    },
    true,
  );
  // Ändert sich devicePixelRatio ohne resize, etwa beim Wechsel auf einen
  // anderen Bildschirm oder beim Zoom des Browsers, zeichnet die Karte die
  // geladenen Ebenen neu, damit die Icons Pixel auf Pixel bleiben.
  const beiDpr = (): void => {
    matchMedia(`(resolution: ${devicePixelRatio}dppx)`).addEventListener(
      'change',
      () => {
        beiDpr();
        for (const e of eintraege) if (geladen.has(e.id)) void ladeEbene(e, true).catch((fehler: unknown) => console.error(e.id, fehler));
      },
      { once: true },
    );
  };
  beiDpr();
  await abgleichen(erste);
}
