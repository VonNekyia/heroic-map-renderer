/**
 * Rahmen und Tisch um die Karte: Die Welt liegt in einem Tablett, dessen
 * Oberkante auf dem Wasserspiegel liegt, und das Tablett auf einem Tisch.
 * Alles besteht aus ebenen Rechtecken; in der Parallelprojektion geht jedes
 * affin aufs Bild. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/tablett.md.
 */
import type { Grenzen, Kontext, Projektion, Rechteck } from 'heroic-map-renderer/skin-api';
import {
  bildName,
  DICHTEN,
  DIFFUS,
  einheit,
  lichtUndBlick,
  mal,
  MASS,
  plus,
  RAND,
  type Richtung,
  UMGEBUNG,
  type Vektor,
} from './atlas';

type Punkt = [number, number];

/** Der Blick, aus dem das Tablett gezeichnet wird, so wie ihn die Grundkarte reicht. */
export type Blick = Pick<Kontext, 'projektion' | 'k' | 'projiziere'>;

export type Art = 'tisch' | 'zarge' | 'boden' | 'rand' | 'wand' | 'innen' | 'pfeiler' | 'ding';

/**
 * Die Textur einer Fläche: das Bild im Atlas, an einer Wand dazu das Bild
 * ihrer Stösse, und die Längen der Kanten a und b in w. Siehe
 * docs/tablett.md, „Texturen“.
 */
export interface Textur {
  bild: string;
  stoss?: string;
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
  /** CSS-Farbe, schon im Licht; mit Textur die Farbe, wenn ihr Bild fehlt. */
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

/** Grundfarben, bevor das Licht sie trifft. */
const FARBE = {
  rand: '#8a542c',
  wand: '#4e2c18',
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

const skalar = (u: Vektor, v: Vektor) => u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
const kreuz = (u: Vektor, v: Vektor): Vektor => [
  u[1] * v[2] - u[2] * v[1],
  u[2] * v[0] - u[0] * v[2],
  u[0] * v[1] - u[1] * v[0],
];

/** Die Richtung einer waagrechten Achse. */
const richtung = ([x, , z]: Vektor): Richtung => (x > 0.5 ? '+x' : x < -0.5 ? '-x' : z > 0.5 ? '+z' : '-z');

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
 * Die Masse im Blick, in Blöcken: die Kanten der Welt, der Rand w, die Tiefe
 * D vom Wasserspiegel bis zur Platte und die Breite der Pfeiler. Ohne `w`
 * gilt der Anteil `RAND` der Kante. Siehe docs/tablett.md, „Masse“.
 */
function masse(area: Rechteck, k: number, w?: number) {
  const [x0, z0, x1, z1] = imBlick(area, k);
  const kante = (x1 - x0 + (z1 - z0)) / 2;
  const rand = w ?? RAND * kante;
  return { x0, z0, x1, z1, kante, w: rand, D: MASS.tiefe * rand, pfeiler: MASS.pfeiler * rand };
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

/** So viel des Fensters soll der Rahmen in der Gesamtansicht füllen, wie in der Vorlage. */
const FUELLUNG = 0.9;

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
 * Das Raster des Tabletts bei `s` Pixeln des Bildschirms je Pixel der
 * feinsten Stufe: die Dichte des Atlas, die Pixel, die ein Texel waagrecht
 * breit ist, und die Breite w in Blöcken, die dazu gehört. Ein Texel ist
 * 1 px breit, solange es einen Atlas so dicht gibt, sonst ganze Pixel. w
 * liegt so nahe am Anteil `RAND` der Kante, wie es geht. Siehe
 * docs/tablett.md, „Ganze Pixel“.
 */
export function raster(area: Rechteck, { u }: Projektion, s: number): { dichte: number; pixel: number; w: number } {
  const band = RAND * (area[2] - area[0]) * u * s;
  const [duenn, dicht] = [DICHTEN[0], DICHTEN[DICHTEN.length - 1]!];
  const pixel = Math.max(1, Math.ceil(band / (dicht + 0.5)));
  const dichte = Math.min(dicht, Math.max(duenn, Math.round(band / pixel)));
  return { dichte, pixel, w: (dichte * pixel) / (u * s) };
}

/**
 * Tisch, Tablett und Gegenstände im Blick mit k Vierteldrehungen, in der
 * Reihenfolge, in der sie gemalt werden: Ein späteres Teil deckt ein
 * früheres. `meer` ist `seaLevel`, `minY` die Unterkante der Welt: So weit
 * reicht ihr Schnitt, den die Zarge des Tischs vor ihr verdeckt. `ansicht`
 * ist das Fenster bei der ganzen Karte; an ihm liegt die Vorderkante des
 * Tischs. `rand` ist w in Blöcken, ohne Angabe der Anteil `RAND` der Kante.
 */
export function tablett(
  area: Rechteck,
  meer: number,
  minY: number,
  { projektion: p, k, projiziere }: Blick,
  ansicht: Grenzen,
  rand?: number,
): Teil[] {
  const { x0, z0, x1, z1, kante, w, D, pfeiler } = masse(area, k, rand);
  const genordet = p.azimuth === 'north';
  const [al, ao, ar, au] = ansicht;
  // Über die Ansicht hinaus, auch eine Stufe weiter draussen: so weit reicht
  // der Tisch hinten und an den Seiten, so tief die Zarge.
  const weit = 2 * kante + (2 * (ar - al + (au - ao))) / Math.min(p.u, p.v);
  const zarge = Math.max(weit, meer - minY);
  const B = MASS.tischkante * w;
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
   * Mit `textur`, dem Namen eines Bilds im Atlas, liegt dieses Bild darauf:
   * seine Breite entlang a, seine Höhe entlang b, ab der Ecke o.
   */
  const rechteck = (o: Vektor, a: Vektor, b: Vektor, aussen: Vektor, art: Art, farbe: string, textur?: string): Flaeche[] => {
    let n = einheit(kreuz(a, b));
    if (skalar(n, aussen) < 0) n = mal(-1, n);
    if (skalar(n, kamera) <= 1e-9) return [];
    const nah = fussNah([o, plus(o, a), plus(o, b), plus(plus(o, a), b)]);
    const flaeche: Flaeche = { form: 'flaeche', o: bild(o), a: kante3(a), b: kante3(b), n, art, farbe: beleuchte(farbe, n), nah };
    if (textur) flaeche.textur = { bild: textur, la: Math.hypot(...a) / w, lb: Math.hypot(...b) / w };
    return [flaeche];
  };
  /** Ein Quader von–bis in x, y und z: erst die Seiten, dann oben. */
  const quader = (
    [qx0, qx1]: [number, number],
    [qy0, qy1]: [number, number],
    [qz0, qz1]: [number, number],
    art: Art,
    farbe: string,
    mitOben = true,
  ): Flaeche[] => {
    const [dx, dy, dz] = [qx1 - qx0, qy1 - qy0, qz1 - qz0];
    return [
      ...rechteck([qx0, qy0, qz1], [dx, 0, 0], [0, dy, 0], [0, 0, 1], art, farbe),
      ...rechteck([qx0, qy0, qz0], [dx, 0, 0], [0, dy, 0], [0, 0, -1], art, farbe),
      ...rechteck([qx1, qy0, qz0], [0, 0, dz], [0, dy, 0], [1, 0, 0], art, farbe),
      ...rechteck([qx0, qy0, qz0], [0, 0, dz], [0, dy, 0], [-1, 0, 0], art, farbe),
      ...(mitOben ? rechteck([qx0, qy1, qz0], [dx, 0, 0], [0, 0, dz], [0, 1, 0], art, farbe) : []),
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
  // bleibt. Die Holzkante trägt ihr Bild entlang, von innen nach aussen.
  const platte = ([qx0, qx1]: [number, number], [qz0, qz1]: [number, number], art: Art, farbe: string) =>
    rechteck([qx0, -D, qz0], [qx1 - qx0, 0, 0], [0, 0, qz1 - qz0], [0, 1, 0], art, farbe);
  const marmor = (qx: [number, number], qz: [number, number]) =>
    platte(qx, qz, 'tisch', FARBE.marmor).map((f): Flaeche => ({ ...f, muster: 'marmor' }));
  const holzkante = (o: Vektor, entlang: Vektor, laenge: number, raus: Vektor) =>
    rechteck(o, mal(laenge, entlang), mal(B, raus), [0, 1, 0], 'tisch', FARBE.tischkante, bildName(p.azimuth, 'tischkante', richtung(raus)));
  const platten = [
    ...quader([tx0, tx1], [-D - zarge, -D], [tz0, tz1], 'zarge', FARBE.zarge, false),
    ...platte([tx0, tx1], [tz0, tz1], 'tisch', FARBE.tischkante),
    ...marmor([tx0 + B, tx1 - B], [tz0 + B, tz1 - B]),
    ...marmor([tx0 + B, tx1 - B], [z1, tz1 - B]),
    ...marmor([x1, tx1 - B], [tz0 + B, tz1 - B]),
    ...holzkante([tx0, -D, tz1 - B], [1, 0, 0], tx1 - tx0, [0, 0, 1]),
    ...holzkante([tx1 - B, -D, tz0], [0, 0, 1], tz1 - tz0, [1, 0, 0]),
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

  // Der Rahmen, je Seite drei Flächen: die Innenseite bis zum Boden, das
  // Band der Oberkante auf dem Wasserspiegel und die Wand von seiner
  // Aussenkante bis zur Platte. Das Profil dazwischen, Lippe, Schrägen,
  // Leisten, Fries, Fuge und Sockel, liegt in ihren Bildern. Nichts liegt
  // über dem Wasserspiegel. Siehe docs/tablett.md, „Masse“.
  const runter: Vektor = [0, -D, 0];
  /**
   * Eine Seite: Anfang an einer Ecke der Welt, Richtung entlang, Länge,
   * Richtung nach aussen. Zuerst die Innenseite: Wo Welt liegt, deckt sie
   * sie; wo keine liegt, schliesst sie das Tablett.
   */
  const seite = (start: Vektor, entlang: Vektor, laenge: number, raus: Vektor): Flaeche[] => {
    const name = (rolle: 'oben' | 'wand' | 'stoss') => bildName(p.azimuth, rolle, richtung(raus));
    const a = mal(laenge, entlang);
    return [
      ...rechteck(start, a, runter, mal(-1, raus), 'innen', FARBE.wand),
      ...rechteck(start, a, mal(w, raus), [0, 1, 0], 'rand', FARBE.rand, name('oben')),
      ...rechteck(plus(start, mal(w, raus)), a, runter, raus, 'wand', FARBE.wand, name('wand')).map(
        (f): Flaeche => ({ ...f, textur: { ...f.textur!, stoss: name('stoss') } }),
      ),
    ];
  };
  /** Ein Pfeiler an der Ecke (cx, cz), nach (sx, sz) hinaus: erst die Seiten, dann der Deckel. */
  const ecke = (cx: number, cz: number, sx: number, sz: number): Flaeche[] => {
    const [qx0, qx1] = sx < 0 ? [cx - pfeiler, cx] : [cx, cx + pfeiler];
    const [qz0, qz1] = sz < 0 ? [cz - pfeiler, cz] : [cz, cz + pfeiler];
    const name = (r?: Richtung) => bildName(p.azimuth, r ? 'pfeiler' : 'kappe', r);
    const flanke = (o: Vektor, entlang: Vektor, raus: Vektor) =>
      rechteck(o, mal(pfeiler, entlang), runter, raus, 'pfeiler', FARBE.pfeiler, name(richtung(raus)));
    return [
      ...flanke([qx0, 0, qz1], [1, 0, 0], [0, 0, 1]),
      ...flanke([qx0, 0, qz0], [1, 0, 0], [0, 0, -1]),
      ...flanke([qx1, 0, qz0], [0, 0, 1], [1, 0, 0]),
      ...flanke([qx0, 0, qz0], [0, 0, 1], [-1, 0, 0]),
      ...rechteck([qx0, 0, qz0], [pfeiler, 0, 0], [0, 0, pfeiler], [0, 1, 0], 'pfeiler', FARBE.pfeiler, name()),
    ];
  };
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
