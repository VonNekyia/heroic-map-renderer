/**
 * Prüfen, was aus den Dateien der Ebenen kommt: Jede Ansicht nimmt nur, was
 * die Form hat, und übergeht den Rest. Siehe docs/benutzung/ebenen.md,
 * „Gemeinsame Felder“.
 */
import type { Punkt } from './gelaende';

export const FARBE = /^#[0-9a-f]{6}([0-9a-f]{2})?$/i;

export const istText = (wert: unknown, max: number): wert is string => typeof wert === 'string' && wert.length > 0 && wert.length <= max;
export const istZahl = (wert: unknown): wert is number => typeof wert === 'number' && Number.isFinite(wert);
export const istObjekt = (wert: unknown): wert is Record<string, unknown> => typeof wert === 'object' && wert !== null && !Array.isArray(wert);
export const farbe = (wert: unknown): string | undefined => (typeof wert === 'string' && FARBE.test(wert) ? wert : undefined);

/** Ein Punkt `[x, z]` aus zwei endlichen Zahlen. */
export const punkt = (wert: unknown): Punkt | undefined =>
  Array.isArray(wert) && wert.length === 2 && wert.every(istZahl) ? [wert[0] as number, wert[1] as number] : undefined;

/** Eine Liste von Punkten mit `min` bis `max` Einträgen, jeder ein Punkt. */
export function punkte(wert: unknown, min: number, max: number): Punkt[] | undefined {
  if (!Array.isArray(wert) || wert.length < min || wert.length > max) return undefined;
  const aus = wert.map(punkt);
  return aus.every((p) => p !== undefined) ? aus : undefined;
}
