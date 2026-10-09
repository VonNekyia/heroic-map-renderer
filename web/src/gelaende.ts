/**
 * Formen der Ebenen auf dem Gelände: Abtasten, Höhen, was verdeckt ist und
 * das Netz der Flächen. Rechnet in Blöcken der Welt und in Pixeln der
 * feinsten Stufe. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/benutzung/ebenen.md, „Zeichnen“.
 */
import { projiziere, type Projektion } from './pick';

/** Ein Punkt `[x, z]` in Blöcken, oder `[px, py]` in Pixeln der feinsten Stufe. */
export type Punkt = [number, number];

/** Ein Polygon der Welt; Ringe schliessen sich selbst. */
export interface Polygon {
  aussen: Punkt[];
  loecher: Punkt[][];
}

/** Die Höhen, wie Formen und Nadeln sie lesen. */
export interface Gelaende {
  /** `heightsCell`: Kante einer Zelle in Blöcken. */
  c: number;
  /** `seaLevel`, sonst 64. */
  grund: number;
  /** Der Wert der Zelle mit dem Index (i, j) in der Welt, `undefined` ohne Wert. */
  zelle: (i: number, j: number) => number | undefined;
  /** Die höchste Oberseite; darüber kann nichts verdecken. */
  max: number;
}

/** Projektion und Drehung des Blicks. */
export interface Blick {
  p: Projektion;
  /** Vierteldrehungen gegen die Vorgabe, siehe `RICHTUNGEN` in pick.ts. */
  k: number;
}

/** Ein Punkt der Welt im Blick, k Vierteldrehungen; ohne −1 wie bei Blöcken. */
export function punktImBlick(x: number, z: number, k: number): Punkt {
  let [a, b] = [x, z];
  for (let i = 0; i < k; i++) [a, b] = [b, -a];
  return [a, b];
}

/** Der Bildpunkt von (x, y, z) der Welt in Pixeln der feinsten Stufe. */
export function bildpunkt(x: number, y: number, z: number, { p, k }: Blick): Punkt {
  const [vx, vz] = punktImBlick(x, z, k);
  return projiziere(vx, y, vz, p);
}

/**
 * `H(x, z)`, die Oberseite des Geländes: bilinear zwischen den Mitten der
 * Zellen; eine Zelle ohne Wert nimmt den Mittelwert der Nachbarn mit Wert
 * im Umkreis von zwei Zellen, sonst `grund`. Siehe docs/benutzung/ebenen.md,
 * „Die Oberfläche im iso“.
 */
export function oberflaeche(g: Gelaende, x: number, z: number): number {
  const { c } = g;
  const zelle = (i: number, j: number): number => {
    const eigen = g.zelle(i, j);
    if (eigen !== undefined) return eigen;
    let summe = 0;
    let anzahl = 0;
    for (let di = -2; di <= 2; di++) {
      for (let dj = -2; dj <= 2; dj++) {
        const n = g.zelle(i + di, j + dj);
        if (n !== undefined) {
          summe += n;
          anzahl++;
        }
      }
    }
    return anzahl ? summe / anzahl : g.grund;
  };
  const [fx, fz] = [x / c - 0.5, z / c - 0.5];
  const [i, j] = [Math.floor(fx), Math.floor(fz)];
  const [tx, tz] = [fx - i, fz - j];
  const oben = zelle(i, j) * (1 - tx) + zelle(i + 1, j) * tx;
  const unten = zelle(i, j + 1) * (1 - tx) + zelle(i + 1, j + 1) * tx;
  return oben * (1 - tz) + unten * tz + 1;
}

/** Ein Kreis als Vieleck mit Seiten von höchstens c Blöcken, mindestens acht. */
export function vieleck([mx, mz]: Punkt, r: number, c: number): Punkt[] {
  const n = Math.max(8, Math.ceil((2 * Math.PI * r) / c));
  return Array.from({ length: n }, (_, i): Punkt => [mx + r * Math.cos((2 * Math.PI * i) / n), mz + r * Math.sin((2 * Math.PI * i) / n)]);
}

/**
 * Tastet einen Linienzug an den Zellen ab: je Strecke der Anfang, ihre
 * Schnitte mit den Linien durch die Mitten der Zellen und je ein Punkt
 * dazwischen. Geschlossen kommt die Strecke zurück zum ersten Punkt dazu.
 */
export function abtasten(punkte: readonly Punkt[], geschlossen: boolean, c: number): Punkt[] {
  const aus: Punkt[] = [];
  const n = punkte.length;
  for (let i = 0; i < (geschlossen ? n : n - 1); i++) {
    const [a, b] = [punkte[i]!, punkte[(i + 1) % n]!];
    const ts = [0, 1];
    for (const achse of [0, 1] as const) {
      const [p, q] = [a[achse], b[achse]];
      if (p === q) continue;
      const [lo, hi] = [Math.min(p, q), Math.max(p, q)];
      for (let m = Math.ceil(lo / c - 0.5); (m + 0.5) * c < hi; m++) {
        const linie = (m + 0.5) * c;
        if (linie > lo) ts.push((linie - p) / (q - p));
      }
    }
    ts.sort((s, t) => s - t);
    const auf = (t: number): Punkt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    for (let j = 0; j < ts.length - 1; j++) {
      if (ts[j + 1]! - ts[j]! < 1e-12) continue;
      aus.push(auf(ts[j]!), auf((ts[j]! + ts[j + 1]!) / 2));
    }
  }
  if (n > 0) aus.push(geschlossen ? punkte[0]! : punkte[n - 1]!);
  return aus;
}

/**
 * Liegt Gelände vor dem Punkt (x, y, z)? Geht den Strahl zur Kamera ab,
 * ab mehr als einer Zelle vor dem Punkt, bis über das höchste Gelände.
 * Von oben verdeckt nichts. Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
 */
export function verdeckt(g: Gelaende, x: number, y: number, z: number, { p, k }: Blick): boolean {
  if (p.y === 0) return false;
  const genordet = p.azimuth === 'north';
  // Zur Kamera: im Blick (1, 1) oder genordet (0, 1); y steigt dabei so,
  // dass der Bildpunkt bleibt. Zurück in die Welt mit (x, z) ← (−z, x).
  let [wx, wz] = genordet ? [0, 1] : [1, 1];
  for (let i = 0; i < k; i++) [wx, wz] = [-wz, wx];
  const steigung = ((genordet ? 1 : 2) * p.v) / p.y;
  const schritt = g.c / 2;
  // Mehr als eine Zelle vor dem Punkt, in Blöcken auf dem Boden gemessen.
  const anfang = g.c / Math.hypot(wx, wz) + 1e-6;
  for (let d = anfang; y + steigung * d <= g.max; d += schritt) {
    if (oberflaeche(g, x + wx * d, z + wz * d) > y + steigung * d) return true;
  }
  return false;
}

/**
 * Vereinfacht einen Linienzug nach Douglas–Peucker: Kein weggelassener
 * Punkt liegt weiter als `toleranz` vom Zug. Anfang und Ende bleiben.
 */
export function vereinfache(punkte: readonly Punkt[], toleranz: number): Punkt[] {
  const n = punkte.length;
  if (n < 3) return [...punkte];
  const behalten = new Uint8Array(n);
  behalten[0] = behalten[n - 1] = 1;
  const offen: [number, number][] = [[0, n - 1]];
  while (offen.length) {
    const [von, bis] = offen.pop()!;
    const [a, b] = [punkte[von]!, punkte[bis]!];
    const [dx, dz] = [b[0] - a[0], b[1] - a[1]];
    const laenge = Math.hypot(dx, dz);
    let weitester = -1;
    let abstand = toleranz;
    for (let i = von + 1; i < bis; i++) {
      const [px, pz] = [punkte[i]![0] - a[0], punkte[i]![1] - a[1]];
      const d = laenge ? Math.abs(px * dz - pz * dx) / laenge : Math.hypot(px, pz);
      if (d > abstand) [weitester, abstand] = [i, d];
    }
    if (weitester < 0) continue;
    behalten[weitester] = 1;
    offen.push([von, weitester], [weitester, bis]);
  }
  return punkte.filter((_, i) => behalten[i]);
}

/** Behält vom Ring, was auf der Seite `seite` der Grenze liegt (Sutherland–Hodgman). */
function halbe(ring: readonly Punkt[], achse: 0 | 1, grenze: number, seite: 1 | -1): Punkt[] {
  const aus: Punkt[] = [];
  for (let i = 0; i < ring.length; i++) {
    const [a, b] = [ring[i]!, ring[(i + 1) % ring.length]!];
    const [da, db] = [seite * (a[achse] - grenze), seite * (b[achse] - grenze)];
    if (da >= 0) aus.push(a);
    if (da >= 0 !== db >= 0) {
      const t = da / (da - db);
      const schnitt: Punkt = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
      schnitt[achse] = grenze;
      aus.push(schnitt);
    }
  }
  return aus;
}

/** Der Ring zwischen `lo` und `hi` entlang einer Achse. */
function zwischen(ring: readonly Punkt[], achse: 0 | 1, lo: number, hi: number): Punkt[] {
  return halbe(halbe(ring, achse, lo, 1), achse, hi, -1);
}

/** Ein Ring mit je einem Punkt in der Mitte jeder Kante. */
function mitMitten(ring: readonly Punkt[]): Punkt[] {
  return ring.flatMap((a, i): Punkt[] => {
    const b = ring[(i + 1) % ring.length]!;
    return [a, [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]];
  });
}

/**
 * Das Netz einer Fläche in Pixeln der feinsten Stufe: Ringe, die gerade/
 * ungerade gefüllt die Fläche auf dem Gelände ergeben, ohne das Verdeckte.
 *
 * Geschnitten wird an den Linien durch die Mitten der Zellen, denn
 * dazwischen mischt `H` bilinear. Ein Feld, durch das kein Rand geht, liegt
 * ganz drinnen oder draussen; die drinnen fassen sich je Reihe zu einem
 * Streifen zusammen, dessen Umriss jede Ecke der Felder trägt. Ein Feld mit
 * Rand gibt seine Stücke einzeln. Verdeckt ist ein Feld, wenn seine Mitte es
 * ist. Zuletzt fällt weg, was weniger als `toleranz` Pixel beiträgt.
 * Siehe docs/benutzung/ebenen.md, „Flächen“.
 */
export function netz(polygone: readonly Polygon[], g: Gelaende, blick: Blick, toleranz: number): Punkt[][] {
  const ringe = polygone.flatMap((p) => [p.aussen, ...p.loecher]).filter((r) => r.length >= 3);
  const aus: Punkt[][] = [];
  const projiziert = (ring: readonly Punkt[]): void => {
    const punkte = ring.map(([x, z]) => bildpunkt(x, oberflaeche(g, x, z), z, blick));
    // Geschlossen vereinfachen: Der erste Punkt steht am Ende noch einmal.
    const zu = vereinfache([...punkte, punkte[0]!], toleranz);
    zu.pop();
    if (zu.length >= 3) aus.push(zu);
  };
  if (ringe.length === 0) return aus;
  // Von oben liegt alles eben: die Ringe selbst.
  if (blick.p.y === 0) return ringe.map((r) => r.map(([x, z]) => bildpunkt(x, 0, z, blick)));
  const { c } = g;
  const o = c / 2;
  let [xmin, xmax, zmin, zmax] = [Infinity, -Infinity, Infinity, -Infinity];
  for (const r of ringe) {
    for (const [x, z] of r) [xmin, xmax, zmin, zmax] = [Math.min(xmin, x), Math.max(xmax, x), Math.min(zmin, z), Math.max(zmax, z)];
  }
  const [pa, pb] = [Math.floor((xmin - o) / c), Math.floor((xmax - o) / c)];
  const sichtbar = (mx: number, mz: number) => !verdeckt(g, mx, oberflaeche(g, mx, mz), mz, blick);

  for (let q = Math.floor((zmin - o) / c); q <= Math.floor((zmax - o) / c); q++) {
    const [z0, z1] = [q * c + o, (q + 1) * c + o];
    const streifen = ringe.map((r) => zwischen(r, 1, z0, z1)).filter((r) => r.length >= 3);
    if (streifen.length === 0) continue;
    // Felder mit Rand: Kanten auf dem Rand des Streifens kommen vom Schneiden.
    const rand = new Uint8Array(pb - pa + 1);
    for (const r of streifen) {
      for (let i = 0; i < r.length; i++) {
        const [a, b] = [r[i]!, r[(i + 1) % r.length]!];
        if (a[1] === b[1] && (a[1] === z0 || a[1] === z1)) continue;
        const von = Math.max(pa, Math.floor((Math.min(a[0], b[0]) - o) / c));
        const bis = Math.min(pb, Math.floor((Math.max(a[0], b[0]) - o) / c));
        for (let p = von; p <= bis; p++) rand[p - pa] = 1;
      }
    }
    // Drinnen ohne Rand: gerade/ungerade an der Mittellinie der Reihe.
    const zm = z0 + o;
    const kreuzungen: number[] = [];
    for (const r of ringe) {
      for (let i = 0; i < r.length; i++) {
        const [a, b] = [r[i]!, r[(i + 1) % r.length]!];
        if (a[1] <= zm !== b[1] <= zm) kreuzungen.push(a[0] + ((zm - a[1]) / (b[1] - a[1])) * (b[0] - a[0]));
      }
    }
    kreuzungen.sort((s, t) => s - t);
    let gezaehlt = 0;
    let lauf: number | undefined;
    const schliesse = (bis: number): void => {
      if (lauf === undefined) return;
      const [xa, xb] = [lauf * c + o, bis * c + o];
      const ring: Punkt[] = [];
      for (let x = xa; x < xb; x += o) ring.push([x, z0]);
      ring.push([xb, z0], [xb, zm]);
      for (let x = xb; x > xa; x -= o) ring.push([x, z1]);
      ring.push([xa, z1], [xa, zm]);
      projiziert(ring);
      lauf = undefined;
    };
    for (let p = pa; p <= pb; p++) {
      const [mx, xl, xr] = [(p + 1) * c, p * c + o, (p + 1) * c + o];
      while (gezaehlt < kreuzungen.length && kreuzungen[gezaehlt]! < mx) gezaehlt++;
      if (rand[p - pa]) {
        schliesse(p);
        if (!sichtbar(mx, zm)) continue;
        for (const r of streifen) {
          const stueck = zwischen(r, 0, xl, xr);
          if (stueck.length >= 3) projiziert(mitMitten(stueck));
        }
      } else if (gezaehlt % 2 === 1 && sichtbar(mx, zm)) {
        lauf ??= p;
      } else {
        schliesse(p);
      }
    }
    schliesse(pb + 1);
  }
  return aus;
}

/** Ein Linienzug auf dem Gelände: Bildpunkte und je Punkt, ob er verdeckt ist. */
export interface Zug {
  punkte: Punkt[];
  verdeckt: boolean[];
}

/**
 * Ein Rand oder eine Linie auf dem Gelände. Von oben bleibt der Zug, wie er
 * ist, und liegt eben. Siehe docs/benutzung/ebenen.md, „Ränder, Linien und
 * Kreise“.
 */
export function zug(punkte: readonly Punkt[], geschlossen: boolean, g: Gelaende, blick: Blick): Zug {
  if (blick.p.y === 0) {
    const ebene = geschlossen && punkte.length ? [...punkte, punkte[0]!] : [...punkte];
    return { punkte: ebene.map(([x, z]) => bildpunkt(x, 0, z, blick)), verdeckt: ebene.map(() => false) };
  }
  const abgetastet = abtasten(punkte, geschlossen, g.c);
  const hoehen = abgetastet.map(([x, z]) => oberflaeche(g, x, z));
  return {
    punkte: abgetastet.map(([x, z], i) => bildpunkt(x, hoehen[i]!, z, blick)),
    verdeckt: abgetastet.map(([x, z], i) => verdeckt(g, x, hoehen[i]!, z, blick)),
  };
}

/**
 * Der Pfad einer Kartenschrift in Pixeln der feinsten Stufe: abgetastet wie
 * ein Rand, die Höhen gleitend über 32 Blöcke des Pfads gemittelt, damit die
 * Schrift nicht mit jeder Kuppe springt. Von oben eben. Siehe
 * docs/benutzung/ebenen.md, „Kartenschrift“ unter „Zeichnen“.
 */
export function schriftPfad(pfad: readonly Punkt[], g: Gelaende, blick: Blick): Punkt[] {
  if (blick.p.y === 0) return pfad.map(([x, z]) => bildpunkt(x, 0, z, blick));
  const punkte = pfad.length > 1 ? abtasten(pfad, false, g.c) : [...pfad];
  const hoehen = punkte.map(([x, z]) => oberflaeche(g, x, z));
  const weg = [0];
  for (let i = 1; i < punkte.length; i++) weg.push(weg[i - 1]! + Math.hypot(punkte[i]![0] - punkte[i - 1]![0], punkte[i]![1] - punkte[i - 1]![1]));
  const gemittelt: number[] = [];
  let [von, bis, summe] = [0, 0, 0];
  for (let i = 0; i < punkte.length; i++) {
    for (; bis < punkte.length && weg[bis]! <= weg[i]! + 16; bis++) summe += hoehen[bis]!;
    for (; weg[von]! < weg[i]! - 16; von++) summe -= hoehen[von]!;
    gemittelt.push(summe / (bis - von));
  }
  return punkte.map(([x, z], i) => bildpunkt(x, gemittelt[i]!, z, blick));
}
