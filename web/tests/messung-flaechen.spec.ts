/**
 * Messung nach Regel 26: was ein Kreis mit 2000 Blöcken Radius im Browser
 * kostet, gefüllt und nur mit Rand, von oben und im iso. Läuft nur mit
 * MESSUNG=1, siehe docs/messungen/2026-10-09-flaechen-im-browser.md.
 */
import { expect, test, type Page } from '@playwright/test';
import { execSync } from 'node:child_process';
import { DEMO, STAEDTE, welt } from './ebenen-welt';

test.skip(!process.env.MESSUNG, 'nur mit MESSUNG=1');
test.setTimeout(30 * 60_000);

const MITTE = [-2500, -2500];
const KREIS = { id: 'kreis', type: 'circle', center: MITTE, radius: 2000, stroke: { color: '#FF0000', width: 2 } };
const VARIANTEN = [
  { name: 'Rand', kreis: KREIS },
  { name: 'gefüllt', kreis: { ...KREIS, fill: '#FF000055' } },
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
    last = Number(execSync('powershell -NoProfile -Command "(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average"').toString().trim());
    if (!Number.isFinite(last)) throw new Error('Last unlesbar');
    if (last < grenze) return last;
    execSync('powershell -NoProfile -Command "Start-Sleep -Seconds 5"');
  }
  throw new Error(`Last blieb 5 min über ${grenze} %, zuletzt ${last} %`);
}

/** JS-Heap nach Speicherbereinigung, in Byte. */
async function heap(page: Page): Promise<number> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('HeapProfiler.collectGarbage');
  const { usedSize } = await cdp.send('Runtime.getHeapUsage');
  await cdp.detach();
  return usedSize;
}

for (const [ansicht, mehr] of [
  ['von oben', { camera: 'top', projection: { azimuth: 'north', u: 16, v: 16, y: 0 } }],
  ['iso 2:1', {}],
] as const) {
  test(`Kreis mit 2000 Blöcken Radius, ${ansicht}`, async ({ page }) => {
    const zeilen: string[] = [];
    // Abwechselnd Rand, gefüllt, je drei Läufe.
    for (let lauf = 1; lauf <= 3; lauf++) {
      for (const { name, kreis } of VARIANTEN) {
        await page.unrouteAll({ behavior: 'wait' });
        await welt(page, {
          liste: () => [{ ...STAEDTE, visible: false }],
          datei: () => ({ objects: [kreis] }),
          mehr: { ...mehr, minZoom: -8 },
          hoehe,
        });
        await page.goto(`${DEMO}&at=${MITTE[0]},64,${MITTE[1]}&zoom=-6`);
        await page.locator('.ebenen summary').click();
        const vorher = await heap(page);
        const last = ruhe();
        const ms = await page.evaluate(
          (gefuellt) =>
            new Promise<number>((fertig) => {
              const box = document.querySelector<HTMLInputElement>('.ebenen input')!;
              const anfang = performance.now();
              box.click();
              const sieh = () => {
                const da = gefuellt ? document.querySelector('path[fill="#FF000055"]') : document.querySelector('path[stroke="#FF0000"]');
                if (da?.getAttribute('d')) fertig(performance.now() - anfang);
                else requestAnimationFrame(sieh);
              };
              sieh();
            }),
          name === 'gefüllt',
        );
        const nachher = await heap(page);
        const punkte = await page.evaluate(() =>
          [...document.querySelectorAll('.leaflet-pane svg path')].reduce((s, p) => s + (p.getAttribute('d')?.match(/[ML]/g)?.length ?? 0), 0),
        );
        zeilen.push(`${ansicht} | ${name} | ${lauf} | ${ms.toFixed(0)} ms | ${((nachher - vorher) / 2 ** 20).toFixed(1)} MiB | ${punkte} Punkte | Last ${last} %`);
        expect(ms).toBeGreaterThan(0);
      }
    }
    console.log(zeilen.join('\n'));
  });
}
