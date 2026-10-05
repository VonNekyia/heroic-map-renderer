---
title: Welten und Kennung
description: Welche Welten der Renderer liest und welche Chunks darin, wie er Weltwurzel, Dimension, Seed, Datenversion und Wasserspiegel findet, wie daraus die Kennung der Welt im Kachelbaum wird und wie er liest, während der Server schreibt.
code:
  - renderer/src/assets/dimension.rs
  - renderer/src/world/mod.rs
  - renderer/src/world/region.rs
  - renderer/src/world/chunk.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
---

# Welten und Kennung

Der Renderer liest Welten ab Minecraft 26.1, mit den Regionen unter
`world/dimensions/<namensraum>/<name>/region`. Jeder Kachelbaum gehört zu
genau einer Welt und Dimension: `map.json` trägt dafür eine Kennung aus
einem Salz und einem verketteten Hash des Seeds, nie den Seed selbst. Die
Kennung rechnet `world_id` in
[`renderer/src/render/pyramid.rs`](../../renderer/src/render/pyramid.rs),
den Seed sucht `renderer/src/world/mod.rs`.

## Welche Welten

Eine ältere Welt, etwa aus 1.21 mit `world/region` und `DIM-1`, vorher mit
dem Server von Minecraft 26.2 oder 26.3 und `--forceUpgrade` hochziehen: er baut
Verzeichnisse, Seed und Chunks um, bevor er startet. Sonst kennt der
Renderer ihren Seed nicht und manche ihrer Blocknamen nicht, und ein Block
ohne Asset bricht den Lauf vor der ersten Kachel ab. Warum die Grenze bei
26.1 liegt: [0015](../entscheidungen/0015-nur-welten-ab-26-1.md).

Welten aus 26.3 liest er ebenso, auch solche, die der Server erst zum Teil
neu gespeichert hat ([0059](../entscheidungen/0059-welten-aus-26-2-und-26-3.md)):
- **Palette:** Ab 26.3 heissen ihre Felder `id` und `properties` statt
  `Name` und `Properties` (`BlockStateFieldNamesFix`, DataVersion 5006).
  Der Renderer liest beide (`PaletteEntry` in
  [`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs),
  getestet in `palette_ab_26_3`).
- **Scherben eines Krugs:** Sie stehen ab 26.3 als Objekt, siehe
  [Blockentities](../renderer/blockentities.md), „Krug“.

## Nicht fertig erzeugte Chunks

Am Rand jedes erzeugten Gebiets liegen Chunks, die das Spiel angefangen,
aber nicht fertig erzeugt hat, von innen nach aussen etwa mit dem Status
`minecraft:initialize_light`, `carvers` (ab 26.3 `terrain`), `biomes` und
`structure_starts`.
Ihnen fehlen Bäume, Seen und Schnee ganz oder zum Teil, die äusseren sind
noch ganz Luft. Das Spiel zeigt sie nie: `ChunkHolder.getChunkToSend` gibt
dem Client nur fertige Chunks heraus (Client 26.2, per javap).

Der Renderer liest deshalb nur Chunks ab dem Status `minecraft:light`, also
`light`, `spawn` und `full`, und behandelt die übrigen wie fehlende
(`Chunk::is_generated` in
[`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs)). Ab
`light` setzt die Erzeugung keinen Block mehr, belegt per javap am Client
26.2 (`ChunkPyramid.GENERATION_PYRAMID`). In 26.3 ist es gleich, nur
heissen `noise`, `surface` und `carvers` dort zusammen `terrain` (#98):

- Der Schritt `light` verlangt die Nachbarn im Radius 1 mindestens in
  `initialize_light`, also hinter `features`.
- Nur `features` schreibt über den eigenen Chunk hinaus, einen Chunk weit
  (`blockStateWriteRadius(1)`). Die Schritte danach setzen keinen Radius,
  und der Standard −1 erlaubt keinen Block.

Warum nicht erst ab `full` wie der Client:
[0037](../entscheidungen/0037-chunks-ab-dem-status-light.md).

So sehen Vorlauf, Render und Höhen dieselbe Welt:

- **Kacheln:** Ein solcher Chunk bringt keine Kachel und keinen Blockstate.
  Der Vorlauf zählt ihn und nennt die Zahl, siehe die Ausgabe in
  [Kacheln exportieren](kacheln.md). Wie viele Kacheln so wegfallen,
  steht in [Was ein Lauf kostet](kosten.md), „Je scale“.
- **Licht:** Er lässt kein Licht herein, wie ein Chunk, der fehlt, siehe
  [Wasser und Licht](../renderer/wasser-und-licht.md), „Licht
  ausbreiten“. Sonst läge die äusserste Blockreihe davor im Licht 14,
  auch tief unter Wasser.
- **Deckung:** Er deckt nichts. Am Ost- und Südrand bleibt deshalb ein
  Schnitt durch den Untergrund stehen, wie an jedem Rand der Welt; er
  liegt ohne Licht im Dunkeln.
- **Biome:** Am neuen Rand mischt die Farbe mit plains wie neben jedem
  fehlenden Chunk, siehe [Biomfarben](../renderer/biomfarben.md),
  „Übergänge zwischen Biomen“.
- **Höhen:** Seine Zellen sind leer, siehe [map.json](map-json.md),
  „Höhen“.
- **`--at` und `--scan`:** `--at` nennt seinen Status und dass der Renderer
  ihn nicht zeichnet. `--scan` dekodiert und zählt ihn, löst seine
  Blockstates aber nicht auf, siehe [Schalter und Beispiele](schalter.md),
  „Die ganze Welt prüfen: `--scan`“.

Ein Baum aus einem früheren Stand zeigt die Fehler am Rand noch, denn
`--resume` behält seine Basiskacheln. Neu wird der Rand erst mit einem
Export ohne `--resume`; mit `--prune` entfernt er dabei auch die Kacheln,
die jetzt kein Chunk mehr berührt, siehe [Kacheln exportieren](kacheln.md),
„Kacheln ohne Chunk: `--prune`“.

## Weltwurzel und Dimension

`--world` zeigt auf die Weltwurzel, das Verzeichnis mit `level.dat`, oder
auf eine Dimension darin, `dimensions/<namensraum>/<name>`. Die Wurzel ist
die Oberwelt, auch über `dimensions/minecraft/overworld`; eine Kopie von
`level.dat` in einer Dimension macht diese nicht zur Oberwelt. Der Pfad
zählt so, wie er auf der Platte steht: unter Windows gibt
`DIMENSIONS\MINECRAFT\THE_NETHER` dieselbe Kennung wie
`dimensions\minecraft\the_nether`, und ein Weg über `..` dieselbe wie der
direkte. Führt er auf der Platte über einen Link aus der Welt hinaus, etwa
zu einer Dimension auf einer anderen Platte, oder lässt er sich dort nicht
auflösen, zählt er so, wie er angegeben ist, auch in seiner Schreibweise:
`dimensions\Minecraft\the_nether` gibt dann die Kennung von
`Minecraft:the_nether`.

## Datenversion

Aus `level.dat` der Weltwurzel liest der Renderer nur `Data.DataVersion`,
die Version des Spiels, das die Welt zuletzt gestartet hat, für 26.2 4903,
für 26.3 5023. Danach wählt er die Sicht in der Ecke der weichen
Beleuchtung, siehe [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md),
„Welten aus 26.2“. Der Lauf nennt sie in der Zeile `Version:`. Ohne
Weltwurzel gibt es keine, dann gilt 26.3. Eine `level.dat`, die sich nicht
lesen lässt, bricht den Lauf ab.

## Wo der Seed steht

Den Seed liest der Renderer zuerst aus der Dimension selbst, aus
`data/minecraft/world_gen_settings.dat` darin: so schreibt Paper ihn je
Dimension, und eine Plugin-Welt hat oft einen eigenen. Sonst aus derselben
Datei an der Weltwurzel, wie Vanilla seit 26.1, oder aus der der
Paper-Oberwelt, von beiden aus der jüngeren, bei gleichem Alter aus der von
Paper: unter Paper bleibt an der Wurzel eine ältere liegen, etwa aus der
Zeit vor einer neu erzeugten Welt. Frühere Stände
des Renderers lasen zuerst die Datei an der Wurzel. Hat eine Dimension eine
eigene mit anderem Seed, passt ihr Baum aus einem solchen Stand nicht mehr
zu ihr, und der Lauf lehnt ihn ab, er gehöre zu einer anderen Welt oder
Dimension; einen solchen Baum neu rendern.

Mit dem Seed würfelt der Renderer auch, welches Biom jeder Block trägt, wie
das Spiel; ohne ihn gilt je Block das Biom seiner Zelle aus 4×4×4 Blöcken,
siehe [Biomfarben](../renderer/biomfarben.md), „Biom je Block“. Aus dem
Speicher des Laufs kommt er dabei nicht heraus.

## Wasserspiegel

Den Wasserspiegel für `seaLevel` in `map.json` liest der Renderer aus
derselben Datei wie den Seed, `world_gen_settings.dat`, aus dem Generator
der Dimension, `data.dimensions.<dimension>.generator`, wie
`ChunkGenerator.getSeaLevel` in 26.2 und 26.3, per javap
(`wasserspiegel` in
[`renderer/src/assets/dimension.rs`](../../renderer/src/assets/dimension.rs)):

| Generator | Wasserspiegel |
|---|---|
| `minecraft:noise` mit einer ID unter `settings` | `sea_level` dieser Noise Settings des Spiels, aus `dimensionstypen.txt`: `overworld`, `large_biomes`, `amplified` 63, `nether` und `caves` 32, `end` 0, `floating_islands` −64 |
| `minecraft:noise` mit Noise Settings in der Datei | ihr `sea_level` |
| `minecraft:flat` | −63 (`FlatLevelSource`) |
| `minecraft:debug` | 63 (`DebugLevelSource`) |
| ein anderer, Noise Settings aus einem Datenpaket, keine Datei | keiner, `null` |

Noise Settings aus einem Datenpaket der Welt liest der Renderer nicht.

## Die Kennung

`world` in `map.json` ist die Kennung der Welt: vorn ein Salz, das der Baum
bei seinem ersten Lauf zufällig bekommt, dahinter ein Hash ihres Seeds und
ihrer Dimension, SipHash-2-4 mit diesem Salz, eine Million Mal verkettet
(2^20). Die Dimension gehört dazu, weil die Dimensionen einer Welt meist
denselben Seed tragen: sonst käme der Nether in den Baum der Oberwelt und
die Oberwelt in seinen.

Der Seed selbst steht nicht in der Datei: `map.json` liegt öffentlich neben
den Kacheln, und mit dem Seed fände jeder Strukturen und Erze ohne zu
suchen. Warum verkettet:
[0014](../entscheidungen/0014-kennung-der-welt-ohne-seed.md). Der Export
zahlt dafür 16 ms je Lauf.

Ohne `level.dat` darüber ist die Welt nicht zu erkennen, etwa bei einer
Kopie ohne sie, und ohne Seed auch nicht, etwa bei einer Kopie ohne `data`.
Die Ausgabe sagt dann, was fehlt, und nennt jeden Ort, an dem der Seed
gesucht wurde. Dann steht `"world": null` da, und ein solcher Baum nimmt
keine Welt mit Kennung auf; zwei Welten ohne Kennung kann der Renderer
nicht auseinanderhalten.

Was mit einem Baum geschieht, dessen Kennung nicht passt, steht in
[Zoomstufen](zoomstufen.md), „Ein Baum, eine Welt“.

## Während der Server schreibt

Der Renderer darf eine Welt lesen, während der Server sie schreibt. Er
öffnet jede Datei nur lesend; unter Windows gibt `std::fs::File::open`
Schreiben, Löschen und Umbenennen frei, der Server scheitert also an
nichts. Was er dabei halb geschrieben sehen kann, liest er neu (#143,
`Region::stored_chunk` in
[`renderer/src/world/region.rs`](../../renderer/src/world/region.rs),
`read_nbt` in [`renderer/src/world/mod.rs`](../../renderer/src/world/mod.rs)):

| Fall | Woran der Renderer es merkt | Was er tut |
|---|---|---|
| Der Server hat die Regionsdatei verlängert, seit der Renderer sie geöffnet hat | Ein Eintrag zeigt hinter die gemerkte Länge | liest die Länge neu und, weil sie sich geändert hat, ohne Pause den Chunk |
| Ein ausgelagerter Chunk: der Kopf ist neu, die `.mcc` noch nicht verschoben | Die `.mcc` fehlt | liest nach der Pause neu |
| Der Server hat den Chunk zwischen Eintrag und Daten verlegt und die Sektoren neu vergeben | Fehler beim Lesen oder Entpacken, Fehler beim Dekodieren oder eine andere Position im NBT | liest Eintrag und Daten neu; nach einem Fehler vor dem Dekodieren nach der Pause, sonst ohne |
| `world_gen_settings.dat`, `world_border.dat` oder `level.dat` halb geschrieben oder eben ersetzt | Fehler beim Lesen | liest nach der Pause neu |

- **Höchstens dreimal** (`VERSUCHE`). 100 ms Pause (`PAUSE`) nur nach
  einem Fehler vor dem Dekodieren, also beim Lesen, Entpacken oder bei
  fehlender `.mcc`, und nur, wenn die Länge der Datei gleich geblieben ist.
  Bleibt der Fehler, bricht der Lauf ab wie zuvor.
- **Ein Fehler beim Dekodieren** nach sauberem Entpacken kommt aus einem
  Schreiben nur, wenn der Server die Sektoren eben neu vergeben hat; dann
  liefert ein zweites Lesen andere Bytes. Der Renderer liest deshalb einmal
  ohne Pause neu; sind die Bytes gleich, gilt der Fehler sofort. Kann der Decoder ein neues
  Format nicht lesen, kostet das so nur einen zweiten Lesevorgang.
- **`--scan`** zählt kaputte Chunks wie zuvor. Ein Fehler vor dem
  Dekodieren kostet dort die zwei Pausen, 0,2 s, einer beim Dekodieren
  nichts davon.
- **Eine andere Position, zweimal mit demselben Eintrag,** gilt: Dann ist
  der Chunk nicht eben verlegt, sondern steht so in der Datei, etwa in einer
  von Hand kopierten. Er liegt dann an seinem Platz, wie im Spiel. So ein
  Chunk kostet einen zweiten Lesevorgang, aber keine Pause.
- **Belegt** per javap am Client 26.2:
  - `RegionFile.write` gibt die alten Sektoren eines Chunks erst frei
    (`RegionBitmap.free`), nachdem der Kopf auf die neuen zeigt und die
    `.mcc` verschoben ist. Der nächste Chunk, den der Server schreibt, kann
    sie belegen. Die übrige Reihenfolge steht in [Updates](updates.md),
    „Was als geändert gilt“.
  - `SavedDataStorage.tryWrite` schreibt `world_gen_settings.dat` und
    `world_border.dat` mit `NbtIo.writeCompressed` an Ort und Stelle:
    `Files.newOutputStream` mit `SYNC`, `WRITE`, `CREATE` und
    `TRUNCATE_EXISTING`.
  - `level.dat` schreibt `LevelStorageAccess.saveLevelData` in eine
    temporäre Datei und ersetzt sie dann mit `Util.safeReplaceFile`, in
    `safeReplaceOrMoveFile` vier Schritte ohne Pause dazwischen:
    `level.dat_old` löschen, `level.dat` nach `level.dat_old` umbenennen,
    `level.dat` löschen, die neue Datei nach `level.dat` umbenennen.
    `runWithRetries` wiederholt einen Schritt nur, wenn er scheitert, und
    wartet dabei nicht. Das Fenster ist also kurz, 100 ms reichen.
- **Zwischen den zwei Umbenennungen** fehlt `level.dat`, und
  `level.dat_old` liegt schon da. Findet die Suche nach der Weltwurzel nur
  `level.dat_old`, sieht sie nach der Pause noch einmal nach `level.dat`
  (`ist_wurzel`). Sonst fände ein Lauf, der genau dann startet, die Wurzel
  nicht, und ein neuer Baum bekäme keine Kennung, siehe „Die Kennung“. Ein
  Verzeichnis mit `level.dat_old` allein bleibt keine Wurzel, wie zuvor.
- **Vorlauf und Render** lesen jeden Chunk für sich, bei einem Vollrender
  bis Stunden auseinander. Schreibt der Server einen Chunk dazwischen,
  zeichnet der Render den neueren. Was danach noch veraltet ist, holt das
  nächste Update nach: Der Stempel des Chunks hat sich geändert, siehe
  [Updates](updates.md), „Was als geändert gilt“.
- Getestet in `region.rs`: `laenge_nach_dem_anhaengen_neu_gelesen`,
  `ausgelagerter_chunk_nach_dem_verschieben`,
  `wiederverwendete_sektoren_neu_gelesen`, `verlegter_chunk_bleibt_verlegt`,
  `bleibender_fehler_nach_dem_letzten_versuch` und
  `unlesbarer_chunk_ohne_pause`; in `mod.rs`
  `halbe_datei_der_welt_neu_gelesen` und
  `wurzel_waehrend_level_dat_ersetzt_wird`. Die Tests ändern die Datei
  zwischen zwei Versuchen, wie der Server es täte. Die echten Pausen messen
  `pausen_im_echten_lesen` und `pausen_beim_lesen_der_welt`.

## Was bleibt eine Näherung

- **Ein Seed aus einem Text ist nur für Stunden bis Tage geschützt.** Ein
  Zufallsseed hat nur 2^48 Werte, Vanilla zieht ihn mit 48 Bit Zustand;
  verkettet sind es 2^68 Aufrufe je Baum, auf einer Grafikkarte
  Jahrzehnte. Ein Seed aus einem Text hat nur 2^32 Werte, 2^52 Aufrufe:
  den schützt die Kennung für Stunden bis Tage, nicht für immer. Das gilt
  nur, wenn man alle durchprobieren muss. Jeder geratene Seed kostet einen
  Versuch von 16 ms, und ein eingetippter wie 12345 oder einer aus einer
  öffentlichen Liste steht in jedem Wörterbuch: den findet man in
  Sekunden. Dagegen hülfe nur ein Geheimnis, das der Renderer nicht hat.
