---
title: "0025: Streifen und ein Cache je Thread"
description: Warum die Kacheln in Streifen Zeile für Zeile laufen, jeder Thread Cache und Zeichner behält und erst ab einem Mindestrest stiehlt.
status: gilt
date: 2026-09-27
issues: [9, 10, 12]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
---

# 0025: Streifen und ein Cache je Thread

## Anlass

In #9 entstand der Chunk-Cache je Kachel neu, und benachbarte Kacheln
dekodierten dieselben Chunks 50- bis 70-mal. #10 hielt ihn je Stapel
aufeinanderfolgender Kacheln. Danach lud noch jede Kachel Spalte für Spalte
die Chunks am unteren Rand ihrer ganzen Breite neu, und jeder Stapel fing
kalt an: 7,2 Chunks je Kachel auf einem Thread.

## Entscheidung

Die Kacheln laufen in Streifen, Zeile für Zeile, bei scale 32 bis zu acht
Kacheln breit (`streifenbreite`). `verteile` gibt jedem Thread ein
zusammenhängendes Stück; wer fertig ist, nimmt die hintere Hälfte des
grössten, das noch übrig ist, aber erst ab vier Streifenbreiten Rest. Jeder
Thread behält Chunk-Cache und Zeichner über den ganzen Lauf. Nach einem
Fehler nimmt kein Thread mehr etwas. Siehe
[Der Weg einer Kachel](../renderer/renderpfad.md),
„Streifen und Cache je Thread“.

## Verworfene Alternativen

- **Der Verteiler der ersten Fassung von #12**, ein Paket nach dem anderen
  im eigenen Streifen und Helfer dazwischen, und **Rayon mit einem Cache je
  Thread**, weil Rayon die Reihe schon beim Verteilen in viele kleine Stücke
  zerteilt: Bei 1024 Kacheln auf 24 Threads luden sie 17 und 20 Chunks je
  Kachel, der Verteiler 8,2.
- **Stehlen ohne Schwelle:** 13 Chunks je Kachel bei 1024 Kacheln, denn wer
  stiehlt, fängt kalt an.
- **Ein Cache je Kachel oder je Stapel**, wie in #9 und #10: siehe Anlass.
- **Ein Cache für alle Threads.** Er wäre eine Sperre im Renderpfad.

## Folgen

- Chunks je Kachel: 3,0 statt 7,2 auf einem Thread, 2,0 statt 7,0 über 65
  536 Kacheln, siehe
  [2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md).
- Mehr Speicher: Jeder Thread hält eine Zeile seines Streifens im Cache,
  gemessen höchstens 430 bis 520 Chunks bei scale 32.
- Der Cache räumt nach Verfallsdatum je Kachel statt echtem LRU
  (`ponytail:` an `CACHE_CHUNKS`); das reicht, solange die Kacheln in
  Streifen kommen.
- In Streifen mit einer Zweierpotenz als Breite liegen Geschwister ab zwei
  Spalten beieinander; `--pyramid` baut ihre Elternkachel selten zweimal.
