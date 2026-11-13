import { expect, test, type Page } from './test';
import { DEMO, HAFEN, staedte, welt } from './ebenen-welt';

/** Was Leaflet aus einem Pixel am Rad macht: Chrome unter Windows zählt doppelt. */
const FAKTOR = process.platform === 'win32' ? 2 : 1;
/** Eine Raste, wie die Tests sie erzeugen: 60 Pixel nach `getWheelDelta`, bei Leaflets `wheelPxPerZoomLevel` 60 eine Stufe. Siehe docs/entscheidungen/0140-rad-von-leaflet.md. */
const RASTE = 60 * FAKTOR;

const zoomDerAdresse = (page: Page) => Number(new URL(page.url()).searchParams.get('zoom'));
/** Der Demobaum reicht von −2 bis 2; die Adresse nennt die Stufe nur mit Höhen, darum die Welt. */
const aufStufe = async (page: Page, zoom: number) => {
  await welt(page, staedte([]));
  await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
  await page.waitForSelector('.kacheln img.leaflet-tile', { state: 'attached' });
  await page.mouse.move(640, 360);
};
/** Wie viele ms nach einem Ereignis die Animation beginnt, gemessen in der Seite; −1, wenn in 1 s keine beginnt. */
const reaktionRoh = (page: Page, delta: number, deltaMode = 0) =>
  page.evaluate(
    ([deltaY, modus]) =>
      new Promise<number>((fertig) => {
        const start = performance.now();
        const beobachter = new MutationObserver(() => {
          if (document.querySelector('.leaflet-zoom-anim')) fertig(performance.now() - start);
        });
        beobachter.observe(document.body, { attributes: true, subtree: true, attributeFilter: ['class'] });
        setTimeout(() => fertig(-1), 1000);
        document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY: deltaY!, deltaMode: modus!, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
      }),
    [delta, deltaMode],
  );
const reaktion = (page: Page, pixel: number) => reaktionRoh(page, -pixel * FAKTOR);
/** Wartet, bis Animationen und Rest der Geste durch sind, und nennt die Stufe. */
const zuletzt = async (page: Page) => {
  await page.waitForTimeout(1000);
  return zoomDerAdresse(page);
};

  test('eine Raste ist eine Stufe, nach Leaflets Entprellen von 40 ms', async ({ page }) => {
  await aufStufe(page, -2);
  const ms = await reaktion(page, 60);
  expect(ms).toBeGreaterThanOrEqual(30);
  expect(ms).toBeLessThan(150);
  expect(await zuletzt(page)).toBe(-1);
});

test('an der Grenze tut eine Raste nichts, und die Gegenrichtung wirkt gleich', async ({ page }) => {
  await aufStufe(page, 2);
  expect(await reaktion(page, 60)).toBe(-1);
  const ms = await reaktion(page, -60);
  expect(ms).toBeGreaterThanOrEqual(0);
  expect(ms).toBeLessThan(100);
  expect(await zuletzt(page)).toBe(1);
});

test('das Rad zoomt um den Zeiger: Die Nadel unter ihm bleibt stehen, auch über mehrere Stufen', async ({ page }) => {
  await welt(page, staedte([{ ...HAFEN, at: [200.5, 100.5] }]));
  await page.goto(`${DEMO}&at=0,0,0&zoom=-2`);
  const nadel = page.locator('.nadel-icon[title="Hafenstadt"]');
  await page.mouse.move(100, 100);
  const vorher = (await nadel.boundingBox())!;
  const [x, y] = [vorher.x + vorher.width / 2, vorher.y + vorher.height];
  await page.mouse.move(x, y - 1);
  for (let i = 0; i < 3; i++) {
    await page.mouse.wheel(0, -RASTE);
    await page.waitForTimeout(400);
  }
  expect(await zuletzt(page)).toBe(1);
  const nachher = (await nadel.boundingBox())!;
  // Der Zeiger steht 1 px über dem Fuss; drei Stufen machen daraus 7 px.
  expect(Math.abs(nachher.x - vorher.x)).toBeLessThanOrEqual(8);
  expect(Math.abs(nachher.y - vorher.y)).toBeLessThanOrEqual(8);
});
