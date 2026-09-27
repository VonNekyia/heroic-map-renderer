---
title: Vollrender der grossen Welt mit #11
description: Der erste ganz gemessene Vollrender der grossen Welt, mit dem Stand von #11, Grafikkarte und dem alten WebP-Encoder - Dauer, Kacheln und Grösse.
date: 2026-09-26
commits: [6f0e365]
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
---

# Vollrender der grossen Welt mit #11

Der ganze Lauf brauchte 68 Minuten und schrieb 3,33 Millionen Kacheln,
354 GB: 266 GB Basis, 88 GB Pyramide. Die Grösse kam vom einfachen Encoder
aus `image`; daraus wurde #13, siehe
[0028](../entscheidungen/0028-libwebp-statt-image.md). Spätere Stände sind
nur hochgerechnet, siehe [Was ein Lauf kostet](../benutzung/kosten.md),
„Die grosse Welt“.

## Aufbau

- Welt: die grosse Welt, 26.2, scale 32, ohne native Stufen.
- Stand: das Binär von #11 (`6f0e365`), `--gpu auto` mit einer
  eigenständigen Karte über Vulkan, WebP noch mit dem Encoder aus `image`.
- Nebenher lief eine Live-Ansicht: alle fünf Minuten `--pyramid` über
  denselben Baum.
- Die Ausnahme vom Echtzeitschutz kam erst mitten im Lauf dazu.

## Ablauf

Ein Export über die ganze Welt, in einem Stück, am 26.09. von 15:03 bis
16:11. Die Zahlen stammen aus seiner Ausgabe und aus der Ausgabe der
Aufrufe von `--pyramid` daneben, die mit Uhrzeit mitgeschrieben war. Die
Ausgabe zählt MB binär: 253 580,6 MiB Basis sind 266 GB, 84 317,4 MiB
Pyramide 88 GB. Die Raten ohne Nebenlauf sind Stichproben, der Fortschritt
über je eine Minute um 15:06 und um 15:17.

## Ergebnis

| | Wert |
|---|---|
| ganzer Lauf | 68 min |
| Vorlauf | 64 s |
| Basis | 2 504 461 Kacheln, 2 496 892 geschrieben und 7 569 leer, in 44 min |
| Pyramide | 835 252 Kacheln auf Zoom 0 bis 10 in 23 min |
| Grösse | 354 GB: Basis 266 GB, Pyramide 88 GB |

- Ohne Nebenlauf stieg die Rate mit der Ausnahme vom Echtzeitschutz von 891
  auf 1593 Kacheln/s.
- Solange ein Aufruf von `--pyramid` nebenher lief, sank das Tempo, und die
  Aufrufe wurden mit der Basis länger: 426, 581 und 782 s, also 7, 10 und
  13 min. Gemessen ist der Einbruch nur während des ersten Aufrufs: etwa
  610 statt 891 Kacheln/s, aus zwei Ständen des Fortschritts, 281 600
  Kacheln um 15:11:27 und 430 000 um 15:15:31. In diese Minuten fiel auch
  die Ausnahme; wann genau, ist nicht festgehalten.
- Dass sich das Tempo danach mit Nebenlauf etwa halbierte, ist gerechnet,
  nicht gemessen:
  - Um 15:17:37, am Ende der zweiten Stichprobe, standen 632 400 Kacheln.
  - Die Basis endete gegen 15:49: 1353,6 s Pyramide vor dem Ende des
    Exports, das zwischen 16:11:32 und 16:11:37 lag, denn die Uhrzeit im
    Log kam bis zu 5 s danach.
  - Dazwischen liegen 1 872 061 Kacheln in 1881 bis 1886 s. 1363 s davon
    lief ein Aufruf von `--pyramid` nebenher, der zweite mit 581 und der
    dritte mit 782 s; der dritte endete um 15:48:05, vor der Basis. Die
    übrigen 518 bis 523 s liefen ohne.
  - Mit 1593 Kacheln/s ohne Nebenlauf bleiben für die 1363 s mit
    Nebenlauf gut 1,04 Millionen Kacheln, 762 bis 768 Kacheln/s, knapp die
    Hälfte.
- Ohne Live-Ansicht und mit der Ausnahme von Anfang an wären es geschätzt
  etwa 50 Minuten gewesen; gemessen ist das nicht.

## Schluss

Das ist der einzige ganz gemessene Vollrender der grossen Welt. Grösse und
Dauer der Stände danach, libwebp, Wasser im Licht und weiche Beleuchtung,
sind aus Ausschnitten hochgerechnet.
