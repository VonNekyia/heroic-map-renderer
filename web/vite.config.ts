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
  // ein Link auf einen Render mit Millionen Kacheln, und Vite folgte ihm
  // beim Kopieren nach dist, auch unter `npm test`. Sonst wie Vite: vor
  // dem Bundle, damit dessen Dateien gewinnen, und Links mit ihrem Inhalt.
  // Der Filter greift, bevor cpSync einen Eintrag ansieht.
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
  // Die Kacheln sind Millionen Dateien, die ein laufender Render ständig
  // anlegt. Der Watcher des Devservers würde sie alle beobachten und
  // dabei Kerne verbrennen, die der Render braucht. Neue Kacheln liefert
  // Vite dann nur, wenn public/tiles ein Link ist: dann fragt es je
  // Anfrage die Platte, statt in seiner Liste vom Start nachzusehen.
  server: { watch: { ignored: ['**/public/tiles/**'] } },
});
