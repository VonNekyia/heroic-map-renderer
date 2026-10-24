/**
 * Regionen, Kreise und Linien einer Ebene als Pfade von Leaflet, auf dem
 * Gelände. Das Format steht in docs/benutzung/ebenen.md, „Region“, „Kreis“
 * und „Linie“; wie die Karte sie zeichnet in docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import {
  bildpunkt,
  kreisNachSchirm,
  kreisRing,
  netz,
  oberflaeche,
  rechteck,
  ringImRechteck,
  wand,
  weltRechteck,
  zug,
  zugImRechteck,
  type Blick,
  type Gelaende,
  type Polygon,
  type Punkt,
  type Rechteck,
  type Schirm,
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
 * nach oben linear bis 0, gewählt in 0104; höchstens so viele Bänder, wie
 * viele nach 0111 `wandBaender` gibt.
 */
const WAND = { hoehe: 6, unten: 0.6, baender: 12 };

/** So viele ms rechnet `zeichneAnsicht` am Stück, dann gibt sie den Hauptfaden frei. */
const STUECK_MS = 8;

/** Wie hoch die Wand auf der feinsten Stufe ist, in Pixeln. */
export const wandHoehe = (blick: Blick): number => WAND.hoehe * blick.p.y;

/**
 * Wie viele Bänder die Wand hat, wenn sie auf dem Schirm `hoch` Pixel hoch
 * ist: je Band mindestens 8 px, höchstens 12, unter 1 px keine. Gewählt vom
 * User am 20.10. in 0111.
 */
export const wandBaender = (hoch: number): number => (hoch < 1 ? 0 : Math.max(1, Math.min(WAND.baender, Math.floor(hoch / 8))));


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
  /** Die Höhen der Formen im Ausschnitt, je Ansicht geladen. */
  gelaende: Gelaende;
  /**
   * Ob die Ebene ohne Gelände auf `seaLevel` liegt, ohne Höhen oder über der
   * Grenze der Webkarte. Dann ist nichts verdeckt, und es gibt nichts
   * abzutasten: Flächen und Ränder liegen als Ringe da, und es gibt keine
   * Wand. Siehe docs/frontend.md, „Ebenen“.
   */
  eben: boolean;
  /** `area` aus map.json: Ausserhalb gibt es weder Kacheln noch Höhen. */
  area?: Rechteck;
  /** Die Tafel aus ihren Bausteinen, für das Objekt mit dieser `id`. */
  tafel: (bausteine: unknown[], objekt: string) => HTMLElement;
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
    // Schon auf ε vereinfacht, siehe docs/entscheidungen/0111-geometrie-nach-bildschirmfehler.md; Leaflet vereinfachte sonst noch einmal um 1 px.
    smoothFactor: 0,
  };
  // Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
  const verdecktStil: L.PolylineOptions = { ...stil, weight: Math.max(1, r.breite / 2), opacity: 0.4, dashArray: '3 4', lineCap: 'butt' };
  const aus: Strich[] = [];
  let weg = 0;
  for (let i = 1; i < z.punkte.length; ) {
    // Die Linie selbst bleibt hinter Häusern voll: `gestrichelt`, nicht `verdeckt`.
    const [von, verdeckt, versatz] = [i - 1, z.gestrichelt[i], weg];
    for (; i < z.punkte.length && z.gestrichelt[i] === verdeckt; i++) {
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

/** Die Polygone einer Region oder eines Kreises; ein Kreis ist `kreis`, sonst ein Vieleck nach `kreisRing`. */
export const polygone = (f: Form, c: number, eben = false, kreis?: Punkt[]): Polygon[] =>
  f.kreis ? [{ aussen: kreis ?? kreisRing(f.kreis.mitte, f.kreis.radius, c, eben), loecher: [] }] : f.polygone;

/** Die Linienzüge eines Rands oder einer Linie, in `area` beschnitten; jedes Stück offen. */
export function zuege(f: Form, c: number, area: Rechteck | undefined, eben = false, kreis?: Punkt[]): Punkt[][] {
  const ganz: [Punkt[], boolean][] = f.linie ? [[f.linie, false]] : polygone(f, c, eben, kreis).flatMap((p) => [p.aussen, ...p.loecher].map((r): [Punkt[], boolean] => [r, true]));
  return ganz.flatMap(([punkte, geschlossen]) =>
    area ? zugImRechteck(punkte, geschlossen, area) : [geschlossen ? [...punkte, punkte[0]!] : punkte],
  );
}

/**
 * Die Flächen einer Ebene als Ziele, einmal je `version`, noch ohne Ringe:
 * Die setzt `zeichneAnsicht` je Ansicht, so bleiben Tooltip, Tafel und
 * Fokus beim Neurechnen. Eine Fläche mit Namen oder Tafel ist ein Ziel, auch
 * ohne Füllung. Gibt je Fläche den Index ihrer Form.
 */
export function flaechenZiele(formen: readonly Form[], z: Pick<Zeichnen, 'renderer' | 'blick' | 'tafel' | 'bediene'>): { flaeche: L.Polygon; index: number }[] {
  const { renderer, blick, tafel, bediene } = z;
  const ziele: { flaeche: L.Polygon; index: number }[] = [];
  formen.forEach((f, index) => {
    if (!hatFlaeche(f)) return;
    // Im iso füllt die Fläche nicht, sie ist nur Ziel für Name und Tafel; siehe ebenen.md, „Flächen“.
    const fuellung = f.fuellung;
    // Ohne Vereinfachen durch Leaflet: Es nähme jeden Ring für sich, gemeinsame Kanten liefen auseinander.
    const flaeche = L.polygon([], {
      renderer,
      stroke: false,
      fillColor: fuellung ?? '#000000',
      fillOpacity: fuellung && blick.p.y === 0 ? 1 : 0,
      interactive: f.name !== undefined || f.panel !== undefined,
      smoothFactor: 0,
    });
    // Die Füllung, wie die Ebene sie nennt, auch wo die Fläche sie im iso nicht zeichnet.
    if (fuellung) flaeche.on('add', () => flaeche.getElement()?.setAttribute('data-fill', fuellung));
    if (f.name) {
      const name = document.createElement('span');
      name.textContent = f.name;
      flaeche.bindTooltip(name, { sticky: true, className: 'ebene-name' });
      // Leaflet hängt für den Tooltip focus und blur an den Pfad; damit macht Chromium ihn per Klick fokussierbar,
      // zeichnet seinen Fokusring um die Grenzen und nimmt der Karte die Tastatur. Ohne Tafel ist die Fläche kein
      // Ziel für Tab, also weg damit, nach dem add von bindTooltip. Mit zwei Argumenten nimmt Leaflet 1.9 jeden
      // Listener dieser Arten; die Typen kennen die Form nicht. Siehe docs/frontend.md, „Ebenen“.
      const ohne = L.DomEvent.off as unknown as (el: Element, arten: string) => void;
      if (!f.panel) flaeche.on('add', () => ohne(flaeche.getElement()!, 'focus blur'));
    }
    const panel = f.panel;
    if (panel) {
      const id = f.id;
      bediene(flaeche, f.name, () => tafel(panel, id));
    }
    ziele.push({ flaeche, index });
  });
  return ziele;
}

/**
 * Rechnet die Ringe der Flächen und zeichnet Ränder, Linien und Wände einer
 * Ebene nach 0111: so fein, wie der Fehler auf dem Schirm es verlangt, nur
 * im Ausschnitt, die Kreise nach ihrem Radius auf dem Schirm, die Flächen
 * als `netz` mit `schirm` und die Wand mit Bändern nach `wandBaender`.
 * `flaechen` hat je Form ihre Ringe, leer, wenn ganz verdeckt oder
 * ausserhalb; `punkte` zählt alle Punkte. Siehe
 * docs/entscheidungen/0111-geometrie-nach-bildschirmfehler.md.
 */
export async function zeichneAnsicht(
  formen: readonly Form[],
  z: Omit<Zeichnen, 'tafel' | 'bediene'>,
  schirm: Schirm,
  gilt: () => boolean = () => true,
): Promise<{ flaechen: L.LatLng[][][]; waende: L.Layer[]; striche: Strich[]; punkte: number } | undefined> {
  const { renderer, blick, gelaende: g, area, eben } = z;
  const flach = ([x, zz]: Punkt): Punkt => bildpunkt(x, oberflaeche(g, x, zz), zz, blick);
  const wandLagen: L.Layer[] = [];
  const striche: Strich[] = [];
  const flaechen: L.LatLng[][][] = formen.map(() => []);
  let punkte = 0;
  // Von oben ist die Wand 0 px hoch, hat also keine Bänder.
  const baenderZahl = wandBaender(wandHoehe(blick) * schirm.faktor);
  // Was im Ausschnitt liegen kann, bei jeder Höhe des Geländes; eine Form ganz ausserhalb gibt keinen Punkt.
  const welt = weltRechteck(schirm.rahmen, blick, g.grund - 128, g.max);
  /** Je Farbe die Ringe der Wände, je Band von unten nach oben. */
  const waende = new Map<string, Punkt[][][]>();
  let seit = performance.now();
  for (const [index, f] of formen.entries()) {
    // Nach `STUECK_MS` gibt die Rechnung den Hauptfaden frei, so kommen Bilder und Eingaben durch; siehe docs/entscheidungen/0123-formen-im-iso-gemerkt-je-stufe.md.
    if (performance.now() - seit > STUECK_MS) {
      await new Promise((weiter) => setTimeout(weiter));
      // Gilt die Ansicht nicht mehr, etwa nach dem nächsten Verschieben, bricht die Rechnung ab.
      if (!gilt()) return undefined;
      seit = performance.now();
    }
    const [x0, z0, x1, z1] = bereich(f);
    if (x1 < welt[0] || x0 > welt[2] || z1 < welt[1] || z0 > welt[3]) continue;
    const kreis = f.kreis ? kreisNachSchirm(f.kreis.mitte, f.kreis.radius, blick, schirm) : undefined;
    if (hatFlaeche(f)) {
      const beschnitten = polygone(f, g.c, eben, kreis).map((p) => {
        const im = (r: Punkt[]) => (area ? ringImRechteck(r, area) : r);
        return { aussen: im(p.aussen), loecher: p.loecher.map(im) };
      });
      const ringe =
        eben && blick.p.y > 0
          ? beschnitten.flatMap((p) => [p.aussen, ...p.loecher]).filter((r) => r.length >= 3).map((r) => r.map(flach))
          : netz(beschnitten, g, blick, schirm);
      for (const r of ringe) punkte += r.length;
      flaechen[index] = ringe.map((r) => r.map(latLng));
    }
    if (f.rand.breite === 0) continue;
    for (const stueck of zuege(f, g.c, area, eben, kreis)) {
      const nie = stueck.map(() => false);
      const zz = eben && blick.p.y > 0 ? { punkte: stueck.map(flach), verdeckt: nie, gestrichelt: nie } : zug(stueck, false, g, blick, schirm);
      punkte += zz.punkte.length;
      striche.push(...laeufe(zz, f.rand, renderer));
      // Die Wand nur im iso, mit Gelände und nur am Rand einer Fläche, nicht an einer Linie oder einem Kreis ohne Füllung, Namen und Tafel.
      if (eben || !hatFlaeche(f) || baenderZahl === 0) continue;
      const farbeWand = f.rand.farbe.slice(0, 7);
      const baender = waende.get(farbeWand) ?? Array.from({ length: baenderZahl }, (): Punkt[][] => []);
      waende.set(farbeWand, baender);
      wand(zz, WAND.hoehe * blick.p.y, baenderZahl).forEach((ringe, k) => baender[k]!.push(...ringe));
    }
  }
  // Die Wände über den Flächen der Ebene, je Farbe und Band ein Pfad.
  for (const [farbeWand, baender] of waende) {
    baender.forEach((ringe, k) => {
      if (!ringe.length) return;
      punkte += ringe.reduce((summe, r) => summe + r.length, 0);
      const deckkraft = WAND.unten * (1 - (k + 0.5) / baenderZahl);
      wandLagen.push(L.polygon(ringe.map((r) => [r.map(latLng)]), { renderer, stroke: false, fillColor: farbeWand, fillOpacity: deckkraft, fillRule: 'nonzero', interactive: false, smoothFactor: 0 }));
    });
  }
  return { flaechen, waende: wandLagen, striche, punkte };
}
