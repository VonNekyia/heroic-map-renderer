/** Kameras beim Namen und die Einträge des Renderers, für die Tests. */
import { readFileSync } from 'node:fs';
import type { Block, Projektion } from '../src/pick';

type Punkt = [number, number];

/**
 * Die Zahlen einer Kamera bei einem scale, wie sie der Renderer in
 * `map.json` schreibt: docs/renderer/kamera.md, „Projektion“. Nur der Test
 * kennt Kameras beim Namen; das Frontend liest die Zahlen.
 */
export function kamera(name: string, scale: number): Projektion {
  if (name === 'top-north') return { azimuth: 'north', u: scale, v: scale, y: 0 };
  if (name === 'north-45') return { azimuth: 'north', u: scale, v: scale, y: scale };
  const [w, h] = name === 'top' ? [1, 1] : (name.split(':').map(Number) as [number, number]);
  const y = name === 'top' ? 0 : scale / 2;
  const p: Projektion = { azimuth: 'diagonal', u: scale / 2, v: (scale * h) / (2 * w), y };
  // Nur gültige Paare, wie beim Renderer: jede Ecke auf ganzen Pixeln.
  if (scale % 2 !== 0 || !Number.isInteger(p.v)) throw new Error(`${name} bei ${scale} ungültig`);
  return p;
}

/** Die Einträge des Renderers, aktuell gehalten von einem seiner Tests. */
export const eintraege = JSON.parse(
  readFileSync(new URL('../../renderer/tests/fixtures/projektion.json', import.meta.url), 'utf8'),
) as {
  camera: string;
  direction: string;
  scale: number;
  block: Block;
  pixel: Punkt;
  eben?: 0;
  wand?: 'south' | 'east';
}[];
