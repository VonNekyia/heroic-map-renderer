---
title: "0008: Sprite-Kanten werden nicht geglättet"
description: Warum der Rasterizer die Geometrie nur im Pixelmittelpunkt prüft, mit der Füllregel der Grafikkarten, und die Textur trotzdem über den Pixel mittelt.
status: gilt
date: 2026-09-25
issues: [8]
code:
  - renderer/src/render/rasterizer.rs
---

# 0008: Sprite-Kanten werden nicht geglättet

Die Treppen liegen seit [0051](0051-kameras-und-richtungen.md) im Raster
der Kamera des Laufs, nicht mehr nur in 2:1.

## Anlass

Das zweite Review von #8 wies Nähte nach: Jede Oberseite wurde einzeln mit
2×2 Abtastpunkten gerastert und im Sprite reduziert. An einer gemeinsamen
Kante steckten danach Teildeckung und Materialtransparenz zusammen im
Alpha, und das Mischen beim Zusammensetzen konnte nicht wissen, dass zwei
Nachbarn dasselbe Pixel teilen: Alpha 156 statt 180 an inneren
Wasserkanten, dunkle Linien auf jedem Boden. Das alte Goldbild hatte sie
auch.

## Entscheidung

Die Geometrie wird nur im Pixelmittelpunkt geprüft: an einer gemeinsamen
Kante gehört jeder Pixel genau einer der beiden Flächen. Liegt ein
Mittelpunkt genau auf einer Kante, entscheidet die Füllregel der
Grafikkarten, oben-links; beide Dreiecke rechnen die gemeinsame Kante von
derselben Ecke aus. Die Textur wird weiter über den Pixel gemittelt, in
linearem Licht. Siehe [Rastern ohne Nähte](../renderer/naehte.md).

## Verworfene Alternativen

- **Teildeckung bis zum Zusammensetzen mitführen.** Das hätte jedes Sprite
  und jede Kachel mit vierfacher Pixelzahl gerastert; so verschwinden die
  Nähte ohne einen Pixel mehr.
- **Zweifache Überabtastung der Geometrie**, wie seit Schritt 3: die
  Ursache der Nähte.

## Folgen

- Kanten gegen Luft sind Treppen im 2:1-Raster statt Verläufe, die
  Silhouette, die isometrische Pixelkunst ohnehin hat.
- Das Goldbild wurde neu erzeugt.
- Randpixel an echten Öffnungen gehören nach der Füllregel der Öffnung;
  bei scale 8 und 16 betrifft das einzelne Pixel in Zauntoren.
