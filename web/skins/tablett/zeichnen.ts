/**
 * Die Teile des Tabletts auf einer Leinwand: Flächen mit Textur als Bild aus
 * dem Atlas, affin gelegt nach dem nächsten Nachbarn; der Marmor als Muster
 * im Bild; alles andere in seiner Farbe. Siehe docs/tablett.md, „Zeichnen“.
 */
import type { Grenzen } from 'heroic-map-renderer/skin-api';
import { type Bereich, FELD, stoss } from './atlas';
import { type Flaeche, GRUND, type Schatten, type Teil } from './tablett';

/** Die Bilder aus dem Ordner bilder/, geladen. */
export interface Bilder {
  /** Der Atlas einer Dichte; jedes Bild, das sich wiederholt, auch für sich, für sein Muster. */
  atlas: { dichte: number; bild: ImageBitmap; bereiche: Map<string, Bereich>; einzeln: Map<string, ImageBitmap> };
  marmor: ImageBitmap;
}

/**
 * Wo an einer Wand der Länge `la` in w die Stösse des Frieses liegen, in
 * Texeln des Gitters, wenn die Wand bei `anfang` beginnt: je einer an den
 * Enden, dazwischen ganze Felder so nahe an `FELD` wie möglich, gleich
 * weit, jeder Stoss auf einem ganzen Texel.
 */
export function stoesse(la: number, dichte: number, anfang = 0): number[] {
  const felder = Math.max(1, Math.round(la / FELD));
  return Array.from({ length: felder + 1 }, (_, i) => Math.round(anfang + (i * la * dichte) / felder));
}

/** Senkrecht liegen die Ecken des Texelgitters so weit unter dem Pixelraster. */
const UNTER_DEM_RASTER = 1 / 64;

/**
 * Das Texelgitter einer Fläche mit Textur auf der Leinwand: die Schritte
 * eines Texels entlang a und b in Pixeln, eine Ecke des Gitters und wo die
 * Fläche darin beginnt, in Texeln. Die Ecken liegen senkrecht
 * `UNTER_DEM_RASTER` unter dem Pixelraster; waagrecht auf Pixelmitten, wenn
 * beide Schritte waagrecht laufen, wie auf Oberseiten diagonal, sonst auf
 * Pixelkanten. So deckt jedes Texel mindestens ein Pixel, und keine
 * Pixelmitte liegt auf einer Kante. Dafür rückt das Gitter gegen die Fläche
 * um höchstens ein halbes Pixel je Richtung, also um höchstens ein Texel.
 * `s` ist ein Pixel der feinsten Stufe in Pixeln der Leinwand, (`x0`, `y0`)
 * der Punkt (0, 0) darauf. Siehe docs/tablett.md, „Ganze Pixel“.
 */
export function gitter({ o, a, b, textur }: Flaeche, dichte: number, s: number, [x0, y0]: [number, number]) {
  const [nu, nv] = [textur!.la * dichte, textur!.lb * dichte];
  const schrittA: [number, number] = [(s * a[0]) / nu, (s * a[1]) / nu];
  const schrittB: [number, number] = [(s * b[0]) / nv, (s * b[1]) / nv];
  const [px, py] = [s * o[0] + x0, s * o[1] + y0];
  const mitte = Math.abs(schrittA[0]) > 1e-9 && Math.abs(schrittB[0]) > 1e-9;
  // Zum nächsten Punkt mit diesem Rest, um höchstens ein halbes Pixel.
  const rueck = (wert: number, rest: number) => rest - wert - Math.round(rest - wert);
  const [dx, dy] = [rueck(px, mitte ? 0.5 : 0), rueck(py, UNTER_DEM_RASTER)];
  // Derselbe Weg in Texeln: (dx, dy) = du · schrittA + dv · schrittB.
  const det = schrittA[0] * schrittB[1] - schrittA[1] * schrittB[0];
  const du = (dx * schrittB[1] - dy * schrittB[0]) / det;
  const dv = (schrittA[0] * dy - schrittA[1] * dx) / det;
  return { schrittA, schrittB, ecke: [px + dx, py + dy] as [number, number], anfang: [-du, -dv] as [number, number], nu, nv };
}

/**
 * Legt das Bild einer Fläche auf die Leinwand, affin nach dem nächsten
 * Nachbarn, im Gitter von `gitter`: je w so viele Texel entlang a und b wie
 * die Dichte. Ein Bild, das sich wiederholt, liegt als Muster darauf, eines,
 * das es nicht tut, mit seinem Anschnitt; beide nur innerhalb der Fläche.
 * Auf eine Wand kommen danach die Stösse des Frieses, je auf ein ganzes
 * Texel. Ohne ihr Bild im Atlas legt es nichts und sagt es.
 */
function lege(
  ctx: CanvasRenderingContext2D,
  flaeche: Flaeche,
  atlas: Bilder['atlas'],
  s: number,
  versatz: [number, number],
): boolean {
  const { bild, stoss: stossBild, la } = flaeche.textur!;
  const bereich = atlas.bereiche.get(bild);
  if (!bereich) return false;
  const { schrittA, schrittB, ecke, anfang, nu, nv } = gitter(flaeche, atlas.dichte, s, versatz);
  ctx.setTransform(schrittA[0], schrittA[1], schrittB[0], schrittB[1], ecke[0], ecke[1]);
  ctx.save();
  ctx.beginPath();
  ctx.rect(anfang[0], anfang[1], nu, nv);
  ctx.clip();
  if (bereich.periodisch) {
    // Die erste Zeile des Musters ist sein Anschnitt.
    const muster = ctx.createPattern(atlas.einzeln.get(bild)!, 'repeat')!;
    muster.setTransform(new DOMMatrix([1, 0, 0, 1, 0, -1]));
    ctx.fillStyle = muster;
    ctx.fillRect(anfang[0], anfang[1], nu, nv);
  } else {
    // Ringsum ein Texel Anschnitt.
    const { x, y, breite, hoehe } = bereich;
    ctx.drawImage(atlas.bild, x, y, breite, hoehe, -1, -1, breite, hoehe);
  }
  const fuge = stossBild === undefined ? undefined : atlas.bereiche.get(stossBild);
  if (fuge) {
    // Die Stösse an den Enden reichen halb über die Wand hinaus.
    const { halb, von } = stoss(atlas.dichte);
    for (const u of stoesse(la, atlas.dichte, anfang[0])) {
      ctx.drawImage(atlas.bild, fuge.x, fuge.y, fuge.breite, fuge.hoehe, u - halb, von, fuge.breite, fuge.hoehe);
    }
  }
  ctx.restore();
  return true;
}

/** Das Licht auf dem Marmor, vom Punkt unter der Lichtquelle bis in die fernste Ecke. */
const SCHIMMER = [
  [0, 'rgb(110 70 40 / 0.08)'],
  [0.3, 'rgb(110 70 40 / 0)'],
  [0.5, 'rgb(0 0 0 / 0)'],
  [0.8, 'rgb(0 0 0 / 0.32)'],
  [1, 'rgb(0 0 0 / 0.4)'],
] as const;

/**
 * Die Gesamtansicht: ihr Fenster in Pixeln der feinsten Stufe und ein Pixel
 * der feinsten Stufe darin, in Pixeln des Bildschirms.
 */
export interface Gesamt {
  ansicht: Grenzen;
  sGesamt: number;
}

/**
 * Der Marmor über die ganze Leinwand: die Kachel, darüber das Licht. Die
 * Kachel liegt fest auf der Welt, in der Gesamtansicht Pixel auf Pixel, auf
 * näheren Stufen so viel grösser wie alles andere, nach dem nächsten
 * Nachbarn. Das Licht kommt von oben, leicht von links: Um einen Punkt am
 * oberen Rand der Gesamtansicht, bei 0,4 ihrer Breite, schimmert die Platte
 * warm, weiter weg wird sie dunkler. Jede Fläche aus Marmor nimmt daraus ihr
 * Stück, so laufen Muster und Licht über die Stösse der Platte.
 */
function marmor(
  kachel: ImageBitmap,
  breite: number,
  hoehe: number,
  s: number,
  [x0, y0]: [number, number],
  { ansicht, sGesamt }: Gesamt,
): OffscreenCanvas {
  const leinwand = new OffscreenCanvas(breite, hoehe);
  const ctx = leinwand.getContext('2d')!;
  ctx.imageSmoothingEnabled = false;
  const muster = ctx.createPattern(kachel, 'repeat')!;
  const gross = s / sGesamt;
  muster.setTransform(new DOMMatrix([gross, 0, 0, gross, x0, y0]));
  ctx.fillStyle = muster;
  ctx.fillRect(0, 0, breite, hoehe);
  const [links, oben, rechts, unten] = ansicht;
  const [lx, ly] = [s * (links + 0.4 * (rechts - links)) + x0, s * oben + y0];
  const licht = ctx.createRadialGradient(lx, ly, 0, lx, ly, s * Math.hypot(rechts - links, unten - oben));
  for (const [stelle, farbe] of SCHIMMER) licht.addColorStop(stelle, farbe);
  ctx.fillStyle = licht;
  ctx.fillRect(0, 0, breite, hoehe);
  return leinwand;
}

/** Was ein Schatten zum Malen braucht, auf einer Leinwand oder ausserhalb. */
type Pinsel = CanvasTransform & CanvasShadowStyles & CanvasFillStrokeStyles & CanvasPath & CanvasDrawPath;

/** Ab so viel Unschärfe in Pixeln wird ein Schatten verkleinert gerechnet. */
const UNSCHAERFE = 24;

/**
 * Wirft einen Schatten weich auf eine Leinwand der Breite `breite`, über den
 * Schatten des Canvas: Die Vielecke liegen weit ausserhalb, ihr Schatten
 * fällt zurück an ihren Platz.
 */
function wirf(ctx: Pinsel, breite: number, teil: Schatten, s: number, [x0, y0]: [number, number]): void {
  const unschaerfe = teil.weich * s;
  const weg = breite + 4 * unschaerfe + 100;
  ctx.setTransform(s, 0, 0, s, x0 + weg, y0);
  ctx.shadowColor = `rgb(0 0 0 / ${teil.deckkraft})`;
  ctx.shadowBlur = unschaerfe;
  ctx.shadowOffsetX = -weg;
  ctx.fillStyle = '#000';
  ctx.beginPath();
  for (const vieleck of teil.vielecke) {
    vieleck.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
    ctx.closePath();
  }
  ctx.fill();
}

/**
 * Malt die Teile auf eine Leinwand. `s` ist ein Pixel der feinsten Stufe in
 * Pixeln der Leinwand, (`x0`, `y0`) der Punkt (0, 0) darauf. Flächen mit
 * Textur nehmen ihr Bild aus `bilder`, der Marmor sein Stück aus `platte`;
 * fehlen sie, malt es die Fläche in ihrer Farbe. Vor den Kacheln (`nah`)
 * fällt der Schatten nur auf das, was schon gemalt ist: die nahen Stücke der
 * Platte. So glättet seine Kante wie die ihre, und an der Grenze zur fernen
 * Platte bleibt keine Linie.
 */
function male(
  ctx: CanvasRenderingContext2D,
  teile: Teil[],
  s: number,
  [x0, y0]: [number, number],
  nah: boolean,
  bilder?: Bilder,
  platte?: OffscreenCanvas,
): void {
  const richte = ({ o, a, b }: { o: number[]; a: number[]; b: number[] }) =>
    ctx.setTransform(s * a[0]!, s * a[1]!, s * b[0]!, s * b[1]!, s * o[0]! + x0, s * o[1]! + y0);
  for (const teil of teile) {
    if (teil.form === 'flaeche' && teil.textur && bilder && lege(ctx, teil, bilder.atlas, s, [x0, y0])) continue;
    if (teil.form === 'flaeche' && teil.muster && platte) {
      // Das Muster liegt fest im Bild; der Pfad gibt nur die Fläche.
      const { o, a, b } = teil;
      const ecke = (i: number, j: number): [number, number] => [
        s * (o[0] + i * a[0] + j * b[0]) + x0,
        s * (o[1] + i * a[1] + j * b[1]) + y0,
      ];
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = ctx.createPattern(platte, 'no-repeat')!;
      ctx.beginPath();
      ctx.moveTo(...ecke(0, 0));
      ctx.lineTo(...ecke(1, 0));
      ctx.lineTo(...ecke(1, 1));
      ctx.lineTo(...ecke(0, 1));
      ctx.fill();
    } else if (teil.form === 'flaeche') {
      richte(teil);
      ctx.fillStyle = teil.farbe;
      ctx.fillRect(0, 0, 1, 1);
    } else if (teil.form === 'saum') {
      richte(teil);
      const verlauf = ctx.createLinearGradient(0, 0, 0, 1);
      verlauf.addColorStop(0, `rgb(0 0 0 / ${teil.deckkraft})`);
      verlauf.addColorStop(1, 'rgb(0 0 0 / 0)');
      ctx.fillStyle = verlauf;
      ctx.fillRect(0, 0, 1, 1);
    } else {
      ctx.save();
      if (nah) ctx.globalCompositeOperation = 'source-atop';
      // Auf näheren Stufen wird ein Schatten sehr weich. Dann wird er
      // verkleinert gerechnet und geglättet vergrössert: So hängen die
      // Kosten am Fenster, nicht an der Stufe.
      const f = Math.min(1, UNSCHAERFE / (teil.weich * s));
      if (f === 1) {
        wirf(ctx, ctx.canvas.width, teil, s, [x0, y0]);
      } else {
        const klein = new OffscreenCanvas(Math.ceil(ctx.canvas.width * f), Math.ceil(ctx.canvas.height * f));
        wirf(klein.getContext('2d')!, klein.width, teil, s * f, [x0 * f, y0 * f]);
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.imageSmoothingEnabled = true;
        ctx.drawImage(klein, 0, 0, klein.width / f, klein.height / f);
      }
      ctx.restore();
    }
  }
}

/**
 * Malt beide Ebenen: fern auf dem Grund alles ausser dem Saum, nah nur, was
 * Gelände nie verdeckt, und den Saum. `s` ist ein Pixel der feinsten Stufe
 * in Pixeln der Leinwand, `versatz` der Punkt (0, 0) darauf. Bilder werden
 * nach dem nächsten Nachbarn gelegt, nie geglättet. Ohne `bilder` bleiben
 * alle Flächen in ihrer Farbe. Siehe docs/tablett.md, „Vor und hinter der
 * Welt“.
 */
export function ebenen(
  fern: CanvasRenderingContext2D,
  nah: CanvasRenderingContext2D,
  teile: Teil[],
  s: number,
  versatz: [number, number],
  gesamt: Gesamt,
  bilder?: Bilder,
): void {
  const [breite, hoehe] = [fern.canvas.width, fern.canvas.height];
  const platte = bilder && marmor(bilder.marmor, breite, hoehe, s, versatz, gesamt);
  for (const ctx of [fern, nah]) ctx.imageSmoothingEnabled = false;
  fern.setTransform(1, 0, 0, 1, 0, 0);
  fern.fillStyle = GRUND;
  fern.fillRect(0, 0, breite, hoehe);
  male(fern, teile.filter((t) => t.form !== 'saum'), s, versatz, false, bilder, platte);
  male(nah, teile.filter((t) => t.nah), s, versatz, true, bilder, platte);
}
