// Das Bild docs/bilder/regionen-boden.webp: eine Region über Wald und Ufer der
// Testwelt auf dem Boden ohne Laub, mit Wand und Nebel, schräg in 2:1 und von
// oben; dazu eine Linie unter Baumkronen und eine hinter einem Hang. Die
// Kacheln samt ground rendert der Renderer vorher in <wurzel>, siehe SKILL.md.
// Aus web/, nach `npm run build`:
//   node ../skills/doku-bilder-rendern/regionen-bild.mjs <wurzel> [<bild>]
import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { fileURLToPath } from 'node:url';

// Playwright aus web/node_modules: Das Skript liegt ausserhalb von web/.
const { chromium } = createRequire(`${process.cwd()}/`)('playwright');

const repo = fileURLToPath(new URL('../../', import.meta.url));
const [wurzel, bild = `${repo}docs/bilder/regionen-boden.webp`] = process.argv.slice(2);
if (!wurzel) throw new Error('Aufruf: node regionen-bild.mjs <wurzel mit trees.json und ground> [<bild>]');

// In Blöcken der Testwelt: Wald im Nordwesten, Ufer im Osten, ein Hang im Südosten.
const GEBIET = {
  id: 'gebiet',
  type: 'region',
  name: 'Dorfgrund',
  polygons: [{ outer: [[-478, 488], [-446, 478], [-412, 492], [-408, 528], [-430, 548], [-468, 544]] }],
  fill: '#40E53F26',
  stroke: { color: '#40E53FEE', width: 2 },
};
const IM_WALD = { id: 'waldweg', type: 'line', points: [[-508, 428], [-492, 452], [-470, 470], [-452, 452]], stroke: { color: '#D9443A', width: 3 } };
const AM_HANG = { id: 'hangweg', type: 'line', points: [[-396, 652], [-370, 660], [-344, 666], [-316, 672]], stroke: { color: '#D9443A', width: 3 } };
const ANSICHTEN = [
  ['schräg, 2:1', '2x1-se', [-443, 66, 513], -1],
  ['von oben, unverändert', 'top-north-s', [-443, 66, 513], -2],
  ['unter Kronen: ganz zu sehen', '2x1-se', [-480, 70, 450], 0],
  ['hinter dem Hang: gestrichelt', '2x1-se', [-346, 74, 664], -1],
];

const datei = JSON.stringify({ id: 'beispiel:staedte', objects: [GEBIET, IM_WALD, AM_HANG] });
const liste = JSON.stringify({ layers: [{ id: 'beispiel:staedte', name: { de: 'Städte' }, visible: true, order: 100, version: 'bild' }] });

// Ein freier Port: Bei einem festen, schon belegten käme die Antwort von einem fremden Server.
const PORT = await new Promise((fertig) => {
  const probe = createServer().listen(0, '127.0.0.1', () => {
    const { port } = probe.address();
    probe.close(() => fertig(port));
  });
});
// Ohne Shell, damit kill() vite trifft und nicht nur die Shell.
const server = spawn(process.execPath, ['node_modules/vite/bin/vite.js', 'preview', '--port', String(PORT), '--strictPort', '--host', '127.0.0.1'], { stdio: 'ignore' });
try {
  for (let i = 0; ; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${PORT}/`)).ok) break;
    } catch {
      if (i > 100) throw new Error('vite preview startet nicht');
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  const browser = await chromium.launch();
  const bilder = [];
  for (const [titel, baum, mitte, zoom] of ANSICHTEN) {
    const page = await browser.newPage({ viewport: { width: 480, height: 360 }, deviceScaleFactor: 1 });
    // Die Ebene aus dem Skript, alles andere aus der Wurzel.
    await page.route('**/tiles-ebenen/**', (route) => {
      const pfad = decodeURIComponent(new URL(route.request().url()).pathname.replace(/^\/tiles-ebenen\//, ''));
      if (pfad === 'layers.json') return route.fulfill({ body: liste, contentType: 'application/json' });
      if (pfad.startsWith('layers/beispiel/staedte.json')) return route.fulfill({ body: datei, contentType: 'application/json' });
      try {
        return route.fulfill({ body: readFileSync(`${wurzel}/${pfad}`), contentType: pfad.endsWith('.json') ? 'application/json' : 'application/octet-stream' });
      } catch {
        return route.fulfill({ status: 404 });
      }
    });
    await page.goto(`http://127.0.0.1:${PORT}/?tiles=/tiles-ebenen&tree=${baum}&at=${mitte.join(',')}&zoom=${zoom}`);
    await page.locator('path[stroke="#D9443A"]').first().waitFor({ state: 'attached' });
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
    bilder.push({ titel, bild: (await page.screenshot()).toString('base64') });
    await page.close();
  }
  // Zwei mal zwei, mit einer Zeile über jedem Bild.
  const blatt = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await blatt.setContent(`<body style="margin:0;font:13px system-ui,sans-serif;background:#fff">
    <div style="display:inline-grid;grid-template-columns:auto auto;gap:12px;padding:8px">${bilder
      .map((b) => `<figure style="margin:0"><figcaption style="margin:0 0 4px">${b.titel}</figcaption><img src="data:image/png;base64,${b.bild}" width="480" height="360" style="border-radius:4px"></figure>`)
      .join('')}</div></body>`);
  // Als WebP mit Qualität 90: ein Bildschirmfoto, verlustfrei wäre es viermal so gross.
  const png = (await blatt.locator('div').screenshot()).toString('base64');
  const webp = await blatt.evaluate(async (png) => {
    const b = new Image();
    b.src = `data:image/png;base64,${png}`;
    await b.decode();
    const leinwand = Object.assign(document.createElement('canvas'), { width: b.width, height: b.height });
    leinwand.getContext('2d').drawImage(b, 0, 0);
    return leinwand.toDataURL('image/webp', 0.9).split(',')[1];
  }, png);
  writeFileSync(bild, Buffer.from(webp, 'base64'));
  await browser.close();
} finally {
  server.kill();
}
