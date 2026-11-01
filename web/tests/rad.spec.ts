import { expect, test, type Page } from './test';
import { DEMO, HAFEN, staedte, welt } from './ebenen-welt';

/** Was Leaflet aus einem Pixel am Rad macht: Chrome unter Windows zählt doppelt. */
const FAKTOR = process.platform === 'win32' ? 2 : 1;
/** Eine Raste wie am echten Rad, 60 Pixel nach `getWheelDelta`, genau eine Stufe. */
const RASTE = 60 * FAKTOR;

const zoomDerAdresse = (page: Page) => Number(new URL(page.url()).searchParams.get('zoom'));
/** Der Demobaum reicht von −2 bis 2; die Adresse nennt die Stufe nur mit Höhen, darum die Welt. */
const amUnterenEnde = async (page: Page) => {
  await welt(page, staedte([]));
  await page.goto(`${DEMO}&at=35,0,-15&zoom=-2`);
  await page.waitForSelector('.kacheln img.leaflet-tile-loaded');
  await page.mouse.move(640, 360);
};
/** Viele Ereignisse in einem Zug, damit keine Pause unter Last die Geste teilt; `pixel` nach `getWheelDelta`, hinein positiv. */
const strom = (page: Page, zahl: number, pixel: number) =>
  page.evaluate(
    ([n, d]) => {
      const karte = document.querySelector('#map')!;
      for (let i = 0; i < n!; i++) karte.dispatchEvent(new WheelEvent('wheel', { deltaY: d!, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
    },
    [zahl, -pixel * FAKTOR],
  );
/** Wie viele ms nach einem Ereignis die Animation beginnt, gemessen in der Seite; −1, wenn in 1 s keine beginnt. */
const reaktion = (page: Page, pixel: number) =>
  page.evaluate(
    (deltaY) =>
      new Promise<number>((fertig) => {
        const start = performance.now();
        const beobachter = new MutationObserver(() => {
          if (document.querySelector('.leaflet-zoom-anim')) fertig(performance.now() - start);
        });
        beobachter.observe(document.body, { attributes: true, subtree: true, attributeFilter: ['class'] });
        setTimeout(() => fertig(-1), 1000);
        document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
      }),
    -pixel * FAKTOR,
  );
/** Wartet, bis Animationen und Rest der Geste durch sind, und nennt die Stufe. */
const zuletzt = async (page: Page) => {
  await page.waitForTimeout(1000);
  return zoomDerAdresse(page);
};

for (const takt of [50, 100, 150, 250]) {
  test(`drei Rasten im Takt von ${takt} ms sind drei Stufen, keine verloren, keine zu viel`, async ({ page }) => {
    await amUnterenEnde(page);
    for (let i = 0; i < 3; i++) {
      await page.mouse.wheel(0, -RASTE);
      await page.waitForTimeout(takt);
    }
    expect(await zuletzt(page)).toBe(1);
    for (let i = 0; i < 3; i++) {
      await page.mouse.wheel(0, RASTE);
      await page.waitForTimeout(takt);
    }
    expect(await zuletzt(page)).toBe(-2);
  });
}

for (const [name, zahl, pixel, stufen] of [
  ['49 Pixel in kleinen Schritten sind nur eine Stufe, wenn die Geste endet', 49, 1, 1],
  ['50 Pixel in kleinen Schritten sind eine Stufe', 25, 2, 1],
  ['98 Pixel in kleinen Schritten sind eine Stufe', 49, 2, 1],
  ['100 Pixel in kleinen Schritten sind zwei Stufen', 50, 2, 2],
  ['der Rest einer Stufe verfällt: zwei Schritte von 75 Pixeln sind zwei Stufen', 2, 75, 2],
] as const) {
  test(name, async ({ page }) => {
    await amUnterenEnde(page);
    await strom(page, zahl, pixel);
    expect(await zuletzt(page)).toBe(-2 + stufen);
  });
}

test('eine Raste, die eine Stufe erreicht, zoomt sofort, unter 100 ms', async ({ page }) => {
  await amUnterenEnde(page);
  const ms = await reaktion(page, 60);
  expect(ms).toBeGreaterThanOrEqual(0);
  expect(ms).toBeLessThan(100);
  expect(await zuletzt(page)).toBe(-1);
});

test('eine Geste unter einer Stufe wirkt erst nach 150 ms Ruhe, dann als eine Stufe', async ({ page }) => {
  await amUnterenEnde(page);
  expect(await reaktion(page, 8)).toBeGreaterThanOrEqual(149);
  expect(await zuletzt(page)).toBe(-1);
});

test('an der Grenze verfällt der Rest: Die nächste Raste in die Gegenrichtung wirkt gleich', async ({ page }) => {
  await amUnterenEnde(page);
  await strom(page, 1, 500);
  expect(await zuletzt(page)).toBe(2);
  await page.mouse.wheel(0, RASTE);
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
  for (let i = 0; i < 3; i++) await page.mouse.wheel(0, -RASTE);
  expect(await zuletzt(page)).toBe(1);
  const nachher = (await nadel.boundingBox())!;
  // Der Zeiger steht 1 px über dem Fuss; drei Stufen machen daraus 7 px.
  expect(Math.abs(nachher.x - vorher.x)).toBeLessThanOrEqual(8);
  expect(Math.abs(nachher.y - vorher.y)).toBeLessThanOrEqual(8);
});

test('bleibt zoomend aus, ist das Rad nach 1000 ms wieder frei, und was wartete, folgt', async ({ page }) => {
  // Leaflet startet die Animation im nächsten Bild; verschluckt, kommen weder zoomstart noch zoomend.
  await page.addInitScript(() => {
    const w = window as unknown as { schlucke?: boolean };
    const echt = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = (f) => (w.schlucke ? 0 : echt(f));
  });
  await amUnterenEnde(page);
  const ms = await page.evaluate(
    (deltaY) =>
      new Promise<number>((fertig) => {
        const w = window as unknown as { schlucke?: boolean };
        const raste = () => document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
        const start = performance.now();
        new MutationObserver(() => {
          if (document.querySelector('.leaflet-zoom-anim')) fertig(performance.now() - start);
        }).observe(document.body, { attributes: true, subtree: true, attributeFilter: ['class'] });
        setTimeout(() => fertig(-1), 3000);
        w.schlucke = true;
        raste();
        w.schlucke = false;
        raste();
      }),
    -RASTE,
  );
  expect(ms).toBeGreaterThanOrEqual(990);
  // Die verschluckte Stufe ist weg, die wartende kommt an.
  expect(await zuletzt(page)).toBe(-1);
});

test('während der Animation der Knöpfe geht eine Raste nicht verloren', async ({ page }) => {
  await amUnterenEnde(page);
  await page.locator('.leaflet-control-zoom-in').click();
  await page.mouse.move(640, 360);
  await page.mouse.wheel(0, -RASTE);
  expect(await zuletzt(page)).toBe(0);
});
