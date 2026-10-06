import { cpSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const WEB = fileURLToPath(new URL('.', import.meta.url));
const PUBLIC = join(WEB, 'public');
const TILES = join(PUBLIC, 'tiles');

/** Was der Betreiber beim Build setzt, siehe docs/frontend.md, „Ausliefern“. */
const SEITE = {
  url: adresse(process.env.SITE_URL),
  // Leere Werte, etwa aus der CI, zählen wie keine.
  titel: process.env.SITE_TITLE || 'Heroic Map Renderer',
  beschreibung: process.env.SITE_DESCRIPTION || 'Isometrische Karte einer Minecraft-Welt.',
  bild: process.env.SITE_IMAGE || 'vorschau.jpg',
};

/**
 * Der Skin: ein Modul, ein Pfad ab `web/` wie `./skins/tablett` oder ein
 * Paket. Ohne Angabe keiner. Siehe docs/frontend.md, „Skins“.
 */
const SKIN = process.env.SKIN || '';
/** Texte für den Skin: `titel`, dazu je `SKIN_TEXT_<NAME>` ein Eintrag `<name>`. */
const SKIN_TEXTE = Object.fromEntries([
  ['titel', SEITE.titel],
  ...Object.entries(process.env)
    .filter(([name, wert]) => name.startsWith('SKIN_TEXT_') && wert)
    .map(([name, wert]) => [name.slice('SKIN_TEXT_'.length).toLowerCase(), wert]),
]) as Record<string, string>;

/** Die Adresse der Seite, mit `/` am Ende; leer ohne Angabe. */
function adresse(wert: string | undefined): URL | undefined {
  if (!wert) return undefined;
  const url = URL.canParse(wert) ? new URL(wert) : undefined;
  if (!url || (url.protocol !== 'https:' && url.protocol !== 'http:')) {
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
  // `license`: die Lizenzen aller gebündelten Abhängigkeiten, etwa Leaflet
  // (BSD-2-Clause), als Datei im Build. Die Karte verlinkt sie.
  build: { outDir: 'dist', emptyOutDir: true, copyPublicDir: false, license: { fileName: 'lizenzen.txt' } },
  // Ohne Skin fällt sein Import beim Build weg.
  define: {
    __SKIN__: JSON.stringify(Boolean(SKIN)),
    __SKIN_TEXTE__: JSON.stringify(SKIN ? SKIN_TEXTE : {}),
  },
  // Ein Skin von ausserhalb nimmt Leaflet aus web/, wie die Karte.
  resolve: { dedupe: ['leaflet'] },
  plugins: [
    {
      // `virtual:skin` ist das Modul aus SKIN, aufgelöst ab web/.
      name: 'skin',
      async resolveId(id) {
        if (id !== 'virtual:skin') return;
        if (!SKIN) return '\0virtual:skin';
        const skin = await this.resolve(SKIN, join(WEB, 'index.html'));
        if (!skin) this.error(`SKIN nicht gefunden: ${SKIN}`);
        return skin;
      },
      load(id) {
        if (id === '\0virtual:skin') return 'export default undefined;';
      },
    },
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
          injectTo: 'head' as const,
        });
        return {
          html: kopf,
          tags: [
            { tag: 'link', attrs: { rel: 'canonical', href: url.href }, injectTo: 'head' },
            meta('og:url', url.href),
            meta('og:image', bild),
            meta('og:image:alt', `Ausschnitt der Karte: ${SEITE.titel}`),
            {
              tag: 'meta',
              attrs: { name: 'twitter:card', content: 'summary_large_image' },
              injectTo: 'head',
            },
          ],
        };
      },
      // Suchmaschinen finden die Seite, rufen aber nicht jede Kachel ab;
      // trees.json und die map.json der Bäume brauchen sie, um die Seite zu
      // rendern; `tiles/map.json` für einen Baum ohne Liste. Der Pfad zählt
      // ab der Wurzel der Domain.
      generateBundle() {
        const pfad = SEITE.url?.pathname ?? '/';
        const erlaubt = ['trees.json', '*/map.json', 'map.json'];
        this.emitFile({
          type: 'asset',
          fileName: 'robots.txt',
          source: [
            'User-agent: *',
            ...erlaubt.map((datei) => `Allow: ${pfad}tiles/${datei}`),
            `Disallow: ${pfad}tiles/`,
            '',
          ].join('\n'),
        });
      },
    },
    {
      // Vor die Lizenzen der Pakete NOTICE und LICENSE des Projekts: Apache-2.0
      // verlangt beide bei jeder Weitergabe. Vorn ein BOM: Server senden .txt
      // meist ohne charset, und der Browser läse UTF-8 sonst als Latin-1.
      // Siehe docs/frontend.md, „Lizenzen“.
      name: 'notice',
      apply: 'build',
      writeBundle({ dir }) {
        if (!dir) return;
        const datei = join(dir, 'lizenzen.txt');
        const eigen = ['NOTICE', 'LICENSE'].map((name) => readFileSync(join(WEB, '..', name), 'utf8').trimEnd());
        writeFileSync(datei, '﻿' + [...eigen, readFileSync(datei, 'utf8')].join('\n\n---\n\n'));
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
  // Produktion; dieselben setzt der Server des Renderers. Siehe
  // docs/frontend.md, „Ausliefern“.
  preview: { headers: JSON.parse(readFileSync(join(WEB, 'headers.json'), 'utf8')) as Record<string, string> },
});
