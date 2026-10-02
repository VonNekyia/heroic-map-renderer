import { expect, test, type Locator, type Page } from '@playwright/test';
import { deflateSync } from 'node:zlib';

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
  const kachel = await page.locator('img[src$="/2/0/0.webp"]').boundingBox();
  if (!kachel) throw new Error('Kachel 2/0/0 nicht zu sehen');
  return [kachel.x + u + 0.5, kachel.y + v + 0.5];
}

/** Die Zoomstufen, aus denen die sichtbaren Kacheln stammen. */
async function tileZooms(page: Page): Promise<number[]> {
  const quellen = await page.locator('img.leaflet-tile-loaded').evaluateAll((bilder) =>
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
    .locator('.leaflet-tile-container')
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

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await tileZooms(page)).not.toEqual([]);
  expect(fehler).toEqual([]);
  // Ohne Höhen in map.json keine Koordinaten.
  await expect(page.locator('.koordinaten')).toHaveCount(0);
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
  expect(antwort?.headers()['content-security-policy']).toContain("default-src 'self'");

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  // Koordinaten holen Höhen und entpacken sie; auch das muss erlaubt sein.
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
  expect(await page.evaluate(() => (window as unknown as { verletzt: string[] }).verletzt)).toEqual(
    [],
  );
});

test('die Koordinaten rechnen mit projection aus map.json', async ({ page }) => {
  // Eine Draufsicht über den Kacheln von 2:1: falsch fürs Auge, aber so
  // zeigt sich, dass projection gilt und nicht scale.
  await welt(page, { camera: 'top', projection: { azimuth: 'diagonal', u: 8, v: 8, y: 0 } });
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(page.locator('.koordinaten')).toHaveText('X 27  Y 0  Z -23');
});

test('aus Nordwesten zeigt die Anzeige Weltkoordinaten', async ({ page }) => {
  // Im Blick aus Nordwesten liegt (35, 5, -15) dort, wo aus Südosten
  // derselbe Pixel ist; in der Welt ist das (-36, 5, 14). Dort steht die
  // Säule: Zelle der Spalten -36 bis -33 und 12 bis 15, Region -1.0.
  await welt(page, { direction: 'nw' }, ['-1.0.bin', 3 * 128 + 119]);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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

    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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

  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('.koordinaten')).toHaveCount(0);
});

test('die Maus zeigt Koordinaten des Blocks darunter, ohne Umriss', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  await expect(anzeige).toHaveText('X –  Y –  Z –');

  // Die Mitte der Oberseite von (35, 5, -15) und von (40, 0, 20).
  await page.mouse.move(...(await bildschirm(page, 400, 36)));
  await expect(anzeige).toHaveText('X 35  Y 5  Z -15');
  await page.mouse.move(...(await bildschirm(page, 160, 236)));
  await expect(anzeige).toHaveText('X 40  Y 0  Z 20');
  // Den Block zeigt der Mauszeiger; ein Umriss hat keine Linie.
  await expect(page.locator('.leaflet-overlay-pane path')).not.toHaveAttribute('d', /M[^M]+M/);
});

test('das Kopiersymbol kopiert /tp, ein Klick hält den Block dafür fest', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  const anzeige = page.locator('.koordinaten');
  const kopieren = page.getByRole('button', { name: '/tp kopieren' });
  const meldung = page.getByRole('status');
  const linie = page.locator('.leaflet-overlay-pane path');

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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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

test('Escape oder ein Klick daneben bricht den Eintrag ab', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
    const knopf = (await page.getByRole('button', { name: '/tp kopieren' }).boundingBox())!;
    await page.touchscreen.tap(knopf.x + knopf.width / 2, knopf.y + knopf.height / 2);
    await expect(page.getByRole('status')).toHaveText('Kopiert: /tp 35 6 -15');
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('/tp 35 6 -15');
  });

  test('Tippen auf Z, ein negativer Wert und Enter springt hin', async ({ page }) => {
    await welt(page);
    await page.goto(DEMO);
    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
    await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
    await page.touchscreen.tap(...(await bildschirm(page, 400, 36)));
    await expect(page.locator('.koordinaten')).toHaveText('X 35  Y 5  Z -15');
    // Ohne Zeiger zeigt der Umriss den Block: Sechseck und die drei Kanten
    // der vorderen Ecke.
    const linie = page.locator('.leaflet-overlay-pane path');
    await expect(linie).toHaveAttribute('d', /^M[^M]+M[^M]+M[^M]+$/);

    // Kommt danach die Maus, zeigt wieder ihr Zeiger den Block.
    await page.mouse.move(...(await bildschirm(page, 160, 236)));
    await expect(page.locator('.koordinaten')).toHaveText('X 40  Y 0  Z 20');
    await expect(linie).not.toHaveAttribute('d', /M[^M]+M/);
  });
});

test('die Adresse folgt der Karte, ohne Einträge im Verlauf', async ({ page }) => {
  await welt(page);
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  expect(await mitte(page)).toBe(`X ${at[0]}  Y ${at[1]}  Z ${at[2]}`);
  expect(parameter('zoom')).toBe(String(zoom - 1));
});

test('zoomen wechselt die Kachelstufe, bis es keine feinere gibt', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();

  expect(await tileZooms(page)).toEqual([0]);
  await expect.poll(() => tileStretch(page)).toBe(0.5);
  await expect(page.locator('.leaflet-control-zoom-out')).toHaveClass(/leaflet-disabled/);
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
 * seine Stufe z ist die Stufe z − 1 des Demobaums. Zwei weitere Einträge
 * stehen nur in der Liste, für die Namen im Umschalter.
 */
async function baeume(page: Page): Promise<void> {
  const trees = [
    ...['se', 'nw'].map((direction) => ({
      path: `2x1-${direction}`,
      camera: '2:1',
      direction,
      look: 'map',
    })),
    { path: 'top-se', camera: 'top', direction: 'se', look: 'map' },
    { path: 'top-north-w-cinematic', camera: 'top-north', direction: 'w', look: 'cinematic' },
  ];
  await page.route('**/tiles-baeume/trees.json', (route) => route.fulfill({ json: { trees } }));
  await page.route('**/tiles-baeume/*/**', async (route) => {
    const url = route
      .request()
      .url()
      .replace(/\/tiles-baeume\/2x1-se\//, '/tiles-demo/')
      .replace(/\/tiles-baeume\/2x1-nw\/(\d+)\//, (_, z: string) => `/tiles-demo/${Number(z) - 1}/`);
    await route.fulfill({ response: await route.fetch({ url }) });
  });
  await page.route('**/tiles-baeume/*/map.json', async (route) => {
    const direction = /2x1-(\w+)\//.exec(route.request().url())![1]!;
    const url = route.request().url().replace(/\/tiles-baeume\/2x1-\w+\//, '/tiles-demo/');
    const response = await route.fetch({ url });
    const info = (await response.json()) as { minZoom: number; maxZoom: number };
    const hoehen = { heights: '../heights/{x}.{z}.bin', heightsCell: 4, minY: -64, maxY: 319 };
    const stufen =
      direction === 'nw' ? { minZoom: info.minZoom + 1, maxZoom: info.maxZoom + 1 } : {};
    await route.fulfill({
      response,
      json: { ...info, ...hoehen, ...stufen, camera: '2:1', direction },
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
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  const quellen = await page
    .locator('img.leaflet-tile-loaded')
    .evaluateAll((bilder) => bilder.map((bild) => (bild as HTMLImageElement).src));
  const stufen = quellen.map((src) => Number(/\/tiles-baeume\/[\w-]+\/(\d+)\//.exec(src)?.[1]));
  return [...new Set(stufen)];
}

test('der Umschalter öffnet den anderen Baum mit demselben Block in der Mitte', async ({
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
    'Von oben aus Südost',
    'Von oben, Osten oben · Cinematic',
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

test('ein Baum allein braucht keinen Umschalter, nur den Kompass', async ({ page }) => {
  await page.goto(DEMO);
  await expect(page.locator('img.leaflet-tile-loaded').first()).toBeVisible();
  await expect(page.locator('select.baeume')).toHaveCount(0);
  await expect(page.locator('.kompass')).toHaveAttribute('style', /rotate\(63\.4deg\)/);
});
