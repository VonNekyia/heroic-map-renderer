import { cpSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const PUBLIC = fileURLToPath(new URL('./public', import.meta.url));
const TILES = join(PUBLIC, 'tiles');

/** Was der Betreiber beim Build setzt, siehe docs/frontend.md, „Ausliefern“. */
const SEITE = {
  url: adresse(process.env.SITE_URL),
  titel: process.env.SITE_TITLE ?? 'Heroic Map Renderer',
  beschreibung: process.env.SITE_DESCRIPTION ?? 'Isometrische Karte einer Minecraft-Welt.',
  bild: process.env.SITE_IMAGE ?? 'vorschau.jpg',
};

/** Die Adresse der Seite, mit `/` am Ende; leer ohne Angabe. */
function adresse(wert: string | undefined): URL | undefined {
  if (!wert) return undefined;
  const url = new URL(wert);
  if (url.protocol !== 'https:' && url.protocol !== 'http:') {
    throw new Error(`SITE_URL muss mit http:// oder https:// beginnen: ${wert}`);
  }
  if (!url.pathname.endsWith('/')) url.pathname += '/';
  return url;
}

const html = (text: string): string =>
  text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

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
      // Titel und Beschreibung immer; was eine absolute Adresse braucht,
      // nur mit SITE_URL.
      name: 'seite',
      transformIndexHtml(text) {
        const kopf = text
          .replaceAll('%TITEL%', html(SEITE.titel))
          .replaceAll('%BESCHREIBUNG%', html(SEITE.beschreibung));
        const { url } = SEITE;
        if (!url) return kopf;
        const bild = new URL(SEITE.bild, url).href;
        const meta = (property: string, content: string) => ({
          tag: 'meta',
          attrs: { property, content },
        });
        return {
          html: kopf,
          tags: [
            { tag: 'link', attrs: { rel: 'canonical', href: url.href } },
            meta('og:url', url.href),
            meta('og:image', bild),
            meta('og:image:alt', `Ausschnitt der Karte: ${SEITE.titel}`),
            { tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
          ],
        };
      },
      // Suchmaschinen finden die Seite, rufen aber nicht jede Kachel ab.
      // Der Pfad zählt ab der Wurzel der Domain.
      generateBundle() {
        const pfad = SEITE.url?.pathname ?? '/';
        this.emitFile({
          type: 'asset',
          fileName: 'robots.txt',
          source: `User-agent: *\nDisallow: ${pfad}tiles/\n`,
        });
      },
    },
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
