/**
 * Der Skin „Tablett“: die Welt in einem Holztablett auf einem Tisch, auf
 * jeder Stufe. Er zeichnet je Ansicht zwei Bilder, fern unter den Kacheln
 * und nah darüber, so gross wie das Fenster mit Überstand, und legt sie als
 * Bild-Ebenen auf die Karte. Rahmen, Tisch, Lilien und Gegenstände sind
 * Bilder aus der Vorlage in bilder/. Während einer Bewegung gleiten die
 * Bilder mit der Karte. Siehe docs/tablett.md.
 */
import type { Grenzen, Rechteck, Skin } from 'heroic-map-renderer/skin-api';
import L from 'leaflet';
import { PERGAMENT } from './bilder';
import { type Figur, gesamtmitte, gesamtstufe, grenzen, tablett, type Teil, vorlageImBild } from './tablett';
import { type Bilder, ebenen as malen } from './zeichnen';
import './tablett.css';

/** Für diese Version der Schnittstelle ist der Skin geschrieben. */
const API = 2;

/** Wie weit die Bilder je Seite über das Fenster reichen, als Anteil des Fensters. */
const UEBERSTAND = 0.25;

/**
 * Die Ecken der UI und wohin jede ausweicht, deckte sie in der
 * Gesamtansicht einen Gegenstand, eine Lilie oder das Pergament: entlang
 * ihres Rands, zur Mitte hin. Siehe docs/tablett.md, „UI“.
 */
const AUSWEICHEN = [
  { ecke: '.leaflet-top.leaflet-left', x: 0, y: 1 },
  { ecke: '.leaflet-top.leaflet-right', x: -1, y: 0 },
  { ecke: '.leaflet-bottom.leaflet-left', x: 1, y: 0 },
  { ecke: '.leaflet-bottom.leaflet-right', x: -1, y: 0 },
] as const;

/** So viel Platz bleibt zwischen der UI und einem Gegenstand, in Pixeln. */
const ABSTAND = 8;

/**
 * Die Bilder aus bilder/ als Adressen. Vite legt jedes als eigene Datei ab;
 * als `data:` verböte es die Content-Security-Policy.
 */
const ADRESSEN = import.meta.glob<string>('./bilder/*.webp', { query: '?url&no-inline', import: 'default', eager: true });

/** So lange wartet der Skin nach den Kacheln höchstens auf die Meldung, dass eine gemalt ist, in ms. */
const FRIST = 1000;

/**
 * Wartet, bis die Ebene der Kacheln zum ersten Mal fertig ist und der
 * Browser eine Kachel als grösstes Element gemalt meldet: Erst dann zählt
 * Lighthouse keine Anfrage mehr zum LCP. Ohne diese Meldung im Browser zwei
 * Bilder nach `load`; wird keine Kachel das grösste Element, nach FRIST.
 * Siehe docs/entscheidungen/0073-bilder-nach-den-kacheln.md.
 */
function nachDenKacheln(ebene: L.TileLayer): Promise<void> {
  return new Promise((fertig) => {
    ebene.once('load', () => {
      if (!PerformanceObserver.supportedEntryTypes.includes('largest-contentful-paint')) {
        requestAnimationFrame(() => requestAnimationFrame(() => fertig()));
        return;
      }
      const beobachter = new PerformanceObserver((liste) => {
        if (liste.getEntries().some((e) => (e as LargestContentfulPaint).element?.classList.contains('leaflet-tile'))) los();
      });
      const frist = setTimeout(() => los(), FRIST);
      const los = (): void => {
        beobachter.disconnect();
        clearTimeout(frist);
        fertig();
      };
      beobachter.observe({ type: 'largest-contentful-paint', buffered: true });
    });
  });
}

/** Ein Bild aus bilder/, hinter allem anderen. */
async function lade(adresse: string): Promise<ImageBitmap> {
  const antwort = await fetch(adresse, { priority: 'low' });
  if (!antwort.ok) throw new Error(`${adresse}: ${antwort.status}`);
  return createImageBitmap(await antwort.blob());
}

/** Alle Bilder, nach Namen. Was nicht lädt, fehlt, und die Konsole sagt es. */
async function ladeAlle(): Promise<Bilder> {
  const eintraege = Object.entries(ADRESSEN).map(([pfad, adresse]) => [pfad.slice('./bilder/'.length, -'.webp'.length), adresse] as const);
  const geladen = await Promise.allSettled(eintraege.map(([, adresse]) => lade(adresse)));
  const bilder = new Map<string, ImageBitmap>();
  geladen.forEach((ergebnis, i) => {
    const name = eintraege[i]![0];
    if (ergebnis.status === 'fulfilled') bilder.set(name, ergebnis.value);
    else console.warn(`Tablett: bilder/${name}.webp nicht geladen, die Fläche bleibt einfarbig.`, ergebnis.reason);
  });
  return bilder;
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

  // Die Karte ist der Inhalt, das Tablett Schmuck: Seine Bilder laden
  // einmal, nach den Kacheln. Die Grundkarte legt deren Ebene nach dem Skin
  // an.
  const kacheln = new Promise<void>((fertig) => {
    karte.on('layeradd', ({ layer }: L.LayerEvent) => {
      if (layer instanceof L.TileLayer) void nachDenKacheln(layer).then(fertig);
    });
  });
  let bilder: Promise<Bilder> | undefined;

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

  /** Die Teile; sie hängen an keiner Stufe, das Tablett wird beim Zoomen nur grösser. */
  const teile: Teil[] = tablett(area, seaLevel, minY, kontext, kontext.texte);
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
    bilder ??= kacheln.then(ladeAlle);
    const geladen = await bilder;
    const male = (): void => {
      // Eine neuere Ansicht ist schon unterwegs.
      if (auftrag !== nummer) return;
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
      malen(fern!, nah!, teile, s, [-links, -oben], geladen);
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
    const g = gezeichnet;
    const drin = g && L.bounds([g.links, g.oben], [g.rechts, g.unten]).contains(karte.getPixelBounds());
    if (!g || g.zoom !== karte.getZoom() || !drin) void zeichne();
  };

  // Gegenstände, Lilien und das Pergament in der Gesamtansicht, in Pixeln des Fensters.
  let dinge: Grenzen[] = [];
  /**
   * Schiebt jede Ecke der UI so weit entlang ihres Rands, dass sie in der
   * Gesamtansicht nichts davon deckt; passt sie dann nicht mehr ins Fenster,
   * bleibt sie, wo sie ist. Es zählt, was man sieht, ohne den Abstand der
   * Ecke zum Rand des Fensters.
   */
  const weiche = (): void => {
    const da = container.getBoundingClientRect();
    for (const { ecke, x, y } of AUSWEICHEN) {
      const element = container.querySelector<HTMLElement>(ecke);
      if (!element) continue;
      element.style.transform = '';
      const sichtbar = [...element.children].map((kind) => kind.getBoundingClientRect()).filter((r) => r.width > 0 && r.height > 0);
      if (sichtbar.length === 0) continue;
      const [links, oben, rechts, unten] = [
        Math.min(...sichtbar.map((r) => r.left)) - da.left,
        Math.min(...sichtbar.map((r) => r.top)) - da.top,
        Math.max(...sichtbar.map((r) => r.right)) - da.left,
        Math.max(...sichtbar.map((r) => r.bottom)) - da.top,
      ];
      const trifft = (d: number) =>
        dinge.find(([l, o, re, u]) => l - ABSTAND < rechts + d * x && re + ABSTAND > links + d * x && o - ABSTAND < unten + d * y && u + ABSTAND > oben + d * y);
      let d = 0;
      for (let i = 0, ding = trifft(0); ding && i < dinge.length; i++, ding = trifft(d)) {
        d = x > 0 ? ding[2] + ABSTAND - links : x < 0 ? rechts - ding[0] + ABSTAND : ding[3] + ABSTAND - oben;
      }
      const passt = links + d * x >= 0 && rechts + d * x <= da.width && unten + d * y <= da.height;
      if (d > 0 && passt) element.style.transform = `translate(${d * x}px, ${d * y}px)`;
    }
  };
  // Die Ecken wachsen, wenn ihre UI kommt oder mehr zeigt.
  const beobachter = new ResizeObserver(weiche);
  for (const { ecke } of AUSWEICHEN) {
    const element = container.querySelector(ecke);
    if (element) beobachter.observe(element);
  }

  // Bei jeder neuen Grösse des Fensters: die Gesamtansicht wie in der
  // Vorlage, ihre Mitte unter der Mitte der Karte. Weiter heraus geht es
  // nicht, und `maxBounds` ist dieses Fenster: Es zeigt den ganzen Tisch,
  // und auf jeder Stufe bleibt die Ansicht darin.
  let fit = Number.NaN;
  const baue = (): void => {
    const groesse = karte.getSize();
    const [alt, vorher] = [fit, karte.getZoom()];
    fit = gesamtstufe(rahmen, maxZoom, groesse.x, groesse.y);
    // Ein Pixel des Bildschirms in Pixeln der feinsten Stufe in der Gesamtansicht.
    const f = 2 ** (maxZoom - fit);
    const [mx, my] = gesamtmitte(area, seaLevel, kontext, groesse.x * f, groesse.y * f);
    const ansicht: Grenzen = [mx - (groesse.x * f) / 2, my - (groesse.y * f) / 2, mx + (groesse.x * f) / 2, my + (groesse.y * f) / 2];
    const fenster = L.latLngBounds(karte.unproject([ansicht[0], ansicht[1]], maxZoom), karte.unproject([ansicht[2], ansicht[3]], maxZoom));
    karte.options.maxBoundsViscosity = 1;
    karte.options.minZoom = fit;
    karte.fire('zoomlevelschange');
    // Wer die ganze Karte sah, sieht sie auch im neuen Fenster ganz, und zwar
    // sofort: Schöbe Leaflet die Ansicht animiert hinein, endete das mitten
    // in einem Zug. Auf der kleinsten Stufe gibt es nur diese Mitte. Ohne
    // Animation rundet Leaflet jede Stufe auf eine ganze; mit zoomSnap 0
    // bleibt sie gebrochen. Die neuen Grenzen gelten schon dabei.
    karte.options.maxBounds = fenster;
    if (vorher <= fit || vorher === alt) {
      const snap = karte.options.zoomSnap;
      karte.options.zoomSnap = 0;
      karte.setView(karte.unproject([mx, my], maxZoom), fit, { animate: false });
      karte.options.zoomSnap = snap;
    }
    karte.setMaxBounds(fenster);
    gezeichnet = undefined;
    pruefe();
    gebrochen();
    const s = 2 ** (fit - maxZoom);
    const imFenster = ([px, py]: [number, number]): [number, number] => [(px - mx) * s + groesse.x / 2, (py - my) * s + groesse.y / 2];
    dinge = teile
      .filter((t): t is Figur => t.form === 'figur')
      .map(({ fuss, anker, mass, groesse: g }) => {
        const ecke: [number, number] = [fuss[0] - mass * anker[0], fuss[1] - mass * anker[1]];
        return [...imFenster(ecke), ...imFenster([ecke[0] + mass * g[0], ecke[1] + mass * g[1]])] as Grenzen;
      });
    // Dazu das Pergament, flach im Tisch.
    const [l, o, r, u] = PERGAMENT;
    const pergament = ([[l, o], [r, o], [l, u], [r, u]] as const).map((p) => imFenster(vorlageImBild(area, seaLevel, kontext, [...p])));
    const xs = pergament.map((p) => p[0]);
    const ys = pergament.map((p) => p[1]);
    dinge.push([Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]);
    weiche();
  };
  karte.whenReady(baue);
  karte.on('resize', baue);
  karte.on('moveend', pruefe);
  karte.on('zoomend', gebrochen);

  return { ganzeKarte: rahmen };
};

export default skin;
