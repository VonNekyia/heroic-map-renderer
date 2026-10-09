import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import {
  inDenBlick,
  inDieWelt,
  pick,
  projiziere,
  REGION,
  RICHTUNGEN,
  strahl,
  umriss,
  zweiZuEins,
  type Block,
  type Projektion,
} from './pick';
import { ebenen } from './ebenen';
import { FRISCH, hoehen, type Hoehenkarten } from './hoehen';
import { VERSION, type Grenzen, type Rechteck } from './skin-api';
import './style.css';

/** Was `map.json` aus dem Renderer mitbringt. */
interface MapInfo {
  tileSize: number;
  scale: number;
  minZoom: number;
  maxZoom: number;
  tiles: string;
  bounds: [number, number, number, number];
  /** Pfadmuster der Höhenkarten je Region; ohne sie keine Koordinaten. */
  heights?: string;
  /** Spalten je Kante einer Zelle der Höhenkarten. */
  heightsCell?: number;
  /** Der Bereich in Y, in dem jeder gezeichnete Block liegt. */
  minY?: number;
  maxY?: number;
  /** Die Kamera des Baums, gekürzt, etwa `8:5` oder `top`; ohne sie 2:1. */
  camera?: string;
  /** Wo die Kamera steht; ohne Angabe `se`, genordet `s`. */
  direction?: string;
  /** Die Zahlen der Projektion; ohne sie rechnet das Frontend 2:1 aus `scale`. */
  projection?: Projektion;
  /** Höhe der Wasseroberfläche in Blöcken; für einen Skin. */
  seaLevel?: number;
  /** Das Rechteck der Welt in Blöcken; für einen Skin. */
  area?: Rechteck;
}

/**
 * Wie viele Stufen über die feinste gerenderte hinaus gezoomt werden darf.
 *
 * Darüber vergrössert Leaflet nur noch die vorhandenen Kacheln. Bei
 * Pixelkunst ist das kein Verlust, solange der Browser nicht glättet —
 * dafür sorgt `image-rendering: pixelated`.
 */
const EXTRA_ZOOM = 2;

/**
 * Ein Bildpunkt der feinsten Stufe als Leaflet-Koordinate.
 *
 * `L.LatLng` ist `(lat, lng)`, und `CRS.Simple` bildet `lat` auf y ab —
 * die Reihenfolge ist also vertauscht.
 */
function point(x: number, y: number): L.LatLng {
  return L.latLng(y, x);
}

/**
 * Das Koordinatensystem der Karte.
 *
 * Eine Karteneinheit ist ein Pixel der feinsten Stufe. Leaflets
 * `CRS.Simple` rechnet mit `2^zoom`, hier muss stattdessen die feinste
 * Stufe den Faktor 1 bekommen.
 * Siehe docs/frontend.md, „Das Koordinatensystem“.
 */
/**
 * Leaflets Typen führen `transformation` nicht auf, obwohl jede
 * mitgelieferte CRS sie setzt und `latLngToPoint` sie benutzt.
 */
type Crs = L.CRS & { transformation: L.Transformation };

function crs(info: MapInfo): Crs {
  const simple = L.CRS.Simple as Crs;
  return {
    ...simple,
    // Nicht spiegeln: `screen_y` des Renderers zeigt schon nach unten.
    transformation: new L.Transformation(1, 0, 1, 0),
    scale: (zoom: number) => 2 ** (zoom - info.maxZoom),
    zoom: (scale: number) => Math.log2(scale) + info.maxZoom,
  };
}

/**
 * `map.json` und ihr Stand: `Last-Modified`, die Zeit des letzten Laufs, der
 * sie geschrieben hat. Ohne brauchbaren Header kein Stand.
 */
async function load(base: string): Promise<{ info: MapInfo; stand: Date | undefined }> {
  const path = `${base}/map.json`;
  const response = await fetch(path, FRISCH);
  if (!response.ok) {
    throw new Error(`${path}: ${response.status} ${response.statusText}`);
  }

  // Ein Server, der auf unbekannte Pfade die index.html ausliefert, kommt
  // bis hierher. Die Meldung muss trotzdem den Dateinamen nennen, sonst
  // steht da nur "Unexpected token '<'".
  let info: unknown;
  try {
    info = await response.json();
  } catch {
    throw new Error(`${path}: keine gültige JSON-Datei`);
  }
  if (!isMapInfo(info)) {
    throw new Error(`${path}: fehlende oder unbrauchbare Felder`);
  }
  const stand = new Date(response.headers.get('last-modified') ?? Number.NaN);
  return { info, stand: Number.isNaN(stand.getTime()) ? undefined : stand };
}

/**
 * Unten rechts der Stand der Karte, etwa „Stand: 02.10.2026, 21:40“. Siehe
 * docs/frontend.md, „Stand der Karte“.
 */
function standAnzeigen(map: L.Map, stand: Date): void {
  const element = L.DomUtil.create('div', 'stand');
  const zeit = new Intl.DateTimeFormat('de-DE', { dateStyle: 'medium', timeStyle: 'short' });
  element.textContent = `Stand: ${zeit.format(stand)}`;
  untenRechts(map, element);
}

/** Der Pflichthinweis von Mojang, wörtlich. Siehe NOTICE. */
const MOJANG = 'NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.';

/**
 * Unten rechts der Hinweis von Mojang und der Link auf `lizenzen.txt` aus
 * dem Build: NOTICE, LICENSE und die Lizenzen der gebündelten
 * Abhängigkeiten. Siehe docs/frontend.md, „Lizenzen“.
 */
function lizenzenAnzeigen(map: L.Map): void {
  const fuss = L.DomUtil.create('div', 'lizenzen');
  L.DomUtil.create('span', '', fuss).textContent = `${MOJANG} · `;
  const link = L.DomUtil.create('a', '', fuss);
  link.href = 'lizenzen.txt';
  link.textContent = 'Lizenzen';
  untenRechts(map, fuss);
}

/** Ein Element als Control unten rechts, über den schon vorhandenen. */
function untenRechts(map: L.Map, element: HTMLElement): void {
  const control = new L.Control({ position: 'bottomright' });
  control.onAdd = () => element;
  control.addTo(map);
}

function isMapInfo(value: unknown): value is MapInfo {
  if (typeof value !== 'object' || value === null) return false;
  const info = value as Record<string, unknown>;
  return (
    typeof info.tileSize === 'number' &&
    typeof info.scale === 'number' &&
    typeof info.minZoom === 'number' &&
    typeof info.maxZoom === 'number' &&
    typeof info.tiles === 'string' &&
    Array.isArray(info.bounds) &&
    info.bounds.length === 4 &&
    info.bounds.every((n) => typeof n === 'number')
  );
}

/** Die Felder, die die Koordinaten brauchen. */
type Hoehen = Required<Pick<MapInfo, 'heights' | 'heightsCell' | 'minY' | 'maxY'>>;

/**
 * Stehen die Felder für die Koordinaten vollständig und brauchbar da?
 * Geprüft getrennt von `isMapInfo`: Fehlen sie, lädt die Karte trotzdem.
 */
function hatHoehen(info: MapInfo): info is MapInfo & Hoehen {
  const { heights, heightsCell, minY, maxY } = info;
  return (
    typeof heights === 'string' &&
    typeof minY === 'number' &&
    typeof maxY === 'number' &&
    typeof heightsCell === 'number' &&
    Number.isInteger(heightsCell) &&
    heightsCell > 0 &&
    REGION % heightsCell === 0
  );
}

/**
 * Die Projektion, mit der die Koordinaten rechnen, oder der Grund, warum
 * es keine gibt: Azimut oder Richtung kennt das Frontend nicht.
 * Siehe docs/frontend.md, „Koordinaten“.
 */
function projektion(info: MapInfo): { p: Projektion; k: number } | string {
  const { projection = zweiZuEins(info.scale) } = info;
  const { azimuth, u, v, y } = projection;
  const richtungen = Object.hasOwn(RICHTUNGEN, azimuth) ? RICHTUNGEN[azimuth] : undefined;
  if (richtungen === undefined) return `azimuth ${String(azimuth)} unbekannt`;
  // Ohne Angabe die Vorgabe der Kamera, se oder s.
  const { direction = richtungen[0]! } = info;
  const k = richtungen.indexOf(direction);
  if (k < 0) return `direction ${direction} unbekannt`;
  const ganz = (n: unknown, min: number) => Number.isInteger(n) && (n as number) >= min;
  if (!ganz(u, 1) || !ganz(v, 1) || !ganz(y, 0)) return 'projection ohne ganze u, v und y';
  return { p: { azimuth, u, v, y }, k };
}

/** Setzt die Mitte der Oberseite eines Blocks der Welt in die Mitte der Karte. */
function zentriere(
  map: L.Map,
  { p, k }: { p: Projektion; k: number },
  block: Block,
  zoom: number,
): void {
  const [x, y, z] = inDenBlick(block, k);
  const [px, py] = projiziere(x + 0.5, y + 1, z + 0.5, p);
  map.setView(point(px, py), zoom);
}

/** Wie weit X und Z gehen: die Weltgrenze des Spiels. */
const WELTGRENZE = 30_000_000;

/**
 * Koordinaten und Umriss des Blocks, der unter Maus oder Finger zu sehen
 * ist, auf wenige Blöcke genau. Siehe docs/frontend.md, „Koordinaten“.
 */
function koordinaten(
  map: L.Map,
  karten: Hoehenkarten,
  { p, k }: { p: Projektion; k: number },
  { minY, maxY }: Hoehen,
): (px: number, py: number) => Promise<Block | undefined> {
  // Der Strahl läuft im Blick, Höhen und Anzeige sind in der Welt.
  const hoehe = (x: number, z: number) => {
    const [wx, , wz] = inDieWelt([x, 0, z], k);
    return karten.hoehe(wx, wz);
  };
  // Unten links die Anzeige, daneben ein Knopf, der `/tp` kopiert, und die
  // Rückmeldung. Siehe docs/frontend.md, „Koordinaten kopieren“.
  const leiste = L.DomUtil.create('div', 'leiste');
  const anzeige = L.DomUtil.create('div', 'koordinaten', leiste);
  const kopieren = L.DomUtil.create('button', 'kopieren', leiste);
  kopieren.type = 'button';
  kopieren.textContent = '⧉';
  kopieren.title = '/tp kopieren';
  kopieren.setAttribute('aria-label', '/tp kopieren');
  const meldung = L.DomUtil.create('span', 'meldung', leiste);
  meldung.setAttribute('role', 'status');
  // Ein Klick in die Leiste verschiebt die Karte nicht und wählt keinen
  // Block; die Maus darüber ändert die Anzeige nicht.
  L.DomEvent.disableClickPropagation(leiste);
  L.DomEvent.on(leiste, 'mousemove', L.DomEvent.stopPropagation);
  const control = new L.Control({ position: 'bottomleft' });
  control.onAdd = () => leiste;
  control.addTo(map);
  // Wie der Auswahlrahmen im Spiel.
  const rahmen = L.polyline([], { color: '#000', weight: 2, opacity: 0.8, interactive: false });
  rahmen.addTo(map);

  // Den Umriss zeigt nur ein Finger oder Stift; mit der Maus zeigt der
  // Zeiger selbst, wohin man zielt. Siehe docs/frontend.md, „Koordinaten“.
  let ohneZeiger = false;
  const merke = (event: PointerEvent): void => {
    ohneZeiger = event.pointerType !== 'mouse';
  };
  map.getContainer().addEventListener('pointerdown', merke);
  map.getContainer().addEventListener('pointermove', merke);

  // Ein Mausklick auf die Karte hält den Block fest, bis sich die Karte
  // bewegt oder Escape kommt; so kommt die Maus zur Leiste, ohne dass die
  // Anzeige unterwegs einen anderen Block zeigt.
  let gehalten = false;
  let gezeigt: Block | undefined;
  // Jeder Wert ist ein Knopf: Ein Klick macht ihn editierbar.
  const werte = ['X', 'Y', 'Z'].map((achse) => {
    const wert = document.createElement('button');
    wert.type = 'button';
    wert.className = 'wert';
    wert.title = `${achse} eingeben`;
    return wert;
  }) as [HTMLButtonElement, HTMLButtonElement, HTMLButtonElement];
  anzeige.append('X ', werte[0], '  Y ', werte[1], '  Z ', werte[2]);
  const schreibe = (welt: Block | undefined): void => {
    werte.forEach((wert, achse) => (wert.textContent = welt ? String(welt[achse]) : '–'));
  };
  const zeige = (block: Block | undefined): void => {
    gezeigt = block;
    schreibe(block && inDieWelt(block, k));
    // Gehalten zeigt es die Anzeige; der Umriss bleibt nach 0049 beim Finger
    // und Stift.
    anzeige.classList.toggle('gehalten', gehalten && block !== undefined);
    rahmen.setLatLngs(
      block && ohneZeiger
        ? umriss(block, p).map((linie) => linie.map(([x, y]) => point(x, y)))
        : [],
    );
  };
  const lasse = (): void => {
    if (!gehalten) return;
    gehalten = false;
    zeige(gezeigt);
  };
  let rueckmeldung: number | undefined;
  const melde = (text: string): void => {
    meldung.textContent = text;
    window.clearTimeout(rueckmeldung);
    rueckmeldung = window.setTimeout(() => (meldung.textContent = ''), 2000);
  };
  kopieren.addEventListener('click', () => {
    const welt = gezeigt && inDieWelt(gezeigt, k);
    if (!welt) {
      melde('Erst einen Block wählen');
      return;
    }
    // Einen Block höher, sonst steht man im Block; x und z rückt das Spiel
    // auf die Mitte. Siehe docs/frontend.md, „Koordinaten kopieren“.
    const befehl = `/tp ${welt[0]} ${welt[1] + 1} ${welt[2]}`;
    // Die Zwischenablage gibt es nur im sicheren Kontext, HTTPS oder localhost.
    if (!('clipboard' in navigator)) {
      melde('Kopieren geht nur über HTTPS');
      return;
    }
    navigator.clipboard.writeText(befehl).then(
      () => melde(`Kopiert: ${befehl}`),
      () => melde('Kopieren fehlgeschlagen'),
    );
  });
  // Lädt eine Bewegung noch Höhen, kann eine spätere vor ihr fertig sein.
  // Es gilt die letzte. Während eines Eintrags folgt die Anzeige keinem
  // Zeiger.
  let zuletzt = 0;
  let eintrag: { stand: Block; achse: number; feld: HTMLInputElement } | undefined;
  const ziele = async (event: L.LeafletMouseEvent): Promise<void> => {
    const nummer = ++zuletzt;
    const bloecke = strahl(event.latlng.lng, event.latlng.lat, p, minY, maxY);
    await karten.lade(bloecke.map((block) => inDieWelt(block, k)));
    if (nummer === zuletzt && !eintrag) zeige(pick(bloecke, hoehe));
  };
  /** Der Block in der Welt, den ein Bildpunkt zeigt. */
  const bei = async (px: number, py: number): Promise<Block | undefined> => {
    const bloecke = strahl(px, py, p, minY, maxY);
    await karten.lade(bloecke.map((block) => inDieWelt(block, k)));
    const block = pick(bloecke, hoehe);
    return block && inDieWelt(block, k);
  };

  // Ein Klick auf einen Wert macht ihn editierbar; Enter springt dorthin.
  // Siehe docs/frontend.md, „Zu Koordinaten springen“.
  const beende = (): void => {
    if (!eintrag) return;
    const { achse, feld } = eintrag;
    eintrag = undefined;
    feld.replaceWith(werte[achse]!);
    zeige(gezeigt);
  };
  const weise = (feld: HTMLInputElement, text: string): void => {
    feld.classList.add('falsch');
    feld.setAttribute('aria-invalid', 'true');
    melde(text);
  };
  const springe = async (): Promise<void> => {
    if (!eintrag) return;
    const { stand, achse, feld } = eintrag;
    const text = feld.value.trim();
    if (!/^-?\d+$/.test(text)) return weise(feld, 'Nur ganze Zahlen');
    const wert = Number(text);
    if (achse === 1 ? wert < minY || wert > maxY : Math.abs(wert) > WELTGRENZE) {
      return weise(feld, achse === 1 ? `Y von ${minY} bis ${maxY}` : `X und Z bis ±${WELTGRENZE}`);
    }
    let [x, y, z] = stand;
    if (achse === 0) x = wert;
    if (achse === 1) y = wert;
    if (achse === 2) z = wert;
    // Ändert sich X oder Z, kommt Y aus der Höhenkarte, sonst läge die Mitte
    // in der Schrägsicht neben dem Block. Ohne Höhe dort bleibt Y.
    if (achse !== 1) {
      await karten.lade([[x, y, z]]);
      // Ein Abbruch oder ein zweites Enter während des Ladens: dann springt
      // dieser Aufruf nicht.
      if (eintrag?.feld !== feld) return;
      y = karten.hoehe(x, z) ?? y;
    }
    const ziel: Block = [x, y, z];
    beende();
    zentriere(map, { p, k }, ziel, map.getZoom());
    // Wie nach einem Klick: Die Anzeige hält den Block, bis sich die Karte
    // wieder bewegt.
    zuletzt++;
    gehalten = true;
    zeige(inDenBlick(ziel, k));
  };
  const beginne = async (achse: number): Promise<void> => {
    if (eintrag) return;
    zuletzt++;
    // Ohne gezeigten Block gilt der in der Mitte der Karte.
    const { lat, lng } = map.getCenter();
    const stand = (gezeigt && inDieWelt(gezeigt, k)) ?? (await bei(lng, lat));
    if (!stand) return melde('Erst einen Block wählen');
    const feld = document.createElement('input');
    // Text statt Zahl: Nur so bietet jede Tastatur auf dem Handy das Minus.
    feld.type = 'text';
    feld.className = 'feld';
    feld.value = String(stand[achse]);
    feld.size = Math.max(feld.value.length, 3);
    feld.enterKeyHint = 'go';
    feld.autocomplete = 'off';
    feld.spellcheck = false;
    feld.setAttribute('aria-label', `${'XYZ'[achse]!} eingeben`);
    feld.addEventListener('input', () => {
      feld.classList.remove('falsch');
      feld.removeAttribute('aria-invalid');
    });
    feld.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') void springe();
      if (event.key === 'Escape') {
        // Escape bricht nur den Eintrag ab, nicht auch das Festhalten.
        event.stopPropagation();
        beende();
      }
    });
    feld.addEventListener('blur', beende);
    eintrag = { stand, achse, feld };
    schreibe(stand);
    werte[achse]!.replaceWith(feld);
    feld.focus();
    feld.select();
  };
  werte.forEach((wert, achse) => wert.addEventListener('click', () => void beginne(achse)));

  zeige(undefined);
  map.on('mousemove', (event) => {
    if (!gehalten && !eintrag) void ziele(event);
  });
  // Beginnt ein Druck in der Leiste und endet er über der Karte, schickt der
  // Browser den click an die Karte; der wählt keinen Block.
  let inLeiste = false;
  map.getContainer().addEventListener('pointerdown', (event) => {
    inLeiste = leiste.contains(event.target as Node);
  });
  // Auf dem Touchscreen kommt ein Tippen als click. Festhalten braucht es
  // nur mit der Maus; ohne sie folgt die Anzeige ohnehin keinem Zeiger.
  map.on('click', (event) => {
    if (inLeiste) return;
    gehalten = !ohneZeiger;
    void ziele(event);
  });
  map.on('movestart', lasse);
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') lasse();
  });
  map.on('mouseout', () => {
    if (gehalten || eintrag) return;
    zuletzt++;
    zeige(undefined);
  });
  return bei;
}

/** Ein Baum aus `trees.json`. Siehe docs/frontend.md, „Ansichten und Kompass“. */
interface Baum {
  path: string;
  camera: string;
  direction: string;
  look: string;
}

function istBaum(value: unknown): value is Baum {
  if (typeof value !== 'object' || value === null) return false;
  const baum = value as Record<string, unknown>;
  return ['path', 'camera', 'direction', 'look'].every((feld) => typeof baum[feld] === 'string');
}

/** Die Bäume aus `trees.json`, oder `null` ohne sie: dann ist `wurzel` selbst ein Baum. */
async function ladeListe(wurzel: string): Promise<Baum[] | null> {
  const path = `${wurzel}/trees.json`;
  const response = await fetch(path, FRISCH);
  // Ein Server, der auf unbekannte Pfade die index.html ausliefert, meint
  // dasselbe wie 404.
  if (response.status === 404 || response.headers.get('content-type')?.startsWith('text/html')) {
    return null;
  }
  if (!response.ok) throw new Error(`${path}: ${response.status} ${response.statusText}`);
  const liste = (await response.json().catch(() => undefined)) as { trees?: unknown } | undefined;
  const trees = liste?.trees;
  if (!Array.isArray(trees) || trees.length === 0 || !trees.every(istBaum)) {
    throw new Error(`${path}: keine brauchbare Liste der Bäume`);
  }
  return trees;
}

/** Wohin Norden auf dem Bildschirm zeigt, in Grad im Uhrzeigersinn von oben. */
function norden({ azimuth, u, v }: Projektion, k: number): number {
  // Norden ist −z in der Welt; im Blick dreht jede Vierteldrehung (dx, dz)
  // zu (dz, −dx).
  let [dx, dz] = [0, -1];
  for (let i = 0; i < k; i++) [dx, dz] = [dz, -dx];
  const [sx, sy] = azimuth === 'north' ? [dx * u, dz * v] : [(dx - dz) * u, (dx + dz) * v];
  return (Math.atan2(sx, -sy) * 180) / Math.PI;
}

/** Der Kompass oben rechts: ein Pfeil nach Norden. */
function kompass(map: L.Map, grad: number): void {
  const element = L.DomUtil.create('div', 'kompass');
  element.textContent = '↑';
  element.title = 'Norden';
  element.setAttribute('role', 'img');
  element.setAttribute('aria-label', 'Norden');
  element.style.transform = `rotate(${grad.toFixed(1)}deg)`;
  const control = new L.Control({ position: 'topright' });
  control.onAdd = () => element;
  control.addTo(map);
}

const HIMMEL: Record<string, string> = {
  se: 'Südost',
  sw: 'Südwest',
  nw: 'Nordwest',
  ne: 'Nordost',
  // Genordet steht oben, wohin die Kamera blickt.
  s: 'Norden',
  w: 'Osten',
  n: 'Süden',
  e: 'Westen',
};

/** Der Name eines Baums im Umschalter, etwa „2:1 aus Südost“. */
function anzeigename({ camera, direction, look }: Baum): string {
  const himmel = HIMMEL[direction];
  const name = !himmel
    ? `${camera} · ${direction}`
    : camera === 'top-north'
      ? `Von oben, ${himmel} oben`
      : camera === 'north-45'
        ? `Schräg, ${himmel} oben`
        : `${camera === 'top' ? 'Von oben' : camera} aus ${himmel}`;
  if (look === 'map') return name;
  return `${name} · ${look === 'cinematic' ? 'Cinematic' : look}`;
}

/**
 * Die Adresse der Ansicht: der Baum, der Block der Mitte in Weltkoordinaten
 * und der Zoom ab der feinsten Stufe, denn `maxZoom` hängt je Baum an seiner
 * Ausdehnung. Siehe docs/frontend.md, „Ansichten und Kompass“.
 */
async function adresse(
  map: L.Map,
  maxZoom: number,
  mitte: () => Promise<Block | undefined>,
  tree: string | undefined,
): Promise<URL> {
  const block = await mitte();
  const url = new URL(location.href);
  if (tree !== undefined) url.searchParams.set('tree', tree);
  if (block) url.searchParams.set('at', block.join(','));
  else url.searchParams.delete('at');
  url.searchParams.set('zoom', String(map.getZoom() - maxZoom));
  return url;
}

/**
 * Der Umschalter zwischen den Bäumen. Er öffnet den gewählten Baum mit dem
 * Block, der in der Mitte zu sehen ist, wieder in der Mitte, mit derselben
 * Vergrösserung.
 */
function umschalter(
  map: L.Map,
  liste: Baum[],
  aktuell: Baum,
  ansicht: (tree: string) => Promise<URL>,
): void {
  const auswahl = L.DomUtil.create('select', 'baeume');
  auswahl.setAttribute('aria-label', 'Ansicht');
  for (const baum of liste) {
    auswahl.add(new Option(anzeigename(baum), baum.path, false, baum === aktuell));
  }
  L.DomEvent.disableClickPropagation(auswahl);
  auswahl.addEventListener('change', () => {
    void ansicht(auswahl.value).then((url) => location.assign(url));
  });
  const control = new L.Control({ position: 'topright' });
  control.onAdd = () => auswahl;
  control.addTo(map);
}

/**
 * Ein Knopf ⌂ unter + und −, der die ganze Karte einpasst. Er ist ein Link
 * wie die beiden, also per Tastatur erreichbar. Siehe docs/frontend.md,
 * „Zoom über und unter den Kacheln“.
 */
function ganzeKarte(map: L.Map, bounds: L.LatLngBounds): void {
  const knopf = L.DomUtil.create('a', 'ganze-karte', map.zoomControl.getContainer());
  knopf.href = '#';
  knopf.role = 'button';
  knopf.textContent = '⌂';
  knopf.title = 'Ganze Karte';
  knopf.setAttribute('aria-label', 'Ganze Karte');
  L.DomEvent.on(knopf, 'click', (event) => {
    L.DomEvent.preventDefault(event);
    map.fitBounds(bounds);
  });
}

/**
 * Die feinste Stufe, auf der `grenzen` in ein Fenster dieser Grösse passen.
 * Ohne Fläche oder ohne Fenster gilt `minZoom` aus `map.json`.
 */
function fitZoom(grenzen: Grenzen, info: MapInfo, size: L.Point): number {
  const [left, top, right, bottom] = grenzen;
  const faktor = Math.min(size.x / (right - left), size.y / (bottom - top));
  if (!(faktor > 0 && Number.isFinite(faktor))) return info.minZoom;
  return info.maxZoom + Math.floor(Math.log2(faktor));
}

async function start(): Promise<void> {
  // Ein Skin lädt, während map.json kommt; ohne ihn fällt sein Import weg.
  const skinModul = __SKIN__ ? import('virtual:skin') : undefined;
  // Ohne Angabe liegen die Kacheln neben der Seite. Der Parameter ist für
  // den Smoke-Test und für mehrere Karten auf demselben Server da.
  const parameter = new URLSearchParams(location.search);
  const wurzel = parameter.get('tiles') ?? 'tiles';
  const liste = await ladeListe(wurzel);
  const baum = liste && (liste.find((b) => b.path === parameter.get('tree')) ?? liste[0]!);
  const base = baum ? `${wurzel}/${baum.path}` : wurzel;
  const { info, stand } = await load(base);

  const [left, top, right, bottom] = info.bounds;
  const bounds = L.latLngBounds(point(left, top), point(right, bottom));
  const blick = projektion(info);

  const map = L.map('map', {
    crs: crs(info),
    maxZoom: info.maxZoom + EXTRA_ZOOM,
    attributionControl: false,
  });
  // Ein Skin gestaltet um die Karte und sagt, was als ganze Karte gilt. Er
  // bekommt nur, was in skin-api.ts steht. Siehe docs/frontend.md, „Skins“.
  let ganz: Grenzen = info.bounds;
  if (skinModul && typeof blick !== 'string') {
    const { default: skin } = await skinModul;
    const { p, k } = blick;
    const antwort = skin?.({
      version: VERSION,
      karte: map,
      container: map.getContainer(),
      projektion: p,
      k,
      projiziere: (x, y, z) => projiziere(x, y, z, p),
      maxZoom: info.maxZoom,
      area: info.area,
      seaLevel: info.seaLevel,
      minY: info.minY ?? -64,
      fitZoom: (grenzen, breite, hoehe) => fitZoom(grenzen, info, L.point(breite, hoehe)),
      texte: __SKIN_TEXTE__,
    });
    if (antwort) ganz = antwort.ganzeKarte;
  }
  const einpassen = L.latLngBounds(point(ganz[0], ganz[1]), point(ganz[2], ganz[3]));
  // Ein Baum behält seine Stufen, wenn die Welt wächst, und Zoom 0 passt
  // dann nicht mehr ins Fenster. Darunter verkleinert Leaflet die Kacheln
  // von Zoom 0, bis die ganze Karte zu sehen ist.
  const minZoom = Math.min(info.minZoom, fitZoom(ganz, info, map.getSize()));
  map.setMinZoom(minZoom);
  ganzeKarte(map, einpassen);

  L.tileLayer(`${base}/${info.tiles}`, {
    tileSize: info.tileSize,
    // Die Untergrenze setzt die Karte, und ein Skin senkt sie, wird das
    // Fenster kleiner. Die Ebene zeigt auf jeder Stufe Kacheln.
    minZoom: -Infinity,
    maxZoom: info.maxZoom + EXTRA_ZOOM,
    // Über die gerenderte Stufe hinaus gibt es keine Kacheln mehr; Leaflet
    // soll dann die vorhandenen vergrössern statt ins Leere zu laden.
    // Unter Zoom 0 ebenso, nur verkleinert.
    maxNativeZoom: info.maxZoom,
    minNativeZoom: info.minZoom,
    // Ausserhalb liegt nichts. Ohne diese Grenze fragt Leaflet beim
    // Herumziehen reihenweise Kacheln an, die es nicht gibt.
    bounds,
    noWrap: true,
  }).addTo(map);

  if (typeof blick !== 'string') kompass(map, norden(blick.p, blick.k));
  lizenzenAnzeigen(map);
  if (stand) standAnzeigen(map, stand);
  let bei: ((px: number, py: number) => Promise<Block | undefined>) | undefined;
  // Koordinaten und Ebenen teilen sich die Höhen.
  const karten = hatHoehen(info) ? hoehen(base, info.heights, info.heightsCell) : undefined;
  if (karten && hatHoehen(info)) {
    if (typeof blick === 'string') console.warn(`${base}/map.json: keine Koordinaten, ${blick}`);
    else bei = koordinaten(map, karten, blick, info);
  } else if (info.heights !== undefined) {
    console.warn(`${base}/map.json: heights ohne brauchbare heightsCell, minY und maxY`);
  }
  // Ebenen neben trees.json; ohne layers.json keine. Siehe docs/frontend.md, „Ebenen“.
  if (typeof blick !== 'string') {
    void ebenen({ map, wurzel, blick, scale: info.scale, maxZoom: info.maxZoom, karten, heightsCell: info.heightsCell, seaLevel: info.seaLevel }).catch(
      (error: unknown) => console.error(error),
    );
  }
  const mitte = () => {
    const { lat, lng } = map.getCenter();
    return bei ? bei(lng, lat) : Promise.resolve(undefined);
  };
  const ansicht = (tree: string | undefined) => adresse(map, info.maxZoom, mitte, tree);
  if (liste && baum && liste.length > 1) umschalter(map, liste, baum, ansicht);
  // Die Adresse folgt der Karte, ohne Einträge im Verlauf. Ohne Koordinaten
  // gibt es keinen Block für `at`, und sie bleibt, wie sie ist. Es gilt die
  // letzte Bewegung.
  if (bei) {
    let zuletzt = 0;
    map.on('moveend', () => {
      const nummer = ++zuletzt;
      void ansicht(baum?.path).then((url) => {
        if (nummer === zuletzt) history.replaceState(history.state, '', url);
      });
    });
  }

  // Kommt die Seite aus dem Umschalter oder aus einer kopierten Adresse,
  // steht der Block der Mitte darin, in Weltkoordinaten, und `zoom` zählt ab
  // der feinsten Stufe.
  const at = parameter.get('at')?.split(',').map(Number);
  const zoom = Number(parameter.get('zoom') ?? Number.NaN);
  if (at?.length === 3 && at.every(Number.isInteger) && typeof blick !== 'string') {
    zentriere(map, blick, at as unknown as Block, info.maxZoom + (Number.isFinite(zoom) ? zoom : 0));
  } else {
    map.fitBounds(einpassen);
  }
}

start().catch((error: unknown) => {
  console.error(error);
  const element = document.getElementById('map');
  if (element) {
    const message = document.createElement('p');
    message.className = 'error';
    message.textContent = `Karte lässt sich nicht laden — ${String(error)}`;
    element.replaceChildren(message);
  }
});
