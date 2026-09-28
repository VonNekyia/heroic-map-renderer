---
title: Rastern ohne Nähte
description: Wie der Rasterizer Sprites ohne Nähte und ohne Löcher rastert, Fragmente je Pixel mischt, die Textur über den Pixel mittelt und Flächen mit Löchern wie das Spiel ausschneidet.
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/assets/baker.rs
  - renderer/src/assets/texture.rs
---

# Rastern ohne Nähte

Sprite-Kanten werden nicht geglättet: Die Geometrie wird nur im
Pixelmittelpunkt geprüft, mit der Füllregel der Grafikkarten, damit an einer
gemeinsamen Kante jeder Pixel genau einer der beiden Flächen gehört.
Innerhalb eines Sprites legt jede Fläche je Pixel ein Fragment ab, das am
Schluss von hinten nach vorn gemischt wird. Die Textur dagegen wird über
den Pixel gemittelt, in linearem Licht. Alles in
[`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs).

## Geometrie im Pixelmittelpunkt

Sprite-Kanten werden nicht geglättet. Die Geometrie wird nur im
Pixelmittelpunkt geprüft, damit an einer gemeinsamen Kante jeder Pixel
genau einer der beiden Flächen gehört und Nachbarflächen nahtlos
aneinanderstossen. Geglättete Kanten trügen Teildeckung im Alpha, und beim
Zusammensetzen der Sprites könnte niemand mehr unterscheiden, ob zwei
Nachbarflächen dasselbe Pixel teilen oder ob eine durch die andere scheint:
ein Wasserbecken bekam an jeder Blockgrenze eine hellere Naht, ein Boden aus
deckenden Blöcken dunkle Linien, bis Schritt 8 hatte das Goldbild sie. Siehe
[0008](../entscheidungen/0008-sprite-kanten-nicht-glaetten.md).

## Füllregel

Liegt ein Mittelpunkt genau auf der Kante, entscheidet die Füllregel der
Grafikkarten: der Pixel gehört dem Dreieck, für das die Kante oben oder
links liegt. Dafür rechnen beide Dreiecke die gemeinsame Kante von derselben
Ecke aus. Von verschiedenen Ecken aus rundet f32 bei gedrehter Geometrie
verschieden, und ein Pixel genau auf der Kante fiel bei beiden durch, bei
scale 32 derselbe Pixel in jedem Kreuzmodell. Ohne die Regel nahmen beide
Dreiecke einer Fläche die Pixel auf ihrer Diagonale an, und bei scale 2
bekam Wasser dort Alpha 233 statt 180. Geprüft ist beides an 300 zufällig
gedrehten Quadern bei scale 4 bis 64, gedreht vom Baker selbst.

## Flächen parallel zur Blickrichtung

Dabei zeigte sich eine Fläche genau parallel zur Blickrichtung: Der Baker
dreht in f32, und die Summe ihrer Normalen liegt um 1e-7 ihrer Länge neben
null, mal davor, mal dahinter. Lag sie davor, legte die Fläche einen
Streifen von 2e-7 Pixeln Breite auf die Kante ihres Nachbarn, und ein
Pixelmittelpunkt genau darauf bekam beide. Solche Flächen zählen jetzt als
abgewandt (`EDGE_ON`). Echte Drehungen liegen weit darüber: um eine Achse
in Schritten von 22,5 Grad, dazu Vielfache von 90, ist die kleinste Summe
ungleich null 0,54 der Länge. In Vanilla und im Pack haben 82
Blockstates eine, etwa Kerzen, Hängeschilder und schräge Schienen. Ihre
Pixel bleiben auf allen fünf scales gleich, nur 370 von 1705 Sprites
bekommen einen kleineren Rahmen.

## Fragmente je Pixel

Durchsichtige Flächen mischen sich auch innerhalb eines Sprites: jede
Fläche legt je Pixel ein Fragment ab, und am Schluss wird je Pixel von
hinten nach vorne gemischt. Ein durchscheinendes Texel liegt so immer über
dem, was dahinter liegt, Wasser über einem Zaunpfosten, Buntglas über dem
Block dahinter, gleich in welcher Reihenfolge die Flächen kommen, und eine
durchscheinende Fläche, die den Pixel nur zum Teil deckt, verdeckt die
Fläche dahinter nicht. Nur ein deckendes Fragment verwirft, was dahinter
liegt. Bei gleicher Tiefe gewinnt die spätere
Fläche; die Flächen sind deshalb stabil nach ihrer vordersten Ecke
sortiert, und der Grasblock legt sein Overlay auf den Grundwürfel. Vorher
gewann ein Tiefenpuffer, und ein gefluteter Zaun war ein Wasserwürfel ohne
Zaun.

## Textur über den Pixel gemittelt

Die Textur wird über den Pixel gemittelt, sonst fiele auf einer acht Pixel
breiten Seitenfläche jeder zweite Texel weg. Gemittelt wird wie in der
Pyramide in linearem Licht und mit vormultipliziertem Alpha, siehe
[Zoomstufen](../benutzung/zoomstufen.md), „Verkleinern“. Die Abtastpunkte
bleiben dabei im Texturausschnitt der Fläche, und ihre Zahl folgt dem
scale, damit auch bei wenigen Pixeln je Block alle Texel zählen. Treffen
alle Abtastpunkte dasselbe Texel, bei scale 32 je nach Textur bei einem
Zehntel bis gut einem Drittel der Pixel, ist das Texel selbst das Mittel,
ohne Umweg über lineares Licht. Für Flächen mit Löchern folgt danach der
Alpha-Test des Spiels.

## Ausgeschnitten statt gemischt

Das Spiel legt jede Fläche nach dem Ausschnitt ihrer Textur in eine
Schicht, belegt per javap am Client 26.2 (`FaceBakery
.computeMaterialTransparency`, `SpriteContents.computeTransparency`,
`ChunkSectionLayer.byTransparency`):

- **TRANSLUCENT** mit halb durchsichtigen Texeln oder mit
  `force_translucent`: Dort wird gemischt.
- **CUTOUT** mit Löchern, also Texeln mit Alpha 0, sonst aber deckend.
- **SOLID** sonst.

Der Ausschnitt reicht von floor bis ceil der UV-Grenzen der Fläche, bei
einer Animation über jedes Bild, das sie zeigt; der volle Ausschnitt
zählt nach der ganzen Datei. So prüft auch der Renderer, je Fläche
(`Textures::durchscheinend` in
[`renderer/src/assets/texture.rs`](../../renderer/src/assets/texture.rs),
`Deckung` in
[`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs)).
Was durchscheint, mischt er, alles andere schneidet er aus; eine Fläche
ohne Löcher deckt dabei so oder so ganz. Flüssigkeiten baut das Spiel
ohne `FaceBakery`; sie mischen weiter.

In CUTOUT verwirft `terrain.fsh` jedes Fragment mit Alpha unter 0,5
(`ALPHA_CUTOUT` in `pipeline/cutout_terrain`), gemischt wird nicht: Ein
Laubpixel ist Laub oder Loch, nie halb. Der Renderer testet jeden
Abtastpunkt wie der Shader ein Fragment, mittelt dann wie oben und prüft:

- Deckt weniger als die Hälfte, bleibt der Pixel leer.
- Deckt mehr als die Hälfte, deckt er ganz, in der Farbe der deckenden.
- Deckt genau die Hälfte, entscheidet das Texel in der Pixelmitte; dort
  tastet auch das Spiel ab.

In TRANSLUCENT verwirft `translucent_terrain` Fragmente unter Alpha 0,1;
der Renderer verwirft solche Abtastpunkte ebenso und mischt den Rest. Unter
den Blocktexturen von Vanilla haben nur die Abbaustufen solche Werte, und
die trägt kein Modell. Die Schichten der Blockentities bringen ihren
eigenen Test mit, siehe [Blockentities](blockentities.md), „Schichten“.

Laub und Mangrovenwurzeln tragen `"mipmap_strategy": "dark_cutout"`.
Das Spiel schreibt dann in jedes Loch drei Viertel des dunkelsten
deckenden Texels (`TextureUtil.fillEmptyAreasWithDarkColor`), und wo die
Grafikkarte zwischen Texeln oder aus kleineren Mip-Stufen liest, mischt
sich diese Farbe an den Rändern der Nadeln ein. Der Renderer zählt dafür
die Löcher unter einem Pixel in dieser Farbe mit: Es decken dieselben
Pixel, ihre Ränder werden dunkler.

Unter der festen Blickachse liegen die Löcher aller Laubblöcke einer
Krone genau übereinander. Durch einige Prozent der Pixel einer Krone
sieht man deshalb den Boden, mehr als mit der Kamera des Spiels, siehe
[0038](../entscheidungen/0038-cutout-wie-im-spiel.md).

## Was bleibt eine Näherung

- **Randpixel an echten Öffnungen bleiben leer.** Liegt eine Pixelmitte
  genau auf einer Aussenkante neben einer Öffnung im Modell, gehört sie
  nach der Füllregel der Öffnung, wie auf einer Grafikkarte. Das betrifft
  bei scale 8 und 16 einzelne Pixel in Zauntoren.
- **Kanten gegen Luft sind Treppen** im 2:1-Raster statt Verläufe, die
  Silhouette, die isometrische Pixelkunst ohnehin hat.
- **Keine Mip-Stufen.** Das Spiel baut für Flächen mit Löchern kleinere
  Stufen, die ihre Deckung halten (`MipmapGenerator.scaleAlphaToCoverage`).
  Der Renderer mittelt stattdessen je Pixel über alle Texel darunter und
  testet danach. Auf den nativen Stufen wird Laub so dichter, gemessen in
  [2026-09-28, Cutout](../messungen/2026-09-28-cutout.md).
- **`dark_cutout` in jedem Pixel.** Das Spiel mischt die Füllfarbe nur,
  wo die Grafikkarte filtert; der Renderer in jedem Pixel, dessen
  Abtastpunkte ein Loch treffen.
- **Farbe der Löcher sonst.** Bei den anderen Strategien färbt das Spiel
  die Löcher mit den Nachbarn (`TextureUtil.solidify`), bei `mean` gar
  nicht; der Renderer nimmt immer die Farbe der deckenden Abtastpunkte.
