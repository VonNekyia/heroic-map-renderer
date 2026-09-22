import { defineConfig } from 'vite';

export default defineConfig({
  // Relative Pfade: die fertige Karte soll auch unter einem Unterpfad
  // liegen koennen, ohne neu gebaut zu werden.
  base: './',
  build: { outDir: 'dist', emptyOutDir: true },
});
