---
title: map.json
description: Die Felder von map.json, wann der Export die Datei schreibt, die Höhen je Region für die Koordinatenanzeige, wie das Frontend die Projektion nachrechnet und warum ein Baum seinen Radius der Mischung behält.
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/heights.rs
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
| `minY`, `maxY` | unterster und oberster Block, den der Renderer zeichnet; stehen mit `heights` | „Höhen“ unten |

Die Projektion selbst steht nicht drin: sie hängt allein an `scale`, die
Formel steht in [Kamera](../renderer/kamera.md), „Projektion“. Für die
Koordinaten rechnet das Frontend sie nach. Damit es dabei nicht vom Renderer
abweicht, liegen Paare aus Block und Bildpunkt in
[`renderer/tests/fixtures/projektion.json`](../../renderer/tests/fixtures/projektion.json).
Ein Test des Renderers hält die Datei aktuell, und das Frontend prüft sein
Modell daran.

## Höhen

Das Frontend zeigt unter Maus und Finger die Koordinaten des Blocks, auf
den das Spiel zielen würde, siehe [Frontend](../frontend.md), „Koordinaten“.
Dafür braucht es je Spalte die Höhe des obersten Blocks. Die liefert der
Renderer:

- **Datei:** je Region `heights/{x}.{z}.bin` neben den Kacheln, x und z
  wie in `r.x.z.mca`. Darin steht ein zlib-Strom nach RFC 1950, im Browser
  zu entpacken mit `DecompressionStream('deflate')`.
- **Inhalt:** 512 × 512 Werte, je i16 little-endian, zeilenweise nach z.
  Die Spalte (x, z) steht an (z − 512·rz)·512 + (x − 512·rx).
- **Wert:** das y des obersten Blocks zwischen `minY` und `maxY`, der ein
  Sprite bekommt und nicht nur Flüssigkeit ist; −32768, wenn es keinen
  gibt oder der Chunk fehlt.
  - Luft, Licht, Barrieren und Blöcke ohne Geometrie wie Truhen zählen
    nicht.
  - Wasser, Lava und Blasensäulen zählen nicht, eine geflutete Truhe auch
    nicht: von ihr zeichnet der Renderer nur das Wasser.
  - Laub, Blumen und geflutete Blöcke mit Modell zählen, ein gefluteter
    Zaun also mit seinem Pfosten.

Das Spiel zielt genauso durch Flüssigkeiten hindurch. Am Client 26.2 per
javap: `LocalPlayer.pick(Entity, double, double, float)` ruft
`Entity.pick(d, f, false)` auf, und das baut
`ClipContext(…, Block.OUTLINE, Fluid.NONE, …)`.

Die Höhen entstehen in einem eigenen Durchgang durch die Welt, nach der
Sprite-Tabelle: erst sie weiss, welcher Block ein Sprite bekommt, und sie
entsteht aus dem Vorlauf. Geschrieben werden sie vor der ersten `map.json`
eines Laufs, ein Frontend, das dem Render zusieht, findet sie also mit der
ersten Kachel. Welcher Lauf welche Höhen schreibt:

- **Ein Export über die ganze Welt** schreibt jede Region neu.
- **Ein Ausschnitt** schreibt die Chunks im schrägen Band seiner Kacheln
  neu, deren Blöcke im Ausschnitt landen können. Ein Chunk, den es dort
  nicht gibt, wird leer. Die übrigen Chunks einer Region behalten ihre
  Höhen, wie ihre Kacheln: von einem Chunk, dessen Blöcke ausserhalb
  landen, kennt die Sprite-Tabelle des Ausschnitts nicht jeden Block.
- **`--heights DIR`** schreibt Höhen und Felder in einen bestehenden Baum,
  ohne zu rendern, etwa in einen aus einem Stand ohne Höhen. Der Aufruf
  liest die ganze Welt, braucht `--world` und `--assets`, nimmt den scale
  aus `map.json` und prüft wie ein Export, ob die Welt zum Baum gehört.
- **`--resume`** schreibt die Höhen neu wie ein Export.
- **`--pyramid`** lässt Höhen und Felder stehen.
- **`--prune`** entfernt am Ende des Laufs die Höhen von Regionen ohne
  Regionsdatei, soweit der Lauf sie läse, wie die Kacheln ohne Chunk. Ohne
  den Schalter bleiben sie stehen.
- **Unfertige Chunks** liest der Durchgang wie das Rendern.

Die Höhen einer Region sind gepackt 70 bis 100 kB. Auf der grossen Welt
sind das zusammen 249 MB neben 184 GB Kacheln, und der Durchgang braucht
knapp eine Minute, gemessen in
[2026-09-28, Höhen](../messungen/2026-09-28-hoehen.md).

Was die Anzeige damit nähert, steht bei
[Frontend](../frontend.md), „Was bleibt eine Näherung“. Warum Höhen und
kein Puffer je Basiskachel:
[0035](../entscheidungen/0035-koordinaten-aus-hoehenkarten.md).

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
