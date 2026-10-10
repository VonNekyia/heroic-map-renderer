/**
 * Regionen, Kreise und Linien einer Ebene als Pfade von Leaflet, auf dem
 * Gelände. Das Format steht in docs/benutzung/ebenen.md, „Region“, „Kreis“
 * und „Linie“; wie die Karte sie zeichnet in docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import {
  netz,
  rechteck,
  ringImRechteck,
  vieleck,
  wand,
  zug,
  zugImRechteck,
  type Blick,
  type Gelaende,
  type Polygon,
  type Punkt,
  type Rechteck,
  type Zug,
} from './gelaende';
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

/**
 * Die Wand am Rand einer Fläche im iso: Höhe in Blöcken, Deckkraft am Boden,
 * nach oben linear bis 0, in Bändern. Gewählt in 0104.
 */
const WAND = { hoehe: 6, unten: 0.6, baender: 12 };

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
  /** Die Höhen aller Formen der Ebene, einmal geladen. */
  gelaende: Gelaende;
  /** `area` aus map.json: Ausserhalb gibt es weder Kacheln noch Höhen. */
  area?: Rechteck;
  tafel: (bausteine: unknown[]) => HTMLElement;
  /** Hängt die Tafel an eine Fläche: beim Zeigen, per Klick und per Tastatur. */
  bediene: (flaeche: L.Polygon, name: string | undefined, inhalt: () => HTMLElement) => void;
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
    // Ein gestrichelter Lauf bleibt ungeschnitten, auch ein verdeckter; schnitte Leaflet ihn am Rand des Renderers, verschöben sich seine Striche.
    const linie = L.polyline(z.punkte.slice(von, i).map(latLng), { ...(verdeckt ? verdecktStil : stil), noClip: verdeckt || r.strich !== undefined });
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

/** Ob eine Form eine Fläche zeichnet: mit Füllung, oder als Ziel für Namen und Tafel. */
export const hatFlaeche = (f: Form): boolean => !f.linie && (f.fuellung !== undefined || f.name !== undefined || f.panel !== undefined);

/** Das Rechteck der Welt um eine Form. */
export function bereich(f: Form): Rechteck {
  if (f.kreis) {
    const { mitte, radius } = f.kreis;
    return [mitte[0] - radius, mitte[1] - radius, mitte[0] + radius, mitte[1] + radius];
  }
  return rechteck(f.linie ?? f.polygone.flatMap((q) => q.aussen));
}

/** Die Polygone einer Region oder eines Kreises; ein Kreis ist ein Vieleck mit Seiten von höchstens `c` Blöcken. */
export const polygone = (f: Form, c: number): Polygon[] => (f.kreis ? [{ aussen: vieleck(f.kreis.mitte, f.kreis.radius, c), loecher: [] }] : f.polygone);

/** Die Linienzüge eines Rands oder einer Linie, in `area` beschnitten; jedes Stück offen. */
export function zuege(f: Form, c: number, area: Rechteck | undefined): Punkt[][] {
  const ganz: [Punkt[], boolean][] = f.linie ? [[f.linie, false]] : polygone(f, c).flatMap((p) => [p.aussen, ...p.loecher].map((r): [Punkt[], boolean] => [r, true]));
  return ganz.flatMap(([punkte, geschlossen]) =>
    area ? zugImRechteck(punkte, geschlossen, area) : [geschlossen ? [...punkte, punkte[0]!] : punkte],
  );
}

/**
 * Zeichnet die Formen einer Ebene: erst alle Flächen, dann alle Ränder und
 * Linien, so liegt kein Rand unter einer Fläche derselben Ebene. Eine Fläche
 * mit Namen oder Tafel ist ein Ziel, auch ohne Füllung. Was ausserhalb von
 * `area` liegt, fällt vorher weg.
 */
export function zeichne(formen: readonly Form[], z: Zeichnen): { flaechen: L.Layer[]; striche: Strich[] } {
  // Ausgepackt, und keine Closure greift auf z oder das Gelände: Sonst hielte
  // die Tafel einer Fläche die Höhen am Leben.
  const { renderer, blick, gelaende: g, area, tafel, bediene } = z;
  const flaechen: L.Layer[] = [];
  const striche: Strich[] = [];
  /** Je Farbe die Ringe der Wände, je Band von unten nach oben. */
  const waende = new Map<string, Punkt[][][]>();
  for (const f of formen) {
    if (hatFlaeche(f)) {
      const beschnitten = polygone(f, g.c).map((p) => {
        const im = (r: Punkt[]) => (area ? ringImRechteck(r, area) : r);
        return { aussen: im(p.aussen), loecher: p.loecher.map(im) };
      });
      const ringe = netz(beschnitten, g, blick);
      // Ganz verdeckt oder ausserhalb von area: keine Fläche, also auch kein Ziel.
      if (ringe.length) {
        // Ohne Vereinfachen durch Leaflet: Es nähme jeden Ring für sich, gemeinsame Kanten liefen auseinander.
        const flaeche = L.polygon(ringe.map((r) => r.map(latLng)), {
          renderer,
          stroke: false,
          fillColor: f.fuellung ?? '#000000',
          fillOpacity: f.fuellung ? 1 : 0,
          interactive: f.name !== undefined || f.panel !== undefined,
          smoothFactor: 0,
        });
        if (f.name) {
          const name = document.createElement('span');
          name.textContent = f.name;
          flaeche.bindTooltip(name, { sticky: true, className: 'ebene-name' });
        }
        const panel = f.panel;
        if (panel) {
          bediene(flaeche, f.name, () => tafel(panel));
        }
        flaechen.push(flaeche);
      }
    }
    if (f.rand.breite === 0) continue;
    for (const stueck of zuege(f, g.c, area)) {
      const z = zug(stueck, false, g, blick);
      striche.push(...laeufe(z, f.rand, renderer));
      // Die Wand nur im iso und nur am Rand einer Fläche, nicht an einer Linie.
      if (blick.p.y === 0 || f.linie) continue;
      const farbeWand = f.rand.farbe.slice(0, 7);
      const baender = waende.get(farbeWand) ?? Array.from({ length: WAND.baender }, (): Punkt[][] => []);
      waende.set(farbeWand, baender);
      wand(z, WAND.hoehe * blick.p.y, WAND.baender).forEach((ringe, k) => baender[k]!.push(...ringe));
    }
  }
  // Die Wände über den Flächen der Ebene, je Farbe und Band ein Pfad.
  for (const [farbeWand, baender] of waende) {
    baender.forEach((ringe, k) => {
      if (!ringe.length) return;
      const deckkraft = WAND.unten * (1 - (k + 0.5) / WAND.baender);
      flaechen.push(L.polygon(ringe.map((r) => [r.map(latLng)]), { renderer, stroke: false, fillColor: farbeWand, fillOpacity: deckkraft, fillRule: 'nonzero', interactive: false, smoothFactor: 0 }));
    });
  }
  return { flaechen, striche };
}
