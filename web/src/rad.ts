import L from 'leaflet';

/** So viele Pixel nach `getWheelDelta` sind eine Stufe. */
const PIXEL_JE_STUFE = 50;
/** So viele ms ohne Ereignis beenden eine Geste. */
const GESTE_ZU = 150;
/** Mehr Stufen auf einmal animiert Leaflet nicht (`zoomAnimationThreshold`). */
const STUFEN_JE_ANIMATION = 4;
/** So viele ms gilt eine Animation höchstens als laufend, auch wenn ihr `zoomend` ausbleibt. */
const ANIMATION_HOECHSTENS = 1000;

/**
 * Pixel eines Ereignisses, hinein positiv. Chromium unter Windows meldet eine Raste ohne die
 * Skalierung, `getWheelDelta` teilt dort trotzdem durch sie. Siehe docs/frontend.md, „Zoom mit dem Rad“.
 */
const pixel = (e: WheelEvent) => (L.Browser.win && L.Browser.chrome && e.deltaMode === 0 ? -e.deltaY / 2 : L.DomEvent.getWheelDelta(e));

/**
 * Zoomt mit dem Rad in ganzen Stufen um den Zeiger; die Karte schaltet dafür
 * `scrollWheelZoom` aus. Stufen während einer Animation warten auf deren Ende.
 * Siehe docs/frontend.md, „Zoom mit dem Rad“.
 */
export function radZoom(map: L.Map): void {
  /** Pixel der laufenden Geste, die noch keine Stufe ergeben haben. */
  let gesammelt = 0;
  let hatStufe = false;
  /** Stufen, die noch auf die Karte warten, hinein positiv. */
  let offen = 0;
  let laeuft = false;
  let punkt = L.point(0, 0);
  let ende: ReturnType<typeof setTimeout> | undefined;
  let frist: ReturnType<typeof setTimeout> | undefined;

  /** Leaflets Regel aus `_limitZoom`, mit öffentlichen Mitteln. */
  const grenze = (z: number) => Math.max(map.getMinZoom(), Math.min(map.getMaxZoom(), Math.round(z)));
  const weiter = () => {
    if (laeuft || offen === 0) return;
    const schritt = Math.max(-STUFEN_JE_ANIMATION, Math.min(STUFEN_JE_ANIMATION, offen));
    offen -= schritt;
    const zoom = map.getZoom();
    // Zweimal, weil `setView` das Ziel noch einmal rundet: Eine gebrochene Grenze wie −6,4 ergäbe dort −6, und die Mitte stimmte nicht.
    const ziel = grenze(grenze(zoom + schritt));
    // An der Grenze verfällt der Rest, sonst schluckte er die Gegenrichtung.
    if ((ziel - zoom) * schritt <= 0) {
      offen = 0;
      return;
    }
    belegt();
    map.setZoomAround(punkt, ziel);
  };
  const belegt = () => {
    laeuft = true;
    clearTimeout(frist);
    frist = setTimeout(frei, ANIMATION_HOECHSTENS);
  };
  const frei = () => {
    clearTimeout(frist);
    laeuft = false;
    weiter();
  };
  // Auch fremdes Zoomen zählt, etwa Knöpfe oder Pinch: Während seiner Animation täte setZoomAround nichts.
  map.on('zoomstart', belegt);
  map.on('zoomend', frei);
  L.DomEvent.on(map.getContainer(), 'wheel', (e) => {
    L.DomEvent.stop(e);
    punkt = map.mouseEventToContainerPoint(e as MouseEvent);
    gesammelt += pixel(e as WheelEvent);
    const stufen = Math.trunc(gesammelt / PIXEL_JE_STUFE);
    if (stufen !== 0) {
      offen += stufen;
      gesammelt = 0;
      hatStufe = true;
      weiter();
    }
    clearTimeout(ende);
    ende = setTimeout(() => {
      // Eine Geste ohne ganze Stufe zählt als eine, damit eine klein gemeldete Raste etwas tut.
      if (gesammelt !== 0 && !hatStufe) {
        offen += Math.sign(gesammelt);
        weiter();
      }
      gesammelt = 0;
      hatStufe = false;
    }, GESTE_ZU);
  });
}
