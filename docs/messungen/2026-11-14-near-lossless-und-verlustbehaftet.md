---
title: Near-lossless und verlustbehaftetes WebP
description: Was near-lossless und verlustbehaftetes WebP an 409 Kacheln der Testwelt brächten, Bytes, Zeit und Abweichung der Pixel, in der CI; near-lossless spart 19 bis 49 % bei höchstens 1 bis 4 Stufen Abweichung je Kanal, verlustbehaftet 44 bis 69 % bei 34 bis 36 dB; nur Zahlen, 0004 gilt.
date: 2026-11-14
commits: [de436f3a]
code:
  - renderer/src/render/tiles.rs
---

# Near-lossless und verlustbehaftetes WebP

Mit near-lossless werden kompakte Kacheln 19 bis 49 % kleiner, und kein
Kanal eines Pixels weicht um mehr als 1, 2 oder 4 Stufen ab, je nach
Einstellung. Verlustbehaftet werden sie 44 bis 69 % kleiner; einzelne Pixel
weichen dann um mehr als 100 Stufen ab, die PSNR liegt bei 34 bis 36 dB.
Die Seite nennt nur Zahlen.
[0004](../entscheidungen/0004-webp-verlustfrei.md) gilt weiter.

## Aufbau

- **Frage:** Was läge auf dem Tisch, wenn Kacheln nicht mehr Pixel für
  Pixel stimmen müssten? Gemessen sind Bytes, Zeit und wie weit die Pixel
  abweichen. Wie es aussieht, sagt die Messung nicht.
- **Varianten:**

  | Variante | Einstellung in `WebPConfig` |
  |---|---|
  | heute | kompakt wie `kodiere` in [`tiles.rs`](../../renderer/src/render/tiles.rs) |
  | near-lossless 80, 60, 40 | wie heute, dazu `near_lossless`; libwebp rundet Pixel vor dem Packen, das Format bleibt das verlustfreie mit demselben Decoder |
  | verlustbehaftet 95, 90 | VP8 mit dieser `quality`, method 4, Alpha verlustfrei; je mit und ohne `use_sharp_yuv` |

- **Abweichung:**
  - Gezählt wird an sichtbaren Pixeln, also dort, wo das Original Alpha
    über 0 hat; für Alpha selbst an allen Pixeln.
  - Geändert heisst: Wenigstens ein Kanal weicht ab.
  - Die grösste Abweichung steht je Kanal, in Stufen von 0 bis 255.
  - PSNR über R, G und B der sichtbaren Pixel, nicht mit Alpha gewichtet;
    dazu die PSNR der schlechtesten einzelnen Kachel.
- **Messcode:** der Tag `versuch/libwebp-leiter`, `de436f3a`, Test
  `probe_verlust`. Er packt selbst mit libwebp; für „heute“ prüft er an
  jeder Kachel, dass dieselben Bytes entstehen wie mit `encode_webp`.
  Gemessen am Stand `f99228f6` eines Probezweigs; nicht gemergt.
- **Kacheln:** die 409 Kacheln der Testwelt aus dem zweiten Lauf von
  [Farb-Cache fest statt gesucht](2026-11-13-farbcache-fest-statt-suchen.md),
  182 von oben und 227 im iso.
- **Arbeit:** ein Thread; Packen und Dekodieren mit libwebp.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build, ein geteilter
  Rechner.

## Ablauf

Am 14.11.2026, ein Lauf des Workflows. Die Zahlen sind die Ausgabe des
Tests `probe_verlust` im Log.

- Alle acht Varianten im selben Prozess im Wechsel, je Gruppe drei Runden;
  das Ganze dreimal. Für die Zeiten der Median der drei.
- Bytes und Abweichung sind in allen drei gleich.
- Die Zeiten der drei Durchgänge liegen gegen heute über alle Kacheln eines
  Baums höchstens 0,5 Prozentpunkte auseinander, je Gruppe höchstens 1,8.

## Ergebnis

### Von oben

182 Kacheln, 9 058 560 sichtbare Pixel. Heute: 8 088 532 Byte, Packen
7,71 ms und Dekodieren 0,63 ms je Kachel.

| Variante | Bytes | Packen | Dekodieren, ms | geänderte Pixel | grösste Abweichung R, G, B, A | PSNR, dB | schlechteste Kachel, dB |
|---|---|---|---|---|---|---|---|
| near-lossless 80 | −19,2 % | +7,0 % | 0,57 | 84,8 % | 1, 1, 1, 0 | 51,2 | 50,3 |
| near-lossless 60 | −33,9 % | +13,9 % | 0,49 | 95,4 % | 2, 2, 2, 0 | 46,6 | 46,2 |
| near-lossless 40 | −43,7 % | +25,7 % | 0,45 | 96,8 % | 4, 4, 4, 0 | 41,4 | 40,1 |
| verlustbehaftet 95 | −56,1 % | −17,9 % | 1,28 | 99,0 % | 123, 59, 121, 0 | 34,6 | 29,8 |
| verlustbehaftet 95, `use_sharp_yuv` | −52,1 % | +20,2 % | 1,36 | 99,1 % | 96, 61, 120, 0 | 34,5 | 30,1 |
| verlustbehaftet 90 | −69,0 % | −25,5 % | 1,03 | 99,3 % | 122, 58, 122, 0 | 33,9 | 29,4 |
| verlustbehaftet 90, `use_sharp_yuv` | −65,3 % | +12,4 % | 1,11 | 99,3 % | 96, 58, 124, 0 | 34,0 | 29,9 |

### Im iso mit Cinematic

227 Kacheln, 12 475 196 sichtbare Pixel. Heute: 11 293 004 Byte, Packen
8,10 ms und Dekodieren 0,72 ms je Kachel.

| Variante | Bytes | Packen | Dekodieren, ms | geänderte Pixel | grösste Abweichung R, G, B, A | PSNR, dB | schlechteste Kachel, dB |
|---|---|---|---|---|---|---|---|
| near-lossless 80 | −24,5 % | +7,4 % | 0,60 | 83,7 % | 1, 1, 1, 1 | 51,4 | 50,4 |
| near-lossless 60 | −40,0 % | +16,3 % | 0,52 | 95,3 % | 2, 2, 2, 2 | 46,7 | 45,9 |
| near-lossless 40 | −49,3 % | +25,4 % | 0,46 | 96,8 % | 4, 4, 4, 4 | 41,4 | 40,3 |
| verlustbehaftet 95 | −48,7 % | −11,2 % | 1,52 | 99,3 % | 96, 41, 119, 0 | 35,5 | 30,5 |
| verlustbehaftet 95, `use_sharp_yuv` | −43,9 % | +25,5 % | 1,64 | 99,3 % | 89, 53, 116, 0 | 36,2 | 31,3 |
| verlustbehaftet 90 | −63,8 % | −20,3 % | 1,19 | 99,6 % | 92, 42, 123, 0 | 34,7 | 30,2 |
| verlustbehaftet 90, `use_sharp_yuv` | −59,2 % | +16,7 % | 1,29 | 99,6 % | 94, 56, 116, 0 | 35,3 | 31,0 |

### Je Gruppe

Bytes gegen heute; verlustbehaftet ohne `use_sharp_yuv`.

| Gruppe | Kacheln | near-lossless 80 | 60 | 40 | verlustbehaftet 95 | 90 | PSNR bei 90, dB |
|---|---|---|---|---|---|---|---|
| von oben, Wasser | 36 | −24 % | −43 % | −51 % | −67 % | −79 % | 38,3 |
| von oben, Wüste | 30 | −18 % | −33 % | −42 % | −62 % | −74 % | 36,3 |
| von oben, Wald | 36 | −19 % | −30 % | −41 % | −44 % | −58 % | 31,2 |
| von oben, Rand der Welt | 12 | −16 % | −29 % | −38 % | −58 % | −73 % | 35,3 |
| von oben, Pyramide | 68 | −17 % | −31 % | −43 % | −55 % | −68 % | 33,5 |
| iso, Wasser | 36 | −28 % | −45 % | −51 % | −54 % | −72 % | 37,5 |
| iso, Wüste | 36 | −23 % | −38 % | −44 % | −43 % | −63 % | 37,1 |
| iso, Wald | 36 | −21 % | −37 % | −49 % | −40 % | −53 % | 30,9 |
| iso, Rand der Welt | 46 | −20 % | −33 % | −45 % | −29 % | −47 % | 35,7 |
| iso, Pyramide | 73 | −27 % | −44 % | −54 % | −64 % | −75 % | 35,8 |

## Schluss

- **Near-lossless** bleibt im verlustfreien Format; Browser und Mod lesen
  es mit dem Decoder von heute.
  - Die Abweichung hat eine feste Grenze: 1, 2 oder 4 Stufen je Kanal. Sie
    trifft 84 bis 97 % der sichtbaren Pixel.
  - Im iso weicht auch Alpha so weit ab, in den Gruppen Pyramide und Rand
    der Welt; von oben nicht.
  - Packen dauert 7 bis 26 % länger, Dekodieren mit libwebp wird schneller.
- **Verlustbehaftet** spart mehr, 44 bis 69 %.
  - Einzelne Pixel weichen in einem Kanal um bis zu 124 Stufen ab. Das
    Format speichert die Farbe in halber Auflösung.
  - Im Wald ist die PSNR am niedrigsten, um 31 dB. Am wenigsten spart der
    Rand der Welt im iso.
  - Dekodieren mit libwebp dauert rund doppelt so lang wie heute.
  - Alpha bleibt genau.
- **`use_sharp_yuv`** macht die Kacheln 9 bis 13 % grösser als ohne und das
  Packen 41 bis 51 % langsamer; die PSNR ändert sich um −0,1 bis +0,7 dB.
- **Nur Zahlen.** Die Messung ändert nichts an
  [0004](../entscheidungen/0004-webp-verlustfrei.md).
- **Grenzen:** nur die Testwelt, je Gruppe ein Ort, ein Lauf; Zeiten von
  libwebp auf einem geteilten Rechner unter Linux, nicht aus Browser oder
  Mod. Nicht gemessen: wie es aussieht, und was es für die Pyramide heisst,
  wenn sie aus schon geänderten Kacheln entsteht.
