---
title: Wasser im Licht des Spiels
description: Was Licht je Block unter Wasser kostet, an Rate, Laufzeit, Grösse und Sprite-Tabelle, gegen master auf der grossen Welt, dazu die Hochrechnung für einen Vollrender.
date: 2026-09-27
commits: [9479471, 2654309, 9752261, da26cec, 088cc53]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/gpu.wgsl
---

# Wasser im Licht des Spiels

Die Basis ist ohne Karte 6 bis 10 % langsamer als master, mit Karte
gleichauf; die schnellere Sprite-Tabelle, 5,3 statt 11,8 s, gleicht das im
ganzen Lauf zum Teil aus. Die Kacheln werden grösser, weil man ins Wasser
sieht: über Land mit viel Wasser ×1,25, über Ozean ×1,56.

## Aufbau

- Welt: die grosse Welt, scale 32: ein Ausschnitt mit 65 536 Basiskacheln,
  Ozean mit 15 682, ein 8192er-Ausschnitt fast nur Land. Dazu die Testwelt
  wie in [Was ein Lauf kostet](../benutzung/kosten.md).
- Stände: master mit #16 (`9479471`), die erste Runde von #17 (`2654309`),
  der Stand danach (`9752261`), die zweite Runde (`da26cec`) und der Kopf
  (`088cc53`).
- 24 Threads, wo nichts anderes steht; der Kachelordner war vom
  Echtzeitschutz ausgenommen.

## Ablauf

Abwechselnd, jeder Lauf frisch. Über Ozean streuten die Läufe zweigipflig,
dieselbe Fassung mal mit 1800 bis 2300, mal mit 1200 bis 1400 Kacheln/s;
dort stehen deshalb vier Läufe je Fassung und das beste.

## Ergebnis

| | master | erste Runde `2654309` | `9752261` |
|---|---|---|---|
| 65 536 Basiskacheln: Basis ohne Karte, Kacheln/s | 2054, 1982 | 1867, 1912 | 1852, 1862 |
| dito, mit Karte | 2260, 2088 | 2221, 2078 | 2227, 2157 |
| dito, ganzer Lauf ohne Karte | 57,3 / 59,6 s | 60,2 / 59,1 s | 60,8 / 60,4 s |
| dito, mit Karte | 54,3 / 57,0 s | 53,7 / 56,0 s | 54,2 / 55,2 s |
| dito, Basis und Pyramide | 2,92 GB | 3,65 GB | 3,66 GB |
| dito, Dateien mit anderen Pixeln als master | – | 45 745 von 87 517 | 45 743 von 87 517 |
| Ozean, 15 682 Basiskacheln: Basis ohne Karte, bestes von vier (die übrigen) | 2095 (2053, 2033, 2031) | 1973 (1948, 1890, 1827) | 1928 (1870, 1782, 1397) |
| dito, mit Karte | 2278 (2263, 1846, 1666) | 2080 (1938, 1875, 1860) | 2241 (2207, 2204, 1219) |
| dito, ganzer Lauf ohne Karte, bester von vier | 14,0 s | 14,0 s | 14,2 s |
| dito, mit Karte | 13,4 s | 13,7 s | 13,1 s |
| dito, Basis und Pyramide | 0,50 GB | 0,77 GB | 0,78 GB |
| 8192er Land, ein Thread, bestes von drei, je Kachel | 5,49 ms | 5,68 ms | 5,65 ms |
| dito, 24 Threads, bestes von drei, Kacheln/s | 1445 | 1416 | 1405 |
| dito, ganzer Lauf | 2,0 / 2,0 / 2,0 s | 1,7 / 1,7 / 2,2 s | 2,1 / 1,8 / 1,7 s |
| dito, Basis und Pyramide | 78 MB | 84 MB | 84 MB |

Nach der zweiten Runde, derselbe Ausschnitt mit 65 536 Basiskacheln ohne
Karte, abwechselnd, drei Läufe je Fassung:

| | `da26cec` | `088cc53` |
|---|---|---|
| Basis ohne Karte, Kacheln/s | 1917, 1803, 1846 | 1827, 1874, 1728 |
| dito, ganzer Lauf | 62,2 / 61,8 / 60,5 s | 62,3 / 60,9 / 63,1 s |
| dito, Basis und Pyramide | 3,66 GB | 3,66 GB |
| dito, Dateien mit anderen Pixeln als `da26cec` | – | 1475 von 87 517 |

Im Mittel 1855 gegen 1810 Kacheln/s, 2 % weniger; zwischen den Läufen
derselben Fassung streut es um 6 bis 8 %.

Aus der ersten Messung, gegen master:

| | master | #17 |
|---|---|---|
| Sprite-Tabelle der ganzen Welt, scale 32 | 56 761 Sprites in 11,8 / 11,7 s | 26 341 in 5,4 / 5,3 s |
| dito, Spitze bis dahin | 0,61 GiB | 0,61 GiB |
| 65 536 Basiskacheln noch einmal kodiert, ein Thread | 2,35 / 2,27 ms je Kachel | 2,19 / 2,16 ms |

- Die Tabelle halbiert sich: Die Fassungen je Tiefe fallen weg, und die
  Messung der Deckung rasterte bei jedem scale ausser 32 jedes Modell ein
  zweites Mal.
- Am Kodieren liegt der Verlust der Basis nicht: libwebp kodiert die
  Kacheln von #17 schneller als die von master, obwohl sie grösser sind.
- Eine Abkürzung, die an Land gar nicht zählt, brachte auf einem Thread
  nichts, 5,59 statt 5,56 ms. Vier statt fünf Wörter je Instanz brachten auf
  der Karte nichts, 2246 und 2187 Kacheln/s bei gleichen Pixeln.
- Wo der Rest bleibt, ist nicht aufgeteilt; in Frage kommen der Blit, der
  unter Wasser jeden Pixel abdunkelt, und die Nachschläge dort.

**Grösse** gegen master: der Ausschnitt mit 65 536 Kacheln ×1,25, reiner
Ozean ×1,56, der 8192er fast nur Land ×1,08, die Testwelt bei scale 32
×1,58.

**Vollrender der grossen Welt, hochgerechnet:** rund 150 GB statt rund
120 GB, je nach Anteil Wasser 140 bis 180 GB. Die Dauer rund eine Stunde,
60 bis 70 min: Der ganze Lauf braucht ohne Karte 1 bis 6 % länger als auf
master, mit Karte gleich lang.

## Schluss

Licht je Block kostet ohne Karte einige Prozent der Basis und nichts mit
Karte; die Kacheln wachsen, weil man jetzt ins Wasser sieht. Geglättet oder
gerundet wird nichts. Entscheidung:
[0030](../entscheidungen/0030-licht-je-block.md).
