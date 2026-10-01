import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

/** Baut die Seite mit SITE_URL in ein eigenes Verzeichnis. */
function baue(url: string): { kopf: string; robots: string } {
  const dir = mkdtempSync(join(tmpdir(), 'seite-'));
  try {
    const vite = join('node_modules', 'vite', 'bin', 'vite.js');
    execFileSync(
      process.execPath,
      [vite, 'build', '--outDir', dir, '--emptyOutDir', '--logLevel', 'error'],
      { env: { ...process.env, SITE_URL: url } },
    );
    return {
      kopf: readFileSync(join(dir, 'index.html'), 'utf8'),
      robots: readFileSync(join(dir, 'robots.txt'), 'utf8'),
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
  expect(robots).toBe('User-agent: *\nDisallow: /tiles/\n');
  expect((await request.get('/favicon.png')).ok()).toBe(true);
});

test('mit SITE_URL: absolute Adressen, robots.txt ab der Wurzel der Domain', () => {
  const { kopf, robots } = baue('https://example.org/karte');
  expect(kopf).toContain('<link rel="canonical" href="https://example.org/karte/">');
  expect(kopf).toContain('<meta property="og:url" content="https://example.org/karte/">');
  expect(kopf).toContain(
    '<meta property="og:image" content="https://example.org/karte/vorschau.jpg">',
  );
  expect(kopf).not.toContain('noindex');
  expect(robots).toBe('User-agent: *\nDisallow: /karte/tiles/\n');
});
