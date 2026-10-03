/**
 * Die Teile des Tabletts auf einer Leinwand, im Fenster wie im Worker.
 * Siehe docs/tablett.md, „Zeichnen“.
 */
import { ADERFARBEN, adern, KACHEL, type Pixel } from './stoffe';
import { type Flaeche, GRUND, type Teil } from './tablett';

type Ctx = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

/**
 * Der Marmor über die ganze Leinwand: der Grund aus der Kachel `grund`
 * (RGBA, `KACHEL` im Quadrat), darüber die Adern. Jede Fläche aus Marmor
 * nimmt daraus ihr Stück, so laufen die Adern über die Stösse der Platte.
 */
export function marmor(grund: Uint8ClampedArray<ArrayBuffer>, breite: number, hoehe: number, streckung: number): OffscreenCanvas {
  const kachel = new OffscreenCanvas(KACHEL, KACHEL);
  kachel.getContext('2d')!.putImageData(new ImageData(grund, KACHEL), 0, 0);
  const leinwand = new OffscreenCanvas(breite, hoehe);
  const ctx = leinwand.getContext('2d')!;
  ctx.fillStyle = ctx.createPattern(kachel, 'repeat')!;
  ctx.fillRect(0, 0, breite, hoehe);
  // Je Farbe ein Pfad: Stücke der Adern, die breit genug für sie sind.
  ctx.lineCap = 'round';
  const alle = adern(breite, hoehe, streckung);
  for (const [i, { ab, farbe }] of ADERFARBEN.entries()) {
    const bis = ADERFARBEN[i + 1]?.ab ?? Infinity;
    ctx.strokeStyle = farbe;
    ctx.lineWidth = Math.min(bis, 1.8);
    ctx.beginPath();
    for (const ader of alle) {
      for (let j = 1; j < ader.length; j++) {
        const [p, q] = [ader[j - 1]!, ader[j]!];
        const breit = (p.breite + q.breite) / 2;
        if (breit < ab || breit >= bis) continue;
        ctx.moveTo(p.x, p.y);
        ctx.lineTo(q.x, q.y);
      }
    }
    ctx.stroke();
  }
  return leinwand;
}

/**
 * Legt aufeinanderfolgende Flächen mit Textur über ein Bild ihres
 * gemeinsamen Rechtecks auf die Leinwand, in ihrer Reihenfolge.
 */
function setze(ctx: Ctx, flaechen: Pixel[]): void {
  const breite = ctx.canvas.width;
  const [links, oben, rechts, unten] = flaechen.reduce(
    ([l, o, r, u], { rahmen }) => [
      Math.min(l, rahmen[0]),
      Math.min(o, rahmen[1]),
      Math.max(r, rahmen[2]),
      Math.max(u, rahmen[3]),
    ],
    [Infinity, Infinity, 0, 0],
  );
  if (rechts <= links || unten <= oben) return;
  const daten = new ImageData(rechts - links, unten - oben);
  const ziel = new Uint32Array(daten.data.buffer);
  for (const { stellen, farben } of flaechen) {
    for (let i = 0; i < stellen.length; i++) {
      const y = (stellen[i]! / breite) | 0;
      ziel[(y - oben) * daten.width + stellen[i]! - y * breite - links] = farben[i]!;
    }
  }
  const bild = new OffscreenCanvas(daten.width, daten.height);
  bild.getContext('2d')!.putImageData(daten, 0, 0);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.drawImage(bild, links, oben);
}

/**
 * Malt die Teile auf eine Leinwand. `s` ist ein Pixel der feinsten Stufe in
 * Pixeln der Leinwand, (`x0`, `y0`) der Punkt (0, 0) darauf. Flächen mit
 * Textur nehmen ihre Pixel aus `pixel`, der Marmor sein Stück aus `marmor`;
 * fehlen sie, malt es die Fläche in ihrer Farbe. Vor den Kacheln (`nah`)
 * fällt der Schatten nur auf das, was schon gemalt ist: die nahen Stücke
 * der Platte. So glättet seine Kante wie die ihre, und an der Grenze zur
 * fernen Platte bleibt keine Linie.
 */
function male(
  ctx: Ctx,
  teile: Teil[],
  s: number,
  [x0, y0]: [number, number],
  nah: boolean,
  pixel?: Map<Flaeche, Pixel>,
  marmor?: OffscreenCanvas,
): void {
  const lege = ({ o, a, b }: { o: number[]; a: number[]; b: number[] }) =>
    ctx.setTransform(s * a[0]!, s * a[1]!, s * b[0]!, s * b[1]!, s * o[0]! + x0, s * o[1]! + y0);
  let folge: Pixel[] = [];
  for (const teil of teile) {
    const gerechnet = teil.form === 'flaeche' ? pixel?.get(teil) : undefined;
    if (gerechnet) {
      folge.push(gerechnet);
      continue;
    }
    if (folge.length > 0) {
      setze(ctx, folge);
      folge = [];
    }
    if (teil.form === 'flaeche' && teil.muster && marmor) {
      // Das Muster liegt fest im Bild; der Pfad gibt nur die Fläche.
      const { o, a, b } = teil;
      const ecke = (i: number, j: number): [number, number] => [
        s * (o[0] + i * a[0] + j * b[0]) + x0,
        s * (o[1] + i * a[1] + j * b[1]) + y0,
      ];
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = ctx.createPattern(marmor, 'no-repeat')!;
      ctx.beginPath();
      ctx.moveTo(...ecke(0, 0));
      ctx.lineTo(...ecke(1, 0));
      ctx.lineTo(...ecke(1, 1));
      ctx.lineTo(...ecke(0, 1));
      ctx.fill();
    } else if (teil.form === 'flaeche') {
      lege(teil);
      ctx.fillStyle = teil.farbe;
      ctx.fillRect(0, 0, 1, 1);
    } else if (teil.form === 'saum') {
      lege(teil);
      const verlauf = ctx.createLinearGradient(0, 0, 0, 1);
      verlauf.addColorStop(0, `rgb(0 0 0 / ${teil.deckkraft})`);
      verlauf.addColorStop(1, 'rgb(0 0 0 / 0)');
      ctx.fillStyle = verlauf;
      ctx.fillRect(0, 0, 1, 1);
    } else {
      ctx.save();
      if (nah) ctx.globalCompositeOperation = 'source-atop';
      // Weich über den Schatten des Canvas: Die Vielecke liegen weit
      // ausserhalb, ihr Schatten fällt zurück an ihren Platz.
      const unschaerfe = teil.weich * s;
      const weg = ctx.canvas.width + 4 * unschaerfe + 100;
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
      ctx.restore();
    }
  }
  if (folge.length > 0) setze(ctx, folge);
}

/**
 * Malt beide Ebenen. Fern auf dem Grund alles ausser dem Saum, nah nur, was
 * Gelände nie verdeckt, und den Saum. Mit Texturen ist nah das ferne Bild,
 * beschnitten auf die Umrisse der nahen Stücke: So malt jedes Stück nur
 * einmal. Ohne sie ist es billiger, nah gleich zu malen. Siehe
 * docs/tablett.md, „Vor und hinter der Welt“.
 */
export function ebenen(
  fern: Ctx,
  nah: Ctx,
  teile: Teil[],
  s: number,
  versatz: [number, number],
  pixel?: Map<Flaeche, Pixel>,
  marmor?: OffscreenCanvas,
): void {
  fern.fillStyle = GRUND;
  fern.fillRect(0, 0, fern.canvas.width, fern.canvas.height);
  male(fern, teile.filter((t) => t.form !== 'saum'), s, versatz, false, pixel, marmor);
  if (!pixel) {
    male(nah, teile.filter((t) => t.nah), s, versatz, true);
    return;
  }
  // Erst die Umrisse der nahen Stücke, dann das ferne Bild nur darin.
  male(nah, teile.filter((t) => t.nah && t.form !== 'saum'), s, versatz, true);
  nah.setTransform(1, 0, 0, 1, 0, 0);
  nah.globalCompositeOperation = 'source-in';
  nah.drawImage(fern.canvas, 0, 0);
  nah.globalCompositeOperation = 'source-over';
  male(nah, teile.filter((t) => t.form === 'saum'), s, versatz, true);
}
