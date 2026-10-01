---
title: Vollrender der grossen Welt mit #49
description: Zwei Vollrender der grossen Welt mit dem Licht aus der Ausbreitung (#49) und den Masken aus den Klassen (#53), scale 32 mit drei nativen Stufen und scale 24 mit einer, mit Karte und Live-Ansicht. Dauer je Stufe, Kacheln, Grösse und der Vergleich mit dem Lauf mit #21 und mit der Hochrechnung.
date: 2026-09-29
commits: [6496410]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/licht.rs
---

# Vollrender der grossen Welt mit #49

Mit dem Licht aus der Ausbreitung brauchte die Basis der grossen Welt bei
scale 32 43,6 min, 12 % mehr als im Vollrender mit #21. Hochgerechnet
waren 51 bis 62 min. Der ganze Lauf mit drei nativen Stufen brauchte
95 min für 188 GB, davon 50 min für die nativen Stufen, mehr als für die
Basis. Bei scale 24 mit einer nativen Stufe waren es 55 min für 110 GB.

## Aufbau

- Welt: die grosse Welt, 26.2, mit ihren eigenen Biomdaten neben denen von
  Vanilla.
- Stand: `6496410`, das ist master `8308264` mit dem Licht aus #34 (#49)
  und dazu der Commit aus #53, der `Masks::of` beschleunigt (seit
  `6f44cc7` in master). Release-Build, alle 24 Threads, Karte an, eine
  eigenständige Karte über Vulkan. Den Stand hat der Maintainer genannt;
  die Ausgabe zeigt ihn nicht.
- Der Kachelordner lag in einem Verzeichnis, das vom Echtzeitschutz
  ausgenommen ist, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Den
  Hinweis dazu gibt die Ausgabe trotzdem aus, denn sie prüft die Ausnahme
  nicht.
- Nach Angabe des Maintainers lief bei beiden Läufen eine Live-Ansicht
  nebenher, wie beim Lauf mit #21. Ihre Aufrufe sind nicht mitgeschrieben.
- Zwei Läufe hintereinander, je in ein leeres Verzeichnis:
  - **scale 32** mit drei nativen Stufen, scale 16, 8 und 4;
  - **scale 24** mit einer nativen Stufe, scale 12.

Der Befehl für den ersten Lauf, aus der Wurzel des Repositorys:

```bash
renderer/target/release/terranova-render --world <grosse Welt> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --data <ihre Biomdaten> --tiles <ordner> --native-levels 3
```

## Ablauf

Alle Zahlen stammen aus der Ausgabe der beiden Läufe vom 29.09. Die
Ausgabe nennt keine Uhrzeit. Die Dauer eines Laufs ist deshalb die Summe
seiner Phasen; das Laden der Assets und der Bau der Sprite-Tabellen fehlen
darin. Die Ausgabe zählt MB binär. Die Grössen sind hier in GB umgerechnet
und sind Summen der Dateigrössen; der belegte Platz ist nicht gemessen.
Die Spitze des Arbeitsspeichers ist nicht gemessen.

## Ergebnis

**scale 32, drei native Stufen:**

| | Kacheln | Grösse | Dauer | Kacheln/s |
|---|---|---|---|---|
| Vorlauf | 2 449 850 Chunks; 70 928 nicht fertig erzeugt, nicht gezeichnet | | 69 s | |
| Höhen | | | 1 s | |
| Basis, Zoom 11 | 2 491 797: 2 484 334 geschrieben, 7 463 leer | 142,2 GB, 57,2 kB je Kachel | 2619 s, 43,6 min | 952 |
| nativ, Zoom 10, scale 16 | 622 000 | 34,1 GB | 1341 s, 22,4 min | 464 |
| nativ, Zoom 9, scale 8 | 156 155 | 8,2 GB | 918 s, 15,3 min | 170 |
| nativ, Zoom 8, scale 4 | 39 311 | 2,4 GB | 745 s, 12,4 min | 53 |
| Pyramide, Zoom 0 bis 7 | 13 588 | 1,1 GB | 15 s | |
| zusammen | | 187,8 GB | 5709 s, 95 min | |

**scale 24, eine native Stufe:**

| | Kacheln | Grösse | Dauer | Kacheln/s |
|---|---|---|---|---|
| Vorlauf | wie oben | | 68 s | |
| Basis, Zoom 11 | 1 403 211: 1 398 754 geschrieben, 4 457 leer | 82,5 GB, 59,0 kB je Kachel | 1777 s, 29,6 min | 790 |
| nativ, Zoom 10, scale 12 | 350 818 | 19,4 GB | 1398 s, 23,3 min | 251 |
| Pyramide, Zoom 0 bis 9 | 118 140, davon 88 193 schon während der nativen Stufe | 8,5 GB | 40 s | |
| zusammen | | 110,3 GB | 3284 s, 55 min | |

## Gegen den Lauf mit #21 und die Hochrechnung

- **Basis bei scale 32:**
  - Mit #21 brauchte sie 2338 s für 2 504 461 Kacheln, 1071 Kacheln/s,
    siehe [2026-09-27, Vollrender mit #21](2026-09-27-vollrender-mit-21.md).
  - Jetzt sind es 2619 s für 2 491 797 Kacheln, 12 % mehr Zeit, je Kachel
    13 % mehr.
  - Dazwischen liegen #29 bis #53, darunter das Licht aus #34 und das
    schnellere `Masks::of` aus #53. Getrennt ist das nicht.
- **Hochrechnung:** [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md)
  rechnete für die Basis 51 bis 62 min hoch, aus den Raten an zwei
  Ausschnitten der Testwelt. Gemessen sind 43,6 min, darin steckt auch #53,
  das an den Ausschnitten 1,7 bis 5,5 % brachte.
  - Die Ausschnitte liefen bei 24 Threads mit Streifen von 4 und 2
    Spalten, die grosse Welt mit 8.
  - Schmale Streifen dekodieren einen Chunk öfter und rechnen sein Licht
    öfter. Das macht das Licht an den Ausschnitten vermutlich teurer als
    auf der grossen Welt; gemessen ist das nicht.
- **Grösse:** Eine Basiskachel wiegt 57,2 kB statt 54,7 kB, 5 % mehr. Die
  Hochrechnung nannte 3 bis 8 %.
- **Die nativen Stufen** sind der grösste Posten: 50 min gegen 43,6 min
  Basis.
  - Zusammen haben sie 817 466 Kacheln, ein Drittel der Basis, und
    brauchen trotzdem länger.
  - Je Stufe sinkt die Rate von 464 über 170 auf 53 Kacheln je Sekunde.
    Eine Kachel zeigt viermal so viel Welt wie eine der Stufe davor, bei
    scale 4 64-mal so viel wie bei scale 32.
  - Jede native Stufe dekodiert die Chunks und breitet ihr Licht noch
    einmal aus, siehe
    [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md).
  - Ohne native Stufen ist bisher kein Vollrender mit #49 gemessen.
- **scale 24** braucht für 56 % der Kacheln von scale 32 68 % der Zeit der
  Basis, je Kachel 1,27 statt 1,05 ms.

## Schluss

Die Basis ist mit dem Licht 12 % langsamer als mit #21, nicht die 30 bis
60 % der Hochrechnung aus den Ausschnitten. Mit drei nativen Stufen
überwiegen diese: 50 der 95 min. Sie schneller zu machen ist die nächste
Aufgabe.
