import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import {
  pick,
  REGION,
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
function projektion(info: MapInfo): Projektion | string {
  const { projection = zweiZuEins(info.scale) } = info;
  const { azimuth, u, v, y } = projection;
  // Die Richtung, aus der die Kamera schaut, solange es nur eine gibt.
  const richtung = azimuth === 'diagonal' ? 'se' : azimuth === 'north' ? 's' : undefined;
  if (richtung === undefined) return `azimuth ${String(azimuth)} unbekannt`;
  const { direction = richtung } = info;
  if (direction !== richtung) return `direction ${direction} unbekannt`;
  const ganz = (n: unknown, min: number) => Number.isInteger(n) && (n as number) >= min;
  if (!ganz(u, 1) || !ganz(v, 1) || !ganz(y, 0)) return 'projection ohne ganze u, v und y';
  return { azimuth, u, v, y };
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
  p: Projektion,
  { heights, heightsCell, minY, maxY }: Hoehen,
): void {
  const karten = hoehen(base, heights, heightsCell);
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
    anzeige.textContent = block ? `X ${block[0]}  Y ${block[1]}  Z ${block[2]}` : 'X –  Y –  Z –';
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
    await karten.lade(bloecke);
    if (nummer === zuletzt) zeige(pick(bloecke, karten.hoehe));
  };

  zeige(undefined);
  map.on('mousemove', (event) => void ziele(event));
  // Auf dem Touchscreen kommt ein Tippen als click.
  map.on('click', (event) => void ziele(event));
  map.on('mouseout', () => {
    zuletzt++;
    zeige(undefined);
  });
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
  const base = new URLSearchParams(location.search).get('tiles') ?? 'tiles';
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

  if (hatHoehen(info)) {
    const p = projektion(info);
    if (typeof p === 'string') console.warn(`${base}/map.json: keine Koordinaten, ${p}`);
    else koordinaten(map, base, p, info);
  } else if (info.heights !== undefined) {
    console.warn(`${base}/map.json: heights ohne brauchbare heightsCell, minY und maxY`);
  }

  map.fitBounds(bounds);
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
