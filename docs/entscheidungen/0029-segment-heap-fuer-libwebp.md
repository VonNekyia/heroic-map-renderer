---
title: "0029: Der Segment-Heap für libwebp unter Windows"
description: Warum das Binär unter Windows ein Manifest mit dem Segment-Heap bekommt, statt libwebp auf mimalloc umzubiegen.
status: gilt
date: 2026-09-27
issues: [13, 16]
code:
  - renderer/build.rs
  - renderer/segmentheap.manifest
---

# 0029: Der Segment-Heap für libwebp unter Windows

## Anlass

libwebp holt sich je Kachel rund 2 MB über `malloc` der C-Laufzeit, nicht
über mimalloc wie der Rust-Teil. Der gewöhnliche Heap von Windows gibt sie
beim Freigeben ans System zurück: gut 500 Seitenfehler je Kachel, die sich
auf vielen Threads stauen.

## Entscheidung

`renderer/build.rs` bettet unter MSVC ein Manifest mit dem Segment-Heap ins
Binär ein (`segmentheap.manifest`); der behält den Speicher. Unter Linux
ändert sich nichts. Der Test `binaer_bekommt_den_segment_heap` prüft unter
Windows, dass das Manifest im gebauten Binär steckt.

## Verworfene Alternativen

- **mimalloc auch für libwebp.** Damit kodierte libwebp in einem Benchmark
  aus dem Review noch schneller, auch auf einem Thread. Dafür bräuchte es
  aber eine umgebogene Kopie von `libwebp-sys`, rund 200 Dateien, und die
  müsste man bei jedem Update von libwebp neu umbiegen. Holen könnte
  mimalloc im ganzen Lauf nur den Teil, der beim Kodieren aufs Allokieren
  fällt. Im ganzen Lauf gemessen ist mimalloc nicht.
- **Der gewöhnliche Heap:** siehe Anlass.

## Folgen

- Das Kodieren allein, 1024 Kacheln auf 24 Threads, schafft über Land 4992
  statt 3834 Kacheln/s und über Ozean 6016 statt 4399; auf einem Thread ist
  es knapp 10 % langsamer, siehe
  [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md).
- Im ganzen Lauf über Land geht das in der Streuung unter; über Ozean mit
  Karte dauert er 13,0 statt 13,8 bis 14,0 s. Die Karte ist die Vorgabe,
  und die Welt hat viel Ozean, deshalb bleibt er.
