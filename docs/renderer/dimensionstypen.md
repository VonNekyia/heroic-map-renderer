---
title: Dimensionstypen
description: Welchen Dimensionstyp der Renderer der gezeichneten Dimension gibt, aus der Tabelle des Spiels und aus Datenwurzeln, was er von ihm liest und wie die Seiten danach schattiert werden, belegt am Client 26.2.
code:
  - renderer/src/assets/dimension.rs
  - renderer/src/assets/dimensionstypen.txt
  - renderer/src/assets/Dimensionstypen.java
  - renderer/src/assets/mod.rs
  - renderer/src/assets/pack.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/cli.rs
---

# Dimensionstypen

Jede Dimension hat einen Typ, und der bestimmt im Spiel, wie hell Blöcke
aussehen: wie die Seiten nach ihrer Richtung abschattiert werden
(`cardinal_light`), ob es Himmelslicht gibt (`has_skylight`) und die Farben
der Lightmap. Der Renderer gibt der Dimension, die ein Lauf zeichnet, ihren
Typ (`Assets::set_dimension`, `Assets::dimension_type` in
[`renderer/src/assets/dimension.rs`](../../renderer/src/assets/dimension.rs)).
Die Typen des Spiels stehen in `dimensionstypen.txt`, Datenwurzeln liegen
darüber, siehe [0041](../entscheidungen/0041-dimensionstypen-aus-dem-spiel.md).
Er nutzt alles davon: `cardinal_light` für die Schattierung nach Richtung,
`has_skylight` und die Farben für das Licht.

## Welcher Typ

Welche Dimension ein Lauf zeichnet, sagt `World::dimension`, aus dem Pfad
unter `--world`, siehe [Welten und Kennung](../benutzung/welten.md),
„Weltwurzel und Dimension“. Dazu der Typ:

- **Die drei des Spiels** haben ihren Typ fest: `minecraft:overworld`,
  `minecraft:the_nether` und `minecraft:the_end` den gleichen Namens.
- **Eigene Dimensionen** aus ihrer Definition in einer Datenwurzel,
  `<namespace>/dimension/**/*.json`, wie `LevelStem.CODEC`: `type` ist die
  ID eines Typs oder der Typ selbst.
- **Der Typ** kommt aus den Datenwurzeln, `<namespace>/dimension_type/**/*.json`,
  sonst aus der Tabelle des Spiels. Spätere Wurzeln überschreiben frühere
  und die Typen des Spiels, wie gestapelte Datenpakete.
- **Sonst der der Oberwelt,** mit einer Meldung in der Ausgabe: ohne
  Weltwurzel, also nur mit einem Ordner `region`; für eine eigene Dimension,
  die keine Datenwurzel definiert; für einen Typ, den niemand kennt.

Die Ausgabe nennt die Dimension (`Dimension:`), dazu je Datenwurzel, wie
viele Dimensionen und Typen sie bringt, und jede Datei, die der Codec
ablehnt.

## Was der Renderer liest

Wie `DimensionType.DIRECT_CODEC` in 26.2, so weit er es braucht:

| Feld | Vorgabe, wenn es fehlt | Oberwelt | Nether | Ende |
|---|---|---|---|---|
| `has_skylight` | Pflicht | wahr | falsch | wahr |
| `cardinal_light` | `default` | `default` | `nether` | `default` |
| `visual/ambient_light_color` | `#000000` | `#0a0a0a` | `#302821` | `#3f473f` |
| `visual/sky_light_factor` | 1 | 1 | 0 | 0 |
| `visual/sky_light_color` | `#ffffff` | `#ffffff` | `#7a7aff` | `#ac60cd` |
| `visual/block_light_tint` | `#ffd88c` | `#ffd88c` | `#ffd88c` | `#ffd88c` |

- Die vier Attribute stehen unter `attributes`, als
  `minecraft:visual/…`. Ihre Vorgaben sind die aus `EnvironmentAttributes`;
  ein Typ, der ein Attribut setzt, gilt darüber wie der Constant-Layer in
  `EnvironmentAttributeSystem.addDimensionLayer`.
- Eine Farbe liest der Renderer wie `ExtraCodecs.STRING_RGB_COLOR`, siehe
  [Biomfarben](biomfarben.md), „Biome lesen“; `sky_light_factor` muss von 0
  bis 1 reichen (`AttributeRange.UNIT_FLOAT`).
- Ein Typ, den der Codec ablehnt, fehlt: `has_skylight` fehlt oder ist kein
  Wahrheitswert, `cardinal_light` ist weder `default` noch `nether`, ein
  Attribut hat einen falschen Wert. Der Client lüde sein Datenpaket nicht.
- `overworld_caves` steht auch in der Tabelle; das Spiel nimmt ihn für
  keine seiner drei Dimensionen.

Die Lightmap aus den vier Attributen und ob der Lauf Himmelslicht
ausbreitet, legt `SpriteSet::build_in` aus dem Typ fest, siehe
[Wasser und Licht](wasser-und-licht.md), „Helligkeit wie im Spiel“.

## Schattierung nach Richtung

Die Seiten eines Blocks macht das Spiel je nach Richtung dunkler, im Sprite
schon beim Rastern (`shade_factor` in
[`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs)).
Die Faktoren sind die von `CardinalLighting.DEFAULT` und
`CardinalLighting.NETHER`, `byFace` je Seite:

| | unten | oben | Norden, Süden | Westen, Osten |
|---|---|---|---|---|
| `default` | 0,5 | 1 | 0,8 | 0,6 |
| `nether` | 0,9 | 0,9 | 0,8 | 0,6 |

- **Blöcke:** `BlockModelLighter.prepareQuadFlat` und
  `prepareQuadAmbientOcclusion` nehmen `byFace` der Richtung der Fläche,
  eine Fläche ohne `shade` bekommt `up()`, im Nether also 0,9.
- **Flüssigkeiten:** `FluidRenderer` nimmt für die Oberseite `up()`, für die
  Unterseite `down()` und für die Seiten `up()` mal `north()` oder
  `west()`. Im Nether sind die Seiten also 0,72 und 0,54 hell, in der
  Oberwelt wie bei Blöcken.
- **Entity-Modelle** liegen im Licht der Entities, dessen zweite Richtung
  im Nether von unten kommt, siehe [Blockentities](blockentities.md),
  „Licht“.

Welche Schattierung ein Lauf nimmt, legt `SpriteSet::build_in` aus dem Typ
fest. Die Faktoren stecken damit im Sprite, auf der CPU wie auf der Karte.

## Belege

Belegt per javap am Client 26.2 und an den Daten im JAR:

- `DimensionType` und sein Codec (`createDirectCodec`): Felder, Pflicht und
  Vorgaben; `CardinalLighting`, `CardinalLighting.Type` mit `default` und
  `nether`.
- `DimensionTypes.bootstrap` und die JSON-Dateien unter
  `data/minecraft/dimension_type/`: Nur `the_nether` setzt
  `cardinal_light` `nether` und `has_skylight` falsch.
- `EnvironmentAttributes`, statischer Initialisierer: die Vorgaben der vier
  Attribute. `EnvironmentAttributeMap`, `AttributeTypes.RGB_COLOR`,
  `EnvironmentAttributeSystem.addDefaultLayers`.
- `GameRenderer.setLevel` ruft `Lighting.updateLevel` mit
  `cardinalLightType` des Typs; `Lighting` mit den beiden Lichtpaaren.
- `BlockModelLighter.prepareQuadFlat`,
  `BlockModelLighter.prepareQuadAmbientOcclusion`, `FluidRenderer` mit
  `BlockAndTintGetter.cardinalLighting`.

Die Tabelle schreibt `Dimensionstypen.java` aus dem Spiel selbst, über
`VanillaRegistries.createLookup`, siehe
[Erzeugte Tabellen](../entwicklung/tabellen.md). Die Tests
`tabelle_wie_im_spiel`, `seiten_wie_cardinal_lighting`,
`typ_wie_der_codec`, `dimension_findet_ihren_typ` in `dimension.rs` und
`nether_schattiert_wie_im_spiel` in `tests/cli.rs` halten das fest.

## Was bleibt eine Näherung

- **Modifikatoren:** Setzt ein Typ eines der vier Attribute nicht mit einem
  Wert, sondern mit einem Modifikator (`{"modifier": …}`), rechnet der
  Renderer ihn nicht und nimmt die Vorgabe. Die Ausgabe nennt jede solche
  Datei. Vanilla setzt keinen.
- **Nur die gelesenen Felder** prüft der Renderer. Was der Codec sonst
  verlangt, etwa `height` oder `infiniburn`, fehlt einem Typ aus einer
  Datenwurzel womöglich, ohne dass er es merkt.
- **Die drei Dimensionen des Spiels** haben ihren Typ fest. Ein
  Datenpaket, das etwa `minecraft:the_nether` in `dimension/` neu
  definiert, ändert den Typ nicht; ein neuer Typ gleichen Namens unter
  `dimension_type/` dagegen schon.
- **Biome und Zeitleisten** können die Attribute im Spiel weiter ändern.
  In Vanilla setzt kein Biom eines der vier Attribute, und die Zeitleiste
  `day` der Oberwelt lässt sie am Tag, wie sie sind.
