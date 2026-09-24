import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './style.css';

/** Was `map.json` aus dem Renderer mitbringt. */
interface MapInfo {
  tileSize: number;
  scale: number;
  minZoom: number;
  maxZoom: number;
  tiles: string;
  bounds: [number, number, number, number];
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
    typeof info.minZoom === 'number' &&
    typeof info.maxZoom === 'number' &&
    typeof info.tiles === 'string' &&
    Array.isArray(info.bounds) &&
    info.bounds.length === 4 &&
    info.bounds.every((n) => typeof n === 'number')
  );
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
