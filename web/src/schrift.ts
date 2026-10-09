/**
 * Kartenschrift: ein Name entlang einer gebogenen Linie, in der Schrift der
 * Karte, gesperrt und mit Kontur. Das Format steht in
 * docs/benutzung/ebenen.md, „Kartenschrift“; wie die Karte sie zeichnet in
 * docs/frontend.md, „Ebenen“.
 */
import L from 'leaflet';
import type { Punkt } from './gelaende';
import { farbe, istObjekt, istText, istZahl, punkte } from './pruefen';
import schriftDatei from './ebenen/schrift/IMFeENsc28P.ttf?url&no-inline';

/** Eine Kartenschrift, wie die Ansicht sie braucht. */
export interface Schriftzug {
  id: string;
  text: string;
  pfad: Punkt[];
  /** Höhe der Grossbuchstaben in Blöcken. */
  groesse: number;
  /** Zusätzlicher Abstand zwischen den Zeichen, in Anteilen von `groesse`. */
  sperrung: number;
  farbe: string;
  kontur?: { farbe: string; breite: number };
}

/** Liest eine Kartenschrift; anderes ist `undefined`. Eine unbekannte `font` gilt als `map`. */
export function schriftzug(wert: Record<string, unknown>): Schriftzug | undefined {
  if (wert.type !== 'label' || !istText(wert.id, 64) || !istText(wert.text, 64)) return undefined;
  const pfad = punkte(wert.path, 1, 64);
  if (!pfad) return undefined;
  const kontur = istObjekt(wert.outline) && istZahl(wert.outline.width) && wert.outline.width > 0 ? wert.outline : undefined;
  return {
    id: wert.id,
    text: wert.text,
    pfad,
    groesse: istZahl(wert.size) && wert.size > 0 ? wert.size : 16,
    sperrung: istZahl(wert.spacing) ? wert.spacing : 0,
    farbe: farbe(wert.color) ?? '#2B2B2B',
    kontur: kontur && { farbe: farbe(kontur.color) ?? '#F2E8D0', breite: kontur.width as number },
  };
}

/**
 * Höhe der Grossbuchstaben je Pixel Schriftgrösse: die Oberkante des „H“
 * (1384) durch `unitsPerEm` (2048), aus der Schrift gelesen. Siehe
 * docs/frontend.md, „Ebenen“.
 */
const KAPPE = 1384 / 2048;

/** Grenzen aus docs/benutzung/ebenen.md, „Kartenschrift“, in Pixeln der Höhe der Grossbuchstaben. */
const LESBAR = { min: 8, max: 96 };

/** Die Schrift der Karte, einmal geladen; bis dahin misst der Browser mit einer anderen. */
let schrift: Promise<unknown> | undefined;
function ladeSchrift(): Promise<unknown> {
  schrift ??= new FontFace('Kartenschrift', `url(${schriftDatei})`)
    .load()
    .then((f) => document.fonts.add(f))
    .catch((fehler: unknown) => console.error('Kartenschrift', fehler));
  return schrift;
}

const SVG = 'http://www.w3.org/2000/svg';
let zaehler = 0;

/**
 * Eine Kartenschrift auf der Karte: ein eigenes SVG im Pane der Ebene, neu
 * gesetzt bei jedem Zoom. `pfad` in Pixeln der feinsten Stufe,
 * `faktor()` Pixel des Schirms je Pixel der feinsten Stufe, `scale` Pixel
 * der feinsten Stufe je Block.
 */
export class Schrift extends L.Layer {
  private svg?: SVGSVGElement;
  private karte?: L.Map;

  constructor(
    private readonly zug: Schriftzug,
    private readonly pfad: readonly Punkt[],
    private readonly faktor: () => number,
    private readonly scale: number,
    pane: string,
  ) {
    super({ pane });
  }

  override onAdd(map: L.Map): this {
    const svg = document.createElementNS(SVG, 'svg');
    svg.classList.add('ebene-schrift', 'leaflet-zoom-hide');
    svg.dataset.id = this.zug.id;
    const id = `ebene-schrift-${++zaehler}`;
    const linie = document.createElementNS(SVG, 'path');
    linie.id = id;
    // Nur die Führung der Schrift, selbst unsichtbar.
    linie.setAttribute('fill', 'none');
    const text = document.createElementNS(SVG, 'text');
    const entlang = document.createElementNS(SVG, 'textPath');
    entlang.setAttribute('href', `#${id}`);
    entlang.setAttribute('startOffset', '50%');
    entlang.textContent = this.zug.text;
    text.append(entlang);
    svg.append(linie, text);
    map.getPane(this.options.pane!)!.append(svg);
    this.svg = svg;
    this.karte = map;
    map.on('zoomend viewreset', this.zeichne);
    this.zeichne();
    void ladeSchrift().then(() => this.svg && this.zeichne());
    return this;
  }

  override onRemove(map: L.Map): this {
    map.off('zoomend viewreset', this.zeichne);
    this.svg?.remove();
    this.svg = this.karte = undefined;
    return this;
  }

  /**
   * Setzt die Schrift für den Zoom: Höhe der Grossbuchstaben
   * `groesse · scale · faktor`, unter 8 Pixeln aus, über 96 gedeckelt. Läuft
   * der Pfad auf dem Schirm nach links, kehrt er um, damit die Schrift
   * aufrecht steht. Ist er kürzer als der Text, geht er an beiden Enden in
   * Richtung seines letzten Stücks weiter.
   */
  private readonly zeichne = (): void => {
    const { svg, karte: map } = this;
    if (!svg || !map) return;
    const kappe = this.zug.groesse * this.scale * this.faktor();
    svg.style.display = kappe < LESBAR.min ? 'none' : '';
    if (kappe < LESBAR.min) return;
    const hoehe = Math.min(kappe, LESBAR.max);
    const groesse = hoehe / KAPPE;
    const text = svg.querySelector('text')!;
    const entlang = svg.querySelector('textPath')!;
    text.setAttribute('font-size', String(groesse));
    text.setAttribute('letter-spacing', String(this.zug.sperrung * hoehe));
    text.setAttribute('fill', this.zug.farbe);
    // Die Grundlinie eine halbe Höhe der Grossbuchstaben unter dem Pfad: Die Schrift steht mittig auf ihm.
    entlang.setAttribute('dy', String(hoehe / 2));
    if (this.zug.kontur) {
      // Die Kontur liegt unter den Zeichen; sichtbar bleibt ihre äussere Hälfte.
      text.setAttribute('stroke', this.zug.kontur.farbe);
      text.setAttribute('stroke-width', String(this.zug.kontur.breite * 2));
      text.setAttribute('stroke-linejoin', 'round');
      text.setAttribute('paint-order', 'stroke');
    }

    let punkte = this.pfad.map(([x, y]) => map.latLngToLayerPoint(L.latLng(y, x)));
    if (punkte.at(-1)!.x < punkte[0]!.x) punkte = punkte.reverse();
    const laenge = punkte.reduce((s, p, i) => (i ? s + p.distanceTo(punkte[i - 1]!) : 0), 0);
    const noetig = entlang.getComputedTextLength() + hoehe;
    if (noetig > laenge) {
      const mehr = (noetig - laenge) / 2;
      // Die Richtung von `von` nach `nach`; ein einzelner Punkt heisst waagrecht.
      const richtung = (von: L.Point, nach: L.Point, waagrecht: L.Point) =>
        von.equals(nach) ? waagrecht : nach.subtract(von).divideBy(nach.distanceTo(von));
      const [erster, letzter] = [punkte[0]!, punkte.at(-1)!];
      punkte = [
        erster.add(richtung(punkte[1] ?? erster, erster, L.point(-1, 0)).multiplyBy(mehr)),
        ...punkte,
        letzter.add(richtung(punkte.at(-2) ?? letzter, letzter, L.point(1, 0)).multiplyBy(mehr)),
      ];
    }
    const rand = groesse + (this.zug.kontur?.breite ?? 0);
    const links = L.bounds(punkte).min!.subtract(L.point(rand, rand));
    const rechts = L.bounds(punkte).max!.add(L.point(rand, rand));
    L.DomUtil.setPosition(svg as unknown as HTMLElement, links);
    svg.setAttribute('width', String(rechts.x - links.x));
    svg.setAttribute('height', String(rechts.y - links.y));
    svg.querySelector('path')!.setAttribute('d', 'M' + punkte.map((p) => `${p.x - links.x} ${p.y - links.y}`).join('L'));
  };
}
