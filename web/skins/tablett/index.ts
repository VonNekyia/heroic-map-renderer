/**
 * Der Skin „Tablett“: die Welt in einem Holztablett auf einem Tisch, auf
 * jeder Stufe. Er zeichnet je Ansicht zwei Bilder, fern unter den Kacheln
 * und nah darüber, so gross wie das Fenster mit Überstand, und legt sie als
 * Bild-Ebenen auf die Karte. Die Texturen sind feste Bilder aus bilder/.
 * Während einer Bewegung gleiten die Bilder mit der Karte. Siehe
 * docs/tablett.md.
 */
import type { Grenzen, Rechteck, Skin } from 'heroic-map-renderer/skin-api';
import L from 'leaflet';
import { atlas } from './atlas';
import { gesamtstufe, grenzen, raster, tablett, type Teil } from './tablett';
import { type Bilder, ebenen as malen } from './zeichnen';
import './tablett.css';

/** Für diese Version der Schnittstelle ist der Skin geschrieben. */
const API = 1;

/** Wie weit die Bilder je Seite über das Fenster reichen, als Anteil des Fensters. */
const UEBERSTAND = 0.25;

/**
 * Die Bilder aus bilder/ als Adressen. Vite legt jedes als eigene Datei ab;
 * als `data:` verböte es die Content-Security-Policy.
 */
const ADRESSEN = import.meta.glob<string>('./bilder/*.png', { query: '?url&no-inline', import: 'default', eager: true });

/** Ein Bild aus bilder/, ohne Umrechnung der Farben. */
async function lade(name: string): Promise<ImageBitmap> {
  const adresse = ADRESSEN[`./bilder/${name}.png`];
  if (!adresse) throw new Error(`bilder/${name}.png fehlt`);
  const antwort = await fetch(adresse);
  if (!antwort.ok) throw new Error(`bilder/${name}.png: ${antwort.status}`);
  return createImageBitmap(await antwort.blob(), { colorSpaceConversion: 'none', premultiplyAlpha: 'none' });
}

/**
 * Der Atlas einer Dichte, so gross, wie `atlas` ihn plant. Jedes Bild, das
 * sich wiederholt, kommt dazu für sich: Ein Muster nimmt nur ganze Bilder.
 */
async function ladeAtlas(dichte: number): Promise<Bilder['atlas']> {
  const bild = await lade(`atlas-${dichte}`);
  const { breite, hoehe, bereiche } = atlas(dichte);
  if (bild.width !== breite || bild.height !== hoehe) throw new Error(`bilder/atlas-${dichte}.png passt nicht zum Atlas`);
  const einzeln = new Map(
    await Promise.all(
      [...bereiche]
        .filter(([, bereich]) => bereich.periodisch)
        .map(async ([name, { x, y, breite: b, hoehe: h }]) => [name, await createImageBitmap(bild, x, y, b, h)] as const),
    ),
  );
  return { dichte, bild, bereiche, einzeln };
}

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

  // Jedes Bild wird einmal geladen. Fehlt eines, bleiben die Flächen in
  // ihrer Farbe.
  const atlanten = new Map<number, Promise<Bilder['atlas']>>();
  let kachel: Promise<ImageBitmap> | undefined;
  const bilder = async (dichte: number): Promise<Bilder | undefined> => {
    if (!atlanten.has(dichte)) atlanten.set(dichte, ladeAtlas(dichte));
    kachel ??= lade('marmor');
    try {
      const [holz, marmor] = await Promise.all([atlanten.get(dichte)!, kachel]);
      return { atlas: holz, marmor };
    } catch (fehler) {
      console.warn('Tablett: Bilder nicht geladen, die Flächen bleiben einfarbig.', fehler);
      return undefined;
    }
  };

  // Zwischen zwei Stufen verkleinert Leaflet die Kacheln; dann glättet der
  // Browser sie, statt Pixel auszulassen. Siehe
  // docs/entscheidungen/0067-gesamtansicht-zwischen-zwei-stufen.md.
  const gebrochen = (): void => {
    const zoom = karte.getZoom();
    container.classList.toggle('tablett-gebrochen', !Number.isInteger(zoom) && zoom < maxZoom);
  };

  // Kommen die Bilder, während die Karte sich bewegt, malt der Skin erst
  // danach.
  let bewegt = false;
  karte.on('movestart', () => (bewegt = true));
  karte.on('moveend', () => (bewegt = false));

  /**
   * Was je Fenstergrösse fest ist: die Gesamtansicht, das Raster und die
   * Teile. Sie hängen an der Gesamtansicht, nicht an der Stufe; so bleibt das
   * Tablett beim Zoomen, wie es ist, und wird nur grösser.
   */
  let fest: { sGesamt: number; ansicht: Grenzen; dichte: number; teile: Teil[] } | undefined;
  /** Was die Leinwände zeigen: die Stufe und ihr Ausschnitt in Pixeln dieser Stufe. */
  let gezeichnet: { zoom: number; links: number; oben: number; rechts: number; unten: number } | undefined;

  let nummer = 0;
  /**
   * Zeichnet beide Ebenen für die Ansicht, sobald die Bilder da sind, um ihre
   * Mitte, so gross wie das Fenster mit Überstand.
   */
  const zeichne = async (): Promise<void> => {
    const auftrag = ++nummer;
    const beginn = performance.now();
    const geladen = await bilder(fest!.dichte);
    const male = (): void => {
      // Eine neuere Ansicht ist schon unterwegs.
      if (auftrag !== nummer || !fest) return;
      const start = performance.now();
      const zoom = karte.getZoom();
      // Ein Pixel der feinsten Stufe in Pixeln des Bildschirms.
      const s = 2 ** (zoom - maxZoom);
      const groesse = karte.getSize();
      const mitte = karte.project(karte.getCenter(), zoom);
      // Die linke obere Ecke auf ganzen Pixeln: So trifft jedes Pixel der
      // Leinwand eines des Bildschirms.
      const breite = Math.round(groesse.x * (1 + 2 * UEBERSTAND));
      const hoehe = Math.round(groesse.y * (1 + 2 * UEBERSTAND));
      const [links, oben] = [Math.round(mitte.x - breite / 2), Math.round(mitte.y - hoehe / 2)];
      const ecken = L.latLngBounds(
        karte.unproject([links / s, oben / s], maxZoom),
        karte.unproject([(links + breite) / s, (oben + hoehe) / s], maxZoom),
      );
      for (const { leinwand } of ebenen) [leinwand.width, leinwand.height] = [breite, hoehe];
      const [fern, nah] = ebenen.map(({ leinwand }) => leinwand.getContext('2d')!);
      malen(fern!, nah!, fest.teile, s, [-links, -oben], fest, geladen);
      for (const eintrag of ebenen) {
        // Leaflets SVGOverlay legt jedes Element als Bild-Ebene, auch eine
        // Leinwand. Ein Bild aus ihr ginge nur über data: oder blob:, und
        // das verbietet die Content-Security-Policy.
        const { leinwand, name } = eintrag;
        eintrag.ebene ??= L.svgOverlay(leinwand as unknown as SVGElement, ecken, { pane: name, interactive: false }).addTo(karte);
        eintrag.ebene.setBounds(ecken);
      }
      gezeichnet = { zoom, links, oben, rechts: links + breite, unten: oben + hoehe };
      const fertig = performance.now();
      performance.measure('tablett: zeichnen', { start: beginn, end: fertig, detail: { laden: start - beginn, malen: fertig - start } });
    };
    if (bewegt) karte.once('moveend', male);
    else male();
  };

  // Neu gezeichnet wird nach jedem Zoom und wenn das Fenster über den
  // Überstand hinaus gezogen ist. Bis dahin gleiten die Bilder mit der Karte.
  const pruefe = (): void => {
    if (!fest) return;
    const g = gezeichnet;
    const drin = g && L.bounds([g.links, g.oben], [g.rechts, g.unten]).contains(karte.getPixelBounds());
    if (!g || g.zoom !== karte.getZoom() || !drin) void zeichne();
  };

  // Bei jeder neuen Grösse des Fensters: die Gesamtansicht um die Mitte des
  // Rahmens wie fitBounds. Weiter heraus geht es nicht, und `maxBounds` ist
  // dieses Fenster: Es zeigt den ganzen Tisch, und auf jeder Stufe bleibt
  // die Ansicht darin.
  let fit = Number.NaN;
  let gebaut = '';
  const baue = (): void => {
    const groesse = karte.getSize();
    const [alt, vorher] = [fit, karte.getZoom()];
    fit = gesamtstufe(rahmen, maxZoom, groesse.x, groesse.y);
    // Ein Pixel der feinsten Stufe in Pixeln des Bildschirms in der Gesamtansicht.
    const s = 2 ** (fit - maxZoom);
    const mitte: [number, number] = [((rahmen[0] + rahmen[2]) / 2) * s, ((rahmen[1] + rahmen[3]) / 2) * s];
    const ansicht: Grenzen = [
      (mitte[0] - groesse.x / 2) / s,
      (mitte[1] - groesse.y / 2) / s,
      (mitte[0] + groesse.x / 2) / s,
      (mitte[1] + groesse.y / 2) / s,
    ];
    karte.options.maxBoundsViscosity = 1;
    karte.setMaxBounds(L.latLngBounds(karte.unproject([ansicht[0], ansicht[1]], maxZoom), karte.unproject([ansicht[2], ansicht[3]], maxZoom)));
    // Leaflet rundet jede gewünschte Stufe, bevor es sie auf die Untergrenze
    // hebt: Eine gebrochene erreicht es nur von darunter. Wer die ganze
    // Karte sah, sieht sie auch im neuen Fenster ganz.
    karte.options.minZoom = fit;
    karte.fire('zoomlevelschange');
    if (vorher < fit || vorher === alt) karte.setZoom(Math.floor(fit), { animate: false });
    if (`${groesse.x} ${groesse.y}` !== gebaut) {
      gebaut = `${groesse.x} ${groesse.y}`;
      const { dichte, w } = raster(area, kontext.projektion, s);
      fest = { sGesamt: s, ansicht, dichte, teile: tablett(area, seaLevel, minY, kontext, ansicht, w) };
      gezeichnet = undefined;
    }
    pruefe();
    gebrochen();
  };
  karte.whenReady(baue);
  karte.on('resize', baue);
  karte.on('moveend', pruefe);
  karte.on('zoomend', gebrochen);

  return { ganzeKarte: rahmen };
};

export default skin;
