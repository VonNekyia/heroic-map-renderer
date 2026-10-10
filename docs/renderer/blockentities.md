---
title: Blockentities
description: Wie der Renderer Truhen, Shulkerkisten, Banner, Köpfe, Krüge und die übrigen Blöcke mit Blockentity-Renderer aus den Modellen des Spiels zeichnet, in welchem Licht, wie Bannermuster und Scherben aus dem Chunk dazukommen und wie ein Banner ohne Welt, auch mit Krone, zum Sprite für die Ebenen wird.
code:
  - renderer/src/assets/blockentity.rs
  - renderer/src/assets/Blockentities.java
  - renderer/src/assets/blockentities.txt
  - renderer/src/world/chunk.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/banner.rs
  - renderer/src/render/krone.png
  - renderer/tests/banner.rs
  - docs/bilder/quellen/krone/krone.aseprite
  - renderer/src/cli.rs
---

# Blockentities

Truhen, Shulkerkisten, Banner, Köpfe, Verzierte Krüge, Glocken, den
Aquisator, Kupfergolemstatuen und die Bücher auf Lesepult und Zaubertisch
zeichnet das Spiel ganz oder teilweise mit einem Blockentity-Renderer aus
Modellen. Ihr Blockmodell hat meist keine Elemente. Der
Renderer zeichnet sie aus der Tabelle
[`blockentities.txt`](../../renderer/src/assets/blockentities.txt), die
[`Blockentities.java`](../../renderer/src/assets/Blockentities.java) aus
dem Client 26.3 schreibt: je Zustand die Flächen, wie die Renderer des
Spiels sie abgeben, mit Lage, Textur, Schicht und Farbe. Bannermuster und
Scherben liest er aus `block_entities` im Chunk. Code in
[`renderer/src/assets/blockentity.rs`](../../renderer/src/assets/blockentity.rs).

## Die Tabelle

`Blockentities.java` lässt jeden Blockentity-Renderer des Spiels einen
Zustand nach dem anderen zeichnen:

- Je Zustand baut er das Blockentity (`newBlockEntity`), liest es mit
  `extractRenderState` und ruft `submit` mit einem Collector auf, der
  jedes `submitModel` mitschreibt.
- Gezeichnet wird wie in `ModelFeatureRenderer.prepareModel`: `setupAnim`,
  dann `renderToBuffer`, ohne Atlas. Die UV bleiben im Raum der Textur.
- Jede Zeichnung steht im Raum ihres Modells, dazu die Matrix des Aufrufs.
  Der Generator prüft, dass Matrix mal Modell genau die Ecken ergibt, die
  das Spiel zeichnet, und dass die Matrix nichts spiegelt.
- Ohne Welt nähme `extractRenderState` eine Truhe als Gegenstand: einfach,
  nach Süden, im Material nach dem Datum. Art und Lage setzt der Generator
  wie im Zweig mit Welt aus dem Zustand, das Material mit
  `getChestMaterial` ohne Weihnachten.
- Seine eigenen Prüfungen und die Renderer, die für 26.3 ohne Spiel nicht
  laufen oder nichts zeichnen, führt er als Mengen. Weicht eine ab, endet
  er mit Exit-Code 1 und schreibt keine Tabelle.

Das Format der Zeilen steht im Kopf von `Blockentities.java`. Stand 26.3, wie 26.2:
87 Blöcke, 713 Bilder, 27 Formen mit 544 Flächen, 62 Lagen, 54 Texturen,
43 Bannermuster.
Neu erzeugt wird die Tabelle mit dem Skill
[`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md),
siehe [Erzeugte Tabellen](../entwicklung/tabellen.md). Warum eine Tabelle
aus dem Spiel und nicht eigene Modelle:
[0039](../entscheidungen/0039-blockentities-aus-dem-spiel.md).

Das Bild hängt am Zustand, wie im Spiel: bei der Truhe an Art und Lage,
nicht am Wasser; ein Lesepult ohne Buch zeichnet nichts dazu. Eine Truhe
hat in jeder Lage dasselbe Blockmodell, deshalb gehört das Bild zu dem, was
eine Familie der Sprite-Tabelle ausmacht (`FamilyKey` in
[`renderer/src/render/sprites.rs`](../../renderer/src/render/sprites.rs)).

## Schichten

Jede Zeichnung trägt die Schicht, in die das Spiel sie legt, samt dem, was
deren Pipeline festlegt (`RenderPipelines` in 26.2):

| Schicht | Alpha-Test | Rückseite | Licht der Rückseite | Mischen | Blöcke |
|---|---|---|---|---|---|
| `entity_cutout_cull` | 0,1 | nein | – | nein | Truhen |
| `entity_solid` | – | nein | – | nein | Banner, Glocke, Aquisator, Krug, Bücher |
| `entity_cutout_z_offset` | 0,1 | ja | umgekehrte Normale | nein | Köpfe |
| `entity_cutout` | 0,1 | ja | umgekehrte Normale | nein | Shulkerkisten, Statuen |
| `banner_pattern` | – | nein | – | ja | Grundfarbe und Muster der Banner |

Der Rasterizer
([`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs))
nimmt sie so:

- **Alpha-Test** `ALPHA_CUTOUT`: Ein Texel unter der Schwelle verwirft er
  vor dem Mitteln, wie `entity.fsh`; 0,1 heisst Alpha unter 26 von 255.
  Was bleibt, deckt ganz, siehe [Rastern ohne Nähte](naehte.md),
  „Ausgeschnitten statt gemischt“. `entity_solid` testet nicht: Jedes Texel
  deckt.
- **Rückseite:** Eine Schicht ohne Culling (`RenderPipeline.isCull`)
  zeigt auch die Seite einer Fläche, die von der Kamera wegzeigt, mit
  `PER_FACE_LIGHTING` im Licht der umgekehrten Normalen.
- **Mischen:** Grundfarbe und Muster (`entity/banner/base` und die
  Texturen der Muster) mischen mit ihrem Alpha über das Tuch. Sie liegen in
  derselben Ebene wie dieses; bei gleicher Tiefe liegt die spätere
  Zeichnung oben, wie `order` im Spiel sie reiht.
- **Farbe:** Die Farbe einer Zeichnung multipliziert die Textur, bei
  Bannern die ihres Farbstoffs (`DyeColor.getTextureDiffuseColor`).

## Licht

Entity-Modelle liegen im Spiel nicht im Licht der Blockseiten, sondern in
dem der Entities, belegt per javap am Client 26.2:

- **Richtungen:** `Lighting.updateLevel` setzt sie nach `cardinal_light`
  im Typ der Dimension, siehe [Dimensionstypen](dimensionstypen.md), für
  `CardinalLighting.Type.DEFAULT` (0,2, 1, −0,7) und (−0,2, 1, 0,7), für
  `NETHER` (0,2, 1, −0,7) und (−0,2, −1, 0,7), je normiert
  (`CardinalLight::entity_light`).
- **Stärke:** `minecraft_mix_light` in `shaders/include/light.glsl`: je
  Richtung 0,6 mal dem Kosinus zur Normalen, nicht unter 0, dazu 0,4
  Umgebung, höchstens 1.
- **Ergebnis:** oben 1, nach Norden und Süden 0,74, nach Osten und Westen
  0,50, unten 0,4 (`entity_light`). Im Nether kommt das zweite Licht von
  unten: oben und unten je 0,885, die Seiten wie sonst.
- **Licht des Blocks:** `BlockEntityRenderState.extractBase` nimmt das
  Licht an der Position des Blockentity, wie der Renderer für jeden Block,
  siehe [Wasser und Licht](wasser-und-licht.md); für einen gefluteten Block
  an der Oberfläche dort „Was bleibt eine Näherung“.
- **Keine weiche Beleuchtung:** Die Renderer der Blockentities gehen nicht
  durch `ModelBlockRenderer`; ihre Flächen bekommen keine AO-Werte, siehe
  [Weiche Beleuchtung](weiche-beleuchtung.md).

## Daten aus dem Chunk

Zwei Blockentities tragen Daten, die ihr Bild ändern: Banner ihre Muster in
`patterns`, Verzierte Krüge ihre Scherben in `sherds`. Der Chunk liest aus
`block_entities` nur `id`, `x`, `y`, `z` und diese beiden Felder
(`Blockdaten` in
[`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs)), so wie
das Spiel sie liest:

- **Lesen:** wie `BannerBlockEntity` und `DecoratedPotBlockEntity` über
  ihre Codecs. Einen Eintrag, den der Codec ablehnt, lässt die Liste weg,
  die übrigen rücken auf (`ListCodec`, `TagValueInput.read` nimmt das
  Teilergebnis).
- **Listen aus verschiedenen Werten** speichert das Spiel als Liste von
  Compounds, jeden Wert unter dem leeren Namen, und packt sie beim Lesen
  aus (`ListTag.addAndUnwrap`); der Renderer ebenso.
- **Nicht lesbare Einträge:** `getList` gibt für ein `block_entities`, das
  keine Liste ist, nichts, und `compoundStream` übergeht Elemente, die kein
  Compound sind (`SerializableChunkData.parse`). Der Rest des Chunks gilt.
- **Lage:** `getPosFromTag` liest `x`, `y`, `z` mit `getIntOr` und 0 als
  Vorgabe: jede Zahl, eine Kommazahl abgerundet (`Mth.floor`), von einem
  Long die unteren 32 Bit. Die Weltlage ergibt sich aus der Lage im Chunk
  und dem Platz des Chunks: Steht er in der Regionsdatei an einer anderen
  Stelle, als `xPos` und `zPos` sagen, legt das Spiel ihn samt
  Blockentities an diese Stelle, der Renderer ebenso
  (`Chunk::blockentities`). Ein Blockentity ausserhalb seines Chunks rückt
  so mit seiner Lage im Chunk hinein.
- **Mehrere an einer Stelle:** Es gilt der letzte Eintrag, der sich laden
  lässt (`LevelChunk.setBlockEntity` endet in `put`). Einer, der nicht zum
  Block passt, ersetzt keinen, siehe unten; der Renderer behält deshalb je
  Stelle und Art den letzten.
- **Kennung:** `minecraft:banner` und `minecraft:decorated_pot`, auch ohne
  Namensraum oder mit leerem (`Identifier.bySeparator`), nur als Text
  (`getStringOr`).
- **Falscher Block:** Ein Blockentity, das nicht zu seinem Block passt,
  verwirft das Spiel beim Laden: Der Konstruktor prüft den Block
  (`validateBlockState`), und `BlockEntity.loadStatic` gibt dann keines
  zurück. Der Renderer nimmt Daten nur, wo das Bild des Blocks eine Stelle
  für sie hat (`blockentity::aendert`).

### Banner

Eine Lage ist `pattern` und `color`. `pattern` ist die ID eines Musters oder
ein Muster mit `asset_id` und `translation_key` (`BannerPattern.CODEC`),
`color` der Name eines Farbstoffs.

- Die Grundlage in der Farbe des Banners ist die letzte Zeichnung des
  Banners. Jede Lage kommt als weitere Zeichnung wie sie dazu, mit der
  Textur `<namensraum>:entity/banner/<pfad>` ihres `asset_id`
  (`Sheets.getBannerSprite`) und der Farbe ihres Farbstoffs, höchstens 16
  (`BannerRenderer.submitPatterns`). Die Regel prüft der Generator am
  Spiel mit 64 Lagen und schreibt die Höchstzahl in die Tabelle.
- **Muster:** Die Muster des Spiels schreibt der Generator mit ihrem
  `asset_id` in die Tabelle (`BannerPatterns.bootstrap`). Darüber liegen die
  der Datenwurzeln unter `<namensraum>/banner_pattern/**/*.json`, mit
  `asset_id` und `translation_key` wie `BannerPattern.DIRECT_CODEC`: Eine
  spätere Wurzel überschreibt gleichnamige Muster früherer und des Spiels,
  so wie Datenpakete über dem des Spiels liegen, siehe
  [Assets und Biomdaten](../benutzung/assets.md).
- **Unbekanntes:** Ein Muster, das weder das Spiel noch eine Datenwurzel
  kennt, und ein Farbstoff, den es nicht gibt, lehnt der Codec ab: Die Lage
  fehlt, im Spiel wie hier. Die Ausgabe nennt sie unter „Unbekanntes in
  Bannern“.

### Banner ohne Welt

Für die Ebenen zeichnet der Renderer ein Banner aus einem Entwurf als
Sprite, ohne Welt (`zeichne` in
[`renderer/src/render/banner.rs`](../../renderer/src/render/banner.rs)),
siehe [0100](../entscheidungen/0100-der-renderer-zeichnet-die-banner.md).

- **Wie:** dieselben Sprites wie im Baum, ein Zustand
  `<grundfarbe>_banner` mit seinen Lagen als Blockentity, siehe
  „Banner“ oben. `SpriteSet::familie_aus` rastert die Modelle mit Lagen und,
  für eine Hauptstadt, Krone; anders als `add_entities` darf die Familie
  über die Form des Blocks hinausragen. `render_familie` in
  [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
  legt alle Teile der Familie, auch die in den Würfeln darüber, nach Höhe
  und Tiefe auf eine Leinwand, die gerade alle fasst.
- **Licht:** volles Himmelslicht, ohne Nachbarn, Wasser und weiche
  Beleuchtung, im Look der Karte. Die Seiten schattiert das Spiel nach ihrer
  Richtung in der Welt; ein Tuch nach Westen ist darum dunkler als eins nach
  Süden, auch im Sprite.
- **Massstab:** Ein Pixel des Modells, ein Texel des Tuchs, ist ein Pixel
  des Sprites. Das Spiel zeichnet Banner um 2/3 verkleinert
  (`BannerRenderer.MODEL_SCALE`, per `javap` am Client von 26.2), ein Texel
  ist also 2/3 von 1/16 Block. Schräg ist ein Block in der Breite und in der
  Höhe je scale/2 Pixel, `north-45` je scale; das Banner zeichnet darum
  schräg bei scale 48, in `north-45` bei 24 (`massstab`). Von oben gibt es
  kein Sprite.
- **Drehung:** `4 · Vierteldrehungen der Richtung`, so zeigt das Tuch im
  Blick nach Süden, zur Kamera. Belegt per `javap` am Client von 26.2:
  - `BannerBlock.getStateForPlacement` setzt `rotation` auf
    `RotationSegment.convertToSegment(Blickwinkel + 180°)`: Das Banner
    schaut zum Spieler zurück.
  - `Direction.fromYRot` zählt die Blickwinkel in Vierteln ab Süden, über
    Westen, Norden und Osten (`BY_2D_DATA` nach `data2d`).
  - Drehung 0 zeigt also nach Süden, 4 nach Westen, 8 nach Norden, 12 nach
    Osten. `RotationSegment.convertToDirection` nennt die Gegenrichtung.
  - Dass das Sprite die Vorderseite zeigt und nie die gespiegelte Rückseite,
    prüft `banner_zeigt_die_vorderseite` in
    [`renderer/tests/banner.rs`](../../renderer/tests/banner.rs) mit den
    Assets des Spiels.
- **Fuss und Winkel:** der Fuss in der Mitte der Unterseite des Blocks; der
  Winkel der Unterkante des Tuchs aus der Strecke eines Blocks quer zum
  Blick, schräg `atan(H / W)`, genordet 0°.
- **Grösse,** gemessen am 10.10. mit `banner_massstab_winkel_und_grenze`,
  Breite × Höhe ohne und mit Krone:

  | Kamera | ohne Krone | mit Krone |
  |---|---|---|
  | `2:1` | 23 × 51 | 23 × 54 |
  | `4:3` | 23 × 57 | 23 × 58 |
  | `1:1` | 23 × 63 | 23 × 63 |
  | `north-45` | 20 × 46 | 20 × 55 |

  Alle bleiben unter der Grenze von 32 × 64 aus
  [Ebenen](../benutzung/ebenen.md), „Grenzen“. In `1:1` ragt das Ende des
  schrägen Querholzes höher als die Krone.
- **Gleich in jeder Richtung:** Weil sich die Drehung mit der Richtung
  dreht, gibt jede Richtung einer Kamera dieselbe Form mit demselben Fuss
  und Winkel; nur die Schattierung der Seiten folgt der Welt
  (`banner_in_jeder_richtung_gleich`).
- **Unbekanntes:** Ein Muster, das der Renderer nicht kennt, fehlt und steht
  in `unbekannt` (`banner_ohne_unbekanntes_muster`).
- **Goldbilder** unter `renderer/tests/fixtures/golden-banner`, getrennt von
  denen der Bäume: Ändert sich eines, steigt `BANNERSTAND`, nicht der
  Zeichenstand eines Looks (`bannerstand_folgt_den_goldbildern`).

#### Die Krone

Mit `krone` setzt `zeichne` eine Krone auf das Banner, für eine Hauptstadt,
siehe [0100](../entscheidungen/0100-der-renderer-zeichnet-die-banner.md),
„Die Krone“. Nichts davon stammt aus dem Spiel.

- **Modell:** `KRONE` in
  [`banner.rs`](../../renderer/src/render/banner.rs), in Pixeln des
  Modells: ein Reif von 8 × 3 × 8 aus vier Wänden, 1 dick, und je Seite
  mittig eine Zacke von 2 × 3 × 1, zusammen 6 hoch. Die Quader nimmt
  `box_quads`, so passen Umlaufsinn, Normale und Texturrichtung wie bei
  jedem gebackenen Modell.
- **Lage:** mittig auf dem Querholz, im Raum und in der Lage des
  Bannermodells, also um 2/3 verkleinert und mit dem Banner gedreht. Die
  Krone steht um 90° gedreht, Wahl des Users am ersten Goldbild: Zur Seite
  des Tuchs mit den Mustern zeigt ihre Seite mit dem Saphir, die mit dem
  Rubin nach links und rechts. `blockentity::erste_form`
  liefert die Lage, die Oberkante von Stange und Querholz, im Raum des
  Modells bei y = −44/16, und deren Schicht `entity_solid`
  (`krone_auf_dem_querholz`).
- **Textur:** [`krone.png`](../../renderer/src/render/krone.png), 16 × 16,
  eigene Pixelkunst, deckend, eingebunden als Textur `#krone` ohne Datei
  in den Packs (`Assets::eigene_textur`). Licht und Schatten sind gemalt;
  die Helligkeit der Seiten gibt der Renderer dazu wie dem Querholz.
- **Bereiche:** als Konstanten in `banner.rs`, die Aussenseiten vorn und
  hinten mit Rubin, links und rechts mit Saphir, innen, oben und die
  Zacken. Liegt ein Bereich quer zu seiner Seite, dreht er mit. Die Enden
  der Wände vorn und hinten nehmen die Spalte am Rand ihrer Aussenseite,
  so läuft sie um die Ecke.
- **Quelle:**
  [`docs/bilder/quellen/krone/krone.aseprite`](../bilder/quellen/krone/krone.aseprite)
  mit den Bereichen als Slices. `aseprite -b
  docs/bilder/quellen/krone/krone.aseprite --save-as
  renderer/src/render/krone.png` gibt `krone.png` Pixel für Pixel.
- **Nur oben:** Unter dem Querholz bleibt das Banner Pixel für Pixel
  gleich, um den Fuss ausgerichtet, Winkel und Fuss im Bild bleiben
  (`krone_aendert_nur_oben`).

### Krug

`sherds` nennt bis zu vier Items: hinten, links, rechts, vorne
(`PotDecorations.ordered`).

- **Bis 26.2** ist es eine Liste von Item-Namen, wie oben. Der Renderer
  liest eine Liste immer so: Was kein Text ist, fällt heraus, die übrigen
  rücken auf. 26.3 liest keine Liste; erst der Fix beim Laden
  (`PotDecorationsBlockEntityUnflatteningFix.unpackList`) setzt sie nach
  Platz. Ein Eintrag, der fehlt oder kein Text ist, wird dort
  `minecraft:brick` an seinem Platz, ein leerer Text bricht den Fix ab,
  und der Krug ist leer. Eine Liste aus dem Spiel hat immer vier gültige
  Namen, dort ist das Bild gleich.
- **Ab 26.3** ist es ein Objekt mit `back`, `left`, `right` und `front`
  (`PotDecorationsBlockEntityUnflatteningFix`, DataVersion 4996), je
  optional ein `ItemStackTemplate`: ein Item-Name oder ein Compound mit
  `id`, dazu `count` und `components`.
  - Eine Seite, die fehlt oder sich nicht lesen lässt, etwa eine Zahl oder
    ein Compound ohne `id`, ist leer; die übrigen bleiben
    (`OptionalFieldCodec` setzt das Teilergebnis, `TagValueInput.read`
    nimmt es).
  - Ein `count` ausserhalb von 1 bis 99 behält das Item, ebenso als
    Teilergebnis. `components` liest der Renderer nicht; die Scherbe kommt
    im Spiel vom Item.
  - Getestet: `krug_ab_26_3` in `chunk.rs`.

- Welche Zeichnung welchen Platz trägt, findet der Generator, indem er
  den Krug mit vier verschiedenen Scherben zeichnen lässt.
- Die Textur kommt aus `DecoratedPotRenderer.DECORATED_POT_SPRITES`, nach
  dem Item. Ziegel, ein Item ohne Scherbe und ein Platz ohne Eintrag geben
  die Seite ohne Scherbe (`getSideSprite`).
- Vorne liegt gegenüber `facing`, bei dem, der den Krug gesetzt hat:
  `facing` ist seine Blickrichtung (`DecoratedPotBlock.getStateForPlacement`).

### Im Renderpfad

- Der Vorlauf sammelt je Blockentity mit Daten die Blockstate an seiner
  Stelle mit den Daten (`Survey::entities`).
- `SpriteSet::add_entities` baut je Familie und Daten eine eigene Familie
  mit eigenen Sprites, auch mit den Fassungen fürs Wasser eines gefluteten
  Krugs.
- `ChunkCache` sucht beim Zeichnen an der Stelle des Blocks die Familie
  mit seinen Daten. Für die Nachbarn zählt der Block wie ohne Daten: Muster
  und Scherben liegen auf den Flächen des Modells und verdecken nichts
  anderes, das prüft ein `debug_assert`. Ebenso bleibt seine Form für die
  Sonne in Cinematic (`Sonnenform`: voller Würfel, leer, die Zellen, in die
  er ragt), nach der die Bits des schnellen Gangs gehen, siehe
  [Cinematic](cinematic.md), „Der schnelle Gang“.
- Was das Lesen kostet:
  [2026-09-28, Blockentities](../messungen/2026-09-28-blockentities.md).

## Was fehlt

Was ein Blockentity-Renderer zeichnet, aber nicht aus einem Modell, fehlt
auf der Karte:

- **Text auf Schildern:** Schrift aus den Daten des Schilds, gerendert mit
  dem Font des Clients. Die Schilder selbst haben in 26.2 Blockmodelle.
- **Gegenstände:** auf dem Lagerfeuer und im Regal, im Tresor, in
  verdächtigem Sand und Kies. Sie bräuchten die Item-Modelle.
- **Das Wesen im Spawner** und im Prüfungsspawner.
- **Strahl des Leuchtfeuers, End-Portal und End-Transitportal:** eigene
  Geometrie mit Zeit und Himmel, kein Modell. Das End-Portal bleibt leer.
- **Rahmen von Konstruktions- und Testblöcken:** nur für Spieler, die
  Konstruktionsblöcke bedienen dürfen, und für Zuschauer
  (`BlockEntityWithBoundingBoxRenderer.extract`).
- **Ein Kolben in Bewegung** zeichnet den Block, den er schiebt; im
  gespeicherten Chunk ist das ein Zwischenstand.

`--scan` und `--block` nennen, was weder ein Modell noch ein Bild aus
seinem Blockentity hat, siehe [Schalter](../benutzung/schalter.md).

## Was bleibt eine Näherung

- **Alles steht still, zur Zeit 0:** Deckel geschlossen, die Glocke ruht,
  das Buch auf dem Zaubertisch in seiner Ruhelage, jeder Banner im selben
  Schwung. Im Spiel wehen Banner nach Zeit und Position verschieden.
- **Jeder Aquisator als inaktive Schale, Drehung 0:** Käfig, Wind und Auge
  des aktiven kommen im Spiel auch aus Modellen (`ConduitRenderer.submit`),
  drehen sich aber mit der Zeit, und ob er aktiv ist, rechnet das Spiel
  laufend aus den Blöcken um ihn.
- **Truhen ohne Weihnachten:** Vom 24. bis 26. Dezember zeichnet das Spiel
  sie als Geschenke (`SpecialDates.isExtendedChristmas`), der Renderer nie.
- **Spielerköpfe wie ohne `profile`:** Ohne `profile` zeichnet das Spiel
  `entity/player/slim/steve` in `entity_cutout_z_offset`, wie die Tabelle.
  Mit `profile` nimmt es die Haut des Spielers, die der Client aus dem Netz
  lädt, und bis dahin eine von 18 Standardhäuten nach der UUID
  (`DefaultPlayerSkin.get`), beide in `entity_translucent`
  (`SkullBlockRenderer.resolveSkullRenderType`). Der Renderer liest
  `profile` nicht.
- **Licht der Oberwelt überall:** Im Nether kommt für Entity-Modelle das
  zweite Licht von unten (`NETHER_DIFFUSE_LIGHT_1`); der Renderer nimmt in
  jeder Dimension das der Oberwelt. Offen in
  [#45](https://github.com/VonNekyia/heroic-map-renderer/issues/45).
- **Ein Item, das es nicht gibt, in `sherds`** lässt das Spiel bis 26.2
  weg, und die übrigen rücken auf. Der Renderer kennt nur die Scherben und
  lässt es an seinem Platz, als Seite ohne Scherbe. Ab 26.3 bleibt es nach
  dem Fix beim Laden ebenso an seinem Platz. So schreibt nur ein Editor.
- **Mehrere Einträge mit `keepPacked` an einer Stelle:** Solche legt das
  Spiel ungeprüft beiseite (`ChunkAccess.setBlockEntityNbt`), an einer
  Stelle ohne geladenes Blockentity den letzten, gleich welcher Art. Der
  Renderer behält auch hier je Stelle und Art den letzten. So schreibt nur
  ein Editor.
