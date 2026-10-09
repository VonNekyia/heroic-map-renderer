/**
 * Regionen, Kreise und Linien einer Ebene als Pfade von Leaflet, auf dem
 * Gelände. Das Format steht in docs/benutzung/ebenen.md, „Region“, „Kreis“
 * und „Linie“; wie die Karte sie zeichnet in docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import { netz, vieleck, zug, type Blick, type Gelaende, type Polygon, type Punkt, type Zug } from './gelaende';
import { farbe, istObjekt, istText, istZahl, punkt, punkte } from './pruefen';

/** Ein Rand: Farbe, Breite in Pixeln des Schirms, Strich und Lücke, wenn gestrichelt. */
export interface Rand {
  farbe: string;
  breite: number;
  strich?: [number, number];
}

/** Eine Region, ein Kreis oder eine Linie, wie die Ansicht sie braucht. */
export interface Form {
  id: string;
  /** Die Flächen einer Region. */
  polygone: Polygon[];
  /** Ein Kreis wird erst beim Zeichnen ein Vieleck, mit Seiten nach `heightsCell`. */
  kreis?: { mitte: Punkt; radius: number };
  linie?: Punkt[];
  name?: string;
  fuellung?: string;
  rand: Rand;
  panel?: unknown[];
}

/** Grenzen aus docs/benutzung/ebenen.md, „Grenzen“, und der Radius aus „Kreis“. */
const GRENZEN = { punkte: 10_000, loecher: 100, radius: 100_000 };

/** Ohne Farbe am Rand: die Füllung ohne Alpha, sonst die der Kartenschrift. */
const RANDFARBE = '#2B2B2B';

function rand(wert: unknown, vorgabe: string): Rand {
  const r = istObjekt(wert) ? wert : {};
  const strich = Array.isArray(r.dash) && r.dash.length === 2 && r.dash.every((d) => istZahl(d) && d > 0) ? (r.dash as [number, number]) : ([8, 6] as [number, number]);
  return {
    farbe: farbe(r.color) ?? vorgabe,
    breite: istZahl(r.width) && r.width >= 0 ? r.width : 2,
    strich: r.style === 'dashed' ? strich : undefined,
  };
}

/**
 * Liest eine Region, einen Kreis oder eine Linie; anderes ist `undefined`.
 * Was über die Grenzen geht, nennt `warne` und übergeht es.
 */
export function form(wert: Record<string, unknown>, warne: (text: string) => void): Form | undefined {
  const art = wert.type;
  if ((art !== 'region' && art !== 'circle' && art !== 'line') || !istText(wert.id, 64)) return undefined;
  const id = wert.id;
  const flaeche = art !== 'line';
  const fuellung = flaeche ? farbe(wert.fill) : undefined;
  const grund = {
    id,
    polygone: [] as Polygon[],
    name: flaeche && istText(wert.name, 64) ? wert.name : undefined,
    fuellung,
    rand: rand(wert.stroke, fuellung?.slice(0, 7) ?? RANDFARBE),
    panel: flaeche && istObjekt(wert.panel) && Array.isArray(wert.panel.blocks) ? wert.panel.blocks : undefined,
  };
  if (art === 'line') {
    const linie = punkte(wert.points, 2, GRENZEN.punkte);
    if (!linie && Array.isArray(wert.points) && wert.points.length > GRENZEN.punkte) warne(`Linie ${id}: mehr als ${GRENZEN.punkte} Punkte, übergangen`);
    return linie && { ...grund, linie };
  }
  if (art === 'circle') {
    const mitte = punkt(wert.center);
    const radius = wert.radius;
    if (!mitte || !istZahl(radius) || radius <= 0) return undefined;
    if (radius > GRENZEN.radius) {
      warne(`Kreis ${id}: Radius ${radius} über ${GRENZEN.radius}, übergangen`);
      return undefined;
    }
    return { ...grund, kreis: { mitte, radius } };
  }
  if (!Array.isArray(wert.polygons)) return undefined;
  let anzahl = 0;
  for (const p of wert.polygons) {
    const aussen = istObjekt(p) ? punkte(p.outer, 3, Infinity) : undefined;
    const roh = istObjekt(p) ? (p.holes ?? []) : undefined;
    const loecher = Array.isArray(roh) ? roh.map((l) => punkte(l, 3, Infinity)) : undefined;
    if (!aussen || !loecher || loecher.some((l) => !l)) return undefined;
    if (loecher.length > GRENZEN.loecher) {
      warne(`Region ${id}: mehr als ${GRENZEN.loecher} Löcher in einem Polygon, übergangen`);
      return undefined;
    }
    anzahl += aussen.length + loecher.reduce((s, l) => s + l!.length, 0);
    grund.polygone.push({ aussen, loecher: loecher as Punkt[][] });
  }
  if (anzahl > GRENZEN.punkte) {
    warne(`Region ${id}: ${anzahl} Punkte, mehr als ${GRENZEN.punkte}, übergangen`);
    return undefined;
  }
  return grund.polygone.length ? grund : undefined;
}

/** Ein Lauf eines Rands und wie weit er vom Anfang des ganzen Zugs beginnt, in Pixeln der feinsten Stufe. */
export interface Strich {
  linie: L.Polyline;
  versatz: number;
}

/** Was `zeichne` von der Ebene braucht. */
export interface Zeichnen {
  renderer: L.Renderer;
  blick: Blick;
  /** Die Höhen im Rechteck [x0, z0, x1, z1] der Welt. */
  gelaende: (bereich: [number, number, number, number]) => Promise<Gelaende>;
  tafel: (bausteine: unknown[]) => HTMLElement;
  tafelOptionen: L.PopupOptions;
}

const latLng = ([x, y]: Punkt) => L.latLng(y, x);

/**
 * Teilt einen Zug in Läufe, sichtbar oder verdeckt. Eine Strecke gehört zum
 * Lauf ihres Endpunkts; benachbarte Läufe teilen sich einen Punkt.
 */
function laeufe(z: Zug, r: Rand, renderer: L.Renderer): Strich[] {
  const stil: L.PolylineOptions = {
    renderer,
    color: r.farbe,
    weight: r.breite,
    opacity: 1,
    interactive: false,
    dashArray: r.strich?.join(' '),
    lineCap: r.strich ? 'butt' : 'round',
  };
  // Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
  const verdecktStil: L.PolylineOptions = { ...stil, weight: Math.max(1, r.breite / 2), opacity: 0.4, dashArray: '3 4', lineCap: 'butt' };
  const aus: Strich[] = [];
  let weg = 0;
  for (let i = 1; i < z.punkte.length; ) {
    const [von, verdeckt, versatz] = [i - 1, z.verdeckt[i], weg];
    for (; i < z.punkte.length && z.verdeckt[i] === verdeckt; i++) {
      const [a, b] = [z.punkte[i - 1]!, z.punkte[i]!];
      weg += Math.hypot(b[0] - a[0], b[1] - a[1]);
    }
    const linie = L.polyline(z.punkte.slice(von, i).map(latLng), verdeckt ? verdecktStil : stil);
    aus.push({ linie, versatz: verdeckt ? 0 : versatz });
  }
  return aus;
}

/**
 * Setzt die Striche jedes Laufs fort, wo der vorige aufhört: `dashOffset`
 * in Pixeln des Schirms, `faktor` Pixel des Schirms je Pixel der feinsten
 * Stufe.
 */
export function versetze(striche: readonly Strich[], faktor: number): void {
  for (const { linie, versatz } of striche) if (versatz && linie.options.dashArray) linie.setStyle({ dashOffset: String(versatz * faktor) });
}

/** Das Rechteck der Welt um eine Form. */
function bereich(f: Form): [number, number, number, number] {
  if (f.kreis) {
    const { mitte, radius } = f.kreis;
    return [mitte[0] - radius, mitte[1] - radius, mitte[0] + radius, mitte[1] + radius];
  }
  let [x0, z0, x1, z1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of f.linie ?? f.polygone.flatMap((q) => q.aussen)) [x0, z0, x1, z1] = [Math.min(x0, p[0]), Math.min(z0, p[1]), Math.max(x1, p[0]), Math.max(z1, p[1])];
  return [x0, z0, x1, z1];
}

/**
 * Zeichnet die Formen einer Ebene: erst alle Flächen, dann alle Ränder und
 * Linien, so liegt kein Rand unter einer Fläche derselben Ebene. Eine Fläche
 * mit Namen oder Tafel ist ein Ziel, auch ohne Füllung.
 */
export async function zeichne(formen: readonly Form[], z: Zeichnen): Promise<{ flaechen: L.Layer[]; striche: Strich[] }> {
  const flaechen: L.Layer[] = [];
  const striche: Strich[] = [];
  for (const f of formen) {
    const g = await z.gelaende(bereich(f));
    const polygone = f.kreis ? [{ aussen: vieleck(f.kreis.mitte, f.kreis.radius, g.c), loecher: [] }] : f.polygone;
    const ziel = f.name !== undefined || f.panel !== undefined;
    if (polygone.length && (f.fuellung || ziel)) {
      const ringe = netz(polygone, g, z.blick, 0.25);
      const flaeche = L.polygon(ringe.map((r) => r.map(latLng)), {
        renderer: z.renderer,
        stroke: false,
        fillColor: f.fuellung ?? '#000000',
        fillOpacity: f.fuellung ? 1 : 0,
        interactive: ziel,
      });
      if (f.name) {
        const name = document.createElement('span');
        name.textContent = f.name;
        flaeche.bindTooltip(name, { sticky: true, className: 'ebene-name' });
      }
      if (f.panel) flaeche.bindPopup(() => z.tafel(f.panel!), z.tafelOptionen);
      flaechen.push(flaeche);
    }
    if (f.rand.breite === 0) continue;
    if (f.linie) striche.push(...laeufe(zug(f.linie, false, g, z.blick), f.rand, z.renderer));
    for (const p of polygone) for (const ring of [p.aussen, ...p.loecher]) striche.push(...laeufe(zug(ring, true, g, z.blick), f.rand, z.renderer));
  }
  return { flaechen, striche };
}
