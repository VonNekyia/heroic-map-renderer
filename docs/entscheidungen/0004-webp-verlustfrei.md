---
title: "0004: Kacheln als verlustfreies WebP"
description: Warum die Kacheln verlustfreies WebP sind und nicht PNG oder verlustbehaftetes WebP.
status: gilt
date: 2026-09-22
issues: [5]
code:
  - renderer/src/render/tiles.rs
---

# 0004: Kacheln als verlustfreies WebP

## Anlass

Schritt 5 schreibt die ersten Kacheln. Das Format entscheidet über Platz
und Aussehen jeder Kachel.

## Entscheidung

Die Kacheln sind verlustfreies WebP (`encode_webp`).

## Verworfene Alternativen

- **Verlustbehaftetes WebP.** Minecraft-Texturen sind Pixelkunst mit wenigen
  flachen Farben; verlustbehaftet würde daraus Matsch, und an den
  Kachelrändern sähe man die Artefakte im Raster, genau dort, wo das Auge
  beim Scrollen hinsieht.
- **PNG.** Ein Bild von 256 × 256 Pixeln wog als PNG 173 kB, die Kacheln um
  denselben Punkt als verlustfreies WebP im Mittel 137 kB, gut ein Fünftel
  weniger, siehe
  [2026-09-22, WebP gegen PNG](../messungen/2026-09-22-webp-gegen-png.md).

## Folgen

- Der damalige Encoder aus `image` konnte ohnehin nur verlustfrei;
  verlustbehaftet hätte libwebp und damit einen C-Compiler verlangt.
- Seit [0028](0028-libwebp-statt-image.md) packt libwebp, weiterhin
  verlustfrei und Pixel für Pixel gleich, auf ein Drittel der Grösse.
