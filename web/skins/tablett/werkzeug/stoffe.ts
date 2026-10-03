/**
 * Stoffe, Höhenkarten und Licht der Bilder des Tabletts, für das Skript
 * texturen.ts. Jedes Texel nimmt Höhe und Albedo seiner Rolle, das Licht
 * aus der Normalen und dann die Farbe der Rampe mit der nächsten
 * Helligkeit. Die Rampen hat der Researcher an der Vorlage gemessen (#112);
 * kein Pixel stammt aus ihr. Siehe docs/tablett.md, „Texturen“.
 */
import type { Projektion } from 'heroic-map-renderer/skin-api';
import { anschnitt, type Bild, groesse, lichtUndBlick, MASS, type Richtung, stoss, STOSS, type Vektor, WAND } from '../atlas.ts';

export type Rgb = [number, number, number];

/**
 * Ein Stoff: Farben von Schatten bis Glanz, die Helligkeit der Albedo im
 * vollen Licht und das Glanzlicht nach Blinn-Phong.
 */
interface Stoff {
  rampe: string[];
  albedo: number;
  glanz: number;
  exponent: number;
}

/**
 * Holz der Oberkante, auch für Leisten und Pfeiler; Holz der Wand für
 * Fries, Fuge und Sockel; Holz des Tischs für seine Kante; Messing für die
 * Nägel. Die Albedo ist die Helligkeit der Mitte der Rampe, wo die Vorlage
 * sie im vollen Licht zeigt.
 */
export const STOFFE = {
  oberholz: {
    rampe: ['#4c2113', '#673420', '#743f27', '#82472c', '#935532', '#bf864d', '#f9c27f'],
    albedo: 0.094,
    glanz: 0.6,
    exponent: 64,
  },
  wandholz: {
    rampe: ['#140500', '#220e09', '#2e1811', '#402112', '#572f1b', '#643a20', '#875d31'],
    albedo: 0.094,
    glanz: 0.15,
    exponent: 24,
  },
  tischholz: {
    rampe: ['#2c140b', '#35180e', '#401d10', '#4c2211', '#592a14', '#70391b', '#99562b'],
    albedo: 0.027,
    glanz: 0.25,
    exponent: 20,
  },
  messing: {
    rampe: ['#421f09', '#4c240a', '#5d2f10', '#763c14', '#a46125', '#e49d49', '#ffd78d'],
    albedo: 0.09,
    glanz: 0.72,
    exponent: 12,
  },
} satisfies Record<string, Stoff>;

type Stoffname = keyof typeof STOFFE;

/** Lineare Helligkeit einer sRGB-Farbe, Gewichte nach Rec. 709. */
function helligkeit([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const x = c / 255;
    return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

export const rgb = (hex: string): Rgb => {
  const zahl = Number.parseInt(hex.slice(1), 16);
  return [(zahl >> 16) & 255, (zahl >> 8) & 255, zahl & 255];
};

/**
 * Die Rampe eines Stoffs nach Helligkeit, dazu die Schwellen zwischen zwei
 * Farben: das geometrische Mittel ihrer Helligkeiten, also die Mitte im
 * Logarithmus.
 */
const PALETTEN = Object.fromEntries(
  Object.entries(STOFFE).map(([name, { rampe }]) => {
    const farben = rampe.map(rgb).sort((a, b) => helligkeit(a) - helligkeit(b));
    const schwellen = farben.slice(1).map((f, i) => Math.sqrt(helligkeit(f) * helligkeit(farben[i]!)));
    return [name, { farben, schwellen }];
  }),
) as Record<Stoffname, { farben: Rgb[]; schwellen: number[] }>;

/** Alle Farben der Rampen, die Palette der Atlanten. */
export const HOLZ: Rgb[] = Object.values(STOFFE).flatMap(({ rampe }) => rampe.map(rgb));

/** Die Farbe der Rampe, deren Helligkeit `y` im Logarithmus am nächsten liegt. */
export function farbe(stoff: Stoffname, y: number): Rgb {
  const { farben, schwellen } = PALETTEN[stoff];
  let i = 0;
  while (i < schwellen.length && y > schwellen[i]!) i++;
  return farben[i]!;
}

/** Ein Wert in [0, 1) je ganzem Gitterpunkt, fest für alle Läufe. */
function zufall(x: number, y: number): number {
  let h = Math.imul(x, 374761393) + Math.imul(y, 668265263);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Glattes Wertrauschen in [0, 1]; mit `periode` wiederholt es sich in x nach so vielen Zellen. */
function rauschen(x: number, y: number, periode = 0): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const fx = x - xi;
  const fy = y - yi;
  const u = fx * fx * (3 - 2 * fx);
  const v = fy * fy * (3 - 2 * fy);
  const ring = (i: number) => (periode > 0 ? ((i % periode) + periode) % periode : i);
  const a = zufall(ring(xi), yi);
  const b = zufall(ring(xi + 1), yi);
  const c = zufall(ring(xi), yi + 1);
  const d = zufall(ring(xi + 1), yi + 1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

/** Eine runde Erhebung über −1 < t < 1, 1 in der Mitte. */
const buckel = (t: number) => (Math.abs(t) < 1 ? Math.sqrt(1 - t * t) : 0);

/** Wie fein die Maserung entlang u läuft: Zellen des Rauschens je w. */
const MASERUNG = 0.15;

/**
 * Holzmaserung entlang u, als Faktor der Albedo um 1: wenige, weiche
 * Linien. Mit `periode` in w wiederholt sie sich entlang u.
 */
function maserung(u: number, v: number, periode = 0): number {
  const linie = 0.5 + 0.5 * Math.sin((v * 4 + rauschen(u * MASERUNG, v * 1.5, periode * MASERUNG) * 2) * Math.PI);
  return 0.9 + 0.2 * linie ** 3;
}

/** Das Profil der Oberkante quer zur Seite, von der Welt nach aussen: flach, dann drei Schrägen. */
const PROFIL: [v: number, h: number][] = [
  [0, 0],
  [0.45, 0],
  [0.68, -0.04],
  [0.86, -0.12],
  [1, -0.25],
];

/** Das Profil an v, linear zwischen seinen Punkten. */
function profil(v: number): number {
  const i = PROFIL.findIndex(([pv]) => pv >= v);
  if (i <= 0) return PROFIL[i < 0 ? PROFIL.length - 1 : 0]![1];
  const [[v0, h0], [v1, h1]] = [PROFIL[i - 1]!, PROFIL[i]!];
  return h0 + ((h1 - h0) * (v - v0)) / (v1 - v0);
}

/**
 * Was eine Rolle an einer Stelle zeigt: die Höhe über der Fläche in w,
 * davon das Relief, das Vertieftes abdunkelt, dazu die Albedo als Faktor
 * und den Stoff.
 */
interface Stelle {
  hoehe: number;
  relief: number;
  m: number;
  stoff: Stoffname;
}

/** Ein Wulst über eine Leiste von `von` bis `bis`. */
const wulst = (v: number, von: number, bis: number) => 0.1 + 0.04 * buckel((v - (von + bis) / 2) / ((bis - von) / 2));

/**
 * Die Wand an (u, v), mit der Maserung `m`. Leisten und Sockel stehen vor,
 * die Fuge liegt zurück; der Sprung zwischen zwei Zeilen kippt die Normale,
 * so fängt jede Kante oben Licht und wirft unten Schatten. Im Fries liegen
 * vertiefte Felder im erhabenen Rahmen. `abstand` ist der Abstand zum
 * Stoss zweier Felder in w; dort steht der Rahmen auch quer, mit einem
 * Messingnagel. Ohne ihn läuft das Feld durch.
 */
function wand(v: number, m: number, abstand = Infinity): Stelle {
  if (v < WAND.leiste) return { hoehe: wulst(v, 0, WAND.leiste), relief: 0, m, stoff: 'oberholz' };
  if (v < WAND.fries) {
    // Die Ranken in den Feldern kommen gemalt.
    const [fv, fb] = [v - WAND.leiste, WAND.fries - WAND.leiste];
    const nagel = Math.hypot(abstand, fv - fb / 2) / 0.2;
    if (nagel < 1) return { hoehe: 0.08 + 0.12 * Math.sqrt(1 - nagel * nagel), relief: 0, m: 1, stoff: 'messing' };
    const rand = Math.min(abstand, fv, fb - fv);
    const relief = rand < 0.12 ? 0.08 * (rand / 0.12) : rand < STOSS ? 0.08 : -0.04;
    return { hoehe: relief, relief, m, stoff: 'wandholz' };
  }
  if (v < WAND.leiste2) return { hoehe: wulst(v, WAND.fries, WAND.leiste2), relief: 0, m, stoff: 'oberholz' };
  if (v < WAND.fuge) return { hoehe: -0.1, relief: 0, m: 0.5, stoff: 'wandholz' };
  // Der Sockel mit einer Fase an der oberen Kante.
  return { hoehe: 0.1 + 0.05 * Math.min(1, (v - WAND.fuge) / 0.2), relief: 0, m, stoff: 'wandholz' };
}

/**
 * Was ein Bild an (u, v) in w zeigt; u läuft entlang seiner Breite, also der
 * Kante a der Fläche, v entlang b. Beim Stoss zählt u ab seiner Mitte und v
 * ab dem Wasserspiegel wie an der Wand. Siehe docs/tablett.md, „Texturen“.
 */
function stelle(bild: Bild, u: number, v: number): Stelle {
  const { breite: la, hoehe: lb } = bild;
  switch (bild.rolle) {
    case 'oben': {
      // Die Lippe zur Karte hin, dahinter eine schmale Kehle; nach aussen
      // fallen die Schrägen ab.
      const relief = 0.07 * buckel((v - 0.1) / 0.08) - 0.025 * buckel((v - 0.22) / 0.04);
      return { hoehe: profil(v) + relief, relief, m: maserung(u, v, MASS.periode), stoff: 'oberholz' };
    }
    case 'wand':
      return wand(v, maserung(u, v, MASS.periode));
    case 'stoss':
      return wand(v, maserung(u, v), Math.abs(u));
    case 'pfeiler': {
      // Ein vertieftes Feld je Seite, darin oben ein Messingnagel.
      const nagel = Math.hypot(u - la / 2, v - 0.2 * lb) / 0.22;
      if (nagel < 1) return { hoehe: 0.12 * Math.sqrt(1 - nagel * nagel), relief: 0, m: 1, stoff: 'messing' };
      const rand = Math.min(u, la - u, v, lb - v);
      const relief = rand < 0.25 ? 0.06 * Math.min(1, rand / 0.12) : -0.03;
      return { hoehe: relief, relief, m: maserung(u, v), stoff: 'oberholz' };
    }
    case 'kappe':
      // Eine flache Pyramide.
      return {
        hoehe: 0.25 * (1 - Math.max(Math.abs((2 * u) / la - 1), Math.abs((2 * v) / lb - 1))),
        relief: 0,
        m: maserung(u, v),
        stoff: 'oberholz',
      };
    case 'tischkante': {
      // Quer zur Kante: eine Nut, dann ein runder Abschluss nach aussen.
      const aussen = lb - v;
      const hoehe = -0.06 * buckel((v - 0.5) / 0.12) + (aussen < 0.8 ? 0.25 * (Math.sqrt(1 - (1 - aussen / 0.8) ** 2) - 1) : 0);
      return { hoehe, relief: 0, m: maserung(u, v, la), stoff: 'tischholz' };
    }
  }
}

/** Die Achse einer Richtung. */
const achse = (r: Richtung): Vektor =>
  ({ '+x': [1, 0, 0], '-x': [-1, 0, 0], '+z': [0, 0, 1], '-z': [0, 0, -1] })[r] as Vektor;

/**
 * Normale und Kanten einer Fläche mit diesem Bild, im Blick, wie
 * `tablett` in tablett.ts sie legt: a entlang der Seite, bei Bändern b nach
 * aussen, bei Wänden und Pfeilern b nach unten.
 */
function lage({ rolle, richtung }: Bild): { n: Vektor; t: Vektor; q: Vektor } {
  if (rolle === 'kappe') return { n: [0, 1, 0], t: [1, 0, 0], q: [0, 0, 1] };
  const raus = achse(richtung!);
  const entlang: Vektor = raus[0] !== 0 ? [0, 0, 1] : [1, 0, 0];
  if (rolle === 'oben' || rolle === 'tischkante') return { n: [0, 1, 0], t: entlang, q: raus };
  return { n: raus, t: entlang, q: [0, -1, 0] };
}

/**
 * Aus diesen Kameras wird das Licht eingebacken: diagonal aus 8:5, genordet
 * aus 45°. Siehe docs/tablett.md, „Texturen“.
 */
const KAMERA: Record<Projektion['azimuth'], Projektion> = {
  diagonal: { azimuth: 'diagonal', u: 8, v: 5, y: 8 },
  north: { azimuth: 'north', u: 1, v: 1, y: 1 },
};

/**
 * Wo im Texel das Licht gerechnet wird, als Versatz von seiner Mitte: vier
 * Proben im gedrehten Gitter, gemittelt. Sonst zerfielen Lippe und Kanten
 * der Leisten in Punkte.
 */
const PROBEN = [
  [-0.125, -0.375],
  [0.375, -0.125],
  [0.125, 0.375],
  [-0.375, 0.125],
] as const;

/**
 * Malt ein Bild bei `dichte` Texeln je w, Zeile für Zeile, so gross, wie
 * `atlas` es plant. Je Probe kippt die Ableitung der Höhe die Normale, über
 * ein Texel; dann Umgebung und diffuses Licht, dazu das Glanzlicht nach
 * Blinn-Phong mit dem Vektor zwischen Licht und Blick. Albedo und Stoff
 * kommen aus der Mitte des Texels. Ein Stoss bleibt durchsichtig, wo die
 * Wand ohne ihn mit derselben Maserung dieselbe Farbe hätte: Dort zeigt
 * sich die Wand mit ihrer eigenen. Der Anschnitt wiederholt die Texel der
 * Kante. Siehe docs/tablett.md, „Texturen“.
 */
export function male(bild: Bild, dichte: number): (Rgb | undefined)[] {
  const [breite, hoehe] = groesse(bild, dichte);
  const [au, av] = anschnitt(bild);
  const { n, t, q } = lage(bild);
  const l = lichtUndBlick(KAMERA[bild.azimuth]);
  const d = 1 / dichte;
  // Der Stoss liegt um seine Mitte und in den Zeilen der Wand, die er deckt.
  const { halb: ab, von } = bild.rolle === 'stoss' ? stoss(dichte) : { halb: 0, von: 0 };
  // Bis hierher reicht ein Bild, das sich nicht wiederholt; am Ende wird
  // nach innen abgeleitet.
  const [endeU, endeV] = bild.rolle === 'stoss' ? [Infinity, MASS.tiefe] : [bild.periodisch ? Infinity : bild.breite, bild.hoehe];
  /** Die Farbe des Texels (i, k), wo an jeder Stelle (u, v) `zeigt` liegt. */
  const texel = (zeigt: (u: number, v: number) => Stelle, i: number, k: number): Rgb => {
    let [diffus, relief] = [0, 0];
    const halb: number[] = [];
    for (const [pu, pv] of PROBEN) {
      const [u, v] = [(i - ab + 0.5 + pu) * d, (k + von + 0.5 + pv) * d];
      const hier = zeigt(u, v);
      const su = u + d <= endeU ? d : -d;
      const sv = v + d <= endeV ? d : -d;
      const hu = (zeigt(u + su, v).hoehe - hier.hoehe) / su;
      const hv = (zeigt(u, v + sv).hoehe - hier.hoehe) / sv;
      const normale: Vektor = [n[0] - hu * t[0] - hv * q[0], n[1] - hu * t[1] - hv * q[1], n[2] - hu * t[2] - hv * q[2]];
      const laenge = Math.hypot(...normale);
      const skalar = (w: Vektor) => (normale[0] * w[0] + normale[1] * w[1] + normale[2] * w[2]) / laenge;
      diffus += Math.max(0, skalar(l.licht));
      halb.push(skalar(l.halb));
      relief += hier.relief;
    }
    const mitte = zeigt((i - ab + 0.5) * d, (k + von + 0.5) * d);
    const { albedo, glanz, exponent } = STOFFE[mitte.stoff];
    // Vertieftes liegt im Schatten der Ränder.
    const m = mitte.m * (1 + 3 * Math.min(0, relief / PROBEN.length));
    const glaenzend = halb.reduce((summe, c) => summe + (c > 0 ? c ** exponent : 0), 0);
    const y = albedo * m * (l.umgebung + (l.diffus * diffus) / PROBEN.length) + (glanz * glaenzend) / PROBEN.length;
    return farbe(mitte.stoff, y);
  };
  const zeigt = (u: number, v: number) => stelle(bild, u, v);
  const ohneStoss = (u: number, v: number) => wand(v, maserung(u, v));
  const innen = (j: number, rand: number, laenge: number) => Math.min(laenge - 2 * rand - 1, Math.max(0, j - rand));
  const farben: (Rgb | undefined)[] = [];
  for (let k = 0; k < hoehe; k++) {
    for (let i = 0; i < breite; i++) {
      const [ii, kk] = [innen(i, au, breite), innen(k, av, hoehe)];
      const hier = texel(zeigt, ii, kk);
      const gleich = bild.rolle === 'stoss' && texel(ohneStoss, ii, kk).join() === hier.join();
      farben.push(gleich ? undefined : hier);
    }
  }
  return farben;
}

/** Folge fester Zufallszahlen in [0, 1) aus einem Startwert (Mulberry32). */
function folge(start: number): () => number {
  let a = start >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Der Grund des Marmors, gemessen an der Vorlage, von dunkel nach hell. Die
 * dunklen Farben sind grünlich, die hellen wärmer.
 */
const GRUND = ['#13120c', '#151511', '#191813', '#1e1c15', '#232118', '#2a261b', '#322d20'].map(rgb);

/**
 * Die Adern nach ihrer Breite, mit dem Radius, in dem sie gezogen werden,
 * aus der Rampe der Adern: dünne dunkler, nur die breitesten hell.
 */
export const ADERFARBEN = [
  { ab: 0.25, linie: 0.5, farbe: '#54391c' },
  { ab: 0.5, linie: 0.8, farbe: '#5d4228' },
  { ab: 0.8, linie: 1.1, farbe: '#6e4628' },
  { ab: 1.1, linie: 1.3, farbe: '#705538' },
  { ab: 1.3, linie: 1.6, farbe: '#846749' },
  { ab: 1.55, linie: 1.9, farbe: '#a58260' },
] as const;

/** Die Palette der Kachel Marmor: erst der Grund, dann die Adern. */
export const MARMOR: Rgb[] = [...GRUND, ...ADERFARBEN.map(({ farbe }) => rgb(farbe))];

/** Seitenlänge der Kachel Marmor in Pixeln. */
export const KACHEL = 512;

/**
 * Gewicht des Rauschens je Stufe, von Zellen zu 32 Pixeln bis zu einzelnen.
 * Fast gleich stark: So sind die Flecken meist klein und die Ähnlichkeit
 * benachbarter Pixel fällt wie in der Vorlage schon nach wenigen Pixeln.
 */
const FLECKEN = [1, 1, 1, 1, 0.9, 0.5];

/**
 * Schwellen zwischen den sieben Farben des Grunds, in Standardabweichungen
 * des Rauschens: Jede Farbe nimmt so viel Fläche, wie in der Vorlage ihr am
 * nächsten liegt, eine Spur dunkler. Den Rest hellt der Schleier um die
 * Adern auf.
 */
const SCHWELLEN = [-1.34, -0.755, -0.253, 0.253, 0.755, 1.34].map((s) => s + 0.35);

/**
 * Der Schleier um die Adern: so weit reicht er in Pixeln, so viele
 * Standardabweichungen hellt er den Grund an einer Ader auf.
 */
const SCHLEIER = { breite: 3, weich: 4, staerke: 1.5 };

/** Die Adern sind auf der Platte gedacht und im Bild um v/u von 8:5 gestaucht. */
const STRECKUNG = 5 / 8;

/** Ein Punkt einer Ader in Pixeln, mit ihrer Breite dort. */
export interface Aderpunkt {
  x: number;
  y: number;
  breite: number;
}

/** Ein Knoten einer Ader auf der Platte, mit ihrer Breite dort. */
interface Knoten {
  x: number;
  y: number;
  breite: number;
}

/** Abstand der Punkte einer Ader in Pixeln, höchstens. */
const SCHRITT = 7;

/** Fläche der Platte in Pixeln je Gruppe von Adern. */
const GRUPPE = 45_000;

/** Anteil der Knoten einer Gruppe, die eine Masche schliessen. */
const MASCHEN = 0.35;

/**
 * Die Adern im Marmor über eine Fläche von `breite` × `hoehe`, in Gruppen:
 * Je Gruppe liegen Knoten in einer schmalen, schrägen Ellipse, verbunden zu
 * einem verzweigten Netz mit einzelnen Maschen, zur Mitte der Gruppe
 * breiter, an losen Enden spitz. Lange, feine Risse verbinden die meisten
 * Gruppen mit ihrer nächsten. Jede Verbindung ist im Kleinen zackig, ihre
 * Breite schwankt. Gedacht auf der Platte, in der Höhe um `streckung`
 * zusammengedrückt. Für dieselbe Grösse immer dieselben.
 */
export function adern(breite: number, hoehe: number, streckung: number): Aderpunkt[][] {
  const zufall = folge(112);
  const tief = hoehe / streckung;
  const alle: Aderpunkt[][] = [];
  // Von a nach b, in der Breite von a nach b: Die Mitte jedes Stücks rückt
  // quer, um bis zu `zacken` seiner Länge, bis kein Stück länger als
  // SCHRITT ist.
  const riss = (a: Knoten, b: Knoten, zacken: number): void => {
    const punkte: [number, number][] = [[a.x, a.y]];
    const teile = (px: number, py: number, qx: number, qy: number): void => {
      if (Math.hypot(qx - px, qy - py) <= SCHRITT) {
        punkte.push([qx, qy]);
        return;
      }
      const versatz = (zufall() - 0.5) * zacken;
      const [mx, my] = [(px + qx) / 2 - (qy - py) * versatz, (py + qy) / 2 + (qx - px) * versatz];
      teile(px, py, mx, my);
      teile(mx, my, qx, qy);
    };
    teile(a.x, a.y, b.x, b.y);
    const n = punkte.length - 1;
    // Die Breite schwankt von Punkt zu Punkt: Die Ader wird mal dicker, mal dünner.
    const dicke = (i: number) => Math.min(2, (a.breite + ((b.breite - a.breite) * i) / n) * (0.75 + 0.5 * zufall()));
    alle.push(punkte.map(([x, y], i) => ({ x, y: y * streckung, breite: dicke(i) })));
  };
  // Um 0 gehäuft, in [−1, 1].
  const gehaeuft = () => (zufall() + zufall() + zufall() - 1.5) / 1.5;
  const mitten: Knoten[] = [];
  const zahl = Math.max(2, Math.round((breite * tief) / GRUPPE));
  for (let g = 0; g < zahl; g++) {
    const mitte = { x: zufall() * breite, y: zufall() * tief, breite: 0.3 };
    const winkel = 0.45 + (zufall() - 0.5) * 1.4;
    const [cos, sin] = [Math.cos(winkel), Math.sin(winkel)];
    const lang = 70 + 130 * zufall();
    const quer = lang * (0.15 + 0.2 * zufall());
    const staerke = 1.2 + 0.6 * zufall();
    const knoten: Knoten[] = Array.from({ length: 10 + Math.floor(zufall() * 16) }, () => {
      const [s, r] = [gehaeuft(), gehaeuft()];
      const innen = Math.max(0, 1 - Math.hypot(s, r));
      return {
        x: mitte.x + lang * s * cos - quer * r * sin,
        y: mitte.y + lang * s * sin + quer * r * cos,
        breite: 0.5 + (staerke - 0.5) * innen ** 0.6 * (0.6 + 0.4 * zufall()),
      };
    });
    // Der kürzeste Baum durch alle Knoten (Prim) verzweigt sie wie Adern;
    // manche Knoten schliessen zu ihrem zweitnächsten eine Masche.
    const kanten = new Set<number>();
    const verbinde = (i: number, j: number) => kanten.add(Math.min(i, j) * 64 + Math.max(i, j));
    const weit = (i: number, j: number) => Math.hypot(knoten[i]!.x - knoten[j]!.x, knoten[i]!.y - knoten[j]!.y);
    const naechster = knoten.map(() => 0);
    const frei = new Set(knoten.keys());
    frei.delete(0);
    while (frei.size > 0) {
      const neu = [...frei].reduce((i, j) => (weit(j, naechster[j]!) < weit(i, naechster[i]!) ? j : i));
      verbinde(neu, naechster[neu]!);
      frei.delete(neu);
      for (const i of frei) if (weit(i, neu) < weit(i, naechster[i]!)) naechster[i] = neu;
    }
    knoten.forEach((_, i) => {
      if (zufall() >= MASCHEN) return;
      const [, zweiter] = [...knoten.keys()].filter((j) => j !== i).sort((p, r) => weit(i, p) - weit(i, r));
      if (zweiter !== undefined) verbinde(i, zweiter);
    });
    const grad = knoten.map(() => 0);
    for (const kante of kanten) {
      grad[kante >> 6]!++;
      grad[kante & 63]!++;
    }
    // Lose Enden laufen spitz aus.
    knoten.forEach((k, i) => {
      if (grad[i] === 1) k.breite = 0.2;
    });
    for (const kante of kanten) riss(knoten[kante >> 6]!, knoten[kante & 63]!, 0.55);
    mitten.push(mitte);
  }
  // Manche Gruppen laufen in einem langen, feinen Riss zur nächsten.
  for (const [i, a] of mitten.entries()) {
    if (zufall() < 0.2) continue;
    const b = mitten.filter((_, j) => j !== i).reduce((p, r) => (Math.hypot(r.x - a.x, r.y - a.y) < Math.hypot(p.x - a.x, p.y - a.y) ? r : p));
    riss({ ...a, breite: 0.7 }, { ...b, breite: 0.7 }, 0.45);
  }
  return alle;
}

/** Ein Feld von n × n, ringsum `r` Pixel weit gemittelt, erst waagrecht, dann senkrecht. */
function verwische(feld: Float64Array, n: number, r: number): Float64Array {
  const ring = (i: number) => ((i % n) + n) % n;
  let ein = feld;
  for (const [schritt, quer] of [
    [1, n],
    [n, 1],
  ] as const) {
    const aus = new Float64Array(n * n);
    for (let j = 0; j < n; j++) {
      let summe = 0;
      for (let i = -r; i <= r; i++) summe += ein[j * quer + ring(i) * schritt]!;
      for (let i = 0; i < n; i++) {
        aus[j * quer + i * schritt] = summe / (2 * r + 1);
        summe += ein[j * quer + ring(i + r + 1) * schritt]! - ein[j * quer + ring(i - r) * schritt]!;
      }
    }
    ein = aus;
  }
  return ein;
}

/**
 * Eine Kachel Marmor ohne Naht, `n` Pixel im Quadrat, als Stellen in
 * `MARMOR`. Der Grund ist Rauschen aus einer Pyramide: Jede Stufe ist die
 * vorige, bilinear auf das Doppelte vergrössert, mit eigenem Rauschen dazu.
 * Um die Adern hellt ein weicher Schleier ihn auf; dann wird er ohne
 * Rasterung in die sieben Farben gestuft. Die Adern liegen darüber, ringsum
 * gezogen, jedes Stück in der Farbe seiner Breite. Siehe docs/tablett.md,
 * „Texturen“.
 */
export function marmor(n = KACHEL): Uint8Array {
  const zufall = folge(n);
  let m = n >> (FLECKEN.length - 1);
  let feld = Float64Array.from({ length: m * m }, () => FLECKEN[0]! * (zufall() - 0.5));
  for (const gewicht of FLECKEN.slice(1)) {
    const neu = new Float64Array(4 * m * m);
    for (let y = 0; y < 2 * m; y++) {
      // Jeder neue Punkt liegt ein Viertel neben einem alten, zu dem er
      // drei Viertel zählt; ringsum, damit keine Naht entsteht.
      const y0 = y >> 1;
      const y1 = (y0 + (y & 1 ? 1 : m - 1)) % m;
      for (let x = 0; x < 2 * m; x++) {
        const x0 = x >> 1;
        const x1 = (x0 + (x & 1 ? 1 : m - 1)) % m;
        const oben = 0.75 * feld[y0 * m + x0]! + 0.25 * feld[y0 * m + x1]!;
        const unten = 0.75 * feld[y1 * m + x0]! + 0.25 * feld[y1 * m + x1]!;
        neu[y * 2 * m + x] = 0.75 * oben + 0.25 * unten + gewicht * (zufall() - 0.5);
      }
    }
    [feld, m] = [neu, 2 * m];
  }
  let [summe, quadrate] = [0, 0];
  for (const wert of feld) {
    summe += wert;
    quadrate += wert * wert;
  }
  const mittel = summe / feld.length;
  const sigma = Math.sqrt(quadrate / feld.length - mittel * mittel);

  // Die Adern: Jedes Stück färbt die Pixel, deren Mitte höchstens den
  // halben Strich, mindestens ein halbes Pixel, von ihm liegt.
  const ader = new Int8Array(n * n).fill(-1);
  const ring = (i: number) => ((i % n) + n) % n;
  for (const linie of adern(n, n, STRECKUNG)) {
    for (let i = 1; i < linie.length; i++) {
      const [p, q] = [linie[i - 1]!, linie[i]!];
      // Die Stufen sind nach `ab` sortiert.
      const stufe = ADERFARBEN.filter(({ ab }) => (p.breite + q.breite) / 2 >= ab).length - 1;
      if (stufe < 0) continue;
      const r = Math.max(0.5, ADERFARBEN[stufe]!.linie / 2);
      const [dx, dy] = [q.x - p.x, q.y - p.y];
      const lang2 = dx * dx + dy * dy || 1;
      for (let y = Math.floor(Math.min(p.y, q.y) - r); y <= Math.ceil(Math.max(p.y, q.y) + r); y++) {
        for (let x = Math.floor(Math.min(p.x, q.x) - r); x <= Math.ceil(Math.max(p.x, q.x) + r); x++) {
          const t = Math.min(1, Math.max(0, ((x + 0.5 - p.x) * dx + (y + 0.5 - p.y) * dy) / lang2));
          if (Math.hypot(x + 0.5 - p.x - t * dx, y + 0.5 - p.y - t * dy) > r) continue;
          const ort = ring(y) * n + ring(x);
          ader[ort] = Math.max(ader[ort]!, stufe);
        }
      }
    }
  }

  // Der Schleier: die Adern auf einen Streifen verbreitert, dann weich.
  const breit = verwische(Float64Array.from(ader, (a) => (a >= 0 ? 1 : 0)), n, SCHLEIER.breite).map((a) => (a > 1e-9 ? 1 : 0));
  const schleier = verwische(verwische(verwische(breit, n, SCHLEIER.weich), n, SCHLEIER.weich), n, SCHLEIER.weich);
  const kachel = new Uint8Array(n * n);
  for (let i = 0; i < n * n; i++) {
    if (ader[i]! >= 0) {
      kachel[i] = GRUND.length + ader[i]!;
      continue;
    }
    const z = (feld[i]! - mittel) / sigma + SCHLEIER.staerke * schleier[i]!;
    let stufe = 0;
    while (stufe < SCHWELLEN.length && z > SCHWELLEN[stufe]!) stufe++;
    kachel[i] = stufe;
  }
  return kachel;
}
