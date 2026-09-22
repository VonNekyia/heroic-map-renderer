import { defineConfig } from 'vite';

export default defineConfig({
  // Relative Pfade: die fertige Karte soll auch unter einem Unterpfad
  // liegen koennen, ohne neu gebaut zu werden.
  base: './',
  build: { outDir: 'dist', emptyOutDir: true },
  // Die Kacheln sind Millionen Dateien, die ein laufender Render ständig
  // anlegt. Der Watcher des Devservers würde sie alle beobachten und
  // dabei Kerne verbrennen, die der Render braucht.
  server: { watch: { ignored: ['**/public/tiles/**'] } },
});
