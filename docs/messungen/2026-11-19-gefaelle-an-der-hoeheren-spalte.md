---
title: Gefälle an der höheren Spalte, Platz
description: Wie viel Platz das Relief mit dem Gefälle an der höheren Spalte (0147) gegen die mittlere Differenz braucht, von oben mit slope auf der Testwelt, maximal_compact und good; der Baum wird 6,1 und 6,8 % kleiner, die Schicht 30,7 %, der Rest 13,2 %.
date: 2026-11-19
commits: [0aab8539, e80ca232]
code:
  - renderer/src/render/metatile/relief.rs
---

# Gefälle an der höheren Spalte, Platz

Mit dem Gefälle an der höheren Spalte wird ein Baum von oben mit `slope`
auf der Testwelt kleiner: mit `maximal_compact` um 6,1 %, mit `good` um
6,8 %. Die Kacheln schrumpfen um 4,1 bis 4,5 %, die Schicht des Reliefs um
30,7 % und ihr Rest um 13,2 %. Gezeichnet wird nur anders, wo Höhen
springen; dort fallen helle und dunkle Säume um schmale Dinge weg.

## Aufbau

- **Welt:** dieselbe Kopie der Testwelt wie in
  [Stufen am Baum](2026-11-17-stufen-am-baum.md), 316 223 Chunks, davon
  249 103 fertig erzeugt und gezeichnet, nach den Updates jener Messung
  wieder im Zustand vom Anfang.
- **Schalter:** `--camera top-north --scale 4 --relief slope --gpu off
  --quality <stufe>`, je ein voller Lauf in einen leeren Baum, Release-Build.
- **Vorher:** die Bäume des zweiten Durchgangs aus
  [Stufen am Baum](2026-11-17-stufen-am-baum.md), am Stand `0aab8539`, mit
  der mittleren Differenz.
- **Nachher:** der Zweig mit dem Gefälle an der höheren Spalte auf master
  `e80ca232`. Zwischen beiden Ständen kamen die Gebiete (#450) dazu; ohne
  Gebiete zeichnen und packen sie nichts anders.

## Ablauf

Am 19.11.2026 mit Idle-Priorität, ohne Sperre: Gemessen sind nur Bytes,
keine Zeiten. Die Bytes stammen aus den Grössen der Dateien je Teil des
Baums.

## Ergebnis

| Teil | `maximal_compact` vorher | nachher | | `good` vorher | nachher | |
|---|---|---|---|---|---|---|
| Basis, Zoom 8 | 641 363 466 B | 613 505 048 B | −4,34 % | 388 025 220 B | 372 015 548 B | −4,13 % |
| Pyramide, Zoom 0 bis 7 | 265 412 524 B | 253 486 372 B | −4,49 % | 162 301 788 B | 155 643 572 B | −4,10 % |
| Schicht | 59 685 046 B | 41 383 794 B | −30,66 % | gleich | gleich | |
| Rest der Schicht | 24 546 916 B | 21 301 138 B | −13,22 % | gleich | gleich | |
| Hashes der Schicht | 663 270 B | 663 270 B | 0 | gleich | gleich | |
| Stand, Höhen, Pixel, `map.json` | 11 653 130 B | 11 653 130 B | 0 | 11 653 100 B | 11 653 100 B | 0 |
| zusammen | 1,003 GB | 0,942 GB | −6,11 % | 0,647 GB | 0,603 GB | −6,84 % |

- **Die Schicht** ist in beiden Stufen dieselbe: Sie packt immer
  verlustfrei kompakt.
- **Der Rest** hat 12 662 statt 12 632 Dateien. Das sind 30 Kacheln mehr,
  in denen das Relief irgendwo bei 255 abschneidet.

## Schluss

- **Kleiner, nicht grösser:** Die mittlere Differenz legte um jedes
  schmale Ding vier helle und dunkle Felder auf den Boden. Mit dem Gefälle
  an der höheren Spalte bleiben diese Spalten mit dem Code für
  unverändert, und gleiche Flächen packen besser.
- **Die Schicht** schrumpft am stärksten, um fast ein Drittel: Sie hat
  einen Code je Pixel, und mehr Pixel tragen 4.
- **Grenzen:** eine Welt, nur von oben bei scale 4, ein Lauf je Stufe; die
  Zeit ist nicht gemessen, gerechnet wird je Spalte so viel wie vorher.
