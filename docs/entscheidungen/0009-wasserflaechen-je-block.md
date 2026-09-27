---
title: "0009: Wasserflächen je Block nach den Nachbarn"
description: Warum jede Flüssigkeit acht Fassungen ohne die Flächen zu gleichem Nachbarwasser bekommt und an Stufen einen Streifen statt angehobener Ecken.
status: gilt
date: 2026-09-25
issues: [8]
code:
  - renderer/src/assets/fluid.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0009: Wasserflächen je Block nach den Nachbarn

## Anlass

In der ersten Fassung von #8 wurde jede innere Fläche mitgezeichnet: An
jeder Blockgrenze lag Wasser über Wasser, Alpha 220 statt 180, und über dem
Grund lag ein Raster.

## Entscheidung

Ein Sprite kennt seine Nachbarn nicht, der Renderer schon. Jede Blockstate
mit Flüssigkeit bekommt acht Fassungen, eine je Kombination aus „Nachbar
+x, +y, +z führt dieselbe Flüssigkeit“, und der Renderpfad wählt die
passende nach den Nachbarn. Zur selben Flüssigkeit nebenan entfällt die
Seitenfläche, wie bei `shouldRenderFace`; darüber verdeckt dieselbe
Flüssigkeit die eigene Oberfläche. Steht der Nachbar tiefer, am Fuss eines
Wasserfalls oder an einer Stufe fliessenden Wassers, zeichnet der Renderer
genau den Streifen zwischen beiden Höhen, als eigenes Sprite je
Höhenpaar. Siehe [Wasser und Licht](../renderer/wasser-und-licht.md).

## Verworfene Alternativen

- **Innere Flächen mitzeichnen:** siehe Anlass.
- **Die Ecken der Oberfläche an die Nachbarn angleichen wie Minecraft.**
  Nicht gebaut: Der Streifen schliesst dieselbe Lücke mit einem Sprite je
  Höhenpaar, und jede Oberfläche bleibt eben.

## Folgen

- Ein Wasserblock mitten im Ozean hat keine Fläche mehr und kostet nichts;
  seit [0021](0021-bitmasken-statt-blockbesuche.md) kommt er gar nicht erst
  zur Sprite-Wahl.
- Masken und Streifen brauchten bis zu fünf Nachschläge je Wasserblock.
- Die Oberfläche bleibt eben, wo Minecraft die Ecken angleicht.
