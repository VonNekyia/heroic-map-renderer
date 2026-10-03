import { expect, test } from '@playwright/test';
import { projiziere, RICHTUNGEN, type Projektion } from '../src/pick';
import { grenzen, imBlick, tablett, type Flaeche, type Grenzen, type Rechteck, type Teil } from '../src/tablett';
import { eintraege, kamera } from './kamera';

type Punkt = [number, number];

const paare = eintraege.filter((e) => e.eben === undefined && e.wand === undefined);

/** Jede Kamera aus den Einträgen des Renderers mit einem ihrer scales. */
const KAMERAS = [...new Map(paare.map((e) => [e.camera, e.scale])).entries()];

/** Die deckenden Flächen; Schatten und Saum lassen durchscheinen, was darunter liegt. */
const flaechen = (teile: Teil[]) => teile.filter((t): t is Flaeche => t.form === 'flaeche');

/** Grenzen um ihre Mitte vergrössert, mit `faktor`. */
function weiter([links, oben, rechts, unten]: Grenzen, faktor: number): Grenzen {
  const [mx, my, b, h] = [(links + rechts) / 2, (oben + unten) / 2, (rechts - links) / 2, (unten - oben) / 2];
  return [mx - faktor * b, my - faktor * h, mx + faktor * b, my + faktor * h];
}

/** Das Tablett in einem Fenster, das die Karte samt Rahmen zu 70 % füllt. */
function stueck(area: Rechteck, meer: number, minY: number, p: Projektion, k: number): Teil[] {
  return tablett(area, meer, minY, p, k, weiter(grenzen(area, meer, p, k), 1 / 0.7));
}

/** Liegt der Punkt in der Fläche? `rand` > 0 schrumpft sie, < 0 dehnt sie, in Teilen der Kanten. */
function deckt({ o, a, b }: Flaeche, [px, py]: Punkt, rand = 0): boolean {
  const det = a[0] * b[1] - a[1] * b[0];
  if (det === 0) return false;
  const [dx, dy] = [px - o[0], py - o[1]];
  const s = (dx * b[1] - dy * b[0]) / det;
  const t = (a[0] * dy - a[1] * dx) / det;
  return [s, t].every((w) => w > rand && w < 1 - rand);
}

test('die Oberkante liegt auf den Ecken von area in Höhe seaLevel, auf das Pixel', () => {
  expect(new Set(paare.map((e) => e.camera)).size).toBeGreaterThan(5);
  for (const { camera, direction, scale, block, pixel } of paare) {
    const p = kamera(camera, scale);
    const k = RICHTUNGEN[p.azimuth].indexOf(direction);
    // Eine Welt aus einem Block: Die Ecke vorn links oben im Blick ist das
    // Pixel des Renderers für diesen Block.
    const [bx, by, bz] = block;
    // Waagrecht auf dem Wasserspiegel liegen nur die Oberkante und die
    // Pfeiler; die Schrägen fallen nach aussen ab.
    const oben = flaechen(stueck([bx, bz, bx + 1, bz + 1], by, by - 128, p, k)).filter(
      (f) => (f.art === 'rand' || f.art === 'pfeiler') && f.n[1] > 0.99,
    );
    // Ein Stück daneben: Nur in Richtung der Welt liegt kein Rand.
    const [dx, dz] = [projiziere(1, 0, 0, p), projiziere(0, 0, 1, p)];
    const e = 1 / 64;
    for (const sx of [-1, 1]) {
      for (const sz of [-1, 1]) {
        const punkt: Punkt = [
          pixel[0] + e * (sx * dx[0] + sz * dz[0]),
          pixel[1] + e * (sx * dx[1] + sz * dz[1]),
        ];
        const name = `${camera} ${direction}, scale ${scale}, Block ${String(block)}, ${sx} ${sz}`;
        expect(oben.some((f) => deckt(f, punkt)), name).toBe(sx < 0 || sz < 0);
      }
    }
  }
});

/** Die Seiten eines Blocks, die die Kamera sieht, je als Ecke und zwei Kanten. */
function seiten([x, y, z]: [number, number, number], p: Projektion) {
  const liste: [number, number, number][][] = [[[x, y + 1, z], [1, 0, 0], [0, 0, 1]]];
  if (p.y > 0) liste.push([[x, y, z + 1], [1, 0, 0], [0, 1, 0]]);
  if (p.y > 0 && p.azimuth === 'diagonal') liste.push([[x + 1, y, z], [0, 0, 1], [0, 1, 0]]);
  return liste;
}

/** Mitte und vier Punkte nahe den Ecken einer Seite, im Bild. */
function proben([o, a, b]: [number, number, number][], p: Projektion): Punkt[] {
  const bei = (s: number, t: number) =>
    projiziere(o![0] + s * a![0] + t * b![0], o![1] + s * a![1] + t * b![1], o![2] + s * a![2] + t * b![2], p);
  return [bei(0.5, 0.5), bei(0.1, 0.1), bei(0.9, 0.1), bei(0.1, 0.9), bei(0.9, 0.9)];
}

const MEER = 63;
const MIN_Y = -64;

/**
 * Zwei Welten: eine kleine, in der jede Spalte am Rand geprüft wird, und eine
 * grosse, in der Rahmen und Gegenstände so gross sind wie in einer echten.
 */
const WELTEN: { area: Rechteck; schritt: number }[] = [
  { area: [-40, -24, 40, 56], schritt: 1 },
  { area: [-1000, -800, 1200, 1500], schritt: 37 },
];

test('was vor den Kacheln liegt, deckt kein Gelände über dem Wasserspiegel, an keinem Rand', () => {
  // Gesammelt statt je Punkt geprüft: Es sind Hunderttausende.
  const gedeckt: string[] = [];
  for (const { area, schritt } of WELTEN) {
    for (const [camera, scale] of KAMERAS) {
      const p = kamera(camera, scale);
      for (let k = 0; k < 4; k++) {
        const nah = flaechen(stueck(area, MEER, MIN_Y, p, k)).filter((f) => f.nah);
        expect(nah.length).toBeGreaterThan(0);
        const [x0, z0, x1, z1] = imBlick(area, k);
        // Gelände über dem Wasser an jedem Rand, vom Wasserspiegel bis
        // fast an die Bauhöhe.
        const bloecke: [number, number, number][] = [];
        for (const y of [MEER, MEER + 1, MEER + 40, MEER + 120, MEER + 250]) {
          for (let x = x0; x < x1; x += schritt) bloecke.push([x, y, z0], [x, y, z1 - 1]);
          for (let z = z0; z < z1; z += schritt) bloecke.push([x0, y, z], [x1 - 1, y, z]);
          bloecke.push([x1 - 1, y, z1 - 1], [x1 - 1, y, z0], [x0, y, z1 - 1]);
        }
        for (const block of bloecke) {
          for (const seite of seiten(block, p)) {
            for (const punkt of proben(seite, p)) {
              const deckend = nah.find((f) => deckt(f, punkt));
              if (deckend) gedeckt.push(`${String(area)} ${camera} k=${k}, Block ${String(block)}: ${deckend.art}`);
            }
          }
        }
      }
    }
  }
  expect(gedeckt.slice(0, 10)).toEqual([]);
});

/** Eine schmale Welt: Hier reicht der Tisch nur über den Schnitt, weil er es muss. */
const SCHMAL: Rechteck = [-12, -4, 12, 20];

test('der Schnitt der Welt zur Kamera liegt ganz unter Rahmen, Tisch und Zarge', () => {
  const offen: string[] = [];
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    // Von oben zeigt die Welt keinen Schnitt.
    if (p.y === 0) continue;
    for (let k = 0; k < 4; k++) {
      const nah = flaechen(stueck(SCHMAL, MEER, MIN_Y, p, k)).filter((f) => f.nah);
      const [x0, z0, x1, z1] = imBlick(SCHMAL, k);
      const punkte: [number, number, number][] = [];
      for (let y = MIN_Y + 0.5; y < MEER; y += 4) {
        for (let x = x0 + 0.5; x < x1; x += 2) punkte.push([x, y, z1]);
        if (p.azimuth === 'diagonal') for (let z = z0 + 0.5; z < z1; z += 2) punkte.push([x1, y, z]);
      }
      for (const punkt of punkte) {
        const bild = projiziere(...punkt, p);
        if (!nah.some((f) => deckt(f, bild, -1e-9))) offen.push(`${camera} k=${k}, ${String(punkt)}`);
      }
    }
  }
  expect(offen.slice(0, 10)).toEqual([]);
});

test('der Tisch füllt das Fenster, auch eine Stufe weiter draussen', () => {
  const offen: string[] = [];
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const rahmen = grenzen(area, MEER, p, k);
      // Bei der ganzen Karte füllt sie das Fenster zu 70 % oder zur Hälfte;
      // eine Stufe hinaus ist das Fenster doppelt so gross.
      for (const anteil of [0.7, 0.5]) {
        const ansicht = weiter(rahmen, 1 / anteil);
        const alle = flaechen(tablett(area, MEER, MIN_Y, p, k, ansicht));
        const [links, oben, rechts, unten] = weiter(ansicht, 2);
        for (let i = 0; i <= 20; i++) {
          for (let j = 0; j <= 20; j++) {
            const punkt: Punkt = [links + ((rechts - links) * i) / 20, oben + ((unten - oben) * j) / 20];
            if (!alle.some((f) => deckt(f, punkt, -1e-9))) offen.push(`${camera} k=${k} ${anteil}: ${String(punkt)}`);
          }
        }
      }
    }
  }
  expect(offen.slice(0, 10)).toEqual([]);
});

test('die Vorderkante des Tischs liegt am unteren Rand des Fensters', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    // Von oben gibt es keine Zarge zu sehen.
    if (p.y === 0) continue;
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const ansicht = weiter(grenzen(area, MEER, p, k), 2);
      const [, oben, , unten] = ansicht;
      const zarge = flaechen(tablett(area, MEER, MIN_Y, p, k, ansicht)).filter((f) => f.art === 'zarge');
      // Die Oberkante der Zarge ist die Vorderkante des Tischs; b zeigt nach
      // oben. Ihr tiefster Punkt ist diagonal die vordere Ecke, knapp unter
      // dem Fenster; genordet liegt die ganze Kante im unteren Teil.
      const vorn = Math.max(...zarge.flatMap(({ o, a, b }) => [o[1] + b[1], o[1] + a[1] + b[1]]));
      const name = `${camera} k=${k}`;
      expect(vorn, name).toBeGreaterThan(unten - 0.15 * (unten - oben));
      expect(vorn, name).toBeLessThan(unten + 0.1 * (unten - oben));
    }
  }
});

test('die Grenzen für das Einpassen umfassen den Rahmen, nicht mehr', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const [links, oben, rechts, unten] = grenzen(area, MEER, p, k);
      const rahmen = flaechen(stueck(area, MEER, MIN_Y, p, k)).filter((f) =>
        ['rand', 'leiste', 'wand', 'pfeiler'].includes(f.art),
      );
      const ecken = rahmen.flatMap(({ o, a, b }): Punkt[] => [
        o,
        [o[0] + a[0], o[1] + a[1]],
        [o[0] + b[0], o[1] + b[1]],
        [o[0] + a[0] + b[0], o[1] + a[1] + b[1]],
      ]);
      const xs = ecken.map((e) => e[0]);
      const ys = ecken.map((e) => e[1]);
      const name = `${camera} k=${k}`;
      expect(Math.min(...xs), name).toBeCloseTo(links, 6);
      expect(Math.max(...xs), name).toBeCloseTo(rechts, 6);
      expect(Math.min(...ys), name).toBeCloseTo(oben, 6);
      expect(Math.max(...ys), name).toBeLessThanOrEqual(unten + 1e-6);
    }
  }
});
