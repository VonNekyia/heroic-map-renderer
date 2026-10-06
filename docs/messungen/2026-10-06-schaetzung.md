---
title: Schätzung gegen gemessene Läufe
description: Was --estimate für Kacheln, Platz und Dauer sagt, gegen echte Läufe an der Testwelt und gegen die gemessenen Vollrender der grossen Welt, über vier Stände des Codes; dazu, woher jeder Faktor der Eichung kommt und wie stark die Schätzung selbst streut.
date: 2026-10-06
commits: [ba21e6d, d46e983, 16903e4, db96dfc]
code:
  - renderer/src/cli/schaetzung.rs
---

# Schätzung gegen gemessene Läufe

Mit `db96dfc` liegen die echten Läufe an der Testwelt in jeder Spanne der
Schätzung: Dauer und Bytes bei scale 4, 8 und 32, mit 24 Threads und mit
einem, die Kacheln bei scale 8 und 32. Bei scale 4 lagen die Kacheln 2 %
über dem oberen Rand; unter scale 8 reicht er seitdem bis 1,3. An der
grossen Welt trafen mit `16903e4` die Kacheln alle vier gemessenen
Vollrender, die Bytes auch, die Dauer drei von vier. Den vierten verfehlte
ein Fehler bei den nativen Stufen, den `db96dfc` behebt. Zu #149.

## Aufbau

- **Testwelt:** nur gelesen; 2:1 aus `se`, `--gpu off`, Vorgabe der nativen
  Stufen.
  - scale 8 mit 24 Threads;
  - scale 8 mit `--threads 1 --low-priority`;
  - scale 32 mit 24 Threads;
  - scale 4 mit 24 Threads, nur in Reihe C.
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
| C | 03:24:43 bis 03:28:21 | `db96dfc` | Schätzungen an der Testwelt, dazu scale 4 mit echtem Lauf |

- **Last:** vor den Läufen 1 bis 17 %, kein Spiel, kein Server.
  Hintergrundprogramme brauchten rund 1,5 Kerne.
- **Streuung der Schätzung:** Um 02:50 lief die Schätzung von `16903e4` für
  scale 32 viermal hintereinander, ohne Sperre und ohne fremde Last. Jedes
  Mal kam „3 bis 6 min“ heraus, die Basis zu 194 bis 221 s.

## Ergebnis an der Testwelt

Echte Läufe aus Reihe A, scale 4 aus Reihe C:

| | Dauer | Kacheln | Bytes | Dateien |
|---|---|---|---|---|
| scale 8 | 89,2 s | 18 164 | 1,560 GB | 24 353 |
| scale 8, ein Thread | 732,5 s | 18 164 | 1,560 GB | 24 353 |
| scale 32 | 266,9 s | 280 630 | 24,53 GB | 367 994 |
| scale 4 | 69,3 s | 4 769 | 0,459 GB | 6 530 |

Dauer der Schätzung in Sekunden, `[unten, oben]`:

| | A, `ba21e6d` | B1, `d46e983` | B2, `16903e4` | C, `db96dfc` | echt |
|---|---|---|---|---|---|
| scale 8 | 78 bis 154 | 73 bis 153 | 69 bis 146 | 64 bis 137 | 89,2 |
| scale 8, ein Thread | 555 bis 1098 | 415 bis 866 | 445 bis 979 | 431 bis 950 | 732,5 |
| scale 32 | 350 bis 695, **zu hoch** | 279 bis 610, **zu hoch** | 254 bis 558 | 187 bis 409 | 266,9 |
| scale 4 | | | | 55 bis 120 | 69,3 |

Kacheln, Bytes und Dateien mit `db96dfc`:

| | Kacheln | Bytes | Dateien |
|---|---|---|---|
| scale 8 | 15 600 bis 18 721 | 1,15 bis 1,96 GB | 20 801 bis 24 961 |
| scale 8, ein Thread | 15 600 bis 18 721 | 1,10 bis 1,87 GB | 20 801 bis 24 961 |
| scale 32 | 249 608 bis 299 529 | 19,6 bis 33,2 GB | 332 811 bis 399 373 |
| scale 4 | 3 900 bis 4 680, **zu niedrig** | 0,32 bis 0,54 GB | 5 200 bis 6 240, **zu niedrig** |

- **Fertig erzeugt** sagt die Stichprobe zu 78,9 %; gezählt sind es 78,8 %,
  249 103 von 316 223.
- **Kacheln je Fläche der Oberseite:** 1,13 bei scale 32, 1,14 bei 16,
  1,17 bei 8, 1,23 bei 4. Je kleiner der scale, desto mehr Kacheln am Rand
  werden nur angeschnitten. Seit dem Fix nach Reihe C reicht der obere
  Faktor unter scale 8 bis 1,3; bei scale 4 ergäbe das 3 900 bis 5 060
  Kacheln.
- **Die Pyramide** wog bei scale 4 und 8 je 45 % der Basis, bei scale 32 35 %. Sie
  brauchte 0,7 % (scale 32) bis 2,7 % (scale 8, ein Thread) der Zeit der
  Basis.
- **Die Proben** lagen in den Bytes je geplanter Basiskachel 7 % (scale 32)
  bis 18 % (scale 8) über der echten Basis.
- **Der Vorlauf** brauchte je Chunk aus den Köpfen das 1,10- bis 1,55-Fache
  dessen, was das Dekodieren der Stichprobe mit denselben Threads je Chunk
  brauchte: 0,025 und 0,031 ms gegen 0,020 ms mit 24 Threads, 0,263 gegen
  0,239 ms mit einem.
- **Streuung:** Bei scale 32 lag der untere Rand in B2 bei 254 s, mit den
  grösseren Proben von `db96dfc` bei 187 s. Ohne Sperre lag er mit
  `db96dfc` in zwei Läufen bei 182 und 228 s.

## Ergebnis an der grossen Welt

Schätzung mit `16903e4` gegen die gemessenen Vollrender:

| | Kacheln | Bytes | Dauer |
|---|---|---|---|
| scale 32, drei Stufen | 2,49 bis 2,99 Mio., gemessen 2 491 797 | 161 bis 272 GB, gemessen 187,8 | 101 bis 194 min, gemessen 95, **verfehlt** |
| scale 24, eine Stufe | 1,40 bis 1,68 Mio., gemessen 1 403 211 | 94 bis 160 GB, gemessen 110,3 | 49 bis 105 min, gemessen 55 |
| Cinematic 8:5 | 3,03 bis 3,64 Mio., gemessen 3 076 640 | 206 bis 349 GB, gemessen 222,8 | 4,6 bis 10,5 h, gemessen 5 h 26 min |
| Cinematic 4:3 | 2,05 bis 2,46 Mio., gemessen 2 073 988 | 145 bis 246 GB, gemessen 160,2 | 2,2 bis 5,1 h, gemessen 2 h 43 min |

- **Die ganze Welt** hat heute 2 662 204 Chunks in den Köpfen, beim Lauf mit
  #49 waren es 2 520 778, 5,3 % weniger. Fertig erzeugt sind heute nach der
  Stichprobe rund 2 486 000, damals 2 449 850, rund 1,5 % weniger. Das
  Rechteck hat heute wie damals 2 442 346 Chunks.
- **Die Vollrender** liefen mit älterem Code, die mit #49 und 8:5 mit
  Live-Ansicht nebenher. Gegen sie ist nur grob zu vergleichen.
- **Mit `d46e983`** lagen die Bytes an der grossen Welt 8 bis 21 % über den
  Vollrendern. Seitdem zählen die Bytes je Kachel der Proben mal 0,8 bis 1,0.
- **Die Faktoren aus diesen Läufen,** gerechnet aus ihren Messseiten:
  - Kacheln je Fläche der Oberseite: 1,017 bei scale 32 mit drei Stufen,
    1,018 bei scale 24, 1,014 bei Cinematic 8:5, 1,013 bei 4:3;
  - native Stufen und Pyramide in Bytes: 32 %, 34 %, 35 % und 37 % der
    Basis;
  - Pyramide in der Zeit: 0,6 %, 2,3 %, 0,2 % und 0,8 % der Basis.

## Die drei Fehler

- **Basis je Kachel zu hoch (`ba21e6d`):** Ein kleiner Ausschnitt liest mehr
  Chunks je Kachel als die Welt, denn unter ihm ragen Säulen hinein, die er
  dekodiert und beleuchtet. Bei scale 32 waren es je nach Ort und Grösse
  1,3- bis 5,2-mal so viele, an der ganzen Welt 0,89 je Kachel.
  Seit `d46e983` rendert der Probelauf je Mitte Kante k und 2k. Ein Fit
  trennt die Zeit der Basis in einen Anteil je Kachel und einen je Chunk.
- **Vorlauf zu hoch und streuend (`d46e983`):** Der Vorlauf eines Ausschnitts
  berührt zwei bis vier Regionen und läuft kaum parallel. Er sagte für die
  Testwelt 19 bis 25 s statt 7,9 s. Seit `16903e4` zählt die Stichprobe.
- **Native Stufen dreifach (`16903e4`):** Die Proben zählten jede native
  Stufe als eigene Phase, aus `tiles / rate` ihrer letzten Zeile. Die Stufen
  laufen aber in Bändern zugleich, mit einem Beginn; mit drei Stufen zählte
  die Phase dreifach. Daher kam der Fehlgriff bei scale 32 mit drei Stufen.
  Die Testwelt lief ohne native Stufen und prüfte diesen Weg nicht. Seit
  `db96dfc` zählt die Phase einmal, so lange wie die längste Stufe, aus dem
  Feld `s` der Zeilen.

## Schluss

- An der Testwelt trifft `db96dfc` Dauer und Bytes in jeder Einstellung,
  die Kacheln bei scale 8 und 32. Die Spanne der Dauer ist etwa ×2,2 breit,
  die der Bytes ×1,7.
- Bei scale 4 lagen die Kacheln 2 % über dem oberen Rand. Unter scale 8
  reicht er seitdem bis 1,3.
- An der grossen Welt trafen die Kacheln und Bytes von `16903e4` alle vier
  Vollrender. Die Dauer bei scale 32 mit drei Stufen verfehlte sie wegen
  der dreifach gezählten Stufen; gegen `db96dfc` ist sie nicht neu
  geschätzt.
