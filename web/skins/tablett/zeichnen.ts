/**
 * Die Teile des Tabletts auf einer Leinwand: Flächen mit ihrem Bild aus der
 * Vorlage, affin gelegt und geglättet; Lilien und Gegenstände aufrecht;
 * alles andere in seiner Farbe. Siehe docs/tablett.md, „Zeichnen“.
 */
import { type Figur, type Flaeche, GRUND, type Teil } from './tablett';

/** Die Bilder aus dem Ordner bilder/, geladen, nach Namen. */
export type Bilder = ReadonlyMap<string, ImageBitmap>;

/**
 * So viele Pixel der Leinwand reicht ein Bild über seine Fläche hinaus.
 * Nachbarn überlappen so, und an ihrer Kante scheint nichts durch.
 */
const UEBERLAPP = 0.75;

/** Legt das Bild einer Fläche affin auf sie: seine Breite entlang a, seine Höhe entlang b. */
function lege(ctx: CanvasRenderingContext2D, { o, a, b }: Flaeche, bild: ImageBitmap, s: number, [x0, y0]: [number, number]): void {
  const [la, lb] = [Math.hypot(...a) * s, Math.hypot(...b) * s];
  const [da, db] = [Math.min(0.25, UEBERLAPP / la), Math.min(0.25, UEBERLAPP / lb)];
  ctx.setTransform(s * a[0], s * a[1], s * b[0], s * b[1], s * o[0] + x0, s * o[1] + y0);
  ctx.drawImage(bild, -da, -db, 1 + 2 * da, 1 + 2 * db);
}

/** Stellt ein freigestelltes Bild aufrecht auf seinen Fuss. */
function stelle(ctx: CanvasRenderingContext2D, { fuss, anker, mass }: Figur, bild: ImageBitmap, s: number, [x0, y0]: [number, number]): void {
  const m = s * mass;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.drawImage(bild, s * fuss[0] + x0 - m * anker[0], s * fuss[1] + y0 - m * anker[1], m * bild.width, m * bild.height);
}

/**
 * Malt die Teile auf eine Leinwand. `s` ist ein Pixel der feinsten Stufe in
 * Pixeln der Leinwand, (`x0`, `y0`) der Punkt (0, 0) darauf. Fehlt das Bild
 * einer Fläche, malt es sie in ihrer Farbe; fehlt das einer Figur, nichts.
 */
function male(ctx: CanvasRenderingContext2D, teile: Teil[], s: number, [x0, y0]: [number, number], bilder: Bilder): void {
  const richte = ({ o, a, b }: { o: number[]; a: number[]; b: number[] }) =>
    ctx.setTransform(s * a[0]!, s * a[1]!, s * b[0]!, s * b[1]!, s * o[0]! + x0, s * o[1]! + y0);
  for (const teil of teile) {
    if (teil.form === 'figur') {
      const bild = bilder.get(teil.bild);
      if (bild) stelle(ctx, teil, bild, s, [x0, y0]);
      continue;
    }
    ctx.save();
    if (teil.form === 'flaeche' && teil.nurIn) {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.beginPath();
      for (const vieleck of teil.nurIn) {
        vieleck.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(s * x + x0, s * y + y0) : ctx.lineTo(s * x + x0, s * y + y0)));
        ctx.closePath();
      }
      ctx.clip();
      ctx.fillStyle = GRUND;
      ctx.fill();
    }
    const bild = teil.form === 'flaeche' && teil.bild !== undefined ? bilder.get(teil.bild) : undefined;
    if (teil.form === 'flaeche' && bild) {
      lege(ctx, teil, bild, s, [x0, y0]);
    } else if (teil.form === 'flaeche') {
      richte(teil);
      ctx.fillStyle = teil.farbe;
      ctx.fillRect(0, 0, 1, 1);
    } else {
      richte(teil);
      const verlauf = ctx.createLinearGradient(0, 0, 0, 1);
      verlauf.addColorStop(0, `rgb(0 0 0 / ${teil.deckkraft})`);
      verlauf.addColorStop(1, 'rgb(0 0 0 / 0)');
      ctx.fillStyle = verlauf;
      ctx.fillRect(0, 0, 1, 1);
    }
    ctx.restore();
  }
}

/**
 * Malt beide Ebenen: fern auf dem Grund alles ausser dem Saum und den
 * Kopien vor den Kacheln, nah, was Gelände nie verdeckt, den Saum und, wie
 * in der Vorlage über den Ecken der Karte, Eckstücke und Lilien.
 * `s` ist ein Pixel der feinsten Stufe in Pixeln der Leinwand, `versatz` der
 * Punkt (0, 0) darauf. Die Bilder werden geglättet gelegt. Siehe
 * docs/tablett.md, „Vor und hinter der Welt“.
 */
export function ebenen(
  fern: CanvasRenderingContext2D,
  nah: CanvasRenderingContext2D,
  teile: Teil[],
  s: number,
  versatz: [number, number],
  bilder: Bilder = new Map(),
): void {
  for (const ctx of [fern, nah]) {
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
  }
  fern.setTransform(1, 0, 0, 1, 0, 0);
  fern.fillStyle = GRUND;
  fern.fillRect(0, 0, fern.canvas.width, fern.canvas.height);
  male(fern, teile.filter((t) => t.form !== 'saum' && !(t.form === 'flaeche' && t.nurIn)), s, versatz, bilder);
  male(nah, teile.filter((t) => t.nah), s, versatz, bilder);
}
