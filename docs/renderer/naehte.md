---
title: Rastern ohne Nähte
description: Wie der Rasterizer Sprites ohne Nähte und ohne Löcher rastert, Fragmente je Pixel mischt und die Textur über den Pixel mittelt.
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/assets/baker.rs
---

# Rastern ohne Nähte

Sprite-Kanten werden nicht geglättet: Die Geometrie wird nur im
Pixelmittelpunkt geprüft, mit der Füllregel der Grafikkarten, damit an einer
gemeinsamen Kante jeder Pixel genau einer der beiden Flächen gehört.
Innerhalb eines Sprites legt jede Fläche je Pixel ein Fragment ab, das am
Schluss von hinten nach vorn gemischt wird. Die Textur dagegen wird über
den Pixel gemittelt, in linearem Licht. Alles in
[`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs).

## Geometrie im Pixelmittelpunkt

Sprite-Kanten werden nicht geglättet. Die Geometrie wird nur im
Pixelmittelpunkt geprüft, damit an einer gemeinsamen Kante jeder Pixel
genau einer der beiden Flächen gehört und Nachbarflächen nahtlos
aneinanderstossen. Geglättete Kanten trügen Teildeckung im Alpha, und beim
Zusammensetzen der Sprites könnte niemand mehr unterscheiden, ob zwei
Nachbarflächen dasselbe Pixel teilen oder ob eine durch die andere scheint:
ein Wasserbecken bekam an jeder Blockgrenze eine hellere Naht, ein Boden aus
deckenden Blöcken dunkle Linien, bis Schritt 8 hatte das Goldbild sie. Siehe
[0008](../entscheidungen/0008-sprite-kanten-nicht-glaetten.md).

## Füllregel

Liegt ein Mittelpunkt genau auf der Kante, entscheidet die Füllregel der
Grafikkarten: der Pixel gehört dem Dreieck, für das die Kante oben oder
links liegt. Dafür rechnen beide Dreiecke die gemeinsame Kante von derselben
Ecke aus. Von verschiedenen Ecken aus rundet f32 bei gedrehter Geometrie
verschieden, und ein Pixel genau auf der Kante fiel bei beiden durch, bei
scale 32 derselbe Pixel in jedem Kreuzmodell. Ohne die Regel nahmen beide
Dreiecke einer Fläche die Pixel auf ihrer Diagonale an, und bei scale 2
bekam Wasser dort Alpha 233 statt 180. Geprüft ist beides an 300 zufällig
gedrehten Quadern bei scale 4 bis 64, gedreht vom Baker selbst.

## Flächen parallel zur Blickrichtung

Dabei zeigte sich eine Fläche genau parallel zur Blickrichtung: Der Baker
dreht in f32, und die Summe ihrer Normalen liegt um 1e-7 ihrer Länge neben
null, mal davor, mal dahinter. Lag sie davor, legte die Fläche einen
Streifen von 2e-7 Pixeln Breite auf die Kante ihres Nachbarn, und ein
Pixelmittelpunkt genau darauf bekam beide. Solche Flächen zählen jetzt als
abgewandt (`EDGE_ON`). Echte Drehungen liegen weit darüber: um eine Achse
in Schritten von 22,5 Grad, dazu Vielfache von 90, ist die kleinste Summe
ungleich null 0,54 der Länge. In Vanilla und im Pack haben 82
Blockstates eine, etwa Kerzen, Hängeschilder und schräge Schienen. Ihre
Pixel bleiben auf allen fünf scales gleich, nur 370 von 1705 Sprites
bekommen einen kleineren Rahmen.

## Fragmente je Pixel

Durchsichtige Flächen mischen sich auch innerhalb eines Sprites: jede
Fläche legt je Pixel ein Fragment ab, und am Schluss wird je Pixel von
hinten nach vorne gemischt. Ein durchscheinendes Texel liegt so immer über
dem, was dahinter liegt, Wasser über einem Zaunpfosten, Glas über dem Block
dahinter, gleich in welcher Reihenfolge die Flächen kommen, und eine
Halmkante, die den Pixel nur zum Teil deckt, verdeckt die Fläche dahinter
nicht. Nur ein deckendes Fragment
verwirft, was dahinter liegt. Bei gleicher Tiefe gewinnt die spätere
Fläche; die Flächen sind deshalb stabil nach ihrer vordersten Ecke
sortiert, und der Grasblock legt sein Overlay auf den Grundwürfel. Vorher
gewann ein Tiefenpuffer, und ein gefluteter Zaun war ein Wasserwürfel ohne
Zaun.

## Textur über den Pixel gemittelt

Die Textur wird über den Pixel gemittelt, sonst fiele auf einer acht Pixel
breiten Seitenfläche jeder zweite Texel weg. Gemittelt wird wie in der
Pyramide in linearem Licht und mit vormultipliziertem Alpha, siehe
[Zoomstufen](../benutzung/zoomstufen.md), „Verkleinern“. Die Abtastpunkte
bleiben dabei im Texturausschnitt der Fläche, und ihre Zahl folgt dem
scale, damit auch bei wenigen Pixeln je Block alle Texel zählen. Treffen
alle Abtastpunkte dasselbe Texel, bei scale 32 je nach Textur bei einem
Zehntel bis gut einem Drittel der Pixel, ist das Texel selbst das Mittel,
ohne Umweg über lineares Licht.

## Was bleibt eine Näherung

- **Randpixel an echten Öffnungen bleiben leer.** Liegt eine Pixelmitte
  genau auf einer Aussenkante neben einer Öffnung im Modell, gehört sie
  nach der Füllregel der Öffnung, wie auf einer Grafikkarte. Das betrifft
  bei scale 8 und 16 einzelne Pixel in Zauntoren.
- **Kanten gegen Luft sind Treppen** im 2:1-Raster statt Verläufe, die
  Silhouette, die isometrische Pixelkunst ohnehin hat.
