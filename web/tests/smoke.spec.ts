import { expect, test, type Locator, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { projiziere, zweiZuEins } from '../src/pick';

/** Ein kleiner Kachelbaum, der mit im Repository liegt. */
const DEMO = '/?tiles=/tiles-demo';

/**
 * Höhen für den Demobaum, je 4 × 4 Spalten: eben auf Y 0, dazu eine Säule
 * bis Y 5 in der Zelle der Spalten 32 bis 35 und -16 bis -13, in einer
 * negativen Region. Ein falscher Platz in der Höhenkarte fiele so auf.
 * `saeule` setzt sie in eine andere Datei und Zelle.
 */
async function welt(
  page: Page,
  mehr: object = {},
  saeule: [datei: string, zelle: number] = ['0.-1.bin', (-4 + 128) * 128 + 8],
): Promise<void> {
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    const hoehen = { heights: 'heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    await route.fulfill({ response, json: { ...info, ...hoehen, ...mehr } });
  });
  await page.route('**/tiles-demo/heights/*.bin', async (route) => {
    const karte = new Int16Array(128 * 128);
    if (route.request().url().endsWith(`/${saeule[0]}`)) karte[saeule[1]] = 5;
    await route.fulfill({ body: deflateSync(Buffer.from(karte.buffer)) });
  });
}

/**
 * Die Mitte eines Pixels der feinsten Stufe auf dem Bildschirm. Der
 * Demobaum passt auf Stufe 2 ins Fenster, ein Pixel der Kachel ist dann
 * einer des Bildschirms.
 */
async function bildschirm(page: Page, u: number, v: number): Promise<[number, number]> {
  const kachel = await page.locator('.kacheln img[src$="/2/0/0.webp"]').boundingBox();
  if (!kachel) throw new Error('Kachel 2/0/0 nicht zu sehen');
  return [kachel.x + u + 0.5, kachel.y + v + 0.5];
}

/** Die Zoomstufen, aus denen die sichtbaren Kacheln stammen. */
async function tileZooms(page: Page): Promise<number[]> {
  const quellen = await page.locator('.kacheln img.leaflet-tile-loaded').evaluateAll((bilder) =>
    bilder.map((bild) => (bild as HTMLImageElement).src),
  );
  const stufen = quellen
    .map((src) => /\/tiles-demo\/(\d+)\//.exec(src)?.[1])
    .filter((z): z is string => z !== undefined)
    .map(Number);
  return [...new Set(stufen)].sort((a, b) => a - b);
}

/**
 * Um welchen Faktor Leaflet die vorhandenen Kacheln vergrössert.
 *
 * Jenseits der feinsten gerenderten Stufe gibt es keine neuen Kacheln
 * mehr; Leaflet skaliert dann den Kachelcontainer.
 */
async function tileStretch(page: Page): Promise<number> {
  const transform = await page
    .locator('.kacheln .leaflet-tile-container')
    .first()
    .evaluate((element) => getComputedStyle(element).transform);
  return Number(/^matrix\(([\d.]+)/.exec(transform)?.[1] ?? 1);
}

/**
 * Klickt einen Zoomknopf und wartet, bis Leaflet fertig ist.
 *
 * Ein zweiter Klick mitten in der Animation geht verloren. Leaflet hängt
 * für ihre Dauer `leaflet-zoom-anim` an die Kartenebene — erst kommt die
 * Klasse, dann ist sie wieder weg, und dann ist es sicher.
 */
async function zoomClick(page: Page, control: Locator): Promise<void> {
  const pane = page.locator('.leaflet-map-pane');
  await control.click();
  await expect(pane).toHaveClass(/leaflet-zoom-anim/);
  await expect(pane).not.toHaveClass(/leaflet-zoom-anim/);
}

test('die Karte laedt Kacheln, ohne zu meckern', async ({ page }) => {
  const fehler: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') fehler.push(m.text());
  });
  page.on('pageerror', (e) => fehler.push(e.message));

  await page.goto(DEMO);

  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await tileZooms(page)).not.toEqual([]);
  expect(fehler).toEqual([]);
  // Ohne Höhen in map.json keine Koordinaten, ohne seaLevel und area kein Tablett.
  await expect(page.locator('.koordinaten')).toHaveCount(0);
  await expect(page.locator('canvas.tablett')).toHaveCount(0);
});

test('ohne Skin hat die UI die Farben von heute, aus den Variablen der Grundkarte', { tag: '@ohne-skin' }, async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('.stand')).toBeVisible();
  const stil = (selector: string, eigenschaft: string) =>
    page.locator(selector).first().evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), eigenschaft);
  expect(await stil('#map', 'background-color')).toBe('rgb(18, 22, 28)');
  expect(await stil('.leaflet-control-zoom-in', 'background-color')).toBe('rgb(255, 255, 255)');
  expect(await stil('.leaflet-control-zoom-in', 'color')).toBe('rgb(0, 0, 0)');
  expect(await stil('.leaflet-bar', 'border-top-color')).toBe('rgba(0, 0, 0, 0.2)');
  expect(await stil('.stand', 'background-color')).toBe('rgba(255, 255, 255, 0.8)');
  expect(await stil('.kompass', 'background-color')).toBe('rgba(255, 255, 255, 0.8)');
});

test('die Variablen der UI in skin-api.ts sind die aus style.css', { tag: '@ohne-skin' }, () => {
  const text = (datei: string) => readFileSync(new URL(`../src/${datei}`, import.meta.url), 'utf8');
  const namen = (quelle: string | undefined, muster: RegExp) => [...(quelle ?? '').matchAll(muster)].map((m) => m[1]).sort();
  const css = namen(/:root\s*\{([^}]*)\}/.exec(text('style.css'))?.[1], /(--[\w-]+):/g);
  expect(css.length).toBeGreaterThan(10);
  expect(namen(/type UiVariable =([^;]*);/.exec(text('skin-api.ts'))?.[1], /'(--[\w-]+)'/g)).toEqual(css);
});

test('ohne Skin beim Build kein Tablett und kein Code eines Skins, auch mit quadratischem area', {
  tag: '@ohne-skin',
}, async ({ page }) => {
  const skripte: string[] = [];
  page.on('request', (anfrage) => {
    if (anfrage.resourceType() === 'script') skripte.push(anfrage.url());
  });
  await welt(page, { seaLevel: 0, area: [-64, -64, 64, 64] });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('canvas.tablett')).toHaveCount(0);
  // Nur das Bündel der Karte.
  expect(skripte).toHaveLength(1);
});

test('map.json, trees.json und Höhen fragt die Seite jedes Mal beim Server nach', async ({
  page,
}) => {
  // Mit Routing schaltet Playwright den Cache ab; geprüft wird deshalb, mit
  // welchem Cache-Modus die Seite `fetch` aufruft.
  await page.addInitScript(() => {
    const modi: Record<string, string | undefined> = {};
    (window as unknown as { modi: typeof modi }).modi = modi;
    const holen = window.fetch.bind(window);
    window.fetch = (eingabe, init) => {
      const url = eingabe instanceof Request ? eingabe.url : eingabe.toString();
      const datei = /\/(map\.json|trees\.json|[-\d]+\.[-\d]+\.bin)$/.exec(url)?.[1];
      if (datei) modi[datei.endsWith('.bin') ? 'Höhen' : datei] = init?.cache;
      return holen(eingabe, init);
    };
  });
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
  expect(await page.evaluate(() => (window as unknown as { modi: object }).modi)).toEqual({
    'trees.json': 'no-cache',
    'map.json': 'no-cache',
    Höhen: 'no-cache',
  });
});

test('ohne Routing schickt Chromium dafür Cache-Control: max-age=0', async ({ page }) => {
  // Dieselbe Anfrage, wie sie beim Server ankommt; ohne Routing wirkt der
  // Cache des Browsers. Der Demobaum hat keine Höhen.
  const kopf = new Map<string, string | undefined>();
  page.on('request', (anfrage) => {
    const datei = /\/(map\.json|trees\.json)$/.exec(anfrage.url())?.[1];
    if (datei) void anfrage.allHeaders().then((k) => kopf.set(datei, k['cache-control']));
  });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect
    .poll(() => Object.fromEntries(kopf))
    .toEqual({ 'trees.json': 'max-age=0', 'map.json': 'max-age=0' });
});

test('die Karte läuft unter strengen Headern', async ({ page }) => {
  await page.addInitScript(() => {
    const verletzt: string[] = [];
    (window as unknown as { verletzt: string[] }).verletzt = verletzt;
    document.addEventListener('securitypolicyviolation', (e) =>
      verletzt.push(`${e.effectiveDirective} ${e.blockedURI}`),
    );
  });
  await welt(page);
  const antwort = await page.goto(DEMO);
  // Jeder Header aus web/headers.json, wörtlich; dieselbe Datei nimmt der Server des Renderers.
  const header = JSON.parse(readFileSync(new URL('../headers.json', import.meta.url), 'utf8')) as Record<string, string>;
  expect(Object.keys(header)).toContain('Content-Security-Policy');
  for (const [name, wert] of Object.entries(header)) expect(antwort?.headers()[name.toLowerCase()], name).toBe(wert);

  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  // Koordinaten holen Höhen und entpacken sie; auch das muss erlaubt sein.
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual(
    [],
  );
});

test('Kacheln folgt Leaflet 1.9.4: Ein anderes Leaflet braucht einen Blick auf die fünf Methoden der Unterklasse', () => {
  const { version } = JSON.parse(readFileSync(new URL('../node_modules/leaflet/package.json', import.meta.url), 'utf8')) as { version: string };
  expect(version, 'siehe docs/frontend.md, „Ausliefern“').toBe('1.9.4');
});

/**
 * Ein grosser Baum unter `/tiles-gross` mit Stufen 0 bis 5, auf Stufe 5 also
 * 32 × 32 Kacheln, jede die Kachel 0/0/0 des Demobaums. `halte` entscheidet je
 * Adresse, ob die Antwort wartet, bis der Test sie freigibt; `angefragt`
 * zählt jede Anfrage.
 */
async function grosserBaum(page: Page, halte: (url: string) => boolean = () => true) {
  const bild = readFileSync(new URL('../public/tiles-demo/0/0/0.webp', import.meta.url));
  const angefragt: string[] = [];
  const haengend: (() => void)[] = [];
  await page.route('**/tiles-gross/trees.json', (route) => route.fulfill({ status: 404 }));
  await page.route('**/tiles-gross/layers.json', (route) => route.fulfill({ status: 404 }));
  await page.route('**/tiles-gross/map.json', (route) => route.fulfill({ json: { tileSize: 256, scale: 16, minZoom: 0, maxZoom: 5, tiles: '{z}/{x}/{y}.webp', bounds: [0, 0, 8192, 8192] } }));
  await page.route('**/tiles-gross/*/*/*.webp', async (route) => {
    angefragt.push(route.request().url());
    if (halte(route.request().url())) await new Promise<void>((weiter) => haengend.push(weiter));
    await route.fulfill({ body: bild, contentType: 'image/webp' }).catch(() => undefined);
  });
  // Der Block, der auf Stufe 5 in der Mitte der Welt liegt.
  let [bx, bz, best] = [0, 0, Infinity];
  for (let x = -40; x <= 600; x++) {
    for (let z = -600; z <= 40; z++) {
      const [px, py] = projiziere(x + 0.5, 1, z + 0.5, zweiZuEins(16));
      const d = Math.hypot(px - 4096, py - 4096);
      if (d < best) [bx, bz, best] = [x, z, d];
    }
  }
  return { angefragt, haengend, mitte: `/?tiles=/tiles-gross&at=${bx},0,${bz}&zoom=0` };
}

/** Wie viele Kacheln einer Ebene laden: `src` gesetzt, noch nicht fertig. */
const laufend = (page: Page, ebene = '') => page.locator(`${ebene} img.leaflet-tile`).evaluateAll((l) => l.filter((i) => (i as HTMLImageElement).hasAttribute('src') && !(i as HTMLImageElement).complete).length);
/** Ersetzt das Protokoll, das Resource Timing nennt, etwa '' für eine Kachel aus dem Cache. */
const protokoll = (page: Page, wert: string) =>
  page.addInitScript((w) => Object.defineProperty(PerformanceResourceTiming.prototype, 'nextHopProtocol', { get: () => w }), wert);

test('die Warteschlange lädt höchstens 6 Kacheln zugleich, die Grundschicht zuerst; ist das Protokoll leer, wie aus dem Cache, bleibt es bei 6', async ({ page }) => {
  await protokoll(page, '');
  // Nur die erste Kachel kommt gleich, so ist eine fertig, bevor gezählt wird.
  let erste = true;
  const { angefragt, haengend, mitte } = await grosserBaum(page, () => {
    const halten = !erste;
    erste = false;
    return halten;
  });
  await page.goto(mitte);
  await expect.poll(() => angefragt.length).toBeGreaterThan(1);
  await page.waitForTimeout(500);
  // Die Grundschicht auf Stufe 2 ging zuerst ans Netz.
  expect(angefragt[0]).toMatch(/\/tiles-gross\/2\//);
  expect(await laufend(page)).toBe(6);
  expect(angefragt.length).toBe(7);
  // Es warten mehr, ohne src.
  expect(await page.locator('.kacheln img.leaflet-tile:not([src])').count()).toBeGreaterThan(6);
  // Eine fertig: die nächste rückt nach, wieder 6.
  haengend.shift()!();
  await expect.poll(() => angefragt.length).toBe(8);
  expect(await laufend(page)).toBe(6);
});

test('mit h2 lädt die Warteschlange 16 Kacheln zugleich', async ({ page }) => {
  await protokoll(page, 'h2');
  let erste = true;
  const { angefragt, mitte } = await grosserBaum(page, () => {
    const halten = !erste;
    erste = false;
    return halten;
  });
  await page.goto(mitte);
  await expect.poll(() => laufend(page)).toBe(16);
  await page.waitForTimeout(300);
  expect(angefragt.length).toBe(17);
});

test('nach dem Zoomen geht eine nicht mehr gebrauchte Kachel nie ans Netz; laufende laden losgelöst zu Ende, ohne Abbruch, ohne Verletzung der CSP, und tauchen nicht wieder auf', async ({ page }) => {
  await page.addInitScript(() => {
    const verletzt: string[] = [];
    (window as unknown as { verletzt: string[] }).verletzt = verletzt;
    document.addEventListener('securitypolicyviolation', (e) => verletzt.push(`${e.effectiveDirective} ${e.blockedURI}`));
  });
  // Stufe 5 hängt, alles andere kommt gleich.
  const { angefragt, haengend, mitte } = await grosserBaum(page, (url) => url.includes('/tiles-gross/5/'));
  const abgebrochen: string[] = [];
  page.on('requestfailed', (r) => r.url().endsWith('.webp') && abgebrochen.push(r.url()));
  await page.goto(mitte);
  await expect.poll(() => laufend(page, '.kacheln')).toBe(6);
  // Die laufenden der Stufe 5 merken, samt den wartenden.
  const wartend = await page.locator('.kacheln img.leaflet-tile:not([src])').count();
  expect(wartend).toBeGreaterThan(0);
  await page.evaluate(() => {
    (window as unknown as { alte: HTMLImageElement[] }).alte = [...document.querySelectorAll<HTMLImageElement>('.kacheln img.leaflet-tile[src*="/tiles-gross/5/"]')];
  });
  await page.locator('.leaflet-control-zoom-out').click();
  await page.waitForTimeout(300);
  // Die 6 laufenden halten ihre Plätze, bis sie fertig sind; dann kommt Stufe 4.
  for (const weiter of haengend) weiter();
  await expect(page.locator('.kacheln img.leaflet-tile-loaded[src*="/tiles-gross/4/"]').first()).toBeVisible();
  await page.waitForTimeout(500);
  // Nur die 6 laufenden gingen auf Stufe 5 ans Netz, keine abgebrochen.
  expect(angefragt.filter((u) => u.includes('/tiles-gross/5/')).length).toBe(6);
  expect(abgebrochen).toEqual([]);
  // Die losgelösten sind fertig, aber nicht in der Karte und nicht als geladen markiert.
  const alte = await page.evaluate(() => (window as unknown as { alte: HTMLImageElement[] }).alte.map((i) => [i.complete, i.isConnected, i.classList.contains('leaflet-tile-loaded')]));
  expect(alte).toEqual(Array.from({ length: 6 }, () => [true, false, false]));
  await expect(page.locator('img[src*="/tiles-gross/5/"]')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual([]);
});

test('sind alle Plätze belegt, bekommt die Grundschicht den nächsten freien vor den wartenden feinen Kacheln', async ({ page }) => {
  // Die Grundschicht der ersten Stufe kommt gleich; alles andere hängt.
  const { angefragt, haengend, mitte } = await grosserBaum(page, (url) => !url.includes('/tiles-gross/2/'));
  await page.goto(mitte);
  await expect.poll(() => laufend(page, '.kacheln')).toBe(6);
  // Hinaus: Die neue Grundschicht auf Stufe 1 und die feinen auf Stufe 4 warten, denn die 6 alten laden losgelöst weiter.
  await page.locator('.leaflet-control-zoom-out').click();
  await page.waitForTimeout(400);
  const bisher = angefragt.length;
  haengend.shift()!();
  await expect.poll(() => angefragt.length).toBeGreaterThan(bisher);
  expect(angefragt[bisher]).toMatch(/\/tiles-gross\/1\//);
  for (const weiter of haengend.splice(0)) weiter();
});

test('zurückgezoomt markiert eine losgelöste Kachel, die zu Ende lädt, nicht die neue an derselben Stelle als geladen', async ({ page }) => {
  const { haengend, mitte } = await grosserBaum(page, (url) => url.includes('/tiles-gross/5/'));
  await page.goto(mitte);
  await expect.poll(() => laufend(page, '.kacheln')).toBe(6);
  // Hinaus und gleich wieder hinein: Die neuen Kacheln der Stufe 5 haben dieselben Schlüssel und warten, die alten laufen losgelöst.
  await page.locator('.leaflet-control-zoom-out').click();
  await page.waitForTimeout(400);
  await page.locator('.leaflet-control-zoom-in').click();
  await page.waitForTimeout(400);
  // Zählt, wie oft eine Kachel als geladen markiert wird, bevor sie überhaupt ein src hat; die Einträge kommen in ihrer Reihenfolge.
  await page.evaluate(() => {
    const w = window as unknown as { zuFrueh: number };
    w.zuFrueh = 0;
    const mitSrc = new WeakSet<Element>();
    for (const i of document.querySelectorAll('img[src]')) mitSrc.add(i);
    new MutationObserver((l) => {
      for (const m of l) {
        if (m.attributeName === 'src') mitSrc.add(m.target as Element);
        else if ((m.target as Element).classList.contains('leaflet-tile-loaded') && !(m.oldValue ?? '').includes('leaflet-tile-loaded') && !mitSrc.has(m.target as Element)) w.zuFrueh++;
      }
    }).observe(document, { subtree: true, attributes: true, attributeFilter: ['class', 'src'], attributeOldValue: true });
  });
  const vorher = haengend.length;
  for (const weiter of haengend.splice(0, vorher)) weiter();
  await page.waitForTimeout(500);
  // Keine Kachel wurde als geladen markiert, bevor sie selbst angefragt war.
  expect(await page.evaluate(() => (window as unknown as { zuFrueh: number }).zuFrueh)).toBe(0);
  for (const weiter of haengend.splice(0)) weiter();
});

/** Die Stufen der geladenen Kacheln einer Ebene, `.grund` oder `.kacheln`. */
const stufenIn = (page: Page, ebene: string) =>
  page.locator(`${ebene} img.leaflet-tile-loaded`).evaluateAll((l) => [...new Set(l.map((i) => Number(/\/tiles-demo\/(\d+)\//.exec((i as HTMLImageElement).src)![1])))].sort());

test('die Grundschicht liegt unter den Kacheln, gröber, und steht da, solange die feinen noch laden', async ({ page }) => {
  // Kacheln der Stufe 2 hängen, bis der Test sie freigibt.
  const haengend: (() => void)[] = [];
  await page.route('**/tiles-demo/2/*/*.webp', async (route) => {
    await new Promise<void>((weiter) => haengend.push(weiter));
    await route.fulfill({ response: await route.fetch() }).catch(() => undefined);
  });
  await page.goto(`${DEMO}&at=20,0,-20&zoom=0`);
  // Die feinen fehlen noch; die Grundschicht der Stufe 0 steht schon da.
  await expect(page.locator('.grund img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await stufenIn(page, '.grund')).toEqual([0]);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded')).toHaveCount(0);
  const rechtecke = (ebene: string) =>
    page.locator(`${ebene} img.leaflet-tile-loaded`).evaluateAll((l) => l.map((i) => i.getBoundingClientRect()).map((r) => [r.left, r.top, r.right, r.bottom]));
  const grund = await rechtecke('.grund');
  for (const weiter of haengend) weiter();
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await stufenIn(page, '.kacheln')).toEqual([2]);
  // Wo jetzt eine feine Kachel liegt, lag vorher schon die Grundschicht.
  for (const [x0, y0, x1, y1] of await rechtecke('.kacheln')) {
    const [x, y] = [(x0! + x1!) / 2, (y0! + y1!) / 2];
    expect(grund.some(([a, b, c, d]) => x >= a! && x < c! && y >= b! && y < d!), `${x}, ${y}`).toBe(true);
  }
  // Die feinen liegen darüber.
  const lagen = await page.evaluate(() => ['.grund', '.kacheln'].map((k) => Number(getComputedStyle(document.querySelector(k)!).zIndex)));
  expect(lagen[0]).toBeLessThan(lagen[1]!);
});

test('die Grundschicht liegt drei Stufen gröber und lädt nur das Fenster', async ({ page }) => {
  // Zwei Stufen über der feinsten nehmen die feinen Stufe 2, eine Kachel 1024 px; ein kleines Fenster mitten in Kachel 2/0/0 sieht nur sie.
  await page.setViewportSize({ width: 400, height: 300 });
  let [bx, bz, best] = [0, 0, Infinity];
  for (let x = -60; x <= 60; x++) {
    for (let z = -60; z <= 60; z++) {
      const [px, py] = projiziere(x + 0.5, 1, z + 0.5, zweiZuEins(16));
      const d = Math.hypot(px - 128, py - 128);
      if (d < best) [bx, bz, best] = [x, z, d];
    }
  }
  await page.goto(`${DEMO}&at=${bx},0,${bz}&zoom=2`);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded')).toHaveCount(1);
  // Die Grundschicht drei Stufen darunter: Stufe 1, eine Kachel über 2048 px, die das Fenster deckt; sonst keine.
  await expect(page.locator('.grund img.leaflet-tile-loaded')).toHaveCount(1);
  await page.waitForTimeout(300);
  await expect(page.locator('.grund img.leaflet-tile')).toHaveCount(1);
  expect(await stufenIn(page, '.grund')).toEqual([1]);
});

test('die Koordinaten rechnen mit projection aus map.json', async ({ page }) => {
  // Eine Draufsicht über den Kacheln von 2:1: falsch fürs Auge, aber so
  // zeigt sich, dass projection gilt und nicht scale.
  await welt(page, { camera: 'top', projection: { azimuth: 'diagonal', u: 8, v: 8, y: 0 } });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 27  Y 0  Z -23');
});

test('aus Nordwesten zeigt die Anzeige Weltkoordinaten', async ({ page }) => {
  // Im Blick aus Nordwesten liegt (35, 5, -15) dort, wo aus Südosten
  // derselbe Pixel ist; in der Welt ist das (-36, 5, 14). Dort steht die
  // Säule: Zelle der Spalten -36 bis -33 und 12 bis 15, Region -1.0.
  await welt(page, { direction: 'nw' }, ['-1.0.bin', 3 * 128 + 119]);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X -36  Y 5  Z 14');
});

test('genordet rechnen die Koordinaten mit u = x und v = z', async ({ page }) => {
  await welt(page, {
    camera: 'top-north',
    direction: 's',
    projection: { azimuth: 'north', u: 16, v: 16, y: 0 },
  });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  // Die Mitte des Pixels (400, 36) liegt über Spalte 400,5 / 16 und 36,5 / 16.
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 25  Y 0  Z 2');
});

for (const [mehr, grund] of [
  [{ projection: { azimuth: 'up', u: 16, v: 16, y: 0 } }, 'azimuth up unbekannt'],
  [{ direction: 's' }, 'direction s unbekannt'],
  [
    { direction: 'se', projection: { azimuth: 'north', u: 16, v: 16, y: 16 } },
    'direction se unbekannt',
  ],
  [{ projection: { azimuth: 'diagonal', u: 8, v: 0, y: 8 } }, 'projection ohne ganze u, v und y'],
] as const) {
  test(`eine Kamera, die das Frontend nicht kennt, zeigt keine Koordinaten: ${grund}`, async ({
    page,
  }) => {
    const warnungen: string[] = [];
    page.on('console', (m) => {
      if (m.type() === 'warning') warnungen.push(m.text());
    });
    await welt(page, mehr);
    await page.goto(DEMO);

    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    await expect(page.locator('.koordinaten')).toHaveCount(0);
    expect(warnungen).toContain(`/tiles-demo/map.json: keine Koordinaten, ${grund}`);
  });
}

test('unvollständige Höhen lassen die Karte stehen', async ({ page }) => {
  await page.route('**/tiles-demo/map.json', async (route) => {
    const response = await route.fetch();
    const info = (await response.json()) as object;
    // heights ohne heightsCell, etwa aus einem halben Stand.
    await route.fulfill({ response, json: { ...info, heights: 'heights/{x}.{z}.bin' } });
  });
  await page.goto(DEMO);

  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.koordinaten')).toHaveCount(0);
});

test('die Maus zeigt Koordinaten des Blocks darunter, ohne Umriss', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  await expect(anzeige).toHaveText('X –  Y –  Z –');

  // Die Mitte der Oberseite von (35, 5, -15) und von (40, 0, 20).
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  await page.mouse.move(...(await bildschirm(page, 160, 236)));
  await expect(anzeige).toHaveText('X 40  Y 0  Z 20');
  // Den Block zeigt der Mauszeiger; ein Umriss hat keine Linie.
  await expect(page.locator('.leaflet-shadow-pane path')).not.toHaveAttribute('d', /M[^M]+M/);
});

test('das Kopiersymbol kopiert /tp, ein Klick hält den Block dafür fest', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  const kopieren = page.getByRole('button', { name: '/tp kopieren' });
  const meldung = page.getByRole('status');
  const linie = page.locator('.leaflet-shadow-pane path');

  // Gehalten zeigt es die Anzeige; einen Umriss gibt es mit der Maus nach
  // 0049 weiter nicht. Unterwegs zum Knopf bleibt der Block.
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  await expect(anzeige).toHaveClass(/gehalten/);
  await expect(linie).not.toHaveAttribute('d', /M[^M]+M/);
  await page.mouse.move(...(await bildschirm(page, 160, 236)), { steps: 5 });
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');

  // Ziehen in der Leiste verschiebt die Karte nicht und wählt keinen Block,
  // auch wenn es über der Karte endet.
  const ebene = page.locator('.leaflet-map-pane');
  const lage = await ebene.evaluate((e) => (e as HTMLElement).style.transform);
  const leiste = (await anzeige.boundingBox())!;
  await page.mouse.move(leiste.x + 10, leiste.y + 5);
  await page.mouse.down();
  await page.mouse.move(leiste.x + 80, leiste.y - 60, { steps: 5 });
  await page.mouse.up();
  expect(await ebene.evaluate((e) => (e as HTMLElement).style.transform)).toBe(lage);
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');

  // Einen Block höher als der gezeigte, sonst steht man im Block; x und z
  // rückt das Spiel auf die Mitte (TeleportCommand, Client 26.2, per javap).
  await kopieren.click();
  await expect(meldung).toHaveText('Kopiert: /tp 35 6 -15');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('/tp 35 6 -15');
  await expect(meldung).toBeEmpty();

  // Per Tastatur ebenso.
  await page.evaluate(() => navigator.clipboard.writeText(''));
  await kopieren.focus();
  await page.keyboard.press('Enter');
  await expect(meldung).toHaveText('Kopiert: /tp 35 6 -15');

  // Escape lässt los, dann folgt die Anzeige wieder der Maus.
  await page.keyboard.press('Escape');
  await expect(anzeige).not.toHaveClass(/gehalten/);
  await page.mouse.move(...(await bildschirm(page, 160, 236)));
  await expect(anzeige).toHaveText('X 40  Y 0  Z 20');
});

test('ein Klick auf X macht es editierbar, Enter springt hin, Y aus der Höhenkarte', async ({
  page,
}) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');

  await page.locator('.wert').first().click();
  const feld = page.getByRole('textbox', { name: 'X eingeben' });
  await expect(feld).toHaveValue('35');
  await expect(feld).toBeFocused();
  // Text statt Zahl, damit jede Tastatur auf dem Handy das Minus bietet.
  await expect(feld).toHaveAttribute('type', 'text');
  await expect(feld).not.toHaveAttribute('inputmode', /.*/);
  // Die anderen Werte bleiben, wie sie beim Klick standen.
  await page.mouse.move(...(await bildschirm(page, 160, 236)), { steps: 5 });
  await expect(page.locator('.wert')).toHaveText(['5', '-15']);

  // Neben der Säule ist der Boden bei Y 0; die Adresse folgt.
  await feld.fill('40');
  await feld.press('Enter');
  await expect(anzeige).toHaveText('X 40  Y 0  Z -15');
  await expect.poll(() => new URL(page.url()).searchParams.get('at')).toBe('40,0,-15');
  // Die Mitte der Karte zeigt jetzt diesen Block.
  await page.keyboard.press('Escape');
  expect(await mitte(page)).toBe('X 40  Y 0  Z -15');
});

test('wird Y selbst geändert, gilt es', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await page.locator('.wert').nth(1).click();
  const feld = page.getByRole('textbox', { name: 'Y eingeben' });
  await feld.fill('70');
  await feld.press('Enter');
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 70  Z -15');
});

test('unbrauchbare Eingaben weist die Anzeige sichtbar ab', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await page.locator('.wert').nth(2).click();
  const feld = page.getByRole('textbox', { name: 'Z eingeben' });
  const meldung = page.getByRole('status');
  for (const [text, grund] of [
    ['abc', 'Nur ganze Zahlen'],
    ['1.5', 'Nur ganze Zahlen'],
    ['', 'Nur ganze Zahlen'],
    ['30000001', 'X und Z bis ±30000000'],
  ]) {
    await feld.fill(text!);
    await feld.press('Enter');
    await expect(feld).toHaveClass(/falsch/);
    await expect(feld).toHaveAttribute('aria-invalid', 'true');
    await expect(meldung).toHaveText(grund!);
  }
  // Eine neue Eingabe nimmt die Markierung weg; ein Minus geht.
  await feld.fill('-20');
  await expect(feld).not.toHaveClass(/falsch/);
  await feld.press('Enter');
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 0  Z -20');

  await page.locator('.wert').nth(1).click();
  const hoehe = page.getByRole('textbox', { name: 'Y eingeben' });
  await hoehe.fill('400');
  await hoehe.press('Enter');
  await expect(meldung).toHaveText('Y von -64 bis 319');
});

test('wer abbricht, während die Höhenkarte lädt, springt nicht', async ({ page }) => {
  await welt(page);
  // Die Höhenkarte der Region um X 600 kommt erst, wenn der Test sie freigibt.
  let freigeben = () => {};
  const frei = new Promise<void>((weiter) => (freigeben = weiter));
  await page.route('**/tiles-demo/heights/1.-1.bin', async (route) => {
    await frei;
    await route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) });
  });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const ebene = page.locator('.leaflet-map-pane');
  const anzeige = page.locator('.koordinaten');
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  const lage = await ebene.evaluate((e) => (e as HTMLElement).style.transform);
  const at = new URL(page.url()).searchParams.get('at');

  await page.locator('.wert').first().click();
  const feld = page.getByRole('textbox', { name: 'X eingeben' });
  await feld.fill('600');
  const anfrage = page.waitForRequest('**/tiles-demo/heights/1.-1.bin');
  await feld.press('Enter');
  await anfrage;
  await feld.press('Escape');
  const antwort = page.waitForResponse('**/tiles-demo/heights/1.-1.bin');
  freigeben();
  await antwort;

  await expect(page.getByRole('textbox')).toHaveCount(0);
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  expect(await ebene.evaluate((e) => (e as HTMLElement).style.transform)).toBe(lage);
  expect(new URL(page.url()).searchParams.get('at')).toBe(at);
});

test('Escape oder ein Klick daneben bricht den Eintrag ab', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  const ebene = page.locator('.leaflet-map-pane');
  const lage = await ebene.evaluate((e) => (e as HTMLElement).style.transform);
  await page.mouse.click(...(await bildschirm(page, 400, 36)));

  await page.locator('.wert').first().click();
  await page.getByRole('textbox').fill('99');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('textbox')).toHaveCount(0);
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  // Escape bricht nur den Eintrag ab; der Block bleibt gehalten.
  await expect(anzeige).toHaveClass(/gehalten/);

  await page.locator('.wert').first().click();
  await page.getByRole('textbox').fill('99');
  await page.mouse.click(...(await bildschirm(page, 160, 236)));
  await expect(page.getByRole('textbox')).toHaveCount(0);
  await expect(anzeige).toHaveText('X 40  Y 0  Z 20');
  // Gesprungen ist die Karte dabei nicht.
  expect(await ebene.evaluate((e) => (e as HTMLElement).style.transform)).toBe(lage);
});

test('ohne gezeigten Block nimmt der Eintrag den Block in der Mitte, auch per Tastatur', async ({
  page,
}) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect.poll(() => new URL(page.url()).searchParams.get('at')).not.toBeNull();
  const at = new URL(page.url()).searchParams.get('at')!.split(',');
  await page.locator('.wert').first().focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('textbox', { name: 'X eingeben' })).toHaveValue(at[0]!);
  await expect(page.locator('.wert')).toHaveText([at[1]!, at[2]!]);
  // Auch ungehalten folgt die Anzeige beim Eintrag nicht der Maus.
  await page.mouse.move(...(await bildschirm(page, 400, 36)), { steps: 5 });
  await expect(page.locator('.wert')).toHaveText([at[1]!, at[2]!]);
});

test('ohne Zwischenablage sagt die Rückmeldung, warum nicht kopiert wird', async ({ page }) => {
  // Wie ausserhalb eines sicheren Kontexts: navigator.clipboard fehlt.
  await page.addInitScript(() => {
    delete (Navigator.prototype as { clipboard?: unknown }).clipboard;
  });
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const kopieren = page.getByRole('button', { name: '/tp kopieren' });
  await kopieren.click();
  await expect(page.getByRole('status')).toHaveText('Erst einen Block wählen');
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await kopieren.click();
  await expect(page.getByRole('status')).toHaveText('Kopieren geht nur über HTTPS');
});

test('schlägt das Kopieren fehl, sagt die Rückmeldung es', async ({ page }) => {
  await page.addInitScript(() => {
    Clipboard.prototype.writeText = () => Promise.reject(new Error('verweigert'));
  });
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.click(...(await bildschirm(page, 400, 36)));
  await page.getByRole('button', { name: '/tp kopieren' }).click();
  await expect(page.getByRole('status')).toHaveText('Kopieren fehlgeschlagen');
});

test.describe('auf dem Touchscreen', () => {
  test.use({ hasTouch: true });

  test('Tippen auf den Block, dann auf das Kopiersymbol kopiert /tp', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
    const knopf = (await page.getByRole('button', { name: '/tp kopieren' }).boundingBox())!;
    await page.touchscreen.tap(knopf.x + knopf.width / 2, knopf.y + knopf.height / 2);
    await expect(page.getByRole('status')).toHaveText('Kopiert: /tp 35 6 -15');
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('/tp 35 6 -15');
  });

  test('für Finger sind Werte und Kopiersymbol gross genug, das Feld hat 16 px', async ({
    page,
  }) => {
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
    // Auch ohne Block, wenn ein Wert nur „–“ zeigt: mindestens 24 px
    // (WCAG 2.5.8).
    for (const ziel of [
      ...(await page.locator('.wert').all()),
      page.getByRole('button', { name: '/tp kopieren' }),
    ]) {
      const { width, height } = (await ziel.boundingBox())!;
      expect(Math.min(width, height)).toBeGreaterThanOrEqual(24);
    }
    // Unter 16 px vergrössert iOS beim Fokus die ganze Seite.
    await page.locator('.wert').first().click();
    const feld = page.getByRole('textbox', { name: 'X eingeben' });
    expect(await feld.evaluate((e) => getComputedStyle(e).fontSize)).toBe('16px');
  });

  test('Tippen auf Z, ein negativer Wert und Enter springt hin', async ({ page }) => {
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    const z = (await page.locator('.wert').nth(2).boundingBox())!;
    await page.touchscreen.tap(z.x + z.width / 2, z.y + z.height / 2);
    const feld = page.getByRole('textbox', { name: 'Z eingeben' });
    await expect(feld).toBeFocused();
    await page.keyboard.press('Control+A');
    await page.keyboard.type('-20');
    await page.keyboard.press('Enter');
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 0  Z -20');
  });

  test('ein Tippen zeigt den Block darunter', async ({ page }) => {
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
    // Ohne Zeiger zeigt der Umriss den Block: Sechseck und die drei Kanten
    // der vorderen Ecke.
    const linie = page.locator('.leaflet-shadow-pane path');
    await expect(linie).toHaveAttribute('d', /^M[^M]+M[^M]+M[^M]+$/);

    // Kommt danach die Maus, zeigt wieder ihr Zeiger den Block.
    await page.mouse.move(...(await bildschirm(page, 160, 236)));
    await expect(page.locator('.koordinaten')).toHaveText('X 40  Y 0  Z 20');
    await expect(linie).not.toHaveAttribute('d', /M[^M]+M/);
  });
});

test('der Knopf ⌂ passt die ganze Karte ein, per Maus und Tastatur', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const parameter = (name: string) => new URL(page.url()).searchParams.get(name);
  await expect.poll(() => parameter('at')).not.toBeNull();
  const [at, zoom] = [parameter('at'), parameter('zoom')];
  const ganz = page.getByRole('button', { name: 'Ganze Karte' });

  // Hinein und weggezogen, dann zurück; die Adresse folgt.
  const hinein = page.locator('.leaflet-control-zoom-in');
  await zoomClick(page, hinein);
  await zoomClick(page, hinein);
  const karte = (await page.locator('#map').boundingBox())!;
  const [x, y] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x - 150, y + 80, { steps: 5 });
  await page.mouse.up();
  await expect.poll(() => parameter('at')).not.toBe(at);
  await ganz.click();
  await expect.poll(() => [parameter('at'), parameter('zoom')]).toEqual([at, zoom]);

  // Per Tastatur ebenso.
  await zoomClick(page, hinein);
  await expect.poll(() => parameter('zoom')).not.toBe(zoom);
  await ganz.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => [parameter('at'), parameter('zoom')]).toEqual([at, zoom]);
});

test('die Adresse folgt der Karte, ohne Einträge im Verlauf', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const parameter = (name: string) => new URL(page.url()).searchParams.get(name);
  // Schon die erste Ansicht steht in der Adresse.
  await expect.poll(() => parameter('at')).toMatch(/^-?\d+,-?\d+,-?\d+$/);
  const zoom = Number(parameter('zoom'));
  const verlauf = await page.evaluate(() => history.length);

  // Ein Zug mit der Maus, dann eine Stufe heraus.
  const karte = (await page.locator('#map').boundingBox())!;
  const [x, y] = [karte.x + karte.width / 2, karte.y + karte.height / 2];
  const vorher = parameter('at');
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x - 120, y - 60, { steps: 5 });
  await page.mouse.up();
  await expect.poll(() => parameter('at')).not.toBe(vorher);
  await zoomClick(page, page.locator('.leaflet-control-zoom-out'));
  await expect.poll(() => parameter('zoom')).toBe(String(zoom - 1));
  expect(await page.evaluate(() => history.length)).toBe(verlauf);

  // Neu geladen steht derselbe Block in der Mitte, auf derselben Stufe.
  const at = parameter('at')!.split(',');
  await page.reload();
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await mitte(page)).toBe(`X ${at[0]}  Y ${at[1]}  Z ${at[2]}`);
  expect(parameter('zoom')).toBe(String(zoom - 1));
});

test('zoomen wechselt die Kachelstufe, bis es keine feinere gibt', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const hinein = page.locator('.leaflet-control-zoom-in');
  const heraus = page.locator('.leaflet-control-zoom-out');

  // Stufe für Stufe heraus und wieder hinein: die Kachelstufe folgt.
  await zoomClick(page, heraus);
  await expect.poll(() => tileZooms(page)).toEqual([1]);
  await zoomClick(page, heraus);
  await expect.poll(() => tileZooms(page)).toEqual([0]);
  await expect(heraus).toHaveClass(/leaflet-disabled/);

  await zoomClick(page, hinein);
  await expect.poll(() => tileZooms(page)).toEqual([1]);
  await zoomClick(page, hinein);
  await expect.poll(() => tileZooms(page)).toEqual([2]);

  // Über die feinste gerenderte Stufe hinaus gibt es keine Kacheln mehr.
  // Leaflet vergrössert dann die vorhandenen, statt ins Leere zu laden —
  // genau dafür ist maxNativeZoom da.
  await zoomClick(page, hinein);
  await expect.poll(() => tileStretch(page)).toBe(2);
  await zoomClick(page, hinein);
  await expect.poll(() => tileStretch(page)).toBe(4);
  expect(await tileZooms(page)).toEqual([2]);
  await expect(hinein).toHaveClass(/leaflet-disabled/);
});

test('passt Zoom 0 nicht ins Fenster, geht es weiter heraus', async ({ page }) => {
  // Zoom 0 des Demobaums ist 128 px gross. Einer gewachsenen Welt geht es
  // in jedem Fenster so: ihr Baum behält seine Stufen.
  await page.setViewportSize({ width: 100, height: 100 });
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();

  expect(await tileZooms(page)).toEqual([0]);
  await expect.poll(() => tileStretch(page)).toBe(0.5);
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);
});

test.describe('der Stand der Karte', () => {
  test.use({ timezoneId: 'Europe/Berlin' });

  test('kommt aus Last-Modified von map.json, in Ortszeit', async ({ page }) => {
    await page.route('**/tiles-demo/map.json', async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        headers: { ...response.headers(), 'last-modified': 'Fri, 02 Oct 2026 19:40:00 GMT' },
      });
    });
    await page.goto(DEMO);
    await expect(page.locator('.stand')).toHaveText('Stand: 02.10.2026, 21:40');
  });

  test('fehlt ohne den Header', async ({ page }) => {
    await page.route('**/tiles-demo/map.json', async (route) => {
      const response = await route.fetch();
      await route.fulfill({ status: 200, contentType: 'application/json', body: await response.text() });
    });
    await page.goto(DEMO);
    await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
    await expect(page.locator('.stand')).toHaveCount(0);
  });

  test('zeigt Vite von selbst, wie übliche Webserver', async ({ page }) => {
    await page.goto(DEMO);
    await expect(page.locator('.stand')).toHaveText(/^Stand: \d\d\.\d\d\.\d{4}, \d\d:\d\d$/);
  });
});

test('unten rechts unter dem Stand stehen der Hinweis von Mojang und der Link auf die Lizenzen aus dem Build: NOTICE, LICENSE, die Lizenz der Kartenschrift und jede Abhängigkeit, darin Leaflet', async ({ page }) => {
  await page.goto(DEMO);
  const fuss = page.locator('.lizenzen');
  await expect(fuss).toHaveText(
    'NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT. · Lizenzen',
  );
  const [stand, unten] = [(await page.locator('.stand').boundingBox())!, (await fuss.boundingBox())!];
  expect(unten.y).toBeGreaterThanOrEqual(stand.y + stand.height);
  expect(Math.round(unten.x + unten.width)).toBe(Math.round(stand.x + stand.width));
  // Gut sichtbar, wie die Usage Guidelines von Mojang es verlangen: so gross wie der Stand.
  const schrift = (ort: string) => page.locator(ort).evaluate((e) => getComputedStyle(e).fontSize);
  expect(await schrift('.lizenzen')).toBe(await schrift('.stand'));
  await fuss.getByRole('link', { name: 'Lizenzen' }).click();
  await expect(page).toHaveURL(/\/lizenzen\.txt$/);
  const text = (await page.locator('body').innerText()).replace(/\r\n/g, '\n');
  // Apache-2.0, Abschnitt 4 (a) und (d): Lizenz und NOTICE bei jeder Weitergabe, NOTICE vorn.
  const eigen = (name: string) => readFileSync(new URL(`../../${name}`, import.meta.url), 'utf8').replace(/\r\n/g, '\n').trimEnd();
  expect(text.startsWith(eigen('NOTICE'))).toBe(true);
  expect(text).toContain(eigen('LICENSE'));
  // OFL 1.1, Bedingung 2: Copyright und Lizenz bei jeder Kopie der Schrift.
  expect(text).toContain(eigen('web/src/ebenen/schrift/OFL.txt'));
  const paket = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')) as { dependencies: Record<string, string> };
  for (const name of Object.keys(paket.dependencies)) expect(text).toMatch(new RegExp(`^## ${name} - `, 'm'));
  // BSD-2-Clause, Klausel 2: der Copyright-Hinweis in jeder Weitergabe des Bündels.
  expect(text).toMatch(/^## leaflet - [\d.]+ \(BSD-2-Clause\)$/m);
  expect(text).toMatch(/^Copyright \(c\) [\d-]+, \S+ Agafonkin$/m);
});

test('auf einem Telefon und knapp über 600 px Breite deckt kein Control ein anderes, auch nicht mit Koordinaten', async ({ page }) => {
  await welt(page);
  for (const breite of [360, 601]) {
    await page.setViewportSize({ width: breite, height: 740 });
    await page.goto(DEMO);
    await page.mouse.move(breite / 2, 370);
    await expect(page.locator('.koordinaten')).toContainText('X');
    const kaesten = await page.locator('.leaflet-control').evaluateAll((elemente: HTMLElement[]) =>
      elemente.map((e) => ({ name: e.className, ...e.getBoundingClientRect().toJSON() }) as { name: string } & DOMRect),
    );
    for (const [i, a] of kaesten.entries()) {
      expect(a.right, `${breite}: ${a.name}`).toBeLessThanOrEqual(breite);
      for (const b of kaesten.slice(i + 1)) {
        const deckt = a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
        expect(deckt, `${breite}: ${a.name} über ${b.name}`).toBe(false);
      }
    }
  }
});

test('ohne map.json sagt die Seite warum', async ({ page }) => {
  await page.goto('/?tiles=/gibt-es-nicht');
  await expect(page.locator('.error')).toContainText('map.json');
});

/**
 * Zwei Bäume unter `/tiles-baeume`, wie der Renderer sie anlegt: `trees.json`
 * mit `2x1-se` und `2x1-nw`, je ein `map.json`, Höhen geteilt in `heights/`.
 * Beide zeigen die Kacheln des Demobaums; die Höhen sind eben auf Y 0.
 * `2x1-nw` hat eine Stufe mehr, wie ein Baum mit grösserer Ausdehnung:
 * seine Stufe z ist die Stufe z − 1 des Demobaums. `2x1-se-flat` ist
 * einfarbig mit `scale` 1, zeigt aber die Kacheln von `2x1-se` mit deren
 * Projektion. Zwei weitere Einträge stehen nur in der Liste, für die Namen
 * im Umschalter; keiner ist 2D, darum bleibt es bei der Liste.
 */
async function baeume(page: Page): Promise<void> {
  const trees = [
    ...['se', 'nw'].map((direction) => ({
      path: `2x1-${direction}`,
      camera: '2:1',
      direction,
      look: 'map',
    })),
    { path: '2x1-se-flat', camera: '2:1', direction: 'se', look: 'flat' },
    { path: 'top-se', camera: 'top', direction: 'se', look: 'map' },
    { path: 'north-45-w-cinematic', camera: 'north-45', direction: 'w', look: 'cinematic' },
  ];
  await page.route('**/tiles-baeume/trees.json', (route) => route.fulfill({ json: { trees } }));
  await page.route('**/tiles-baeume/*/**', async (route) => {
    const url = route
      .request()
      .url()
      .replace(/\/tiles-baeume\/2x1-se(-flat)?\//, '/tiles-demo/')
      .replace(/\/tiles-baeume\/2x1-nw\/(\d+)\//, (_, z: string) => `/tiles-demo/${Number(z) - 1}/`);
    await route.fulfill({ response: await route.fetch({ url }) });
  });
  await page.route('**/tiles-baeume/*/map.json', async (route) => {
    const [, direction, flat] = /2x1-(\w+)(-flat)?\//.exec(route.request().url())!;
    const url = route.request().url().replace(/\/tiles-baeume\/2x1-[\w-]+\//, '/tiles-demo/');
    const response = await route.fetch({ url });
    const info = (await response.json()) as { minZoom: number; maxZoom: number };
    const hoehen = { heights: '../heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    const stufen =
      direction === 'nw' ? { minZoom: info.minZoom + 1, maxZoom: info.maxZoom + 1 } : {};
    const einfarbig = flat ? { scale: 1, projection: zweiZuEins(16) } : {};
    await route.fulfill({
      response,
      json: { ...info, ...hoehen, ...stufen, ...einfarbig, camera: '2:1', direction },
    });
  });
  await page.route('**/tiles-baeume/heights/*.bin', (route) =>
    route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) }),
  );
}

/** Die Koordinaten des Blocks in der Mitte der Karte, unter der Maus. */
async function mitte(page: Page): Promise<string | null> {
  const karte = await page.locator('#map').boundingBox();
  if (!karte) throw new Error('Karte nicht zu sehen');
  await page.mouse.move(karte.x + karte.width / 2, karte.y + karte.height / 2);
  await expect(page.locator('.koordinaten')).toHaveText(/^X -?\d/);
  return page.locator('.koordinaten').textContent();
}

/** Die Stufen, aus denen die sichtbaren Kacheln eines Baums unter `/tiles-baeume` stammen. */
async function baumStufen(page: Page): Promise<number[]> {
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  const quellen = await page
    .locator('.kacheln img.leaflet-tile-loaded')
    .evaluateAll((bilder) => bilder.map((bild) => (bild as HTMLImageElement).src));
  const stufen = quellen.map((src) => Number(/\/tiles-baeume\/[\w-]+\/(\d+)\//.exec(src)?.[1]));
  return [...new Set(stufen)];
}

test('ohne 2D ist der Umschalter eine Liste; sie öffnet den anderen Baum mit demselben Block in der Mitte', async ({
  page,
}) => {
  await baeume(page);
  await page.goto('/?tiles=/tiles-baeume');
  // Ohne tree der erste Baum; Norden zeigt aus Südosten nach rechts oben.
  const auswahl = page.locator('select.baeume');
  await expect(auswahl).toHaveValue('2x1-se');
  await expect(auswahl.locator('option')).toHaveText([
    '2:1 aus Südost',
    '2:1 aus Nordwest',
    '2:1 aus Südost · Einfarbig',
    'Von oben aus Südost',
    'Schräg, Osten oben · Cinematic',
  ]);
  await expect(page.locator('.kompass')).toHaveAttribute('style', /rotate\(63\.4deg\)/);
  const [stufe] = await baumStufen(page);
  const vorher = await mitte(page);

  await auswahl.selectOption('2x1-nw');
  await page.waitForURL(/tree=2x1-nw/);
  const adresse = new URL(page.url()).searchParams;
  const at = adresse.get('at')!.split(',');
  expect(vorher).toBe(`X ${at[0]}  Y ${at[1]}  Z ${at[2]}`);
  // `zoom` zählt ab der feinsten Stufe: Der neue Baum hat eine Stufe mehr
  // und zeigt dieselbe Vergrösserung eine Stufe höher.
  expect(adresse.get('zoom')).toBe(String(stufe! - 2));
  expect(await baumStufen(page)).toEqual([stufe! + 1]);
  await expect(page.locator('select.baeume')).toHaveValue('2x1-nw');
  await expect(page.locator('.kompass')).toHaveAttribute('style', /rotate\(-116\.6deg\)/);
  expect(await mitte(page)).toBe(vorher);
});

/** Wie breit eine Kachel auf dem Schirm ist; über der feinsten Stufe vergrössert Leaflet sie. */
async function kachelBreite(page: Page): Promise<number> {
  const kachel = page.locator('.kacheln img.leaflet-tile-loaded').first();
  await expect(kachel).toBeVisible();
  return (await kachel.boundingBox())!.width;
}

test('beim Wechsel bleibt ein Block gleich gross, auch zu einem einfarbigen Baum mit scale 1; dort reicht der Zoom, bis ein Block 64 Pixel breit ist', async ({
  page,
}) => {
  await baeume(page);
  // scale 16, eine Stufe über der feinsten: ein Block 32 Pixel, eine Kachel 512.
  await page.goto('/?tiles=/tiles-baeume&tree=2x1-se&at=20,0,-20&zoom=1');
  expect(await kachelBreite(page)).toBe(512);
  await page.locator('select.baeume').selectOption('2x1-se-flat');
  await page.waitForURL(/tree=2x1-se-flat/);
  // scale 1: Für 32 Pixel je Block fünf Stufen über der feinsten, eine Kachel 256 · 2^5.
  expect(new URL(page.url()).searchParams.get('zoom')).toBe('5');
  expect(await kachelBreite(page)).toBe(256 * 32);
  // Und zurück.
  await page.locator('select.baeume').selectOption('2x1-se');
  await page.waitForURL(/tree=2x1-se&/);
  expect(new URL(page.url()).searchParams.get('zoom')).toBe('1');
  expect(await kachelBreite(page)).toBe(512);
  // Die Grenzen: bei scale 1 sechs Stufen über der feinsten, 64 Pixel je Block; bei scale 16 zwei.
  await page.goto('/?tiles=/tiles-baeume&tree=2x1-se-flat&at=20,0,-20&zoom=7');
  expect(await kachelBreite(page)).toBe(256 * 64);
  await page.goto('/?tiles=/tiles-baeume&tree=2x1-se&at=20,0,-20&zoom=3');
  expect(await kachelBreite(page)).toBe(256 * 4);
});

/**
 * Unter `/tiles-oben` ein Baum aus `top-north` mit scale 4 und ein Baum 2:1
 * mit scale 16, beide mit den Kacheln des Demobaums und ebenen Höhen; davor
 * ein einfarbiger von oben nur in der Liste.
 */
async function zweiUndDrei(page: Page): Promise<void> {
  const trees = [
    { path: 'top-north-s-flat', camera: 'top-north', direction: 's', look: 'flat' },
    { path: 'top-north-s', camera: 'top-north', direction: 's', look: 'map' },
    { path: '2x1-se', camera: '2:1', direction: 'se', look: 'map' },
  ];
  await page.route('**/tiles-oben/trees.json', (route) => route.fulfill({ json: { trees } }));
  await page.route(/\/tiles-oben\/(top-north-s|2x1-se)\/\d+\/.*\.webp$/, async (route) => {
    const url = route.request().url().replace(/\/tiles-oben\/[\w-]+\//, '/tiles-demo/');
    await route.fulfill({ response: await route.fetch({ url }) });
  });
  await page.route(/\/tiles-oben\/(top-north-s|2x1-se)\/map\.json$/, async (route) => {
    const oben = route.request().url().includes('/top-north-s/');
    const response = await route.fetch({ url: route.request().url().replace(/\/tiles-oben\/[\w-]+\//, '/tiles-demo/') });
    const info = (await response.json()) as object;
    const hoehen = { heights: '../heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    const art = oben ? { scale: 4, camera: 'top-north', direction: 's', projection: { azimuth: 'north', u: 4, v: 4, y: 0 } } : { camera: '2:1', direction: 'se' };
    await route.fulfill({ response, json: { ...info, ...hoehen, ...art } });
  });
  await page.route('**/tiles-oben/heights/*.bin', (route) =>
    route.fulfill({ body: deflateSync(Buffer.from(new Int16Array(128 * 128).buffer)) }),
  );
}

test('mit 2D und 3D ein Knopf statt der Liste: er wechselt an denselben Block, ein Block bleibt gleich gross; ein alter Link mit px zeigt die volle Ansicht', async ({
  page,
}) => {
  await zweiUndDrei(page);
  await page.goto('/?tiles=/tiles-oben&tree=top-north-s&at=40,0,40&zoom=0');
  const knopf = page.locator('button.zwei-drei');
  await expect(knopf).toHaveText('3D');
  await expect(knopf).toHaveAttribute('title', '2:1 aus Südost');
  await expect(page.locator('select.baeume')).toHaveCount(0);
  const vorher = await mitte(page);
  await knopf.click();
  await page.waitForURL(/tree=2x1-se/);
  // scale 4 gegen 16: Für dieselbe Grösse eines Blocks zwei Stufen unter der feinsten.
  expect(new URL(page.url()).searchParams.get('zoom')).toBe('-2');
  await expect(page.locator('button.zwei-drei')).toHaveText('2D');
  await expect(page.locator('button.zwei-drei')).toHaveAttribute('title', 'Von oben, Norden oben');
  expect(await mitte(page)).toBe(vorher);
  await page.locator('button.zwei-drei').click();
  await page.waitForURL(/tree=top-north-s&/);
  expect(new URL(page.url()).searchParams.get('zoom')).toBe('0');
  await expect(page.locator('button.zwei-drei')).toHaveText('3D');
  // Ansichten mit 1 und 2 px je Block gibt es nicht mehr: scale 4 auf Stufe 2, dem Zoom 0, eine Kachel 256 Pixel.
  await page.goto('/?tiles=/tiles-oben&tree=top-north-s&px=1&at=40,0,40&zoom=0');
  expect(await kachelBreite(page)).toBe(256);
});

test('ein Baum allein braucht keinen Umschalter, nur den Kompass', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('.kacheln img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('select.baeume')).toHaveCount(0);
  await expect(page.locator('.kompass')).toHaveAttribute('style', /rotate\(63\.4deg\)/);
});
