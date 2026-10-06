---
title: Schätzung gegen gemessene Läufe
description: Was --estimate für Kacheln, Platz und Dauer sagt, gegen echte Läufe an der Testwelt und gegen die gemessenen Vollrender der grossen Welt, über drei Stände des Codes; dazu, woher jeder Faktor der Eichung kommt und wie stark die Schätzung selbst streut.
date: 2026-10-06
commits: [ba21e6d, d46e983, 16903e4]
code:
  - renderer/src/cli/schaetzung.rs
---

# Schätzung gegen gemessene Läufe

Mit `16903e4` liegen die echten Läufe an der Testwelt in jeder Spanne der
Schätzung: Kacheln, Bytes und Dauer, bei scale 8 und 32, mit 24 Threads und
mit einem. An der grossen Welt treffen die Kacheln alle vier gemessenen
Vollrender, die Bytes auch und die Dauer drei von vier. Zwei frühere Stände
schätzten die Dauer falsch; beide Fehler sind unten beschrieben. Zu #149.

## Aufbau

- **Testwelt:** nur gelesen; 2:1 aus `se`, `--gpu off`, Vorgabe der nativen
  Stufen.
  - scale 8 mit 24 Threads;
  - scale 8 mit `--threads 1 --low-priority`;
  - scale 32 mit 24 Threads.
- **Grosse Welt:** eine Kopie, nur gelesen, mit ihren Biomdaten. Die
  Schalter der gemessenen Vollrender aus
  [Was ein Lauf kostet](../benutzung/kosten.md), „Die grosse Welt“:
  - scale 32 mit drei nativen Stufen und scale 24 mit einer, die ganze
    Welt, [2026-09-29, Vollrender mit #49](2026-09-29-vollrender-mit-49.md);
  - Cinematic 8:5 mit einer nativen Stufe,
    [2026-10-04, Vollrender mit Cinematic](2026-10-04-vollrender-cinematic.md);
  - Cinematic 4:3 bei scale 24 ohne native Stufe,
    [2026-10-04, Vollrender 4:3](2026-10-04-vollrender-4x3.md);
  - die beiden mit Cinematic über das Rechteck aus 1564 × 1564 Chunks.
- **Je Einstellung** `--estimate --progress json`; gewertet wird die Zeile
  `estimate`. An der Testwelt danach der echte Lauf in einen leeren Ordner:
  Dauer vom Messskript, Kacheln aus der Ausgabe, Bytes und Dateien aus dem
  Baum.

```bash
heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --scale 8 --gpu off --estimate --progress json
```

## Ablauf

Am 06.10., je mit Sperrdatei, atomar gelegt, Last vor jedem Lauf notiert.
Zwischen den Reihen blieb die Sperre mindestens 10 min frei.

| Reihe | Zeit | Stand | Was |
|---|---|---|---|
| A | 01:44:12 bis 02:08:18 | `ba21e6d` | je Schätzung und echter Lauf an der Testwelt |
| B1 | 02:29:06 bis 02:33:45 | `d46e983` | nur Schätzungen, Testwelt und grosse Welt |
| B2 | 02:43:47 bis 02:48:24 | `16903e4` | wie B1 |

- **Last:** vor den Läufen 1 bis 17 %, kein Spiel, kein Server.
  Hintergrundprogramme brauchten rund 1,5 Kerne.
- **Streuung der Schätzung:** Um 02:50, ohne Sperre und ohne fremde Last,
  lief die Schätzung für scale 32 viermal hintereinander. Jedes Mal kam
  „3 bis 6 min“ heraus, die Basis zu 194 bis 221 s.

## Ergebnis an der Testwelt

Echte Läufe aus Reihe A:

| | Dauer | Kacheln | Bytes | Dateien |
|---|---|---|---|---|
| scale 8 | 89,2 s | 18 164 | 1,560 GB | 24 353 |
| scale 8, ein Thread | 732,5 s | 18 164 | 1,560 GB | 24 353 |
| scale 32 | 266,9 s | 280 630 | 24,53 GB | 367 994 |

Dauer der Schätzung in Sekunden, `[unten, oben]`:

| | A, `ba21e6d` | B1, `d46e983` | B2, `16903e4` | echt |
|---|---|---|---|---|
| scale 8 | 78 bis 154 | 73 bis 153 | 69 bis 146 | 89,2 |
| scale 8, ein Thread | 555 bis 1098 | 415 bis 866 | 445 bis 979 | 732,5 |
| scale 32 | 350 bis 695, **zu hoch** | 279 bis 610, **zu hoch** | 254 bis 558 | 266,9 |

Kacheln, Bytes und Dateien mit `16903e4`:

| | Kacheln | Bytes | Dateien |
|---|---|---|---|
| scale 8 | 15 600 bis 18 721 | 1,13 bis 1,92 GB | bis 24 961 |
| scale 8, ein Thread | 15 600 bis 18 721 | 1,10 bis 1,87 GB | bis 24 961 |
| scale 32 | 249 608 bis 299 529 | 17,9 bis 30,3 GB | bis 399 373 |

- **Fertig erzeugt** sagt die Stichprobe zu 78,9 %; gezählt sind es 78,8 %,
  249 103 von 316 223.
- **Die Pyramide** wog bei scale 8 45 % der Basis, bei scale 32 35 %.
- **Die Proben** lagen in den Bytes je Basiskachel 4 % (scale 32) bis 16 %
  (scale 8) über dem Schnitt der echten Basis.

## Ergebnis an der grossen Welt

Schätzung mit `16903e4` gegen die gemessenen Vollrender:

| | Kacheln | Bytes | Dauer |
|---|---|---|---|
| scale 32, drei Stufen | 2,49 bis 2,99 Mio., gemessen 2 491 797 | 161 bis 272 GB, gemessen 187,8 | 101 bis 194 min, gemessen 95, **verfehlt** |
| scale 24, eine Stufe | 1,40 bis 1,68 Mio., gemessen 1 403 211 | 94 bis 160 GB, gemessen 110,3 | 49 bis 105 min, gemessen 55 |
| Cinematic 8:5 | 3,03 bis 3,64 Mio., gemessen 3 076 640 | 206 bis 349 GB, gemessen 222,8 | 4,6 bis 10,5 h, gemessen 5 h 26 min |
| Cinematic 4:3 | 2,05 bis 2,46 Mio., gemessen 2 073 988 | 145 bis 246 GB, gemessen 160,2 | 2,2 bis 5,1 h, gemessen 2 h 43 min |

- **Die ganze Welt** hat heute 2 662 204 Chunks in den Köpfen, beim Lauf mit
  #49 waren es 2 520 778, 5,6 % weniger. Das Rechteck hat heute wie damals
  2 442 346.
- **Die Vollrender** liefen mit älterem Code, die mit #49 und 8:5 mit
  Live-Ansicht nebenher. Gegen sie ist nur grob zu vergleichen.
- **Mit `d46e983`** lagen die Bytes an der grossen Welt 8 bis 21 % über den
  Vollrendern. Seitdem zählen die Bytes je Kachel der Proben mal 0,8 bis 1,0.

## Die zwei Fehler

- **Basis je Kachel zu hoch (`ba21e6d`):** Ein kleiner Ausschnitt liest mehr
  Chunks je Kachel als die Welt, denn unter ihm ragen Säulen hinein, die er
  dekodiert und beleuchtet. Bei scale 32 waren es je nach Ort und Grösse
  1,3- bis 5,2-mal so viele, an der ganzen Welt 0,89 je Kachel.
  Seit `d46e983` rendert der Probelauf je Mitte Kante k und 2k. Ein Fit
  trennt die Zeit der Basis in einen Anteil je Kachel und einen je Chunk.
- **Vorlauf zu hoch und streuend (`d46e983`):** Der Vorlauf eines Ausschnitts
  berührt zwei bis vier Regionen und läuft kaum parallel. Er sagte für die
  Testwelt 19 bis 25 s statt 7,9 s. Seit `16903e4` zählt die Zeit, die das
  Dekodieren der Stichprobe mit denselben Threads je Chunk braucht:
  0,020 ms mit 24 Threads und 0,239 ms mit einem. Die echten Vorläufe
  brauchten das 1,10- bis 1,55-Fache: 0,025 und 0,031 ms mit 24 Threads,
  0,263 ms mit einem.

## Schluss

- An der Testwelt trifft `16903e4` jede Grösse in jeder Einstellung. Die
  Spanne der Dauer ist etwa ×2,2 breit, die der Bytes ×1,7.
- An der grossen Welt verfehlt nur die Dauer bei scale 32 mit drei Stufen,
  um 6 min. Die Welt hat heute 5,6 % mehr Chunks als bei diesem Lauf.
- **Streuung:** Bei scale 32 mit 24 Threads dauert die Basis eines
  Ausschnitts nur Zehntelsekunden. Der untere Rand lag in B2 bei 254 s, in
  vier Läufen ohne Sperre bei rund 165 s. Grössere Ausschnitte bei vielen
  Threads machten die Schätzung ruhiger und kosteten einige Sekunden mehr.
