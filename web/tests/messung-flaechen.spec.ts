/**
 * Messung nach Regel 26: was ein Kreis mit 2000 Blöcken Radius im Browser
 * kostet, nur mit Rand, gefüllt und gefüllt mit Tafel und Schrift, von oben
 * und im iso: Zeit bis zum Pfad, JS-Heap danach nach der Speicherbereinigung,
 * Spitze beim Laden, Punkte. Läuft nur mit MESSUNG=1, siehe
 * docs/messungen/2026-10-09-flaechen-im-browser.md.
 */
import { expect, test } from '@playwright/test';
import { execSync } from 'node:child_process';
import { DEMO, STAEDTE, welt } from './ebenen-welt';

test.skip(!process.env.MESSUNG, 'nur mit MESSUNG=1');
test.setTimeout(30 * 60_000);

const MITTE = [-2500, -2500];
const KREIS = { id: 'kreis', type: 'circle', center: MITTE, radius: 2000, stroke: { color: '#FF0000', width: 2 } };
const SCHRIFT = { id: 'name', type: 'label', text: 'Weites Land', path: [[-3000, -2500], [-2000, -2500]], size: 40 };
const VARIANTEN = [
  { name: 'Rand', objekte: [KREIS], gefuellt: false },
  { name: 'gefüllt', objekte: [{ ...KREIS, fill: '#FF000055' }], gefuellt: true },
  { name: 'gefüllt, Tafel, Schrift', objekte: [{ ...KREIS, fill: '#FF000055', name: 'Weit', panel: { blocks: [{ type: 'title', text: 'Weit' }] } }, SCHRIFT], gefuellt: true },
];
/** Hügeliges Gelände um 64, damit „verdeckt“ etwas zu tun hat. */
const hoehe = (i: number, j: number) => Math.round(64 + 30 * Math.sin(i / 7) * Math.cos(j / 9));

/**
 * Wartet, bis die Last der Maschine unter der Grenze liegt, nach
 * skills/messung-protokollieren 10 %; mit einer festen Grundlast anderer
 * Programme gibt `MESSUNG_LAST` eine höhere Grenze, das Protokoll nennt sie.
 * Nach 5 min bricht die Reihe ab.
 */
function ruhe(): number {
  const grenze = Number(process.env.MESSUNG_LAST ?? 10);
  const ende = Date.now() + 5 * 60_000;
  let last = Number.NaN;
  while (Date.now() < ende) {
    last = Number(execSync('powershell -NoProfile -Command "(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average"', { timeout: 30_000 }).toString().trim());
    if (!Number.isFinite(last)) throw new Error('Last unlesbar');
    if (last < grenze) return last;
    execSync('powershell -NoProfile -Command "Start-Sleep -Seconds 5"', { timeout: 30_000 });
  }
  throw new Error(`Last blieb 5 min über ${grenze} %, zuletzt ${last} %`);
}

/** Ein Schritt mit Frist: Hängt er, sagt der Fehler, welcher. */
async function frist<T>(schritt: string, ms: number, versprechen: Promise<T>): Promise<T> {
  let uhr: ReturnType<typeof setTimeout> | undefined;
  const ablauf = new Promise<never>((_, nein) => (uhr = setTimeout(() => nein(new Error(`${schritt}: nach ${ms} ms nicht fertig`)), ms)));
  try {
    return await Promise.race([versprechen, ablauf]);
  } finally {
    clearTimeout(uhr);
  }
}

for (const [ansicht, mehr] of [
  ['von oben', { camera: 'top', projection: { azimuth: 'north', u: 16, v: 16, y: 0 } }],
  ['iso 2:1', {}],
] as const) {
  test(`Kreis mit 2000 Blöcken Radius, ${ansicht}`, async ({ page }) => {
    const fehler: string[] = [];
    page.on('pageerror', (e) => fehler.push(e.message));
    page.on('console', (m) => {
      if (m.type() === 'error' || m.type() === 'warning') fehler.push(m.text());
    });
    // Die Karte merkt sich die Wahl je Ebene; ohne das stünde die Ebene ab dem zweiten Lauf schon an, und der Klick schaltete sie aus.
    await page.addInitScript(() => localStorage.clear());
    const cdp = await page.context().newCDPSession(page);
    const heap = async (bereinigen: boolean) => {
      if (bereinigen) await frist('Speicherbereinigung', 60_000, cdp.send('HeapProfiler.collectGarbage'));
      return (await frist('Heap lesen', 10_000, cdp.send('Runtime.getHeapUsage'))).usedSize;
    };
    // Abwechselnd die Varianten, je drei Läufe.
    for (let lauf = 1; lauf <= 3; lauf++) {
      for (const { name, objekte, gefuellt } of VARIANTEN) {
        await page.unrouteAll({ behavior: 'ignoreErrors' });
        await welt(page, { liste: () => [{ ...STAEDTE, visible: false }], datei: () => ({ objects: objekte }), mehr: { ...mehr, minZoom: -8 }, hoehe });
        await frist('Seite laden', 60_000, page.goto(`${DEMO}&at=${MITTE[0]},64,${MITTE[1]}&zoom=-6`));
        await frist('Liste öffnen', 30_000, page.locator('.ebenen summary').click());
        const vorher = await heap(true);
        const last = ruhe();
        // Die Spitze beim Laden: der Heap alle 50 ms, bis der Pfad steht.
        let spitze = vorher;
        let fertig = false;
        const abtasten = (async () => {
          while (!fertig) {
            spitze = Math.max(spitze, await heap(false));
            await new Promise((r) => setTimeout(r, 50));
          }
        })();
        const ms = await frist(
          `${ansicht}, ${name}, Lauf ${lauf}: Pfad`,
          180_000,
          page.evaluate(
            (gefuellt) =>
              new Promise<number>((ja) => {
                const anfang = performance.now();
                const da = () => {
                  const pfad = gefuellt ? document.querySelector('path[fill="#FF000055"]') : document.querySelector('path[stroke="#FF0000"]');
                  return Boolean(pfad?.getAttribute('d'));
                };
                const beobachter = new MutationObserver(() => {
                  if (!da()) return;
                  beobachter.disconnect();
                  ja(performance.now() - anfang);
                });
                beobachter.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['d'] });
                document.querySelector<HTMLInputElement>('.ebenen input')!.click();
              }),
            gefuellt,
          ),
        ).finally(() => {
          fertig = true;
        });
        await abtasten;
        const nachher = await heap(true);
        const punkte = await page.evaluate(() =>
          [...document.querySelectorAll('.leaflet-pane svg path')].reduce((s, p) => s + (p.getAttribute('d')?.match(/[ML]/g)?.length ?? 0), 0),
        );
        const mib = (b: number) => (b / 2 ** 20).toFixed(1);
        console.log(`${ansicht} | ${name} | ${lauf} | ${ms.toFixed(0)} ms | ${mib(nachher - vorher)} MiB | Spitze ${mib(spitze - vorher)} MiB | ${punkte} Punkte | Last ${last} %`);
        expect(ms).toBeGreaterThan(0);
      }
    }
    if (fehler.length) console.log(`Meldungen der Seite:\n${[...new Set(fehler)].join('\n')}`);
  });
}
