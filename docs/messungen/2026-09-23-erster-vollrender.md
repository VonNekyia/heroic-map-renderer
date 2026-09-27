---
title: Erster Vollrender der grossen Welt
description: Vorlauf, Kachelzahl, Rate und die daraus hochgerechnete Dauer des ersten Vollrenders der grossen Welt, früher Stand vor allen Umbauten.
date: 2026-09-23
commits: [4fbb51c]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
---

# Erster Vollrender der grossen Welt

**Überholt.** Diese Zahlen gelten für einen frühen Stand vor allen Umbauten
von #10 bis #18. Den aktuellen Stand nennt
[Was ein Lauf kostet](../benutzung/kosten.md), „Die grosse Welt“; der erste
ganz gemessene Vollrender steht in
[2026-09-26, Vollrender mit #11](2026-09-26-vollrender-mit-11.md). Die „120“
unten sind KiB je Kachel, nicht GB.

Auf 24 Threads schaffte der Renderer 44 Kacheln/s; für 2,5 Millionen
Basiskacheln wären das knapp 16 Stunden gewesen.

## Aufbau

- Welt: die grosse Welt, 2,5 Millionen Chunks, 30 GiB, scale 32.
- Stand: #8 vor seinem Merge; die Zahlen stehen seit `4fbb51c` (23.09.,
  00:46) im README. Welcher Commit genau lief und wann, ist nicht
  festgehalten.
- 24 Threads, ohne Grafikkarte; die Maschine war nicht frei, nur 9 Kerne
  standen dem Lauf zur Verfügung.

## Ablauf

Ein Lauf über die ganze Welt; er endete nach rund 321 000 Basiskacheln,
lange vor dem Ende der Basis. Gemessen sind Vorlauf, Kachelzahl, Grösse je
Kachel und Rate, hochgerechnet Grösse und Dauer der ganzen Basis.

## Ergebnis

| | Wert | |
|---|---|---|
| Vorlauf | 259 s | gemessen |
| Basiskacheln | 2 504 461 | gemessen, vom Vorlauf |
| je Kachel | rund 120 KiB | gemessen |
| Basis zusammen | ~300 GB | hochgerechnet |
| Rate | 44 Kacheln/s auf 24 Threads, nur 9 Kerne frei | gemessen |
| Basisstufe | knapp 16 Stunden | hochgerechnet |

## Schluss

Das ist keine Eigenschaft der Welt, sondern des Renderers: Eine Kachel ist
ein schräger Schnitt durch die volle Bauhöhe, rund 320 000 Blockpositionen,
gut hundert Chunks, und neun von zehn nicht-leeren Blöcken liegen unter der
Oberfläche. Daraus wurden #9 (Pyramide aus Kacheln, native Stufen auf
Wunsch) und #10 (Bitmasken, Cache, mimalloc), siehe
[2026-09-23, Phasen je Kachel](2026-09-23-phasen-je-kachel.md).
