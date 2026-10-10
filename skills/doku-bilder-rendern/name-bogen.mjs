// Rendert docs/bilder/quellen/name-bogen/name-bogen.html zu docs/bilder/name-bogen.png,
// mit dem Chromium von Playwright aus web/. Aus web/: node ../skills/doku-bilder-rendern/name-bogen.mjs
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Playwright aus web/node_modules: Das Skript liegt ausserhalb von web/.
const { chromium } = createRequire(`${process.cwd()}/`)('playwright');

const wurzel = fileURLToPath(new URL('../../', import.meta.url));
const browser = await chromium.launch();
const page = await browser.newPage({ deviceScaleFactor: 1 });
await page.goto(pathToFileURL(`${wurzel}docs/bilder/quellen/name-bogen/name-bogen.html`).href);
await page.waitForSelector('body[data-fertig="1"]');
await page.locator('#blatt').screenshot({ path: `${wurzel}docs/bilder/name-bogen.png` });
await browser.close();
