---
title: Wasser und Licht
description: Wie der Renderer Wasser und Lava baut, welche Flächen er weglässt und wie er Tiefe über das Himmelslicht und das Blocklicht des Spiels zeigt.
code:
  - renderer/src/assets/fluid.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/leuchten.txt
  - renderer/src/assets/Leuchten.java
  - renderer/src/assets/licht.txt
  - renderer/src/assets/Licht.java
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.wgsl
---

# Wasser und Licht

Flüssigkeiten haben kein Blockmodell; der Renderer baut sie wie das Spiel im
Code (`renderer/src/assets/fluid.rs`), lässt Flächen zu gleichem Wasser weg
und zeichnet an Stufen einen Streifen. Tiefe wirkt wie im Spiel nur über das
Licht: Man sieht durch genau eine Oberfläche, und darunter liegt jeder Block
in seinem Himmelslicht, das je Block Wasser eine Stufe verliert, und in
seinem Blocklicht, wenn er selbst leuchtet. Welches Licht ein Block bekommt,
bestimmt `light_at` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs),
die Helligkeit `brightness` in `renderer/src/render/rasterizer.rs`. Belegt
gegen 26.2.

## Flüssigkeiten als Würfel

`water`, `lava`, `bubble_column`, Seegras und Kelp sowie jede Blockstate mit
`waterlogged=true` bekommen einen Würfel mit der Flüssigkeitstextur,
fliessendes Wasser (`level` 1 bis 7) einen flacheren. Wie im Spiel endet
eine Quelle bei 8/9 der Blockhöhe, knapp zwei Texturpixel unter der Kante
(`FlowingFluid.getHeight`), und die Oberseiten gefluteter oberer Platten,
Treppen und Zaunpfosten bleiben trocken. Liegt dieselbe Flüssigkeit
darüber, reicht der Würfel bis oben, sonst hätte jede Schicht eines Ozeans
eine Fuge. `water.json` und `lava.json` nennen nur eine Partikeltextur; ohne
den Nachbau von `FluidRenderer` blieben Ozeane nackter Meeresboden, im
Frontend war das der auffälligste Fehlbestand.

Auch die Seiten gefluteter Blöcke bleiben trocken: Minecraft rückt jede
Flüssigkeitsfläche ein Tausendstel ins Blockinnere, der Renderer legt sie
dafür in der Tiefe knapp hinter die Blockfläche an derselben Stelle. Bei
gleicher Tiefe gewann das Wasser: ein Film auf jeder gefluteten Platte,
Treppe und Falltür.

![Übersicht](../bilder/map-wide.png)

Um den Ursprung, `--center 0 0 --size 900 --scale 4`, sonst wie das Bild in
[Schalter und Beispiele](../benutzung/schalter.md), „Einen Ausschnitt
rendern“. Stand `fccdef5`, mit dem Licht des Spiels und den Übergängen
zwischen Biomen.

## Flächen zu gleichem Wasser

Die Wassertextur ist grau und durchscheinend. Ihre Farbe kommt aus
`water_color` des Bioms, siehe [Biomfarben](biomfarben.md). Flächen zwischen
zwei Wasserblöcken werden nicht gezeichnet, wie im Spiel: ein Sprite kennt
seine Nachbarn zwar nicht, aber der Renderer, und er wählt je Block die
Fassung ohne die Flächen zu gleichem Wasser daneben und darüber. Sonst läge
in jedem Becken Wasser über Wasser, die Deckkraft stiege an jeder
Blockgrenze, und der Grund schimmerte durch ein Raster. Ein Wasserblock
mitten im Ozean hat danach keine Fläche mehr und kostet nichts. Siehe
[0009](../entscheidungen/0009-wasserflaechen-je-block.md).

## Streifen an Stufen

Steht das Wasser daneben tiefer, am Fuss eines Wasserfalls oder an jeder
Stufe fliessenden Wassers, fehlte über dessen Oberfläche ein Streifen der
eigenen Seite. Minecraft hebt dort die Ecken der Oberfläche an; der
Renderer zeichnet stattdessen genau diesen Streifen, als eigenes Sprite je
Paar aus eigener Höhe und Nachbarhöhe in Neunteln.

## Tiefe über das Licht

Man sieht durch genau eine Oberfläche, die Wassertextur hat überall
Alpha 180, und darunter zeichnet der Renderer jeden Block in seinem
Himmelslicht. Jeder Block Wasser nimmt eine Stufe, denn
`LiquidBlock.propagatesSkylightDown` ist falsch und `getLightDampening`
gibt damit 1: Der oberste Block Wasser liegt im Licht 14, der Grund n
Blöcke tief im Licht 15 − n, ab 15 Blöcken im Licht 0. Über dem Grund D
ergibt die Oberfläche W damit α · W + (1 − α) · b · D, b die Helligkeit im
Licht dort unten:

| Tiefe des Grunds | 1 | 2 | 3 | 5 | 10 | ab 15 |
|---|---|---|---|---|---|---|
| Licht | 14 | 13 | 12 | 10 | 5 | 0 |
| vom Grund sichtbar | 27 % | 24 % | 22 % | 18 % | 9 % | 3 % |
| vorher, eine Schicht je Block | 29 % | 8,6 % | 2,5 % | 0,7 % | 0,7 % | 0,7 % |

Vorher trug die Oberfläche die Deckkraft aller Schichten dahinter, so als
läge je Block Wasser eine weitere Oberfläche darüber. Was einen Block tief
lag, stand hell und blass neben fast deckendem Wasser, Pfosten und Wracks in
eckigen Flecken. Siehe [0030](../entscheidungen/0030-licht-je-block.md), die
abgelöste [0010](../entscheidungen/0010-tiefe-entlang-des-blickstrahls.md)
und die Messung
[2026-09-27, Wasser im Licht](../messungen/2026-09-27-wasser-im-licht.md).

## Helligkeit wie im Spiel

Die Helligkeit b rechnet `brightness` in `renderer/src/render/rasterizer.rs`
wie `shaders/core/lightmap.fsh` in 26.2: `get_brightness(l) = l / (4 − 3·l)`
für die Stufe l / 15, mal `SkyFactor` und `SkyLightColor`, dazu die
Umgebungsfarbe, auf 1 begrenzt; das Ergebnis liegt zwischen diesem Wert und
`notGamma(c) = 1 − (1 − c)⁴`, gewichtet mit `BrightnessFactor`. Die Werte
des Spiels am Tag in der Oberwelt: die Umgebungsfarbe `#0a0a0a`
(`visual/ambient_light_color` in `dimension_type/overworld.json`),
`SkyLightColor` weiss und `SkyFactor` 1 (`timeline/day.json`), und
`BrightnessFactor` 0,5, denn das ist `options.gamma` in der Voreinstellung
(`LightmapRenderStateExtractor`, `Options`). Licht 15 gibt 1, so hell
zeichnet der Renderer jede Fläche. Nebel gibt es nicht; den zeichnet das
Spiel nur, wenn die Kamera selbst unter Wasser ist.

## Blocklicht

Mit Blocklicht rechnet `brightness_rgb` weiter wie der Shader. Die Stufe
geht mit `BlockFactor` 1,4 in `get_brightness`; das Flackern, das das Spiel
um 0 laufen lässt, fehlt. Ihre Farbe `BlockLightColor`
liegt zwischen `BlockLightTint` und Weiss, gemischt mit 0,9 · (2l − 1)², und
`BlockLightTint` ist `#FFD88C`, der Standard aus `EnvironmentAttributes`,
den die Oberwelt nicht ändert. Die Summe mit dem Himmelslicht wird auf 1
begrenzt, und `notGamma` hebt alle Kanäle mit dem hellsten. Schwaches
Blocklicht färbt so warm, bei Stufe 15 ist alles hell.

Was selbst leuchtet, bringt sein Blocklicht mit, wie in
`LightCoordsUtil.getLightCoords`: Seelaterne, Glowstone und Konduit 15, eine
geflutete Meeresgurke 6 bis 15, je nach Anzahl. Mit `emissiveRendering`,
beim Magmablock etwa, ist der Block voll hell. Wie hell ein Block leuchtet,
steht in `renderer/src/assets/leuchten.txt`, siehe
[Erzeugte Tabellen](../entwicklung/tabellen.md).

## Licht ausbreiten

Wie hell jede Zelle ist, rechnet der Renderer selbst aus, wie
`SkyLightEngine` und `BlockLightEngine` in 26.2, in
`renderer/src/render/licht.rs`. Das gespeicherte Licht der Welt liest er
nicht, es fehlt in vielen Chunks, siehe
[0040](../entscheidungen/0040-licht-selbst-ausbreiten.md). Die Regeln,
belegt per javap:

- **Schritte.** Jeder Schritt zur Nachbarzelle kostet eine Stufe
  (`LightEngine.propagateIncrease`: `max(1, getLightDampening)`). In einen
  Block, der um 15 dämpft, Stein etwa, kommt kein Licht; Luft, Glas,
  Wasser und Laub kosten gleich viel.
- **Kanten.** Eine Kante schliesst, wenn die Flächen der beiden Blöcke an
  ihr zusammen die ganze Seite decken (`LightEngine.shapeOccludes`): die
  Unterseite einer unteren Platte allein, eine obere neben einer unteren
  Platte zusammen, zwei untere nebeneinander nicht.
- **Himmel.** In jeder Spalte ist jede Zelle 15, die über dem obersten
  Block liegt, der dämpft oder dessen Kante nach oben schliesst
  (`ChunkSkyLightSources`); von dort breitet es sich aus. Unter Wasser
  und unter Laub verliert es so eine Stufe je Block, unter einem Überhang
  eine je Block Abstand zur offenen Spalte. Eine geschlossene Höhle bleibt
  bei 0.
- **Block.** Was leuchtet, beginnt mit seiner Stufe aus `leuchten.txt`,
  auch ein dichter Block wie die Seelaterne oder der Magmablock.
- **Rand.** Gerechnet wird je Chunk in einem Fenster, das 14 Blöcke in
  die Nachbarn reicht, so weit wie Licht kommt. Ein Chunk, der fehlt oder
  nicht fertig ist, lässt kein Licht herein. In der Höhe reicht es wie im
  Spiel eine Section unter und über die Sections mit Blöcken
  (`LevelLightEngine.getMinLightSection`).

Welche Blöcke wie stark dämpfen und mit welchen Flächen sie Kanten
schliessen, steht in `renderer/src/assets/licht.txt`, siehe
[Erzeugte Tabellen](../entwicklung/tabellen.md). Gerechnet wird skalar,
mit einem Eimer je Stufe von 15 abwärts, wenn ein Chunk zum ersten Mal
Licht braucht; es bleibt im Cache des Threads, solange der Chunk dort
liegt. Gegen einen Lauf von Vanilla 26.2 stimmt jede Zelle, siehe
[Tests](../entwicklung/tests.md), „Fixtures“.

## Welches Licht ein Block bekommt

Welches Licht ein Block bekommt, bestimmt `light_at` beim Zeichnen, aus den
Blöcken über und neben ihm:

- Die Oberseite des Grunds liegt im Licht des Wassers über ihr, auch in
  einer Luftblase darunter.
- Ein Block mit eigenem Wasser, Seegras, Kelp, ein gefluteter Zaun, liegt im
  Licht dieses Wassers. Reines Wasser liegt eine Stufe heller:
  `FluidRenderer` zeichnet es im helleren Licht aus seiner Zelle und der
  darüber, unter einer Brücke also im Licht der Luft darunter; ein deckender
  Block darüber hat selbst kein Licht. So sieht man Kelp knapp unter der
  Oberfläche auch über tiefem Grund, wie im Spiel.
- Verdeckt der Block darüber die Oberseite, gilt das Wasser vor der Ost-
  und der Südseite: ein Schiffsrumpf, eine Klippe unter Wasser.
- Ein deckender Block nimmt ebenso eine Stufe wie ein Block Wasser. So
  bleiben eine geflutete Höhle unter dem Meeresboden, der Grund unter einem
  Stein im See und eine Luftblase im Meer dunkel.
- An Land bleibt alles im Licht 15, auch unter einem Überhang.
- Was selbst leuchtet, bringt sein Blocklicht mit, siehe oben.

## Licht von der Seite

An zwei Stellen kommt das Licht von der Seite, und mit ihm endet die
Zählung:

- **Neben Luft.** Hat ein Block Wasser Luft neben sich, die selbst im Licht
  liegt, über der also kein Wasser steht, liegt er im Licht 14.
  `FluidRenderer` zeichnet eine Flüssigkeit im Licht ihrer Zelle und der
  darüber, und das kommt im Spiel auch von der Seite: Der oberste Block
  eines Wasserfalls liegt unter freiem Himmel im Licht 15, jeder darunter
  bis zum Fuss im Licht 14. Unter einem Fall liegt der Grund eines Beckens
  eine Stufe tiefer als daneben. Luft unter Wasser, eine Luftblase oder ein
  Kasten aus Glas am Grund, liegt selbst im Dunkeln; neben ihr zählt das
  Wasser weiter bis zur Oberfläche.
- **Unter einem Deckel.** Liegt unter einem deckenden Block eine Lücke,
  weder Wasser noch deckend, kommt das Licht dort von der Seite: Wasser auf
  einer Brücke ändert am Boden darunter nichts, und unter einem Felsbogen
  liegt die Oberfläche eines Flusses im Licht 14, ihre Zelle und der Grund
  einen Block tiefer im Licht 13, gleich wie dick der Fels ist.

## Wie gezählt wird

Gezählt wird aus den Bitmasken der Sections, ein paar Wörter je Block
(`column_above`); nur wo Wasser steht, kommen die vier Spalten daneben dazu.
Ob über einer Lücke Wasser steht, sagt je Chunk und Spalte die Höhe des
obersten Wassers, einmal beim Laden aus den Masken bestimmt. Ein Chunk, der
fehlt oder nicht fertig erzeugt ist, gilt dabei nicht als Luft, am Rand der
Welt kommt kein Licht von der Seite, siehe
[Welten und Kennung](../benutzung/welten.md), „Nicht fertig erzeugte
Chunks“.

Der Blit multipliziert jeden Pixel je Kanal mit b, ganzzahlig wie das
Mischen, auf der CPU wie im Shader der Karte; die Oberfläche selbst bleibt,
wie sie ist. Ein gefluteter Block an der Oberfläche zeichnet sein Wasser im
eigenen Sprite, und was er darunter trägt, liegt dort im Licht 14 und in
seinem eigenen Blocklicht: Eine geflutete Laterne bleibt auch unter ihrer
Oberfläche hell.

## Was bleibt eine Näherung

- **Eine Zahl je Block.** Im Spiel liegen die Seiten eines Blocks unter
  Wasser eine Stufe dunkler als seine Oberseite, am Ufer die Seite unter
  der Oberfläche im Licht 14, während die Oberseite trocken im Licht 15
  liegt.
- **Ein gefluteter Block an der Oberfläche** liegt hier im Licht 15; nur
  was im Bild hinter seiner eigenen Wasseroberfläche liegt, liegt im
  Licht 14, siehe „Wie gezählt wird“. Im Spiel liegt ein Bild aus dem
  Blockentity ganz im Licht seiner Zelle, 14
  (`BlockEntityRenderState.extractBase`): Bei einer gefluteten Truhe liegt
  der untere Teil der Seiten hier heller als der Deckel. Ein geflutetes
  Blockmodell nimmt das Spiel je Fläche, wie im Punkt davor.
- **Licht von der Seite nur an den zwei Stellen oben.** Wie weit es im
  Spiel unter ein Dach oder in eine Höhle fällt, eine Stufe weniger je
  Block, zählt der Renderer nicht, denn das gespeicherte Licht der Welt
  liest er nicht; unter einem breiten Überhang liegt Wasser deshalb heller
  als im Spiel.
- **Luft und Glas unter Wasser ohne Verlust.** Unter Wasser nimmt im Spiel
  jeder Block eine Stufe, auch Luft und Glas: Ohne Verlust fällt nur volles
  Himmelslicht, sonst kostet jeder Schritt mindestens eine
  (`LightEngine.getOpacity`). Hier lassen Luft, Glas und trockenes Laub das
  Licht durch. Der Grund in einer Luftblase liegt so eine Stufe heller als
  im Spiel, der Boden einer Kuppel aus Glas am Grund um ihre Höhe heller.
  Ob Luft im Licht liegt, entscheidet allein, ob in ihrer Spalte darüber
  Wasser steht, wie weit oben auch immer: Luft unter einem Überhang, auf
  dem ein Teich liegt, gilt als dunkel.
- **Blocklicht nur am Block selbst.** Den Schein auf die Nachbarn, im Spiel
  eine Stufe weniger je Block, rechnet der Renderer nicht. Der Grund neben
  einer Seelaterne liegt im Himmelslicht.
- **Kein Flackern.** Das Spiel lässt `BlockFactor` zufällig um 1,4
  flackern; der Renderer nimmt 1,4.
- **Die Oberfläche bleibt eben.** Minecraft gleicht die Eckhöhen an die
  Nachbarn an; hier endet jeder Block auf seiner eigenen Höhe. Wo das eine
  Lücke liesse, zeichnet der Renderer einen Streifen.
