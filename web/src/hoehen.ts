/**
 * Die Höhenkarten des Renderers, geteilt von Koordinaten und Ebenen.
 * Siehe docs/benutzung/map-json.md, „Höhen“.
 */
import { region, REGION, type Block } from './pick';

/**
 * `map.json`, `trees.json` und Höhen fragt der Browser jedes Mal beim Server
 * nach, statt sie aus dem Cache zu nehmen; unverändert kommt 304. So zeigt
 * die Seite nach einem neuen Lauf seinen Stand, gleich welche Header der
 * Server setzt. Siehe docs/frontend.md, „Ausliefern“.
 */
export const FRISCH: RequestInit = { cache: 'no-cache' };

/** In einer Höhenkarte: keine Zelle mit Block, oder kein fertiger Chunk. */
export const LEER = -32768;

/** Eine Höhenkarte: zlib, darin n × n Zellen, je i16 little-endian. */
async function ladeKarte(path: string, n: number): Promise<Int16Array | null> {
  const response = await fetch(path, FRISCH);
  // Keine Datei heisst kein Chunk. Ein Server, der auf unbekannte Pfade die
  // index.html ausliefert, meint dasselbe.
  if (response.status === 404 || response.headers.get('content-type')?.startsWith('text/html')) {
    return null;
  }
  if (!response.ok || response.body === null) {
    throw new Error(`${path}: ${response.status} ${response.statusText}`);
  }
  const daten = await new Response(
    response.body.pipeThrough(new DecompressionStream('deflate')),
  ).arrayBuffer();
  if (daten.byteLength !== n * n * 2) {
    throw new Error(`${path}: ${daten.byteLength} Byte statt ${n * n * 2}`);
  }
  // ponytail: liest in der Byte-Reihenfolge der Maschine, also nur auf
  // little-endian richtig; auf big-endian bräuchte es eine DataView.
  return new Int16Array(daten);
}

/**
 * Die Höhenkarten der Regionen, je Zelle aus `zelle` × `zelle` Spalten die
 * Höhe dessen, was man sieht. Siehe docs/benutzung/map-json.md.
 */
export function hoehen(base: string, muster: string, zelle: number) {
  const karten = new Map<string, Int16Array | null>();
  const unterwegs = new Map<string, Promise<Int16Array | null>>();

  const ladeRegion = (rx: number, rz: number): Promise<Int16Array | null> => {
    const name = `${rx}.${rz}`;
    if (karten.has(name)) return Promise.resolve(karten.get(name)!);
    let laden = unterwegs.get(name);
    if (laden === undefined) {
      const path = `${base}/${muster.replace('{x}', String(rx)).replace('{z}', String(rz))}`;
      laden = ladeKarte(path, REGION / zelle)
        .catch((error: unknown) => {
          console.error(error);
          return null;
        })
        .then((karte) => {
          unterwegs.delete(name);
          karten.set(name, karte);
          // ponytail: verdrängt die älteste statt der am längsten
          // ungenutzten; eine verdrängte lädt neu, mit Nachfrage beim Server
          // (no-cache, meist 304) und neuem Entpacken.
          if (karten.size > 64) karten.delete(karten.keys().next().value!);
          return karte;
        });
      unterwegs.set(name, laden);
    }
    return laden;
  };

  return {
    /** Lädt die Karten aller Regionen, durch deren Spalten die Würfel gehen. */
    lade: async (bloecke: readonly Block[]): Promise<void> => {
      const regionen = new Map<string, [number, number]>();
      for (const [x, , z] of bloecke) {
        const { rx, rz } = region(x, z, zelle);
        regionen.set(`${rx}.${rz}`, [rx, rz]);
      }
      await Promise.all([...regionen.values()].map(([rx, rz]) => ladeRegion(rx, rz)));
    },
    /**
     * Die Karte einer Region, `null` ohne Datei. Wer sie hält, behält sie,
     * auch wenn der Cache sie verdrängt; so reichen Formen über mehr
     * Regionen, als er fasst.
     */
    karte: ladeRegion,
    /** Die Höhe der Zelle einer Spalte, `undefined` für leer oder nicht geladen. */
    hoehe: (x: number, z: number): number | undefined => {
      const { rx, rz, i } = region(x, z, zelle);
      const wert = karten.get(`${rx}.${rz}`)?.[i];
      return wert === undefined || wert === LEER ? undefined : wert;
    },
  };
}


/** Ein Cache der Höhenkarten, wie `hoehen` ihn gibt. */
export type Hoehenkarten = ReturnType<typeof hoehen>;
