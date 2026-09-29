---
title: Welten und Kennung
description: Welche Welten der Renderer liest und welche Chunks darin, wie er Weltwurzel, Dimension und Seed findet und wie daraus die Kennung der Welt im Kachelbaum wird.
code:
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
dem Server von Minecraft 26.2 und `--forceUpgrade` hochziehen: er baut
Verzeichnisse, Seed und Chunks um, bevor er startet. Sonst kennt der
Renderer ihren Seed nicht und manche ihrer Blocknamen nicht, und ein Block
ohne Asset bricht den Lauf vor der ersten Kachel ab. Warum die Grenze bei
26.1 liegt: [0015](../entscheidungen/0015-nur-welten-ab-26-1.md).

## Nicht fertig erzeugte Chunks

Am Rand jedes erzeugten Gebiets liegen Chunks, die das Spiel angefangen,
aber nicht fertig erzeugt hat, von innen nach aussen etwa mit dem Status
`minecraft:initialize_light`, `carvers`, `biomes` und `structure_starts`.
Ihnen fehlen Bäume, Seen und Schnee ganz oder zum Teil, die äusseren sind
noch ganz Luft. Das Spiel zeigt sie nie: `ChunkHolder.getChunkToSend` gibt
dem Client nur fertige Chunks heraus (Client 26.2, per javap).

Der Renderer liest deshalb nur Chunks ab dem Status `minecraft:light`, also
`light`, `spawn` und `full`, und behandelt die übrigen wie fehlende
(`Chunk::is_generated` in
[`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs)). Ab
`light` setzt die Erzeugung keinen Block mehr, belegt per javap am Client
26.2 (`ChunkPyramid.GENERATION_PYRAMID`):

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
