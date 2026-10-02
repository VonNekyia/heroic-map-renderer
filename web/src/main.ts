import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import {
  inDenBlick,
  inDieWelt,
  pick,
  projiziere,
  REGION,
  RICHTUNGEN,
  region,
  strahl,
  umriss,
  zweiZuEins,
  type Block,
  type Projektion,
} from './pick';
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
}

/** In einer Höhenkarte: keine Zelle mit Block, oder kein fertiger Chunk. */
const LEER = -32768;

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

async function load(base: string): Promise<MapInfo> {
  const path = `${base}/map.json`;
  const response = await fetch(path);
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
  return info;
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

/** Eine Höhenkarte: zlib, darin n × n Zellen, je i16 little-endian. */
async function ladeKarte(path: string, n: number): Promise<Int16Array | null> {
  const response = await fetch(path);
  // Keine Datei heisst kein Chunk. Ein Server, der auf unbekannte Pfade die
  // index.html ausliefert, meint dasselbe.
  if (response.status === 404 || response.headers.get('content-type')?.startsWith('text/html')) {
    return null;
  }
  if (!response.ok || response.body === null) {
    throw new Error(`${path}: ${response.status} ${response.statusText}`);
  }
  const daten = await new Response(
    response.body.pipeThrough(new DecompressionStream('deflate')),
  ).arrayBuffer();
  if (daten.byteLength !== n * n * 2) {
    throw new Error(`${path}: ${daten.byteLength} Byte statt ${n * n * 2}`);
  }
  // ponytail: liest in der Byte-Reihenfolge der Maschine, also nur auf
  // little-endian richtig; auf big-endian bräuchte es eine DataView.
  return new Int16Array(daten);
}

/**
 * Die Höhenkarten der Regionen, je Zelle aus `zelle` × `zelle` Spalten die
 * Höhe dessen, was man sieht. Siehe docs/benutzung/map-json.md.
 */
function hoehen(base: string, muster: string, zelle: number) {
  const karten = new Map<string, Int16Array | null>();
  const unterwegs = new Map<string, Promise<void>>();

  const ladeRegion = (rx: number, rz: number): Promise<void> => {
    const name = `${rx}.${rz}`;
    if (karten.has(name)) return Promise.resolve();
    let laden = unterwegs.get(name);
    if (laden === undefined) {
      const path = `${base}/${muster.replace('{x}', String(rx)).replace('{z}', String(rz))}`;
      laden = ladeKarte(path, REGION / zelle)
        .catch((error: unknown) => {
          console.error(error);
          return null;
        })
        .then((karte) => {
          unterwegs.delete(name);
          karten.set(name, karte);
          // ponytail: verdrängt die älteste statt der am längsten
          // ungenutzten; eine verdrängte kommt aus dem HTTP-Cache wieder.
          if (karten.size > 64) karten.delete(karten.keys().next().value!);
        });
      unterwegs.set(name, laden);
    }
    return laden;
  };

  return {
    /** Lädt die Karten aller Regionen, durch deren Spalten die Würfel gehen. */
    lade: async (bloecke: readonly Block[]): Promise<void> => {
      const regionen = new Map<string, [number, number]>();
      for (const [x, , z] of bloecke) {
        const { rx, rz } = region(x, z, zelle);
        regionen.set(`${rx}.${rz}`, [rx, rz]);
      }
      await Promise.all([...regionen.values()].map(([rx, rz]) => ladeRegion(rx, rz)));
    },
    /** Die Höhe der Zelle einer Spalte, `undefined` für leer oder nicht geladen. */
    hoehe: (x: number, z: number): number | undefined => {
      const { rx, rz, i } = region(x, z, zelle);
      const wert = karten.get(`${rx}.${rz}`)?.[i];
      return wert === undefined || wert === LEER ? undefined : wert;
    },
  };
}

/**
 * Koordinaten und Umriss des Blocks, der unter Maus oder Finger zu sehen
 * ist, auf wenige Blöcke genau. Siehe docs/frontend.md, „Koordinaten“.
 */
function koordinaten(
  map: L.Map,
  base: string,
  { p, k }: { p: Projektion; k: number },
  { heights, heightsCell, minY, maxY }: Hoehen,
): (px: number, py: number) => Promise<Block | undefined> {
  const karten = hoehen(base, heights, heightsCell);
  // Der Strahl läuft im Blick, Höhen und Anzeige sind in der Welt.
  const hoehe = (x: number, z: number) => {
    const [wx, , wz] = inDieWelt([x, 0, z], k);
    return karten.hoehe(wx, wz);
  };
  const anzeige = L.DomUtil.create('div', 'koordinaten');
  const control = new L.Control({ position: 'bottomleft' });
  control.onAdd = () => anzeige;
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

  const zeige = (block: Block | undefined): void => {
    const welt = block && inDieWelt(block, k);
    anzeige.textContent = welt ? `X ${welt[0]}  Y ${welt[1]}  Z ${welt[2]}` : 'X –  Y –  Z –';
    rahmen.setLatLngs(
      block && ohneZeiger
        ? umriss(block, p).map((linie) => linie.map(([x, y]) => point(x, y)))
        : [],
    );
  };
  // Lädt eine Bewegung noch Höhen, kann eine spätere vor ihr fertig sein.
  // Es gilt die letzte.
  let zuletzt = 0;
  const ziele = async (event: L.LeafletMouseEvent): Promise<void> => {
    const nummer = ++zuletzt;
    const bloecke = strahl(event.latlng.lng, event.latlng.lat, p, minY, maxY);
    await karten.lade(bloecke.map((block) => inDieWelt(block, k)));
    if (nummer === zuletzt) zeige(pick(bloecke, hoehe));
  };
  /** Der Block in der Welt, den ein Bildpunkt zeigt. */
  const bei = async (px: number, py: number): Promise<Block | undefined> => {
    const bloecke = strahl(px, py, p, minY, maxY);
    await karten.lade(bloecke.map((block) => inDieWelt(block, k)));
    const block = pick(bloecke, hoehe);
    return block && inDieWelt(block, k);
  };

  zeige(undefined);
  map.on('mousemove', (event) => void ziele(event));
  // Auf dem Touchscreen kommt ein Tippen als click.
  map.on('click', (event) => void ziele(event));
  map.on('mouseout', () => {
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
  const response = await fetch(path);
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
 * Der Umschalter zwischen den Bäumen. Er öffnet den gewählten Baum mit dem
 * Block, der in der Mitte zu sehen ist, wieder in der Mitte, und mit
 * derselben Vergrösserung gegenüber der feinsten Stufe: `maxZoom` hängt je
 * Baum an seiner Ausdehnung.
 */
function umschalter(
  map: L.Map,
  maxZoom: number,
  liste: Baum[],
  aktuell: Baum,
  mitte: () => Promise<Block | undefined>,
): void {
  const auswahl = L.DomUtil.create('select', 'baeume');
  auswahl.setAttribute('aria-label', 'Ansicht');
  for (const baum of liste) {
    auswahl.add(new Option(anzeigename(baum), baum.path, false, baum === aktuell));
  }
  L.DomEvent.disableClickPropagation(auswahl);
  auswahl.addEventListener('change', () => {
    void mitte().then((block) => {
      const adresse = new URL(location.href);
      adresse.searchParams.set('tree', auswahl.value);
      if (block) adresse.searchParams.set('at', block.join(','));
      else adresse.searchParams.delete('at');
      adresse.searchParams.set('zoom', String(map.getZoom() - maxZoom));
      location.assign(adresse);
    });
  });
  const control = new L.Control({ position: 'topright' });
  control.onAdd = () => auswahl;
  control.addTo(map);
}

/**
 * Die feinste Stufe, auf der die ganze Karte in ein Fenster dieser Grösse
 * passt. Ohne Fläche oder ohne Fenster gilt `minZoom` aus `map.json`.
 */
function fitZoom(info: MapInfo, size: L.Point): number {
  const [left, top, right, bottom] = info.bounds;
  const faktor = Math.min(size.x / (right - left), size.y / (bottom - top));
  if (!(faktor > 0 && Number.isFinite(faktor))) return info.minZoom;
  return info.maxZoom + Math.floor(Math.log2(faktor));
}

async function start(): Promise<void> {
  // Ohne Angabe liegen die Kacheln neben der Seite. Der Parameter ist für
  // den Smoke-Test und für mehrere Karten auf demselben Server da.
  const parameter = new URLSearchParams(location.search);
  const wurzel = parameter.get('tiles') ?? 'tiles';
  const liste = await ladeListe(wurzel);
  const baum = liste && (liste.find((b) => b.path === parameter.get('tree')) ?? liste[0]!);
  const base = baum ? `${wurzel}/${baum.path}` : wurzel;
  const info = await load(base);

  const [left, top, right, bottom] = info.bounds;
  const bounds = L.latLngBounds(point(left, top), point(right, bottom));

  const map = L.map('map', {
    crs: crs(info),
    maxZoom: info.maxZoom + EXTRA_ZOOM,
    attributionControl: false,
  });
  // Ein Baum behält seine Stufen, wenn die Welt wächst, und Zoom 0 passt
  // dann nicht mehr ins Fenster. Darunter verkleinert Leaflet die Kacheln
  // von Zoom 0, bis die ganze Karte zu sehen ist.
  const minZoom = Math.min(info.minZoom, fitZoom(info, map.getSize()));
  map.setMinZoom(minZoom);

  L.tileLayer(`${base}/${info.tiles}`, {
    tileSize: info.tileSize,
    minZoom,
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

  const blick = projektion(info);
  if (typeof blick !== 'string') kompass(map, norden(blick.p, blick.k));
  let bei: ((px: number, py: number) => Promise<Block | undefined>) | undefined;
  if (hatHoehen(info)) {
    if (typeof blick === 'string') console.warn(`${base}/map.json: keine Koordinaten, ${blick}`);
    else bei = koordinaten(map, base, blick, info);
  } else if (info.heights !== undefined) {
    console.warn(`${base}/map.json: heights ohne brauchbare heightsCell, minY und maxY`);
  }
  if (liste && baum && liste.length > 1) {
    const mitte = () => {
      const { lat, lng } = map.getCenter();
      return bei ? bei(lng, lat) : Promise.resolve(undefined);
    };
    umschalter(map, info.maxZoom, liste, baum, mitte);
  }

  // Kommt die Seite aus dem Umschalter, steht der Block der Mitte in der
  // Adresse, in Weltkoordinaten, und `zoom` zählt ab der feinsten Stufe.
  const at = parameter.get('at')?.split(',').map(Number);
  const zoom = Number(parameter.get('zoom') ?? Number.NaN);
  if (at?.length === 3 && at.every(Number.isInteger) && typeof blick !== 'string') {
    const [x, y, z] = inDenBlick(at as unknown as Block, blick.k);
    // Die Mitte der Oberseite.
    const [px, py] = projiziere(x + 0.5, y + 1, z + 0.5, blick.p);
    map.setView(point(px, py), info.maxZoom + (Number.isFinite(zoom) ? zoom : 0));
  } else {
    map.fitBounds(bounds);
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
