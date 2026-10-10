import { defineConfig, devices } from '@playwright/test';

const HOST = '127.0.0.1';
/**
 * Der Port des Builds ohne Skin, aus `PW_PORT`, sonst 4173; die beiden
 * anderen liegen 2 und 3 darüber. Mit `PW_PORT` nutzt Playwright keinen
 * laufenden Server: Sonst antwortete womöglich ein fremder, und die Tests
 * prüften fremden Code. Siehe docs/entwicklung/tests.md, „Laufen lassen“.
 */
const EIGENER = process.env.PW_PORT;
if (EIGENER !== undefined && !/^\d+$/.test(EIGENER)) throw new Error(`PW_PORT=${EIGENER}: keine Zahl`);
const PORT = EIGENER === undefined ? 4173 : Number(EIGENER);
/** Der Build mit dem Skin aus web/skins/tablett: vorerst nur der Marmor. */
const PORT_SKIN = PORT + 2;
/** Der Build mit dem ganzen, vertagten Tablett aus web/skins/tablett/voll. */
const PORT_TABLETT = PORT + 3;
const WIEDER = !process.env.CI && EIGENER === undefined;

export default defineConfig({
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'list' : 'html',
  use: { baseURL: `http://${HOST}:${PORT}` },
  projects: [
    // Ohne Skin: die Grundkarte.
    { name: 'grund', testDir: './tests', use: devices['Desktop Chrome'] },
    // Mit Skin: die Smoke-Tests noch einmal und die Tests des Skins, ausser
    // denen des ganzen Tabletts im Browser.
    {
      name: 'skin',
      testDir: '.',
      testMatch: [/[\\/]tests[\\/]smoke\.spec\.ts$/, /[\\/]skins[\\/][^\\/]+[\\/]tests[\\/][^\\/]+\.spec\.ts$/],
      testIgnore: /[\\/]karte\.spec\.ts$/,
      grepInvert: /@ohne-skin/,
      use: { ...devices['Desktop Chrome'], baseURL: `http://${HOST}:${PORT_SKIN}` },
    },
    // Das ganze Tablett, vertagt: Seine Tests laufen weiter, am eigenen Build.
    {
      name: 'tablett',
      testDir: './skins/tablett/tests',
      testMatch: /[\\/]karte\.spec\.ts$/,
      use: { ...devices['Desktop Chrome'], baseURL: `http://${HOST}:${PORT_TABLETT}` },
    },
  ],
  // Geprueft wird der fertige Build, nicht der Dev-Server: ausgeliefert
  // wird schliesslich der Build.
  webServer: [
    {
      // Host ausdruecklich: sonst bindet Vite unter Windows nur an ::1,
      // und Playwright wartet vergeblich auf 127.0.0.1.
      command: `npm run build && npm run preview -- --host ${HOST} --port ${PORT} --strictPort`,
      url: `http://${HOST}:${PORT}`,
      reuseExistingServer: WIEDER,
      timeout: 120_000,
    },
    {
      command:
        `npx vite build --outDir dist-skin && ` +
        `npx vite preview --outDir dist-skin --host ${HOST} --port ${PORT_SKIN} --strictPort`,
      // Texte für die Buchrücken, nur zum Prüfen; echte setzt der Betreiber.
      env: { SKIN: './skins/tablett' },
      url: `http://${HOST}:${PORT_SKIN}`,
      reuseExistingServer: WIEDER,
      timeout: 120_000,
    },
    {
      command:
        `npx vite build --outDir dist-tablett && ` +
        `npx vite preview --outDir dist-tablett --host ${HOST} --port ${PORT_TABLETT} --strictPort`,
      // Texte für die Buchrücken, nur zum Prüfen; echte setzt der Betreiber.
      env: { SKIN: './skins/tablett/voll', SKIN_TEXT_BUCH1: 'Probe Eins', SKIN_TEXT_BUCH2: 'Probe Zwei' },
      url: `http://${HOST}:${PORT_TABLETT}`,
      reuseExistingServer: WIEDER,
      timeout: 120_000,
    },
  ],
});
