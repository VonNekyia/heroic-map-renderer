/**
 * Rahmen und Tisch um die Karte: Die Welt liegt in einem Tablett, dessen
 * Oberkante auf dem Wasserspiegel liegt, und das Tablett auf einem Tisch.
 * Alles besteht aus ebenen Rechtecken; in der Parallelprojektion geht jedes
 * affin aufs Bild. Ohne Leaflet, damit die Tests es in Node laden.
 * Siehe docs/frontend.md, „Rahmen und Tisch“.
 */
import { projiziere, type Projektion } from './pick';

type Punkt = [number, number];

/** Ein Rechteck der Welt in Blöcken, `[x0, z0, x1, z1]`; x1 und z1 sind Kanten dahinter. */
export type Rechteck = [x0: number, z0: number, x1: number, z1: number];

/** Ein ebenes Rechteck im Bild: Ecke `o`, Kanten `a` und `b`, in Pixeln der feinsten Stufe. */
export interface Flaeche {
  o: Punkt;
  a: Punkt;
  b: Punkt;
  art: 'tisch' | 'boden' | 'rand' | 'wand' | 'ding';
  /** CSS-Farbe, schon schattiert. */
  farbe: string;
  /** Liegt vor den Kacheln. Siehe docs/frontend.md, „Vor und hinter der Welt“. */
  nah: boolean;
}

/** Ein Quader im Blick: von–bis in x, y und z. */
type Quader = [x0: number, x1: number, y0: number, y1: number, z0: number, z1: number];

/** Das Rechteck der Welt im Blick mit k Vierteldrehungen; Punkte drehen sich mit (z, −x). */
export function imBlick([x0, z0, x1, z1]: Rechteck, k: number): Rechteck {
  for (let i = 0; i < k; i++) [x0, z0, x1, z1] = [z0, -x1, z1, -x0];
  return [x0, z0, x1, z1];
}

/**
 * Die Breite des Rands in Blöcken: ein Anteil der Welt, damit er auf den
 * äusseren Stufen zu sehen ist, mindestens 16.
 */
// ponytail: fester Anteil 1/64, bis die Masse des Researchers da sind.
export function randbreite(welt: number): number {
  return Math.max(16, Math.round(welt / 64));
}

const FARBE = {
  tisch: '#2b2a30',
  boden: '#3a1f10',
  holz: '#723521',
  buch: ['#6b1d1d', '#1d3b5c', '#2f4a24'],
  kerze: '#e8dcc0',
  sphaere: '#c9a24a',
  kompass: '#8a6a3a',
  blumen: '#c0506a',
};

/** Eine Farbe, abgedunkelt mit dem Faktor f. */
function toene(farbe: string, f: number): string {
  const n = Number.parseInt(farbe.slice(1), 16);
  const kanal = (i: number) => Math.round(((n >> (16 - 8 * i)) & 255) * f);
  return `rgb(${kanal(0)} ${kanal(1)} ${kanal(2)})`;
}

/**
 * Die Flächen von Tisch, Tablett und Gegenständen im Blick mit k
 * Vierteldrehungen, in der Reihenfolge, in der sie gemalt werden. Eine
 * spätere deckt eine frühere. `minY` ist die Unterkante der Welt: so weit
 * reicht ihr Schnitt, den der Tisch vor ihr verdeckt.
 */
export function tablett(
  area: Rechteck,
  meer: number,
  minY: number,
  p: Projektion,
  k: number,
): Flaeche[] {
  const [x0, z0, x1, z1] = imBlick(area, k);
  const welt = Math.max(x1 - x0, z1 - z0);
  const R = randbreite(welt);
  const tisch = meer - Math.round(1.5 * R);
  // Der Tisch reicht über den Fensterrand und vorn über das Bild des
  // Schnitts bis minY hinaus, das in der Achse (b, 2a, b) bis zu
  // (tisch − minY)·b/a weiter vorn liegt.
  const E = 2 * welt + Math.ceil(((tisch - minY) * p.y) / p.v);
  const genordet = p.azimuth === 'north';

  // Nah ist, was Gelände nie verdecken kann. Ein Bildpunkt zeigt Gelände
  // vor einem Punkt nur, wenn dieses entlang der Achse weiter vorn liegt:
  // diagonal bei grösserem x und z, genordet bei grösserem z und gleichem x.
  // Siehe docs/frontend.md, „Vor und hinter der Welt“.
  const istNah = ([qx0, qx1, , , qz0]: Quader) =>
    qx0 >= x1 || qz0 >= z1 || (genordet && qx1 <= x0);
  // Wie im Spiel: Seiten nach Norden und Süden 0,8, nach Osten und Westen 0,6.
  // Im Blick ist +x nach jeder ungeraden Drehung Süden.
  const [hellX, hellZ] = k % 2 === 0 ? [0.6, 0.8] : [0.8, 0.6];

  const flaechen: Flaeche[] = [];
  const rechteck = (
    o: [number, number, number],
    a: [number, number, number],
    b: [number, number, number],
    art: Flaeche['art'],
    farbe: string,
    nah: boolean,
  ) => flaechen.push({ o: projiziere(...o, p), a: projiziere(...a, p), b: projiziere(...b, p), art, farbe, nah });
  /** Ein Quader mit den Seiten, die die Kamera sieht: oben, +z, diagonal auch +x. */
  const quader = (q: Quader, art: Flaeche['art'], farbe: string) => {
    const [qx0, qx1, qy0, qy1, qz0, qz1] = q;
    const nah = istNah(q);
    if (qy1 > qy0 && p.y > 0) {
      rechteck([qx0, qy0, qz1], [qx1 - qx0, 0, 0], [0, qy1 - qy0, 0], art, toene(farbe, hellZ), nah);
      if (!genordet) {
        rechteck([qx1, qy0, qz0], [0, 0, qz1 - qz0], [0, qy1 - qy0, 0], art, toene(farbe, hellX), nah);
      }
    }
    rechteck([qx0, qy1, qz0], [qx1 - qx0, 0, 0], [0, 0, qz1 - qz0], art, farbe, nah);
  };
  const tiefe = ([qx0, qx1, qy0, qy1, qz0, qz1]: Quader) =>
    genordet
      ? p.v * (qy0 + qy1) + p.y * (qz0 + qz1)
      : p.y * (qx0 + qx1 + qz0 + qz1) + 2 * p.v * (qy0 + qy1);

  // Der Tisch ganz hinter den Kacheln, vorn und rechts noch einmal davor.
  // Die Streifen überlappen, damit keine Naht bleibt.
  quader([x0 - E, x1 + E, tisch, tisch, z0 - E, z1 + E], 'tisch', FARBE.tisch);
  quader([x0 - E, x1 + E, tisch, tisch, z1, z1 + E], 'tisch', FARBE.tisch);
  quader([x1, x1 + E, tisch, tisch, z0 - E, z1 + E], 'tisch', FARBE.tisch);
  if (genordet) quader([x0 - E, x0, tisch, tisch, z0 - E, z1 + E], 'tisch', FARBE.tisch);
  // Wo in area keine Welt liegt, zeigt das Tablett seinen Boden.
  quader([x0, x1, tisch, tisch, z0, z1], 'boden', FARBE.boden);

  // Platzhalter der Gegenstände: Bücher hinten links, Kerze und Sphäre
  // hinten, Kompass vorn rechts, Blumen vorn links. Was über den Wasserspiegel
  // ragt, steht hinten; vorn bliebe es sonst vor dem Gelände.
  const g = R / 2;
  const links = x0 - R - g;
  const hinten = z0 - R - g;
  const dinge: [Quader, string][] = [
    [[links - 3 * R, links, tisch, tisch + R / 2, z0, z0 + 2 * R], FARBE.buch[0]!],
    [[links - 2.8 * R, links - 0.3 * R, tisch + R / 2, tisch + R, z0 + 0.3 * R, z0 + 2 * R], FARBE.buch[1]!],
    [[links - 2.6 * R, links - 0.6 * R, tisch + R, tisch + 1.4 * R, z0 + 0.2 * R, z0 + 1.6 * R], FARBE.buch[2]!],
    [[x1 - 4 * R, x1 - 3.4 * R, tisch, tisch + 3 * R, hinten - 0.6 * R, hinten], FARBE.kerze],
    [[x1 - 8 * R, x1 - 6 * R, tisch, tisch + 2.5 * R, hinten - 2 * R, hinten], FARBE.sphaere],
    [[x1 + R + g, x1 + 3 * R + g, tisch, tisch + R / 4, z1 - 3 * R, z1 - R], FARBE.kompass],
    [[x0, x0 + R, tisch, tisch + R, z1 + R + g, z1 + 2 * R + g], FARBE.blumen],
  ];
  const ding = ([q, farbe]: [Quader, string]) => quader(q, 'ding', farbe);
  const [nahe, ferne] = [true, false].map((nah) =>
    dinge.filter(([q]) => istNah(q) === nah).sort(([a], [b]) => tiefe(a) - tiefe(b)),
  ) as [typeof dinge, typeof dinge];

  // Was hinter dem Rahmen steht, deckt er; was davor steht, deckt ihn.
  ferne.forEach(ding);
  const H = meer - tisch;
  if (p.y > 0) {
    // Die Wände zur Kamera, vom Tisch bis zur Oberkante.
    rechteck([x0 - R, tisch, z1 + R], [x1 - x0 + 2 * R, 0, 0], [0, H, 0], 'wand', toene(FARBE.holz, hellZ), true);
    if (!genordet) {
      rechteck([x1 + R, tisch, z0 - R], [0, 0, z1 - z0 + 2 * R], [0, H, 0], 'wand', toene(FARBE.holz, hellX), true);
    }
  }
  // Die Oberkante in vier Streifen um die Welt, hinten, links, rechts, vorn.
  quader([x0 - R, x1 + R, meer, meer, z0 - R, z0], 'rand', FARBE.holz);
  quader([x0 - R, x0, meer, meer, z0 - R, z1 + R], 'rand', FARBE.holz);
  quader([x1, x1 + R, meer, meer, z0 - R, z1 + R], 'rand', FARBE.holz);
  quader([x0 - R, x1 + R, meer, meer, z1, z1 + R], 'rand', FARBE.holz);
  nahe.forEach(ding);
  return flaechen;
}
