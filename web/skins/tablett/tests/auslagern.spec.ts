import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SKIN = fileURLToPath(new URL('..', import.meta.url));
const WEB = fileURLToPath(new URL('../../..', import.meta.url));

test('die Karte baut mit dem Skin aus einem Ordner ausserhalb des Repositorys', () => {
  test.setTimeout(120_000);
  const ort = mkdtempSync(join(tmpdir(), 'skin-'));
  try {
    // Der Skin ohne seine Tests, wie ein Paket von aussen.
    const ordner = join(ort, 'tablett');
    cpSync(SKIN, ordner, { recursive: true, filter: (quelle) => !quelle.startsWith(join(SKIN, 'tests')) });
    const ziel = join(ort, 'dist');
    execFileSync(
      process.execPath,
      [join(WEB, 'node_modules', 'vite', 'bin', 'vite.js'), 'build', '--outDir', ziel, '--emptyOutDir', '--logLevel', 'error'],
      { cwd: WEB, env: { ...process.env, SKIN: ordner }, stdio: 'pipe' },
    );
    const js = readdirSync(join(ziel, 'assets'))
      .filter((datei) => datei.endsWith('.js'))
      .map((datei) => readFileSync(join(ziel, 'assets', datei), 'utf8'));
    // Der Skin ist ein eigenes Stück; Leaflet steht nur im Bündel der Karte,
    // der Skin nimmt dieses.
    const skin = js.filter((text) => text.includes('tablett-fern'));
    expect(skin).toHaveLength(1);
    expect(skin[0]).not.toContain('_leaflet_id');
    expect(js.filter((text) => text.includes('_leaflet_id'))).toHaveLength(1);
  } finally {
    rmSync(ort, { recursive: true, force: true });
  }
});
