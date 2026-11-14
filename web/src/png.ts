/**
 * Liest die Pixel eines PNG ohne Leinwand: nur RGBA mit 8 Bit, ohne
 * Interlace, wie die Felder der Nadel. Kein allgemeiner PNG-Leser, keine
 * Prüfsummen. Siehe docs/frontend.md, „Leinwände“.
 */
export async function lesePng(adresse: string): Promise<{ breite: number; hoehe: number; pixel: Uint8ClampedArray }> {
  const antwort = await fetch(adresse);
  if (!antwort.ok) throw new Error(`${adresse}: ${antwort.status}`);
  const d = new Uint8Array(await antwort.arrayBuffer());
  const sicht = new DataView(d.buffer);
  const teile: Uint8Array[] = [];
  let [breite, hoehe] = [0, 0];
  for (let o = 8; o + 8 <= d.length; ) {
    const laenge = sicht.getUint32(o);
    const art = String.fromCharCode(...d.subarray(o + 4, o + 8));
    const inhalt = d.subarray(o + 8, o + 8 + laenge);
    if (art === 'IHDR') {
      [breite, hoehe] = [sicht.getUint32(o + 8), sicht.getUint32(o + 12)];
      // Bittiefe 8, Farbtyp 6 (RGBA), ohne Interlace.
      if (inhalt[8] !== 8 || inhalt[9] !== 6 || inhalt[12] !== 0) throw new Error(`${adresse}: nur RGBA mit 8 Bit ohne Interlace`);
    }
    if (art === 'IDAT') teile.push(inhalt);
    o += 12 + laenge;
  }
  const roh = new Uint8Array(await new Response(new Blob(teile as BlobPart[]).stream().pipeThrough(new DecompressionStream('deflate'))).arrayBuffer());
  const zeile = breite * 4;
  if (!breite || roh.length !== hoehe * (zeile + 1)) throw new Error(`${adresse}: keine lesbaren Pixel`);
  const pixel = new Uint8ClampedArray(hoehe * zeile);
  for (let y = 0; y < hoehe; y++) {
    const filter = roh[y * (zeile + 1)]!;
    for (let x = 0; x < zeile; x++) {
      const i = y * zeile + x;
      const a = x >= 4 ? pixel[i - 4]! : 0;
      const b = y > 0 ? pixel[i - zeile]! : 0;
      const c = x >= 4 && y > 0 ? pixel[i - zeile - 4]! : 0;
      const p = a + b - c;
      const [pa, pb, pc] = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)];
      const vorher = [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][filter];
      if (vorher === undefined) throw new Error(`${adresse}: Filter ${filter} unbekannt`);
      pixel[i] = (roh[y * (zeile + 1) + 1 + x]! + vorher) & 255;
    }
  }
  return { breite, hoehe, pixel };
}
