---
title: "0028: libwebp statt des Encoders aus image"
description: Warum die Kacheln libwebp verlustfrei auf Stufe 0 packt, direkt über libwebp-sys, ohne SIMD-Features und ohne Schalter für die Stufe.
status: gilt
date: 2026-09-27
issues: [13, 16]
code:
  - renderer/src/render/tiles.rs
  - renderer/Cargo.toml
  - renderer/deny.toml
---

# 0028: libwebp statt des Encoders aus image

In der Einstellung abgelöst durch
[0090](0090-webp-mit-quality-75.md): libwebp packt mit method 0 und
quality 75 statt auf Stufe 0.

## Anlass

#13: Der Vollrender der grossen Welt mit #11 war 354 GB gross. Der
Encoder aus `image` packt verlustfrei, aber ohne Palette, Farbcache und
Rückverweise, obwohl mehr als die Hälfte der Basiskacheln höchstens 256
Farben hat.

## Entscheidung

`encode_webp` packt über `libwebp-sys` 0.14.4 mit gebündeltem libwebp 1.6.0:
`WebPConfigLosslessPreset(&config, 0)`, `exact = 1`, `thread_level = 0`. Die
Bytes landen über einen eigenen Writer direkt in einem `Vec`. Das `unsafe`
steht in drei Blöcken mit je einem SAFETY-Kommentar. Kein Schalter für die
Stufe. Die Kacheln bleiben verlustfrei, siehe
[0004](0004-webp-verlustfrei.md).

## Verworfene Alternativen

- **Die Crate `webp` 0.3.** Sie legt nur eine Schicht über `libwebp-sys`;
  für `exact` braucht man trotzdem `WebPConfig`, und mit ihrem
  Standard-Feature zieht sie noch einmal `image` dazu. Direkt sind es rund
  40 Zeilen und eine Abhängigkeit weniger.
- **Höhere Stufen.** Auf 128 Kacheln der grossen Welt, bezogen auf den
  Encoder aus `image`: Stufe 0 bringt 0,39 der Grösse bei 2,3 ms je Kachel,
  Stufe 1 0,36 bei 5,5 ms, Stufe 6 0,29 bei 49 ms, siehe
  [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md).
- **Die SIMD-Features `sse41` und `avx2` der Crate.** Unter MSVC schaltet
  libwebp SSE2, SSE4.1 und AVX2 selbst ein und wählt zur Laufzeit. Die
  Features bräuchte es nur mit gcc oder clang, und dort setzten sie
  `-msse4.1` und `-mavx2` für alle Dateien: Das Binär liefe dann nur noch
  auf Prozessoren mit diesen Befehlen.
- **Ein eigener ARGB-Puffer statt `WebPPictureImportRGBA`:** ändert nichts
  (3678 gegen 3595 Kacheln/s).

## Folgen

- Die Kacheln werden ein Drittel so gross, Pixel für Pixel gleich.
- Das Kodieren kostet auf einem Thread 2,2 statt 0,6 ms je Kachel.
- Zum Bauen braucht es einen C-Compiler. libwebp steht unter BSD-3-Clause:
  `deny.toml` bekommt eine Klarstellung `MIT AND BSD-3-Clause` mit dem Hash
  von `vendor/COPYING`, und BSD-3-Clause gilt nur als Ausnahme für diese
  Crate.
- libwebp allokiert über `malloc` der C-Laufzeit, siehe
  [0029](0029-segment-heap-fuer-libwebp.md).
- Der erste CI-Lauf mit neuer `Cargo.lock` dauert länger; später liegt
  libwebp im Cache.
