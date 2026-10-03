---
title: Weiche Beleuchtung
description: Wie der Renderer Blöcke weich beleuchtet wie das Spiel in der Voreinstellung, nach den Regeln von BlockModelLighter in 26.3, mit dem Licht an jeder Ecke der Flächen auf dem Rand und im Innern, und was eine Näherung bleibt.
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
`BlockModelLighter.prepareQuadAmbientOcclusion` rechnet dafür in 26.3 je
Ecke einer Fläche einen Wert, und die Grafikkarte lässt ihn zwischen den
Ecken verlaufen, mit ihm das Licht an jeder Ecke. Der Renderer tut dasselbe
für die drei Seiten, die er im Blick zeigt, aus der Vorgabe oben, Süden
und Osten, siehe „Aus jeder Richtung“: `ecken_at` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs),
die AO-Karte in `renderer/src/render/rasterizer.rs`. Das gilt für jede
Fläche auf dem Rand des Blocks, auch die einer Treppe, einer Platte oder
eines Zauns, und für jede im Innern, etwa die Oberseite einer unteren
Platte oder eines Trampelpfads, siehe
[0064](../entscheidungen/0064-flaechen-im-innern-weich.md).

## Die Regeln des Spiels

- Gezählt wird in der Schicht vor der Seite, für eine Fläche, die eben auf
  dem Rand des Würfels liegt, bei voller Kollisionsform des Blocks auch im
  Innern (`faceCubic` in `BlockModelLighter.prepareQuadShape`), bis auf
  1e-4, denn der Baker dreht über sin und cos und trifft die Ebene nur
  fast. Ob die Fläche die ganze Seite deckt, spielt dafür keine Rolle: Die
  Fächer einer Chiseled Bookshelf und die Oberseite einer oberen Platte
  zählen wie die Seite eines Steins. Gezählt werden der
  Block direkt davor, je Ecke ihre zwei Nachbarn in dieser Schicht
  (`AdjacencyInfo.corners`) und der Block in der Ecke dazwischen. Jeder gibt
  seine `getShadeBrightness`: 0,2 für Blöcke mit voller Kollisionsform
  (`BlockBehaviour`), Stein etwa, Erde, Eis und Laub, sonst 1, für Wasser,
  Treppen und einfache Platten. Einzelne Blöcke weichen ab: Glas bleibt bei
  1 (`TransparentBlock`), Seelensand und Schlamm dunkeln trotz kleinerer
  Form ab (`SoulSandBlock`, `MudBlock`). Jede Ecke ist das Mittel ihrer vier
  Werte, und `ARGB.gray` macht daraus 255, 204, 153, 102 oder 51.
- Jede andere Fläche zählt das Spiel ab der eigenen Zelle: die Nachbarn und
  die Blöcke in den Ecken in der Schicht des Blocks selbst, statt des
  Blocks davor den Block selbst (`prepareQuadAmbientOcclusion`). Ihre Seite
  ist die Richtung, die ihrer Normalen am nächsten liegt, bei Gleichstand
  die erste aus `Direction.values()`: unten, oben, Norden, Süden, Westen,
  Osten (`FaceBakery.findClosestDirection`). So zählt auch eine schräge
  Fläche, und die Ebene einer Blume mit der Normalen nach Südosten liegt
  im Süden.
- Der Block in der Ecke zählt nur, wenn hinter einem der beiden Nachbarn,
  noch eine Schicht weiter von der Seite weg, kein Block steht, der kein
  Licht durchlässt: nicht `isLightPermeable`, also `solidRender` und
  `getLightDampening` > 0. In 26.2 stand `isViewBlocking` statt
  `solidRender`; seit 26.3 lassen Eis, Brucheis, Schleimblöcke, die
  Shulkerkisten, Leuchtfeuer, Spawner und Barriere die Ecke durch. Sonst nimmt
  das Spiel an seiner Stelle den Wert des ersten Nachbarn aus
  `AdjacencyInfo.corners`, für alle vier Ecken denselben, auch für eine
  Ecke, die dieser Nachbar gar nicht berührt. Der Renderer auch.
- `AmbientVertexRemap` legt die vier Werte auf die Ecken aus `FaceInfo`. In
  diese Reihenfolge bringt `FaceBakery.recalculateWinding` jedes gebackene
  Viereck, und das Spiel zeichnet es als die Dreiecke 0-1-2 und 2-3-0
  (`RenderSystem.sharedSequentialQuad`). Dazwischen verläuft der Wert
  baryzentrisch. Deckt eine Fläche die Seite nur zum Teil (`facePartial`),
  mischt das Spiel die vier Werte an jeder ihrer Ecken nach deren Lage in
  der Seite (`AdjacencyInfo.vert0Weights` bis `vert3Weights`).
- Das Licht mischt das Spiel an denselben Ecken aus denselben Blöcken,
  siehe „Licht an den Ecken“.
- Weich beleuchtet wird nur, was das Modell mit `ambientocclusion` erlaubt.
  Das erbt wie im Client vom nächsten Parent, der es setzt
  (`ResolvedModel.findTopAmbientOcclusion`). Was leuchtet, zeichnet das
  Spiel ohne (`ModelBlockRenderer.tesselateBlock`), Flüssigkeiten ebenso
  (`FluidRenderer`) und die Flächen aus Blockentity-Modellen, siehe
  [Blockentities](blockentities.md), „Licht“.

## Aus jeder Richtung

Das Spiel rechnet jede Fläche mit den Tabellen ihrer Seite in der Welt.
Aus einer anderen Richtung liegt im Blick an einer Seite eine andere Seite
der Welt, siehe [Richtungen](richtungen.md):

- **Die Tabellen** für oben und die vier Seiten rundum, per javap am
  26.2-Client (`AdjacencyInfo`, `AmbientVertexRemap`, `FaceInfo`):

  | Seite | `corners` | `AmbientVertexRemap` | Ecken aus `FaceInfo` |
  |---|---|---|---|
  | oben | Osten, Westen, Norden, Süden | 2, 3, 0, 1 | (0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0) |
  | Norden | oben, unten, Osten, Westen | 3, 0, 1, 2 | (1, 1, 0), (1, 0, 0), (0, 0, 0), (0, 1, 0) |
  | Süden | Westen, Osten, unten, oben | 0, 1, 2, 3 | (0, 1, 1), (0, 0, 1), (1, 0, 1), (1, 1, 1) |
  | Westen | oben, unten, Norden, Süden | 3, 0, 1, 2 | (0, 1, 0), (0, 0, 0), (0, 0, 1), (0, 1, 1) |
  | Osten | unten, oben, Norden, Süden | 1, 2, 3, 0 | (1, 1, 1), (1, 0, 1), (1, 0, 0), (1, 1, 0) |

- **Im Blick** nimmt `ao_seiten` in `metatile.rs` für jede der drei Seiten
  die Zeile der Seite der Welt und dreht ihre Nachbarn in den Blick;
  `ecken_im_blick` in `rasterizer.rs` dreht ihre Ecken. Die AO-Karte und
  die Werte von `ecken_at` stehen damit in der Reihenfolge der Ecken der
  Seite der Welt.
- **Die Diagonale** zwischen den Dreiecken 0-1-2 und 2-3-0 dreht sich mit:
  Aus `sw` und `ne` läuft sie auf der Oberseite über die anderen beiden
  Ecken als aus `se` und `nw`. Auf den Seiten bleibt sie, denn die Ecken
  aus `FaceInfo` liegen dort für jede Seite gleich, von aussen gesehen.
- **Der Ersatz** für einen Block in der Ecke, der nicht zählt, ist der
  erste Nachbar aus `corners` der Seite der Welt: Zeigt die Seite im Süden
  des Blicks aus `nw` den Norden, ist das der Block oben, nicht der im
  Westen wie für den Süden.

## Licht an den Ecken

An jeder Ecke mischt das Spiel auch das Licht der vier Zellen, deren
Schatten es zählt (`LightCoordsUtil.smoothBlend`), wie `smooth_blend` in
`renderer/src/render/rasterizer.rs`. Belegt per javap:

- Jede Zelle gibt ihr Licht wie `LightCoordsUtil.getLightCoords`: Himmels-
  und Blocklicht aus der Ausbreitung, siehe
  [Wasser und Licht](wasser-und-licht.md), „Licht ausbreiten“; ein Block,
  den das Spiel mit `emissiveRendering` zeichnet, der Magmablock etwa,
  gibt beide 15.
- Die Mitte ist auf dem Rand die Zelle vor der Seite. Im Innern ist sie
  es auch, ausser ihr Block ist `isSolidRender`, Stein etwa, nicht aber
  Glas, Eis, Laub oder eine Platte; dann ist es die eigene Zelle
  (`prepareQuadAmbientOcclusion`). `isSolidRender` ist in 26.3 genau,
  was kein Licht durch die Ecke lässt (`getLightDampening` ist dort 15). Die Nachbarn liegen in der Schicht, in
  der das Spiel zählt.
- Ist die Mitte heller als 2, im Himmels- oder im Blocklicht, nimmt ein
  Nachbar ganz ohne Licht ihres und einer ohne Himmelslicht ihr
  Himmelslicht. Ein Stein neben der Seite zieht die Ecke
  so nicht ins Dunkle; dunkler wird sie über seinen Schatten.
- Die Ecke ist das Mittel der vier Werte, in Sechzehnteln einer Stufe. Die
  Lightmap liest das Spiel je Ecke linear gefiltert (`terrain.vsh`,
  `ChunkSectionsToRender`, `FilterMode.LINEAR`), also zwischen den Stufen
  daneben gemischt, in beiden Lichtern: `Lightmap::linear`.
- Mal dem Schatten der Ecke gibt das je Farbkanal ihre Helligkeit, und
  sie verläuft zwischen den Ecken wie der Schatten.

Ohne weiche Beleuchtung, bei einem Block, der leuchtet, oder einem Modell
ohne `ambientocclusion`, liegt jede Fläche auf dem Rand ganz im Licht der
Zelle vor ihr, mit dem eigenen Blocklicht, wenn das heller ist
(`BlockModelLighter.prepareQuadFlat`). Eine Fläche im Innern liegt in
beiden Fällen im Licht der eigenen Zelle.

## Beim Zeichnen

Das Sprite eines Blocks ist an jeder Stelle dasselbe, Schatten und Licht
hängen aber an den Nachbarn. Der Rasterizer legt deshalb je Pixel seinen
Platz und die Anteile der vier Ecken seiner Seite in 255steln ab, die
AO-Karte. Je Seite im Blick gibt es zwei Plätze, einen für Flächen auf dem
Rand und einen für Flächen im Innern, zusammen sechs (`AO_PLAETZE`): Eine
Treppe zeigt oben so beide Stufen, jede mit ihren Ecken. Die Karte gilt
auch ohne `ambientocclusion`, dann trägt jeder Platz sein eigenes Licht.
Ein Pixel ohne Platz liegt im Licht der eigenen Zelle: Flüssigkeiten,
Flächen aus Blockentity-Modellen und Flächen, deren Seite der Blick nicht
zeigt. Liegt Wasser vor einer Fläche, zählt die Fläche: Den Anteil des
Wassers beleuchtet die Tönungskarte mit dem Licht des Wassers, siehe
[Wasser und Licht](wasser-und-licht.md), „Welches Licht ein Block
bekommt“; der Platz gilt für das, was durch das Wasser zu sehen ist. Ob ein Sprite solche Pixel hat, merkt sich die Tabelle
(`SpriteSet::innen`), welche Plätze es zeigt, auch (`SpriteSet::plaetze`).
Ob der Block volle Kollisionsform hat, steht in `schatten.txt` und gehört
zum Schlüssel der Familie. Beim Zeichnen rechnet `ecken_at` die Ecken der
Plätze, die das Sprite zeigt, aus den Nachbarn, ihr Licht aus den 27
Zellen um den Block (`lichter_um`). Ob ein Block abdunkelt, ob er die
Sicht nimmt, liegt dafür wie „deckend“ als eigene Ebene in den
Bitmasken der Sections (`DARK`, `VIEW`), auch für Blöcke ohne Sprite. Die
Sicht nimmt in 26.3 genau ein Block, der `isSolidRender` ist, siehe „Die
Regeln des Spiels“; dieselbe Ebene gibt deshalb auch die Mitte einer
Fläche im Innern, geprüft von `sicht_ist_solid_render` in
`renderer/src/assets/blockstate.rs`. Die 38 Blöcke, nach denen die sechs
Plätze fragen, kommen aus 15 Spalten, je Spalte aus einem Wort
(`umgebung`). Welche Blöcke abdunkeln und welche die Sicht nehmen, steht
in `renderer/src/assets/schatten.txt`, siehe
[Erzeugte Tabellen](../entwicklung/tabellen.md) und
[0031](../entscheidungen/0031-eigene-tabellen-statt-der-masken.md).

Je Pixel ergibt die Karte mit den Ecken je Farbkanal einen Faktor,
ganzzahlig wie das Mischen, auf der CPU wie im Shader der Karte; die
Schattierung nach Richtung steckt wie bisher im Sprite, siehe
[Dimensionstypen](dimensionstypen.md), „Schattierung nach Richtung“.
Eine Instanz auf der Karte trägt dafür je Kanal und Platz ein Wort,
18 Wörter, siehe [Grafikkarte](../benutzung/grafikkarte.md). Haben alle
Ecken der Plätze, die das Sprite zeigt, dasselbe Licht, und die Pixel ohne
Platz, wenn es welche gibt, auch, trägt der Draw es allein, ohne Ecken;
ein Platz, dessen Seite ihr Nachbar deckt, zählt dabei nicht.

## Was es kostet

Eine Kachel wiegt damit je nach Inhalt 18 bis 30 % mehr, auf der grossen
Welt rund ein Viertel. Auf einem Thread braucht sie bei scale 32 ein Siebtel
länger, 6,13 statt 5,38 ms. 0,35 ms davon sind das Abdunkeln und Kodieren
der reicheren Kachel, 0,19 ms die Rechnung je Pixel, 0,06 ms die Ecken
(damals aus `ao_at`, heute `ecken_at`), und 0,15 ms braucht der Stand auch
ohne Ecken. Auf 24 Threads ist
die Basis der grossen Welt ohne Karte 11 bis 14 % langsamer als master, mit
Karte 6 bis 7 %. Gemessen in
[2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md);
die Dauer für die ganze Welt steht in
[Was ein Lauf kostet](../benutzung/kosten.md). Das war vor dem Licht aus
der Ausbreitung. Mit ihr und dem Licht je Ecke schafft die Basis auf 24
Threads 23 bis 40 % weniger Kacheln je Sekunde, auf einem Thread kostet
eine Kachel 22 % mehr, und ein ganzer Lauf mit drei nativen Stufen braucht
51 bis 64 % länger, gemessen in
[2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md); gerechnet waren für
die Basis 27 %, siehe
[0040](../entscheidungen/0040-licht-selbst-ausbreiten.md).

Seit auch die Flächen auf dem Rand von Teilmodellen ihre Seite tragen, hat
fast jedes Sprite eine AO-Karte, 4 Bytes je Pixel wie das Bild. Über alle
32 366 Zustände von 26.2 bei scale 32 sind das 17 449 von 21 185 Sprites
mit 53 MB Karte neben 61 MB Bild, vorher 744 Sprites mit 3 MB. Eine Welt
braucht nur die Zustände, die in ihr vorkommen. Ein Teilmodell rechnet
dazu seine Ecken aus den Nachbarn wie ein Stein, wo vorher ein Licht je
Block reichte. Für sich gemessen ist das nicht, nur mit dem ganzen Licht
zusammen, siehe oben.

Die Flächen im Innern aus
[0064](../entscheidungen/0064-flaechen-im-innern-weich.md) kosten an Stand
und Fichtenwald der Testwelt mit 24 Threads: die Karte auf der
Grafikkarte am Stand 4 bis 5 % mehr Zeit, Cinematic am Stand 3 %, die
Karte auf der CPU und der Fichtenwald nichts über der Streuung. Die
Kacheln wiegen 0,1 bis 5,7 % mehr, am meisten mit Schnee, die Spitze des
Speichers 0,3 bis 5,7 %. Gemessen in
[2026-10-03, Flächen im Innern weich, Kosten](../messungen/2026-10-03-flaechen-im-innern.md).

## Was noch fehlt

Aus den Regeln des Spiels nichts mehr. Seit
[0064](../entscheidungen/0064-flaechen-im-innern-weich.md) liegen auch die
Flächen im Innern weich, die grössten darunter:

- Schneedecken: Eine Lage `snow` ist 2/16 hoch, erst acht Lagen sind ein
  voller Würfel.
- Ackerboden und Trampelpfade (`farmland`, `dirt_path`), 15/16 hoch.
- Die Oberseite einer unteren Platte und die untere Stufe einer Treppe.

Als Nachbarn zählen sie mit ihrem Wert aus der Tabelle: Schneedecken,
Ackerboden, Trampelpfade, Treppen und einfache Platten dunkeln nicht ab,
acht Lagen Schnee und eine doppelte Platte schon.

## Was bleibt eine Näherung

- **Flächen, deren Seite der Blick nicht zeigt**, und die doch zu sehen
  sind, etwa eine geneigte, die eher nach Norden als nach oben zeigt, von
  oben gesehen: Sie liegen flach im Licht der eigenen Zelle. Das Spiel
  beleuchtet sie weich in ihrer Richtung. Gezählt über alle Zustände aus
  `blocks.txt` mit Modell in den Assets von 26.2, nur Modelle mit
  `ambientocclusion`, bei scale 32: aus 2:1 keine Fläche, aus 4:3 6 am
  Haken des Stolperdrahts, aus `top` und `top-north` 26 an drei Blöcken,
  aus `north-45` 285 an 20 Blöcken, fast alle an Kerzen.
- **Teilflächen**, auf dem Rand wie im Innern, nehmen an jedem Pixel den
  Verlauf der ganzen Seite über ihre zwei Dreiecke. Das Spiel mischt die vier Werte an den
  Ecken der Fläche bilinear nach ihrer Lage und lässt sie von dort über die
  Fläche verlaufen. Auf den Kanten der Seite ist beides gleich; im Innern
  weicht es so weit ab, wie eine bilineare Mischung von der über zwei
  Dreiecke.
