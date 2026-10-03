/**
 * Rahmen und Tisch um die Karte: Die Welt liegt in einem Tablett, dessen
 * Oberkante auf dem Wasserspiegel liegt, und das Tablett auf einem Tisch.
 * Alles besteht aus ebenen Rechtecken; in der Parallelprojektion geht jedes
 * affin aufs Bild. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/tablett.md.
 */
import type { Grenzen, Kontext, Projektion, Rechteck } from 'heroic-map-renderer/skin-api';
import type { Rolle } from './stoffe';

type Punkt = [number, number];
export type Vektor = [number, number, number];

/** Der Blick, aus dem das Tablett gezeichnet wird, so wie ihn die Grundkarte reicht. */
export type Blick = Pick<Kontext, 'projektion' | 'k' | 'projiziere'>;

export type Art = 'tisch' | 'zarge' | 'boden' | 'rand' | 'leiste' | 'wand' | 'pfeiler' | 'ding';

/**
 * Die Textur einer Fläche: ihre Rolle, ihre Kanten im Blick in Blöcken und
 * deren Längen in w. Siehe docs/tablett.md, „Texturen“.
 */
export interface Textur {
  rolle: Rolle;
  a3: Vektor;
  b3: Vektor;
  la: number;
  lb: number;
}

/** Ein ebenes Rechteck im Bild: Ecke `o`, Kanten `a` und `b`, in Pixeln der feinsten Stufe. */
export interface Flaeche {
  form: 'flaeche';
  o: Punkt;
  a: Punkt;
  b: Punkt;
  /** Die äussere Normale im Blick, Länge 1. */
  n: Vektor;
  art: Art;
  /** CSS-Farbe, schon im Licht; mit Textur die Farbe, bis sie gerechnet ist. */
  farbe: string;
  /** Liegt vor den Kacheln. Siehe docs/tablett.md, „Vor und hinter der Welt“. */
  nah: boolean;
  textur?: Textur;
  /** Gefüllt mit einem Muster im Bild statt mit `farbe`. */
  muster?: 'marmor';
}

/**
 * Die weichen Schatten auf der Tischplatte, Vielecke im Bild, gefüllt als
 * eines. Vor den Kacheln nur auf den nahen Stücken der Platte.
 */
export interface Schatten {
  form: 'schatten';
  vielecke: Punkt[][];
  /** Die Unschärfe in Pixeln der feinsten Stufe. */
  weich: number;
  deckkraft: number;
  nah: true;
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

export type Teil = Flaeche | Schatten | Saum;

/** Das Rechteck der Welt im Blick mit k Vierteldrehungen; Punkte drehen sich mit (z, −x). */
export function imBlick([x0, z0, x1, z1]: Rechteck, k: number): Rechteck {
  for (let i = 0; i < k; i++) [x0, z0, x1, z1] = [z0, -x1, z1, -x0];
  return [x0, z0, x1, z1];
}

/**
 * Die Breite w der Oberkante als Anteil der Kante der Welt, nach der
 * Vermessung der Vorlage. Siehe docs/tablett.md, „Masse“.
 */
const RAND = 0.013;

/** Grundfarben, bevor das Licht sie trifft. */
const FARBE = {
  rand: '#8a542c',
  leiste: '#784826',
  wand: '#4e2c18',
  fuge: '#140a04',
  pfeiler: '#6e4224',
  marmor: '#1e2620',
  tischkante: '#5c341a',
  zarge: '#462814',
  boden: '#22180f',
  buecher: ['#5a1c18', '#223c26', '#58361e'],
  pergament: '#bea882',
  messing: '#aa823c',
  kerze: '#ebe1c8',
  buch: '#641e1a',
};

/** Hinter dem Tisch: dunkel. */
export const GRUND = '#0c0907';

/**
 * Das Licht nach der Vermessung der Vorlage: von oben, leicht von links im
 * Bild, 77° über der Tischebene, fest im Blick, so dass es aus jeder
 * Richtung gleich aussieht. `oben` zählt entlang der Normalen der Platte,
 * `rechts` nach rechts im Bild. Siehe docs/tablett.md, „Licht und Schatten“.
 */
const LICHT = { oben: 0.975, rechts: -0.223 };
const UMGEBUNG = 0.22;
const DIFFUS = 0.8;

const plus = (u: Vektor, v: Vektor): Vektor => [u[0] + v[0], u[1] + v[1], u[2] + v[2]];
const mal = (s: number, v: Vektor): Vektor => [s * v[0], s * v[1], s * v[2]];
const skalar = (u: Vektor, v: Vektor) => u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
const kreuz = (u: Vektor, v: Vektor): Vektor => [
  u[1] * v[2] - u[2] * v[1],
  u[2] * v[0] - u[0] * v[2],
  u[0] * v[1] - u[1] * v[0],
];
const einheit = (v: Vektor): Vektor => mal(1 / Math.hypot(...v), v);

/**
 * Licht und Blick im Blick: `kamera` zeigt zur Kamera, `halb` liegt zwischen
 * Licht und Kamera, für das Glanzlicht. Siehe docs/tablett.md, „Licht und
 * Schatten“.
 */
export function lichtUndBlick(p: Projektion) {
  const genordet = p.azimuth === 'north';
  const kamera = einheit(genordet ? [0, p.v, p.y] : [p.y, 2 * p.v, p.y]);
  // Nach rechts im Bild, in der Welt waagrecht.
  const rechts: Vektor = genordet ? [1, 0, 0] : [Math.SQRT1_2, 0, -Math.SQRT1_2];
  const licht = plus([0, LICHT.oben, 0], mal(LICHT.rechts, rechts));
  return { kamera, licht, halb: einheit(plus(licht, kamera)), umgebung: UMGEBUNG, diffus: DIFFUS };
}

/** Die konvexe Hülle von Punkten, gegen den Uhrzeigersinn (monotone Kette). */
function huelle(punkte: Punkt[]): Punkt[] {
  const p = [...punkte].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const quer = (o: Punkt, a: Punkt, b: Punkt) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const kette = (liste: Punkt[]) => {
    const k: Punkt[] = [];
    for (const q of liste) {
      while (k.length >= 2 && quer(k[k.length - 2]!, k[k.length - 1]!, q) <= 0) k.pop();
      k.push(q);
    }
    k.pop();
    return k;
  };
  return [...kette(p), ...kette(p.reverse())];
}

/**
 * Die Masse im Blick, alle als Anteil der Welt: die Kanten der Welt, der Rand
 * w, die Tiefe D vom Wasserspiegel bis zur Platte und die Breite der Pfeiler.
 * Siehe docs/tablett.md, „Masse“.
 */
function masse(area: Rechteck, k: number) {
  const [x0, z0, x1, z1] = imBlick(area, k);
  const kante = (x1 - x0 + (z1 - z0)) / 2;
  const w = RAND * kante;
  return { x0, z0, x1, z1, kante, w, D: 6.4 * w, pfeiler: 2.7 * w };
}

/** Die Grenzen des Rahmens im Bild. Auf sie passt die Karte die ganze Ansicht ein. */
export function grenzen(area: Rechteck, meer: number, { k, projiziere }: Blick): Grenzen {
  const { x0, z0, x1, z1, D, pfeiler } = masse(area, k);
  const ecken = [x0 - pfeiler, x1 + pfeiler].flatMap((x) =>
    [meer - D, meer].flatMap((y) => [z0 - pfeiler, z1 + pfeiler].map((z) => projiziere(x, y, z))),
  );
  const xs = ecken.map((e) => e[0]);
  const ys = ecken.map((e) => e[1]);
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/**
 * Tisch, Tablett und Gegenstände im Blick mit k Vierteldrehungen, in der
 * Reihenfolge, in der sie gemalt werden: Ein späteres Teil deckt ein
 * früheres. `meer` ist `seaLevel`, `minY` die Unterkante der Welt: So weit
 * reicht ihr Schnitt, den die Zarge des Tischs vor ihr verdeckt. `ansicht`
 * ist das Fenster bei der ganzen Karte; an ihm liegt die Vorderkante des
 * Tischs.
 */
export function tablett(
  area: Rechteck,
  meer: number,
  minY: number,
  { projektion: p, k, projiziere }: Blick,
  ansicht: Grenzen,
): Teil[] {
  const { x0, z0, x1, z1, kante, w, D, pfeiler } = masse(area, k);
  const genordet = p.azimuth === 'north';
  const [al, ao, ar, au] = ansicht;
  // Über die Ansicht hinaus, auch eine Stufe weiter draussen: so weit reicht
  // der Tisch hinten und an den Seiten, so tief die Zarge.
  const weit = 2 * kante + (2 * (ar - al + (au - ao))) / Math.min(p.u, p.v);
  const zarge = Math.max(weit, meer - minY);
  const B = 0.05 * kante;
  const abstand = 0.04 * kante;

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
   * Mit `rolle` bekommt es eine Textur; ihre Koordinaten laufen entlang a
   * und b, in w.
   */
  const rechteck = (
    o: Vektor,
    a: Vektor,
    b: Vektor,
    aussen: Vektor,
    art: Art,
    farbe: string,
    rolle?: Rolle,
  ): Flaeche[] => {
    let n = einheit(kreuz(a, b));
    if (skalar(n, aussen) < 0) n = mal(-1, n);
    if (skalar(n, kamera) <= 1e-9) return [];
    const nah = fussNah([o, plus(o, a), plus(o, b), plus(plus(o, a), b)]);
    const flaeche: Flaeche = { form: 'flaeche', o: bild(o), a: kante3(a), b: kante3(b), n, art, farbe: beleuchte(farbe, n), nah };
    if (rolle) flaeche.textur = { rolle, a3: a, b3: b, la: Math.hypot(...a) / w, lb: Math.hypot(...b) / w };
    return [flaeche];
  };
  /**
   * Ein Quader von–bis in x, y und z: erst die Seiten, dann oben. Die Kante
   * b der Seiten zeigt nach oben. `rollen` gibt Seiten und Deckel eine Textur.
   */
  const quader = (
    [qx0, qx1]: [number, number],
    [qy0, qy1]: [number, number],
    [qz0, qz1]: [number, number],
    art: Art,
    farbe: string,
    mitOben = true,
    rollen?: [seite: Rolle | undefined, oben: Rolle],
  ): Flaeche[] => {
    const [dx, dy, dz] = [qx1 - qx0, qy1 - qy0, qz1 - qz0];
    const [seite, oben] = rollen ?? [];
    return [
      ...rechteck([qx0, qy0, qz1], [dx, 0, 0], [0, dy, 0], [0, 0, 1], art, farbe, seite),
      ...rechteck([qx0, qy0, qz0], [dx, 0, 0], [0, dy, 0], [0, 0, -1], art, farbe, seite),
      ...rechteck([qx1, qy0, qz0], [0, 0, dz], [0, dy, 0], [1, 0, 0], art, farbe, seite),
      ...rechteck([qx0, qy0, qz0], [0, 0, dz], [0, dy, 0], [-1, 0, 0], art, farbe, seite),
      ...(mitOben ? rechteck([qx0, qy1, qz0], [dx, 0, 0], [0, 0, dz], [0, 1, 0], art, farbe, oben) : []),
    ];
  };

  // Wie weit eine Seite zur Kamera zeigt: diagonal liegen +x und +z vorn,
  // genordet nur +z.
  const vorne = (nx: number, nz: number) => (genordet ? nz : nx + nz);

  // Der Tisch. Seine Vorderkante liegt an der Ansicht: diagonal die vordere
  // Ecke knapp unter der Mitte des unteren Fensterrands, genordet die Kante
  // ein Zehntel über ihm. So laufen Holzkante und Zarge wie in der Vorlage
  // durch die unteren Ecken, gleich wie viel vom Fenster die Karte füllt. Nie
  // näher als 0,05 Kanten am Rahmen. Von oben gibt es keine Zarge zu sehen,
  // dort reicht der Tisch überall weit.
  const aufPlatte = (sx: number, sy: number): [number, number] => {
    const v = (sy + (meer - D) * p.y) / p.v;
    return genordet ? [sx / p.u, v] : [(v + sx / p.u) / 2, (v - sx / p.u) / 2];
  };
  const naechst = pfeiler + 0.05 * kante;
  const [tx0, tz0] = [x0 - weit, z0 - weit];
  let [tx1, tz1] = [x1 + weit, z1 + weit];
  if (p.y > 0 && genordet) {
    tz1 = Math.max(z1 + naechst, aufPlatte(0, au - 0.1 * (au - ao))[1]);
  } else if (p.y > 0) {
    const [ex, ez] = aufPlatte((al + ar) / 2, au + 0.04 * (au - ao));
    [tx1, tz1] = [Math.max(x1 + naechst, ex), Math.max(z1 + naechst, ez)];
  }

  // Zarge, dann die Platte aus Marmor in einer Holzkante. Die nahen Stücke
  // liegen noch einmal vor den Kacheln; sie überlappen, damit keine Naht
  // bleibt.
  const platte = (qx: [number, number], qz: [number, number], art: Art, farbe: string, rolle?: Rolle) =>
    quader(qx, [-D, -D], qz, art, farbe, true, rolle && [undefined, rolle]);
  const marmor = (qx: [number, number], qz: [number, number]) =>
    platte(qx, qz, 'tisch', FARBE.marmor).map((f): Flaeche => ({ ...f, muster: 'marmor' }));
  const platten = [
    ...quader([tx0, tx1], [-D - zarge, -D], [tz0, tz1], 'zarge', FARBE.zarge, false),
    ...platte([tx0, tx1], [tz0, tz1], 'tisch', FARBE.tischkante),
    ...marmor([tx0 + B, tx1 - B], [tz0 + B, tz1 - B]),
    ...marmor([tx0 + B, tx1 - B], [z1, tz1 - B]),
    ...marmor([x1, tx1 - B], [tz0 + B, tz1 - B]),
    ...platte([tx0, tx1], [tz1 - B, tz1], 'tisch', FARBE.tischkante, 'tischkante'),
    ...platte([tx1 - B, tx1], [tz0, tz1], 'tisch', FARBE.tischkante, 'tischkante'),
  ];
  // Wo in area keine Welt liegt, zeigt das Tablett seinen Boden.
  const boden = platte([x0, x1], [z0, z1], 'boden', FARBE.boden);

  // Platzhalter der Gegenstände an den Plätzen der Vorlage, in 2:1 von
  // Südost: links oben Bücher, Pergament und Leuchter, rechts oben die
  // Kerze, links unten die Sphäre, rechts unten der Kompass auf einem Buch.
  // Was vor der Welt über den Wasserspiegel ragt, steht neben ihrem Bild.
  const links = x0 - pfeiler - abstand;
  const hinten = z0 - pfeiler - abstand;
  const vorn = z1 + pfeiler + abstand;
  const rechtsDavon = x1 + pfeiler + abstand;
  const e = kante;
  const dinge: { q: [[number, number], [number, number], [number, number]]; farbe: string }[] = [
    { q: [[links - 0.18 * e, links], [-D, -D + 0.025 * e], [z0 + 0.04 * e, z0 + 0.15 * e]], farbe: FARBE.buecher[0]! },
    { q: [[links - 0.17 * e, links - 0.01 * e], [-D + 0.025 * e, -D + 0.05 * e], [z0 + 0.05 * e, z0 + 0.15 * e]], farbe: FARBE.buecher[1]! },
    { q: [[links - 0.16 * e, links - 0.02 * e], [-D + 0.05 * e, -D + 0.072 * e], [z0 + 0.045 * e, z0 + 0.14 * e]], farbe: FARBE.buecher[2]! },
    { q: [[links - 0.15 * e, links - 0.02 * e], [-D, -D + 0.002 * e], [z0 + 0.19 * e, z0 + 0.3 * e]], farbe: FARBE.pergament },
    { q: [[links - 0.035 * e, links - 0.01 * e], [-D, -D + 0.12 * e], [z0 - 0.04 * e, z0 - 0.015 * e]], farbe: FARBE.messing },
    { q: [[x1 - 0.1 * e, x1 - 0.06 * e], [-D, -D + 0.14 * e], [hinten - 0.04 * e, hinten]], farbe: FARBE.kerze },
    { q: [[links - 0.1 * e, links], [-D, -D + 0.15 * e], [vorn, vorn + 0.1 * e]], farbe: FARBE.messing },
    { q: [[rechtsDavon, rechtsDavon + 0.12 * e], [-D, -D + 0.025 * e], [z1 - 0.1 * e, z1]], farbe: FARBE.buch },
    { q: [[rechtsDavon + 0.03 * e, rechtsDavon + 0.09 * e], [-D + 0.025 * e, -D + 0.035 * e], [z1 - 0.08 * e, z1 - 0.02 * e]], farbe: FARBE.messing },
  ];
  const tiefe = ([[qx0, qx1], [qy0, qy1], [qz0, qz1]]: (typeof dinge)[number]['q']) =>
    genordet ? p.v * (qy0 + qy1) + p.y * (qz0 + qz1) : p.y * (qx0 + qx1 + qz0 + qz1) + 2 * p.v * (qy0 + qy1);
  const [ferne, nahe] = [false, true].map((nah) =>
    dinge
      .filter(({ q: [[qx0, qx1], , [qz0]] }) => istNah(qx0, qx1, qz0) === nah)
      .sort((a, b) => tiefe(a.q) - tiefe(b.q))
      .flatMap(({ q, farbe }) => quader(q[0], q[1], q[2], 'ding', farbe)),
  ) as [Flaeche[], Flaeche[]];

  // Der Rahmen. Das Profil im Schnitt, von innen oben nach aussen unten: d
  // ab der Kante der Welt nach aussen, y ab dem Wasserspiegel. Jeder Punkt
  // gibt Art, Farbe und Rolle der Stufe bis zum nächsten: Oberkante flach,
  // drei Schrägen, Fries zwischen oberer und unterer Leiste, Fuge, Sockel bis
  // zur Platte bei −D. Nichts liegt über dem Wasserspiegel.
  const profil: [d: number, y: number, art: Art, farbe: string, rolle: Rolle][] = [
    [0, 0, 'rand', FARBE.rand, 'oberkante'],
    [0.45 * w, 0, 'rand', FARBE.rand, 'schraege'],
    [0.68 * w, -0.04 * w, 'rand', FARBE.rand, 'schraege'],
    [0.86 * w, -0.12 * w, 'rand', FARBE.rand, 'schraege'],
    [w, -0.25 * w, 'leiste', FARBE.leiste, 'leiste'],
    [1.1 * w, -0.25 * w, 'leiste', FARBE.leiste, 'leiste'],
    [1.1 * w, -0.6 * w, 'leiste', FARBE.leiste, 'leiste'],
    [w, -0.6 * w, 'wand', FARBE.wand, 'fries'],
    [w, -3.28 * w, 'leiste', FARBE.leiste, 'leiste'],
    [1.1 * w, -3.28 * w, 'leiste', FARBE.leiste, 'leiste'],
    [1.1 * w, -3.63 * w, 'leiste', FARBE.leiste, 'leiste'],
    [0.9 * w, -3.63 * w, 'wand', FARBE.fuge, 'fuge'],
    [0.9 * w, -4.4 * w, 'leiste', FARBE.leiste, 'sockel'],
    [1.15 * w, -4.4 * w, 'leiste', FARBE.leiste, 'sockel'],
    [1.15 * w, -D, 'leiste', FARBE.leiste, 'sockel'],
  ];
  /**
   * Eine Seite: Anfang an einer Ecke der Welt, Richtung entlang, Länge,
   * Richtung nach aussen. Zuerst die Innenseite bis zum Boden: Wo Welt
   * liegt, deckt sie sie; wo keine liegt, schliesst sie das Tablett.
   */
  const seite = (start: Vektor, entlang: Vektor, laenge: number, raus: Vektor): Flaeche[] => [
    ...rechteck(start, mal(laenge, entlang), [0, -D, 0], mal(-1, raus), 'wand', FARBE.wand, 'innen'),
    ...profil.slice(0, -1).flatMap(([d0, y0, art, farbe, rolle], i) => {
      const [d1, y1] = profil[i + 1]!;
      const o = plus(plus(start, mal(d0, raus)), [0, y0, 0]);
      const quer = plus(mal(d1 - d0, raus), [0, y1 - y0, 0]);
      return rechteck(o, mal(laenge, entlang), quer, plus(mal(y0 - y1, raus), [0, d1 - d0, 0]), art, farbe, rolle);
    }),
  ];
  const ecke = (cx: number, cz: number, sx: number, sz: number) =>
    quader(
      sx < 0 ? [cx - pfeiler, cx] : [cx, cx + pfeiler],
      [-D, 0],
      sz < 0 ? [cz - pfeiler, cz] : [cz, cz + pfeiler],
      'pfeiler',
      FARBE.pfeiler,
      true,
      ['pfeiler', 'kappe'],
    );
  // Die Reihenfolge nach `vorne`, die Ecken mit der Summe ihrer Seiten:
  // ferne Ecke, ferne Seiten, seitliche Ecken, nahe Seiten, nahe Ecke. So
  // deckt das Nähere das Fernere.
  const rahmen = [
    { schluessel: vorne(-1, 0), teile: seite([x0, 0, z0], [0, 0, 1], z1 - z0, [-1, 0, 0]) },
    { schluessel: vorne(1, 0), teile: seite([x1, 0, z0], [0, 0, 1], z1 - z0, [1, 0, 0]) },
    { schluessel: vorne(0, -1), teile: seite([x0, 0, z0], [1, 0, 0], x1 - x0, [0, 0, -1]) },
    { schluessel: vorne(0, 1), teile: seite([x0, 0, z1], [1, 0, 0], x1 - x0, [0, 0, 1]) },
    ...[
      [x0, z0, -1, -1],
      [x1, z0, 1, -1],
      [x0, z1, -1, 1],
      [x1, z1, 1, 1],
    ].map(([cx, cz, sx, sz]) => ({
      schluessel: 1.5 * (vorne(sx!, 0) + vorne(0, sz!)),
      teile: ecke(cx!, cz!, sx!, sz!),
    })),
  ]
    .sort((a, b) => a.schluessel - b.schluessel)
    .flatMap((r) => r.teile);

  // Weiche Schatten auf die Platte: Rahmen und Gegenstände, entlang des
  // Lichts auf die Ebene der Platte geworfen.
  const wurf = ([x, y, z]: Vektor): Punkt => {
    const t = (y + D) / licht[1];
    return bild([x - t * licht[0], -D, z - t * licht[2]]);
  };
  const quaderSchatten = ([qx0, qx1]: [number, number], [qy0, qy1]: [number, number], [qz0, qz1]: [number, number]) =>
    huelle([qx0, qx1].flatMap((x) => [qy0, qy1].flatMap((y) => [qz0, qz1].map((z) => wurf([x, y, z])))));
  const schatten: Schatten = {
    form: 'schatten',
    vielecke: [
      quaderSchatten([x0 - pfeiler, x1 + pfeiler], [-D, 0], [z0 - pfeiler, z1 + pfeiler]),
      ...dinge.map(({ q }) => quaderSchatten(...q)),
    ],
    weich: 0.9 * w * p.u,
    deckkraft: 0.55,
    nah: true,
  };

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

  // Der Schatten folgt gleich auf die Platte: Vor den Kacheln fällt er nur
  // auf das, was schon gemalt ist.
  return [...platten, schatten, ...boden, ...ferne, ...rahmen, ...nahe, ...saeume];
}
