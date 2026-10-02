---
title: "0055: Die Welt beim Zugriff drehen"
description: Warum der Renderer für --direction im Blick rechnet und die Welt erst beim Nachschlagen dreht, Modelle im Rasterizer statt in der Blockstate, Chunks und Licht in der Lage der Welt, und warum weder Kacheln noch Bilder noch ganze Chunks gedreht werden.
status: gilt
date: 2026-10-02
issues: [68]
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
---

# 0055: Die Welt beim Zugriff drehen

## Anlass

#68 bringt `--direction` für jede Kamera: diagonal `sw`, `nw`, `ne`,
genordet `w`, `n`, `e`. Das Bild soll zeigen, was das Spiel aus dieser
Richtung zeigt. Projektion, Zeichenreihenfolge, Kandidaten und
Deckungsmaske hängen alle daran, dass die Kamera bei +x, +z steht.

## Entscheidung

- **Gerechnet wird im Blick.** Die Kamera steht dort wie aus der Vorgabe;
  `Projection` und der Renderpfad bleiben unverändert. `Richtung` in
  `projection.rs` dreht Blöcke, Chunks, Versätze, Seiten und Punkte von
  Modellen zwischen Welt und Blick.
- **Gedreht wird beim Zugriff.** Der Chunk-Cache führt Slots je Chunk im
  Blick und liest den Chunk der Welt, der dort liegt. Jeder Nachschlag
  eines Blocks, einer Zelle oder eines Bioms dreht seine Stelle in die
  Welt.
- **Die Bitmasken** legen ihre Spalten in den Blick, nur die Ebenen der
  Ausbreitung bleiben in der Welt.
- **Chunks und Licht** bleiben in der Lage der Welt. Die Ausbreitung sieht
  ihre Nachbarn so, wie sie in der Welt liegen.
- **Modelle** dreht der Rasterizer nach dem Backen in den Blick. Schattiert
  wird nach der Fläche in der Welt, weich beleuchtet mit den Tabellen ihrer
  Seite der Welt.
- **Was das Spiel an der Welt festmacht,** bleibt dort: die Saat der
  Alternativen, die Biome, die Seiten der Regeln zu gleichen Nachbarn.
- **Das Licht unbekannter Blöcke,** eine Näherung des Renderers,
  entscheidet weiter das Raster in 2:1 aus der Vorgabe-Richtung. So hat
  dieselbe Welt aus jeder Richtung dasselbe Licht.

Wie das im Einzelnen aussieht, steht in
[Richtungen](../renderer/richtungen.md).

## Verworfene Alternativen

- **Kacheln oder Bilder drehen.** Eine Drehung des Bilds zeigt die Rückseite
  nicht: Aus `nw` sieht die Kamera die Nord- und Westseiten, die aus `se`
  gar nicht gezeichnet sind. Schräg ginge es nicht einmal für die
  Oberseiten, denn die Raute von W:H ist nicht quadratisch. Von oben, in
  `top` und `top-north`, sind die Oberseiten quadratisch; aber dort, wo
  Blockkanten Pixelmitten treffen, gäbe die Füllregel einen Pixel im
  gedrehten Bild dem falschen Block, und für eine Kamera gäbe es zwei Wege
  im Code.
- **Die Richtung in den Vorzeichen der Projektion,** also die Kamera
  wirklich nach `nw` stellen. Dann änderte sich jede feste Seite in den
  heissen Schleifen: `expose`, `fluid_hidden`, `mask_bit`, die Fassungen
  aus #58, `AO_FACES` und `AO_SEITEN`, `ecken_at` und das Licht von der
  Seite. Ein Fehler zeigte sich nur in einer Richtung. Lohnen würde es
  sich erst bei freien Winkeln.
- **Die Blockstate drehen** (`BlockState.rotate`), dann die gedrehte Welt
  aus der Vorgabe zeichnen. Das braucht das Drehverhalten jedes Blocks aus
  dem Spiel nachgebaut, von `facing` über `rotation` bis zu den Formen der
  Schienen. Und das Bild wäre falsch: Schattierung, weiche Beleuchtung,
  Saat, Biome und Texturen gehörten dann zur gedrehten Welt, nicht zur
  gezeigten. Eine Textur ohne Symmetrie auf einem Block ohne Richtung, wie
  Stein, sähe aus `nw` gedreht aus, in der gedrehten Welt nicht. Siehe
  [Richtungen](../renderer/richtungen.md), „Nicht das gedrehte Bild“.
- **Ganze Chunks beim Laden drehen,** Blöcke, Biome und Blockentities in
  neue Sections umgelegt. Dann müssten auch die Formen der Ausbreitung
  mitgedreht werden, die je Seite der Welt gelten, und das Licht hinge an
  der Kamera. Beim Zugriff dreht nur, wer nachschlägt.
- **Die Bitmasken in der Lage der Welt lassen** und jede Leseweise drehen.
  Das wären `expose`, die Kandidaten, die Ränder, `umgebung` und `zelle`,
  jede Wortoperation über Spalten mit einer Umrechnung je Spalte. So
  dreht `Masks::of` einmal je Klasse, 256 Wörter.
- **Modelle in der Sprite-Tabelle drehen,** gleich nach dem Backen. Dann
  wären `full_height`, die Masken der Flüssigkeit und die Regeln zu
  gleichen Nachbarn Seiten im Blick, die Schattierung aber nicht. Im
  Rasterizer bleibt jedes Modell bis zuletzt in der Welt.

## Folgen

- Aus der Vorgabe-Richtung ändert sich kein Pixel; jede Drehung entfällt
  dort, auch das Umlegen der Masken.
- Jede Richtung braucht ihre eigene Sprite-Tabelle und ihren eigenen
  Kachelbaum, siehe [0054](0054-baeume-unter-einer-wurzel.md).
- Was künftig an der Lage hängt, muss beim Zugriff drehen und einen Test
  aus jeder Richtung bekommen. Welche Wege die Tests heute prüfen, steht in
  [Richtungen](../renderer/richtungen.md), „Belegt“; was dort fehlt, ist
  ungeprüft.
