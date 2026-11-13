import { expect, test, type Page } from './test';
import { DEMO, HAFEN, staedte, welt } from './ebenen-welt';

/** Was Leaflet aus einem Pixel am Rad macht: Chrome unter Windows zählt doppelt. */
const FAKTOR = process.platform === 'win32' ? 2 : 1;
/** Eine Raste, wie die Tests sie erzeugen: 60 Pixel nach `getWheelDelta`, genau eine Stufe. */
const RASTE = 60 * FAKTOR;

const zoomDerAdresse = (page: Page) => Number(new URL(page.url()).searchParams.get('zoom'));
/** Der Demobaum reicht von −2 bis 2; die Adresse nennt die Stufe nur mit Höhen, darum die Welt. */
const aufStufe = async (page: Page, zoom: number) => {
  await welt(page, staedte([]));
  await page.goto(`${DEMO}&at=35,0,-15&zoom=${zoom}`);
  await page.waitForSelector('.kacheln img.leaflet-tile', { state: 'attached' });
  await page.mouse.move(640, 360);
};
const amUnterenEnde = (page: Page) => aufStufe(page, -2);
/** Viele Ereignisse in einem Zug, damit keine Pause unter Last die Geste teilt; `pixel` nach `getWheelDelta`, hinein positiv. */
const strom = (page: Page, zahl: number, pixel: number) =>
  page.evaluate(
    ([n, d]) => {
      const karte = document.querySelector('#map')!;
      for (let i = 0; i < n!; i++) karte.dispatchEvent(new WheelEvent('wheel', { deltaY: d!, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
    },
    [zahl, -pixel * FAKTOR],
  );
/** Ereignisse im Takt, gezählt in der Seite; `pixel` nach `getWheelDelta`, hinein positiv. */
const imTakt = (page: Page, pixel: number[], ms: number) =>
  page.evaluate(
    async ([folge, abstand]) => {
      for (const d of folge as number[]) {
        document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY: d, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
        await new Promise((r) => setTimeout(r, abstand as number));
      }
    },
    [pixel.map((p) => -p * FAKTOR), ms],
  );
/**
 * Rasten in einem Zug bei angehaltener Uhr, die danach `her` ms weiter steht. So hängt
 * die Grenze an keiner echten Zeit. `richtungen`: hinein 1, hinaus −1.
 */
const zugBeiUhr = (page: Page, richtungen: number[], her: number) =>
  page.evaluate(
    ([folge, h]) => {
      const w = window as unknown as { uhr: number };
      w.uhr = 1000;
      performance.now = () => w.uhr;
      for (const d of folge as number[]) {
        document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY: d, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
      }
      w.uhr = 1000 + (h as number);
    },
    [richtungen.map((r) => -r * RASTE), her],
  );
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
/** Leaflet liest die Plattform beim Laden; so laufen beide Zweige auf jedem Rechner der Tests. */
const plattform = (page: Page, name: string) => page.addInitScript((n) => Object.defineProperty(navigator, 'platform', { get: () => n }), name);
/** Wartet, bis Animationen und Rest der Geste durch sind, und nennt die Stufe. */
const zuletzt = async (page: Page) => {
  await page.waitForTimeout(1000);
  return zoomDerAdresse(page);
};

for (const takt of [300, 400]) {
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

test('sechs Rasten in einem Zug sind eine Stufe: Steht das Rad, zoomt die Karte nicht nach', async ({ page }) => {
  await amUnterenEnde(page);
  await strom(page, 6, 60);
  expect(await zuletzt(page)).toBe(-1);
});

for (const [ms, stufen] of [[150, 2], [151, 1]] as const) {
  test(`eine Raste während der Animation, das letzte Ereignis ${ms} ms her beim Ende: ${stufen} Stufen`, async ({ page }) => {
    await amUnterenEnde(page);
    await zugBeiUhr(page, [1, 1], ms);
    expect(await zuletzt(page)).toBe(-2 + stufen);
  });
}

test('sechs Rasten im Takt von 100 ms: Solange das Rad dreht, folgen weitere Stufen', async ({ page }) => {
  await amUnterenEnde(page);
  await imTakt(page, [60, 60, 60, 60, 60, 60], 100);
  expect(await zuletzt(page)).toBeGreaterThanOrEqual(0);
});

test('während der Animation zählt die letzte Richtung: hinein, hinein, hinaus endet, wo es begann', async ({ page }) => {
  await amUnterenEnde(page);
  await zugBeiUhr(page, [1, 1, -1], 0);
  expect(await zuletzt(page)).toBe(-2);
});

test('ein Ereignis ist höchstens eine Stufe, auch mit 500 Pixeln', async ({ page }) => {
  await amUnterenEnde(page);
  await strom(page, 1, 500);
  expect(await zuletzt(page)).toBe(-1);
});

test('unter Windows sind 6 Zeilen je Raste, deltaY 200, eine Stufe', async ({ page }) => {
  await plattform(page, 'Win32');
  await amUnterenEnde(page);
  await reaktionRoh(page, -200);
  expect(await zuletzt(page)).toBe(-1);
});

test('der Rest einer Stufe verfällt: 75, 1 und 29 Pixel im Takt von 100 ms sind eine Stufe', async ({ page }) => {
  await amUnterenEnde(page);
  await imTakt(page, [75, 1, 29], 100);
  expect(await zuletzt(page)).toBe(-1);
});

for (const [name, zahl, pixel] of [
  ['49 Pixel in kleinen Schritten sind eine Stufe, wenn die Geste endet', 49, 1],
  ['50 Pixel in kleinen Schritten sind eine Stufe', 25, 2],
  ['98 Pixel in kleinen Schritten sind eine Stufe', 49, 2],
] as const) {
  test(name, async ({ page }) => {
    await amUnterenEnde(page);
    await strom(page, zahl, pixel);
    expect(await zuletzt(page)).toBe(-1);
  });
}

test('50 Pixel auf einmal zoomen sofort, 49 erst nach 150 ms Ruhe', async ({ page }) => {
  await amUnterenEnde(page);
  expect(await reaktion(page, 49)).toBeGreaterThanOrEqual(149);
  expect(await zuletzt(page)).toBe(-1);
  const ms = await reaktion(page, 50);
  expect(ms).toBeGreaterThanOrEqual(0);
  expect(ms).toBeLessThan(100);
});

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

test('an der Grenze tut eine Raste nichts, und die Gegenrichtung wirkt gleich', async ({ page }) => {
  await aufStufe(page, 2);
  expect(await reaktion(page, 60)).toBe(-1);
  const ms = await reaktion(page, -60);
  expect(ms).toBeGreaterThanOrEqual(0);
  expect(ms).toBeLessThan(100);
  expect(await zuletzt(page)).toBe(1);
});

// Nach dem Quelltext von Chromium meldet eine Raste mit 3 Zeilen unter Windows 100, bei jeder Skalierung; siehe 0135.
for (const dpr of [1, 1.25, 1.5, 2]) {
  test.describe(`unter Windows bei DPR ${dpr}`, () => {
    test.use({ deviceScaleFactor: dpr });
    test(`DPR ${dpr}: deltaY 100 ist sofort genau eine Stufe, unter 100 ms`, async ({ page }) => {
      await plattform(page, 'Win32');
      await amUnterenEnde(page);
      const ms = await reaktionRoh(page, -100);
      expect(ms).toBeGreaterThanOrEqual(0);
      expect(ms).toBeLessThan(100);
      expect(await zuletzt(page)).toBe(-1);
    });
    test(`DPR ${dpr}: deltaY 99 bleibt unter der Stufe und wirkt erst nach 150 ms Ruhe`, async ({ page }) => {
      await plattform(page, 'Win32');
      await amUnterenEnde(page);
      expect(await reaktionRoh(page, -99)).toBeGreaterThanOrEqual(149);
      expect(await zuletzt(page)).toBe(-1);
    });
  });
}

test('unter Windows gilt im Zeilenmodus weiter getWheelDelta: 3 Zeilen sind sofort eine Stufe', async ({ page }) => {
  await plattform(page, 'Win32');
  await amUnterenEnde(page);
  const ms = await reaktionRoh(page, -3, 1);
  expect(ms).toBeGreaterThanOrEqual(0);
  expect(ms).toBeLessThan(100);
});

test('ausserhalb von Windows gilt weiter getWheelDelta: Am Mac sind deltaY 149 keine sofortige Stufe', async ({ page }) => {
  await plattform(page, 'MacIntel');
  await amUnterenEnde(page);
  expect(await reaktionRoh(page, -149)).toBeGreaterThanOrEqual(149);
});

test('bei einer gebrochenen Grenze 0,6 unter der ganzen Stufe erreicht das Rad sie und kommt ganz zurück', async ({ page }) => {
  // Das Tablett kommt nach 0067 nie so tief; darum eine eigene Grenze in map.json.
  await welt(page, staedte([], { mehr: { minZoom: 0.4 } }));
  await page.goto(`${DEMO}&at=35,0,-15&zoom=-1`);
  await page.waitForSelector('.kacheln img.leaflet-tile-loaded', { state: 'attached' });
  await page.mouse.move(640, 360);
  await reaktion(page, -60);
  await expect.poll(() => zoomDerAdresse(page)).toBeCloseTo(-1.6, 9);
  await reaktion(page, 60);
  await expect.poll(() => zoomDerAdresse(page)).toBe(-1);
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

test('bleibt zoomend aus, ist das Rad nach 1000 ms wieder frei, und was wartete, verfällt', async ({ page }) => {
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
        // Noch belegt: Diese beginnt nichts und ist bei 1000 ms zu alt.
        setTimeout(raste, 600);
        // Frei: Erst diese zoomt.
        setTimeout(raste, 1100);
      }),
    -RASTE,
  );
  expect(ms).toBeGreaterThanOrEqual(1090);
  expect(await zuletzt(page)).toBe(-1);
});

test('während der Animation der Knöpfe folgt eine Raste, solange das Rad dreht', async ({ page }) => {
  await amUnterenEnde(page);
  await page.evaluate(
    (deltaY) =>
      new Promise<void>((fertig) => {
        const w = window as unknown as { uhr: number };
        w.uhr = 1000;
        performance.now = () => w.uhr;
        new MutationObserver((_, beobachter) => {
          if (!document.querySelector('.leaflet-zoom-anim')) return;
          beobachter.disconnect();
          document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
          fertig();
        }).observe(document.body, { attributes: true, subtree: true, attributeFilter: ['class'] });
        document.querySelector<HTMLElement>('.leaflet-control-zoom-in')!.click();
      }),
    -RASTE,
  );
  expect(await zuletzt(page)).toBe(0);
});

/**
 * Zwei Rasten im Abstand von 150 ms, dazu `dazwischen` im ersten Bild nach dem
 * Ende der ersten Animation, also zwischen ihrem `zoomend` und der Folgestufe;
 * gibt die Dauer jeder Animation in ms zurück. Hinein positiv.
 */
const mitLuecke = (page: Page, dazwischen: number, danach: number[] = []) =>
  page.evaluate(
    async ([raste, d, weiter]) => {
      const pane = document.querySelector('.leaflet-map-pane')!;
      const rad = (r: number) => document.querySelector('#map')!.dispatchEvent(new WheelEvent('wheel', { deltaY: -r * raste, clientX: 640, clientY: 360, bubbles: true, cancelable: true }));
      const dauern: number[] = [];
      let start: number | undefined;
      new MutationObserver(() => {
        const an = pane.classList.contains('leaflet-zoom-anim');
        if (an && start === undefined) start = performance.now();
        if (!an && start !== undefined) {
          dauern.push(Math.round(performance.now() - start));
          start = undefined;
          if (dauern.length === 1) rad(d);
        }
      }).observe(pane, { attributes: true, attributeFilter: ['class'] });
      rad(1);
      await new Promise((r) => setTimeout(r, 150));
      rad(1);
      await new Promise((r) => setTimeout(r, 800));
      for (const w of weiter) {
        rad(w);
        await new Promise((r) => setTimeout(r, 600));
      }
      return dauern;
    },
    [RASTE, dazwischen, danach] as const,
  );

test('ein Ereignis zwischen dem Ende einer Animation und ihrer Folgestufe merkt nur die Richtung: Jede Animation läuft ganz, keine Stufe kommt doppelt', async ({ page }) => {
  await aufStufe(page, -2);
  const dauern = await mitLuecke(page, 1);
  // Zwei Stufen: die erste Raste und die Folgestufe; die Raste in der Lücke wartet und verfällt danach, weil das Rad dann 250 ms still steht.
  expect(zoomDerAdresse(page)).toBe(0);
  expect(dauern).toHaveLength(2);
  for (const ms of dauern) expect(ms).toBeGreaterThanOrEqual(200);
});

test('zwischen dem Ende einer Animation und ihrer Folgestufe an der Grenze bleibt das Rad nicht gesperrt', async ({ page }) => {
  await aufStufe(page, 1);
  // Die Folgestufe ginge über 2 hinaus und tut nichts; eine Raste hinaus danach zoomt sofort.
  await mitLuecke(page, 1, [-1]);
  expect(zoomDerAdresse(page)).toBe(1);
});
