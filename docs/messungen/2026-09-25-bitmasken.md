---
title: Bitmasken gegen Block für Block
description: Der Umbau aus #10 auf den Regeln von #9, gegen #9 gemessen, einfädig und auf 12 und 24 Threads, dazu die grosse Welt und der Nether.
date: 2026-09-25
commits: [eb67d26, 93faf81, 1608d09]
code:
  - renderer/src/render/metatile.rs
---

# Bitmasken gegen Block für Block

Einfädig ist der Umbau 10-mal so schnell wie #9, auf 24 Threads 5,5-mal:
8,6 statt 90,9 ms je Kachel, 510 statt 93 Kacheln/s. Kein Pixel ist anders.

## Aufbau

- Welt: die Testwelt um (0, 0), scale 32, einfädig ein 4096er-Ausschnitt,
  mit 12 und 24 Threads ein 8192er. Das sind dieselben Grössen wie in
  [2026-09-23, Phasen je Kachel](2026-09-23-phasen-je-kachel.md), dort aber
  auf der grossen Welt.
- Stände: #9 Block für Block (`eb67d26`), diese PR vor dem Hochziehen auf
  #9 (`93faf81`), danach (`1608d09`). Gemessen am Stand `1608d09`, vor der
  Lava-Regel (`c354035`) und dem strengen Umriss (`c4dcfd8`).
- Jede geschriebene Kachel ging durch den Echtzeitschutz.

## Ablauf

Am selben Tag nacheinander, jeweils das beste von drei Läufen.

## Ergebnis

| | #9, Block für Block | vor dem Hochziehen | nach dem Hochziehen |
|---|---|---|---|
| ein Kern, 4096er-Ausschnitt | 90,9 ms je Kachel | 8,7 ms | 8,6 ms |
| 12 Threads, 8192er-Ausschnitt | 77 Kacheln/s | 502 | 424 |
| 24 Threads, 8192er-Ausschnitt | 93 Kacheln/s | 551 | 510 |

- Mit 12 und 24 Threads schwankten die Läufe an diesem Tag um bis zu zehn
  Prozent.
- Auf dem Land der grossen Welt schaffte der Stand je nach Gegend 443 bis
  522 Kacheln/s statt 53 bis 86; die Basisstufe hätte damit rund anderthalb
  Stunden gebraucht statt neun bis zehn. Wie oft dort gemessen wurde, ist
  nicht festgehalten.
- Im Nether der grossen Welt sank die Zeit je Kachel bei scale 32 einfädig
  um gut ein Viertel, seit auch Lava im Inneren wegfällt (`c354035`).

Byte für Byte gleich mit #9: neun Ausschnitte der Testwelt und der grossen
Welt, mit und ohne native Stufen, bei scale 32, 16 und 12, dazu zwei Bilder
aus `--render`, zuletzt an `f9cf0b0`. Nur `map.json` weicht ab, dort das
Salz der Kennung. Dazu fünf Ausschnitte bei scale 32, 8 und 4 gegen die
Referenz ohne Abkürzung, 42 Millionen Pixel, keiner anders.

## Schluss

Die Regeln von #9, Deckung Pixel für Pixel, der Boden unter Lava, Streifen
über niedrigerem Wasser, kosten gegenüber der ersten Fassung auf 24 Threads
510 statt 551 Kacheln/s; der Umbau bleibt 5,5-mal so schnell wie #9,
einfädig 10-mal. Entscheidung:
[0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md).
