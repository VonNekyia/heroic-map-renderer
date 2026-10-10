---
title: "0036: Höhen aus der Heightmap, je 4×4"
description: Warum die Höhen für die Koordinatenanzeige aus der gespeicherten Heightmap WORLD_SURFACE kommen, im Vorlauf gelesen, je 4×4 Spalten der obere Median und über Wasser die Oberfläche, statt je Spalte in einem eigenen Durchgang.
status: gilt
date: 2026-09-28
issues: [29]
code:
  - renderer/src/render/heights.rs
  - renderer/src/world/chunk.rs
  - renderer/src/render/tiles.rs
---

# 0036: Höhen aus der Heightmap, je 4×4

Ergänzt durch [0103](0103-boden-ohne-laub.md): Daneben steht der Boden
ohne Laub aus `MOTION_BLOCKING_NO_LEAVES`, für Formen auf dem Gelände.

## Anlass

Die Koordinatenanzeige des Frontends bricht den Strahl eines Pixels an
einer Höhe ab (#29). Die erste Fassung von #35 schrieb je Spalte den
obersten Block, den der Renderer zeichnet, ohne reine Flüssigkeit. Dafür
lief nach der Sprite-Tabelle ein eigener Durchgang durch die Welt, auf der
grossen Welt 57 bis 61 s, und die Höhen wogen 249 MB. Gewünscht ist
möglichst viel Effizienz bei einer Anzeige, die sich richtig anfühlt, aber
nicht genau sein muss.

## Entscheidung

- **Quelle:** die Heightmap `WORLD_SURFACE`, die das Spiel ab dem Status
  `carvers` in jedem Chunk speichert: je Spalte der oberste Block, der nicht
  Luft ist. Der Vorlauf liest sie mit, der eigene Durchgang entfällt. Fehlt
  sie einem Chunk oder passt sie nicht zu ihm, rechnet der Vorlauf sie aus
  den Blöcken, die er ohnehin dekodiert.
- **Auflösung:** je 4×4 Spalten ein Wert, der obere Median der Spalten mit
  Block, 128 × 128 Werte je Region. `map.json` nennt die Zellgrösse in
  `heightsCell`.
- **Wasser:** Über Wasser nennt die Anzeige die Oberfläche, nicht den Grund,
  denn `WORLD_SURFACE` zählt Wasser mit. So zeigt der Strahl, was man sieht.

Vom User am 28.09. entschieden. Die Zahlen stehen in
[2026-09-28, Höhen](../messungen/2026-09-28-hoehen.md), „Auflösung“, das
Format in [map.json](../benutzung/map-json.md), „Höhen“.

## Verworfene Alternativen

- **Je Spalte ohne Flüssigkeiten, in eigenem Durchgang**, die erste Fassung
  von #35. So zielt das Spiel: `LocalPlayer.pick` ruft `Entity.pick(…,
  false)` auf, und das sucht mit `ClipContext.Fluid.NONE` durch Wasser
  hindurch (Client 26.2, per javap).
  Sie kostet einen Durchgang und 249 MB, ist an Land gegen das, was man
  sieht, nicht genauer als je 4×4 und liegt über Wasser um dessen Tiefe
  daneben.
- **Je 2×2:** an Land genauer, aber dreimal so gross, 43,6 statt 13,7 MB
  auf der grossen Welt.
- **Je 8×8 und je 16×16:** kleiner, an Land aber deutlich ungenauer.
- **`OCEAN_FLOOR` je 4×4**, der Grund, worauf das Spiel zielt: an Land
  gleich gut, über Wasser aber der Grund statt der Oberfläche.
- **`MOTION_BLOCKING_NO_LEAVES`:** Unter Laub nennt sie den Boden, an Land
  liegt sie aber seltener nah am Block, den das Bild zeigt.
- **Eine feste Höhe:** kostet nichts, trifft an Land aber nur, was auf
  Meereshöhe liegt.

## Folgen

- Der Vorlauf dekodiert je Chunk ein Long-Array mehr und rechnet 16
  Mediane. Auf der grossen Welt sind die Höhen 13,7 MB.
- `--heights` braucht nur noch den Vorlauf, keine Assets.
- Ein Ausschnitt schreibt die Höhen jedes Chunks, den er liest; sie hängen
  nicht mehr an der Sprite-Tabelle.
- Blöcke, die der Renderer nicht zeichnet, zählen mit, etwa Barrieren und
  Licht. Das ist selten.
- Unter Laub nennt die Anzeige die Krone.
