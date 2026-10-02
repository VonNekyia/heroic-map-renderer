---
title: Richtungen
description: Wie der Renderer die Welt aus sw, nw und ne und genordet aus w, n und e zeigt, was dabei im Blick liegt und was in der Welt bleibt, nach welcher Seite Flächen schattiert werden, warum ein Bild aus einer Richtung nicht das gedrehte Bild der Vorgabe ist und welche Tests das belegen.
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
  - renderer/tests/richtung.rs
  - renderer/tests/metatile.rs
---

# Richtungen

Aus jeder Richtung rechnet der Renderer im Blick: Die Kamera steht dort wie
aus der Vorgabe bei +x, +z. Projektion, Zeichenreihenfolge, Kandidaten und
Deckung bleiben, wie sie sind. Gedreht wird die Welt beim Zugriff, in
Vierteldrehungen nach der Tabelle in [Die Kamera](kamera.md),
„Richtungen“. Was das Spiel an der Lage in der Welt festmacht, bleibt in
der Welt. Entschieden in
[0055](../entscheidungen/0055-welt-beim-zugriff-drehen.md).

## Im Blick

- **Slots des Chunk-Caches** gehören je zu einem Chunk im Blick
  (`ChunkCache::slot`). `load` liest den Chunk der Welt, der dort liegt.
- **Bitmasken** stehen je Spalte im Blick, siehe
  [Der Weg einer Kachel](renderpfad.md), „Bitmasken“. `Masks::of` legt die
  Spalten der Welt dafür um (`spalten_im_blick`); aus der Vorgabe-Richtung
  entfällt das.
- **Nachbarn** für Deckung, Wasser, Lava und die weiche Beleuchtung fragt
  der Cache an Stellen im Blick.
- **Modelle** baut die Sprite-Tabelle in der Welt. Der Rasterizer dreht
  jede Fläche um die Mitte ihres Blocks in den Blick (`quad_im_blick`),
  bevor er sie projiziert; `faces_camera` und `auf_den_vorderseiten` drehen
  ebenso.
- **Masken der Flüssigkeit** (`mask_bit`) nennen Seiten im Blick. Eine
  Fläche der Flüssigkeit trägt ihre Seite der Welt; die Sprite-Tabelle
  dreht sie für die Maske (`seite_in_den_blick`). Die Streifen über
  niedrigeren Nachbarn stehen je Seite im Blick und entstehen aus der
  Seite der Welt, die dort liegt.
- **Die andere Hälfte einer Doppelkiste** legt die Familie relativ im
  Blick ab.
- **Vorlauf und `--center`:** Der Kasten einer Spalte (`column_box` in
  `tiles.rs`) dreht seine Ecken in den Blick, `--center` den Block.

## In der Welt

- **Chunks und ihr Licht** liegen im Cache wie in der Welt. Die
  Ausbreitung nimmt die acht Nachbarn, wie sie in der Welt um den Chunk
  liegen (`licht_slot`), dazu die Ebenen `DAEMPFT` und `DICHT` sowie
  `formen` und `quellen` in der Lage der Welt. Das Licht einer Zelle liest
  `zelle` an ihrem Platz in der Welt.
- **Blöcke:** `block_at` und `family_at` schlagen den Block an seinem Platz
  in der Welt nach, die Varianten aus Blockentities ebenso.
- **Die Alternative** würfelt `Family::wahl` aus der Lage in der Welt,
  siehe [Varianten aus der Position](varianten.md).
- **Biome** mischt `tints_at` um den Block in der Welt, mit dem Zoom des
  Spiels, siehe [Biomfarben](biomfarben.md).
- **Schattierung nach Richtung:** `shade_factor` nimmt die Fläche der Welt,
  siehe [Dimensionstypen](dimensionstypen.md), „Schattierung nach
  Richtung“. Aus `sw` liegt links die Westseite mit 0,6, rechts die
  Südseite mit 0,8; aus `nw` links der Norden, rechts der Westen, so hell
  wie aus `se`.
- **Weiche Beleuchtung:** Jede Seite im Blick rechnet mit den Tabellen der
  Seite der Welt, die dort liegt, siehe
  [Weiche Beleuchtung](weiche-beleuchtung.md), „Aus jeder Richtung“.
- **Flächen zu gleichen Nachbarn:** Die Regel fragt nach der Seite der Welt
  (`Family::nachbarseiten`), den Nachbarn dort sucht der Cache im Blick,
  siehe [Sprites und Deckung](sprites-und-deckung.md), „Flächen zu gleichen
  Nachbarn“.
- **Licht unbekannter Blöcke:** Ob ein Block, den 26.2 nicht kennt, das
  Licht aufhält, entscheidet das Raster in 2:1 aus der Vorgabe-Richtung,
  siehe [Wasser und Licht](wasser-und-licht.md), „Was bleibt eine
  Näherung“.
- **Höhen** für die Koordinaten hängen nur an der Welt, siehe
  [0054](../entscheidungen/0054-baeume-unter-einer-wurzel.md).

## Nicht das gedrehte Bild

Ein Bild aus einer anderen Richtung ist nicht das Bild der Vorgabe von
einer gedrehten Welt. Das Spiel legt einiges in der Welt fest, und der
Renderer auch:

- **Schattierung:** Bei einer Vierteldrehung tauschen die Seiten links und
  rechts ihre Helligkeit, bei einer halben nicht.
- **Weiche Beleuchtung:** Das Spiel teilt jede Fläche in der Reihenfolge
  der Ecken ihrer Seite der Welt in zwei Dreiecke. Aus `sw` und `ne` läuft
  die Diagonale der Oberseite deshalb über die anderen Ecken als aus `se`.
  Wo der Block in der Ecke nicht zählt, nimmt das Spiel den ersten Nachbarn
  der Seite der Welt, für den Norden den Block oben, für den Süden den im
  Westen.
- **Alternativen und Biome:** Sie hängen an der Lage in der Welt, eine
  gedrehte Welt würfelte anders.
- **Blockentities:** Ihr Licht kommt aus zwei Richtungen der Welt, siehe
  [Blockentities](blockentities.md), „Licht“.

## Belegt

In `renderer/tests/richtung.rs`:

- `gedreht_wie_der_gedrehte_block`: Aus `nw` zeigt ein Block mit einer
  Textur ohne Symmetrie Pixel für Pixel, was aus `se` derselbe mit `y: 180`
  zeigt, in 2:1 bei scale 16 und 32, in 4:3 und in `north-45`. Von oben,
  aus `sw` in `top` und aus `w` in `top-north`, zeigt ein Block mit
  `y: 90`, was aus der Vorgabe der ungedrehte zeigt.
- `seiten_so_hell_wie_in_der_welt`: die Seiten links und rechts je
  Richtung, in 2:1 und `north-45`.
- `licht_der_welt_aus_jeder_richtung`: das Licht jeder Zelle der Szene aus
  `common::szene` und um sie herum, aus allen vier Richtungen.
- `alternative_und_biom_aus_der_welt`: die Mitte jeder Oberseite einer
  Schicht aus `zufall` und Gras über zwei Biome, in 2:1 und `top-north`.
- `flaechen_zu_gleichen_nachbarn_aus_der_welt`: Eis in einem Winkel aus
  drei Armen aus `nw` wie derselbe Winkel halb gedreht aus `se`.
- `wasser_aus_der_welt`: ein Becken mit einer Quelle und fliessendem
  Wasser in Stufen, ebenso.

In `renderer/tests/metatile.rs` gehen die Kameras der Invarianten der
Reihe nach durch alle vier Richtungen: Der schnelle Weg gleicht der
Referenz, kleine Ausschnitte dem grossen Bild, Verdecken ändert kein Pixel,
deckendes Gelände hat kein Loch, und höher gesetzt bleibt das Bild gleich.
Die Doppelkiste liegt aus jeder Richtung im helleren Licht beider Hälften
(`doppelkiste_im_helleren_licht_beider_haelften`), und ein Goldbild zeigt
die Szene aus `nw` (`metatile-nw.png`).
In `renderer/src/render/metatile.rs` prüft `ecken_im_blick_passen_zu_den_nachbarn`
die Tabellen der weichen Beleuchtung je Richtung, in
`renderer/tests/cli.rs` `richtung_ist_ein_eigener_baum` den Lauf mit
`--direction`.
