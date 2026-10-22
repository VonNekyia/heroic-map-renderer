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

/**
 * So viele Seiten hat ein Kreis höchstens: mit Gelände folgt er den Zellen,
 * ohne ist er ein 128-Eck. Sonst gäbe ein grosser Kreis Zehntausende.
 */
const KREIS = { seiten: 512, ohneGelaende: 128 };

/** Ein Kreis als Vieleck mit Seiten von einer Zelle, höchstens `KREIS.seiten`, ohne Gelände `KREIS.ohneGelaende`. */
export const kreisRing = (mitte: Punkt, r: number, c: number, eben: boolean): Punkt[] =>
  vieleck(mitte, r, Math.max(c, (2 * Math.PI * r) / (eben ? KREIS.ohneGelaende : KREIS.seiten)));

/** Ein Kreis als Vieleck mit Seiten von höchstens c Blöcken, mindestens acht. */
export function vieleck([mx, mz]: Punkt, r: number, c: number): Punkt[] {
  const n = Math.max(8, Math.ceil((2 * Math.PI * r) / c));
  return Array.from({ length: n }, (_, i): Punkt => [mx + r * Math.cos((2 * Math.PI * i) / n), mz + r * Math.sin((2 * Math.PI * i) / n)]);
}

/**
 * So viele Zellen weit tastet ein Zug höchstens ab; ein längerer nimmt
 * gröbere Schritte. Sonst gäbe ein grosser Kreis Millionen Punkte. Siehe
 * docs/frontend.md, „Ebenen“.
 */
const PROBEN = 512;

/**
 * Tastet einen Linienzug an den Zellen ab: je Strecke der Anfang, ihre
 * Schnitte mit den Linien durch die Mitten der Zellen und je ein Punkt
 * dazwischen. Geschlossen kommt die Strecke zurück zum ersten Punkt dazu.
 * Ist der Zug länger als `PROBEN` Zellen, schneidet er an Linien im Abstand
 * Länge / `PROBEN` statt `c`.
 */
export function abtasten(punkte: readonly Punkt[], geschlossen: boolean, zelle: number, deckel = PROBEN): Punkt[] {
  const aus: Punkt[] = [];
  const n = punkte.length;
  let laenge = 0;
  for (let i = 0; i < (geschlossen ? n : n - 1); i++) laenge += Math.hypot(punkte[(i + 1) % n]![0] - punkte[i]![0], punkte[(i + 1) % n]![1] - punkte[i]![1]);
  const c = Math.max(zelle, laenge / deckel);
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
 * Was die Regel aus 0111 von der Ansicht braucht: `faktor` Pixel des
 * Schirms je Pixel der feinsten Stufe, `rahmen` der Ausschnitt samt Rand in
 * Pixeln der feinsten Stufe, `eps` der erlaubte Fehler auf dem Schirm in
 * Pixeln. Siehe docs/entscheidungen/0111-geometrie-nach-bildschirmfehler.md.
 */
export interface Schirm {
  faktor: number;
  rahmen: Rechteck;
  eps: number;
}

/** Der Abstand des Punkts p von der Strecke a–b. */
function abstand(p: Punkt, a: Punkt, b: Punkt): number {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const l2 = dx * dx + dy * dy;
  const t = l2 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy);
}

/** Wie viele Pixel der feinsten Stufe ein Block längs x oder z höchstens breit ist. */
export function blockBreite(blick: Blick): number {
  const o = bildpunkt(0, 0, 0, blick);
  return Math.max(...[bildpunkt(1, 0, 0, blick), bildpunkt(0, 0, 1, blick)].map(([x, y]) => Math.hypot(x - o[0], y - o[1])));
}

/**
 * Das Rechteck der Welt, dessen Punkte zwischen den Höhen `ymin` und `ymax`
 * im Rahmen `rahmen` der feinsten Stufe liegen können: die Ecken des Rahmens
 * auf beiden Höhen zurückgerechnet.
 */
export function weltRechteck([x0, y0, x1, y1]: Rechteck, blick: Blick, ymin: number, ymax: number): Rechteck {
  const o = bildpunkt(0, 0, 0, blick);
  const [ex, ez, ey] = [bildpunkt(1, 0, 0, blick), bildpunkt(0, 0, 1, blick), bildpunkt(0, 1, 0, blick)].map(([a, b]): Punkt => [a - o[0], b - o[1]]) as [Punkt, Punkt, Punkt];
  const det = ex[0] * ez[1] - ex[1] * ez[0];
  const welt = ([sx, sy]: Punkt, y: number): Punkt => {
    const [dx, dy] = [sx - o[0] - y * ey[0], sy - o[1] - y * ey[1]];
    return [(dx * ez[1] - dy * ez[0]) / det, (ex[0] * dy - ex[1] * dx) / det];
  };
  const ecken: Punkt[] = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]];
  return rechteck([ymin, ymax].flatMap((y) => ecken.map((e) => welt(e, y))));
}

/** So breit ist ein Feld des Netzes nach 0111 auf dem Schirm mindestens, in Pixeln; siehe `feldKante`. Gewählt in 0111, „Flächen“. */
const FELD_PX = 16;

/**
 * Die Kante eines Felds im Netz nach 0111: die Zelle, so oft verdoppelt, bis
 * das Feld auf dem Schirm `FELD_PX` breit ist. Feiner gäbe jedes Feld am Rand
 * sein eigenes Stück, mehr Punkte als der Rand nach ε.
 */
export function feldKante(c: number, blick: Blick, s: Schirm): number {
  const px = c * blockBreite(blick) * s.faktor;
  return px >= FELD_PX ? c : c * 2 ** Math.ceil(Math.log2(FELD_PX / px));
}

/** Die Höhen in Zellen der Kante `kante`, je Zelle die Oberfläche in ihrer Mitte; für die Verdeckung im groben Netz. */
function grob(g: Gelaende, kante: number): Gelaende {
  const gemerkt = new Map<number, number>();
  const zelle = (i: number, j: number): number => {
    const s = (i + 2 ** 20) * 2 ** 21 + (j + 2 ** 20);
    let h = gemerkt.get(s);
    if (h === undefined) gemerkt.set(s, (h = oberflaeche(g, (i + 0.5) * kante, (j + 0.5) * kante) - 1));
    return h;
  };
  return { ...g, c: kante, zelle };
}

/** Seiten eines Kreises mit `r` Pixeln Radius auf dem Schirm, so dass die Pfeilhöhe jeder Seite unter `eps` bleibt; mindestens 12. */
export function kreisSeiten(r: number, eps: number): number {
  return r <= eps ? 12 : Math.max(12, Math.ceil(Math.PI / Math.acos(1 - eps / r)));
}

/** Ein Kreis nach 0111: so viele Seiten, wie sein Radius auf dem Schirm braucht. */
export function kreisNachSchirm([mx, mz]: Punkt, r: number, blick: Blick, s: Schirm): Punkt[] {
  const n = kreisSeiten(r * blockBreite(blick) * s.faktor, s.eps);
  return Array.from({ length: n }, (_, i): Punkt => [mx + r * Math.cos((2 * Math.PI * i) / n), mz + r * Math.sin((2 * Math.PI * i) / n)]);
}

/**
 * Lässt von Punkten auf dem Schirm nur die, ohne die der Zug mehr als `eps`
 * abwiche (Douglas-Peucker); Anfang, Ende und die Indizes in `fest` bleiben,
 * und zwischen ihnen wird je für sich vereinfacht. Gibt die Indizes.
 */
export function vereinfache(bild: readonly Punkt[], eps: number, fest: Iterable<number> = []): number[] {
  const behalte = new Uint8Array(bild.length);
  if (bild.length) [behalte[0], behalte[bild.length - 1]] = [1, 1];
  for (const i of fest) behalte[i] = 1;
  const marken = [...behalte.keys()].filter((i) => behalte[i]);
  const offen: [number, number][] = marken.slice(1).map((bis, j): [number, number] => [marken[j]!, bis]);
  while (offen.length) {
    const [von, bis] = offen.pop()!;
    let [weit, wo] = [0, -1];
    for (let i = von + 1; i < bis; i++) {
      const d = abstand(bild[i]!, bild[von]!, bild[bis]!);
      if (d > weit) [weit, wo] = [d, i];
    }
    if (weit <= eps) continue;
    behalte[wo] = 1;
    offen.push([von, wo], [wo, bis]);
  }
  return [...behalte.keys()].filter((i) => behalte[i]);
}

/**
 * Tastet einen Zug für 0111 ab: Strecken, die im Rahmen liegen können, bei
 * jeder Höhe des Geländes, an den Zellen wie `abtasten`; Strecken ausserhalb
 * nur mit ihrem Anfang. Geschlossen kommt die Strecke zurück zum ersten
 * Punkt dazu. Vereinfacht wird erst nach Glätten und Verdeckung, in `zug`.
 */
export function abtastenImRahmen(punkte: readonly Punkt[], geschlossen: boolean, g: Gelaende, blick: Blick, s: Schirm): Punkt[] {
  const [r0, r1, r2, r3] = s.rahmen;
  // Gelände kann unter grund liegen, im Meer und in Schluchten; 128 Blöcke reichen bis zum Boden der Welt.
  const imRahmen = (a: Punkt, b: Punkt): boolean => {
    const [x0, y0, x1, y1] = rechteck([a, b].flatMap(([x, z]) => [bildpunkt(x, g.grund - 128, z, blick), bildpunkt(x, g.max, z, blick)]));
    return x1 >= r0 && x0 <= r2 && y1 >= r1 && y0 <= r3;
  };
  const aus: Punkt[] = [];
  const n = punkte.length;
  for (let i = 0; i < (geschlossen ? n : n - 1); i++) {
    const [a, b] = [punkte[i]!, punkte[(i + 1) % n]!];
    // Ohne das Ende: Es ist der Anfang der nächsten Strecke.
    if (imRahmen(a, b)) aus.push(...abtasten([a, b], false, g.c, Infinity).slice(0, -1));
    else aus.push(a);
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
 * seine Mitte bei ((p + 1)·c, (q + 1)·c). Sichtbar heisst: `verdeckt` gibt
 * an seiner Mitte `false`. Dieselben Punkte wie dort, die Mitten der Felder
 * auf der Linie und je die halbe Zelle davor, gehen mit
 * `H + steigung·c·t` in ein laufendes Maximum, t die Schritte von der
 * Kamera weg. Gibt je Feld des Rechtecks [pa, qa] + [nx, nz] 1 für
 * sichtbar; `gefragt` wählt die Felder, die zählen.
 */
export function sichtbareFelder(g: Gelaende, blick: Blick, pa: number, qa: number, nx: number, nz: number, gefragt: (i: number) => boolean): Uint8Array {
  const sicht = new Uint8Array(nx * nz);
  const { c } = g;
  let tiefste = Infinity;
  for (let i = 0; i < sicht.length; i++) {
    if (gefragt(i)) tiefste = Math.min(tiefste, oberflaeche(g, (pa + Math.floor(i / nz) + 1) * c, (qa + (i % nz) + 1) * c));
  }
  if (tiefste === Infinity) return sicht;
  const { wx, wz, steigung, kmin } = zurKamera(blick);
  const schritt = steigung * c;
  // So viele Schritte zur Kamera, bis der Strahl über dem höchsten Gelände liegt.
  const weit = Math.max(kmin, Math.ceil((g.max - tiefste) / schritt) + 1);
  const [ea, eb] = [pa + Math.min(0, wx * weit), pa + nx - 1 + Math.max(0, wx * weit)];
  const [fa, fb] = [qa + Math.min(0, wz * weit), qa + nz - 1 + Math.max(0, wz * weit)];
  const ez = fb - fa + 1;
  const index = (p: number, q: number) => (p - ea) * ez + (q - fa);
  const drin = (p: number, q: number) => p >= ea && p <= eb && q >= fa && q <= fb;
  // t wächst je Schritt von der Kamera weg um 1.
  const t = (p: number, q: number) => -(p * wx + q * wz) / (wx * wx + wz * wz);
  // Von der Kamera weg, so steht das Feld davor schon fest. Je Mitte einmal
  // H, gemerkt; mit der halben Zelle davor geht sie ins laufende Maximum.
  const mitte = new Float64Array((eb - ea + 1) * ez);
  const maximum = new Float64Array(mitte.length);
  const reihe = (a: number, b: number, w: number) => (w > 0 ? { von: b, bis: a - 1, d: -1 } : { von: a, bis: b + 1, d: 1 });
  const [rp, rq] = [reihe(ea, eb, wx), reihe(fa, fb, wz)];
  for (let p = rp.von; p !== rp.bis; p += rp.d) {
    for (let q = rq.von; q !== rq.bis; q += rq.d) {
      const [x, z] = [(p + 1) * c, (q + 1) * c];
      const h = oberflaeche(g, x, z);
      const halb = oberflaeche(g, x + (wx * c) / 2, z + (wz * c) / 2);
      mitte[index(p, q)] = h;
      const davor = drin(p + wx, q + wz) ? maximum[index(p + wx, q + wz)]! : -Infinity;
      maximum[index(p, q)] = Math.max(h + schritt * t(p, q), halb + schritt * (t(p, q) - 0.5), davor);
    }
  }
  for (let i = 0; i < sicht.length; i++) {
    if (!gefragt(i)) continue;
    const [p, q] = [pa + Math.floor(i / nz), qa + (i % nz)];
    const [vp, vq] = [p + kmin * wx, q + kmin * wz];
    const vorn = drin(vp, vq) ? maximum[index(vp, vq)]! : -Infinity;
    sicht[i] = vorn > mitte[index(p, q)]! + schritt * t(p, q) ? 0 : 1;
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
 *
 * Mit `schirm` nach 0111: nur, was im Ausschnitt liegen kann, in Feldern
 * von `feldKante`, verdeckt nach der Oberfläche in den Mitten dieser Felder;
 * jede Kante an den Zellen abgetastet und auf dem Schirm auf ε vereinfacht.
 */
export function netz(polygone: readonly Polygon[], g: Gelaende, blick: Blick, schirm?: Schirm): Punkt[][] {
  let ringe = polygone.flatMap((p) => [p.aussen, ...p.loecher]).filter((r) => r.length >= 3);
  if (schirm) {
    const welt = weltRechteck(schirm.rahmen, blick, g.grund - 128, g.max);
    ringe = ringe.map((r) => ringImRechteck(r, welt)).filter((r) => r.length >= 3);
  }
  if (ringe.length === 0) return [];
  // Von oben liegt alles eben: die Ringe selbst.
  if (blick.p.y === 0) return ringe.map((r) => r.map(([x, z]) => bildpunkt(x, 0, z, blick)));
  const c = schirm ? feldKante(g.c, blick, schirm) : g.c;
  const gs = c === g.c ? g : grob(g, c);
  const o = c / 2;
  /** Ob v auf einer Linie zwischen den Feldern liegt. */
  const aufLinie = (v: number) => Number.isInteger((v - o) / c);
  // In groben Feldern liegt keine Kante der Fläche auf einer Linie zwischen ihnen: so ist jede ein Stück und wird abgetastet.
  if (c !== g.c) ringe = ringe.map((r) => r.map(([x, z]): Punkt => [aufLinie(x) ? x + 1e-6 : x, aufLinie(z) ? z + 1e-6 : z]));
  const bild = ([x, z]: Punkt): Punkt => bildpunkt(x, oberflaeche(g, x, z), z, blick);
  // Ein Stück mit seinen Mitten, wo `H` zwischen den Ecken bilinear ist. Nach 0111 jede Kante an den Zellen
  // abgetastet und vereinfacht, die Ecken fest: Zwei Felder teilen eine Kante, und nur mit festen Ecken
  // vereinfachen beide sie gleich; sonst bliebe zwischen ihnen ein Spalt. Eine Seite eines groben Felds liegt
  // nur innen oder an der Grenze zum Verdeckten, die den Feldern folgt; ihr reichen die Ecken.
  const projiziert = (ring: readonly Punkt[], mitten: boolean): Punkt[] => {
    if (!schirm) return (mitten ? mitMitten(ring) : ring).map(bild);
    const seite = (a: Punkt, b: Punkt) => c !== g.c && ((a[0] === b[0] && aufLinie(a[0])) || (a[1] === b[1] && aufLinie(a[1])));
    const dicht: Punkt[] = [];
    const fest: number[] = [];
    ring.forEach((a, i) => {
      const b = ring[(i + 1) % ring.length]!;
      fest.push(dicht.length);
      dicht.push(...(seite(a, b) ? [a] : abtasten([a, b], false, g.c, Infinity).slice(0, -1)).map(bild));
    });
    dicht.push(dicht[0]!);
    return vereinfache(dicht, schirm.eps / schirm.faktor, fest).slice(0, -1).map((i) => dicht[i]!);
  };
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
  const sicht = sichtbareFelder(gs, blick, pa, qa, nx, nz, (i) => art[i] !== 0);
  const aus: Punkt[][] = [];
  // Ein Stück, das nach dem Vereinfachen keine Fläche mehr hat, war kleiner als ε.
  const nimm = (r: Punkt[]) => {
    if (r.length >= 3) aus.push(r);
  };
  for (const [i, teile] of stuecke) if (sicht[i]) for (const t of teile) nimm(projiziert(t, true));
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
      // Ohne Schirm laufen die Kanten von Mitte zu Mitte der Zellen; dort ist H linear, die Projektion gerade: Die Ecken reichen.
      nimm(projiziert(ring, false));
    }
  }
  return aus;
}

/** Ein Linienzug auf dem Gelände: Bildpunkte und je Punkt, ob er verdeckt ist. */
export interface Zug {
  punkte: Punkt[];
  /** Je Punkt, ob Gelände ihn verdeckt; die Strecke zu ihm gehört dazu. Für die Wand. */
  verdeckt: boolean[];
  /** Wie `verdeckt`, nur verdeckte Stücke kürzer als `GLATT.luecke` nicht: Für die Linie selbst, sie bleibt hinter Häusern voll. */
  gestrichelt: boolean[];
}

/** Glätten entlang eines Zugs: erst der Median über ±`median` Blöcke des Wegs, dann das Mittel über ±`mittel`. Gewählt in 0104. */
const GLATT = { median: 8, mittel: 4, luecke: 16 };

/**
 * Die Punkte eines Zugs, an denen die Linie gestrichelt läuft: die
 * verdeckten, ausser in Stücken kürzer als `GLATT.luecke` Blöcke des Wegs.
 * Solche verdeckt ein Haus oder eine Mauer, kein Hang; durch sie hindurch
 * bleibt die Linie voll. Siehe docs/benutzung/ebenen.md, „Was verdeckt ist“.
 */
function gestrichelt(punkte: readonly Punkt[], verdeckt: readonly boolean[]): boolean[] {
  const aus = [...verdeckt];
  for (let i = 0; i < verdeckt.length; ) {
    if (!verdeckt[i]) {
      i++;
      continue;
    }
    // Ein Stück [von, bis) verdeckter Punkte; die Strecke vor dem ersten gehört dazu.
    const von = i;
    while (i < verdeckt.length && verdeckt[i]) i++;
    let lang = 0;
    for (let k = Math.max(1, von); k < i; k++) lang += Math.hypot(punkte[k]![0] - punkte[k - 1]![0], punkte[k]![1] - punkte[k - 1]![1]);
    if (lang < GLATT.luecke) aus.fill(false, von, i);
  }
  return aus;
}

/**
 * Die Höhen eines abgetasteten Zugs, geglättet entlang seines Wegs: erst je
 * Punkt der Median der Höhen im Fenster ±`GLATT.median` Blöcke, dann das
 * Mittel dieser Mediane im Fenster ±`GLATT.mittel`. So fallen Häuser und
 * Mauern bis etwa 8 Blöcke Breite heraus, ein gleichmässiger Hang bleibt, wie
 * er ist. Das Fenster ist symmetrisch, an den Enden eines offenen Zugs
 * entsprechend kleiner; ein Ring, erster gleich letzter Punkt, läuft um.
 * Siehe docs/benutzung/ebenen.md, „Ränder, Linien und Kreise“.
 */
export function glaetten(punkte: readonly Punkt[], hoehen: readonly number[]): number[] {
  const n = punkte.length;
  const ring = n > 2 && punkte[0]![0] === punkte[n - 1]![0] && punkte[0]![1] === punkte[n - 1]![1];
  // Im Ring zählt der doppelte letzte Punkt nicht mit; er bekommt am Ende den Wert des ersten.
  const m = ring ? n - 1 : n;
  const weg = [0];
  for (let i = 1; i < n; i++) weg.push(weg[i - 1]! + Math.hypot(punkte[i]![0] - punkte[i - 1]![0], punkte[i]![1] - punkte[i - 1]![1]));
  const lang = weg[n - 1]!;
  /** Die Werte im Fenster ±r um i, im Ring über den Anfang hinweg. */
  const fenster = (werte: readonly number[], i: number, r: number): number[] => {
    const w = ring ? Math.min(r, lang / 2) : Math.min(r, weg[i]!, lang - weg[i]!);
    const im = [werte[i]!];
    for (const richtung of [1, -1]) {
      for (let k = 1; k < m; k++) {
        const j = ring ? (((i + richtung * k) % m) + m) % m : i + richtung * k;
        if (j < 0 || j >= m) break;
        const d = Math.abs(weg[i]! - weg[j]!);
        if ((ring ? Math.min(d, lang - d) : d) > w + 1e-9) break;
        im.push(werte[j]!);
      }
    }
    return im;
  };
  const roh = hoehen.slice(0, m);
  const mediane = roh.map((_, i) => {
    const im = fenster(roh, i, GLATT.median).sort((a, b) => a - b);
    return im[im.length >> 1]!;
  });
  const glatt = mediane.map((_, i) => {
    const im = fenster(mediane, i, GLATT.mittel);
    return im.reduce((a, b) => a + b, 0) / im.length;
  });
  if (ring) glatt.push(glatt[0]!);
  return glatt;
}

/**
 * Ein Rand oder eine Linie auf dem Gelände, im iso entlang des Wegs geglättet,
 * `glaetten`. Von oben bleibt der Zug, wie er ist, und liegt eben. Siehe
 * docs/benutzung/ebenen.md, „Ränder, Linien und Kreise“.
 */
export function zug(punkte: readonly Punkt[], geschlossen: boolean, g: Gelaende, blick: Blick, schirm?: Schirm): Zug {
  if (blick.p.y === 0) {
    const ebene = geschlossen && punkte.length ? [...punkte, punkte[0]!] : [...punkte];
    const nie = ebene.map(() => false);
    return { punkte: ebene.map(([x, z]) => bildpunkt(x, 0, z, blick)), verdeckt: nie, gestrichelt: nie };
  }
  // Mit Schirm nach 0111 nur im Ausschnitt an den Zellen, sonst überall.
  const abgetastet = schirm ? abtastenImRahmen(punkte, geschlossen, g, blick, schirm) : abtasten(punkte, geschlossen, g.c);
  const hoehen = glaetten(abgetastet, abgetastet.map(([x, z]) => oberflaeche(g, x, z)));
  const hinter = abgetastet.map(([x, z], i) => verdeckt(g, x, hoehen[i]!, z, blick));
  const strich = gestrichelt(abgetastet, hinter);
  const bild = abgetastet.map(([x, z], i) => bildpunkt(x, hoehen[i]!, z, blick));
  if (!schirm) return { punkte: bild, verdeckt: hinter, gestrichelt: strich };
  // Erst nach Glätten und Verdeckung vereinfachen; jeder Wechsel von verdeckt oder gestrichelt bleibt, samt dem Punkt davor.
  const fest: number[] = [];
  for (let i = 1; i < bild.length; i++) if (hinter[i] !== hinter[i - 1] || strich[i] !== strich[i - 1]) fest.push(i - 1, i);
  const wahl = vereinfache(bild, schirm.eps / schirm.faktor, fest);
  return { punkte: wahl.map((i) => bild[i]!), verdeckt: wahl.map((i) => hinter[i]!), gestrichelt: wahl.map((i) => strich[i]!) };
}

/**
 * Die Wand über einem Rand, `hoch` Pixel der feinsten Stufe nach oben, in
 * `baender` Bändern von unten nach oben: je Band ein Ring je Lauf sichtbarer
 * Strecken, die in x in dieselbe Richtung gehen. Eine Strecke gehört zum Lauf
 * ihres Endpunkts, verdeckte fallen weg. Alle Ringe laufen gleich herum; so
 * füllt `nonzero` auch, wo sich zwei überdecken, statt dort ein Loch zu
 * lassen. Siehe docs/benutzung/ebenen.md, „Die Wand“.
 */
export function wand({ punkte, verdeckt }: Pick<Zug, 'punkte' | 'verdeckt'>, hoch: number, baender: number): Punkt[][][] {
  const aus: Punkt[][][] = Array.from({ length: baender }, () => []);
  for (let i = 1; i < punkte.length; ) {
    if (verdeckt[i]) {
      i++;
      continue;
    }
    const von = i - 1;
    let richtung = 0;
    for (; i < punkte.length && !verdeckt[i]; i++) {
      const s = Math.sign(punkte[i]![0] - punkte[i - 1]![0]);
      if (richtung === 0) richtung = s;
      // Die Richtung kehrt um: Hier beginnt der nächste Lauf, am selben Punkt.
      else if (s !== 0 && s !== richtung) break;
    }
    const lauf = punkte.slice(von, i);
    for (let k = 0; k < baender; k++) {
      const [unten, oben] = [(hoch * k) / baender, (hoch * (k + 1)) / baender];
      const ring: Punkt[] = [...lauf.map(([x, y]): Punkt => [x, y - unten]), ...lauf.map(([x, y]): Punkt => [x, y - oben]).reverse()];
      aus[k]!.push(richtung < 0 ? ring.reverse() : ring);
    }
  }
  return aus;
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
