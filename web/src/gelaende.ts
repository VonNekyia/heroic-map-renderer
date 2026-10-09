/**
 * Formen der Ebenen auf dem Gelände: Abtasten, Höhen, was verdeckt ist und
 * das Netz der Flächen. Rechnet in Blöcken der Welt und in Pixeln der
 * feinsten Stufe. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/benutzung/ebenen.md, „Zeichnen“.
 */
import { projiziere, type Projektion } from './pick';

/** Ein Punkt `[x, z]` in Blöcken, oder `[px, py]` in Pixeln der feinsten Stufe. */
export type Punkt = [number, number];

/** Ein Rechteck `[x0, z0, x1, z1]` der Welt in Blöcken. */
export type Rechteck = [number, number, number, number];

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

/** Das Rechteck um Punkte. */
export function rechteck(punkte: Iterable<Punkt>): Rechteck {
  let [x0, z0, x1, z1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const [x, z] of punkte) [x0, z0, x1, z1] = [Math.min(x0, x), Math.min(z0, z), Math.max(x1, x), Math.max(z1, z)];
  return [x0, z0, x1, z1];
}

/**
 * Zur Kamera in der Welt: je Schritt (wx, wz), dabei steigt der Strahl um
 * `steigung` je Block in x oder z, so dass der Bildpunkt bleibt. `kmin`
 * Schritte vor dem Punkt liegt die erste Mitte, die mehr als eine Zelle
 * entfernt ist: diagonal schon die nächste, genordet die übernächste. Nur im
 * iso (b > 0). Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
 */
export function zurKamera({ p, k }: Blick): { wx: number; wz: number; steigung: number; kmin: number } {
  const genordet = p.azimuth === 'north';
  // Im Blick (1, 1) oder genordet (0, 1); zurück in die Welt mit (x, z) ← (−z, x).
  let [wx, wz] = genordet ? [0, 1] : [1, 1];
  for (let i = 0; i < k; i++) [wx, wz] = [-wz, wx];
  return { wx, wz, steigung: ((genordet ? 1 : 2) * p.v) / p.y, kmin: genordet ? 2 : 1 };
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
 * Liegt Gelände vor dem Punkt (x, y, z)? Tastet `H` entlang des Strahls zur
 * Kamera in Schritten einer halben Zelle ab, ab der ersten Mitte, die mehr
 * als eine Zelle vor dem Punkt liegt, bis über das höchste Gelände. Von oben
 * verdeckt nichts. Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
 */
export function verdeckt(g: Gelaende, x: number, y: number, z: number, blick: Blick): boolean {
  if (blick.p.y === 0) return false;
  const { wx, wz, steigung, kmin } = zurKamera(blick);
  for (let d = kmin * g.c; y + steigung * d <= g.max; d += g.c / 2) {
    if (oberflaeche(g, x + wx * d, z + wz * d) > y + steigung * d) return true;
  }
  return false;
}

/**
 * Welche Felder sichtbar sind, in einem Durchgang je Linie zur Kamera. Ein
 * Feld ist das Quadrat zwischen vier Mitten von Zellen; Feld (p, q) hat
 * seine Mitte bei ((p + 1)·c, (q + 1)·c). Es ist verdeckt, wenn die Mitte
 * eines Felds mindestens `kmin` Schritte davor höher liegt als der Strahl:
 * `H(t) + steigung·c·t` mit t der Schritte von der Kamera weg, laufend als
 * Maximum gehalten. Gibt je Feld des Rechtecks [pa, qa] + [nx, nz] 1 für
 * sichtbar; `gefragt` wählt die Felder, die zählen.
 */
export function sichtbareFelder(g: Gelaende, blick: Blick, pa: number, qa: number, nx: number, nz: number, gefragt: (i: number) => boolean): Uint8Array {
  const sicht = new Uint8Array(nx * nz);
  const mitte = (p: number, q: number) => oberflaeche(g, (p + 1) * g.c, (q + 1) * g.c);
  let tiefste = Infinity;
  for (let i = 0; i < sicht.length; i++) if (gefragt(i)) tiefste = Math.min(tiefste, mitte(pa + Math.floor(i / nz), qa + (i % nz)));
  if (tiefste === Infinity) return sicht;
  const { wx, wz, steigung, kmin } = zurKamera(blick);
  const schritt = steigung * g.c;
  // So viele Schritte zur Kamera, bis der Strahl über dem höchsten Gelände liegt.
  const weit = Math.max(kmin, Math.ceil((g.max - tiefste) / schritt) + 1);
  const [ea, eb] = [pa + Math.min(0, wx * weit), pa + nx - 1 + Math.max(0, wx * weit)];
  const [fa, fb] = [qa + Math.min(0, wz * weit), qa + nz - 1 + Math.max(0, wz * weit)];
  const [ex, ez] = [eb - ea + 1, fb - fa + 1];
  // t wächst je Schritt von der Kamera weg um 1.
  const t = (p: number, q: number) => -(p * wx + q * wz) / (wx * wx + wz * wz);
  const maximum = new Float64Array(ex * ez);
  const reihe = (a: number, b: number, w: number) => (w > 0 ? { von: b, bis: a - 1, d: -1 } : { von: a, bis: b + 1, d: 1 });
  const [rp, rq] = [reihe(ea, eb, wx), reihe(fa, fb, wz)];
  for (let p = rp.von; p !== rp.bis; p += rp.d) {
    for (let q = rq.von; q !== rq.bis; q += rq.d) {
      const [vp, vq] = [p + wx, q + wz];
      const davor = vp >= ea && vp <= eb && vq >= fa && vq <= fb ? maximum[(vp - ea) * ez + (vq - fa)]! : -Infinity;
      maximum[(p - ea) * ez + (q - fa)] = Math.max(mitte(p, q) + schritt * t(p, q), davor);
    }
  }
  for (let i = 0; i < sicht.length; i++) {
    if (!gefragt(i)) continue;
    const [p, q] = [pa + Math.floor(i / nz), qa + (i % nz)];
    const [vp, vq] = [p + kmin * wx, q + kmin * wz];
    const vorn = vp >= ea && vp <= eb && vq >= fa && vq <= fb ? maximum[(vp - ea) * ez + (vq - fa)]! : -Infinity;
    sicht[i] = vorn > mitte(p, q) + schritt * t(p, q) ? 0 : 1;
  }
  return sicht;
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

/** Ein Ring, beschnitten auf ein Rechteck. Für Flächen; ein Rand bekäme so falsche Kanten, dafür `zugImRechteck`. */
export function ringImRechteck(ring: readonly Punkt[], [x0, z0, x1, z1]: Rechteck): Punkt[] {
  return zwischen(zwischen(ring, 0, x0, x1), 1, z0, z1);
}

/** Die Stücke eines Linienzugs innerhalb eines Rechtecks (Liang–Barsky je Strecke). */
export function zugImRechteck(punkte: readonly Punkt[], geschlossen: boolean, [x0, z0, x1, z1]: Rechteck): Punkt[][] {
  const aus: Punkt[][] = [];
  let lauf: Punkt[] | undefined;
  const n = punkte.length;
  for (let i = 0; i < (geschlossen ? n : n - 1); i++) {
    const [a, b] = [punkte[i]!, punkte[(i + 1) % n]!];
    const [dx, dz] = [b[0] - a[0], b[1] - a[1]];
    let [t0, t1] = [0, 1];
    for (const [p, q] of [[-dx, a[0] - x0], [dx, x1 - a[0]], [-dz, a[1] - z0], [dz, z1 - a[1]]] as const) {
      if (p === 0) {
        if (q < 0) t0 = 2;
        continue;
      }
      const r = q / p;
      if (p < 0) t0 = Math.max(t0, r);
      else t1 = Math.min(t1, r);
    }
    if (t0 > t1) {
      lauf = undefined;
      continue;
    }
    const anfang: Punkt = [a[0] + dx * t0, a[1] + dz * t0];
    if (!lauf || t0 > 0) {
      lauf = [anfang];
      aus.push(lauf);
    }
    lauf.push([a[0] + dx * t1, a[1] + dz * t1]);
    if (t1 < 1) lauf = undefined;
  }
  return aus.filter((l) => l.length >= 2);
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
 * dazwischen mischt `H` bilinear. Ein Feld ohne Rand der Fläche liegt ganz
 * drinnen oder draussen. Die sichtbaren Felder ganz drinnen ergeben
 * zusammen einen Umriss: Was sichtbar ist, deckt auf dem Schirm genau einen
 * Punkt des Geländes, also umschliesst der projizierte Umriss ihr Bild.
 * Ein Feld mit Rand gibt seine Stücke einzeln, jede Kante mit ihrer Mitte.
 * Kosten wachsen mit dem Umfang, nicht mit der Fläche. Siehe
 * docs/entscheidungen/0096-formen-und-schrift-im-browser.md.
 */
export function netz(polygone: readonly Polygon[], g: Gelaende, blick: Blick): Punkt[][] {
  const ringe = polygone.flatMap((p) => [p.aussen, ...p.loecher]).filter((r) => r.length >= 3);
  const projiziert = (ring: readonly Punkt[]): Punkt[] => ring.map(([x, z]) => bildpunkt(x, oberflaeche(g, x, z), z, blick));
  if (ringe.length === 0) return [];
  // Von oben liegt alles eben: die Ringe selbst.
  if (blick.p.y === 0) return ringe.map((r) => r.map(([x, z]) => bildpunkt(x, 0, z, blick)));
  const { c } = g;
  const o = c / 2;
  const [xmin, zmin, xmax, zmax] = rechteck(ringe.flat());
  const [pa, qa] = [Math.floor((xmin - o) / c), Math.floor((zmin - o) / c)];
  const [nx, nz] = [Math.floor((xmax - o) / c) - pa + 1, Math.floor((zmax - o) / c) - qa + 1];
  // Je Feld: 0 draussen, 1 ganz drinnen, 2 mit Rand; dazu die Stücke der Felder mit Rand.
  const art = new Uint8Array(nx * nz);
  const stuecke = new Map<number, Punkt[][]>();
  for (let q = qa; q < qa + nz; q++) {
    const [z0, z1] = [q * c + o, (q + 1) * c + o];
    const streifen = ringe.map((r) => zwischen(r, 1, z0, z1)).filter((r) => r.length >= 3);
    if (streifen.length === 0) continue;
    // Felder mit Rand: deren Inneres eine Kante berührt. Kanten auf dem Rand
    // des Streifens kommen vom Schneiden; eine Kante genau auf der Grenze
    // zweier Felder berührt keines von innen.
    const rand = new Uint8Array(nx);
    for (const r of streifen) {
      for (let i = 0; i < r.length; i++) {
        const [a, b] = [r[i]!, r[(i + 1) % r.length]!];
        if (a[1] === b[1] && (a[1] === z0 || a[1] === z1)) continue;
        const [lo, hi] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
        const von = Math.max(pa, Math.floor((lo - o) / c));
        const bis = Math.min(pa + nx - 1, lo === hi && (lo - o) % c === 0 ? von - 1 : Math.ceil((hi - o) / c) - 1);
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
    for (let p = pa; p < pa + nx; p++) {
      const i = (p - pa) * nz + (q - qa);
      while (gezaehlt < kreuzungen.length && kreuzungen[gezaehlt]! < (p + 1) * c) gezaehlt++;
      if (rand[p - pa]) {
        const teile = streifen.map((r) => zwischen(r, 0, p * c + o, (p + 1) * c + o)).filter((r) => r.length >= 3);
        if (teile.length) {
          art[i] = 2;
          stuecke.set(i, teile);
        }
      } else if (gezaehlt % 2 === 1) {
        art[i] = 1;
      }
    }
  }
  const sicht = sichtbareFelder(g, blick, pa, qa, nx, nz, (i) => art[i] !== 0);
  const aus: Punkt[][] = [];
  for (const [i, teile] of stuecke) if (sicht[i]) for (const t of teile) aus.push(projiziert(mitMitten(t)));
  // Der Umriss der sichtbaren Felder ganz drinnen: jede Kante, hinter der kein solches Feld liegt.
  const voll = (p: number, q: number) => p >= 0 && p < nx && q >= 0 && q < nz && art[p * nz + q] === 1 && sicht[p * nz + q] === 1;
  const ecke = (p: number, q: number) => p * (nz + 1) + q;
  const weiter = new Map<number, number[]>();
  const kante = (a: number, b: number) => {
    const liste = weiter.get(a);
    if (liste) liste.push(b);
    else weiter.set(a, [b]);
  };
  for (let p = 0; p < nx; p++) {
    for (let q = 0; q < nz; q++) {
      if (!voll(p, q)) continue;
      if (!voll(p, q - 1)) kante(ecke(p, q), ecke(p + 1, q));
      if (!voll(p + 1, q)) kante(ecke(p + 1, q), ecke(p + 1, q + 1));
      if (!voll(p, q + 1)) kante(ecke(p + 1, q + 1), ecke(p, q + 1));
      if (!voll(p - 1, q)) kante(ecke(p, q + 1), ecke(p, q));
    }
  }
  // Zu Ringen verbinden; wo sich zwei berühren, gleich welcher: gerade/ungerade hängt nur an den Kanten.
  const welt = (e: number): Punkt => [(pa + Math.floor(e / (nz + 1))) * c + o, (qa + (e % (nz + 1))) * c + o];
  for (const [start, ziele] of weiter) {
    while (ziele.length) {
      const ring: Punkt[] = [];
      let von = start;
      do {
        const nach = weiter.get(von)!.pop()!;
        ring.push(welt(von));
        von = nach;
      } while (von !== start);
      // Die Kanten laufen von Mitte zu Mitte der Zellen; dort ist H linear, die Projektion gerade: Die Ecken reichen.
      aus.push(projiziert(ring));
    }
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
