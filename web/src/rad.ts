import L from 'leaflet';

/** So viele Pixel nach `getWheelDelta` sind eine Stufe. */
const PIXEL_JE_STUFE = 50;
/** So viele ms ohne Ereignis beenden eine Geste; so lange dreht das Rad noch. */
const GESTE_ZU = 150;
/** So viele ms gilt eine Animation höchstens als laufend, auch wenn ihr `zoomend` ausbleibt. */
const ANIMATION_HOECHSTENS = 1000;

/**
 * Pixel eines Ereignisses, hinein positiv. Chromium unter Windows meldet eine Raste ohne die
 * Skalierung, `getWheelDelta` teilt dort trotzdem durch sie. Siehe docs/frontend.md, „Zoom mit dem Rad“.
 */
const pixel = (e: WheelEvent) => (L.Browser.win && L.Browser.chrome && e.deltaMode === 0 ? -e.deltaY / 2 : L.DomEvent.getWheelDelta(e));

/**
 * Zoomt mit dem Rad in ganzen Stufen um den Zeiger; die Karte schaltet dafür
 * `scrollWheelZoom` aus. Steht das Rad still, beginnt nichts mehr.
 * Siehe docs/frontend.md, „Zoom mit dem Rad“.
 */
export function radZoom(map: L.Map): void {
  /** Pixel der laufenden Geste, die noch keine Stufe ergeben haben. */
  let gesammelt = 0;
  let hatStufe = false;
  /** Richtung der Stufe, die während einer Animation kam, hinein positiv; 0, wenn keine. */
  let wartet = 0;
  /** Wann das letzte Ereignis kam. */
  let zuletzt = 0;
  let laeuft = false;
  let punkt = L.point(0, 0);
  let ende: ReturnType<typeof setTimeout> | undefined;
  let frist: ReturnType<typeof setTimeout> | undefined;

  /** Leaflets Regel aus `_limitZoom`, mit öffentlichen Mitteln. */
  const grenze = (z: number) => Math.max(map.getMinZoom(), Math.min(map.getMaxZoom(), Math.round(z)));
  const zoome = (richtung: number) => {
    const zoom = map.getZoom();
    // Zweimal, weil `setView` das Ziel noch einmal rundet: Eine gebrochene Grenze wie −6,4 ergäbe dort −6, und die Mitte stimmte nicht.
    const ziel = grenze(grenze(zoom + richtung));
    if ((ziel - zoom) * richtung <= 0) return;
    belegt();
    map.setZoomAround(punkt, ziel);
  };
  const stufe = (richtung: number) => {
    if (laeuft) wartet = richtung;
    else zoome(richtung);
  };
  const belegt = () => {
    laeuft = true;
    clearTimeout(frist);
    frist = setTimeout(frei, ANIMATION_HOECHSTENS);
  };
  const frei = () => {
    clearTimeout(frist);
    laeuft = false;
    const richtung = wartet;
    wartet = 0;
    // Nur, solange das Rad noch dreht; sonst zoomte die Karte nach. Erst ein Bild später:
    // Sonst rechnet der Browser den zurückgesetzten Massstab des Renderers nie, und sein
    // Übergang beginnt am Ziel. Siehe docs/entscheidungen/0137-pfade-erst-nach-dem-zoom.md.
    if (richtung === 0 || performance.now() - zuletzt > GESTE_ZU) return;
    // Bis dahin belegt: Ein Ereignis in diesem Bild merkt nur die Richtung, sonst zoomten zwei zur selben Stufe.
    laeuft = true;
    requestAnimationFrame(() => {
      laeuft = false;
      zoome(richtung);
    });
  };
  // Auch fremdes Zoomen zählt, etwa Knöpfe oder Pinch: Während seiner Animation täte setZoomAround nichts.
  map.on('zoomstart', belegt);
  map.on('zoomend', frei);
  L.DomEvent.on(map.getContainer(), 'wheel', (e) => {
    L.DomEvent.stop(e);
    zuletzt = performance.now();
    punkt = map.mouseEventToContainerPoint(e as MouseEvent);
    gesammelt += pixel(e as WheelEvent);
    const stufen = Math.trunc(gesammelt / PIXEL_JE_STUFE);
    if (stufen !== 0) {
      gesammelt = 0;
      hatStufe = true;
      stufe(Math.sign(stufen));
    }
    clearTimeout(ende);
    ende = setTimeout(() => {
      // Eine Geste ohne ganze Stufe zählt als eine, damit eine klein gemeldete Raste etwas tut.
      if (gesammelt !== 0 && !hatStufe) stufe(Math.sign(gesammelt));
      gesammelt = 0;
      hatStufe = false;
    }, GESTE_ZU);
  });
}
