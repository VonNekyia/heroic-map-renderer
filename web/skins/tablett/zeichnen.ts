/** Die Teile des Tabletts auf einer Leinwand. Siehe docs/tablett.md, „Zeichnen“. */
import type { Teil } from './tablett';

/**
 * Malt die Teile auf eine Leinwand. `s` ist ein Pixel der feinsten Stufe in
 * Pixeln der Leinwand, (`x0`, `y0`) der Punkt (0, 0) darauf. Vor den
 * Kacheln (`nah`) fällt der Schatten nur auf das, was schon gemalt ist: die
 * nahen Stücke der Platte. So glättet seine Kante wie die ihre, und an der
 * Grenze zur fernen Platte bleibt keine Linie.
 */
export function male(
  ctx: CanvasRenderingContext2D,
  teile: Teil[],
  s: number,
  [x0, y0]: [number, number],
  nah: boolean,
): void {
  const lege = ({ o, a, b }: { o: number[]; a: number[]; b: number[] }) =>
    ctx.setTransform(s * a[0]!, s * a[1]!, s * b[0]!, s * b[1]!, s * o[0]! + x0, s * o[1]! + y0);
  for (const teil of teile) {
    if (teil.form === 'flaeche') {
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
}
