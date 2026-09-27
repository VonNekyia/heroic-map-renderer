---
title: Welten und Kennung
description: Welche Welten der Renderer liest, wie er Weltwurzel, Dimension und Seed findet und wie daraus die Kennung der Welt im Kachelbaum wird.
code:
  - renderer/src/world/mod.rs
  - renderer/src/world/region.rs
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
