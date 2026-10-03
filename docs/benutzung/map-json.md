---
title: map.json
description: Die Felder von map.json, Kamera und Projektion samt projektion.json mit Kantenpixeln, die Liste der Bäume trees.json, wann der Export die Dateien schreibt, die Höhen je Region für die Koordinatenanzeige, warum ein Baum seinen Radius der Mischung behält und wie look und lookHash Karte und Cinematic trennen.
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/render/look.rs
  - renderer/src/render/heights.rs
  - renderer/src/world/chunk.rs
  - renderer/src/cli.rs
  - renderer/tests/fixtures/projektion.json
  - web/src/main.ts
---

# `map.json`

`map.json` liegt im Ordner jedes Kachelbaums und sagt dem Frontend, was
es vorfindet: Kachelgrösse, scale, Kamera, Zoomstufen, Pfadmuster, den belegten
Bereich, die Zahl nativer Stufen, den Radius der Mischung der Biomfarben,
die Kennung der Welt, wo die Höhen liegen und ob der Baum die Karte oder
Cinematic zeigt. Welche Bäume unter einer
Wurzel liegen, sagt `trees.json`, siehe „Liste der Bäume“. Der Typ ist
`MapInfo` in
[`renderer/src/render/pyramid.rs`](../../renderer/src/render/pyramid.rs);
das Frontend liest die Datei in `web/src/main.ts`.

## Felder

```json
{
  "tileSize": 256,
  "scale": 32,
  "camera": "2:1",
  "direction": "se",
  "projection": { "azimuth": "diagonal", "u": 16, "v": 8, "y": 16 },
  "minZoom": 0,
  "maxZoom": 10,
  "tiles": "{z}/{x}/{y}.webp",
  "bounds": [-10240, 0, -6144, 4096],
  "nativeLevels": 0,
  "biomeBlend": 2,
  "world": "cb13a94d6c88dae1-6872d5d8ff54db07",
  "heights": "../heights/{x}.{z}.bin",
  "heightsCell": 4,
  "minY": -64,
  "maxY": 319,
  "look": "map"
}
```

| Feld | Inhalt | Mehr |
|---|---|---|
| `tileSize` | Kantenlänge einer Kachel in Pixeln | |
| `scale` | Pixelbreite eines Blocks auf der Basis | [Kamera](../renderer/kamera.md) |
| `camera` | `--camera`, gekürzt, etwa `8:5`, `top` oder `north-45` | „Kamera und Projektion“ unten |
| `direction` | wo die Kamera steht, `--direction`: diagonal `se`, `sw`, `nw` oder `ne`, genordet `s`, `w`, `n` oder `e` | „Kamera und Projektion“ unten |
| `projection` | die Projektion in Pixeln der Basis | „Kamera und Projektion“ unten |
| `minZoom`, `maxZoom` | gröbste und feinste Stufe; `maxZoom` ist die Basis | [Zoomstufen](zoomstufen.md) |
| `tiles` | Pfadmuster der Kacheln | [Kacheln exportieren](kacheln.md) |
| `bounds` | belegter Bereich auf der feinsten Stufe in Pixeln, `[links, oben, rechts, unten]` | |
| `nativeLevels` | Zahl der nativen Stufen | [Zoomstufen](zoomstufen.md), „Native Stufen“ |
| `biomeBlend` | Radius der Mischung der Biomfarben, `--biome-blend` | „Radius der Mischung“ unten |
| `world` | Kennung der Welt und Dimension, oder `null` | [Welten und Kennung](welten.md) |
| `heights` | Pfadmuster der Höhen je Region, relativ zum Baum; fehlt es, hat der Baum keine | „Höhen“ unten |
| `heightsCell` | Kantenlänge einer Zelle der Höhen in Blöcken, heute 4; steht mit `heights` | „Höhen“ unten |
| `minY`, `maxY` | unterster und oberster Block, den der Renderer zeichnet; stehen mit `heights` | „Höhen“ unten |
| `look` | `"map"` die Karte oder `"cinematic"`; fehlt es, die Karte | „Look“ unten |
| `lookHash` | Fingerabdruck der Werte von Cinematic, 16 Hexziffern; nur mit `"cinematic"` | „Look“ unten |

## Kamera und Projektion

`camera`, `direction` und `projection` beschreiben die Kamera des Baums
(`mit_kamera` in [`renderer/src/cli.rs`](../../renderer/src/cli.rs)). Bei
`--camera 8:5` und scale 32, dann bei `--camera north-45` und scale 16:

```json
"camera": "8:5",
"direction": "se",
"projection": { "azimuth": "diagonal", "u": 16, "v": 10, "y": 16 }
```

```json
"camera": "north-45",
"direction": "s",
"projection": { "azimuth": "north", "u": 16, "v": 16, "y": 16 }
```

- **`projection`:** die Zahlen der Projektion in Pixeln der Basis: `u` ist
  h, `v` ist a, `y` ist b, siehe [Kamera](../renderer/kamera.md),
  „Projektion“. `azimuth` ist `diagonal` mit u = x − z und v = x + z oder
  `north` mit u = x und v = z.
- **`direction`:** wo die Kamera steht, `--direction`, siehe die Tabelle
  unten. Fehlt das Feld, gilt die Vorgabe; nennt es eine Richtung, die die
  Kamera nicht kennt, bricht der Lauf ab.
- **Ältere Bäume:** Fehlt `camera`, ist der Baum 2:1.
- **`--pyramid`** behält die drei Felder.
- **Ein Baum, eine Kamera:** siehe [Zoomstufen](zoomstufen.md), „Ein Baum,
  eine Kamera“.
- **Das Frontend** rechnet die Koordinaten aus diesen Feldern, siehe
  [Frontend](../frontend.md), „Koordinaten“.

Eine Richtung dreht die Welt um k Vierteldrehungen, bevor die Kamera der
Vorgabe sie zeichnet (`Richtung` in
[`renderer/src/render/projection.rs`](../../renderer/src/render/projection.rs)):

| k | diagonal: W:H, `top` | genordet: `top-north`, `north-45` | Block (x, z) der Welt liegt im Blick bei | Block (x, z) im Blick liegt in der Welt bei |
|---|---|---|---|---|
| 0 | `se`, Südost, Vorgabe | `s`, Süden, Norden oben, Vorgabe | (x, z) | (x, z) |
| 1 | `sw`, Südwest | `w`, Westen, Osten oben | (z, −x − 1) | (−z − 1, x) |
| 2 | `nw`, Nordwest | `n`, Norden, Süden oben | (−x − 1, −z − 1) | (−x − 1, −z − 1) |
| 3 | `ne`, Nordost | `e`, Osten, Westen oben | (−z − 1, x) | (z, −x − 1) |

- **Eine Formel:** Zeile k ist R(x, z) = (z, −x − 1), k-mal angewandt,
  zurück k-mal (x, z) → (−z − 1, x); y bleibt. Sie gilt ebenso für Chunks
  (`Richtung::in_den_blick` und `Richtung::in_die_welt`).
- **`projection`** gilt im Blick: u und v rechnen mit x und z im Blick.
- **Was im Blick liegt und was in der Welt bleibt:** siehe
  [Richtungen](../renderer/richtungen.md).

Für die Koordinaten rechnet das Frontend die Projektion nach. Damit es
dabei nicht vom Renderer abweicht, liegen Einträge in
[`renderer/tests/fixtures/projektion.json`](../../renderer/tests/fixtures/projektion.json),
je Kamera und scale, aus den anderen Richtungen für 2:1 und `top` bei
scale 32, für `top-north` und `north-45` bei 16. Ein Test des Renderers
hält die Datei aktuell
(`projektion_als_datei_ist_aktuell` in `renderer/tests/heights.rs`), und
das Frontend prüft sein Modell daran. Jeder Eintrag nennt `camera`,
`direction`, `scale`, `block` und `pixel`. `block` steht in
Weltkoordinaten, `pixel` gilt im Blick der Richtung; aus der Vorgabe ist
beides dasselbe. `pixel` meint zweierlei, je nachdem, ob `eben` oder `wand`
dasteht:

| Eintrag | `pixel` | `block` |
|---|---|---|
| ohne `eben` und `wand` | `project_block(in_den_blick(block))`, die Ecke des Blocks im Blick mit den kleinsten Koordinaten | irgendein Block der Welt, auch negativ und bei 2²⁴ |
| mit `eben: 0` | ein Pixel, dessen Mitte bei +0,5 genau auf der Kante zweier Oberseiten liegt | der Block, dessen Oberseite der Renderer dort zeigt |
| mit `wand: "south"` oder `"east"` | ein Pixel, dessen Mitte bei +0,5 genau auf der Kante zweier Seitenflächen übereinander liegt | der Block, dessen Seite der Renderer dort zeigt |

- **Kanten- und Wandpixel** gibt es nur aus der Vorgabe: Im Blick fällt
  die Füllregel aus jeder Richtung gleich, der Block der Welt ist dann der
  gedrehte.
- **Kantenpixel** gibt es für die Kameras, deren Blockkanten Pixelmitten
  treffen: `top` und 1:1 bei scale 32, 5:3 bei scale 30, siehe
  [Kamera](../renderer/kamera.md), „Blockkanten auf Pixelmitten“. Von oben
  gibt es keine Seitenflächen, also auch keine Einträge mit `wand`.
  Genordet liegt keine Kante auf einer Pixelmitte, die Einträge für
  `top-north` und `north-45` sind nur Ecken.
- **Auf Oberseiten** gerendert auf einem ebenen Boden aus Oberseiten bei
  y = 0, also mit `block[1]` = −1, im Schachbrett aus zwei Farben, damit
  jeder Pixel seinen Block verrät (`kantenpixel` in
  `renderer/tests/heights.rs`). Je Kamera zwei: einer auf einer Ostkante,
  zwischen (x, z) und (x + 1, z), einer auf einer Südkante, zwischen
  (x, z) und (x, z + 1). Nach der Füllregel bekommt den ersten der östliche
  Block, den zweiten der nördliche.
- **An Wänden** gerendert an einer Säule aus zwei Blöcken verschiedener
  Farbe bei y = 0 und 1 (`wandpixel`). Je Kamera zwei: einer auf der Kante
  der Südseiten, einer auf der Kante der Ostseiten. Nach der Füllregel
  bekommt den auf der Südseite der obere Block, den auf der Ostseite der
  untere: Die Kante zwischen den Südseiten liegt für die obere Fläche links,
  die zwischen den Ostseiten rechts.

## Liste der Bäume

`--tiles` ist die Wurzel. Jeder Baum liegt darunter in seinem Ordner
`<kamera>-<richtung>`, die Kamera mit `x` statt `:`, etwa `2x1-se`,
`8x5-se` oder `top-north-s`; daneben liegen `trees.json` und die Höhen,
die alle Bäume teilen. Entschieden in
[0054](../entscheidungen/0054-baeume-unter-einer-wurzel.md).

```json
{
  "trees": [
    { "path": "2x1-se", "camera": "2:1", "direction": "se", "look": "map" },
    { "path": "2x1-se-cinematic", "camera": "2:1", "direction": "se", "look": "cinematic" },
    { "path": "top-north-s", "camera": "top-north", "direction": "s", "look": "map" }
  ]
}
```

- **Felder:** je Baum `path` relativ zu `trees.json`, `camera`,
  `direction` und `look` wie in seiner `map.json`, ohne `look` dort `map`.
  Ein Baum mit Cinematic liegt im Ordner mit dem Anhang `-cinematic`, etwa
  `2x1-se-cinematic`, siehe
  [0054](../entscheidungen/0054-baeume-unter-einer-wurzel.md) und „Look“.
  Projektion, Zoomstufen und Bereich stehen nur in der `map.json` des
  Baums.
- **Reihenfolge:** `2x1-se` zuerst, wenn es den Baum gibt, sonst nach
  `path`. Der erste ist die Vorgabe des Frontends.
- **Woher:** Der Lauf liest die Liste aus der Platte, je Ordner unter der
  Wurzel mit `map.json` ein Eintrag, und führt sie nicht fort. So stimmt
  sie auch nach einem gelöschten Baum (`schreibe_baeume` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs)). Ein Ordner, dessen
  `map.json` sich nicht lesen lässt oder eine Kamera, eine Richtung oder
  einen `look` nennt, die es nicht gibt, fehlt in der Liste. Der Lauf
  meldet ihn als „übergangen“ und scheitert nicht an ihm; das Frontend
  könnte ihn ohnehin nicht öffnen.
- **Eine Wurzel, eine Welt und Dimension:** Die Bäume einer Wurzel teilen
  sich die Höhen. Bevor ein Lauf einen Chunk liest, prüft er deshalb jeden
  Baum daneben mit dessen eigener Kennung, siehe
  [Welten und Kennung](welten.md). Gehört einer zu einer anderen Welt oder
  Dimension, bricht er ab und rät zu einer neuen Wurzel. Ein Baum aus einem
  Stand ohne Kennung zählt nicht. `--heights` prüft ebenso, wenn der Baum
  unter einer Wurzel mit `trees.json` liegt.
- **Wann:** direkt nach der ersten `map.json` eines Laufs und am Ende,
  jedes Mal über eine eigene Datei, die die alte ersetzt. Ein neuer Baum
  lässt sich so schon während seines ersten Laufs wählen.
- **Alte Ablage:** Liegt `map.json` direkt unter `--tiles`, ist das ein
  Baum aus einem Stand vor #68. Der Lauf bricht dann ab, bevor er die
  Ausnahme im Echtzeitschutz setzt, Assets oder Welt liest, und nennt den
  Ordner, in den der Baum gehört; er deutet ihn nicht um und verschiebt
  nichts. Weiter geht es so: alles ausser `heights/` in den genannten
  Ordner verschieben, `heights/` bleibt in der Wurzel, wo alle Bäume sie
  lesen. Bis zum nächsten Lauf zeigt das Frontend für den Baum Striche
  statt Koordinaten, denn seine `map.json` sucht die Höhen noch in seinem
  eigenen Ordner, siehe [Frontend](../frontend.md), „Koordinaten“. Der nächste Lauf schreibt sie neu, mit `../heights/{x}.{z}.bin`.
  `--pyramid` nimmt weiter jeden Baum, auch einen der alten Ablage.
- **Ein Baum statt der Wurzel:** Ist `--tiles` der Ordner eines Baums unter
  einer Wurzel, zu erkennen an der `trees.json` daneben oder an Höhen unter
  `../`, bricht der Lauf ebenso früh ab und nennt die Wurzel (`pruefe_wurzel`
  in [`renderer/src/cli.rs`](../../renderer/src/cli.rs)).
- **Der scale steht nicht im Namen:** Ein zweiter scale derselben Kamera
  und Richtung braucht eine eigene Wurzel. Im selben Ordner bricht der Lauf
  ab, bevor er einen Chunk liest, siehe [Zoomstufen](zoomstufen.md), „Ein
  Baum, eine Welt“.

## Höhen

Das Frontend zeigt unter Maus und Finger die Koordinaten des Blocks, siehe
[Frontend](../frontend.md), „Koordinaten“. Dafür braucht es je Zelle eine
Höhe. Die liefert der Renderer:

- **Datei:** je Region `heights/{x}.{z}.bin` unter der Wurzel, die alle
  Bäume einer Welt teilen; `map.json` nennt sie als
  `../heights/{x}.{z}.bin`. Ein Baum der alten Ablage hat sie in seinem
  eigenen Ordner, `heights/{x}.{z}.bin`. x und z wie in `r.x.z.mca`. Darin
  steht ein zlib-Strom nach RFC 1950, im Browser zu entpacken mit
  `DecompressionStream('deflate')`.
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
  ohne zu rendern, etwa in einen aus einem Stand ohne Höhen. `DIR` ist der
  Ordner des Baums, auch als `.`. Liegt er unter einer Wurzel mit
  `trees.json`, landen die Höhen dort, sonst in ihm selbst. Der Aufruf
  liest die ganze Welt, braucht nur `--world`, nimmt scale, Kamera und
  Richtung aus `map.json`, nimmt deshalb weder `--scale` noch `--camera`
  noch `--direction` an und prüft wie ein Export, ob die Welt zum Baum und
  zu den Bäumen daneben gehört.
- **`--resume`** schreibt die Höhen neu wie ein Export.
- **`--pyramid`** lässt Höhen und Felder stehen.
- **`--prune`** entfernt am Ende des Laufs die Höhen von Regionen ohne
  Regionsdatei, soweit der Lauf sie läse, wie die Kacheln ohne Chunk. Ohne
  den Schalter bleiben sie stehen.
- **Nicht fertig erzeugte Chunks** übergeht der Vorlauf wie das Rendern,
  ihre Zellen bleiben leer, siehe [Welten und Kennung](welten.md), „Nicht
  fertig erzeugte Chunks“.
- **Nacheinander, nicht gleichzeitig:** Zwei Läufe zugleich in Bäume
  derselben Wurzel gehen nicht. Mit `--size` liest jeder eine Datei der
  Höhen, ändert seine Chunks und schreibt sie zurück; der spätere
  überschreibt, was der frühere geändert hat. Die Bäume einer Wurzel
  rendert man nacheinander.

Was die Höhen an Platz und Zeit kosten, steht in
[Was ein Lauf kostet](kosten.md), „Dauer“.

Wie das Frontend den Strahl abgeht, steht bei ihm, siehe
[Frontend](../frontend.md), „Koordinaten“; was dabei eine Näherung bleibt,
siehe [Frontend](../frontend.md), „Was bleibt eine Näherung“; warum kein
Puffer je Pixel, siehe
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

## Look

`look` sagt, wie der Baum zeichnet: `"map"` die Karte, `"cinematic"` mit
`--cinematic`, siehe [Cinematic](../renderer/cinematic.md). Jeder Export
schreibt es, auch für die Karte. Fehlt es, stammt der Baum aus einem Stand
vor #72 und zeigt die Karte. Einen anderen Wert nimmt kein Lauf an.

- **`lookHash`** steht nur mit `"cinematic"`: der Fingerabdruck der Werte
  des Looks (`Look::fingerabdruck` in
  [`renderer/src/render/look.rs`](../../renderer/src/render/look.rs)).
  - FNV-1a mit 64 Bit: Startwert `0xcbf29ce484222325`, Faktor
    `0x100000001b3`.
  - Darüber je Wert, in der Reihenfolge von `Look::werte`, sein Name in
    UTF-8, ein Nullbyte und die Bits jeder Zahl als f32 in Little Endian,
    bei einer Farbe drei Zahlen.
  - Zuletzt `verfahren`, ein Nullbyte und `VERFAHREN` als u32 in Little
    Endian: der Stand des Verfahrens. Ändert sich das Bild bei gleichen
    Werten, etwa mit der Sonne in #73, steigt er, und mit ihm der
    Fingerabdruck.
  - `Look::werte` zerlegt `Look` ganz: Ein neues Feld kompiliert erst, wenn
    es im Fingerabdruck steht.
  - Nur die Werte, wie sie im Code stehen. Abgeleitete wie der Sinus der
    Höhe der Sonne fehlen, deren letztes Bit kann je System abweichen.
  - Geschrieben als 16 kleine Hexziffern; für die Werte aus
    [0058](../entscheidungen/0058-look-von-cinematic.md)
    `8747842880fab7c7`, mit `VERFAHREN` 2.
- **Ein Baum, ein look:** Ein Lauf mit dem anderen look schreibt in einen
  anderen Ordner, siehe „Liste der Bäume“. Er bricht nur ab, wenn in seinem
  Ordner ein Baum mit dem anderen look oder mit anderen Werten liegt, bevor
  er einen Chunk liest, auch mit `--resume`, siehe
  [Zoomstufen](zoomstufen.md), „Ein Baum, ein look“.
- **`--pyramid`** behält beide Felder.

## Wann sie geschrieben wird

Jeder Export schreibt `map.json` vor seiner ersten Kachel und am Ende,
danach jeweils `trees.json`; `--pyramid` schreibt `map.json` bei jedem
Aufruf. Die Datei geht dabei jedes Mal ganz auf die Platte, bevor sie die
alte ersetzt: Nach einem Stromausfall steht die alte oder die neue da, und
kein Lauf scheitert an einer halben.
