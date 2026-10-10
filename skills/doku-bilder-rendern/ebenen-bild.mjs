// Das Bild docs/bilder/ebenen-testwelt.webp: eine Ebene mit Region, Kreis,
// Linie, Kartenschrift, Banner und Nadel auf einem Ausschnitt der Testwelt,
// schräg in 2:1 und von oben nebeneinander. Die Kacheln rendert der Renderer
// vorher in <wurzel>, siehe SKILL.md; dieses Skript legt die Ebene dazu, lässt
// den Renderer ihre Banner zeichnen, zeigt den Build aus web/dist mit
// `vite preview` und nimmt beide Ansichten auf.
// Aus web/, nach `npm run build`:
//   node ../skills/doku-bilder-rendern/ebenen-bild.mjs <wurzel> <renderer> [<ordner mit Assets>]
import { spawn, spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { fileURLToPath } from 'node:url';

// Playwright aus web/node_modules: Das Skript liegt ausserhalb von web/.
const { chromium } = createRequire(`${process.cwd()}/`)('playwright');

const repo = fileURLToPath(new URL('../../', import.meta.url));
const [wurzel, renderer, ordner = repo] = process.argv.slice(2);
if (!wurzel || !renderer) throw new Error('Aufruf: node ebenen-bild.mjs <wurzel mit trees.json> <renderer> [<ordner mit Assets>]');

// Die Ebene, in Blöcken der Testwelt um das Dorf.
const MITTE = [-420, 66, 520];
const ebene = {
  id: 'beispiel:staedte',
  name: { de: 'Städte' },
  designs: {
    hafen: {
      base: 'light_blue',
      layers: [
        { pattern: 'minecraft:stripe_bottom', color: 'white' },
        { pattern: 'minecraft:circle', color: 'yellow' },
        { pattern: 'minecraft:border', color: 'blue' },
      ],
    },
  },
  objects: [
    { id: 'gebiet', type: 'region', name: 'Dorfgrund', polygons: [{ outer: [[-474, 486], [-432, 486], [-432, 540], [-474, 540]] }], fill: '#40E53F26', stroke: { color: '#40E53FDD', width: 2 } },
    { id: 'nah', type: 'circle', center: [-416, 514], radius: 26, stroke: { color: '#FFFFFFAA', width: 2, style: 'dashed' } },
    { id: 'route', type: 'line', points: [[-410, 540], [-388, 556], [-360, 560]], stroke: { color: '#D9443A', width: 3, style: 'dashed' } },
    { id: 'meer', type: 'label', text: 'Westmeer', path: [[-398, 498], [-374, 504]], size: 2, spacing: 0.2, color: '#2B3A55', outline: { color: '#F2E8D0CC', width: 3 } },
    {
      id: 'stadt',
      type: 'banner',
      at: [-416.5, 514.5],
      design: 'hafen',
      capital: true,
      name: 'Hafenstadt',
      panel: { blocks: [{ type: 'title', text: '✪ Hafenstadt', color: '#40E53F' }, { type: 'lines', lines: ['Nation: keine', 'Einwohner: 12'] }] },
    },
    { id: 'wegpunkt', type: 'pin', at: [-440.5, 548.5], name: 'Wegpunkt', size: 'large', color: '#D9443A' },
  ],
};
mkdirSync(`${wurzel}/layers/beispiel`, { recursive: true });
writeFileSync(`${wurzel}/layers.json`, JSON.stringify({ layers: [{ id: ebene.id, name: ebene.name, visible: true, order: 100, version: 'bild' }] }));
writeFileSync(`${wurzel}/layers/beispiel/staedte.json`, JSON.stringify(ebene));

// Die Sprites der Banner, wie das Plugin sie zeichnen lässt: je Baum, der nicht von oben schaut, ein Satz, dazu `oben`.
const banner = spawnSync(
  renderer,
  ['--banners', `${wurzel}/layers/beispiel/staedte.json`, '--out', `${wurzel}/layers`, '--tiles', wurzel,
    '--assets', `${ordner}/vanilla-assets`, '--assets', `${ordner}/assets`, '--data', `${ordner}/vanilla-data`],
  { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] },
);
const meldung = banner.status === 0 ? JSON.parse(banner.stdout.trim().split('\n').at(-1)) : undefined;
if (!meldung || meldung.failed.length) throw new Error(`--banners ging nicht: ${banner.stdout}`);

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
  const page = await browser.newPage({ viewport: { width: 640, height: 480 }, deviceScaleFactor: 1 });
  await page.route('**/tiles-ebenen/**', (route) => {
    const pfad = decodeURIComponent(new URL(route.request().url()).pathname.replace(/^\/tiles-ebenen\//, ''));
    try {
      const typ = pfad.endsWith('.json') ? 'application/json' : pfad.endsWith('.png') ? 'image/png' : pfad.endsWith('.webp') ? 'image/webp' : 'application/octet-stream';
      return route.fulfill({ body: readFileSync(`${wurzel}/${pfad}`), contentType: typ });
    } catch {
      return route.fulfill({ status: 404 });
    }
  });
  const bilder = [];
  for (const [baum, titel, zoom] of [['2x1-se', 'schräg, 2:1', -1], ['top-north-s', 'von oben', -2]]) {
    await page.goto(`http://127.0.0.1:${PORT}/?tiles=/tiles-ebenen&tree=${baum}&at=${MITTE.join(',')}&zoom=${zoom}`);
    // Den Bogen gibt es nur beim Sprite; ohne `image` übergeht die Ansicht ein Banner ohne Sprite.
    await page.locator('.nadel-bogen').waitFor({ state: 'attached' });
    await page.evaluate(() => document.fonts.ready);
    await page.locator('.nadel-icon[title="Hafenstadt"]').hover();
    await page.locator('.tafel .tafel-titel').waitFor();
    await page.waitForTimeout(800);
    bilder.push({ titel, bild: (await page.screenshot()).toString('base64') });
  }
  // Beide nebeneinander, mit einer Zeile darüber.
  const blatt = await browser.newPage({ viewport: { width: 1320, height: 540 } });
  await blatt.setContent(`<body style="margin:0;font:13px system-ui,sans-serif;background:#fff">
    <div style="display:inline-flex;gap:12px;padding:8px">${bilder
      .map((b) => `<figure style="margin:0"><figcaption style="margin:0 0 4px">${b.titel}</figcaption><img src="data:image/png;base64,${b.bild}" width="640" height="480" style="border-radius:4px"></figure>`)
      .join('')}</div></body>`);
  // Als WebP mit Qualität 90: ein Bildschirmfoto, verlustfrei wäre es viermal so gross.
  const png = (await blatt.locator('div').screenshot()).toString('base64');
  const webp = await blatt.evaluate(async (png) => {
    const bild = new Image();
    bild.src = `data:image/png;base64,${png}`;
    await bild.decode();
    const leinwand = Object.assign(document.createElement('canvas'), { width: bild.width, height: bild.height });
    leinwand.getContext('2d').drawImage(bild, 0, 0);
    return leinwand.toDataURL('image/webp', 0.9).split(',')[1];
  }, png);
  writeFileSync(`${repo}docs/bilder/ebenen-testwelt.webp`, Buffer.from(webp, 'base64'));
  await browser.close();
} finally {
  server.kill();
}
