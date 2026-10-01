import { cpSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const PUBLIC = fileURLToPath(new URL('./public', import.meta.url));
const TILES = join(PUBLIC, 'tiles');

export default defineConfig({
  // Relative Pfade: die fertige Karte soll auch unter einem Unterpfad
  // liegen koennen, ohne neu gebaut zu werden.
  base: './',
  // public/ kopiert der Build selbst, ohne public/tiles: dort liegt oft
  // ein Link auf einen Render mit Millionen Kacheln. Sonst wie Vite: vor
  // dem Bundle, damit dessen Dateien gewinnen, und Links mit ihrem Inhalt.
  // Der Filter greift, bevor cpSync einen Eintrag ansieht.
  // Siehe docs/entscheidungen/0006-kacheln-unter-web-public.md.
  build: { outDir: 'dist', emptyOutDir: true, copyPublicDir: false },
  plugins: [
    {
      name: 'public-ohne-kacheln',
      apply: 'build',
      renderStart({ dir }) {
        if (dir) {
          cpSync(PUBLIC, dir, {
            recursive: true,
            dereference: true,
            filter: (src) => src !== TILES,
          });
        }
      },
    },
  ],
  // Der Watcher des Devservers lässt die Kacheln aus. Neue Kacheln liefert
  // Vite dann nur, wenn public/tiles ein Link ist: dann fragt es je
  // Anfrage die Platte, statt in seiner Liste vom Start nachzusehen.
  // Siehe docs/frontend.md, „Einem Render zusehen“.
  server: { watch: { ignored: ['**/public/tiles/**'] } },
  // Die Header, unter denen die Tests den Build prüfen, streng wie in
  // Produktion. Siehe docs/frontend.md, „Ausliefern“.
  preview: {
    headers: {
      'Content-Security-Policy':
        "default-src 'self'; object-src 'none'; " +
        "base-uri 'self'; frame-ancestors 'self'; form-action 'self'",
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Permissions-Policy': 'camera=(), geolocation=(), microphone=()',
      'Referrer-Policy': 'strict-origin-when-cross-origin',
      'X-Content-Type-Options': 'nosniff',
      'X-Frame-Options': 'SAMEORIGIN',
    },
  },
});
