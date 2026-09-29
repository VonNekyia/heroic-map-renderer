---
title: Weiche Beleuchtung
description: Wie der Renderer volle Würfel weich beleuchtet wie das Spiel in der Voreinstellung, nach den Regeln von BlockModelLighter in 26.2, mit dem Licht an jeder Ecke, und was noch fehlt.
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/render/gpu.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/model.rs
  - renderer/src/assets/schatten.txt
  - renderer/src/assets/Schatten.java
---

# Weiche Beleuchtung

Das Spiel zeichnet Blöcke in der Voreinstellung weich beleuchtet
(`options.ao` ist wahr): Wo eine Fläche an einen Nachbarn stösst, wird sie
zur Kante hin dunkler, in einer Innenecke am meisten.
`BlockModelLighter.prepareQuadAmbientOcclusion` rechnet dafür in 26.2 je
Ecke einer Fläche einen Wert, und die Grafikkarte lässt ihn zwischen den
Ecken verlaufen, mit ihm das Licht an jeder Ecke. Der Renderer tut dasselbe
für die drei Seiten, die er zeigt, oben, Süden und Osten: `ecken_at` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs),
die AO-Karte in `renderer/src/render/rasterizer.rs`. Bisher nur für Modelle
aus vollen Seiten, siehe
[0032](../entscheidungen/0032-weiche-beleuchtung-zuerst-fuer-volle-wuerfel.md).

## Die Regeln des Spiels

- Gezählt wird in der Schicht vor der Seite, für eine Seite, die ganz auf
  dem Rand des Würfels liegt (`faceCubic` in
  `BlockModelLighter.prepareQuadShape`, nicht `facePartial`), bis auf 1e-4,
  denn der Baker dreht über sin und cos und trifft die Ebene nur fast: der
  Block direkt davor, je Ecke ihre zwei Nachbarn in dieser Schicht
  (`AdjacencyInfo.corners`) und der Block in der Ecke dazwischen. Jeder gibt
  seine `getShadeBrightness`: 0,2 für Blöcke mit voller Kollisionsform
  (`BlockBehaviour`), Stein etwa, Erde, Eis und Laub, sonst 1, für Wasser,
  Treppen und einfache Platten. Einzelne Blöcke weichen ab: Glas bleibt bei
  1 (`TransparentBlock`), Seelensand und Schlamm dunkeln trotz kleinerer
  Form ab (`SoulSandBlock`, `MudBlock`). Jede Ecke ist das Mittel ihrer vier
  Werte, und `ARGB.gray` macht daraus 255, 204, 153, 102 oder 51.
- Der Block in der Ecke zählt nur, wenn hinter einem der beiden Nachbarn,
  noch eine Schicht weiter von der Seite weg, kein Block steht, der die
  Sicht nimmt (`isViewBlocking` und `getLightDampening` > 0). Sonst nimmt
  das Spiel an seiner Stelle den Wert des ersten Nachbarn aus
  `AdjacencyInfo.corners`, für alle vier Ecken denselben, auch für eine
  Ecke, die dieser Nachbar gar nicht berührt. Der Renderer auch.
- `AmbientVertexRemap` legt die vier Werte auf die Ecken aus `FaceInfo`. In
  diese Reihenfolge bringt `FaceBakery.recalculateWinding` jedes gebackene
  Viereck, und das Spiel zeichnet es als die Dreiecke 0-1-2 und 2-3-0
  (`RenderSystem.sharedSequentialQuad`). Dazwischen verläuft der Wert
  baryzentrisch.
- Das Licht mischt das Spiel an denselben Ecken aus denselben Blöcken,
  siehe „Licht an den Ecken“.
- Weich beleuchtet wird nur, was das Modell mit `ambientocclusion` erlaubt.
  Das erbt wie im Client vom nächsten Parent, der es setzt
  (`ResolvedModel.findTopAmbientOcclusion`). Was leuchtet, zeichnet das
  Spiel ohne (`ModelBlockRenderer.tesselateBlock`), Flüssigkeiten ebenso
  (`FluidRenderer`) und die Flächen aus Blockentity-Modellen, siehe
  [Blockentities](blockentities.md), „Licht“.

## Licht an den Ecken

An jeder Ecke mischt das Spiel auch das Licht der vier Zellen, deren
Schatten es zählt (`LightCoordsUtil.smoothBlend`), wie `smooth_blend` in
`renderer/src/render/rasterizer.rs`. Belegt per javap:

- Jede Zelle gibt ihr Licht wie `LightCoordsUtil.getLightCoords`: Himmels-
  und Blocklicht aus der Ausbreitung, siehe
  [Wasser und Licht](wasser-und-licht.md), „Licht ausbreiten“; ein Block,
  den das Spiel mit `emissiveRendering` zeichnet, der Magmablock etwa,
  gibt beide 15.
- Ist die Zelle vor der Seite heller als 2, im Himmels- oder im
  Blocklicht, nimmt ein Nachbar ganz ohne Licht ihres und einer ohne
  Himmelslicht ihr Himmelslicht. Ein Stein neben der Seite zieht die Ecke
  so nicht ins Dunkle; dunkler wird sie über seinen Schatten.
- Die Ecke ist das Mittel der vier Werte, in Sechzehnteln einer Stufe. Die
  Lightmap liest das Spiel je Ecke linear gefiltert (`terrain.vsh`,
  `ChunkSectionsToRender`, `FilterMode.LINEAR`), also zwischen den Stufen
  daneben gemischt, in beiden Lichtern: `licht_linear`.
- Mal dem Schatten der Ecke gibt das je Farbkanal ihre Helligkeit, und
  sie verläuft zwischen den Ecken wie der Schatten.

Ohne weiche Beleuchtung, bei einem Block, der leuchtet, oder einem Modell
ohne `ambientocclusion`, liegt jede Seite ganz im Licht der Zelle vor ihr,
mit dem eigenen Blocklicht, wenn das heller ist
(`BlockModelLighter.prepareQuadFlat`).

## Beim Zeichnen

Das Sprite eines Blocks ist an jeder Stelle dasselbe, Schatten und Licht
hängen aber an den Nachbarn. Der Rasterizer legt deshalb je Pixel die
Anteile der vier Ecken seiner Seite in 255steln ab, die AO-Karte, für jedes
Modell aus vollen Seiten, auch ohne `ambientocclusion`: Dann trägt jede
Seite ihr eigenes Licht. Beim Zeichnen rechnet `ecken_at` die Ecken aus den
Nachbarn, ihr Licht aus den 27 Zellen um den Block (`lichter_um`). Ob ein Block
abdunkelt und ob er die Sicht nimmt, liegt dafür wie „deckend“ als eigene
Ebene in den Bitmasken der Sections (`DARK`, `VIEW`), auch für Blöcke ohne
Sprite: Die 31 Blöcke, nach denen die drei Seiten fragen, kommen aus 15
Spalten, je Spalte aus einem Wort (`umgebung`). Welche Blöcke abdunkeln und
welche die Sicht nehmen, steht in `renderer/src/assets/schatten.txt`, siehe
[Erzeugte Tabellen](../entwicklung/tabellen.md) und
[0031](../entscheidungen/0031-eigene-tabellen-statt-der-masken.md).

Je Pixel ergibt die Karte mit den Ecken je Farbkanal einen Faktor,
ganzzahlig wie das Mischen, auf der CPU wie im Shader der Karte; die
Schattierung nach Richtung, oben 1, Nord und Süd 0,8, Ost und West 0,6,
steckt wie bisher im Sprite. Eine Instanz auf der Karte trägt dafür je
Kanal und Seite ein Wort, neun Wörter, siehe
[Grafikkarte](../benutzung/grafikkarte.md). Haben alle Ecken der Seiten,
die zu sehen sind, dasselbe Licht, trägt der Draw es allein, ohne Ecken;
eine Seite, die ihr Nachbar deckt, zählt dabei nicht.

## Was es kostet

Eine Kachel wiegt damit je nach Inhalt 18 bis 30 % mehr, auf der grossen
Welt rund ein Viertel. Auf einem Thread braucht sie bei scale 32 ein Siebtel
länger, 6,13 statt 5,38 ms. 0,35 ms davon sind das Abdunkeln und Kodieren
der reicheren Kachel, 0,19 ms die Rechnung je Pixel, 0,06 ms die Ecken aus
`ao_at`, und 0,15 ms braucht der Stand auch ohne Ecken. Auf 24 Threads ist
die Basis der grossen Welt ohne Karte 11 bis 14 % langsamer als master, mit
Karte 6 bis 7 %. Gemessen in
[2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md);
die Dauer für die ganze Welt steht in
[Was ein Lauf kostet](../benutzung/kosten.md).

## Was noch fehlt

- **Teilflächen.** Alles, was nicht ganz aus vollen Seiten besteht, rechnet
  das Spiel mit `facePartial` und den Gewichten aus `SizeInfo`. Der
  Renderer zeichnet es wie bisher ohne weiche Beleuchtung; eine AO-Karte
  bekommt nur ein Modell aus vollen Seiten. Die grössten Flächen darunter:
  - Schneedecken: Eine Lage `snow` ist 2/16 hoch, erst acht Lagen sind ein
    voller Würfel. Verschneite Hänge und Ebenen bleiben oben deshalb ohne
    weiche Beleuchtung.
  - Ackerboden und Trampelpfade (`farmland`, `dirt_path`), 15/16 hoch.
  - Treppen, Platten und Zäune.

  Als Nachbarn zählen sie schon mit ihrem Wert aus der Tabelle:
  Schneedecken, Ackerboden, Trampelpfade, Treppen und einfache Platten
  dunkeln nicht ab, acht Lagen Schnee und eine doppelte Platte schon.

## Was bleibt eine Näherung

- **Nur volle Seiten**, siehe „Was noch fehlt“. Alles andere liegt ganz
  im Licht seiner Zelle, siehe [Wasser und Licht](wasser-und-licht.md),
  „Welches Licht ein Block bekommt“.
