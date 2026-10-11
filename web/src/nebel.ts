/**
 * Der Nebel einer Fläche im iso: ein gekacheltes Wolkenmuster in ihrer
 * Farbe, zum Weiss hin gemischt, als `pattern` aus SVG. Kein Bild, denn die
 * Content-Security-Policy erlaubt kein `data:`, und kein Filter. Siehe
 * docs/benutzung/ebenen.md, „Der Nebel“, und 0104.
 */

const NS = 'http://www.w3.org/2000/svg';

/** Kante der Kachel in Pixeln des Schirms, Anteil Weiss, Gewicht der Wolken gegen den Grund, Zahl der Wolken. */
const NEBEL = { kachel: 128, weiss: 0.5, wolken: 0.9, anzahl: 18 };

/** Die Muster je Farbe, angelegt erst mit der ersten Fläche im iso. */
const muster = new Map<string, string>();
let ablage: SVGDefsElement | undefined;

/** Die Ablage der Muster: ein leeres SVG im Dokument, erst bei Bedarf. */
function defs(): SVGDefsElement {
  if (!ablage) {
    const svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('width', '0');
    svg.setAttribute('height', '0');
    svg.setAttribute('aria-hidden', 'true');
    svg.style.position = 'absolute';
    ablage = document.createElementNS(NS, 'defs');
    svg.append(ablage);
    document.body.append(svg);
  }
  return ablage;
}

/** Die Farbe `#RRGGBB` zum Weiss hin gemischt. */
export function hell(farbe: string, weiss = NEBEL.weiss): string {
  const kanal = (k: number) => Math.round(parseInt(farbe.slice(k, k + 2), 16) * (1 - weiss) + 255 * weiss);
  return `#${[1, 3, 5].map((k) => kanal(k).toString(16).padStart(2, '0')).join('').toUpperCase()}`;
}

/**
 * Die Füllung des Nebels zu `#RRGGBB`, `url(#nebel-RRGGBB)`: auf einem dünnen
 * Grund weiche Ellipsen mit radialem Verlauf, flach wie der Boden im Bild.
 * Ellipsen am Rand der Kachel kommen gegenüber noch einmal, so ist sie
 * nahtlos. Die Lage folgt einem festen Zufall: Jedes Muster sieht gleich aus.
 */
export function nebel(farbe: string): string {
  const fertig = muster.get(farbe);
  if (fertig) return fertig;
  const n = NEBEL.kachel;
  const id = `nebel-${farbe.slice(1)}`;
  const licht = hell(farbe);
  const verlauf = document.createElementNS(NS, 'radialGradient');
  verlauf.id = `${id}-wolke`;
  for (const [ort, deckkraft] of [[0, 1], [0.6, 0.45], [1, 0]] as const) {
    const stop = document.createElementNS(NS, 'stop');
    stop.setAttribute('offset', String(ort));
    stop.setAttribute('stop-color', licht);
    stop.setAttribute('stop-opacity', String(deckkraft));
    verlauf.append(stop);
  }
  const kachel = document.createElementNS(NS, 'pattern');
  kachel.id = id;
  kachel.setAttribute('patternUnits', 'userSpaceOnUse');
  kachel.setAttribute('width', String(n));
  kachel.setAttribute('height', String(n));
  const grund = document.createElementNS(NS, 'rect');
  grund.setAttribute('width', String(n));
  grund.setAttribute('height', String(n));
  grund.setAttribute('fill', licht);
  grund.setAttribute('fill-opacity', String(1 - NEBEL.wolken));
  kachel.append(grund);
  let saat = 11;
  const zufall = () => (saat = (saat * 1103515245 + 12345) % 2147483648) / 2147483648;
  for (let k = 0; k < NEBEL.anzahl; k++) {
    const [x, y, r] = [zufall() * n, zufall() * n, n * (0.08 + 0.2 * zufall())];
    for (const dx of [-n, 0, n]) {
      for (const dy of [-n, 0, n]) {
        if (x + dx + r < 0 || x + dx - r > n || y + dy + r / 2 < 0 || y + dy - r / 2 > n) continue;
        const wolke = document.createElementNS(NS, 'ellipse');
        wolke.setAttribute('cx', String(x + dx));
        wolke.setAttribute('cy', String(y + dy));
        wolke.setAttribute('rx', String(r));
        wolke.setAttribute('ry', String(r / 2));
        wolke.setAttribute('fill', `url(#${id}-wolke)`);
        wolke.setAttribute('fill-opacity', String(NEBEL.wolken));
        kachel.append(wolke);
      }
    }
  }
  defs().append(verlauf, kachel);
  muster.set(farbe, `url(#${id})`);
  return `url(#${id})`;
}
