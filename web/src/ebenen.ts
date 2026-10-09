/**
 * Ebenen über den Kacheln: die Liste zum Umschalten, Nadeln als Wappenschild
 * und ihre Infotafel. Das Format steht in docs/benutzung/ebenen.md, wie die
 * Karte es zeigt in docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import feldGross from './ebenen/schild_gross.png?url&no-inline';
import rahmenGross from './ebenen/schild_gross_rahmen.png?url&no-inline';
import feldKlein from './ebenen/schild_klein.png?url&no-inline';
import rahmenKlein from './ebenen/schild_klein_rahmen.png?url&no-inline';
import feldMittel from './ebenen/schild_mittel.png?url&no-inline';
import rahmenMittel from './ebenen/schild_mittel_rahmen.png?url&no-inline';
import { FRISCH, type Hoehenkarten } from './hoehen';
import { projiziere, type Projektion } from './pick';

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
  blick: { p: Projektion; k: number };
  scale: number;
  maxZoom: number;
  /** Die Höhen; ohne sie liegt im iso alles auf `seaLevel`. */
  karten?: Hoehenkarten;
  heightsCell?: number;
  seaLevel?: number;
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
const GRENZEN = { liste: 64 * 1024, datei: 4 * 1024 * 1024, ebenen: 64, nadeln: 1000, bausteine: 64, bild: 512, punkte: 20 };

/** Die Panes der Ebenen liegen über den Kacheln und unter dem Popup (700). */
const PANE_GRUND = 610;

const KENNUNG = /^([a-z0-9_-][a-z0-9_.-]{0,63}):([a-z0-9_-][a-z0-9_.-]{0,63})$/;
const FARBE = /^#[0-9a-f]{6}([0-9a-f]{2})?$/i;
/** Ein Bild der Ebene: nur unter `images/`, ohne `..` und ohne Teil mit `.` vorn. */
const BILD = /^images\/[a-z0-9_-][a-z0-9_.-]*$/i;

const istText = (wert: unknown, max: number): wert is string => typeof wert === 'string' && wert.length > 0 && wert.length <= max;
const istZahl = (wert: unknown): wert is number => typeof wert === 'number' && Number.isFinite(wert);
const istObjekt = (wert: unknown): wert is Record<string, unknown> => typeof wert === 'object' && wert !== null && !Array.isArray(wert);

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

/** Ein Punkt der Welt im Blick, k Vierteldrehungen; ohne −1 wie bei Blöcken. */
export function punktImBlick(x: number, z: number, k: number): [number, number] {
  let [a, b] = [x, z];
  for (let i = 0; i < k; i++) [a, b] = [b, -a];
  return [a, b];
}

function eintrag(wert: unknown): Eintrag | undefined {
  if (!istObjekt(wert) || typeof wert.id !== 'string' || !KENNUNG.test(wert.id) || !istText(wert.version, 256)) return undefined;
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
  const at = wert.at;
  if (!Array.isArray(at) || at.length !== 2 || !at.every(istZahl)) return undefined;
  const symbol = istObjekt(wert.symbol) ? wert.symbol : {};
  const bild = (pfad: unknown) => (typeof pfad === 'string' && BILD.test(pfad) ? pfad : undefined);
  return {
    id: wert.id,
    at: [at[0] as number, at[1] as number],
    y: Number.isInteger(wert.y) ? (wert.y as number) : undefined,
    name: istText(wert.name, 64) ? wert.name : undefined,
    size: GROESSEN.includes(wert.size as Groesse) ? (wert.size as Groesse) : 'medium',
    symbol: { large: bild(symbol.large), medium: bild(symbol.medium) },
    color: typeof wert.color === 'string' && FARBE.test(wert.color) ? wert.color : '#D9443A',
    panel: istObjekt(wert.panel) && Array.isArray(wert.panel.blocks) ? wert.panel.blocks : undefined,
  };
}

/** Ein Bild vom eigenen Server, einmal je Adresse; ein Fehlschlag bleibt nicht im Cache. */
const bilder = new Map<string, Promise<HTMLImageElement>>();
function bild(adresse: string): Promise<HTMLImageElement> {
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
 * Symbol mittig 3 Pixel unter der Oberkante, zuletzt Rahmen und Nadel. Ein
 * Symbol in falscher Grösse bleibt weg. Siehe docs/benutzung/ebenen.md,
 * „Nadel“.
 */
async function zeichneNadel(groesse: Groesse, farbe: string, symbol: string | undefined): Promise<HTMLCanvasElement> {
  const { feld, rahmen, b, h, symbol: seite } = SCHILDE[groesse];
  const leinwand = document.createElement('canvas');
  [leinwand.width, leinwand.height] = [b, h];
  const ctx = leinwand.getContext('2d')!;
  const [f, r, s] = await Promise.all([bild(feld), bild(rahmen), symbol ? bild(symbol).catch(() => undefined) : undefined]);
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
  const farbe = (wert: unknown) => (typeof wert === 'string' && FARBE.test(wert) ? wert : undefined);
  const groesse = (wert: unknown) => Number.isInteger(wert) && (wert as number) >= 1 && (wert as number) <= GRENZEN.bild;
  const bildElement = (b: Record<string, unknown>) => {
    if (typeof b.image !== 'string' || !BILD.test(b.image)) return undefined;
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
  const text = await antwort.text();
  if (new TextEncoder().encode(text).length > max) {
    console.warn(`${pfad}: grösser als ${max} Byte, übergangen`);
    return undefined;
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    console.warn(`${pfad}: keine gültige JSON-Datei`);
    return undefined;
  }
}

/**
 * Die Ebenen der Karte: liest `layers.json` neben `trees.json`, zeigt die
 * Liste zum Umschalten und die Nadeln der Ebenen, die an sind, und fragt
 * alle 30 Sekunden nach Änderungen. Ohne `layers.json` geschieht nichts.
 */
export async function ebenen(umgebung: Umgebung): Promise<void> {
  const { map, wurzel, blick, scale, maxZoom, karten, heightsCell, seaLevel } = umgebung;
  const sprache = navigator.language.startsWith('de') ? 'de' : 'en';
  const wahl = gemerkt(wurzel);
  const iso = blick.p.y > 0;

  /** Die Oberseite des Geländes je Punkt, einmal gerechnet; siehe ebenen.md, „Die Oberfläche im iso“. */
  const oberflaechen = new Map<string, Promise<number>>();
  const oberflaeche = (x: number, z: number): Promise<number> => {
    const schluessel = `${x},${z}`;
    let wert = oberflaechen.get(schluessel);
    if (!wert) {
      wert = (async () => {
        const grund = seaLevel ?? 64;
        if (!iso || !karten || !heightsCell) return grund + 1;
        const c = heightsCell;
        // Vier Zellen um den Punkt und für den Ersatz zwei weitere in jede Richtung.
        await karten.lade([-3 * c, 3 * c].flatMap((dx) => [-3 * c, 3 * c].map((dz) => [Math.floor(x + dx), 0, Math.floor(z + dz)] as [number, number, number])));
        const zelle = (i: number, j: number): number => {
          const eigen = karten.hoehe(i * c, j * c);
          if (eigen !== undefined) return eigen;
          const nachbarn: number[] = [];
          for (let di = -2; di <= 2; di++) {
            for (let dj = -2; dj <= 2; dj++) {
              const n = karten.hoehe((i + di) * c, (j + dj) * c);
              if (n !== undefined) nachbarn.push(n);
            }
          }
          return nachbarn.length ? nachbarn.reduce((a, b) => a + b, 0) / nachbarn.length : grund;
        };
        const [fx, fz] = [x / c - 0.5, z / c - 0.5];
        const [i, j] = [Math.floor(fx), Math.floor(fz)];
        const [tx, tz] = [fx - i, fz - j];
        const oben = zelle(i, j) * (1 - tx) + zelle(i + 1, j) * tx;
        const unten = zelle(i, j + 1) * (1 - tx) + zelle(i + 1, j + 1) * tx;
        return oben * (1 - tz) + unten * tz + 1;
      })();
      oberflaechen.set(schluessel, wert);
    }
    return wert;
  };

  /** Breite eines Blocks auf dem Schirm, in Pixeln. */
  const blockPixel = () => scale * 2 ** (map.getZoom() - maxZoom);

  interface Geladen {
    ordner: string;
    version: string;
    gruppe: L.LayerGroup;
    marker: { nadel: Nadel; marker: L.Marker; groesse?: Groesse }[];
  }
  const geladen = new Map<string, Geladen>();
  /** Je Ebene ein Zähler: Ein Laden, das ein späteres Umschalten überholt, verwirft sich selbst. */
  const auftrag = new Map<string, number>();
  let eintraege: Eintrag[] = [];

  const icons = new Map<string, Promise<HTMLCanvasElement>>();
  const icon = async (n: Nadel, groesse: Groesse, ordner: string, v: string): Promise<L.DivIcon> => {
    const pfad = groesse === 'small' ? undefined : n.symbol[groesse];
    const symbol = pfad && `${ordner}/${pfad}?v=${encodeURIComponent(v)}`;
    const schluessel = `${groesse} ${n.color} ${symbol ?? ''}`;
    let leinwand = icons.get(schluessel);
    if (!leinwand) icons.set(schluessel, (leinwand = zeichneNadel(groesse, n.color, symbol)));
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
      [...geladen.values()].flatMap(({ ordner, version, gruppe, marker }) =>
        marker.map(async (eintrag) => {
          const groesse = gezeigt(eintrag.nadel.size, p);
          const fertig = groesse && groesse !== eintrag.groesse ? await icon(eintrag.nadel, groesse, ordner, version) : undefined;
          return { gruppe, eintrag, groesse, fertig };
        }),
      ),
    );
    if (blockPixel() !== p) return;
    for (const { gruppe, eintrag, groesse, fertig } of neu) {
      // Eine Ebene, die inzwischen aus oder ersetzt ist, bleibt, wie sie ist.
      if (![...geladen.values()].some((g) => g.gruppe === gruppe)) continue;
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

  /** Das Pane einer Ebene: über allen mit niedrigerer `order`, bei Gleichstand nach `id`. */
  const pane = (id: string): string => {
    const name = `ebene-${id.replace(/[^a-z0-9_-]/g, '_')}`;
    if (!map.getPane(name)) map.createPane(name);
    return name;
  };
  const stapeln = (): void => {
    eintraege.forEach((e, i) => {
      map.getPane(pane(e.id))!.style.zIndex = String(PANE_GRUND + eintraege.length - 1 - i);
    });
  };

  const an = (e: Eintrag): boolean => wahl.lies()[e.id] ?? e.visible;

  const ladeEbene = async (e: Eintrag): Promise<void> => {
    const nummer = (auftrag.get(e.id) ?? 0) + 1;
    auftrag.set(e.id, nummer);
    const gilt = () => auftrag.get(e.id) === nummer && an(e);
    const [mod, name] = e.id.split(':') as [string, string];
    const ordner = `${wurzel}/layers/${mod}`;
    const datei = await json(`${ordner}/${name}.json`, GRENZEN.datei);
    if (istObjekt(datei) && (datei.permission !== undefined || datei.web === false)) {
      console.warn(`${ordner}/${name}.json: Ebene mit permission oder web: false, übergangen`);
      return;
    }
    const objekte = istObjekt(datei) && Array.isArray(datei.objects) ? datei.objects : [];
    let nadeln = objekte.map(nadel).filter((n): n is Nadel => n !== undefined);
    if (nadeln.length > GRENZEN.nadeln) {
      console.warn(`${e.id}: ${nadeln.length} Nadeln, gezeigt die ersten ${GRENZEN.nadeln}`);
      nadeln = nadeln.slice(0, GRENZEN.nadeln);
    }
    const marker = await Promise.all(
      nadeln.map(async (n, index) => {
        const [vx, vz] = punktImBlick(n.at[0], n.at[1], blick.k);
        const y = n.y !== undefined ? n.y + 1 : await oberflaeche(n.at[0], n.at[1]);
        const [px, py] = projiziere(vx, iso ? y : 0, vz, blick.p);
        // In der Ebene liegt die spätere oben, gleich wo auf dem Schirm.
        const m = L.marker(L.latLng(py, px), {
          icon: L.divIcon({ html: '' }),
          pane: pane(e.id),
          zIndexOffset: index * 100_000,
          interactive: n.panel !== undefined,
          keyboard: n.panel !== undefined,
          title: n.name ?? '',
          alt: n.name ?? '',
        });
        if (n.panel) {
          m.bindPopup(() => tafel(n.panel!, ordner, e.version), { className: 'tafel', maxWidth: 320, minWidth: 120, autoPanPadding: [8, 8] });
        }
        return { nadel: n, marker: m };
      }),
    );
    // Hat jemand inzwischen umgeschaltet, gilt sein Auftrag; erst jetzt die alte Gruppe ersetzen.
    if (!gilt()) return;
    geladen.get(e.id)?.gruppe.remove();
    geladen.set(e.id, { ordner, version: e.version, gruppe: L.layerGroup().addTo(map), marker });
    // Bilder und Icons einer alten version dieser Ebene fallen weg.
    for (const schluessel of [...icons.keys()]) if (schluessel.includes(`${ordner}/`) && !schluessel.includes(`v=${encodeURIComponent(e.version)}`)) icons.delete(schluessel);
    await groessen();
  };

  const entferne = (id: string): void => {
    auftrag.set(id, (auftrag.get(id) ?? 0) + 1);
    geladen.get(id)?.gruppe.remove();
    geladen.delete(id);
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

  /** Liest die Liste; lädt neu, was an ist und sich geändert hat. */
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
    for (const id of [...geladen.keys()]) if (!eintraege.some((e) => e.id === id)) entferne(id);
    const zuLaden = eintraege.filter((e) => an(e) && geladen.get(e.id)?.version !== e.version);
    await Promise.all(zuLaden.map((e) => ladeEbene(e).catch((fehler: unknown) => console.error(e.id, fehler))));
  };

  const nachfragen = async (): Promise<void> => {
    const liste = await json(`${wurzel}/layers.json`, GRENZEN.liste).catch(() => undefined);
    if (istObjekt(liste) && Array.isArray(liste.layers)) await abgleichen(liste.layers);
  };

  // Erst anmelden, dann laden: Ein Fehler beim ersten Abgleich hält das
  // Nachfragen nicht auf.
  map.on('zoomend', () => void groessen());
  setInterval(() => {
    if (document.visibilityState === 'visible') void nachfragen();
  }, TAKT);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') void nachfragen();
  });
  // Die Tafel per Tastatur: Fokus hinein beim Öffnen, Escape schliesst und
  // gibt ihn der Nadel zurück.
  map.on('popupopen', ({ popup }: L.PopupEvent) => {
    const element = popup.getElement()?.querySelector<HTMLElement>('.leaflet-popup-content');
    if (!element) return;
    element.tabIndex = -1;
    element.focus();
    element.addEventListener('keydown', (ereignis) => {
      if (ereignis.key !== 'Escape') return;
      const quelle = (popup as unknown as { _source?: L.Marker })._source;
      map.closePopup(popup);
      quelle?.getElement()?.focus();
    });
  });
  if (iso && !karten) console.warn(`${wurzel}: Ebenen ohne Höhen, im iso auf seaLevel`);
  await nachfragen();
}
