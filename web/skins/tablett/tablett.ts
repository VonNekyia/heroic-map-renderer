/**
 * Rahmen und Tisch um die Karte: Die Welt liegt in einem Tablett, dessen
 * Oberkante auf dem Wasserspiegel liegt, und das Tablett auf einem Tisch.
 * Rahmen und Tisch sind ebene Rechtecke mit Bildern aus der Vorlage; in der
 * Parallelprojektion geht jedes affin aufs Bild. Lilien und Gegenstände
 * stehen als freigestellte Bilder darauf. Ohne Leaflet, damit die Tests es
 * in Node laden. Siehe docs/tablett.md.
 */
import type { Grenzen, Kontext, Rechteck } from 'heroic-map-renderer/skin-api';
import {
  BREITE_VORLAGE,
  DIFFUS,
  type Ecke,
  ECKSTUECKE,
  einheit,
  GEGENSTAENDE,
  LILIEN,
  lichtUndBlick,
  mal,
  MASS,
  PFEILER,
  plus,
  RAND,
  type Richtung,
  SEITEN,
  TISCH,
  UMGEBUNG,
  type Vektor,
} from './bilder';

type Punkt = [number, number];

/** Der Blick, aus dem das Tablett gezeichnet wird, so wie ihn die Grundkarte reicht. */
export type Blick = Pick<Kontext, 'projektion' | 'k' | 'projiziere'>;

export type Art = 'tisch' | 'boden' | 'rand' | 'wand' | 'innen' | 'pfeiler' | 'eck';

/** Ein ebenes Rechteck im Bild: Ecke `o`, Kanten `a` und `b`, in Pixeln der feinsten Stufe. */
export interface Flaeche {
  form: 'flaeche';
  o: Punkt;
  a: Punkt;
  b: Punkt;
  /** Die äussere Normale im Blick, Länge 1. */
  n: Vektor;
  art: Art;
  /** CSS-Farbe, schon im Licht: ohne Bild oder solange es fehlt. */
  farbe: string;
  /** Liegt vor den Kacheln. Siehe docs/tablett.md, „Vor und hinter der Welt“. */
  nah: boolean;
  /** Das Bild aus bilder/, das die Fläche deckt: seine Breite entlang a, seine Höhe entlang b. */
  bild?: string;
  /**
   * Nur in diesen Vielecken im Bild, auf Grund: eine Kopie vor den Kacheln,
   * die das ferne Bild nicht braucht.
   */
  nurIn?: Punkt[][];
}

/** Ein freigestelltes Bild, aufrecht: sein Punkt `anker` liegt auf `fuss`. */
export interface Figur {
  form: 'figur';
  bild: string;
  /** In Pixeln der feinsten Stufe. */
  fuss: Punkt;
  /** In Pixeln des Bilds, wie seine Grösse. */
  anker: Punkt;
  groesse: Punkt;
  /** Pixel der feinsten Stufe je Pixel des Bilds. */
  mass: number;
  nah: boolean;
}

/**
 * Der schmale Schatten der Oberkante auf der Karte, an einer Seite zum
 * Licht: von `deckkraft` an der Kante `a` bis 0 bei `b`. Nur vor den Kacheln.
 */
export interface Saum {
  form: 'saum';
  o: Punkt;
  a: Punkt;
  b: Punkt;
  deckkraft: number;
  nah: true;
}

export type Teil = Flaeche | Figur | Saum;

/** Das Rechteck der Welt im Blick mit k Vierteldrehungen; Punkte drehen sich mit (z, −x). */
export function imBlick([x0, z0, x1, z1]: Rechteck, k: number): Rechteck {
  for (let i = 0; i < k; i++) [x0, z0, x1, z1] = [z0, -x1, z1, -x0];
  return [x0, z0, x1, z1];
}

/** Grundfarben der Flächen ohne Bild, bevor das Licht sie trifft. */
const FARBE = {
  rand: '#8a542c',
  wand: '#4e2c18',
  pfeiler: '#6e4224',
  marmor: '#1e2620',
  boden: '#22180f',
};

/** Hinter dem Tisch: dunkel. */
export const GRUND = '#0c0907';

const skalar = (u: Vektor, v: Vektor) => u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
const kreuz = (u: Vektor, v: Vektor): Vektor => [
  u[1] * v[2] - u[2] * v[1],
  u[2] * v[0] - u[0] * v[2],
  u[0] * v[1] - u[1] * v[0],
];

/** Die Richtung einer waagrechten Achse. */
const richtung = ([x, , z]: Vektor): Richtung => (x > 0.5 ? '+x' : x < -0.5 ? '-x' : z > 0.5 ? '+z' : '-z');

/**
 * Die Masse im Blick, in Blöcken: die Kanten der Welt, der Rand w, die Tiefe
 * D vom Wasserspiegel bis zur Platte und die Breite der Pfeiler. Siehe
 * docs/tablett.md, „Masse“.
 */
function masse(area: Rechteck, k: number) {
  const [x0, z0, x1, z1] = imBlick(area, k);
  const kante = (x1 - x0 + (z1 - z0)) / 2;
  const w = RAND * kante;
  return { x0, z0, x1, z1, kante, w, D: MASS.tiefe * w, pfeiler: MASS.pfeiler * w };
}

/**
 * Pixel der feinsten Stufe je Pixel der Vorlage: die Breite der Karte im
 * Bild zu ihrer Breite in der Vorlage. So gross stehen Lilien und
 * Gegenstände.
 */
function vorlageMass([x0, z0, x1, z1]: Rechteck, meer: number, projiziere: Blick['projiziere']): number {
  const xs = [
    [x0, z0],
    [x1, z0],
    [x1, z1],
    [x0, z1],
  ].map(([x, z]) => projiziere(x!, meer, z!)[0]);
  return (Math.max(...xs) - Math.min(...xs)) / BREITE_VORLAGE;
}

/**
 * Die Lilie einer Ecke: ihr Bild, aufrecht auf der Mitte des Deckels. Alle
 * vier liegen vor den Kacheln, wie die Eckstücke unter ihnen.
 */
function lilie(ecke: Ecke, [cx, cz]: Punkt, [sx, sz]: Punkt, pfeiler: number, meer: number, mass: number, projiziere: Blick['projiziere']): Figur {
  const { bild, fuss, groesse } = LILIEN[ecke];
  return {
    form: 'figur',
    bild,
    fuss: projiziere(cx + (sx * pfeiler) / 2, meer, cz + (sz * pfeiler) / 2),
    anker: fuss,
    groesse,
    mass,
    nah: true,
  };
}

/** Die Ecken der Welt im Blick, mit ihrer Richtung nach aussen und dem Namen ihrer Lilie. */
function ecken(x0: number, z0: number, x1: number, z1: number): { c: Punkt; s: Punkt; ecke: Ecke }[] {
  return [
    { c: [x0, z0], s: [-1, -1], ecke: 'hinten' },
    { c: [x1, z0], s: [1, -1], ecke: 'rechts' },
    { c: [x0, z1], s: [-1, 1], ecke: 'links' },
    { c: [x1, z1], s: [1, 1], ecke: 'vorn' },
  ];
}

/**
 * Die Grenzen des Rahmens im Bild, mit Pfeilern und Lilien. Auf sie passt
 * die Karte die ganze Ansicht ein.
 */
export function grenzen(area: Rechteck, meer: number, { k, projiziere }: Blick): Grenzen {
  const { x0, z0, x1, z1, D, pfeiler } = masse(area, k);
  const punkte = [x0 - pfeiler, x1 + pfeiler].flatMap((x) =>
    [meer - D, meer].flatMap((y) => [z0 - pfeiler, z1 + pfeiler].map((z) => projiziere(x, y, z))),
  );
  const mass = vorlageMass(imBlick(area, k), meer, projiziere);
  for (const { c, s, ecke } of ecken(x0, z0, x1, z1)) {
    const { fuss, anker, groesse } = lilie(ecke, c, s, pfeiler, meer, mass, projiziere);
    const [lx, ly] = [fuss[0] - mass * anker[0], fuss[1] - mass * anker[1]];
    punkte.push([lx, ly], [lx + mass * groesse[0], ly + mass * groesse[1]]);
  }
  const xs = punkte.map((e) => e[0]);
  const ys = punkte.map((e) => e[1]);
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/**
 * So viel des Fensters füllt der Rahmen in der Gesamtansicht, wie in der
 * Vorlage. Siehe docs/entscheidungen/0070-bilder-aus-der-vorlage.md.
 */
const FUELLUNG = 0.925;

/**
 * Die Stufe der Gesamtansicht für ein Fenster von `breite` × `hoehe`: die
 * Stufe, auf der die Grenzen `FUELLUNG` des Fensters füllen, oder die
 * nächste, auf der Leaflet die Kacheln nicht vergrössert. Nie tiefer als
 * die ganze Stufe, auf die Leaflet die Grenzen einpasst.
 * Siehe docs/entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md.
 */
export function gesamtstufe([links, oben, rechts, unten]: Grenzen, maxZoom: number, breite: number, hoehe: number): number {
  const voll = maxZoom + Math.log2(Math.min(breite / (rechts - links), hoehe / (unten - oben)));
  const ziel = voll + Math.log2(FUELLUNG);
  // So rundet Leaflet in `getBoundsZoom`, bevor es abrundet.
  const ganz = Math.floor(Math.round(voll * 100) / 100);
  // Leaflet nimmt die Kacheln der gerundeten Stufe: Ab einem Bruch von 0,5
  // verkleinert es die der Stufe darüber, darunter vergrösserte es die der
  // Stufe darunter. Über der feinsten Stufe vergrösserte es immer.
  const erlaubt = (z: number) => Number.isInteger(z) || (z - Math.floor(z) >= 0.5 && z < maxZoom);
  const stufen = [ziel, ganz + 0.5, ganz].filter((z) => z >= ganz && z <= Math.max(voll, ganz) && erlaubt(z));
  return stufen.reduce((a, b) => (Math.abs(b - ziel) < Math.abs(a - ziel) ? b : a));
}

/**
 * Wo die Mitte des Fensters in der Vorlage liegt, von der Mitte der Karte
 * aus, als Anteil ihrer Breite: ein Stück tiefer, denn vor dem Tablett liegt
 * mehr Tisch als dahinter.
 */
const BLICKPUNKT: Punkt = [-0.0033, 0.058];

/**
 * Die Mitte der Gesamtansicht in Pixeln der feinsten Stufe für ein Fenster,
 * das `breite` × `hoehe` Pixel der feinsten Stufe zeigt: wie in der Vorlage
 * unter der Mitte der Karte, aber nie so, dass der Rahmen aus dem Fenster
 * ragt.
 */
export function gesamtmitte(area: Rechteck, meer: number, blick: Blick, breite: number, hoehe: number): Punkt {
  const [x0, z0, x1, z1] = imBlick(area, blick.k);
  const [mx, my] = blick.projiziere((x0 + x1) / 2, meer, (z0 + z1) / 2);
  const karte = vorlageMass([x0, z0, x1, z1], meer, blick.projiziere) * BREITE_VORLAGE;
  const [links, oben, rechts, unten] = grenzen(area, meer, blick);
  const halte = (wert: number, von: number, bis: number, halb: number) =>
    bis - von > 2 * halb ? (von + bis) / 2 : Math.min(Math.max(wert, bis - halb), von + halb);
  return [
    halte(mx + BLICKPUNKT[0] * karte, links, rechts, breite / 2),
    halte(my + BLICKPUNKT[1] * karte, oben, unten, hoehe / 2),
  ];
}

/**
 * Tisch, Tablett, Saum, Eckstücke, Lilien und Gegenstände im Blick mit k
 * Vierteldrehungen, in der Reihenfolge, in der sie gemalt werden: Ein
 * späteres Teil deckt ein früheres. `meer` ist `seaLevel`, `minY` die
 * Unterkante der Welt: So weit reicht ihr Schnitt, den der Tisch vor ihr
 * deckt.
 */
export function tablett(area: Rechteck, meer: number, minY: number, { projektion: p, k, projiziere }: Blick): Teil[] {
  const { x0, z0, x1, z1, kante, w, D, pfeiler } = masse(area, k);
  const genordet = p.azimuth === 'north';
  const mass = vorlageMass([x0, z0, x1, z1], meer, projiziere);

  // Höhen zählen ab dem Wasserspiegel.
  const bild = ([x, y, z]: Vektor): Punkt => projiziere(x, y + meer, z);
  const kante3 = ([x, y, z]: Vektor): Punkt => projiziere(x, y, z);
  // Zur Kamera: entlang dieser Achse liegt, was weiter vorn ist.
  const { kamera, licht } = lichtUndBlick(p);

  const beleuchte = (farbe: string, n: Vektor): string => {
    const zahl = Number.parseInt(farbe.slice(1), 16);
    const hell = UMGEBUNG + DIFFUS * Math.max(0, skalar(n, licht));
    const kanal = (i: number) => Math.min(255, Math.round(((zahl >> (16 - 8 * i)) & 255) * hell));
    return `rgb(${kanal(0)} ${kanal(1)} ${kanal(2)})`;
  };

  // Nah ist, was Gelände nie verdecken kann. Ein Bildpunkt zeigt Gelände
  // vor einem Punkt nur, wenn dieses entlang der Achse weiter vorn liegt:
  // diagonal bei grösserem x und z, genordet bei grösserem z und gleichem x.
  // Siehe docs/tablett.md, „Vor und hinter der Welt“.
  const istNah = (qx0: number, qx1: number, qz0: number) =>
    qx0 >= x1 || qz0 >= z1 || (genordet && qx1 <= x0);
  const fussNah = (ecken: Vektor[]) =>
    istNah(
      Math.min(...ecken.map((e) => e[0])),
      Math.max(...ecken.map((e) => e[0])),
      Math.min(...ecken.map((e) => e[2])),
    );

  /**
   * Ein Rechteck aus Ecke o und Kanten a und b. `aussen` zeigt grob nach
   * aussen und legt die Normale fest. Was von der Kamera wegzeigt, fällt weg.
   * Mit `name` liegt dieses Bild darauf: seine Breite entlang a, seine Höhe
   * entlang b, ab der Ecke o.
   */
  const rechteck = (o: Vektor, a: Vektor, b: Vektor, aussen: Vektor, art: Art, farbe: string, name?: string): Flaeche[] => {
    let n = einheit(kreuz(a, b));
    if (skalar(n, aussen) < 0) n = mal(-1, n);
    if (skalar(n, kamera) <= 1e-9) return [];
    const nah = fussNah([o, plus(o, a), plus(o, b), plus(plus(o, a), b)]);
    const flaeche: Flaeche = { form: 'flaeche', o: bild(o), a: kante3(a), b: kante3(b), n, art, farbe: beleuchte(farbe, n), nah };
    if (name) flaeche.bild = name;
    return [flaeche];
  };

  // Wie weit eine Seite zur Kamera zeigt: diagonal liegen +x und +z vorn,
  // genordet nur +z.
  const vorne = (nx: number, nz: number) => (genordet ? nz : nx + nz);

  // Der Tisch: ein Bild auf der Platte, so gross, wie die Vorlage den Tisch
  // zeigt; dahinter Grund. Vor den Kacheln liegt noch einmal sein Stück, das
  // Gelände nie verdecken kann, auf Grund: So deckt es den Schnitt der Welt,
  // wie tief er auch reicht. Siehe docs/tablett.md, „Vor und hinter der Welt“.
  const [cx, cz] = [(x0 + x1) / 2, (z0 + z1) / 2];
  const [[s0, s1], [t0, t1]] = [TISCH.s, TISCH.t];
  const ort = (s: number, t: number): Vektor => [cx + ((s + t) / 2) * kante, -D, cz + ((t - s) / 2) * kante];
  const entlangS: Vektor = [((s1 - s0) / 2) * kante, 0, (-(s1 - s0) / 2) * kante];
  const entlangT: Vektor = [((t1 - t0) / 2) * kante, 0, ((t1 - t0) / 2) * kante];
  const tisch = rechteck(ort(s0, t0), entlangS, entlangT, [0, 1, 0], 'tisch', FARBE.marmor, 'tisch').map(
    (f): Flaeche => ({ ...f, nah: false }),
  );
  // Vor den Kacheln: diagonal ab x1 und ab z1, genordet dazu bis x0. So weit
  // hinaus, dass auch der tiefste Schnitt einer kleinen Welt darunter liegt:
  // Ein Block tiefer rückt im Bild so weit wie y/v Blöcke auf der Platte.
  const weit = 3 * kante + (p.v > 0 ? ((meer - minY) * p.y) / p.v : 0);
  const streifen = ([qx0, qx1]: Punkt, [qz0, qz1]: Punkt): Punkt[] =>
    [
      [qx0, qz0],
      [qx1, qz0],
      [qx1, qz1],
      [qx0, qz1],
    ].map(([x, z]) => bild([x!, -D, z!]));
  const vorDerWelt = [
    streifen([x1, x1 + weit], [z0 - weit, z1 + weit]),
    streifen([x0 - weit, x1 + weit], [z1, z1 + weit]),
    ...(genordet ? [streifen([x0 - weit, x0], [z0 - weit, z1 + weit])] : []),
  ];
  const tischNah = tisch.map((f): Flaeche => ({ ...f, nah: true, nurIn: vorDerWelt }));

  // Wo in area keine Welt liegt, zeigt das Tablett seinen Boden.
  const boden = rechteck([x0, -D, z0], [x1 - x0, 0, 0], [0, 0, z1 - z0], [0, 1, 0], 'boden', FARBE.boden);

  // Der Rahmen, je Seite drei Flächen: die Innenseite bis zum Boden, das
  // Band der Oberkante auf dem Wasserspiegel und die Wand von seiner
  // Aussenkante bis zur Platte. Nichts liegt über dem Wasserspiegel ausser
  // den Lilien. Siehe docs/tablett.md, „Masse“.
  const runter: Vektor = [0, -D, 0];
  /**
   * Eine Seite: Anfang an einer Ecke der Welt, Richtung entlang, Länge,
   * Richtung nach aussen. Zuerst die Innenseite: Wo Welt liegt, deckt sie
   * sie; wo keine liegt, schliesst sie das Tablett.
   */
  const seite = (start: Vektor, entlang: Vektor, laenge: number, raus: Vektor): Flaeche[] => {
    const { band, wand } = SEITEN[richtung(raus)];
    const a = mal(laenge, entlang);
    return [
      ...rechteck(start, a, runter, mal(-1, raus), 'innen', FARBE.wand),
      ...rechteck(start, a, mal(w, raus), [0, 1, 0], 'rand', FARBE.rand, band),
      ...rechteck(plus(start, mal(w, raus)), a, runter, raus, 'wand', FARBE.wand, wand),
    ];
  };
  /** Ein Pfeiler an der Ecke (cx, cz), nach (sx, sz) hinaus: erst die Seiten, dann der Deckel. */
  const pfosten = ([px, pz]: Punkt, [sx, sz]: Punkt): Flaeche[] => {
    const [qx0, qx1] = sx < 0 ? [px - pfeiler, px] : [px, px + pfeiler];
    const [qz0, qz1] = sz < 0 ? [pz - pfeiler, pz] : [pz, pz + pfeiler];
    const flanke = (o: Vektor, entlang: Vektor, raus: Vektor) =>
      rechteck(o, mal(pfeiler, entlang), runter, raus, 'pfeiler', FARBE.pfeiler, PFEILER[richtung(raus)]);
    return [
      ...flanke([qx0, 0, qz1], [1, 0, 0], [0, 0, 1]),
      ...flanke([qx0, 0, qz0], [1, 0, 0], [0, 0, -1]),
      ...flanke([qx1, 0, qz0], [0, 0, 1], [1, 0, 0]),
      ...flanke([qx0, 0, qz0], [0, 0, 1], [-1, 0, 0]),
      ...rechteck([qx0, 0, qz0], [pfeiler, 0, 0], [0, 0, pfeiler], [0, 1, 0], 'pfeiler', FARBE.pfeiler),
    ];
  };
  // Die Reihenfolge nach `vorne`, die Ecken mit der Summe ihrer Seiten:
  // ferne Ecke, ferne Seiten, seitliche Ecken, nahe Seiten, nahe Ecke. So
  // deckt das Nähere das Fernere. Eckstücke und Lilien kommen danach.
  const rund = ecken(x0, z0, x1, z1);
  const rahmen = [
    { schluessel: vorne(-1, 0), teile: seite([x0, 0, z0], [0, 0, 1], z1 - z0, [-1, 0, 0]) },
    { schluessel: vorne(1, 0), teile: seite([x1, 0, z0], [0, 0, 1], z1 - z0, [1, 0, 0]) },
    { schluessel: vorne(0, -1), teile: seite([x0, 0, z0], [1, 0, 0], x1 - x0, [0, 0, -1]) },
    { schluessel: vorne(0, 1), teile: seite([x0, 0, z1], [1, 0, 0], x1 - x0, [0, 0, 1]) },
    ...rund.map(({ c, s }) => ({ schluessel: 1.5 * (vorne(s[0], 0) + vorne(0, s[1])), teile: pfosten(c, s) })),
  ]
    .sort((a, b) => a.schluessel - b.schluessel)
    .flatMap((r) => r.teile);
  // Innen ist der Rahmen an den Ecken rund: Je Ecke deckt ein Eckstück auf
  // dem Wasserspiegel die Ecke der Welt, vor den Kacheln. Ohne sein Bild
  // bleibt die Ecke offen. Siehe docs/tablett.md, „Die Ecken“.
  const eck = MASS.eck * w;
  const eckstuecke = rund.flatMap(({ c: [px, pz], s: [sx, sz], ecke }) =>
    rechteck([sx > 0 ? px - eck : px, 0, sz > 0 ? pz - eck : pz], [eck, 0, 0], [0, 0, eck], [0, 1, 0], 'eck', FARBE.rand, ECKSTUECKE[ecke]).map(
      (f): Flaeche => ({ ...f, nah: true, farbe: 'transparent' }),
    ),
  );
  const lilien: Figur[] = rund
    .map(({ c, s, ecke }) => ({ c, s, ecke, schluessel: vorne(s[0], 0) + vorne(0, s[1]) }))
    .sort((a, b) => a.schluessel - b.schluessel)
    .map(({ c, s, ecke }) => lilie(ecke, c, s, pfeiler, meer, mass, projiziere));

  // Der Saum: an den Seiten, über die das Licht auf die Karte fällt.
  const lichtWaagrecht = einheit([licht[0], 0, licht[2]]);
  const saeume: Saum[] = (
    [
      [[x0, 0, z0], [0, 0, z1 - z0], [1, 0, 0]],
      [[x1, 0, z0], [0, 0, z1 - z0], [-1, 0, 0]],
      [[x0, 0, z0], [x1 - x0, 0, 0], [0, 0, 1]],
      [[x0, 0, z1], [x1 - x0, 0, 0], [0, 0, -1]],
    ] as [Vektor, Vektor, Vektor][]
  ).flatMap(([o, a, hinein]) => {
    const staerke = -skalar(hinein, lichtWaagrecht);
    if (staerke < 0.25) return [];
    return [{ form: 'saum', o: bild(o), a: kante3(a), b: kante3(mal(0.5 * w * staerke, hinein)), deckkraft: 0.4 * staerke, nah: true }];
  });

  // Die Gegenstände stehen aufrecht auf ihrem Fuss auf der Platte, nach
  // Tiefe: die fernen vor dem Rahmen, die nahen zuletzt.
  const tiefe = (x: number, z: number) => (genordet ? z : x + z);
  const dinge = GEGENSTAENDE.map(({ bild: name, fuss, groesse, ort: [ox, oz] }) => {
    const [x, z] = [cx + ox * kante, cz + oz * kante];
    const figur: Figur = { form: 'figur', bild: name, fuss: bild([x, -D, z]), anker: fuss, groesse, mass, nah: istNah(x, x, z) };
    return { figur, tiefe: tiefe(x, z) };
  })
    .sort((a, b) => a.tiefe - b.tiefe)
    .map(({ figur }) => figur);

  return [
    ...tisch,
    ...tischNah,
    ...boden,
    ...dinge.filter((d) => !d.nah),
    ...rahmen,
    ...saeume,
    ...eckstuecke,
    ...lilien,
    ...dinge.filter((d) => d.nah),
  ];
}
