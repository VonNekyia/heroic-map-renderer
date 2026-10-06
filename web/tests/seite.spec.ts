import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** Baut die Seite mit diesen Variablen in ein eigenes Verzeichnis. */
function baue(env: Record<string, string>): { kopf: string; robots: string; vorlage: string; robotsVorlage: string } {
  const dir = mkdtempSync(join(tmpdir(), 'seite-'));
  try {
    const vite = join('node_modules', 'vite', 'bin', 'vite.js');
    execFileSync(
      process.execPath,
      [vite, 'build', '--outDir', dir, '--emptyOutDir', '--logLevel', 'error'],
      { env: { ...process.env, ...env }, stdio: 'pipe' },
    );
    return {
      kopf: readFileSync(join(dir, 'index.html'), 'utf8'),
      robots: readFileSync(join(dir, 'robots.txt'), 'utf8'),
      vorlage: readFileSync(join(dir, 'seite.html'), 'utf8'),
      robotsVorlage: readFileSync(join(dir, 'robots.vorlage.txt'), 'utf8'),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('ohne SITE_URL: Titel und Beschreibung, keine Adresse', async ({ page, request }) => {
  await page.goto('/');
  await expect(page).toHaveTitle('Heroic Map Renderer');
  await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /\S/);
  await expect(page.locator('h1')).toHaveText('Heroic Map Renderer');
  await expect(page.locator('link[rel="canonical"]')).toHaveCount(0);
  await expect(page.locator('meta[property="og:url"]')).toHaveCount(0);
  await expect(page.locator('meta[name="robots"]')).toHaveCount(0);

  const robots = await (await request.get('/robots.txt')).text();
  expect(robots).toBe(
    'User-agent: *\nAllow: /tiles/trees.json\nAllow: /tiles/*/map.json\nAllow: /tiles/map.json\nDisallow: /tiles/\n',
  );
  expect((await request.get('/favicon.png')).ok()).toBe(true);
});

test('mit SITE_URL: absolute Adressen, robots.txt ab der Wurzel der Domain', () => {
  const { kopf, robots } = baue({
    SITE_URL: 'https://example.org/karte',
    SITE_TITLE: 'Karte "A" & <B>',
  });
  expect(kopf).toContain('<link rel="canonical" href="https://example.org/karte/" />');
  expect(kopf).toContain('<meta property="og:url" content="https://example.org/karte/" />');
  expect(kopf).toContain(
    '<meta property="og:image" content="https://example.org/karte/vorschau.jpg" />',
  );
  expect(kopf).toContain('<meta name="twitter:card" content="summary_large_image" />');
  expect(kopf).not.toMatch(/%[A-Z]+%|<!--\/?mit-/);
  expect(kopf).not.toContain('noindex');
  // Im Kopf, nach dem Zeichensatz.
  expect(kopf.indexOf('<meta charset')).toBeLessThan(kopf.indexOf('rel="canonical"'));
  expect(kopf.indexOf('rel="canonical"')).toBeLessThan(kopf.indexOf('</head>'));
  // Der Titel ist maskiert, in Text und Attributen.
  expect(kopf).toContain('<title>Karte &quot;A&quot; &amp; &lt;B&gt;</title>');
  expect(kopf).toContain('content="Karte &quot;A&quot; &amp; &lt;B&gt;"');
  expect(robots).toBe(
    'User-agent: *\nAllow: /karte/tiles/trees.json\nAllow: /karte/tiles/*/map.json\nAllow: /karte/tiles/map.json\nDisallow: /karte/tiles/\n',
  );
});

test('leere Angaben zählen wie keine, eine Adresse ohne Schema bricht ab', () => {
  const { kopf } = baue({ SITE_URL: '', SITE_TITLE: '', SITE_DESCRIPTION: '' });
  expect(kopf).toContain('<title>Heroic Map Renderer</title>');
  expect(kopf).not.toContain('canonical');
  expect(() => baue({ SITE_URL: 'example.org/karte' })).toThrow(
    /SITE_URL muss mit http:\/\/ oder https:\/\/ beginnen/,
  );
});

/**
 * Die Regel für seite.html, wie der Server des Renderers sie anwendet, hier
 * unabhängig vom Build nachgebaut: Blöcke ohne Wert fallen ganz weg, mit Wert
 * nur ihre Kommentare; die Marker ersetzt ein Gang, maskiert.
 */
function fuelleVorlage(vorlage: string, werte: { titel: string; beschreibung: string; url?: string; bild?: string }): string {
  const maskiert = (t: string) => t.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
  const block = (t: string, name: string, an: boolean) => {
    const [auf, zu] = [`<!--${name}-->`, `<!--/${name}-->`];
    const [a, z] = [t.indexOf(auf), t.indexOf(zu)];
    return t.slice(0, a) + (an ? t.slice(a + auf.length, z) : '') + t.slice(z + zu.length);
  };
  const ohneBloecke = block(block(vorlage, 'mit-bild', werte.bild !== undefined), 'mit-url', werte.url !== undefined);
  const marker: Record<string, string | undefined> = { TITEL: werte.titel, BESCHREIBUNG: werte.beschreibung, URL: werte.url, BILD: werte.bild };
  return ohneBloecke.replace(/%(TITEL|BESCHREIBUNG|URL|BILD)%/g, (_, name: string) => maskiert(marker[name] ?? ''));
}

test('seite.html ist die gebaute Seite mit allen Markern, robots.vorlage.txt hat %PFAD%; gefüllt gleichen sie index.html und robots.txt Byte für Byte', () => {
  const ohne = baue({});
  const { vorlage, robotsVorlage } = ohne;
  for (const marker of ['%TITEL%', '%BESCHREIBUNG%', '%URL%', '%BILD%']) expect(vorlage, marker).toContain(marker);
  for (const block of ['mit-url', 'mit-bild']) {
    expect(vorlage.split(`<!--${block}-->`)).toHaveLength(2);
    expect(vorlage.split(`<!--/${block}-->`)).toHaveLength(2);
  }
  // Der Bild-Block liegt im URL-Block, und die Namen aus dem Build stehen schon drin.
  expect(vorlage.indexOf('<!--mit-url-->')).toBeLessThan(vorlage.indexOf('<!--mit-bild-->'));
  expect(vorlage.indexOf('<!--/mit-bild-->')).toBeLessThan(vorlage.indexOf('<!--/mit-url-->'));
  expect(vorlage).toMatch(/<script type="module" crossorigin src="\.\/assets\/index-[\w-]+\.js"><\/script>/);
  expect(vorlage).not.toContain('/src/main.ts');
  expect(robotsVorlage).toContain('Disallow: %PFAD%tiles/');

  const standard = { titel: 'Heroic Map Renderer', beschreibung: 'Isometrische Karte einer Minecraft-Welt.' };
  expect(fuelleVorlage(vorlage, standard)).toBe(ohne.kopf);
  expect(robotsVorlage.replaceAll('%PFAD%', '/')).toBe(ohne.robots);

  // Ein Marker im Titel bleibt stehen: Ersetzt wird in einem Gang.
  const titel = 'Karte %URL% "A" & <B>';
  const mit = baue({ SITE_URL: 'https://example.org/karte', SITE_TITLE: titel });
  expect(mit.vorlage).toBe(vorlage);
  const gefuellt = fuelleVorlage(vorlage, { ...standard, titel, url: 'https://example.org/karte/', bild: 'https://example.org/karte/vorschau.jpg' });
  expect(gefuellt).toBe(mit.kopf);
  expect(mit.kopf).toContain('<title>Karte %URL% &quot;A&quot; &amp; &lt;B&gt;</title>');
  expect(mit.robotsVorlage.replaceAll('%PFAD%', '/karte/')).toBe(mit.robots);
});
