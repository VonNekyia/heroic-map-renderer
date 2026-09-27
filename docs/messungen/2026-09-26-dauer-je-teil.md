---
title: Dauer je Teil, Testwelt
description: Wie lange Basis, native Stufen, Pyramide und Sprite-Tabellen der ganzen Testwelt bei scale 32 brauchten, je Teil gemessen am Stand von #10; überholt.
date: 2026-09-26
commits: [fdfb100]
code:
  - renderer/src/cli.rs
  - renderer/src/render/rasterizer.rs
---

# Dauer je Teil, Testwelt

**Überholt.** Diese Zahlen gelten für den Stand von #10, vor der Grafikkarte
(#11) und den grossen Posten (#12), die die Dauer am stärksten geändert
haben, und vor libwebp, dem Licht unter Wasser und der weichen Beleuchtung.
Den aktuellen Stand nennt [Was ein Lauf kostet](../benutzung/kosten.md),
„Dauer“.

Hochgerechnet auf die ganze Testwelt bei scale 32 hätten die drei nativen
Stufen 4,8 Minuten gebraucht, gut drei Viertel der Basis mit 6,1 Minuten,
die Pyramide unter 0,1 Minuten.

## Aufbau

- Welt: die Testwelt, zwei Ausschnitte um (-64, 416) mit 2304 und 6400
  Basiskacheln, scale 32, mit allen nativen Stufen und Pyramide, 24
  Threads.
- Stand: #10 vor seinem Merge. Die Zahlen stehen seit `fdfb100` (26.09.,
  00:54) im README und in zwei Kommentaren; der Commit ändert nur Doku und
  Kommentare, gemessen ist also sein Code.
- Jede geschriebene Kachel ging durch den Echtzeitschutz.

## Ablauf

Der Unterschied der beiden Ausschnitte gibt die Zeit je Kachel für Basis,
native Stufen und Pyramide, ohne den Vorlauf und die Sprite-Tabellen, die
jede Stufe einmal baut; die kommen einmal dazu. Wie viele Läufe je
Ausschnitt liefen und ob abwechselnd, ist nicht festgehalten.

## Ergebnis

Hochgerechnet auf die ganze Testwelt, 292 836 Basiskacheln bei scale 32:

| Teil | Dauer |
|---|---|
| Basis | 6,1 min |
| native Stufen | 4,8 min |
| Pyramide | unter 0,1 min |
| Sprite-Tabellen aller 3110 Blockstates, über die vier Stufen | rund 11 s |

Die Spalte Dauer im README, mit Vorlauf und Sprite-Tabellen:

| `--scale` | Dauer |
|---|---|
| 32 | ~11 min |
| 16 | ~4 min |
| 8 | ~2 min |

- Ein Lauf bei scale 16 samt seinen Stufen über dieselbe Fläche hätte
  hochgerechnet 4,2 Minuten gebraucht, etwa so lange wie die nativen Stufen
  bei scale 32.
- Gegen die Messung der Runde davor war die Spalte ein Viertel kürzer; das
  README nannte es Schwankung von Tag zu Tag.
- Die Spalten zum Platz im selben README stammen aus einer früheren
  Messung; neu gemessen war nur die Dauer.

## Schluss

Die nativen Stufen kosteten gut drei Viertel der Basis, deshalb blieb die
Vorgabe 0, siehe [0016](../entscheidungen/0016-native-stufen-nur-auf-wunsch.md).
Die Sprite-Tabellen sind ein kleiner Teil eines Laufs. Mit libwebp
brauchten die nativen Stufen das 1,2-Fache der Basis, siehe
[2026-09-27, libwebp](2026-09-27-libwebp.md).
