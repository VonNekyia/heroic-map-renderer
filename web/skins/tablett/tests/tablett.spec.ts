import { expect, test } from '@playwright/test';
import type { Grenzen, Projektion, Rechteck } from 'heroic-map-renderer/skin-api';
import { readdirSync } from 'node:fs';
import { eintraege, kamera, projiziere, RICHTUNGEN } from '../../../tests/kamera';
import { BREITE_VORLAGE, ECKSTUECKE, GEGENSTAENDE, LILIEN, MARMOR, MASS, RAND, TISCH_RAND, VORLAGE } from '../bilder';
import { gesamtmitte, gesamtstufe, grenzen, imBlick, tablett, type Blick, type Figur, type Flaeche, type Schrift, type Teil } from '../tablett';

type Punkt = [number, number];

const paare = eintraege.filter((e) => e.eben === undefined && e.wand === undefined);

/** Jede Kamera aus den Einträgen des Renderers mit einem ihrer scales. */
const KAMERAS = [...new Map(paare.map((e) => [e.camera, e.scale])).entries()];

/** Die Flächen; der Saum lässt durchscheinen, was darunter liegt. */
const flaechen = (teile: Teil[]) => teile.filter((t): t is Flaeche => t.form === 'flaeche');

/** Der Blick, wie ihn die Grundkarte dem Skin reicht. */
const blick = (p: Projektion, k: number): Blick => ({ projektion: p, k, projiziere: (x, y, z) => projiziere(x, y, z, p) });

/** Liegt der Punkt im Parallelogramm? `rand` > 0 schrumpft es, < 0 dehnt es, in Teilen der Kanten. */
function imParallelogramm({ o, a, b }: { o: Punkt; a: Punkt; b: Punkt }, [px, py]: Punkt, rand = 0): boolean {
  const det = a[0] * b[1] - a[1] * b[0];
  if (det === 0) return false;
  const [dx, dy] = [px - o[0], py - o[1]];
  const s = (dx * b[1] - dy * b[0]) / det;
  const t = (a[0] * dy - a[1] * dx) / det;
  return [s, t].every((w) => w > rand && w < 1 - rand);
}

/** Liegt der Punkt in einem der Vielecke? Gerade-ungerade-Regel. */
function imVieleck(vielecke: Punkt[][], [px, py]: Punkt): boolean {
  return vielecke.some((vieleck) => {
    let drin = false;
    for (let i = 0, j = vieleck.length - 1; i < vieleck.length; j = i++) {
      const [[xi, yi], [xj, yj]] = [vieleck[i]!, vieleck[j]!];
      if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) drin = !drin;
    }
    return drin;
  });
}

/** Deckt die Fläche den Punkt? Eine Kopie vor den Kacheln deckt ihre Vielecke ganz, mit Grund. */
const deckt = (f: Flaeche, punkt: Punkt, rand = 0) => (f.nurIn ? imVieleck(f.nurIn, punkt) : imParallelogramm(f, punkt, rand));

test('die Oberkante liegt auf den Ecken von area in Höhe seaLevel, auf das Pixel', () => {
  expect(new Set(paare.map((e) => e.camera)).size).toBeGreaterThan(5);
  for (const { camera, direction, scale, block, pixel } of paare) {
    const p = kamera(camera, scale);
    const k = RICHTUNGEN[p.azimuth].indexOf(direction);
    // Eine Welt aus einem Block: Die Ecke vorn links oben im Blick ist das
    // Pixel des Renderers für diesen Block.
    const [bx, by, bz] = block;
    const oben = flaechen(tablett([bx, bz, bx + 1, bz + 1], by, by - 128, blick(p, k))).filter(
      (f) => (f.art === 'rand' || f.art === 'pfeiler') && f.n[1] > 0.99,
    );
    // Ein Stück daneben: Nur in Richtung der Welt liegt kein Rand. Der Rand
    // einer Welt aus einem Block ist 0,016 Blöcke breit; das Stück liegt
    // weit innerhalb davon.
    const [dx, dz] = [projiziere(1, 0, 0, p), projiziere(0, 0, 1, p)];
    const e = 1 / 1024;
    for (const sx of [-1, 1]) {
      for (const sz of [-1, 1]) {
        const punkt: Punkt = [pixel[0] + e * (sx * dx[0] + sz * dz[0]), pixel[1] + e * (sx * dx[1] + sz * dz[1])];
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
 * grosse, in der der Rahmen so gross ist wie in einer echten.
 */
const WELTEN: { area: Rechteck; schritt: number }[] = [
  { area: [-40, -24, 40, 56], schritt: 1 },
  { area: [-1000, -800, 1200, 1400], schritt: 37 },
];

test('was vor den Kacheln liegt, deckt kein Gelände über dem Wasserspiegel, an keinem Rand, ausser an den Ecken', () => {
  // Gesammelt statt je Punkt geprüft: Es sind Hunderttausende.
  const gedeckt: string[] = [];
  for (const { area, schritt } of WELTEN) {
    for (const [camera, scale] of KAMERAS) {
      const p = kamera(camera, scale);
      for (let k = 0; k < 4; k++) {
        // Die Eckstücke decken die Ecken der Welt mit Absicht, wie in der
        // Vorlage; sie prüft der Test der Eckstücke.
        const nah = flaechen(tablett(area, MEER, MIN_Y, blick(p, k))).filter((f) => f.nah && f.art !== 'eck');
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

/** Eine schmale Welt: Ihr Schnitt reicht tiefer, als der Rahmen hoch ist. */
const SCHMAL: Rechteck = [-12, -4, 12, 20];

test('der Schnitt der Welt zur Kamera liegt ganz unter Rahmen und Tisch', () => {
  const offen: string[] = [];
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    // Von oben zeigt die Welt keinen Schnitt.
    if (p.y === 0) continue;
    for (let k = 0; k < 4; k++) {
      const nah = flaechen(tablett(SCHMAL, MEER, MIN_Y, blick(p, k))).filter((f) => f.nah);
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

/** Das Rechteck einer Figur im Bild: links, oben, rechts, unten. */
function rechteckDer({ fuss, anker, mass, groesse }: Figur): Grenzen {
  const [x, y] = [fuss[0] - mass * anker[0], fuss[1] - mass * anker[1]];
  return [x, y, x + mass * groesse[0], y + mass * groesse[1]];
}

/** Die Lilien unter den Teilen. */
const lilienIn = (teile: Teil[]) => teile.filter((t): t is Figur => t.form === 'figur' && t.bild.startsWith('lilie-'));

test('die Grenzen für das Einpassen umfassen Rahmen und Lilien, nicht mehr', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const [links, oben, rechts, unten] = grenzen(area, MEER, blick(p, k));
      const teile = tablett(area, MEER, MIN_Y, blick(p, k));
      const rahmen = flaechen(teile).filter((f) => ['rand', 'wand', 'pfeiler'].includes(f.art));
      const punkte = [
        ...rahmen.flatMap(({ o, a, b }): Punkt[] => [o, [o[0] + a[0], o[1] + a[1]], [o[0] + b[0], o[1] + b[1]], [o[0] + a[0] + b[0], o[1] + a[1] + b[1]]]),
        ...lilienIn(teile).flatMap((f): Punkt[] => {
          const [l, o, r, u] = rechteckDer(f);
          return [
            [l, o],
            [r, u],
          ];
        }),
      ];
      const xs = punkte.map((e) => e[0]);
      const ys = punkte.map((e) => e[1]);
      const name = `${camera} k=${k}`;
      expect(Math.min(...xs), name).toBeCloseTo(links, 6);
      expect(Math.max(...xs), name).toBeCloseTo(rechts, 6);
      expect(Math.min(...ys), name).toBeCloseTo(oben, 6);
      expect(Math.max(...ys), name).toBeCloseTo(unten, 6);
    }
  }
});

test('auf jedem Pfeiler steht eine Lilie so gross wie in der Vorlage, nach dem Rahmen gemalt, vor den Kacheln', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const teile = tablett(area, MEER, MIN_Y, blick(p, k));
      const lilien = lilienIn(teile);
      const name = `${camera} k=${k}`;
      expect(lilien.map((l) => l.bild).sort(), name).toEqual(Object.values(LILIEN).map((l) => l.bild).sort());
      // So viele Pixel der feinsten Stufe je Pixel der Vorlage, wie die Karte
      // im Bild breiter ist als in ihr.
      const [x0, z0, x1, z1] = imBlick(area, k);
      const xs = [projiziere(x0, MEER, z0, p), projiziere(x1, MEER, z0, p), projiziere(x0, MEER, z1, p), projiziere(x1, MEER, z1, p)].map((e) => e[0]);
      for (const l of lilien) expect(l.mass, name).toBeCloseTo((Math.max(...xs) - Math.min(...xs)) / BREITE_VORLAGE, 9);
      // Nach allem Holz.
      const holz = teile.map((t) => t.form === 'flaeche' && ['rand', 'wand', 'pfeiler'].includes(t.art));
      expect(teile.indexOf(lilien[0]!), name).toBeGreaterThan(holz.lastIndexOf(true));
      // Alle vor den Kacheln, auch die ferne: Sie steht über ihrem Eckstück.
      expect(lilien.every((l) => l.nah), name).toBe(true);
    }
  }
});

test('an jeder Ecke deckt ein Eckstück die Ecke der Welt, MASS.eck w nach innen, vor den Kacheln, zwischen Holz und Lilien', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const { area } = WELTEN[1]!;
      const teile = tablett(area, MEER, MIN_Y, blick(p, k));
      const ecken = flaechen(teile).filter((f) => f.art === 'eck');
      const name = `${camera} k=${k}`;
      expect(ecken.map((f) => f.bild).sort(), name).toEqual(Object.values(ECKSTUECKE).sort());
      expect(ecken.every((f) => f.nah && f.n[1] > 0.99), name).toBe(true);
      // Je Ecke der Welt auf dem Wasserspiegel: ein Stück innen gedeckt, eines
      // über MASS.eck hinaus nicht.
      const [x0, z0, x1, z1] = imBlick(area, k);
      const w = (RAND * (x1 - x0 + (z1 - z0))) / 2;
      for (const [x, sx] of [
        [x0, 1],
        [x1, -1],
      ] as const) {
        for (const [z, sz] of [
          [z0, 1],
          [z1, -1],
        ] as const) {
          const bei = (d: number) => projiziere(x + sx * d * w, MEER, z + sz * d * w, p);
          expect(ecken.some((f) => deckt(f, bei(0.5))), `${name} ${x} ${z}`).toBe(true);
          expect(ecken.some((f) => deckt(f, bei(MASS.eck + 0.5))), `${name} ${x} ${z}`).toBe(false);
        }
      }
      // Über allem Holz, unter allen Lilien.
      const holz = teile.map((t) => t.form === 'flaeche' && ['rand', 'wand', 'pfeiler'].includes(t.art)).lastIndexOf(true);
      const eck = teile.map((t) => t.form === 'flaeche' && t.art === 'eck');
      expect(eck.indexOf(true), name).toBeGreaterThan(holz);
      expect(teile.indexOf(lilienIn(teile)[0]!), name).toBeGreaterThan(eck.lastIndexOf(true));
    }
  }
});

test('das Licht für Flächen ohne Bild kommt von oben, leicht von links: die linke nahe Wand ist rund 1,6-mal so hell wie die rechte', () => {
  // Summe der Kanäle, aus `rgb(r g b)`.
  const hell = (f: Flaeche) => f.farbe.match(/\d+/g)!.reduce((summe, wert) => summe + Number(wert), 0);
  for (const camera of ['2:1', '8:5']) {
    const teile = flaechen(tablett(WELTEN[0]!.area, MEER, MIN_Y, blick(kamera(camera, 16), 0)));
    // Nahe Wände zeigen im Blick nach +z, links im Bild, und nach +x, rechts.
    const wand = (nx: number, nz: number) => hell(teile.find((f) => f.art === 'wand' && f.n[0] === nx && f.n[2] === nz)!);
    const verhaeltnis = wand(0, 1) / wand(1, 0);
    expect(verhaeltnis, camera).toBeGreaterThan(1.4);
    expect(verhaeltnis, camera).toBeLessThan(1.8);
  }
});

test('die fernen Seiten haben eine Innenseite bis zum Boden, die nahen nicht', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const innen = flaechen(tablett(WELTEN[0]!.area, MEER, MIN_Y, blick(p, k))).filter((f) => f.art === 'innen');
      // Von oben stehen die Wände auf der Kante; genordet sieht man eine
      // ferne Seite, diagonal zwei.
      const name = `${camera} k=${k}`;
      expect(innen.length, name).toBe(p.y === 0 ? 0 : p.azimuth === 'north' ? 1 : 2);
      // Hinter der Welt, nach innen und zur Kamera.
      for (const f of innen) {
        expect(f.nah, name).toBe(false);
        expect(f.n[1], name).toBeCloseTo(0, 9);
        expect(p.azimuth === 'north' ? f.n[2] : f.n[0] + f.n[2], name).toBeGreaterThan(0);
      }
    }
  }
});

test('Band, Wand, Eckstücke, die Seiten der Pfeiler zur Kamera und der Tisch haben ihr Bild in bilder/', () => {
  const da = new Set(readdirSync(new URL('../bilder/', import.meta.url)).map((datei) => datei.replace(/\.webp$/, '')));
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const teile = tablett(WELTEN[0]!.area, MEER, MIN_Y, blick(p, k));
      const name = `${camera} k=${k}`;
      for (const f of flaechen(teile)) {
        const zeigt = p.azimuth === 'north' ? f.n[2] > 0.5 : f.n[0] > 0.5 || f.n[2] > 0.5;
        const braucht = ['rand', 'eck', 'tisch'].includes(f.art) || (['wand', 'pfeiler'].includes(f.art) && zeigt);
        if (braucht) expect(f.bild && da.has(f.bild), `${name}: ${f.art} ${String(f.n)}`).toBe(true);
        if (f.bild) expect(da.has(f.bild), `${name}: ${f.bild}`).toBe(true);
      }
      for (const f of teile.filter((t): t is Figur => t.form === 'figur')) expect(da.has(f.bild), `${name}: ${f.bild}`).toBe(true);
    }
  }
});

/** Eine quadratische Welt, so gross wie eine echte. */
const GROSS: Rechteck = [-4096, -4096, 4096, 4096];

/** Fenster von Telefonen, hoch und quer, bis 4K, dazu das der Vorlage. */
const FENSTER = [
  [390, 844],
  [844, 390],
  [1280, 720],
  [1491, 1055],
  [1920, 1080],
  [2560, 1440],
  [3840, 2160],
] as const;

test('die Gesamtansicht füllt das Fenster zu 71 bis 100 %, zwischen zwei Stufen nur, wo Leaflet die Kacheln verkleinert', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const rahmen = grenzen(GROSS, MEER, blick(p, k));
      const [links, oben, rechts, unten] = rahmen;
      for (let breite = 640; breite <= 2560; breite += 37) {
        const hoehe = Math.round(breite * 0.62);
        const z = gesamtstufe(rahmen, 11, breite, hoehe);
        // Auf dieser Stufe füllt der Rahmen das Fenster ganz.
        const voll = 11 + Math.log2(Math.min(breite / (rechts - links), hoehe / (unten - oben)));
        const name = `${camera} k=${k} ${breite} × ${hoehe}`;
        expect(2 ** (z - voll), name).toBeLessThanOrEqual(1.01);
        expect(2 ** (z - voll), name).toBeGreaterThan(0.7);
        // So passt Leaflet die ganze Karte ein. Darunter landete der Knopf ⌂
        // neben der Gesamtansicht.
        const einpassen = Math.floor(Math.round(voll * 100) / 100);
        expect(z, name).toBeGreaterThanOrEqual(einpassen);
        // Gebrochen nur, wo Leaflet aufrundet und die Kacheln so verkleinert.
        if (!Number.isInteger(z)) {
          expect(z - Math.floor(z), name).toBeGreaterThanOrEqual(0.5);
          expect(z, name).toBeLessThan(11);
        }
        // Wo es geht, genau wie in der Vorlage.
        const ziel = voll + Math.log2(0.925);
        if (ziel - Math.floor(ziel) >= 0.5 && ziel >= einpassen) expect(2 ** (z - voll), name).toBeCloseTo(0.925, 9);
      }
    }
  }
});

test('ein Fenster ohne Fläche hat keine Gesamtansicht, statt dass sie abbricht', () => {
  const [camera, scale] = KAMERAS[0]!;
  const rahmen = grenzen(GROSS, MEER, blick(kamera(camera, scale), 0));
  for (const [breite, hoehe] of [[0, 0], [1491, 0], [0, 1055]] as const) {
    expect(gesamtstufe(rahmen, 11, breite, hoehe), `${breite} × ${hoehe}`).toBeNaN();
  }
});

test('die Gesamtansicht liegt wie in der Vorlage unter der Mitte der Karte, und der Rahmen ragt nie aus dem Fenster', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const rahmen = grenzen(GROSS, MEER, blick(p, k));
      for (const [breite, hoehe] of FENSTER) {
        const f = 2 ** (11 - gesamtstufe(rahmen, 11, breite, hoehe));
        const [mx, my] = gesamtmitte(GROSS, MEER, blick(p, k), breite * f, hoehe * f);
        const name = `${camera} k=${k} ${breite} × ${hoehe}`;
        const [links, oben, rechts, unten] = rahmen;
        expect(links, name).toBeGreaterThanOrEqual(mx - (breite * f) / 2 - 1e-6);
        expect(rechts, name).toBeLessThanOrEqual(mx + (breite * f) / 2 + 1e-6);
        expect(oben, name).toBeGreaterThanOrEqual(my - (hoehe * f) / 2 - 1e-6);
        expect(unten, name).toBeLessThanOrEqual(my + (hoehe * f) / 2 + 1e-6);
      }
    }
  }
  // Im Fenster der Vorlage, in ihrer Kamera, liegt die Mitte wie dort:
  // 5,8 % der Breite der Karte unter ihrer Mitte.
  const p = kamera('8:5', 16);
  const rahmen = grenzen(GROSS, MEER, blick(p, 0));
  const f = 2 ** (11 - gesamtstufe(rahmen, 11, 1491, 1055));
  const [mx, my] = gesamtmitte(GROSS, MEER, blick(p, 0), 1491 * f, 1055 * f);
  const [cx, cy] = projiziere(0, MEER, 0, p);
  const karte = 2 * 4096 * 2 * p.u;
  expect((mx - cx) / karte).toBeCloseTo(-0.0033, 6);
  expect((my - cy) / karte).toBeCloseTo(0.058, 6);
});

test('im Bezugsrahmen stehen die Gegenstände auf 3 px, wo die Vorlage sie hat, und der Tisch liegt über ihr', () => {
  // Der Bezugsrahmen: 8:5 aus se, die Gesamtansicht im Fenster der Vorlage,
  // mit einer Welt, deren Rahmen es genau zu 92,5 % füllt.
  const p = kamera('8:5', 16);
  const area: Rechteck = [-3200, -3200, 3200, 3200];
  const rahmen = grenzen(area, MEER, blick(p, 0));
  const z = gesamtstufe(rahmen, 11, ...VORLAGE);
  const voll = 11 + Math.log2(Math.min(VORLAGE[0] / (rahmen[2] - rahmen[0]), VORLAGE[1] / (rahmen[3] - rahmen[1])));
  expect(2 ** (z - voll)).toBeCloseTo(0.925, 9);
  const f = 2 ** (11 - z);
  const [mx, my] = gesamtmitte(area, MEER, blick(p, 0), VORLAGE[0] * f, VORLAGE[1] * f);
  const imFenster = ([x, y]: Punkt): Punkt => [(x - mx) / f + VORLAGE[0] / 2, (y - my) / f + VORLAGE[1] / 2];
  const teile = tablett(area, MEER, MIN_Y, blick(p, 0));
  for (const { bild, vorlage } of GEGENSTAENDE) {
    const figur = teile.find((t): t is Figur => t.form === 'figur' && t.bild === bild)!;
    const [x, y] = imFenster(figur.fuss);
    expect(Math.hypot(x - vorlage[0], y - vorlage[1]), `${bild}: ${x.toFixed(1)}, ${y.toFixed(1)}`).toBeLessThanOrEqual(3);
  }
  // Das Bild des Tischs von Ecke zu Ecke des Fensters, dazu sein Rand rundum.
  const { o, a, b } = flaechen(teile).find((t) => t.bild === 'tisch')!;
  for (const [ecke, soll] of [
    [o, [-TISCH_RAND, -TISCH_RAND]],
    [
      [o[0] + a[0] + b[0], o[1] + a[1] + b[1]],
      [VORLAGE[0] + TISCH_RAND, VORLAGE[1] + TISCH_RAND],
    ],
  ] as [Punkt, Punkt][]) {
    const [x, y] = imFenster(ecke);
    expect(Math.hypot(x - soll[0], y - soll[1]), `${x.toFixed(1)}, ${y.toFixed(1)}`).toBeLessThanOrEqual(1);
  }
});

test('jenseits der Vorlage liegt nur Marmor: Er wiederholt sich unter dem Tisch über die ganze Ebene, das Bild des Tischs liegt einmal', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const teile = flaechen(tablett(WELTEN[1]!.area, MEER, MIN_Y, blick(p, k)));
      const name = `${camera} k=${k}`;
      // Fern und vor den Kacheln je erst der Marmor, gleich danach der Tisch.
      for (const nah of [false, true]) {
        const tisch = teile.filter((f) => f.art === 'tisch' && !!f.nurIn === nah);
        expect(tisch.map((f) => [f.bild, f.wiederholt]), name).toEqual([
          ['marmor', true],
          ['tisch', undefined],
        ]);
        expect(teile.indexOf(tisch[1]!) - teile.indexOf(tisch[0]!), name).toBe(1);
      }
      // Der Marmor im Mass des Tischs: MARMOR Pixel der Vorlage je Seite.
      const [marmor, tisch] = [teile.find((f) => f.bild === 'marmor')!, teile.find((f) => f.bild === 'tisch')!];
      const laenge = (v: Punkt) => Math.hypot(...v);
      expect(laenge(marmor.a) / laenge(tisch.a), name).toBeCloseTo(MARMOR / (VORLAGE[0] + 2 * TISCH_RAND), 9);
      expect(laenge(marmor.b) / laenge(tisch.b), name).toBeCloseTo(MARMOR / (VORLAGE[1] + 2 * TISCH_RAND), 9);
    }
  }
});

test('auf zwei Buchrücken steht Text aus SKIN_TEXT_BUCH1 und SKIN_TEXT_BUCH2 gleich nach ihrem Bild, ohne die Texte keiner', () => {
  const texte = { titel: 'Karte', buch1: 'Probe Eins', buch2: 'Probe Zwei' };
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const name = `${camera} k=${k}`;
      expect(tablett(WELTEN[0]!.area, MEER, MIN_Y, blick(p, k), { titel: 'Karte' }).some((t) => t.form === 'schrift'), name).toBe(false);
      const teile = tablett(WELTEN[0]!.area, MEER, MIN_Y, blick(p, k), texte);
      const i = teile.findIndex((t) => t.form === 'figur' && t.bild === 'buecher');
      const buecher = teile[i] as Figur;
      const schriften = teile.filter((t): t is Schrift => t.form === 'schrift');
      expect(teile.slice(i + 1, i + 3), name).toEqual(schriften);
      expect(schriften.map((s) => s.text), name).toEqual(['Probe Eins', 'Probe Zwei']);
      // Jedes Feld liegt im Bild der Bücher, auf derselben Ebene.
      const [links, oben, rechts, unten] = rechteckDer(buecher);
      for (const { o, a, b, nah } of schriften) {
        for (const [x, y] of [o, [o[0] + a[0] + b[0], o[1] + a[1] + b[1]]]) {
          expect(x! > links && x! < rechts && y! > oben && y! < unten, name).toBe(true);
        }
        expect(nah, name).toBe(buecher.nah);
      }
    }
  }
});
