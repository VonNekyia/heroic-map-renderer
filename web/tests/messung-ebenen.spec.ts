/**
 * Keine Prüfung, eine Messung: Punkte in den SVG-Pfaden, JS-Heap nach der
 * Speicherbereinigung und lange Aufgaben beim Verschieben, mit den Ebenen
 * eines Servers, weit und nah, mit und ohne eine Ebene grosser Kreise.
 * Die Daten liegen nicht im Repository; `KOPIE` nennt ihren Ordner mit
 * `layers.json`, den Dateien der Ebenen, `map-4x3.json` und `nah.txt`.
 * Siehe docs/messungen/2026-10-19-ebenen-im-weiten-blick.md.
 */
import { expect, test, type CDPSession, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const KOPIE = process.env.KOPIE ?? '';
test.skip(!KOPIE, 'nur mit KOPIE, siehe oben');
const lies = (name: string) => JSON.parse(readFileSync(`${KOPIE}/${name}`, 'utf8')) as Record<string, unknown>;

async function kopie(page: Page, kreiseAn: boolean): Promise<void> {
  const liste = lies('layers.json') as { layers: { id: string; visible: boolean }[] };
  const layers = liste.layers.map((e) => (e.id.endsWith(':kreise') ? { ...e, visible: kreiseAn } : e));
  const info = { ...lies('map-4x3.json'), tiles: '{z}/{x}/{y}.webp' };
  await page.route('**/tiles-kopie/map.json', (r) => r.fulfill({ json: info }));
  await page.route('**/tiles-kopie/layers.json', (r) => r.fulfill({ json: { layers } }));
  // Die Datei einer Ebene nach ihrem Namen, ohne Datei leer.
  await page.route('**/tiles-kopie/layers/*/*.json', (r) => {
    const name = /\/(\w+)\.json$/.exec(r.request().url())![1]!;
    try {
      return r.fulfill({ json: lies(`${name}.json`) });
    } catch {
      return r.fulfill({ json: { objects: [] } });
    }
  });
  // Höhen überall eben auf 64, ground fehlt: wie ein Baum mit Höhen, ohne Daten der Welt.
  const flach = deflateSync(Buffer.from(new Int16Array(128 * 128).fill(64).buffer));
  await page.route('**/heights/*.bin', (r) => r.fulfill({ body: flach }));
  await page.route('**/ground/*.bin', (r) => r.fulfill({ status: 404 }));
  await page.route('**/tiles-kopie/*/*/*.webp', (r) => r.fulfill({ status: 404 }));
}

async function heap(cdp: CDPSession, schnell: boolean): Promise<{ mb: number; oben: string[] }> {
  await cdp.send('HeapProfiler.collectGarbage');
  if (schnell) {
    const { usedSize } = (await cdp.send('Runtime.getHeapUsage')) as { usedSize: number };
    return { mb: Math.round(usedSize / 1e5) / 10, oben: [] };
  }
  const teile: string[] = [];
  const nimm = (e: { chunk: string }) => teile.push(e.chunk);
  cdp.on('HeapProfiler.addHeapSnapshotChunk', nimm);
  await cdp.send('HeapProfiler.takeHeapSnapshot', { reportProgress: false });
  cdp.off('HeapProfiler.addHeapSnapshotChunk', nimm);
  const s = JSON.parse(teile.join('')) as { snapshot: { meta: { node_fields: string[] } }; nodes: number[]; strings: string[] };
  const f = s.snapshot.meta.node_fields;
  const [iName, iSelf, n] = [f.indexOf('name'), f.indexOf('self_size'), f.length];
  const je = new Map<string, number>();
  let summe = 0;
  for (let i = 0; i < s.nodes.length; i += n) {
    const name = s.strings[s.nodes[i + iName]!]!;
    const g = s.nodes[i + iSelf]!;
    summe += g;
    je.set(name, (je.get(name) ?? 0) + g);
  }
  const oben = [...je].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([k, v]) => `${(v / 1e6).toFixed(1)} MB ${k.slice(0, 40)}`);
  return { mb: Math.round(summe / 1e5) / 10, oben };
}

/** Punkte in allen SVG-Pfaden der Ebenen und die Zahl der Pfade. */
const punkte = (page: Page) =>
  page.evaluate(() => {
    const pfade = [...document.querySelectorAll('.leaflet-pane svg path')];
    const art = (p: Element) => (p.getAttribute('stroke') !== 'none' && p.getAttribute('stroke') ? 'strich' : p.getAttribute('fill-rule') === 'nonzero' ? 'wand' : 'flaeche');
    const je: Record<string, number> = {};
    for (const p of pfade) je[art(p)] = (je[art(p)] ?? 0) + (p.getAttribute('d')?.match(/[ML]/g)?.length ?? 0);
    return { pfade: pfade.length, punkte: pfade.reduce((s, p) => s + (p.getAttribute('d')?.match(/[ML]/g)?.length ?? 0), 0), je };
  });

/** Summe der langen Aufgaben während eines Verschiebens um 300 px. */
async function verschieben(page: Page): Promise<number> {
  await page.evaluate(() => {
    (window as unknown as { lang: number }).lang = 0;
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) (window as unknown as { lang: number }).lang += e.duration;
    }).observe({ type: 'longtask' });
  });
  const b = page.viewportSize()!;
  for (let i = 0; i < 3; i++) {
    await page.mouse.move(b.width / 2, b.height / 2);
    await page.mouse.down();
    await page.mouse.move(b.width / 2 + 300, b.height / 2, { steps: 10 });
    await page.mouse.up();
    await page.waitForTimeout(400);
  }
  return page.evaluate(() => Math.round((window as unknown as { lang: number }).lang));
}

const [nx, nz] = KOPIE ? readFileSync(`${KOPIE}/nah.txt`, 'utf8').trim().split(/\s+/).map(Number) : [0, 0];
const ORTE = { weit: 'at=42,62,42&zoom=-8', nah: `at=${nx},64,${nz}&zoom=0` };
for (const [b, h] of [
  [1278, 1214],
] as const) {
  for (const [ort, suche] of Object.entries(ORTE)) for (const kreise of [false, true]) {
    test(`${b}x${h} ${ort} Kreise ${kreise ? 'an' : 'aus'}`, async ({ page }) => {
      test.setTimeout(900_000);
      await page.setViewportSize({ width: b, height: h });
      await kopie(page, kreise);
      const cdp = await page.context().newCDPSession(page);
      const start = Date.now();
      await page.goto(`/?tiles=/tiles-kopie&${suche}`);
      await expect(page.locator('.ebenen')).toBeVisible();
      // Bis die Ebenen stehen: die Zahl der Pfade ändert sich 2 s lang nicht mehr.
      let vorher = -1;
      for (;;) {
        const { pfade } = await punkte(page);
        if (pfade === vorher && pfade > 0) break;
        vorher = pfade;
        await page.waitForTimeout(2000);
      }
      const laden = Date.now() - start;
      const p = await punkte(page);
      const h1 = await heap(cdp, true);
      const lang = await verschieben(page);
      for (let i = 0; i < 10; i++) {
        await page.locator(i % 2 ? '.leaflet-control-zoom-out' : '.leaflet-control-zoom-in').click();
        await page.waitForTimeout(300);
      }
      const h2 = await heap(cdp, true);
      console.log(JSON.stringify({ fenster: `${b}x${h}`, ort, kreise, laden_ms: laden, ...p, heap_mb: h1.mb, lange_aufgaben_ms: lang, heap_nach_zoom_mb: h2.mb, oben: h1.oben }, null, 1));
    });
  }
}
