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
import { farbe, istObjekt, istText, punkt } from './pruefen';
import { Schrift, schriftzug, type Schriftzug } from './schrift';

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
  heightsCell?: number;
  seaLevel?: number;
  /** Die Bauhöhe aus `map.json`; sie begrenzt, wie weit Gelände eine Form verdecken kann. */
  minY?: number;
  maxY?: number;
  /** `area` aus `map.json`; Höhen gibt es nur darin. */
  area?: Rechteck;
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
const GRENZEN = { liste: 64 * 1024, datei: 4 * 1024 * 1024, ebenen: 64, objekte: 10_000, nadeln: 1000, bausteine: 64, bild: 512, punkte: 20, regionen: 1024 };

/**
 * Die Panes der Nadeln, 510 bis 573: über `shadowPane` (500), unter
 * `markerPane` (600), `tooltipPane` und Tafel. Darunter die Panes der Formen
 * und Schrift, 410 bis 473: So liegen die Nadeln aller Ebenen über allem
 * anderen.
 */
const PANE_GRUND = 510;
const FORM_GRUND = 410;

/** Die Tafel als Popup, für Nadeln und Flächen gleich. */
const TAFEL: L.PopupOptions = { className: 'tafel', maxWidth: 320, minWidth: 120, autoPanPadding: [8, 8] };

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

/**
 * Wie viele Grössen kleiner die Nadel beim Hinauszoomen wird, nach der
 * Breite eines Blocks auf dem Schirm in Pixeln. Siehe
 * docs/benutzung/ebenen.md, „Nadeln“.
 */
export function kleiner(blockPixel: number): number {
  if (blockPixel >= 1 / 2) return 0;
  if (blockPixel >= 1 / 8) return 1;
  if (blockPixel >= 1 / 32) return 2;
  return 3;
}

/** Die Grösse, in der eine Nadel steht, oder `undefined`, wenn sie aus ist. */
export function gezeigt(grund: Groesse, blockPixel: number): Groesse | undefined {
  return GROESSEN[GROESSEN.indexOf(grund) + kleiner(blockPixel)];
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
            if (f) titel.style.color = f;
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
      for (let rx = Math.floor(ax / REGION); rx <= Math.floor(bx / REGION); rx++) {
        for (let rz = Math.floor(az / REGION); rz <= Math.floor(bz / REGION); rz++) menge.add(schluessel(rx, rz));
      }
    };
    for (const f of formen) {
      if (hatFlaeche(f)) nimm(bereich(f));
      if (hatFlaeche(f) || f.rand.breite === 0) continue;
      // Entlang des Zugs alle 128 Blöcke ein Punkt; eine Region ist 512 breit.
      for (const stueck of zuege(f, c, umgebung.area)) {
        for (let i = 1; i < stueck.length; i++) {
          const [a, b] = [stueck[i - 1]!, stueck[i]!];
          const n = Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / 128);
          for (let t = 0; t <= n; t++) {
            const [x, z] = [a[0] + ((b[0] - a[0]) * t) / n, a[1] + ((b[1] - a[1]) * t) / n];
            nimm([x, z, x, z]);
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
   * Die Höhen dieser Regionen, einmal geladen für alle Formen und Schriften
   * einer Ebene. Die Karten hält das Ergebnis selbst, so verdrängt der Cache
   * keine, solange es lebt.
   */
  const ladeGelaende = async (menge: ReadonlySet<number>): Promise<Gelaende> => {
    if (!karten || menge.size === 0) return eben;
    // Gegen eine Ebene ohne area mit riesigem Kreis: je Region 32 KiB.
    if (menge.size > GRENZEN.regionen) {
      console.warn(`${wurzel}: Ebene bräuchte ${menge.size} Regionen Höhen, mehr als ${GRENZEN.regionen}; sie liegt auf seaLevel`);
      return eben;
    }
    const regionen = new Map<number, Int16Array>();
    let max = grund + 1;
    await Promise.all(
      [...menge].map(async (s) => {
        const karte = await karten.karte(Math.floor(s / 131072) - 65536, (s % 131072) - 65536);
        if (!karte) return;
        regionen.set(s, karte);
        for (const v of karte) if (v !== LEER && v + 1 > max) max = v + 1;
      }),
    );
    const n = REGION / c;
    // Die zuletzt gefragte Region: Nachbarn liegen meist in derselben.
    let [letzte, karte]: [number, Int16Array | undefined] = [Number.NaN, undefined];
    return {
      c,
      grund,
      max,
      zelle: (i, j) => {
        const [rx, rz] = [Math.floor(i / n), Math.floor(j / n)];
        const s = schluessel(rx, rz);
        if (s !== letzte) [letzte, karte] = [s, regionen.get(s)];
        const v = karte?.[(j - rz * n) * n + (i - rx * n)];
        return v === undefined || v === LEER ? undefined : v;
      },
    };
  };

  /** Breite eines Blocks auf dem Schirm, in Pixeln. */
  const blockPixel = () => scale * 2 ** (map.getZoom() - maxZoom);

  interface Geladen {
    ordner: string;
    version: string;
    gruppe: L.LayerGroup;
    marker: { nadel: Nadel; marker: L.Marker; groesse?: Groesse }[];
    /** Flächen, Ränder, Linien und Schrift, in dieser Reihenfolge. */
    formen: L.LayerGroup;
    striche: Strich[];
    /** Icons und Symbole dieser `version`; sie fallen mit ihr weg. */
    icons: Map<string, Promise<HTMLCanvasElement>>;
    symbole: Map<string, Promise<HTMLImageElement>>;
  }
  const geladen = new Map<string, Geladen>();
  /** Je Ebene ein Zähler: Ein Laden, das ein späteres Umschalten überholt, verwirft sich selbst. */
  const auftrag = new Map<string, number>();
  /** Je Ebene die `version` mit `permission` oder `web: false`; die holt die Karte nicht noch einmal. */
  const abgewiesen = new Map<string, string>();
  let eintraege: Eintrag[] = [];
  /** Was gerade seine Tafel offen hat, und sein Element, das den Fokus zurückbekommt. */
  let offen: { quelle: L.Layer; element: () => Element | undefined } | undefined;

  /**
   * Die Tafel einer Nadel oder Fläche per Tastatur: Fokus hinein nur, wenn
   * die Tastatur sie geöffnet hat; Escape gibt ihn zurück, siehe unten.
   */
  const bedienbar = (quelle: L.Layer, element: () => Element | undefined, perTastatur: () => boolean): void => {
    quelle.on('popupopen', ({ popup }: L.PopupEvent) => {
      offen = { quelle, element };
      const inhalt = popup.getElement()?.querySelector<HTMLElement>('.leaflet-popup-content');
      if (!inhalt) return;
      inhalt.tabIndex = -1;
      // Ohne preventScroll dürfte der Browser den Container scrollen, um die Tafel zu zeigen.
      if (perTastatur()) inhalt.focus({ preventScroll: true });
    });
    quelle.on('popupclose', () => {
      if (offen?.quelle === quelle) offen = undefined;
    });
  };

  /** Eine Fläche mit Tafel wird ein Ziel für die Tastatur: Tab erreicht sie, Enter oder Leertaste öffnet die Tafel. */
  const bediene = (flaeche: L.Polygon, name: string | undefined): void => {
    let perTastatur = false;
    flaeche.on('click', () => {
      perTastatur = false;
    });
    flaeche.on('add', () => {
      const element = flaeche.getElement();
      if (!element) return;
      element.setAttribute('tabindex', '0');
      element.setAttribute('role', 'button');
      element.setAttribute('aria-label', name ?? 'Tafel');
      element.addEventListener('keydown', (ereignis) => {
        if ((ereignis as KeyboardEvent).key !== 'Enter' && (ereignis as KeyboardEvent).key !== ' ') return;
        ereignis.preventDefault();
        perTastatur = true;
        flaeche.openPopup(flaeche.getBounds().getCenter());
      });
    });
    bedienbar(flaeche, () => flaeche.getElement(), () => perTastatur);
  };

  const icon = async (n: Nadel, groesse: Groesse, g: Geladen): Promise<L.DivIcon> => {
    const pfad = groesse === 'small' ? undefined : n.symbol[groesse];
    const symbol = pfad && `${g.ordner}/${pfad}?v=${encodeURIComponent(g.version)}`;
    const schluessel = `${groesse} ${n.color} ${symbol ?? ''}`;
    let leinwand = g.icons.get(schluessel);
    if (!leinwand) g.icons.set(schluessel, (leinwand = zeichneNadel(groesse, n.color, symbol, g.symbole)));
    const { b, h } = SCHILDE[groesse];
    const html = L.DomUtil.create('div', 'nadel');
    const kopie = document.createElement('canvas');
    [kopie.width, kopie.height] = [b, h];
    kopie.getContext('2d')!.drawImage(await leinwand, 0, 0);
    html.append(kopie);
    if (n.name && groesse === n.size) L.DomUtil.create('span', 'nadel-name', html).textContent = n.name;
    return L.divIcon({ html, className: 'nadel-icon', iconSize: [b, h], iconAnchor: [Math.floor(b / 2), h] });
  };

  /**
   * Setzt jede Nadel in die Grösse der Stufe, oder nimmt sie weg. Erst alle
   * Icons, dann setzen; hat sich die Stufe inzwischen geändert, setzt der
   * spätere Aufruf.
   */
  const groessen = async (): Promise<void> => {
    const p = blockPixel();
    const neu = await Promise.all(
      [...geladen.values()].flatMap((g) =>
        g.marker.map(async (eintrag) => {
          const groesse = gezeigt(eintrag.nadel.size, p);
          const fertig = groesse && groesse !== eintrag.groesse ? await icon(eintrag.nadel, groesse, g) : undefined;
          return { gruppe: g.gruppe, eintrag, groesse, fertig };
        }),
      ),
    );
    if (blockPixel() !== p) return;
    // Eine Ebene, die inzwischen aus oder ersetzt ist, bleibt, wie sie ist.
    const lebend = new Set([...geladen.values()].map((g) => g.gruppe));
    for (const { gruppe, eintrag, groesse, fertig } of neu) {
      if (!lebend.has(gruppe)) continue;
      if (!groesse) {
        gruppe.removeLayer(eintrag.marker);
        continue;
      }
      if (fertig) {
        eintrag.marker.setIcon(fertig);
        eintrag.groesse = groesse;
      }
      gruppe.addLayer(eintrag.marker);
    }
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
    let nadeln = objekte.map(nadel).filter((n): n is Nadel => n !== undefined);
    if (nadeln.length > GRENZEN.nadeln) {
      console.warn(`${e.id}: ${nadeln.length} Nadeln, gezeigt die ersten ${GRENZEN.nadeln}`);
      nadeln = nadeln.slice(0, GRENZEN.nadeln);
    }
    const tafelDerEbene = (bausteine: unknown[]) => tafel(bausteine, ordner, e.version);
    const { renderer, formen: formPane } = pane(e.id);
    // Formen und Schrift rechnen einmal je version, hier vor dem Tausch, mit
    // einmal geladenen Höhen; danach fallen sie weg.
    const gelaende = await ladeGelaende(regionenFuer(formen, schriften));
    const gezeichnet = zeichne(formen, { renderer, blick, gelaende, area: umgebung.area, tafel: tafelDerEbene, tafelOptionen: TAFEL, bediene });
    const faktor = () => 2 ** (map.getZoom() - maxZoom);
    const schriftLagen = schriften.map((s) => new Schrift(s, schriftPfad(s.pfad, gelaende, blick), faktor, scale, formPane));
    const marker = await Promise.all(
      nadeln.map(async (n, index) => {
        const y = n.y !== undefined ? n.y + 1 : await oberflaeche(n.at[0], n.at[1]);
        const [px, py] = bildpunkt(n.at[0], iso ? y : 0, n.at[1], blick);
        // In der Ebene liegt die spätere oben, gleich wo auf dem Schirm.
        const m = L.marker(L.latLng(py, px), {
          icon: L.divIcon({ html: '' }),
          pane: pane(e.id).nadeln,
          zIndexOffset: index * 100_000,
          interactive: n.panel !== undefined,
          keyboard: n.panel !== undefined,
          title: n.name ?? '',
          alt: n.name ?? '',
        });
        if (n.panel) {
          // Vor bindPopup angemeldet, damit es vor dem Öffnen läuft: Fokus in die Tafel nur nach Enter.
          let perTastatur = false;
          m.on('keypress', (ereignis) => {
            perTastatur = ereignis.originalEvent.key === 'Enter';
          });
          m.on('click', () => {
            perTastatur = false;
          });
          m.bindPopup(() => tafelDerEbene(n.panel!), TAFEL);
          bedienbar(m, () => m.getElement(), () => perTastatur);
        }
        return { nadel: n, marker: m };
      }),
    );
    // Hat jemand inzwischen umgeschaltet, gilt sein Auftrag; erst jetzt die alte Gruppe ersetzen.
    if (!gilt()) return;
    weg(e.id);
    // Erst Flächen, dann Ränder und Linien, zuletzt Schrift.
    const formGruppe = L.layerGroup([...gezeichnet.flaechen, ...gezeichnet.striche.map((s) => s.linie), ...schriftLagen]).addTo(map);
    versetze(gezeichnet.striche, faktor());
    geladen.set(e.id, {
      ordner,
      version: e.version,
      gruppe: L.layerGroup().addTo(map),
      marker,
      formen: formGruppe,
      striche: gezeichnet.striche,
      icons: new Map(),
      symbole: new Map(),
    });
    await groessen();
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
    for (const g of geladen.values()) versetze(g.striche, 2 ** (map.getZoom() - maxZoom));
    void groessen();
  });
  setInterval(() => {
    if (document.visibilityState === 'visible') void nachfragen();
  }, TAKT);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void nachfragen();
  });
  // Escape in der Tafel, auch auf ihrem Schliessknopf, schliesst sie und gibt
  // den Fokus der Nadel oder Fläche zurück.
  map.getContainer().addEventListener('keydown', (ereignis) => {
    const jetzt = offen;
    if (ereignis.key !== 'Escape' || !jetzt?.quelle.getPopup()?.getElement()?.contains(ereignis.target as Node)) return;
    jetzt.quelle.closePopup();
    (jetzt.element() as HTMLElement | SVGElement | undefined)?.focus({ preventScroll: true });
  });
  await abgleichen(erste);
}
