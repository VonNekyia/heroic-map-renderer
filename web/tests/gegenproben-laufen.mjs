// Gegenproben: Jede Stelle aus tests/gegenproben.mjs wird einzeln falsch
// gemacht, ihr Test muss dann fallen. Die Datei bekommt danach immer ihre
// Bytes zurück, auch nach einem Fehler oder Strg+C, und wird gegen das
// Original geprüft. Siehe docs/entwicklung/tests.md, „Mutationen“.
// Aus web/:  npm run gegenproben [-- name …]
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import liste from './gegenproben.mjs';

/**
 * Die Bytes der mutierten Datei auch auf der Platte, unter node_modules, das
 * git nicht sieht: Einen harten Abbruch, unter Windows etwa taskkill, fängt
 * kein finally. Der nächste Lauf stellt sie dann zuerst her.
 */
const SICHERUNG = 'node_modules/.gegenprobe-offen.json';

/** Die Datei, die gerade mutiert ist, mit ihren Bytes von vorher. */
let offen;

/** Schreibt die Bytes zurück und prüft, dass die Datei wieder genau wie vorher ist. */
function zurueck() {
  if (!offen) return;
  const { pfad, original } = offen;
  writeFileSync(pfad, original);
  if (!readFileSync(pfad).equals(original)) throw new Error(`${pfad}: nicht wiederhergestellt`);
  rmSync(SICHERUNG, { force: true });
  offen = undefined;
}

if (existsSync(SICHERUNG)) {
  const { pfad, bytes } = JSON.parse(readFileSync(SICHERUNG, 'utf8'));
  offen = { pfad, original: Buffer.from(bytes, 'base64') };
  zurueck();
  console.log(`${pfad} aus einem abgebrochenen Lauf wiederhergestellt`);
}
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    zurueck();
    process.exit(130);
  });
}

/** Läuft der Test, ohne zu fallen? Ohne Shell, damit Strg+C ihn trifft. */
const gruen = (spec, titel) =>
  new Promise((fertig) => {
    const muster = titel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const kind = spawn(process.execPath, ['node_modules/@playwright/test/cli.js', 'test', '--project=grund', spec, '--grep', muster, '--retries=0', '--reporter=line'], { stdio: 'ignore' });
    kind.on('exit', (code) => fertig(code === 0));
  });

const nur = process.argv.slice(2);
const fremd = nur.filter((n) => !liste.some((e) => e.name === n));
if (fremd.length) throw new Error(`keine Gegenprobe heisst ${fremd.join(', ')}`);
let schlecht = 0;
for (const { name, datei, alt, neu, spec, titel } of liste) {
  if (nur.length && !nur.includes(name)) continue;
  const original = readFileSync(datei);
  const text = original.toString('utf8');
  // Die Liste schreibt Zeilenenden als \n; eine Datei mit \r\n bekommt sie so.
  const [a, n] = text.includes('\r\n') ? [alt.replaceAll('\n', '\r\n'), neu.replaceAll('\n', '\r\n')] : [alt, neu];
  const wie = text.split(a).length - 1;
  if (wie !== 1) {
    console.log(`VERALTET ${name}: Stelle ${wie}-mal in ${datei}`);
    schlecht++;
    continue;
  }
  offen = { pfad: datei, original };
  writeFileSync(SICHERUNG, JSON.stringify({ pfad: datei, bytes: original.toString('base64') }));
  try {
    writeFileSync(datei, text.replace(a, n));
    const bleibt = await gruen(spec, titel);
    console.log(`${bleibt ? 'GRÜN' : 'ROT '} ${name}`);
    if (bleibt) schlecht++;
  } finally {
    zurueck();
  }
}
console.log(schlecht ? `${schlecht} Gegenproben ohne Wirkung oder veraltet` : 'alle Gegenproben rot');
process.exitCode = schlecht ? 1 : 0;
