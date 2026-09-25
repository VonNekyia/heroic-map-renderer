import { cpSync, readdirSync } from 'node:fs';
import { defineConfig } from 'vite';

const PUBLIC = new URL('./public/', import.meta.url);

export default defineConfig({
  // Relative Pfade: die fertige Karte soll auch unter einem Unterpfad
  // liegen koennen, ohne neu gebaut zu werden.
  base: './',
  // public/ kopiert der Build selbst, ohne public/tiles: dort liegt oft
  // ein Link auf einen Render mit Millionen Kacheln, und Vite folgte ihm
  // beim Kopieren nach dist, auch unter `npm test`.
  build: { outDir: 'dist', emptyOutDir: true, copyPublicDir: false },
  plugins: [
    {
      name: 'public-ohne-kacheln',
      apply: 'build',
      writeBundle({ dir }) {
        for (const name of readdirSync(PUBLIC)) {
          if (name !== 'tiles') {
            cpSync(new URL(name, PUBLIC), `${dir}/${name}`, { recursive: true });
          }
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
