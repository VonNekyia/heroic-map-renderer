/**
 * Stoffe und Höhenkarten des Tabletts. Jede Fläche mit Textur wird in ihrer
 * Pixelgrösse gerechnet: Höhe und Albedo je Texel, Licht aus der Normalen,
 * dann die Farbe der Rampe mit der nächsten Helligkeit. Die Rampen hat der
 * Researcher an der Vorlage gemessen (#112); kein Pixel stammt aus ihr.
 * Ohne Leaflet und ohne DOM: Es läuft im Worker, und die Tests laden es in
 * Node. Siehe docs/tablett.md, „Texturen“.
 */
import type { Flaeche, Textur, Vektor } from './tablett';

type Rgb = [number, number, number];

/** Wofür eine Fläche steht; danach richtet sich ihre Höhenkarte. */
export type Rolle =
  | 'oberkante'
  | 'innen'
  | 'schraege'
  | 'leiste'
  | 'fries'
  | 'fuge'
  | 'sockel'
  | 'pfeiler'
  | 'kappe'
  | 'tischkante';

/** Was ein Texel trägt: Albedo als Faktor und den Stoff. */
export interface Texel {
  m: number;
  stoff: keyof typeof STOFFE;
}

/** Licht und Blick, wie `lichtUndBlick` sie liefert. */
export interface Licht {
  licht: Vektor;
  halb: Vektor;
  umgebung: number;
  diffus: number;
}

/** Eine Fläche mit Textur, so viel davon, wie das Rechnen braucht. */
export type MitTextur = Pick<Flaeche, 'o' | 'a' | 'b' | 'n'> & { textur: Textur };

/** Die gerechneten Pixel einer Fläche. */
export interface Pixel {
  /** Stellen auf der Leinwand, y · Breite + x. */
  stellen: Int32Array;
  /** Farben als RGBA in einer Zahl, Little Endian wie in `ImageData`. */
  farben: Uint32Array;
  /** Das Rechteck um die Stellen: links, oben, rechts, unten, rechts und unten ausschliesslich. */
  rahmen: [number, number, number, number];
}

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
 * Holz der Oberkante, auch für Schrägen, Leisten und Pfeiler; Holz der Wand
 * für Fries, Fuge, Sockel und Innenseite; Holz des Tischs für seine Kante;
 * Messing für die Nägel. Die Albedo ist die Helligkeit der
 * Mitte der Rampe, wo die Vorlage sie im vollen Licht zeigt.
 */
export const STOFFE: Record<'oberholz' | 'wandholz' | 'tischholz' | 'messing', Stoff> = {
  oberholz: {
    rampe: ['#4c2113', '#673420', '#743f27', '#82472c', '#935532', '#bf864d', '#f9c27f'],
    albedo: 0.094,
    glanz: 0.6,
    exponent: 24,
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
};

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

/** Eine Farbe als RGBA in einer Zahl, Little Endian wie in `ImageData`. */
export const rgba = ([r, g, b]: Rgb): number => ((255 << 24) | (b << 16) | (g << 8) | r) >>> 0;

/**
 * Die Rampe eines Stoffs nach Helligkeit, dazu die Schwellen zwischen zwei
 * Farben: das geometrische Mittel ihrer Helligkeiten, also die Mitte im
 * Logarithmus.
 */
const PALETTEN = Object.fromEntries(
  Object.entries(STOFFE).map(([name, { rampe }]) => {
    const farben = rampe.map(rgb).sort((a, b) => helligkeit(a) - helligkeit(b));
    const schwellen = farben.slice(1).map((f, i) => Math.sqrt(helligkeit(f) * helligkeit(farben[i]!)));
    return [name, { farben: Uint32Array.from(farben, rgba), schwellen }];
  }),
) as Record<keyof typeof STOFFE, { farben: Uint32Array; schwellen: number[] }>;

/** Die Farbe der Rampe, deren Helligkeit `y` im Logarithmus am nächsten liegt, als RGBA. */
export function farbe(stoff: keyof typeof STOFFE, y: number): number {
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

/**
 * Glattes Wertrauschen in [0, 1]. Mit Perioden `px` und `py` in Gitterzellen
 * wiederholt es sich, für Kacheln ohne Naht.
 */
function rauschen(x: number, y: number, px = 0, py = 0): number {
  // Heiss: ohne Arrays und Closures, sonst kostet es das Zehnfache.
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const fx = x - xi;
  const fy = y - yi;
  const u = fx * fx * (3 - 2 * fx);
  const v = fy * fy * (3 - 2 * fy);
  let x0 = xi;
  let x1 = xi + 1;
  let y0 = yi;
  let y1 = yi + 1;
  if (px) {
    x0 = ((x0 % px) + px) % px;
    x1 = ((x1 % px) + px) % px;
  }
  if (py) {
    y0 = ((y0 % py) + py) % py;
    y1 = ((y1 % py) + py) % py;
  }
  const a = zufall(x0, y0);
  const b = zufall(x1, y0);
  const c = zufall(x0, y1);
  const d = zufall(x1, y1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

/** Eine runde Erhebung über −1 < t < 1, 1 in der Mitte. */
const buckel = (t: number) => (Math.abs(t) < 1 ? Math.sqrt(1 - t * t) : 0);

/** Holzmaserung entlang u, als Faktor der Albedo um 1: wenige, weiche Linien. */
function maserung(u: number, v: number): number {
  const linie = 0.5 + 0.5 * Math.sin((v * 4 + rauschen(u * 0.15, v * 1.5) * 2) * Math.PI);
  return 0.9 + 0.2 * linie ** 3;
}

/** Gewünschte Länge eines Felds am Fries in w. */
const FELD = 3.4;

/** Länge eines Felds am Fries in w, so dass je Seite ganze Felder passen. */
const feldLaenge = (la: number) => la / Math.max(1, Math.round(la / FELD));

/** Lage in einem Feld der Länge `laenge`. */
const imFeld = (u: number, laenge: number) => u - Math.floor(u / laenge) * laenge;

/** Abstand zum nächsten Messingnagel, in Radien; unter 1 liegt Messing. */
function nagel(rolle: Rolle, u: number, v: number, la: number, lb: number): number {
  if (rolle === 'fries') {
    const laenge = feldLaenge(la);
    const pu = imFeld(u, laenge);
    const du = Math.min(pu, laenge - pu);
    const dv = v - lb / 2;
    return Math.sqrt(du * du + dv * dv) / 0.2;
  }
  if (rolle === 'pfeiler') {
    const du = u - la / 2;
    const dv = v - lb * 0.8;
    return Math.sqrt(du * du + dv * dv) / 0.22;
  }
  return Infinity;
}

/**
 * Wovon die Höhe einer Rolle abhängt: 0 von nichts, 1 nur von v, 2 von u
 * und v. Was nicht davon abhängt, leitet `rechne` nicht ab.
 */
const ABHAENGIG: Record<Rolle, 0 | 1 | 2> = {
  oberkante: 1,
  innen: 0,
  schraege: 0,
  leiste: 1,
  fries: 2,
  fuge: 0,
  sockel: 1,
  pfeiler: 2,
  kappe: 2,
  tischkante: 2,
};

/**
 * Die Höhe einer Rolle an (u, v) in w über der Fläche: u entlang der Kante a
 * der Fläche, v entlang b. `la` und `lb` sind die Längen der Fläche in w.
 */
export function hoehe(rolle: Rolle, u: number, v: number, la: number, lb: number): number {
  const n = nagel(rolle, u, v, la, lb);
  switch (rolle) {
    case 'oberkante':
      // Die Lippe zur Karte hin, dahinter eine schmale Kehle.
      return 0.07 * buckel((v - 0.1) / 0.08) - 0.025 * buckel((v - 0.22) / 0.04);
    case 'leiste':
      // Ein Wulst über die Mitte der Leiste.
      return 0.04 * buckel((v - lb / 2) / (lb / 2));
    case 'fries': {
      // Vertiefte Felder in einem erhabenen Rahmen, an den Stössen ein
      // Messingnagel. Die Ranken darin kommen gemalt. Siehe docs/tablett.md,
      // „Texturen“.
      if (n < 1) return 0.08 + 0.12 * Math.sqrt(1 - n * n);
      const laenge = feldLaenge(la);
      const pu = imFeld(u, laenge);
      const rand = Math.min(pu, laenge - pu, v, lb - v);
      return rand < 0.12 ? 0.08 * (rand / 0.12) : rand < 0.3 ? 0.08 : -0.04;
    }
    case 'sockel':
      // Eine Fase an der oberen Kante.
      return 0.05 * Math.min(1, v / 0.2);
    case 'pfeiler': {
      // Ein vertieftes Feld je Seite, darin oben ein Messingnagel.
      if (n < 1) return 0.12 * Math.sqrt(1 - n * n);
      const rand = Math.min(u, la - u, v, lb - v);
      return rand < 0.25 ? 0.06 * Math.min(1, rand / 0.12) : -0.03;
    }
    case 'kappe':
      // Eine flache Pyramide.
      return 0.25 * (1 - Math.max(Math.abs((2 * u) / la - 1), Math.abs((2 * v) / lb - 1)));
    case 'tischkante': {
      // Quer zur Kante: eine Nut, dann ein runder Abschluss nach aussen.
      const [quer, breit] = la < lb ? [u, la] : [v, lb];
      const aussen = breit - quer;
      return -0.06 * buckel((quer - 0.5) / 0.12) + (aussen < 0.8 ? 0.25 * (Math.sqrt(1 - (1 - aussen / 0.8) ** 2) - 1) : 0);
    }
    case 'fuge':
    case 'innen':
    case 'schraege':
      return 0;
  }
}

/**
 * Albedo und Stoff einer Rolle an (u, v), wo die Höhe `h` ist, geschrieben
 * in `aus`, damit je Pixel kein Objekt entsteht. Vertieftes liegt im
 * Schatten der Ränder.
 */
export function texel(rolle: Rolle, u: number, v: number, la: number, lb: number, h: number, aus: Texel): Texel {
  if (rolle === 'fuge') [aus.m, aus.stoff] = [0.5, 'wandholz'];
  else if (nagel(rolle, u, v, la, lb) < 1) [aus.m, aus.stoff] = [1, 'messing'];
  else if (rolle === 'tischkante') [aus.m, aus.stoff] = [la < lb ? maserung(v, u) : maserung(u, v), 'tischholz'];
  else {
    aus.m = maserung(u, v) * (1 + 3 * Math.min(0, h));
    aus.stoff = rolle === 'fries' || rolle === 'sockel' || rolle === 'innen' ? 'wandholz' : 'oberholz';
  }
  return aus;
}

/**
 * Wo im Pixel die Normale gerechnet wird, als Versatz von seiner Mitte.
 * Schmale Rollen nehmen vier Proben im gedrehten Gitter und mitteln das
 * Licht; sonst zerfiele die Lippe auf der Schräge des Bilds in Punkte.
 */
const PROBEN: Partial<Record<Rolle, readonly (readonly [number, number])[]>> = Object.fromEntries(
  (['oberkante', 'leiste'] as const).map((rolle) => [
    rolle,
    [
      [-0.125, -0.375],
      [0.375, -0.125],
      [0.125, 0.375],
      [-0.375, 0.125],
    ],
  ]),
);
const MITTE = [[0, 0]] as const;

/**
 * Rechnet eine Fläche mit Textur Pixel für Pixel, soweit sie auf der
 * Leinwand (`breite` × `hoch`) liegt. `s` ist ein Pixel der feinsten Stufe
 * in Pixeln der Leinwand, (`x0`, `y0`) der Punkt (0, 0) darauf. Ein Pixel
 * gehört zur Fläche, wenn seine Mitte darin liegt; es nimmt ihre
 * Koordinaten in w, die Normale aus der Höhenkarte und das Licht: Umgebung
 * und diffuses Licht nach der Normalen, dazu das Glanzlicht nach
 * Blinn-Phong mit `halb`, dem Vektor zwischen Licht und Blick. Siehe
 * docs/tablett.md, „Texturen“.
 */
export function rechne(
  f: MitTextur,
  s: number,
  [x0, y0]: [number, number],
  l: Licht,
  [breite, hoch]: [number, number],
): Pixel {
  const { o, a, b, n } = f;
  const { rolle, a3, b3, la, lb } = f.textur;
  const ox = s * o[0] + x0;
  const oy = s * o[1] + y0;
  const [ax, ay, bx, by] = [s * a[0], s * a[1], s * b[0], s * b[1]];
  const det = ax * by - ay * bx;
  const oben = Math.max(0, Math.floor(Math.min(oy, oy + ay, oy + by, oy + ay + by)));
  const unten = Math.min(hoch, Math.ceil(Math.max(oy, oy + ay, oy + by, oy + ay + by)));
  // Erst je Zeile die Spanne der Pixel: alpha = (cx · by − cy · bx) / det und
  // beta = (ax · cy − ay · cx) / det sind in cx linear, beide in [0, 1].
  const spannen = new Int32Array(2 * Math.max(0, unten - oben));
  let anzahl = 0;
  const rahmen: Pixel['rahmen'] = [breite, hoch, 0, 0];
  for (let py = oben; py < unten && Math.abs(det) > 1e-9; py++) {
    const cy = py + 0.5 - oy;
    let von = -Infinity;
    let bis = Infinity;
    for (const [steigung, wert] of [
      [by / det, (-cy * bx) / det],
      [-ay / det, (ax * cy) / det],
    ] as const) {
      if (Math.abs(steigung) < 1e-12) {
        if (wert < 0 || wert > 1) [von, bis] = [Infinity, -Infinity];
        continue;
      }
      const [g0, g1] = [-wert / steigung, (1 - wert) / steigung];
      von = Math.max(von, Math.min(g0, g1));
      bis = Math.min(bis, Math.max(g0, g1));
    }
    const pxVon = Math.max(0, Math.ceil(von + ox - 0.5));
    const pxBis = Math.min(breite - 1, Math.floor(bis + ox - 0.5));
    const i = 2 * (py - oben);
    [spannen[i], spannen[i + 1]] = [pxVon, pxBis];
    if (pxBis < pxVon) continue;
    anzahl += pxBis - pxVon + 1;
    rahmen[0] = Math.min(rahmen[0], pxVon);
    rahmen[1] = Math.min(rahmen[1], py);
    rahmen[2] = Math.max(rahmen[2], pxBis + 1);
    rahmen[3] = py + 1;
  }
  const stellen = new Int32Array(anzahl);
  const farben = new Uint32Array(anzahl);
  if (anzahl === 0) return { stellen, farben, rahmen };
  const laengeA = Math.sqrt(a3[0] * a3[0] + a3[1] * a3[1] + a3[2] * a3[2]);
  const laengeB = Math.sqrt(b3[0] * b3[0] + b3[1] * b3[1] + b3[2] * b3[2]);
  const [tx, ty, tz] = [a3[0] / laengeA, a3[1] / laengeA, a3[2] / laengeA];
  const [qx, qy, qz] = [b3[0] / laengeB, b3[1] / laengeB, b3[2] / laengeB];
  // Ein Pixel in w entlang a und b, für die Ableitung der Höhe; am Rand
  // der Fläche nach innen.
  const du = la / Math.max(1, Math.sqrt(ax * ax + ay * ay));
  const dv = lb / Math.max(1, Math.sqrt(bx * bx + by * by));
  const t: Texel = { m: 1, stoff: 'oberholz' };
  const proben = PROBEN[rolle] ?? MITTE;
  const abhaengig = ABHAENGIG[rolle];
  // Je Probe der Kosinus zum halben Vektor, für das Glanzlicht.
  const halb = new Float64Array(proben.length);
  let k = 0;
  for (let py = oben; py < unten; py++) {
    const bis = spannen[2 * (py - oben) + 1]!;
    for (let px = spannen[2 * (py - oben)]!; px <= bis; px++) {
      let diffus = 0;
      let hoehen = 0;
      for (let i = 0; i < proben.length; i++) {
        const probe = proben[i]!;
        const cx = px + 0.5 + probe[0] - ox;
        const cy = py + 0.5 + probe[1] - oy;
        const u = ((cx * by - cy * bx) / det) * la;
        const v = ((ax * cy - ay * cx) / det) * lb;
        let h = 0;
        let hu = 0;
        let hv = 0;
        if (abhaengig > 0) {
          h = hoehe(rolle, u, v, la, lb);
          const sv = v + dv <= lb ? dv : -dv;
          hv = (hoehe(rolle, u, v + sv, la, lb) - h) / sv;
        }
        if (abhaengig > 1) {
          const su = u + du <= la ? du : -du;
          hu = (hoehe(rolle, u + su, v, la, lb) - h) / su;
        }
        const nx = n[0] - hu * tx - hv * qx;
        const ny = n[1] - hu * ty - hv * qy;
        const nz = n[2] - hu * tz - hv * qz;
        const laenge = Math.sqrt(nx * nx + ny * ny + nz * nz);
        const nl = (nx * l.licht[0] + ny * l.licht[1] + nz * l.licht[2]) / laenge;
        diffus += nl > 0 ? nl : 0;
        halb[i] = (nx * l.halb[0] + ny * l.halb[1] + nz * l.halb[2]) / laenge;
        hoehen += h;
      }
      // Albedo und Stoff ändern sich im Pixel kaum: einmal in der Mitte.
      const cx = px + 0.5 - ox;
      const cy = py + 0.5 - oy;
      texel(rolle, ((cx * by - cy * bx) / det) * la, ((ax * cy - ay * cx) / det) * lb, la, lb, hoehen / proben.length, t);
      const { albedo, glanz, exponent } = STOFFE[t.stoff];
      let glaenzend = 0;
      for (let i = 0; i < proben.length; i++) if (halb[i]! > 0) glaenzend += halb[i]! ** exponent;
      const y = albedo * t.m * (l.umgebung + (l.diffus * diffus) / proben.length) + (glanz * glaenzend) / proben.length;
      stellen[k] = py * breite + px;
      farben[k++] = farbe(t.stoff, y);
    }
  }
  return { stellen, farben, rahmen };
}

/** Marmor: der Grund, gemessen an der Vorlage, von dunkel nach hell, als RGBA. */
const MARMOR = {
  grund: Uint32Array.from(['#13120c', '#151511', '#191813', '#1e1c15', '#232118', '#2a261b', '#322d20'], (f) => rgba(rgb(f))),
};

/** Seitenlänge der Kachel Marmor in Pixeln. */
export const KACHEL = 512;

/** Schwellen der geordneten Rasterung, 4 × 4 nach Bayer, in [0, 1). */
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((b) => (b + 0.5) / 16);

/**
 * Eine Kachel Marmor in Pixeln des Bildschirms, ohne Naht, als RGBA: nur
 * der Grund, Wolken aus seinen sieben Farben, geordnet gerastert,
 * damit keine Stufen stehen bleiben. Die Platte liegt waagrecht;
 * `streckung` ist v/u der Projektion und drückt die Wolken in der Höhe
 * zusammen wie die Platte selbst. Die Adern zieht `adern`.
 */
export function marmorKachel(groesse: number, streckung: number): Uint8ClampedArray<ArrayBuffer> {
  const pixel = new Uint32Array(groesse * groesse);
  const zx = 4;
  const zy = Math.max(1, Math.round(zx / streckung));
  // Die Wolken sind glatt: Ein Gitter mit acht Punkten je feinster Zelle
  // reicht, dazwischen linear. Das halbiert die Zeit der Kachel.
  const raster = groesse / 64;
  const gitter = new Float64Array(65 * 65);
  for (let j = 0; j <= 64; j++) {
    for (let i = 0; i <= 64; i++) {
      const [fx, fy] = [(i / 64) * zx, (j / 64) * zy];
      gitter[j * 65 + i] = 0.65 * rauschen(fx, fy, zx, zy) + 0.35 * rauschen(fx * 2, fy * 2, zx * 2, zy * 2);
    }
  }
  for (let y = 0; y < groesse; y++) {
    const j = Math.floor(y / raster);
    const b = y / raster - j;
    for (let x = 0; x < groesse; x++) {
      const i = Math.floor(x / raster);
      const a = x / raster - i;
      const k = j * 65 + i;
      const oben = gitter[k]! + (gitter[k + 1]! - gitter[k]!) * a;
      const unten = gitter[k + 65]! + (gitter[k + 66]! - gitter[k + 65]!) * a;
      const stufe = Math.floor((oben + (unten - oben) * b) * 6 + BAYER[(y & 3) * 4 + (x & 3)]!);
      pixel[y * groesse + x] = MARMOR.grund[Math.min(6, Math.max(0, stufe))]!;
    }
  }
  return new Uint8ClampedArray(pixel.buffer);
}

/** Ein Punkt einer Ader in Pixeln der Leinwand, mit ihrer Breite dort. */
export interface Aderpunkt {
  x: number;
  y: number;
  breite: number;
}

/** Abstand der Punkte einer Ader in Pixeln, höchstens. */
const SCHRITT = 4;

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
 * Die Adern im Marmor über eine Leinwand von `breite` × `hoehe`: Risse,
 * schräg über die Platte, im Kleinen zackig, zu den Enden spitz, verzweigt
 * in feinere. Gedacht auf der Platte, in der Höhe um `streckung`
 * zusammengedrückt. Für dieselbe Grösse immer dieselben.
 */
export function adern(breite: number, hoehe: number, streckung: number): Aderpunkt[][] {
  const zufall = folge(112);
  const tief = hoehe / streckung;
  const alle: Aderpunkt[][] = [];
  // Von a nach b: Jede Stufe versetzt die Mitten quer, um einen festen
  // Anteil ihres Stücks, bis kein Stück länger als SCHRITT ist.
  const riss = (ax: number, ay: number, bx: number, by: number, dicke: number, stufe: number): void => {
    let punkte: [number, number][] = [
      [ax, ay],
      [bx, by],
    ];
    while (Math.hypot(punkte[1]![0] - punkte[0]![0], punkte[1]![1] - punkte[0]![1]) > SCHRITT) {
      punkte = punkte.flatMap((q, i) => {
        if (i === 0) return [q];
        const p = punkte[i - 1]!;
        const versatz = (zufall() - 0.5) * 0.3;
        const mitte: [number, number] = [(p[0] + q[0]) / 2 - (q[1] - p[1]) * versatz, (p[1] + q[1]) / 2 + (q[0] - p[0]) * versatz];
        return [mitte, q];
      });
    }
    const n = punkte.length - 1;
    alle.push(punkte.map(([x, y], i) => ({ x, y: y * streckung, breite: dicke * Math.sin((Math.PI * i) / n) ** 0.6 })));
    // Hauptadern zweigen zwei- bis viermal ab, ihre Äste bis zweimal.
    const aeste = stufe === 0 ? 2 + Math.floor(zufall() * 3) : stufe === 1 ? Math.floor(zufall() * 3) : 0;
    for (let k = 0; k < aeste; k++) {
      const [x, y] = punkte[Math.floor((0.15 + 0.7 * zufall()) * n)]!;
      const winkel = Math.atan2(by - ay, bx - ax) + (zufall() < 0.5 ? -1 : 1) * (0.4 + 0.6 * zufall());
      const laenge = (0.2 + 0.25 * zufall()) * Math.hypot(bx - ax, by - ay);
      riss(x, y, x + Math.cos(winkel) * laenge, y + Math.sin(winkel) * laenge, dicke * 0.6, stufe + 1);
    }
  };
  // Eine Hauptader je rund 420 × 420 Pixel der Platte.
  const zahl = Math.max(2, Math.round((breite * tief) / 180_000));
  for (let i = 0; i < zahl; i++) {
    const [x, y] = [zufall() * breite, zufall() * tief];
    const winkel = 0.45 + (zufall() - 0.5) * 0.8;
    const laenge = (0.2 + 0.3 * zufall()) * Math.hypot(breite, tief);
    const [dx, dy] = [(Math.cos(winkel) * laenge) / 2, (Math.sin(winkel) * laenge) / 2];
    riss(x - dx, y - dy, x + dx, y + dy, 0.9 + 0.6 * zufall(), 0);
  }
  return alle;
}

/** Die Farbe einer Ader nach ihrer Breite, aus dem hellen Ende der Rampe der Adern: dünne dunkler. */
export const ADERFARBEN = [
  { ab: 0.3, farbe: '#705538' },
  { ab: 0.6, farbe: '#846749' },
  { ab: 1, farbe: '#a58260' },
] as const;
