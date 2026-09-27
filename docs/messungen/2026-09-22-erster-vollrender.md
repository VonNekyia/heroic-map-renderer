---
title: Erster Vollrender der grossen Welt
description: Vorlauf, Kachelzahl, Rate und die daraus hochgerechnete Dauer des ersten Vollrenders der grossen Welt, früher Stand vor allen Umbauten.
date: 2026-09-22
commits: [32b274e, 4fbb51c]
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
- Stand: der Code von `32b274e` aus #9, damals auf #8 vor dessen Merge.
  Gebaut und gestartet am 22.09. um 22:53; `32b274e` hielt den Stand neun
  Minuten später fest, danach kamen nur Tests und README dazu. Abgebrochen
  am 23.09. gegen 1 Uhr. Die Zahlen stehen seit `4fbb51c` (23.09., 00:46)
  im README.
- 24 Threads, ohne Grafikkarte; die Maschine war nicht frei, nur 9 Kerne
  standen dem Lauf zur Verfügung.

## Ablauf

Ein Lauf über die ganze Welt. Nach gut zwei Stunden und 321 400
Basiskacheln wurde er abgebrochen, lange vor dem Ende der Basis, um zuerst
den Renderer schneller zu machen. Gemessen sind Vorlauf, Kachelzahl, Grösse
je Kachel und Rate, hochgerechnet Grösse und Dauer der ganzen Basis.

Woher die Zahlen stammen: Vorlauf, Kachelzahl und die 321 400 aus der
Ausgabe des Laufs, die Rate aus seinem Fortschritt, 180 000 Kacheln nach 68
Minuten, die Grösse je Kachel aus dem Kachelordner nach einer halben
Stunde, 5,7 GiB in 49 528 Dateien.

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
