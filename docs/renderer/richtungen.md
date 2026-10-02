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
Vierteldrehungen nach der Tabelle in
[`map.json`](../benutzung/map-json.md), „Kamera und Projektion“. Was das
Spiel an der Lage in der Welt festmacht, bleibt in der Welt. Entschieden
in [0055](../entscheidungen/0055-welt-beim-zugriff-drehen.md).

## Im Blick

- **Slots des Chunk-Caches** gehören je zu einem Chunk im Blick
  (`ChunkCache::slot` in `metatile.rs`). `load` liest den Chunk der Welt,
  der dort liegt.
- **Bitmasken** stehen je Spalte im Blick, siehe
  [Der Weg einer Kachel](renderpfad.md), „Bitmasken“. `Masks::of` legt die
  Spalten der Welt dafür um (`spalten_im_blick` in `metatile.rs`); aus der
  Vorgabe-Richtung entfällt das.
- **Nachbarn** für Deckung, Wasser, Lava und die weiche Beleuchtung fragt
  der Cache an Stellen im Blick.
- **Modelle** baut die Sprite-Tabelle in der Welt. Der Rasterizer dreht
  jede Fläche um die Mitte ihres Blocks in den Blick (`quad_im_blick` in
  `rasterizer.rs`), bevor er sie projiziert; `faces_camera` und
  `auf_den_vorderseiten` dort drehen ebenso.
- **Masken der Flüssigkeit** (`mask_bit` in `sprites.rs`) nennen Seiten im
  Blick. Eine Fläche der Flüssigkeit trägt ihre Seite der Welt; die
  Sprite-Tabelle dreht sie für die Maske (`Richtung::seite_in_den_blick`).
  Die Streifen über niedrigeren Nachbarn (`SpriteSet::insert_strips`)
  stehen je Seite im Blick und entstehen aus der Seite der Welt, die dort
  liegt.
- **Die andere Hälfte einer Doppelkiste** legt die Familie relativ im
  Blick ab.
- **Vorlauf und `--center`:** Der Kasten einer Spalte (`column_box` in
  `tiles.rs`) dreht seine Ecken in den Blick, `--center` den Block
  (`window` in `cli.rs`).

## In der Welt

- **Chunks und ihr Licht** liegen im Cache wie in der Welt. Die
  Ausbreitung nimmt die acht Nachbarn, wie sie in der Welt um den Chunk
  liegen (`licht_slot` in `metatile.rs`), dazu die Ebenen `DAEMPFT` und
  `DICHT` sowie `formen` und `quellen` in der Lage der Welt. Das Licht
  einer Zelle liest `zelle` dort an ihrem Platz in der Welt, auch ob ein
  Block voll hell ist.
- **Blöcke:** `block_at` und `family_at` in `metatile.rs` schlagen den
  Block an seinem Platz in der Welt nach, die Varianten aus Blockentities
  ebenso.
- **Die Alternative** würfelt `Family::wahl` in `sprites.rs` aus der Lage
  in der Welt, siehe [Varianten aus der Position](varianten.md).
- **Biome** mischt `tints_at` in `metatile.rs` um den Block in der Welt,
  mit dem Zoom des Spiels, siehe [Biomfarben](biomfarben.md).
- **Schattierung nach Richtung:** `shade_factor` in `rasterizer.rs` nimmt
  die Fläche der Welt, auch die eines Blockentities,
  siehe [Dimensionstypen](dimensionstypen.md), „Schattierung nach
  Richtung“. Aus `sw` liegt links die Westseite mit 0,6, rechts die
  Südseite mit 0,8; aus `nw` links der Norden, rechts der Westen, so hell
  wie aus `se`.
- **Weiche Beleuchtung:** Jede Seite im Blick rechnet mit den Tabellen der
  Seite der Welt, die dort liegt, siehe
  [Weiche Beleuchtung](weiche-beleuchtung.md), „Aus jeder Richtung“.
- **Flächen zu gleichen Nachbarn:** Die Regel fragt nach der Seite der Welt
  (`Family::nachbarseiten` in `sprites.rs`), den Nachbarn dort sucht der
  Cache im Blick,
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
- **Texturen** liegen in der Welt. Eine Textur ohne Symmetrie auf einem
  Block ohne Richtung, wie Stein, sieht aus einer anderen Richtung gedreht
  aus.

Halb gedreht gleicht das Bild einer gedrehten Welt dem der Vorgabe deshalb
nur dort, wo Texturen, Alternativen, Biome und der Ersatz für den Block in
der Ecke nicht abweichen. Die Tests nehmen dafür einfarbige Texturen und
Blöcke ohne Alternativen. Auch dann bleiben zwei kleine Abweichungen:

- **Gerundet:** Die Ecken einer Fläche kommen im Blick in anderer
  Reihenfolge, und das Mischen ihrer Werte rundet eine Farbe um eins anders.
- **Blockentities nach Osten und Westen:** Die Lagen des Spiels tragen dort
  0,99999994 statt 1 (`blockentities.txt`), nach Norden und Süden sind sie
  genau. Trifft eine Kante eine Pixelmitte, kippt ein Pixel nach der
  Füllregel. Eine Vierteldrehung macht aus Norden Osten.

## Kosten

Aus der Vorgabe kostet die Drehung nichts Messbares, und aus `nw` kostet
2:1 so viel wie aus `se`, gemessen in
[2026-10-02, Richtungen](../messungen/2026-10-02-richtungen.md).

## Belegt

In `renderer/tests/richtung.rs`:

- `gedreht_wie_der_gedrehte_block`: Halb gedreht, aus `nw` und in
  `north-45` aus `n`, zeigt ein Block mit einer Textur ohne Symmetrie Pixel
  für Pixel, was aus der Vorgabe derselbe mit `y: 180` zeigt, in 2:1 bei
  scale 16 und 32, in 4:3 und in `north-45`. Von oben, aus `sw` in `top`
  und aus `w` in `top-north`, zeigt ein Block mit `y: 90`, was aus der
  Vorgabe der ungedrehte zeigt.
- `seiten_so_hell_wie_in_der_welt`: die Seiten links und rechts je
  Richtung, in 2:1 und `north-45`, an einem Block und an einem Topf im
  Licht der Entities.
- `gedrehte_szene_wie_aus_der_vorgabe`: Treppen in allen Formen, Platten,
  Türen, Zäune und Scheiben, die sich verbinden, Eis, Licht unter einem
  Dach mit einer Quelle und einem voll hellen Block, weich beleuchtete
  Bretter, Teile in fremden Würfeln, auch in einem belegten, eine
  Doppelkiste, ein Topf und Wasser in Stufen, alles in der Luft über die
  Grenzen von vier Chunks. Die Szene wird in der Welt um k Vierteldrehungen
  gedreht und aus der Richtung k gerendert, für k = 1, 2, 3 und je Kamera
  2:1, 4:3, `top`, `top-north` und `north-45`; gedreht liegt sie bei
  negativen Koordinaten. Halb gedreht gleicht das Bild dem der Vorgabe im
  Alpha genau und in der Farbe bis auf eins je Kanal. Aus k = 1 und 3
  gleicht es ihm im Alpha, also in Geometrie und Weglassen, und beide
  Bilder gleichen einander wie halb gedreht. Die Blockentities, nach Süden
  und Norden, zählen nur halb gedreht, siehe „Nicht das gedrehte Bild“. Die
  Drehung der Blockstates rechnet der Test selbst aus der Tabelle, nicht
  mit dem Renderer.
- `licht_der_welt_aus_jeder_richtung`: das Licht jeder Zelle der Szene aus
  `common::szene` und um sie herum, aus allen vier Richtungen.
- `licht_unbekannter_bloecke_aus_der_vorgabe`: Ein Block, den 26.2 nicht
  kennt und der nur aus `se` seinen Umriss deckt, hält aus jeder Richtung
  das Licht auf wie aus der Vorgabe.
- `alternative_und_biom_aus_der_welt`: die Mitte jeder Oberseite einer
  Schicht aus `zufall` und Gras über zwei Biome, in 2:1 und `top-north`.
- `flaechen_zu_gleichen_nachbarn_aus_der_welt`: Eis in einem Winkel aus
  drei Armen, ebenso gedreht.
- `wasser_aus_der_welt`: Wasser in Stufen in der Luft, ebenso gedreht; aus
  der Vorgabe zeigt es nach Osten einen Streifen.

In `renderer/tests/metatile.rs` läuft jede Kamera der Invarianten aus der
Vorgabe und aus einer anderen Richtung, reihum: Der schnelle Weg gleicht
der Referenz, kleine Ausschnitte dem grossen Bild, Verdecken ändert kein
Pixel, deckendes Gelände hat kein Loch, und höher gesetzt bleibt das Bild
gleich. Die Doppelkiste liegt aus jeder Richtung im helleren Licht beider
Hälften (`doppelkiste_im_helleren_licht_beider_haelften`), und ein
Goldbild zeigt die Szene aus `nw` um die Treppe aus Stein
(`metatile-nw.png`).

In `renderer/src/render/metatile.rs` prüft
`ecken_im_blick_passen_zu_den_nachbarn` die Tabellen der weichen
Beleuchtung je Richtung, in `renderer/src/render/rasterizer.rs`
`ecken_der_seiten_im_blick` ihre Ecken, beide aus `nw` und `sw` mit festen
Zahlen. In `renderer/tests/cli.rs` prüft `richtung_ist_ein_eigener_baum`
den Lauf mit `--direction` aus `nw` und `sw`,
`center_in_der_welt_aus_jeder_richtung` `--center`.
