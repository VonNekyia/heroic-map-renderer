---
title: Weiche Beleuchtung
description: Was die weiche Beleuchtung je Kachel kostet, aufgeteilt nach Ecken, Rechnung je Pixel und Kodieren, dazu die Kosten eines Laufs der ganzen Testwelt je scale und die Hochrechnung für einen Vollrender.
date: 2026-09-27
commits: [713787f, b15d0ab, c36438a, fc62f31]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/gpu.wgsl
---

# Weiche Beleuchtung

Auf einem Thread braucht eine Kachel bei scale 32 mit weicher Beleuchtung
6,13 statt 5,38 ms, 14 % mehr, und sie wiegt ein Fünftel bis ein Viertel
mehr. Hochgerechnet braucht die ganze Testwelt bei scale 32 damit rund
26 GB in 6,5 Minuten. Auf 24 Threads ist die Basis der grossen Welt ohne
Karte 11 bis 14 % langsamer als master, mit Karte 6 bis 7 %; die Fassungen
der zweiten Runde unterscheiden sich dort untereinander nur in der
Streuung.

## Aufbau

- Welten: die Testwelt, der Ausschnitt um (-64, 416) aus
  [Was ein Lauf kostet](../benutzung/kosten.md); die grosse Welt, scale 32,
  ein Ausschnitt mit 65 536 Basiskacheln, Ozean mit 15 682 und ein
  8192er-Ausschnitt fast nur Land.
- Stände: master mit #17 (`713787f`), die erste Fassung von #18
  (`b15d0ab`), die zweite Runde mit Bitebenen (`c36438a`) und dem Faktor bei
  vollem Licht (`fc62f31`).
- 24 Threads ohne Karte, wo nichts anderes steht; der Kachelordner war vom
  Echtzeitschutz ausgenommen.

## Ablauf

Abwechselnd, jeder Lauf frisch.

- **Kosten eines Laufs:** Die Grösse kommt aus dem Ausschnitt mit allen
  nativen Stufen und Pyramide, `--size 8192` bei scale 32, `4096` bei 16 und
  `2048` bei 8, das sind 1600, 400 und 100 Basiskacheln, je ein Lauf. Die
  Dauer kommt aus zwei grösseren Ausschnitten um denselben Punkt, bei scale
  32 mit 2304 und 12 544 Basiskacheln, bei 16 mit 576 und 3136, bei 8 mit 144
  und 784, je neun Läufe in drei Runden, davon der Median. Der Unterschied
  der beiden gibt die Zeit je Basiskachel samt nativen Stufen und Pyramide;
  hochgerechnet auf die Kacheln der ganzen Welt, dazu einmal Vorlauf und
  Sprite-Tabellen, 5,5 s und 6,2 s.
- **Zweite Runde:** ein Thread, 676 Basiskacheln der Testwelt bei scale 32
  ohne native Stufen und ohne Karte, Median aus fünf Läufen samt Spanne,
  mit Schaltern im Code, die die Teile einzeln abschalten.

## Ergebnis

**Die Testwelt**, ohne Karte, mit allen nativen Stufen und Pyramide:

| | master | #18 |
|---|---|---|
| scale 32, 1600 Basiskacheln: Basis, Kacheln/s | 1262, 1266 | 1042, 1075 |
| dito, 6400 Basiskacheln | 1453, 1442 | 1298, 1277 |
| scale 32: je Basiskachel samt Stufen | 1,20 ms | 1,30 ms |
| scale 16 | 2,19 ms | 2,50 ms |
| scale 8 | 4,69 ms | 5,47 ms |
| scale 32: je Kachel | 51 kB | 65 kB |
| scale 16 | 56 kB | 69 kB |
| scale 8 | 62 kB | 73 kB |
| hochgerechnet auf die ganze Testwelt bei scale 32 | ~21 GB in ~6 min | ~26 GB in ~6,5 min |
| Sprite-Tabellen aller vier Stufen | 6,2 s | 6,2 s, 15 096 Sprites wie vorher |

Daraus die Tabelle in [Was ein Lauf kostet](../benutzung/kosten.md): Basis
= Kacheln der Welt × kB je Kachel, native Stufen im Verhältnis des
Ausschnitts, Dauer = Kacheln der Welt × Zeit je Basiskachel + Vorlauf und
Sprite-Tabellen, gerundet auf halbe Minuten.

| `--scale` | Kacheln der Welt | je Kachel | Basis | native Stufen | zusammen | Dauer |
|---|---|---|---|---|---|---|
| 32 | 292 836 | 65 kB | ~19,1 GB | ~6,8 GB | ~26 GB | ~6,5 min |
| 16 | 73 920 | 69 kB | ~5,1 GB | ~1,7 GB | ~6,8 GB | ~3,5 min |
| 8 | 18 951 | 73 kB | ~1,4 GB | ~0,4 GB | ~1,8 GB | ~2 min |

In zwei der drei Serien stockte hin und wieder eine Stufe um 2 bis 4 s, bei
beiden Ständen; acht Paare danach zeigten es nicht. Deshalb der Median aus
neun Läufen.

**Die grosse Welt:**

| | master | #18 |
|---|---|---|
| 65 536 Basiskacheln: Basis ohne Karte, Kacheln/s | 1915, 1712 | 1651, 1518 |
| dito, mit Karte | 2155, 2136 | 2004, 2002 |
| dito, ganzer Lauf ohne Karte | 59,1 / 63,5 s | 65,9 / 69,5 s |
| dito, mit Karte | 55,0 / 55,4 s | 58,6 / 58,4 s |
| dito, Basis und Pyramide | 3,66 GB | 4,53 GB |
| dito, Dateien mit anderen Pixeln als master | – | 85 940 von 87 517 |
| Ozean, 15 682 Basiskacheln: Basis und Pyramide | 0,78 GB | 0,93 GB |
| 8192er, fast nur Land, ein Thread, bestes von drei, je Kachel | 5,52 ms | 6,49 ms |
| dito, Basis und Pyramide | 84 MB | 109 MB |
| Sprite-Tabelle der ganzen Welt, scale 32 | 26 341 Sprites in 5,8 s, Spitze 0,62 GB | 26 341 in 5,5 s, 0,61 GB |

Über Ozean streuen die Raten zweigipflig wie in
[2026-09-27, Wasser im Licht](2026-09-27-wasser-im-licht.md); dort stehen
nur die Grössen.

**Zweite Runde**, ein Thread, ms je Kachel, Median aus fünf Läufen und
Spanne. master: 5,38 (5,24–5,49).

| | `b15d0ab` | neu |
|---|---|---|
| ohne Ecken, `NO_AO` | 5,49 (5,32–6,41) | 5,52 (5,32–5,59) |
| nur die Ecken aus `ao_at`, verworfen | 5,68 (5,56–5,95) | 5,59 (5,43–5,68) |
| dazu die Rechnung je Pixel, nicht angewandt | 6,02 (5,99–6,10) | 5,78 (5,75–5,99) |
| ganz | 6,37 (6,29–6,54) | 6,13 (6,06–6,33) |
| Aufschlag auf master | +0,99 ms, 18 % | +0,76 ms, 14 % |

Daraus, je Kachel:

| | `b15d0ab` | neu |
|---|---|---|
| die Ecken aus `ao_at` | 0,19 ms | 0,06 ms |
| die Rechnung je Pixel | 0,34 ms | 0,19 ms |
| Abdunkeln und Kodieren der reicheren Kacheln, 82 statt 63 kB | 0,35 ms | 0,35 ms |
| der Stand ohne Ecken gegen master | 0,12 ms | 0,15 ms |

Je Teil liegt die Unsicherheit bei etwa 0,05 ms: Nimmt man das Mittel
statt des Medians, verschieben sich die Teile um bis zu 0,07 ms, die Summe
nicht. Der Faktor bei vollem Licht allein (`fc62f31`) spart in drei Serien
je 0,06 bis 0,08 ms, 1 %.

**24 Threads**, grosse Welt, 65 536 Basiskacheln, je zwei Läufe, Kacheln/s:

| | master | `b15d0ab` | Bitebenen | Bitebenen und voller Faktor |
|---|---|---|---|---|
| ohne Karte | 1512, 1581 | 1473, 1406 | 1450, 1340 | 1464, 1378 |
| mit Karte | 1800, 1709 | 1724, 1569 | 1770, 1595 | 1836, 1472 |

Auf der Testwelt mit 10 816 Basiskacheln lief dieselbe Fassung mit 1035
bis 1586 Kacheln/s.

## Die Beispielausgabe

Die Ausgabe in [Kacheln exportieren](../benutzung/kacheln.md), „Ein
Ausschnitt“, 256 Kacheln in 0,3 s mit Karte, je drei Läufe am selben Stück:
master 1010 bis 1052 Kacheln/s, `b15d0ab` 957 bis 1002, der neue Stand 992
bis 1013. In einem früheren Durchgang am selben Tag gab master dort einmal
764; die Rate eines so kurzen Laufs sagt wenig.

## Vollrender der grossen Welt, hochgerechnet

Rund 185 GB statt rund 150 GB aus
[2026-09-27, Wasser im Licht](2026-09-27-wasser-im-licht.md), je nach Anteil
Land und Wasser 170 bis 230 GB: der Ausschnitt mit 65 536 Kacheln ×1,24,
reiner Ozean ×1,20, der 8192er fast nur Land ×1,29, die Testwelt bei scale
32 ×1,26. Die Dauer rund 65 bis 75 min statt 60 bis 70, mit Karte: Der ganze
Lauf braucht am Ausschnitt mit Karte 6 % länger als auf master, ohne Karte
10 %. Vorlauf und Sprite-Tabelle bleiben gleich.

## Schluss

Die weiche Beleuchtung kostet auf einem Thread 14 %, davon knapp die Hälfte,
0,35 von 0,76 ms, das Abdunkeln und Kodieren der reicheren Kacheln; die
Kacheln wachsen, weil sich der Verlauf schlechter packt als eine ebene
Fläche. Geglättet oder gerundet wird nichts. Byte für Byte gleich mit
`b15d0ab` nach der zweiten Runde: auf der Testwelt 16 755 Dateien mit drei
nativen Stufen, auf der grossen Welt 87 517. Entscheidungen:
[0031](../entscheidungen/0031-eigene-tabellen-statt-der-masken.md),
[0032](../entscheidungen/0032-weiche-beleuchtung-zuerst-fuer-volle-wuerfel.md).
