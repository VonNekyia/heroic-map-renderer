---
title: map.json
description: Die Felder von map.json, wann der Export die Datei schreibt, die Höhen je Region für die Koordinatenanzeige, wie das Frontend die Projektion nachrechnet und warum ein Baum seinen Radius der Mischung behält.
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/heights.rs
  - renderer/src/world/chunk.rs
  - renderer/src/cli.rs
  - renderer/tests/fixtures/projektion.json
  - web/src/main.ts
---

# `map.json`

`map.json` liegt an der Wurzel jedes Kachelbaums und sagt dem Frontend, was
es vorfindet: Kachelgrösse, scale, Zoomstufen, Pfadmuster, den belegten
Bereich, die Zahl nativer Stufen, den Radius der Mischung der Biomfarben,
die Kennung der Welt und wo die Höhen liegen. Der Typ ist
`MapInfo` in
[`renderer/src/render/pyramid.rs`](../../renderer/src/render/pyramid.rs);
das Frontend liest die Datei in `web/src/main.ts`.

## Felder

```json
{
  "tileSize": 256,
  "scale": 32,
  "minZoom": 0,
  "maxZoom": 10,
  "tiles": "{z}/{x}/{y}.webp",
  "bounds": [-10240, 0, -6144, 4096],
  "nativeLevels": 0,
  "biomeBlend": 2,
  "world": "cb13a94d6c88dae1-6872d5d8ff54db07",
  "heights": "heights/{x}.{z}.bin",
  "heightsCell": 4,
  "minY": -64,
  "maxY": 319
}
```

| Feld | Inhalt | Mehr |
|---|---|---|
| `tileSize` | Kantenlänge einer Kachel in Pixeln | |
| `scale` | Pixelbreite eines Blocks auf der Basis | [Kamera](../renderer/kamera.md) |
| `minZoom`, `maxZoom` | gröbste und feinste Stufe; `maxZoom` ist die Basis | [Zoomstufen](zoomstufen.md) |
| `tiles` | Pfadmuster der Kacheln | [Kacheln exportieren](kacheln.md) |
| `bounds` | belegter Bereich auf der feinsten Stufe in Pixeln, `[links, oben, rechts, unten]` | |
| `nativeLevels` | Zahl der nativen Stufen | [Zoomstufen](zoomstufen.md), „Native Stufen“ |
| `biomeBlend` | Radius der Mischung der Biomfarben, `--biome-blend` | „Radius der Mischung“ unten |
| `world` | Kennung der Welt und Dimension, oder `null` | [Welten und Kennung](welten.md) |
| `heights` | Pfadmuster der Höhen je Region; fehlt es, hat der Baum keine | „Höhen“ unten |
| `heightsCell` | Kantenlänge einer Zelle der Höhen in Blöcken, heute 4; steht mit `heights` | „Höhen“ unten |
| `minY`, `maxY` | unterster und oberster Block, den der Renderer zeichnet; stehen mit `heights` | „Höhen“ unten |

Die Projektion selbst steht nicht drin: sie hängt allein an `scale`, die
Formel steht in [Kamera](../renderer/kamera.md), „Projektion“. Für die
Koordinaten rechnet das Frontend sie nach. Damit es dabei nicht vom Renderer
abweicht, liegen Paare aus Block und Bildpunkt in
[`renderer/tests/fixtures/projektion.json`](../../renderer/tests/fixtures/projektion.json).
Ein Test des Renderers hält die Datei aktuell, und das Frontend prüft sein
Modell daran.

## Höhen

Das Frontend zeigt unter Maus und Finger die Koordinaten des Blocks, siehe
[Frontend](../frontend.md). Dafür braucht es je Zelle eine Höhe. Die
liefert der Renderer:

- **Datei:** je Region `heights/{x}.{z}.bin` neben den Kacheln, x und z
  wie in `r.x.z.mca`. Darin steht ein zlib-Strom nach RFC 1950, im Browser
  zu entpacken mit `DecompressionStream('deflate')`.
- **Inhalt:** 128 × 128 Werte, je i16 little-endian, zeilenweise nach z. Ein
  Wert gilt für eine Zelle aus `heightsCell` × `heightsCell` Blockspalten,
  heute 4 × 4. Die Spalte (x, z) liegt in der Zelle an
  ⌊(z − 512·rz)/4⌋·128 + ⌊(x − 512·rx)/4⌋.
- **Wert:** je Zelle der obere Median der obersten Blöcke ihrer Spalten, die
  nicht Luft sind. Die Höhen der Spalten mit Block werden aufsteigend
  sortiert, und es gilt der Wert an der Stelle Anzahl/2, von 0 an gezählt:
  bei 16 Spalten der neunte, bei 3 der zweite. Spalten ohne Block zählen
  nicht mit; hat keine einen Block oder fehlt der Chunk, steht −32768 da.
  - Jeder Block ausser Luft zählt: Wasser und Lava, Laub und Truhen, auch
    Blöcke, die der Renderer nicht zeichnet, wie Barrieren und Licht.
  - Über Wasser nennt die Anzeige deshalb die Oberfläche, nicht den Grund.
- **Quelle:** die Heightmap `WORLD_SURFACE`, die das Spiel ab dem Status
  `carvers` in jedem Chunk speichert, je Spalte das y über dem obersten
  Block, der nicht Luft ist (Client 26.2, per javap). Der Vorlauf liest sie
  mit, `Chunk::surface` in
  [`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs). Fehlt
  sie einem Chunk, rechnet er sie aus den Blöcken, die er ohnehin
  dekodiert. Warum aus ihr, warum je 4×4 und warum über Wasser die
  Oberfläche: [0036](../entscheidungen/0036-hoehen-aus-der-heightmap.md).

Geschrieben werden die Höhen vor der ersten `map.json` eines Laufs; ein
Frontend, das dem Render zusieht, findet sie also mit der ersten Kachel.
Welcher Lauf welche Höhen schreibt:

- **Ein Export über die ganze Welt** schreibt jede Region neu.
- **Ein Ausschnitt** schreibt die Chunks im schrägen Band seiner Kacheln
  neu, die er liest, auch die, deren Blöcke daneben landen. Ein Chunk, den
  es dort nicht gibt, wird leer. Die übrigen Chunks einer Region behalten
  ihre Höhen, wie ihre Kacheln.
- **`--heights DIR`** schreibt Höhen und Felder in einen bestehenden Baum,
  ohne zu rendern, etwa in einen aus einem Stand ohne Höhen. Der Aufruf
  liest die ganze Welt, braucht nur `--world`, nimmt den scale aus
  `map.json` und prüft wie ein Export, ob die Welt zum Baum gehört.
- **`--resume`** schreibt die Höhen neu wie ein Export.
- **`--pyramid`** lässt Höhen und Felder stehen.
- **`--prune`** entfernt am Ende des Laufs die Höhen von Regionen ohne
  Regionsdatei, soweit der Lauf sie läse, wie die Kacheln ohne Chunk. Ohne
  den Schalter bleiben sie stehen.
- **Nicht fertig erzeugte Chunks** übergeht der Vorlauf wie das Rendern,
  ihre Zellen bleiben leer, siehe [Welten und Kennung](welten.md), „Nicht
  fertig erzeugte Chunks“.

Was die Höhen an Platz und Zeit kosten, steht in
[Was ein Lauf kostet](kosten.md), „Dauer“.

Wie das Frontend den Strahl abgeht, was dabei eine Näherung bleibt und
warum kein Puffer je Pixel, steht bei ihm, siehe [Frontend](../frontend.md).

## Radius der Mischung

`--biome-blend` ändert jede gefärbte Kachel, der Radius gehört deshalb zum
Baum wie die nativen Stufen, siehe
[Biomfarben](../renderer/biomfarben.md), „Übergänge zwischen Biomen“:

- Ein Lauf ohne `--biome-blend` in einen bestehenden Baum nimmt den Radius
  aus `biomeBlend`, ein neuer Baum bekommt die Vorgabe 2.
- Ein Lauf mit einem anderen Radius bricht ab, bevor er einen Chunk liest.
  Sonst entstünde mit `--resume` oder beim Nachrendern still ein Baum aus
  zwei Fassungen, mit Kanten an den Kachelgrenzen.
- `--pyramid` behält das Feld.
- Ein Baum aus einem älteren Stand nennt keinen Radius; seine Kacheln
  färben je Zelle aus 4×4×4 Blöcken, ohne Mischung und ohne Zoom. Ein Lauf
  in ihn nimmt dann den Schalter oder die Vorgabe, sagt das und trägt den
  Radius ein. Die alten Kacheln bleiben, wie sie sind; einheitlich wird der
  Baum erst, wenn er ganz neu entsteht.

## Wann sie geschrieben wird

Jeder Export schreibt `map.json` vor seiner ersten Kachel und am Ende,
`--pyramid` bei jedem Aufruf. Die Datei geht dabei jedes Mal ganz auf die
Platte, bevor sie die alte ersetzt: Nach einem Stromausfall steht die alte
oder die neue da, und kein Lauf scheitert an einer halben.
