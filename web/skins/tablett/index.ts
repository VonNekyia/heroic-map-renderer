/**
 * Der Skin „Tablett“: die Welt in einem Holztablett auf einem Tisch. Er
 * zeichnet einmal für `fitZoom` zwei Bilder, fern unter den Kacheln und nah
 * darüber, und legt sie als Bild-Ebenen auf die Karte. Beim Ziehen und
 * Zoomen zeichnet er nichts. Siehe docs/tablett.md.
 */
import type { Grenzen, Rechteck, Skin } from 'heroic-map-renderer/skin-api';
import L from 'leaflet';
import { grenzen, lichtUndBlick, tablett } from './tablett';
import type { Auftrag, Ergebnis } from './werkstatt';
import { ebenen as malen } from './zeichnen';
import './tablett.css';

/** Für diese Version der Schnittstelle ist der Skin geschrieben. */
const API = 1;

/** Wie weit die Bilder je Seite über das Fenster reichen, als Anteil des Fensters. */
const UEBERSTAND = 0.02;

/** Ein Rechteck aus ganzen Blöcken, nicht leer. */
function rechteck(area: unknown): area is Rechteck {
  return (
    Array.isArray(area) &&
    area.length === 4 &&
    area.every(Number.isInteger) &&
    area[0] < area[2] &&
    area[1] < area[3]
  );
}

const skin: Skin = (kontext) => {
  const { version, karte, container, maxZoom, area, seaLevel, minY } = kontext;
  if (version !== API) {
    console.warn(`Tablett: geschrieben für die Schnittstelle ${API}, die Karte hat ${version}.`);
    return undefined;
  }
  // Ohne die beiden Felder gibt es nichts einzufassen.
  if (area === undefined || seaLevel === undefined) return undefined;
  if (!Number.isInteger(seaLevel) || !rechteck(area)) {
    console.warn('Tablett: seaLevel oder area in map.json taugen nicht, das Tablett bleibt aus.');
    return undefined;
  }
  if (area[2] - area[0] !== area[3] - area[1]) {
    console.warn('Tablett: area ist kein Quadrat, das Tablett bleibt aus.');
    return undefined;
  }

  container.classList.add('skin-tablett');
  const rahmen = grenzen(area, seaLevel, kontext);
  const ebenen = [
    { name: 'tablett-fern', z: 150 },
    { name: 'tablett-nah', z: 250 },
  ].map(({ name, z }) => {
    karte.createPane(name).style.zIndex = String(z);
    return { name, leinwand: L.DomUtil.create('canvas', 'tablett'), ebene: undefined as L.SVGOverlay | undefined };
  });

  // Ganz zu sehen auf fitZoom, eine Stufe darüber nicht mehr.
  let fit = Number.NaN;
  const blende = (zoom: number, ende: boolean): void => {
    const deckkraft = Math.min(1, Math.max(0, fit + 1 - zoom));
    for (const { ebene } of ebenen) {
      ebene?.setOpacity(deckkraft);
      // Verborgen erst am Ende des Zooms, damit das Ausblenden zu sehen ist.
      const element = ebene?.getElement();
      if (element && (deckkraft > 0 || ende)) element.style.visibility = deckkraft > 0 ? '' : 'hidden';
    }
  };

  // Die Texturen rechnet und malt ein Worker. Bis seine Bilder da sind,
  // stehen die Flächen in ihrer Farbe; dann werden sie einmal ausgetauscht,
  // nicht während die Karte sich bewegt. Siehe docs/tablett.md, „Texturen“.
  let werkstatt: Worker | undefined;
  let nummer = 0;
  let bewegt = false;
  karte.on('movestart', () => (bewegt = true));
  karte.on('moveend', () => (bewegt = false));

  // Gezeichnet wird beim Laden und bei jeder neuen Grösse des Fensters, für
  // fitZoom und um die Mitte des Rahmens wie fitBounds. Weiter als dieses
  // Fenster lässt sich nicht ziehen; die Bilder reichen knapp darüber.
  let gebaut = '';
  const baue = (): void => {
    const groesse = karte.getSize();
    fit = kontext.fitZoom(rahmen, groesse.x, groesse.y);
    karte.setMinZoom(fit);
    if (`${groesse.x} ${groesse.y}` !== gebaut) {
      gebaut = `${groesse.x} ${groesse.y}`;
      const beginn = performance.now();
      // Ein Pixel der feinsten Stufe in Pixeln des Bildschirms auf fitZoom.
      const s = 2 ** (fit - maxZoom);
      const [mx, my] = [((rahmen[0] + rahmen[2]) / 2) * s, ((rahmen[1] + rahmen[3]) / 2) * s];
      const ansicht: Grenzen = [
        (mx - groesse.x / 2) / s,
        (my - groesse.y / 2) / s,
        (mx + groesse.x / 2) / s,
        (my + groesse.y / 2) / s,
      ];
      // Die linke obere Ecke auf ganzen Pixeln: So trifft jedes Pixel der
      // Leinwand eines des Bildschirms.
      const breite = Math.round(groesse.x * (1 + 2 * UEBERSTAND));
      const hoehe = Math.round(groesse.y * (1 + 2 * UEBERSTAND));
      const [links, oben] = [Math.round(mx - breite / 2), Math.round(my - hoehe / 2)];
      const ecken = L.latLngBounds(
        karte.unproject([links / s, oben / s], maxZoom),
        karte.unproject([(links + breite) / s, (oben + hoehe) / s], maxZoom),
      );
      const teile = tablett(area, seaLevel, minY, kontext, ansicht);
      for (const { leinwand } of ebenen) [leinwand.width, leinwand.height] = [breite, hoehe];
      const [fern, nah] = ebenen.map(({ leinwand }) => leinwand.getContext('2d')!);
      malen(fern!, nah!, teile, s, [-links, -oben]);
      for (const eintrag of ebenen) {
        // Leaflets SVGOverlay legt jedes Element als Bild-Ebene, auch eine
        // Leinwand. Ein Bild aus ihr ginge nur über data: oder blob:, und
        // das verbietet die Content-Security-Policy.
        const { leinwand, name } = eintrag;
        eintrag.ebene ??= L.svgOverlay(leinwand as unknown as SVGElement, ecken, { pane: name, interactive: false }).addTo(karte);
        eintrag.ebene.setBounds(ecken);
      }
      karte.options.maxBoundsViscosity = 1;
      karte.setMaxBounds(L.latLngBounds(karte.unproject([ansicht[0], ansicht[1]], maxZoom), karte.unproject([ansicht[2], ansicht[3]], maxZoom)));
      performance.measure('tablett: zeichnen', { start: beginn });

      const auftrag = ++nummer;
      werkstatt ??= new Worker(new URL('./werkstatt.ts', import.meta.url), { type: 'module' });
      werkstatt.onerror = () => console.warn('Tablett: Texturen nicht gerechnet, die Flächen bleiben einfarbig.');
      werkstatt.onmessage = ({ data }: MessageEvent<Ergebnis>) => {
        const tausche = (): void => {
          if (data.nummer === nummer) {
            const neu = performance.now();
            for (const [i, { leinwand }] of ebenen.entries()) {
              const ctx = leinwand.getContext('2d')!;
              ctx.setTransform(1, 0, 0, 1, 0, 0);
              ctx.clearRect(0, 0, breite, hoehe);
              ctx.drawImage(data.bilder[i]!, 0, 0);
            }
            performance.measure('tablett: texturen', {
              start: beginn,
              detail: { worker: data.dauer, tauschen: performance.now() - neu },
            });
          }
          for (const bild of data.bilder) bild.close();
        };
        if (bewegt) karte.once('moveend', tausche);
        else tausche();
      };
      werkstatt.postMessage({
        nummer: auftrag,
        teile,
        s,
        versatz: [-links, -oben],
        leinwand: [breite, hoehe],
        licht: lichtUndBlick(kontext.projektion),
        streckung: kontext.projektion.v / kontext.projektion.u,
      } satisfies Auftrag);
    }
    blende(karte.getZoom(), true);
  };
  karte.whenReady(baue);
  karte.on('resize', baue);
  karte.on('zoomanim', (event: L.ZoomAnimEvent) => blende(event.zoom, false));
  karte.on('zoom', () => blende(karte.getZoom(), false));
  karte.on('zoomend', () => blende(karte.getZoom(), true));

  return { ganzeKarte: rahmen };
};

export default skin;
