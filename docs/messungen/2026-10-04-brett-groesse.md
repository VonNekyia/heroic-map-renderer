---
title: Grösse des gerenderten Bretts
description: Wie gross die gerenderten Bilder des Skins Tablett sind, je Kamera und Richtung fern und nah, gemessen am Platzhalter und mit der Vorlage als Massstab für die Szene hochgerechnet; dazu, was der Ausschnitt je Fenster kostet.
date: 2026-10-04
commits: [2833e1d]
code:
  - web/skins/tablett/werkzeug/brett.py
  - web/skins/tablett/werkzeug/brett_blender.py
---

# Grösse des gerenderten Bretts

Alle 64 Bilder der Szene, 32 Blicke mit fern und nah, kämen hochgerechnet
auf 41 bis 66 MB, ein Blick in 8:5 auf 1,2 bis 2,0 MB. Der Platzhalter
braucht 0,2 MB, hat aber nur 9 bis 11 Farben. Die Grösse treibt der
Ausschnitt: Er deckt Fenster von 9:20 hochkant bis 21:9 quer.

## Aufbau

- Gerendert mit `werkzeug/brett.py` ohne Szene, also der Platzhalter, alle
  8 Kameras in je 4 Richtungen, Blender 5.2, Stand `2833e1d`.
- Ausschnitt je Bild wie in [Tablett aus Blender](../tablett-gerendert.md),
  „Rendern“: jedes Fenster von 9:20 bis 21:9, in dem der Rahmen 71 %
  füllt.
- Massstab für die Szene: die Vorlage selbst, wie die Szene aussehen soll.
  Sie wird je 2 × 2 Pixel gemittelt, also auf die Auflösung des Bretts,
  745 × 527 px. Dann bringt Pillow sie per Median-Cut ohne Dither auf N
  Farben und schreibt sie wie `brett.py`: WebP verlustfrei, Qualität 100,
  Methode 6.

## Ablauf

- Platzhalter: Dateigrössen der WebP nach dem Lauf, mit und ohne die
  nahen Pixel in fern. Ohne sie schreibt `brett.py` seit dieser Messung.
- Vorlage: Bytes je Pixel aus der Grösse des WebP, Skript vom 04.10.
- Hochrechnung: Fläche aller Bilder mal Bytes je Pixel. Die Szene füllt
  ihr Bild ganz, anders als der Tisch des Platzhalters; fern und nah
  zusammen speichern jedes Pixel einmal.
- Andere Ausschnitte: Grösse der Bilder aus derselben Rechnung wie
  `stelle_kamera` in `brett_blender.py`, mit anderen Grenzen.

## Ergebnis

Platzhalter, alle 64 Bilder:

| fern | zusammen |
|---|---|
| mit den nahen Pixeln | 232 596 Byte |
| ohne sie | 195 094 Byte |

Die Vorlage in Auflösung des Bretts, verlustfrei:

| Farben | 16 | 32 | 64 | 128 | 256 |
|---|---|---|---|---|---|
| Byte je Pixel | 0,245 | 0,330 | 0,422 | 0,528 | 0,635 |

Hochgerechnet mit 0,330 bis 0,528 Byte je Pixel, also 32 bis 128 Farben:

| Ausschnitt | 8:5 je Bild | Fläche aller 32 | ein Blick 8:5 | alle 64 Bilder |
|---|---|---|---|---|
| 9:20 bis 21:9, 71 % | 1666 × 2254 | 125,6 Mpx | 1,2 bis 2,0 MB | 41 bis 66 MB |
| 3:4 bis 21:9, 71 % | 1666 × 1390 | 77,5 Mpx | 0,8 bis 1,2 MB | 26 bis 41 MB |
| 3:4 bis 16:9, 71 % | 1292 × 1390 | 59,9 Mpx | 0,6 bis 0,9 MB | 20 bis 32 MB |
| 9:20 bis 21:9, 50 % | 2326 × 3160 | 246,4 Mpx | 2,4 bis 3,9 MB | 81 bis 130 MB |

## Schluss

- Die Szene packt weit schlechter als der Platzhalter. Für die Szene
  zählen nur die hochgerechneten Werte.
- Hochkant macht das Bild 2254 px hoch; dort ist rund 78 % des Fensters
  Tisch. Ohne Fenster unter 3:4 wären es 38 % weniger.
- Füllte die Gesamtansicht bis 50 % statt 71 %, verdoppelte sich die
  Fläche.
- Fern ohne die nahen Pixel spart beim Platzhalter 16 %. Bei der Szene
  spart es, was nah deckt: 86,5 von 211,4 Mpx, rund 40 % der Bytes.
