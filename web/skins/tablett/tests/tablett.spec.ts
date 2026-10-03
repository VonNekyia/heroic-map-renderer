import { expect, test } from '@playwright/test';
import type { Grenzen, Projektion, Rechteck } from 'heroic-map-renderer/skin-api';
import { eintraege, kamera, projiziere, RICHTUNGEN } from '../../../tests/kamera';
import { BILDER, DICHTEN, FELD, RAND } from '../atlas';
import { gesamtstufe, grenzen, imBlick, raster, tablett, type Blick, type Flaeche, type Teil } from '../tablett';
import { gitter, stoesse } from '../zeichnen';

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

/** Der Blick, wie ihn die Grundkarte dem Skin reicht. */
const blick = (p: Projektion, k: number): Blick => ({ projektion: p, k, projiziere: (x, y, z) => projiziere(x, y, z, p) });

/** Das Tablett in einem Fenster, das die Karte samt Rahmen zu 70 % füllt. */
function stueck(area: Rechteck, meer: number, minY: number, p: Projektion, k: number): Teil[] {
  return tablett(area, meer, minY, blick(p, k), weiter(grenzen(area, meer, blick(p, k)), 1 / 0.7));
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
    // Ein Stück daneben: Nur in Richtung der Welt liegt kein Rand. Der Rand
    // einer Welt aus einem Block ist 0,013 Blöcke breit; das Stück liegt
    // weit innerhalb davon.
    const [dx, dz] = [projiziere(1, 0, 0, p), projiziere(0, 0, 1, p)];
    const e = 1 / 1024;
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
      const rahmen = grenzen(area, MEER, blick(p, k));
      // Bei der ganzen Karte füllt sie das Fenster zu 70 % oder zur Hälfte;
      // eine Stufe hinaus ist das Fenster doppelt so gross.
      for (const anteil of [0.7, 0.5]) {
        const ansicht = weiter(rahmen, 1 / anteil);
        const alle = flaechen(tablett(area, MEER, MIN_Y, blick(p, k), ansicht));
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
      const ansicht = weiter(grenzen(area, MEER, blick(p, k)), 2);
      const [, oben, , unten] = ansicht;
      const zarge = flaechen(tablett(area, MEER, MIN_Y, blick(p, k), ansicht)).filter((f) => f.art === 'zarge');
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
      const [links, oben, rechts, unten] = grenzen(area, MEER, blick(p, k));
      const rahmen = flaechen(stueck(area, MEER, MIN_Y, p, k)).filter((f) =>
        ['rand', 'wand', 'pfeiler'].includes(f.art),
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

test('das Licht kommt von oben, leicht von links: die linke nahe Wand ist rund 1,6-mal so hell wie die rechte', () => {
  // Summe der Kanäle, aus `rgb(r g b)`.
  const hell = (f: Flaeche) => f.farbe.match(/\d+/g)!.reduce((summe, wert) => summe + Number(wert), 0);
  for (const camera of ['2:1', '8:5']) {
    const teile = flaechen(stueck(WELTEN[0]!.area, MEER, MIN_Y, kamera(camera, 16), 0));
    // Nahe Wände zeigen im Blick nach +z, links im Bild, und nach +x, rechts.
    const wand = (nx: number, nz: number) =>
      hell(teile.find((f) => f.art === 'wand' && f.n[0] === nx && f.n[2] === nz)!);
    const verhaeltnis = wand(0, 1) / wand(1, 0);
    expect(verhaeltnis, camera).toBeGreaterThan(1.4);
    expect(verhaeltnis, camera).toBeLessThan(1.8);
  }
});

test('die fernen Seiten haben eine Innenseite bis zum Boden, die nahen nicht', () => {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const innen = flaechen(stueck(WELTEN[0]!.area, MEER, MIN_Y, p, k)).filter((f) => f.art === 'innen');
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

test('jede Fläche aus Holz am Rahmen und die Holzkante des Tischs haben ihr Bild im Atlas', () => {
  const namen = new Set(BILDER.map((b) => b.name));
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const teile = flaechen(stueck(WELTEN[0]!.area, MEER, MIN_Y, p, k));
      const name = `${camera} k=${k}`;
      for (const f of teile.filter((t) => ['rand', 'wand', 'pfeiler'].includes(t.art))) {
        expect(f.textur && namen.has(f.textur.bild), `${name}: ${f.art} ${String(f.n)}`).toBe(true);
      }
      expect(teile.filter((f) => f.textur?.bild.endsWith('tischkante +z')).length, name).toBe(1);
      for (const f of teile.filter((t) => t.textur)) expect(namen.has(f.textur!.bild), `${name}: ${f.textur!.bild}`).toBe(true);
    }
  }
});

/** Eine quadratische Welt, so gross wie eine echte. */
const GROSS: Rechteck = [-4096, -4096, 4096, 4096];

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
        // Wo es geht, genau 90 %.
        const ziel = voll + Math.log2(0.9);
        if (ziel - Math.floor(ziel) >= 0.5 && ziel >= einpassen) expect(2 ** (z - voll), name).toBeCloseTo(0.9, 9);
      }
    }
  }
});

/**
 * Die Gesamtansicht der grossen Welt in jeder Kamera und Richtung, in
 * Fenstern von Telefonen, hoch und quer, bis 4K: Raster und Flächen mit
 * Textur.
 */
function* gesamtansichten() {
  for (const [camera, scale] of KAMERAS) {
    const p = kamera(camera, scale);
    for (let k = 0; k < 4; k++) {
      const rahmen = grenzen(GROSS, MEER, blick(p, k));
      for (const [breite, hoehe] of [
        [390, 844],
        [844, 390],
        [1280, 720],
        [1491, 1055],
        [1920, 1080],
        [2560, 1440],
        [3840, 2160],
      ] as const) {
        const s = 2 ** (gesamtstufe(rahmen, 11, breite, hoehe) - 11);
        const r = raster(GROSS, p, s);
        const teile = flaechen(tablett(GROSS, MEER, MIN_Y, blick(p, k), weiter(rahmen, 1 / 0.9), r.w)).filter((f) => f.textur);
        yield { name: `${camera} k=${k} ${breite} × ${hoehe}`, p, s, ...r, teile };
      }
    }
  }
}

test('in der Gesamtansicht ist ein Texel waagrecht 1 px breit, auf grossen Schirmen ganze Pixel, an Wänden ebenso hoch', () => {
  const ganz = (wert: number) => Math.abs(wert - Math.round(wert)) < 1e-6;
  const dichtester = DICHTEN[DICHTEN.length - 1]!;
  for (const { name, p, s, dichte, pixel, w, teile } of gesamtansichten()) {
    expect(DICHTEN, name).toContain(dichte);
    // 1 px, solange es einen Atlas so dicht gibt; sonst ganze Pixel.
    const band = RAND * (GROSS[2] - GROSS[0]) * p.u * s;
    expect(Number.isInteger(pixel) && (pixel === 1 || band > dichtester + 0.5), name).toBe(true);
    // Der Rand liegt höchstens ein halbes Texel neben seinem Anteil an der
    // Kante, solange es Atlanten so dünn gibt.
    if (band >= DICHTEN[0] * pixel) expect(Math.abs(w * p.u * s - band), name).toBeLessThanOrEqual(pixel / 2 + 1e-9);
    expect(teile.length, name).toBeGreaterThan(0);
    for (const f of teile) {
      // Ein Texel entlang a und entlang b, in Pixeln des Bildschirms.
      const [nu, nv] = [f.textur!.la * dichte, f.textur!.lb * dichte];
      const was = `${name}: ${f.textur!.bild}`;
      for (const [x, y] of [
        [(f.a[0] * s) / nu, (f.a[1] * s) / nu],
        [(f.b[0] * s) / nv, (f.b[1] * s) / nv],
      ] as const) {
        expect(ganz(x) && [0, pixel].includes(Math.abs(Math.round(x))), was).toBe(true);
        // Wände stehen in diesen Kameras so hoch wie breit.
        if (x === 0 && p.y === p.u) expect(Math.abs(y), was).toBeCloseTo(pixel, 6);
      }
    }
  }
});

test('in der Gesamtansicht deckt jedes Texel mindestens ein Pixel, und keine Pixelmitte liegt auf einer Kante', () => {
  const fehler: string[] = [];
  for (const { name, s, dichte, pixel, teile } of gesamtansichten()) {
    for (const f of teile) {
      const g = gitter(f, dichte, s, [0, 0]);
      const [[ax, ay], [bx, by], [ex, ey], [u0, v0]] = [g.schrittA, g.schrittB, g.ecke, g.anfang];
      const det = ax * by - ay * bx;
      // Bis zu 24 × 24 ganze Texel in der Fläche, ab ihrer Ecke.
      const [i0, j0] = [Math.ceil(u0), Math.ceil(v0)];
      const [i1, j1] = [Math.min(i0 + 24, Math.floor(u0 + g.nu)), Math.min(j0 + 24, Math.floor(v0 + g.nv))];
      const ecken = [i0, i1].flatMap((i) => [j0, j1].map((j) => [ex + i * ax + j * bx, ey + i * ay + j * by] as const));
      const [xs, ys] = [ecken.map((e) => e[0]), ecken.map((e) => e[1])];
      const zahl = new Map<number, number>();
      // Jede Pixelmitte nimmt das Texel, in dem sie liegt.
      for (let y = Math.floor(Math.min(...ys)); y < Math.max(...ys); y++) {
        for (let x = Math.floor(Math.min(...xs)); x < Math.max(...xs); x++) {
          const [dx, dy] = [x + 0.5 - ex, y + 0.5 - ey];
          const [tu, tv] = [(dx * by - dy * bx) / det, (ax * dy - ay * dx) / det];
          if (tu < i0 || tu >= i1 || tv < j0 || tv >= j1) continue;
          if (Math.min(Math.abs(tu - Math.round(tu)), Math.abs(tv - Math.round(tv))) < 1e-3) {
            fehler.push(`${name}: ${f.textur!.bild}, Pixel ${x} ${y} auf einer Kante`);
          }
          const schluessel = (Math.floor(tu) - i0) * 64 + Math.floor(tv) - j0;
          zahl.set(schluessel, (zahl.get(schluessel) ?? 0) + 1);
        }
      }
      // Mit 1 px je Texel so viele Pixel, wie das Texel Fläche hat, ab- oder
      // aufgerundet: in 2:1 genau 1, in 8:5 auf Oberseiten 1 oder 2.
      const [wenig, viel] = pixel > 1 ? [1, Infinity] : [Math.max(1, Math.floor(Math.abs(det) + 1e-9)), Math.ceil(Math.abs(det) - 1e-9)];
      for (let i = i0; i < i1; i++) {
        for (let j = j0; j < j1; j++) {
          const n = zahl.get((i - i0) * 64 + j - j0) ?? 0;
          if (n < wenig || n > viel) fehler.push(`${name}: ${f.textur!.bild}, Texel ${i} ${j} mit ${n} px`);
        }
      }
    }
  }
  expect(fehler.slice(0, 10)).toEqual([]);
});

test('die Stösse des Frieses liegen auf ganzen Texeln, je einer an den Enden, dazwischen gleich weit', () => {
  for (const dichte of DICHTEN) {
    for (const la of [1, FELD, 10, 76.92, 100.37]) {
      for (const anfang of [0, -0.4, 0.7]) {
        const u = stoesse(la, dichte, anfang);
        const name = `${la} w ab ${anfang}, Dichte ${dichte}`;
        expect(u.every(Number.isInteger), name).toBe(true);
        expect([u[0], u.at(-1)], name).toEqual([Math.round(anfang), Math.round(anfang + la * dichte)]);
        const felder = u.slice(1).map((bis, i) => bis - u[i]!);
        expect(Math.max(...felder) - Math.min(...felder), name).toBeLessThanOrEqual(1);
        expect(felder.length, name).toBe(Math.max(1, Math.round(la / FELD)));
      }
    }
  }
});
