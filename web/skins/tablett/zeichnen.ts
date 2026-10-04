/**
 * Die Teile des Tabletts auf einer Leinwand: Flächen mit ihrem Bild aus der
 * Vorlage, affin gelegt und geglättet; Lilien und Gegenstände aufrecht;
 * alles andere in seiner Farbe. Siehe docs/tablett.md, „Zeichnen“.
 */
import { type Figur, type Flaeche, GRUND, type Schrift, type Teil } from './tablett';

/** Die Bilder aus dem Ordner bilder/, geladen, nach Namen. */
export type Bilder = ReadonlyMap<string, ImageBitmap>;

/**
 * So viele Pixel der Leinwand reicht ein Bild über seine Fläche hinaus.
 * Nachbarn überlappen so, und an ihrer Kante scheint nichts durch.
 */
const UEBERLAPP = 0.75;

/**
 * Die Farben der Schrift auf den Büchern, aus der Vorlage: das helle Gold
 * ihres Schmucks, der Schatten im Leder und ein Glanz darüber.
 */
const SCHRIFT = { gold: '#db9e63', schatten: 'rgb(22 10 5 / 0.85)', licht: 'rgb(240 200 144 / 0.5)' };

/**
 * Legt das Bild einer Fläche affin auf sie: seine Breite entlang a, seine
 * Höhe entlang b. Wiederholt es sich, füllt es die ganze Leinwand.
 */
function lege(ctx: CanvasRenderingContext2D, { o, a, b, wiederholt }: Flaeche, bild: ImageBitmap, s: number, [x0, y0]: [number, number]): void {
  if (wiederholt) {
    const muster = ctx.createPattern(bild, 'repeat')!;
    const [w, h] = [bild.width, bild.height];
    muster.setTransform(new DOMMatrix([(s * a[0]) / w, (s * a[1]) / w, (s * b[0]) / h, (s * b[1]) / h, s * o[0] + x0, s * o[1] + y0]));
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = muster;
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    return;
  }
  const [la, lb] = [Math.hypot(...a) * s, Math.hypot(...b) * s];
  const [da, db] = [Math.min(0.25, UEBERLAPP / la), Math.min(0.25, UEBERLAPP / lb)];
  ctx.setTransform(s * a[0], s * a[1], s * b[0], s * b[1], s * o[0] + x0, s * o[1] + y0);
  ctx.drawImage(bild, -da, -db, 1 + 2 * da, 1 + 2 * db);
}

/**
 * Schreibt Text in Gold auf sein Feld, eingeprägt: Die Kante oben links
 * liegt im Schatten, die unten rechts im Licht. So gross, wie das Feld hoch
 * ist, schmaler, wenn er sonst nicht hineinpasst.
 */
function schreibe(ctx: CanvasRenderingContext2D, { text, o, a, b }: Schrift, s: number, [x0, y0]: [number, number]): void {
  const [la, lb] = [Math.hypot(...a) * s, Math.hypot(...b) * s];
  const schrift = (px: number) => `bold ${px}px Georgia, 'Times New Roman', serif`;
  let groesse = 0.62 * lb;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.font = schrift(groesse);
  const breite = ctx.measureText(text).width;
  if (breite > 0.9 * la) groesse *= (0.9 * la) / breite;
  // Zeile entlang a, aufrecht gegen b, von der Mitte des Felds aus.
  ctx.setTransform(a[0] / Math.hypot(...a), a[1] / Math.hypot(...a), -b[0] / Math.hypot(...b), -b[1] / Math.hypot(...b), s * (o[0] + (a[0] + b[0]) / 2) + x0, s * (o[1] + (a[1] + b[1]) / 2) + y0);
  ctx.font = schrift(groesse);
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  const d = Math.max(0.5, 0.07 * groesse);
  for (const [farbe, dx] of [
    [SCHRIFT.schatten, -d],
    [SCHRIFT.licht, d],
    [SCHRIFT.gold, 0],
  ] as const) {
    ctx.fillStyle = farbe;
    ctx.fillText(text, dx, dx);
  }
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
 * einer Fläche, malt es sie in ihrer Farbe, eine, die sich wiederholt, über
 * die ganze Leinwand; fehlt das einer Figur, nichts.
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
    if (teil.form === 'schrift') {
      schreibe(ctx, teil, s, [x0, y0]);
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
    }
    const bild = teil.form === 'flaeche' && teil.bild !== undefined ? bilder.get(teil.bild) : undefined;
    if (teil.form === 'flaeche' && bild) {
      lege(ctx, teil, bild, s, [x0, y0]);
    } else if (teil.form === 'flaeche') {
      ctx.fillStyle = teil.farbe;
      if (teil.wiederholt) {
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
      } else {
        richte(teil);
        ctx.fillRect(0, 0, 1, 1);
      }
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
