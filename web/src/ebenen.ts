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
import { bereich, form, hatFlaeche, versetze, zeichne, zuege, type Form, type Strich } from './formen';
import { bildpunkt, oberflaeche as hoeheAuf, rechteck, schriftPfad, zurKamera, type Blick, type Gelaende, type Rechteck } from './gelaende';
import { FRISCH, LEER, type Hoehenkarten } from './hoehen';
import { REGION } from './pick';
import { farbe, istObjekt, istText, istZahl, punkt } from './pruefen';
import { KAPPE, ladeSchrift, Schrift, schriftzug, SVG, type Schriftzug } from './schrift';

/** Ein Eintrag in `layers.json`. */
interface Eintrag {
  id: string;
  name: { de?: string; en?: string };
  visible: boolean;
  order: number;
  version: string;
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

/** Grenzen aus docs/benutzung/ebenen.md, „Grenzen“. */
const GRENZEN = { liste: 64 * 1024, datei: 4 * 1024 * 1024, ebenen: 64, objekte: 10_000, nadeln: 1000, bausteine: 64, bild: 512, punkte: 20, banner: [32, 64] };

/**
 * So viele Bytes Höhen lädt die Webkarte höchstens je Ebene, für Formen und
 * für Schrift je für sich: mit `heightsCell` 4 sind das 1024 Regionen. Darüber
 * nehmen Formen `heights` statt `ground`, und reicht es auch dafür nicht,
 * liegt die ganze Ebene auf `seaLevel`. Eine Grenze der Webkarte, nicht des
 * Formats. Siehe docs/frontend.md, „Ebenen“.
 */
const HOEHEN_BYTES = 32 * 1024 * 1024;

/** Bytes einer Region Höhen mit Zellen der Kante `zelle`. */
const regionBytes = (zelle: number) => (REGION / zelle) ** 2 * 2;

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
  const iso = blick.p.y > 0;
  const grund = seaLevel ?? 64;
  const c = heightsCell ?? 4;
  /** Ohne Höhen: eben auf `grund`. */
  const eben: Gelaende = { c, grund, zelle: () => undefined, max: grund + 1 };

  /** Die Oberseite des Geländes je Punkt, einmal gerechnet; siehe ebenen.md, „Die Oberfläche im iso“. */
  const oberflaechen = new Map<string, Promise<number>>();
  const oberflaeche = (x: number, z: number): Promise<number> => {
    const schluessel = `${x},${z}`;
    let wert = oberflaechen.get(schluessel);
    if (!wert) {
      wert = (async () => {
        if (!iso || !karten || !heightsCell) return grund + 1;
        // Vier Zellen um den Punkt und für den Ersatz zwei weitere in jede Richtung.
        await karten.lade([-3 * c, 3 * c].flatMap((dx) => [-3 * c, 3 * c].map((dz) => [Math.floor(x + dx), 0, Math.floor(z + dz)] as [number, number, number])));
        return hoeheAuf({ ...eben, zelle: (i, j) => karten.hoehe(i * c, j * c) }, x, z);
      })();
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
  const regionenFuer = (formen: readonly Form[], schriften: readonly Schriftzug[]): Set<number> => {
    const menge = new Set<number>();
    if (!iso || !karten || !heightsCell) return menge;
    const { wx, wz, steigung } = zurKamera(blick);
    const weit = ((umgebung.maxY ?? 319) - (umgebung.minY ?? -64)) / steigung;
    const nimm = ([x0, z0, x1, z1]: Rechteck): void => {
      let [ax, az] = [Math.min(x0, x0 + wx * weit) - 3 * c, Math.min(z0, z0 + wz * weit) - 3 * c];
      let [bx, bz] = [Math.max(x1, x1 + wx * weit) + 3 * c, Math.max(z1, z1 + wz * weit) + 3 * c];
      const area = umgebung.area;
      if (area) [ax, az, bx, bz] = [Math.max(ax, area[0]), Math.max(az, area[1]), Math.min(bx, area[2]), Math.min(bz, area[3])];
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
        for (const v of karte) if (v !== LEER && v + 1 > max) max = v + 1;
      }),
    );
    const n = REGION / cg;
    // Die zuletzt gefragte Region: Nachbarn liegen meist in derselben.
    let [letzte, eintrag]: [number, { karte: Int16Array; n: number } | undefined] = [Number.NaN, undefined];
    return {
      c: cg,
      grund,
      max,
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
    /** Nadeln und Banner. */
    gruppe: L.LayerGroup;
    /** Flächen, Ränder, Linien und Schrift, in dieser Reihenfolge. */
    formen: L.LayerGroup;
    striche: Strich[];
  }
  const geladen = new Map<string, Geladen>();
  /** Je Ebene ein Zähler: Ein Laden, das ein späteres Umschalten überholt, verwirft sich selbst. */
  const auftrag = new Map<string, number>();
  /** Je Ebene die `version` mit `permission` oder `web: false`; die holt die Karte nicht noch einmal. */
  const abgewiesen = new Map<string, string>();
  let eintraege: Eintrag[] = [];
  /** Die Tafel, die gerade offen ist, das Element ihres Ziels, das den Fokus zurückbekommt, und ob sie gehalten ist. */
  let offen: { popup: L.Popup; element: () => Element | undefined; gehalten: () => boolean } | undefined;

  /**
   * Die Tafel eines Ziels, siehe docs/benutzung/ebenen.md, „Infotafel“:
   * Ruht der Zeiger 50 ms darauf, erscheint sie dort; 300 ms nachdem er Ziel
   * und Tafel verlassen hat, schliesst sie. Ein Klick oder Tippen hält sie,
   * und solange eine gehaltene Tafel offen ist, öffnet Zeigen keine andere.
   * Gibt zurück, womit Tastatur sie öffnet, dann mit dem Fokus darin. `ort`
   * sagt, wo sie erscheint, ohne Ereignis für die Tastatur; `hoehe` hebt sie
   * über das Icon, damit sie es nicht deckt.
   */
  const tafelAn = (
    ziel: L.Marker | L.Polygon,
    inhalt: () => HTMLElement,
    element: () => Element | undefined,
    ort: (ereignis?: L.LeafletMouseEvent) => L.LatLng,
    hoehe = 0,
  ): ((perTastatur: boolean) => void) => {
    // 7 ist der Versatz, den Leaflet sonst hat.
    const popup = L.popup({ ...TAFEL, offset: [0, 7 - hoehe] }).setContent(inhalt);
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

  /**
   * Das Icon einer Nadel oder eines Banners: das Bild Pixel auf Pixel, der
   * Fuss unten bei ⌊b / 2⌋, der Name darunter. Ein Pixel des Bilds ist
   * k = max(1, round(devicePixelRatio)) Pixel des Geräts breit, so bleibt
   * jedes gleich breit, auch bei 1,25 oder 1,5. Siehe docs/frontend.md, „Ebenen“.
   */
  const ortIcon = (
    bild: CanvasImageSource,
    b: number,
    h: number,
    name: string | undefined,
    banner?: { fuss: [number, number]; winkel: number },
  ): L.DivIcon => {
    // Der Fuss als Punkt auf den Kanten der Pixel: bei der Nadel und beim Bild (⌊b / 2⌋, h), beim Sprite aus satz.json.
    const [fx, fy] = banner?.fuss ?? [Math.floor(b / 2), h];
    const k = Math.max(1, Math.round(devicePixelRatio));
    const s = k / devicePixelRatio;
    const html = L.DomUtil.create('div', 'nadel');
    const kopie = document.createElement('canvas');
    [kopie.width, kopie.height] = [b * k, h * k];
    [kopie.style.width, kopie.style.height] = [`${b * s}px`, `${h * s}px`];
    const ctx = kopie.getContext('2d')!;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(bild, 0, 0, b * k, h * k);
    html.append(kopie);
    if (name && banner) {
      // Gemessen in der Kartenschrift: bannerIcon wartet, bis sie geladen ist.
      const svg = nameImBogen(name, h * s, banner.winkel);
      [svg.style.left, svg.style.top] = [`${fx * s}px`, `${fy * s}px`];
      html.append(svg);
    } else if (name) {
      // Der Name steht in der Kartenschrift; bis sie geladen ist, in der Vorgabe von style.css.
      void ladeSchrift();
      L.DomUtil.create('span', 'nadel-name', html).textContent = name;
    }
    return L.divIcon({ html, className: 'nadel-icon', iconSize: [b * s, h * s], iconAnchor: [fx * s, fy * s] });
  };

  /** Das Icon einer Nadel, fest in ihrer `size`. Icons und Symbole gelten je Laden einer Ebene. */
  const nadelIcon = async (
    n: Nadel,
    ordner: string,
    version: string,
    icons: Map<string, Promise<HTMLCanvasElement>>,
    symbole: Map<string, Promise<HTMLImageElement>>,
  ): Promise<L.DivIcon> => {
    const pfad = n.size === 'small' ? undefined : n.symbol[n.size];
    const symbol = pfad && `${ordner}/${pfad}?v=${encodeURIComponent(version)}`;
    const schluessel = `${n.size} ${n.color} ${symbol ?? ''}`;
    let leinwand = icons.get(schluessel);
    if (!leinwand) icons.set(schluessel, (leinwand = zeichneNadel(n.size, n.color, symbol, symbole)));
    const { b, h } = SCHILDE[n.size];
    return ortIcon(await leinwand, b, h, n.name);
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
  ): Promise<L.DivIcon | undefined> => {
    const [b, h] = GRENZEN.banner as [number, number];
    const v = encodeURIComponent(version);
    const passt = (geholt: HTMLImageElement | undefined) => geholt && geholt.naturalWidth <= b && geholt.naturalHeight <= h;
    if (banner.design && satz) {
      const adresse = `${satz.ordner}/${banner.capital ? 'krone/' : ''}${banner.design}.png?v=${v}`;
      const sprite = await bild(adresse, symbole).catch(() => undefined);
      const [fx, fy] = satz.fuss;
      if (sprite && passt(sprite) && fx <= sprite.naturalWidth && fy <= sprite.naturalHeight) {
        if (banner.name) await ladeSchrift();
        return ortIcon(sprite, sprite.naturalWidth, sprite.naturalHeight, banner.name, satz);
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
    return ortIcon(geholt, geholt.naturalWidth, geholt.naturalHeight, banner.name, { fuss: [Math.floor(geholt.naturalWidth / 2), geholt.naturalHeight], winkel: 0 });
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
  /** Ordnet die Panes: über allen mit niedrigerer `order`, bei Gleichstand nach `id`. */
  const stapeln = (): void => {
    eintraege.forEach((e, i) => {
      const rang = eintraege.length - 1 - i;
      map.getPane(pane(e.id).nadeln)!.style.zIndex = String(PANE_GRUND + rang);
      map.getPane(pane(e.id).formen)!.style.zIndex = String(FORM_GRUND + rang);
    });
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
   * Formen und Schrift einer Ebene, mit einmal geladenen Höhen. Eine eigene
   * Funktion, die keine Closure zurücklässt: So leben die Höhen nur in ihrem
   * Aufruf und fallen nach dem Zeichnen weg. In `ladeEbene` hielten die
   * Closures der Tafeln den gemeinsamen Kontext und damit die Höhen fest.
   */
  const formenUndSchrift = async (
    formen: readonly Form[],
    schriften: readonly Schriftzug[],
    renderer: L.Renderer,
    formPane: string,
    tafelDerEbene: (bausteine: unknown[]) => HTMLElement,
  ): Promise<{ flaechen: L.Layer[]; striche: Strich[]; schriftLagen: Schrift[] }> => {
    // Formen auf dem Boden ohne Laub, die Schrift auf der Oberfläche wie Nadeln und Banner.
    const [boden, oben] = await Promise.all([
      ladeGelaende(regionenFuer(formen, []), true),
      schriften.length ? ladeGelaende(regionenFuer([], schriften), false) : eben,
    ]);
    const { flaechen, striche } = zeichne(formen, { renderer, blick, gelaende: boden, area: umgebung.area, tafel: tafelDerEbene, bediene });
    const schriftLagen: Schrift[] = [];
    for (const s of schriften) schriftLagen.push(new Schrift(s, schriftPfad(s.pfad, oben, blick), faktor, scale, formPane));
    return { flaechen, striche, schriftLagen };
  };

  const ladeEbene = async (e: Eintrag): Promise<void> => {
    if (abgewiesen.get(e.id) === e.version) return;
    const nummer = (auftrag.get(e.id) ?? 0) + 1;
    auftrag.set(e.id, nummer);
    // Gilt, solange niemand umgeschaltet hat und die Ebene noch in der Liste steht und an ist.
    const gilt = () => {
      const jetzt = eintraege.find((x) => x.id === e.id);
      return auftrag.get(e.id) === nummer && jetzt !== undefined && an(jetzt);
    };
    const [mod, name] = e.id.split(':') as [string, string];
    const ordner = `${wurzel}/layers/${mod}`;
    const datei = await json(`${ordner}/${name}.json`, GRENZEN.datei);
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
    const tafelDerEbene = (bausteine: unknown[]) => tafel(bausteine, ordner, e.version);
    // Der Satz dieses Baums, einmal je Laden und nur, wenn ein Banner einen Entwurf nennt.
    let satz: Satz | undefined;
    if (umgebung.satz && orte.some((o) => 'design' in o && o.design)) {
      const satzOrdner = `${ordner}/banner/${name}/${umgebung.satz}`;
      const daten = satzDaten(await json(`${satzOrdner}/satz.json?v=${encodeURIComponent(e.version)}`, 4096));
      if (daten) satz = { ordner: satzOrdner, ...daten };
      else console.warn(`${satzOrdner}/satz.json fehlt oder taugt nicht, Banner mit image`);
    }
    const { renderer, formen: formPane } = pane(e.id);
    // Formen und Schrift rechnen einmal je version, hier vor dem Tausch.
    const { flaechen, striche, schriftLagen } = await formenUndSchrift(formen, schriften, renderer, formPane, tafelDerEbene);
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
          icon,
          pane: pane(e.id).nadeln,
          zIndexOffset: index * 100_000,
          interactive: o.panel !== undefined,
          keyboard: o.panel !== undefined,
          title: o.name ?? '',
          alt: o.name ?? '',
        });
        const panel = o.panel;
        if (panel) {
          const oeffne = tafelAn(m, () => tafelDerEbene(panel), () => m.getElement(), () => m.getLatLng(), (icon.options.iconSize as L.PointTuple)[1]);
          m.on('keypress', (ereignis) => {
            if (ereignis.originalEvent.key === 'Enter') oeffne(true);
          });
        }
        return m;
      }),
    );
    // Hat jemand inzwischen umgeschaltet, gilt sein Auftrag; erst jetzt die alte Gruppe ersetzen.
    if (!gilt()) return;
    weg(e.id);
    // Erst Flächen, dann Ränder und Linien, zuletzt Schrift.
    const formGruppe = L.layerGroup([...flaechen, ...striche.map((s) => s.linie), ...schriftLagen]).addTo(map);
    versetze(striche, faktor());
    const gruppe = L.layerGroup(marker.filter((m): m is L.Marker => m !== undefined)).addTo(map);
    geladen.set(e.id, { version: e.version, gruppe, formen: formGruppe, striche });
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
  /** Baut die Liste nur, wenn sich Ebenen, Namen oder Reihenfolge ändern; so bleibt der Fokus. */
  const zeigeListe = (): void => {
    const stand = JSON.stringify(eintraege.map((e) => [e.id, e.name, e.order]));
    if (stand !== gezeigteListe) {
      gezeigteListe = stand;
      inhalt.replaceChildren();
      for (const e of eintraege) {
        const zeile = L.DomUtil.create('label', '', inhalt);
        const box = L.DomUtil.create('input', '', zeile);
        box.type = 'checkbox';
        box.dataset.id = e.id;
        zeile.append(` ${e.name[sprache] ?? e.name.de ?? e.name.en}`);
        box.addEventListener('change', () => {
          wahl.setze(e.id, box.checked);
          if (box.checked) void ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler));
          else entferne(e.id);
        });
      }
    }
    for (const box of inhalt.querySelectorAll<HTMLInputElement>('input')) {
      const e = eintraege.find((x) => x.id === box.dataset.id);
      if (e) box.checked = an(e);
    }
  };

  /** Liest die Liste; nimmt weg, was nicht mehr an ist, und lädt neu, was an ist und sich geändert hat. */
  const abgleichen = async (neu: unknown[]): Promise<void> => {
    if (neu.length > GRENZEN.ebenen) console.warn(`${wurzel}/layers.json: ${neu.length} Ebenen, gezeigt die ersten ${GRENZEN.ebenen}`);
    eintraege = neu
      .slice(0, GRENZEN.ebenen)
      .map(eintrag)
      .filter((e): e is Eintrag => e !== undefined)
      .sort((a, b) => b.order - a.order || a.id.localeCompare(b.id));
    if (eintraege.length > 0 && !control.getContainer()) control.addTo(map);
    zeigeListe();
    stapeln();
    // Auch, wenn der Betreiber `visible` ändert und der Betrachter nie gewählt hat.
    for (const id of [...geladen.keys()]) if (!eintraege.some((e) => e.id === id && an(e))) entferne(id);
    const zuLaden = eintraege.filter((e) => an(e) && geladen.get(e.id)?.version !== e.version);
    await Promise.all(zuLaden.map((e) => ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler))));
  };

  /** Die Ebenen aus `layers.json`, oder `undefined`, wenn sie fehlt oder kaputt ist. */
  const holeListe = async (): Promise<unknown[] | undefined> => {
    const liste = await json(`${wurzel}/layers.json`, GRENZEN.liste).catch(() => undefined);
    return istObjekt(liste) && Array.isArray(liste.layers) ? liste.layers : undefined;
  };
  const nachfragen = async (): Promise<void> => {
    const liste = await holeListe();
    if (liste) await abgleichen(liste);
  };

  // Ohne layers.json beim Laden fragt die Karte bis zum Neuladen nicht nach.
  const erste = await holeListe();
  if (!erste) return;
  if (iso && !karten) console.warn(`${wurzel}: Ebenen ohne Höhen, im iso auf seaLevel`);
  // Erst anmelden, dann abgleichen: Ein Fehler beim ersten Abgleich hält das
  // Nachfragen nicht auf.
  map.on('zoomend', () => {
    for (const g of geladen.values()) versetze(g.striche, faktor());
  });
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
        for (const e of eintraege) if (geladen.has(e.id)) void ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler));
      },
      { once: true },
    );
  };
  beiDpr();
  await abgleichen(erste);
}
