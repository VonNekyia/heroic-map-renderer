# TerraNova Map Renderer

Isometrischer Offline-Renderer für Minecraft-Java-Welten. Liest Weltdaten und
ein Resourcepack, rendert die Welt mit fester isometrischer Kamera und gibt
WebP-Rastertiles aus, die ein schlankes Leaflet-Frontend anzeigt.

```
Minecraft World + Resource Pack  ->  Rust Renderer  ->  WebP Tiles  ->  Leaflet
```

Der Browser rendert keine Minecraft-Geometrie, sondern nur fertige Rasterkacheln.

## Stand

Schritt 8 von 8: **Modelle und Transparenz im Detail**. Wasser und Lava
werden gezeichnet, Gras, Laub und Wasser bekommen die Farbe ihres Bioms,
`uvlock` hält Texturen an der Welt fest, durchsichtige Flächen mischen
sich im Sprite statt zu überschreiben, und Blöcke mit mehreren Varianten —
Sand, Stein, Erde — würfeln ihre Drehung aus der Position wie das Spiel.

![Karte](docs/map.png)

900 mal 900 Pixel um (-64, 416), scale 16, 390 Chunks, 1,4 s —
dieselbe Stelle wie in Schritt 4, jetzt mit Wasser, Biomfarben und
gewürfelten Drehungen.

| Schritt | Inhalt | Status |
|---------|--------|--------|
| 1 | Welt-Reader (Region, Chunk, Palette) | **fertig** |
| 2 | Resourcepack: Blockstates, Models, Texturen | **fertig** |
| 3 | Model-Baking und Iso-Sprite-Rasterizer | **fertig** |
| 4 | Metatile-Renderer | **fertig** |
| 5 | Rayon-Parallelisierung, Tiles, WebP | **fertig** |
| 6 | Zoom-Pyramide und `map.json` | **fertig** |
| 7 | Frontend (Vite, TypeScript, Leaflet) | **fertig** |
| 8 | Modelle und Transparenz im Detail | **fertig** |

## Assets besorgen

Der Renderer braucht einen vollständigen Asset-Baum. Ein Overlay-Pack allein
reicht nicht: das TerraNova-Pack bringt 39 von 1198 Blockstates mit und keine
Colormaps. Die Basis kommt aus dem Client-JAR der Version, die der Server
fährt (hier 26.2):

```powershell
$v = "26.2"; $m = Get-Content "$env:APPDATA\.minecraft\versions\$v\$v.json" | ConvertFrom-Json; Invoke-WebRequest $m.downloads.client.url -OutFile "$env:TEMP\mc.zip"; Expand-Archive "$env:TEMP\mc.zip" "$env:TEMP\mc" -Force; Move-Item "$env:TEMP\mc\assets" vanilla-assets
```

Die SHA1-Prüfsumme steht im selben Manifest unter `downloads.client.sha1`.

Danach wird der Baum gestapelt übergeben, spätere Wurzeln gewinnen:

```bash
--assets ./vanilla-assets --assets ./assets
```

Für die Färbung von Gras, Laub und Wasser braucht der Renderer ausserdem
die Biomdefinitionen. Sie liegen im selben JAR unter `data/`, nicht unter
`assets/`:

```powershell
New-Item -ItemType Directory -Force vanilla-data\minecraft\worldgen | Out-Null; Move-Item "$env:TEMP\mc\data\minecraft\worldgen\biome" vanilla-data\minecraft\worldgen\biome
```

```bash
--data ./vanilla-data
```

66 Dateien, 352 kB. Ohne `--data` bekommt jeder Block die Farben von
`plains`; Ozeane und Wälder sehen dann überall gleich aus, aber nicht
falsch. Datenpakete mit eigenen Biomen kommen als weitere Wurzeln dazu,
spätere überschreiben frühere — auch Vanilla-Biome, die ein Paket
umdefiniert:

```bash
--data ./vanilla-data --data ./terralith-data
```

Erwartet wird `<DIR>/<namespace>/worldgen/biome/**/*.json`; Unterordner
gehören zur ID, `terralith:cave/underground_jungle` liegt unter
`biome/cave/underground_jungle.json`. Kommt in der Welt ein Biom vor, für
das keine Definition geladen ist, sagt der Renderer es beim Start und
färbt es wie `plains`.

Was er aus einem Biom braucht, liest der Renderer wie `Biome.DIRECT_CODEC`
in 26.2: Pflicht sind `has_precipitation`, `temperature`, `downfall`,
`effects` und darin `water_color`. Eine Farbe darf wie im Client eine
ganze Zahl sein, `#rrggbb` oder drei Kommazahlen von 0 bis 1 wie
`[0.2, 0.4, 0.8]`. Ein Biom, das der Codec ablehnt, übergeht der Renderer
und nennt es beim Start; der Client lüde sein Datenpaket gar nicht.

## Benutzung

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --at -64 72 416
```

```
Welt:       ./world
Regionen:   ./world\dimensions\minecraft\overworld\region
            383 Dateien, x -26..22, z -21..20

Chunk:      (-4, 26)  status=minecraft:full
            DataVersion 4903
            24 Sections, y -64..319

Block bei (-64, 72, 416):  minecraft:air
Höchster Block in Spalte:  y=71  minecraft:oak_leaves[distance=2,persistent=false,waterlogged=false]
Biom:                      minecraft:forest
```

Der Renderer liest Welten ab Minecraft 26.1, mit den Regionen unter
`world/dimensions/<namensraum>/<name>/region`. Eine ältere Welt, etwa aus 1.21
mit `world/region` und `DIM-1`, vorher mit dem Server von Minecraft 26.2 und
`--forceUpgrade` hochziehen: er baut Verzeichnisse, Seed und Chunks um, bevor
er startet. Sonst kennt der Renderer ihren Seed nicht und manche ihrer
Blocknamen nicht, und ein Block ohne Asset bricht den Lauf vor der ersten
Kachel ab.

Eine Blockstate direkt auflösen:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --assets ./vanilla-assets --assets ./assets --block "oak_fence[north=true,east=true]"
```

```
minecraft:oak_fence[east=true,north=true]
  minecraft:block/oak_fence_post
      1 Elemente, 6 Flächen
      block/oak_fence
  minecraft:block/oak_fence_side
      2 Elemente, 12 Flächen
      block/oak_fence_planks
  minecraft:block/oak_fence_side y=90
      2 Elemente, 12 Flächen
      block/oak_fence_planks
```

Einzelne Blockstates als Sprites rastern, hier die zwanzig aus dem Bild:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --assets ./vanilla-assets --assets ./assets --scale 64 --sprite docs/sprites.png \
  --block "stone" --block "grass_block[snowy=false]" --block "oak_log[axis=y]" --block "crafting_table" --block "glass" \
  --block "furnace[facing=north,lit=false]" --block "furnace[facing=east,lit=false]" --block "furnace[facing=south,lit=false]" --block "furnace[facing=west,lit=false]" --block "torch" \
  --block "oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]" --block "oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]" \
  --block "oak_slab[type=bottom,waterlogged=false]" --block "oak_fence[north=true,east=true,south=false,west=false,waterlogged=false]" \
  --block "cobblestone_wall[east=none,north=low,south=low,up=true,waterlogged=false,west=none]" \
  --block "oak_door[facing=east,half=lower,hinge=left,open=false,powered=false]" --block "lily_pad" \
  --block "oak_leaves[distance=1,persistent=false,waterlogged=false]" --block "short_grass" \
  --block "oak_hanging_sign[attached=false,rotation=3,waterlogged=false]"
```

![Sprites](docs/sprites.png)

Einen Weltausschnitt rendern:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render docs/map.png --center -64 416 --size 900 --scale 16
```

```
Render:     390 Chunks gelesen, 215 Blockstates, 1207 Sprites
            1 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
            900x900 px bei (-4290, 958) und scale 16 in 1.4 s -> docs/map.png

Texturen:   92 geladen, 0 fehlen
```

`--center` nennt die Blockspalte, die in der Bildmitte landet, `--scale` die
Pixelbreite eines Blocks, `--size` die Kantenlänge, mindestens 1. Die
Ausgabe nennt die linke obere Bildecke in Pixeln. Die Sprite-Tabelle kommt
aus demselben Vorlauf wie beim Kachelexport, nur über den Ausschnitt, und
der dekodiert nur, was im Bild landen kann: der sichtbare Bereich ist ein
schmales diagonales Band in x und z, kein Rechteck. Wer stattdessen die
Hüllbox nähme, läse für einen 1024er Ausschnitt rund das Sechzehnfache an
Chunks.

### Die ganze Welt als Kacheln

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles
```

```
Vorlauf:    316223 Chunks in 5.5 s, 3110 Blockstates, 292836 Kacheln
            33762 Sprites bei scale 32, davon 31265 Fassungen
            18 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
            200/292836 Kacheln
            400/292836 Kacheln
```

Danach stapelt der Lauf die gröberen Zoomstufen darüber und schreibt
`map.json`.

Der Vorlauf liest jeden Chunk einmal und beantwortet zwei Fragen auf einmal:
welche Blockstates vorkommen, und welche Kacheln überhaupt etwas zeigen. Erst
danach steht die Sprite-Tabelle — und erst dann kann parallel gerendert
werden, denn sonst müsste jeder Worker sie unter einer Sperre füllen. Die
Chunks werden deshalb mehrmals gelesen: vom Vorlauf, von der Basis und von
jeder nativen Stufe, bei scale 32 mit allen dreien also fünfmal. Der
Vorlauf kostet für die ganze Welt 5 bis 11 Sekunden. Eine Fassung ist jedes
Sprite, das nicht selbst Alternative einer Blockstate ist: eines je Maske
verdeckter Flüssigkeitsflächen, je Tiefe dahinter und je Biomfarbe, dazu
die Streifen an Wasserstufen.

Gerendert wird mit Rayon über die Kacheln. Geteilt wird nur die
unveränderliche Sprite-Tabelle; jede Kachel legt sich ihren Chunk- und
Regionscache neu an. Benachbarte Kacheln dekodieren dieselben Chunks also
mehrfach — das ist der grösste Kostenblock des Renders, siehe unten.

`--center` und `--size` schränken auf einen Ausschnitt ein:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles --center -64 416 --size 2048 --native-levels 3
```

```
Vorlauf:    788 Chunks in 0.1 s, 247 Blockstates, 256 Kacheln
            1580 Sprites bei scale 32, davon 1336 Fassungen
            1 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
            200/256 Kacheln
            256/256 Kacheln
Kacheln:    256 geschrieben, 0 leer, 256x256 px, 24 Threads
            31.2 MB in 2.2 s (119 Kacheln/s, 125 kB je Kachel)
Zoom  9:     64 Kacheln nativ bei scale 16, 8.0 MB in 1.2 s
Zoom  8:     16 Kacheln nativ bei scale 8, 1.9 MB in 0.7 s
Zoom  7:     4 Kacheln nativ bei scale 4, 0.5 MB in 1.2 s
Zoom  6:     2 Kacheln
...
Pyramide:   9 Kacheln, 0.2 MB in 0.0 s
Karte:      Zoom 0..10, 256 Basiskacheln, -10240/0 bis -6144/4096 px -> ./tiles/map.json
```

Der Ausschnitt wird dabei aufgerundet, bevor der Vorlauf irgendetwas
ausschliesst, und zwar auf ganze Kacheln der gröbsten nativen Stufe — mit
drei Stufen bei scale 32 auf 2048 Pixel, aus 2048 mal 2048 werden hier 4096
mal 4096. Die nativen Stufen zeigen ganze Elternkacheln, und alle Stufen
sollen denselben Stand der Welt zeigen: sonst stünde ein Neubau neben dem
Ausschnitt nur auf den gröberen. Umgekehrt sammelt der Vorlauf Blockstates
nur aus Chunks, die tatsächlich in eine ausgegebene Kachel fallen, und was
die gerundete Fläche gar nicht berühren kann, dekodiert er nicht einmal:
hier 788 Chunks statt der 8192 aller Regionen, die sie schneiden. Ein
kleiner Ausschnitt braucht deshalb keine Assets für Blöcke am anderen Ende
der Welt; fehlt eines in seiner Fläche, bricht der Lauf ab, bevor er die
erste Kachel schreibt.

Die Kacheln liegen als `tiles/<z>/<x>/<y>.webp`; x und y dürfen negativ sein,
weil der Blockursprung mitten in der Welt liegt. Wird eine Kachel bei einem
erneuten Lauf leer, löscht der Export die alte Datei — auf jeder Stufe, sonst
zeigte die Karte weiter, was inzwischen abgerissen wurde. Nur eine native
Elternkachel, unter der eine Kachel stehen bleibt, bleibt durchsichtig
stehen, siehe unten.

Eine Basiskachel, die gar kein Chunk mehr berührt, weil ein Editor ihn
zurückgesetzt hat, entfernt der Export nur mit `--prune`, dann auf jeder
Stufe. Bis zum Ende der Pyramide läuft ein Lauf mit dem Schalter wie einer
ohne ihn; erst dann nimmt er diese Kacheln heraus und setzt die Stufen über
ihnen ohne sie neu zusammen. Ohne den Schalter zählt er sie und lässt sie
stehen; nur wo der Lauf eine native Elternkachel ohnehin neu rendert, fehlt
dort schon, was sie zeigen. Die Elternkachel bleibt dann durchsichtig
stehen, damit keine Kachel ohne Eltern dasteht. Einer Teilkopie der Welt
fehlt vieles, und ein Lauf mit `--prune` leerte über ihr den Baum: der
Schalter gehört nur an die vollständige Welt. Der Lauf nennt deshalb vor der
ersten Kachel, wie viele Kacheln es trifft, von wie vielen. Ein Ausschnitt
sucht nur in seiner gerundeten Fläche. Mit `--prune` läuft er auch dann,
wenn der Vorlauf dort gar nichts mehr findet, und auch, wenn dort schon
aufgeräumt ist.

Entfernt wird erst am Ende des Laufs, auf allen Stufen, auch was nur leer
geworden ist, von der gröbsten Stufe bis zur Basis. Bis dahin zeigt eine
Kachel, die beim Rendern oder in der Pyramide leer geworden ist, schon
nichts mehr, der Lauf überschreibt sie durchsichtig. Bricht er vorher ab,
hat er nichts gelöscht, und auch ein späterer Ausschnitt holt nichts
Abgerissenes in eine Elternkachel zurück. Über den Kacheln ohne Chunk hat
ein Lauf mit `--prune` bis zum Ende der Pyramide nur verändert, was auch ein
Lauf ohne ihn verändert hätte. Danach setzt er die verkleinerten Stufen über
ihnen ohne sie neu zusammen. Was dabei leer wird, entfernt er erst am Ende,
mit diesen Kacheln und ihren nativen Vorfahren, unter denen nichts bleibt.
Bricht er dazwischen ab, zeigen die neu zusammengesetzten Kacheln schon den
aufgeräumten Stand. Die Basis, native Kacheln, die dieser Lauf nicht
gerendert hat, und was leer geworden ist, zeigen noch den alten. Ein Lauf
mit `--prune` über dieselbe Fläche bringt den Baum in Ordnung, einer ohne
den Schalter nicht immer. Bricht er beim Entfernen ab, fehlen feineren
Kacheln die Eltern. Jeder Lauf sucht solche Kacheln, soweit sie seine Fläche
berühren, und baut ihnen die Eltern neu, auch einer, dessen Vorlauf dort
nichts mehr findet; einer mit `--prune` räumt dann auch die Kacheln ohne
Chunk weg.

### Zoomstufen

Gröbere Stufen entstehen aus vier Kacheln der darunterliegenden, auf die
halbe Kantenlänge gestaucht — die Welt wird dafür kein zweites Mal
angefasst. Ausgenommen sind native Stufen direkt unter der Basis, wenn
`--native-levels` sie verlangt, siehe unten.

![Zoomstufen](docs/zoomstufen.png)

Gemittelt wird mit vormultipliziertem Alpha. Geradeaus gemittelt zögen
durchsichtige Pixel ihre Farbe in die Nachbarn, und jede Kante gegen Luft
bekäme einen dunklen Saum — auf einer Karte voller Blattwerk wäre das überall
zu sehen.

Die Nummerierung hängt an der **Welt**, nicht am Ausschnitt: `maxZoom` kommt
beim ersten Lauf aus der Ausdehnung aller Regionsdateien, und dafür wird kein
einziger Chunk gelesen. Ein bestehender Baum behält sie. Zoom 0 hat dabei
ohnehin bis zu vier Kacheln: das Stapeln endet an den vier Kacheln um den
Ursprung, sie sind ihre eigenen Eltern. Wächst die Welt über eine
Zweierpotenz an Kacheln hinaus, bleibt die Basis auf ihrer Stufe, und
Zoom 0 bekommt mehr — sonst müsste der ganze Baum nach der ersten neuen
Region von vorn entstehen. Passt Zoom 0 dann nicht mehr ins Fenster, zoomt
das Frontend darunter weiter heraus.

Ein nachgerenderter Ausschnitt passt damit in einen bestehenden Kachelbaum.
Welche Kinder in eine Elternkachel gehören, entscheidet dabei die Platte und
nicht der laufende Export: die Geschwister ausserhalb des Ausschnitts liegen
ja weiterhin da. Und `map.json` beschreibt den ganzen Baum, nicht den letzten
Lauf. An einer unveränderten Welt ändert ein Nachrendern deshalb keine einzige
Datei.

Dafür müssen Welt und Massstab passen. Weicht die Kennung der Welt oder
`scale` vom `map.json` im Zielverzeichnis ab, bricht der Export ab, bevor
er einen Chunk liest; sonst lägen im Baum Kacheln zweier Welten oder zweier
Massstäbe. `map.json` entsteht deshalb direkt vor der ersten Kachel und am
Ende noch einmal: bricht ein Lauf beim Schreiben ab, steht schon fest, wozu
der Baum gehört, und scheitert er vorher, etwa an einem fehlenden Asset,
legt er nichts fest. Ein Baum eines älteren Stands, dessen `map.json` gar
kein Feld `world` hat, gehört ab dem nächsten Lauf zu dessen Welt, der
Lauf sagt es. Eine Welt ohne Kennung übernimmt ihn nicht, sonst nähme er
danach seine eigene nicht mehr auf. Einer mit scale 2, 6 oder 10 lässt
sich nicht fortsetzen, `--scale` nimmt nur noch Vielfache von 4.

Gemittelt wird in linearem Licht, nicht in sRGB-Werten: die sind
gammakodiert, ihr Mittel ist zu dunkel, und jede Stufe verdunkelt weiter.
Halb Schwarz, halb Weiss ergibt so 188 statt 128.

Verkleinern mittelt trotzdem Nachbarblöcke ineinander; zwei Stufen unter
der Basis ist ein Block noch acht Pixel breit, und Blockkanten werden zu
Verläufen. Wer die Kanten länger scharf haben will, lässt mit
`--native-levels N` die ersten N gröberen Stufen aus der Welt rendern, mit
Sprites in dieser Grösse. Ein nativer Render hält den Umriss jedes Blocks
scharf und mittelt stattdessen die Textur über den Block, was auf einer
Karte niemand vermisst. Das geht, solange jeder Block auf ganzen Pixeln
liegt, der scale der Stufe also durch vier teilbar ist: bei scale 32 drei
Stufen lang, 16, 8 und 4. Bei scale 2 läge jede zweite Blockreihe auf
einem halben Pixel, und benachbarte Reihen überdeckten sich; aus demselben
Grund nimmt `--scale` nur Vielfache von 4. Der Preis ist hoch: Mit allen
drei Stufen kommt bei scale 32 in Bytes ein Drittel dazu, in Zeit fast
noch einmal die Basis, denn jede Stufe zeichnet jeden Block ihrer Fläche
erneut; siehe unten. Deshalb ist die Vorgabe 0.

Die Zahl gehört zum Baum wie der scale: `map.json` hält sie als
`nativeLevels` fest. Ein Lauf ohne `--native-levels` nimmt sie von dort,
einer mit einer anderen bricht ab, bevor er einen Chunk liest. Sonst lägen
über einem nachgerenderten Ausschnitt verkleinerte Kacheln neben nativen,
und an einer unveränderten Welt änderte ein Nachrendern Dateien. Mehr, als
der scale hergibt, heisst alle. Nennt die `map.json` eines Baums aus einem
älteren Stand die Zahl nicht, bricht ein Lauf ohne den Schalter ab und fragt
nach ihr: der Stand davor renderte alle Stufen nativ, die der scale hergibt,
und mit 0 lägen über dem Ausschnitt verkleinerte Kacheln neben nativen. Ein
Lauf mit dem Schalter hält die Zahl fest.

Ein Ausschnitt mit `--size` braucht mit nativen Stufen mehr Welt als sich
selbst: eine native Elternkachel zeigt auch, was neben dem Ausschnitt
liegt. Der Export rundet ihn deshalb auf ganze Kacheln der gröbsten
nativen Stufe auf, siehe oben.

#### Pyramide nachbauen, Karte während des Renders ansehen

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --pyramid ./tiles
```

`--pyramid` rendert nichts und braucht weder Welt noch Assets. Es baut die
gröberen Stufen und `map.json` aus den Basiskacheln, die auf der Platte
liegen. Basisstufe, scale und Welt nennt `map.json`, das jeder Export vor
seiner ersten Kachel schreibt; ohne diese Datei, oder wenn auf ihrer
Basisstufe keine Kachel liegt, ändert es nichts. Native Stufen rendert es
nicht, es verkleinert auch dort. Ein laufender Render ersetzt sie am Ende
durch native.

Neu gebaut wird nur, was sich geändert hat: eine Kachel, unter der ein
Kind jünger ist als sie oder in diesem Aufruf neu gebaut oder entfernt
wurde, und eine, die fehlt. Eine Kachel ohne Kinder verschwindet.
Verglichen wird auf jeder Stufe, ein abgebrochener Aufruf heilt also im
nächsten. Die Zeiten stehen im Verzeichnis, eine Abfrage je Kachel
braucht es nicht. Der Aufruf lässt sich deshalb wiederholen, während ein
Vollrender noch Stunden läuft: die Karte im Browser zeigt, was fertig ist,
und wächst mit jedem Aufruf.

Jede Kachel, die `--pyramid` schreibt, trägt als Zeit den Beginn des
Aufrufs, zwei Sekunden früher. Ein Kind, das der Render währenddessen
fertigstellt, ist so jünger als seine Elternkachel, und der nächste
Aufruf holt es. Zwei Sekunden, weil keine gängige Uhr eines Dateisystems
gröber zählt; eine Kachel aus diesen zwei Sekunden baut der nächste
Aufruf nur noch einmal ein. Eine Kachel, die jemand anders seit der Liste
geschrieben hat, etwa der Render seine nativen Stufen, bleibt stehen. Eine
unlesbare Kachel lässt der Aufruf aus und nennt sie. Nicht bemerkt wird ein
einzelnes Kind, das von aussen verschwindet, solange Geschwister bleiben,
und eine Kachel, die mit ihrer alten Zeit aus einer Sicherung
zurückkommt. Dann die gröberen Stufen löschen, und `--pyramid` baut sie
ganz neu.

### `map.json`

```json
{
  "tileSize": 256,
  "scale": 32,
  "minZoom": 0,
  "maxZoom": 10,
  "tiles": "{z}/{x}/{y}.webp",
  "bounds": [-10240, 0, -6144, 4096],
  "nativeLevels": 0,
  "world": "cb13a94d6c88dae1-6872d5d8ff54db07"
}
```

`bounds` ist der belegte Bereich auf der feinsten Stufe in Pixeln, als
`[links, oben, rechts, unten]`. Die Projektion selbst steht nicht drin: sie
hängt allein an `scale`, und die Formel gehört in den Renderer, nicht in eine
Datei.

`world` ist die Kennung der Welt: vorn ein Salz, das der Baum bei seinem
ersten Lauf zufällig bekommt, dahinter ein Hash ihres Seeds und ihrer
Dimension, SipHash-2-4 mit diesem Salz, eine Million Mal verkettet. Die
Dimension gehört dazu, weil die Dimensionen einer Welt meist denselben Seed
tragen: sonst käme der Nether in den Baum der Oberwelt und die Oberwelt in
seinen. `--world` zeigt auf die Weltwurzel, das Verzeichnis mit
`level.dat`, oder auf eine Dimension darin, `dimensions/<namensraum>/<name>`.
Die Wurzel ist die Oberwelt, auch über `dimensions/minecraft/overworld`; eine
Kopie von `level.dat` in einer Dimension macht diese nicht zur Oberwelt. Der
Pfad zählt so, wie er auf der Platte steht: unter Windows gibt
`DIMENSIONS\MINECRAFT\THE_NETHER` dieselbe Kennung wie
`dimensions\minecraft\the_nether`, und ein Weg über `..` dieselbe wie der
direkte. Führt er auf der Platte über einen Link aus der Welt hinaus, etwa zu
einer Dimension auf einer anderen Platte, oder lässt er sich dort nicht
auflösen, zählt er so, wie er angegeben ist, auch in seiner Schreibweise:
`dimensions\Minecraft\the_nether` gibt dann die Kennung von
`Minecraft:the_nether`.

Den Seed liest der Renderer zuerst aus der Dimension selbst, aus
`data/minecraft/world_gen_settings.dat` darin: so schreibt Paper ihn je
Dimension, und eine Plugin-Welt hat oft einen eigenen. Sonst aus derselben
Datei an der Weltwurzel, wie Vanilla seit 26.1, oder aus der der
Paper-Oberwelt, von beiden aus der jüngeren, bei gleichem Alter aus der von
Paper: unter Paper bleibt an der Wurzel eine ältere liegen. Frühere Stände
des Renderers lasen zuerst die Datei an der Wurzel. Hat eine Dimension eine
eigene mit anderem Seed, passt ihr Baum aus einem solchen Stand nicht mehr zu
ihr, und der Lauf lehnt ihn ab, er gehöre zu einer anderen Welt oder
Dimension; einen solchen Baum neu rendern. Er selbst steht nicht in der Datei: `map.json`
liegt öffentlich neben den Kacheln, und mit dem Seed fände jeder Strukturen
und Erze ohne zu suchen. Ein
Zufallsseed hat nur 2^48 Werte, Vanilla zieht ihn mit 48 Bit Zustand; mit
einem einzelnen Hash liessen sich alle in Stunden bis Tagen durchprobieren.
Verkettet sind es 2^68 Aufrufe je Baum, auf einer Grafikkarte Jahrzehnte,
und der Export zahlt dafür 16 ms je Lauf. Ein Seed aus einem Text hat nur
2^32 Werte, 2^52 Aufrufe: den schützt die Kennung für Stunden bis Tage,
nicht für immer. Das gilt nur, wenn man alle durchprobieren muss. Jeder
geratene Seed kostet einen Versuch von 16 ms, und ein eingetippter wie
12345 oder einer aus einer öffentlichen Liste steht in jedem Wörterbuch:
den findet man in Sekunden. Ohne `level.dat` darüber ist die Welt nicht zu
erkennen, etwa bei einer Kopie ohne sie, und ohne Seed auch nicht, etwa bei
einer Kopie ohne `data`. Die Ausgabe sagt dann, was fehlt, und nennt jeden
Ort, an dem der Seed gesucht wurde. Dann steht `"world": null` da, und ein
solcher Baum nimmt keine Welt mit Kennung auf; zwei Welten ohne Kennung
kann der Renderer nicht auseinanderhalten.

Eine Kachel muss Pixel für Pixel dem entsprechenden Ausschnitt eines grossen
Renderings gleichen, sonst stünden im Browser Kanten dazwischen. Neun Kacheln
nebeneinander, die Grenzen rot eingezeichnet:

![Kacheln](docs/kacheln.png)

### Was das kostet

Gemessen an einem Ausschnitt, hochgerechnet auf die ganze Welt: derselbe
Weltausschnitt um (-64, 416) bei jedem scale, mit allen nativen Stufen und
Pyramide, also `--size 8192` bei scale 32, `4096` bei 16 und `2048` bei 8.
Das sind 1600, 400 und 100 Basiskacheln, gerendert auf 24 Threads. Die
Kachelzahl der ganzen Welt nennt der Vorlauf.

| `--scale` | Kacheln der Welt | je Kachel | Basis | native Stufen | zusammen | Dauer |
|-----------|------------------|-----------|-------|---------------|----------|-------|
| 32 | 292 836 | 109 kB | ~30 GB | ~10 GB | ~40 GB | ~80 min |
| 16 | 73 920 | 111 kB | ~7,8 GB | ~2,1 GB | ~10 GB | ~40 min |
| 8 | 18 951 | 101 kB | ~1,8 GB | ~0,4 GB | ~2,2 GB | ~20 min |

Auf demselben Ausschnitt wiegt eine Kachel bei jedem scale rund 100 bis
110 kB: sie zeigt bei kleinerem scale mehr Welt, aber gleich viele Pixel.
Der Platz hängt deshalb fast nur an der Kachelzahl. Die Dauer nicht: jede
native Stufe zeichnet jeden Block ihrer Fläche noch einmal, und zusammen
kosten sie fast so viel Zeit wie die Basis, bei scale 32 12,1 s gegen
13,6 s. In Bytes sind sie ein Fünftel bis ein Drittel. Die Sprite-Tabellen
aller 3110 Blockstates brauchen über die vier Stufen zusammen rund 12 s.

Der erste Vollrender einer grossen Serverwelt hat die Rechnung geerdet:
2,5 Millionen Chunks, 30 GB, scale 32.

| | |
|---|---|
| Vorlauf | 259 s |
| Basiskacheln | 2 504 461, rund 120 kB je Kachel, also ~300 GB |
| Rate | 44 Kacheln/s auf 24 Threads, davon nur 9 Kerne frei |
| Basisstufe | 2 504 461 / 44 s, knapp 16 Stunden |

Das ist keine Eigenschaft der Welt, sondern des Renderers. Eine Kachel
kostet rund 0,2 CPU-Sekunden, 9 Kerne für 44 Kacheln je Sekunde; jeder
der 24 Threads braucht für eine gut eine halbe Sekunde, weil er auf
einen freien Kern wartet. Sie ist ein schräger Schnitt durch die volle
Bauhöhe von 384 Blöcken, lädt und dekodiert dafür 50 bis 70 Chunks, und
ihr Chunk-Cache entsteht je Kachel neu. Der Vorlauf liest dieselben
Chunks einmal in vier Minuten.

WebP wird **verlustfrei** geschrieben. Minecraft-Texturen sind Pixelkunst mit
wenigen flachen Farben; verlustbehaftet würde daraus Matsch, und an den
Kachelrändern sähe man die Artefakte im Raster. Gegenüber PNG spart
verlustfreies WebP auf diesem Inhalt 20 bis 40 Prozent — dieselbe Kachel wiegt
als PNG 173 kB und als WebP 108 kB.

### Wasser und Biomfarben

Flüssigkeiten haben kein Blockmodell — Minecraft baut ihre Geometrie im
Code. Der Renderer tut dasselbe: `water`, `lava`, `bubble_column`, Seegras
und Kelp sowie jede Blockstate mit `waterlogged=true` bekommen einen Würfel
mit der Flüssigkeitstextur, fliessendes Wasser (`level` 1 bis 7) einen
flacheren. Wie im Spiel endet eine Quelle bei 8/9 der Blockhöhe, knapp
zwei Texturpixel unter der Kante, und die Oberseiten gefluteter oberer
Platten, Treppen und Zaunpfosten bleiben trocken. Liegt dieselbe
Flüssigkeit darüber, reicht der Würfel bis oben, sonst hätte jede Schicht
eines Ozeans eine Fuge. Die Ecken gleicht der Renderer nicht an die
Nachbarn an wie Minecraft, jede Oberfläche bleibt eben. Auch die Seiten
gefluteter Blöcke bleiben trocken: Minecraft rückt jede Flüssigkeitsfläche
ein Tausendstel ins Blockinnere, der Renderer legt sie dafür in der Tiefe
knapp hinter die Blockfläche an derselben Stelle.

![Übersicht](docs/map-wide.png)

Um den Ursprung, `--center 0 0 --size 900 --scale 4`, sonst wie oben.

Die Wassertextur ist grau und durchscheinend. Ihre Farbe kommt aus
`water_color` des Bioms. Flächen zwischen zwei Wasserblöcken werden nicht
gezeichnet — wie im Spiel: ein Sprite kennt seine Nachbarn zwar nicht,
aber der Renderer, und er wählt je Block die Fassung ohne die Flächen zu
gleichem Wasser daneben und darüber. Sonst läge in jedem Becken Wasser
über Wasser, die Deckkraft stiege an jeder Blockgrenze, und der Grund
schimmerte durch ein Raster. Ein Wasserblock mitten im Ozean hat danach
keine Fläche mehr und kostet nichts. Steht das Wasser daneben tiefer, am
Fuss eines Wasserfalls oder an jeder Stufe fliessenden Wassers, fehlte über
dessen Oberfläche ein Streifen der eigenen Seite. Minecraft hebt dort die
Ecken der Oberfläche an; der Renderer zeichnet stattdessen genau diesen
Streifen, als eigenes Sprite je Paar aus eigener Höhe und Nachbarhöhe in
Neunteln.

Die Oberfläche trägt dafür die Deckkraft aller Schichten dahinter. Eine
Schicht der Wassertextur lässt 29 Prozent durch, zwei noch 9, vier noch
unter 1: durch einen Block Wasser sieht man den Grund, durch vier nicht
mehr. Der Renderer zählt je Oberflächenblock die Wasserblöcke entlang
des Blickstrahls, also schräg nach hinten unten auf der Diagonale
(x-1, y-1, z-1), und nimmt die Fassung mit dem entsprechend
hochgerechneten Alpha — ohne das sähe ein Ozean aus wie ein Meeresboden
hinter Milchglas, mit sichtbarem Kies in jeder Tiefe. Im Spiel erledigt
das der Unterwassernebel. Senkrecht gezählt verschwände, was knapp unter
einer tiefen Oberfläche liegt, ein Wrack oder ein Riff. Die Zählung endet
an dem, was den Strahl aufhält: dem Grund, dem Ufer, einem Stein. Dünne
Modelle zählen unter einer Quelle als Wasser, denn neben Seegras, Kelp oder
einem gefluteten Zaun geht der Strahl weiter bis zum Grund; endete die
Zählung an ihnen, stünde über jedem Seegras ein heller Fleck. Ob ein Block den Strahl
aufhält, misst der Renderer an den Pixeln, die die Oberfläche an seiner
Stelle belegen würde, auf ihrer Höhe und immer bei scale 32: hinter
fliessendem Wasser treten die Strahlen tiefer ein als hinter einer Quelle,
und jede Stufe soll gleich zählen. Deckt er mehr als die Hälfte davon,
endet die Zählung. Hinter einer Quelle halten eine obere Platte und ein
Mauerpfosten den Strahl auf, eine untere Platte nicht, denn über sie gehen
156 von 256 Strahlen hinweg. Hohes Seegras deckt dort höchstens die Hälfte
und zählt wie Wasser; sonst stünde über ihm ein heller Fleck. Hinter
fliessendem Wasser verschiebt sich das. Gemessen an Vanilla 26.2 bei
scale 32, je `level` des Wassers davor, deckt ein Block so viele der 256
Pixel; fett heisst, die Zählung endet an ihm:

| Block | Quelle | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| untere Platte | 100 | **132** | **182** | **226** | **256** | **256** | **256** | **256** |
| obere Platte | **256** | **256** | **256** | **256** | **226** | **182** | **132** | 100 |
| Seegras | 37 | 51 | 70 | 82 | 103 | 119 | **141** | **131** |
| hohes Seegras, unten | 128 | **151** | **163** | **167** | **180** | **188** | **194** | **179** |
| hohes Seegras, oben | 75 | 90 | 113 | 122 | **129** | **131** | **130** | 116 |
| Kelp | 80 | 102 | 119 | 128 | 128 | 125 | 106 | 94 |
| Zaunpfosten | 80 | 92 | 108 | 112 | 112 | 108 | 92 | 80 |
| Mauerpfosten | **160** | **184** | **192** | **192** | **192** | **192** | **184** | **160** |

Der Preis: unter einer Quelle verschwindet ein dünnes Modell einen Block
unter der Oberfläche fast, wenn dahinter tiefes Wasser steht, Seegras,
Kelp, Zaunpfosten, Korallenfächer. Die Oberfläche darüber trägt die
Deckkraft aller Schichten dahinter, und statt 29 Prozent bleibt von ihm
unter 1 Prozent sichtbar.

Gras und Laub funktionieren wie das Wasser: die Textur ist grau, das Biom
liefert Temperatur und Niederschlag, und die Colormaps `grass.png` und
`foliage.png` aus den Assets machen daraus die Farbe.
Fichten, Birken und Seerosen haben feste Farben, der Sumpf seinen eigenen
Grünton, der Dunkelwald eine Abdunkelung — alles wie in `BlockColors`, nur
beschränkt auf das, was auf einer Karte Fläche macht. Redstone, Ranken und
Kürbisstiele bleiben ungefärbt.

Welche Blöcke gefärbt werden, steht nicht in den Assets. Minecraft
verdrahtet das im Code, und der Renderer tut es in
`renderer/src/assets/colors.rs` — eine Tabelle mit rund zwanzig Einträgen.

Gefärbte Fassungen entstehen nur für die Biome, mit denen ein Block im
Vorlauf eine Section teilt: auf der ganzen Welt kommt jedes Biom vor, aber
nicht jeder Block in jedem. Pixelgleiche Sprites teilen sich einen Eintrag,
wenn sie sich in jedem Biom gleich färben. Zusammen schrumpft die Tabelle
der Testwelt bei scale 32 damit auf ein Drittel, von 100 688 auf 33 762
Sprites.

Durchsichtige Flächen mischen sich seit diesem Schritt auch innerhalb
eines Sprites: jede Fläche legt je Pixel ein Fragment ab, und am Schluss
wird je Pixel von hinten nach vorne gemischt. Ein durchscheinendes Texel
liegt so immer über dem, was dahinter liegt, und eine Halmkante, die den
Pixel nur zum Teil deckt, verdeckt die Fläche dahinter nicht. Vorher
gewann ein Tiefenpuffer, und ein gefluteter Zaun war ein Wasserwürfel
ohne Zaun.

### Keine Nähte

Sprite-Kanten werden nicht geglättet. Die Geometrie wird nur im
Pixelmittelpunkt geprüft, damit an einer gemeinsamen Kante jeder Pixel
genau einer der beiden Flächen gehört und Nachbarflächen nahtlos
aneinanderstossen. Liegt ein Mittelpunkt genau auf der Kante, entscheidet
die Füllregel der Grafikkarten: der Pixel gehört dem Dreieck, für das die
Kante oben oder links liegt. Dafür rechnen beide Dreiecke die gemeinsame
Kante von derselben Ecke aus. Von verschiedenen Ecken aus rundet f32 bei
gedrehter Geometrie verschieden, und ein Pixel genau auf der Kante fiel bei
beiden durch — bei scale 32 derselbe Pixel in jedem Kreuzmodell. Ohne die
Regel nahmen beide Dreiecke einer Fläche
die Pixel auf ihrer Diagonale an, und bei scale 2 bekam Wasser dort
Alpha 233 statt 180. Geprüft ist beides an 300 zufällig gedrehten Quadern
bei scale 4 bis 64, gedreht vom Baker selbst. Dabei zeigte sich eine
Fläche genau parallel zur Blickrichtung: f32 legt ihre Normale knapp neben
null, und lag sie davor, zeichnete die Fläche einen hauchdünnen Streifen,
dessen Pixel auf der Kante des Nachbarn zweimal kam. Solche Flächen zählen
jetzt als abgewandt. In Vanilla und im Pack haben 82 Blockstates eine,
etwa Kerzen, Hängeschilder und schräge Schienen. Ihre Pixel bleiben auf
allen fünf scales gleich, nur 370 von 1705 Sprites bekommen einen kleineren
Rahmen. Geglättete Kanten trügen
Teildeckung im Alpha, und beim Zusammensetzen der Sprites könnte niemand
mehr unterscheiden, ob zwei Nachbarflächen dasselbe Pixel teilen oder ob
eine durch die andere scheint: ein Wasserbecken bekam an jeder Blockgrenze
eine hellere Naht, ein Boden aus deckenden Blöcken dunkle Linien — bis
Schritt 8 hatte das Goldbild sie. Die Textur dagegen wird über den Pixel
gemittelt, sonst fiele auf einer acht Pixel breiten Seitenfläche jeder
zweite Texel weg. Gemittelt wird wie in der Pyramide in linearem Licht
und mit vormultipliziertem Alpha.

### scale 32

`scale` ist die Breite des ganzen Würfels; eine Seitenfläche ist halb so
breit. Bei scale 16 hat sie acht Pixel für sechzehn Texel, bei scale 32
sechzehn — erst dann ist die Textur vollständig zu sehen. Deshalb ist 32
jetzt der Standard. Der Preis: viermal so viele Kacheln, für die Testwelt
rund 300 000 statt 74 000 bei scale 16. Wer die Hälfte der Texturzeilen
verschmerzen kann, gibt `--scale 16` an.

### Varianten aus der Position

34 Vanilla-Blockstates liegen als Liste vor — Sand, Stein, Erde, Grasblock
in vier Drehungen. Welche ein Block bekommt, würfelt Minecraft aus seiner
Position: `Mth.getSeed(x, y, z)` wird die Saat, `nextInt` über das
Gesamtgewicht zieht eine Zahl, und die Gewichte werden der Reihe nach
abgezählt, bis sie verbraucht ist. Obere Hälften von Doppelpflanzen und
Türen nehmen die Position der unteren, das Fussende eines Betts die des
Kopfendes: `DoublePlantBlock`, `DoorBlock` und `BedBlock` überschreiben
`getSeed`, und beide Hälften passen so immer zusammen. Der Renderer rechnet
genau das nach,
geprüft an sieben Positionen gegen die Klassen des 26.2-Clients. Damit
sieht Sand aus wie im Spiel statt wie eine Tapete, und die Wahl hängt
weder von der Kachel noch vom Thread ab. Alle Alternativen sind vorab
gerastert; der Renderpfad rechnet je Block nur die Saat. Fehlt einer
Alternative das Modell, zeichnet der Renderer dort wie das Spiel den
Missing-Würfel, und ihr Gewicht bleibt. Fiele sie weg, würfelten auch die
intakten Positionen anders als im Client. Das gilt für jeden kaputten
Verweis: auch wenn alle Alternativen einer Blockstate kaputt sind, und für
den einen kaputten Teil eines Multipart-Modells, jeweils mit der Drehung
des Eintrags. Wie der Renderer Blockstate-Dateien liest, steht unten
unter „Blockstates wie im Client“.

Ein Durchlauf über die gesamte Testwelt, der jeden Chunk dekodiert, jede
vorkommende Blockstate auflöst und sie rastert:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --scan
```

```
Scan:       316223 Chunks in 71.3 s (4436 Chunks/s), 0 Fehler
            3110 verschiedene Blockstates
Assets:     3110 Blockstates aufgelöst in 0.6 s, 0 ungelöst
            7 Blöcke ohne Modell:
            minecraft:air
            minecraft:brown_wall_banner
            minecraft:cave_air
            minecraft:chest
            minecraft:decorated_pot
            minecraft:skeleton_skull
            minecraft:white_wall_banner
Sprites:    3076 gerastert bei scale 32 in 0.4 s (7351/s)
            9.0 MB Sprite-Pixel, größtes: minecraft:brain_coral_fan[waterlogged=true] (46x31)
            1 Blöcke sind aus dieser Blickrichtung unsichtbar: minecraft:fire

Texturen:   715 geladen, 0 fehlen
```

Truhen, Banner, Schädel und Töpfe zeichnet Minecraft über Entity-Modelle,
die kennt der Renderer noch nicht. Wasser, Lava und Blasensäule fehlen in
der Liste, weil der Renderer sie wie das Spiel im Code baut; eine geflutete
Truhe steht trotzdem darin, auf der Karte ist dort nur ihr Wasser.

### Blockstates wie im Client

Packs stapeln sich Zustand für Zustand: nennt die Datei eines oberen Packs
nur einen Teil der Zustände, gilt für den Rest die darunter. Eine kaputte
Blockstate-Datei verwirft der Renderer wie der Client nur für ihr Pack.
Kaputt ist, was 26.2 ablehnt, belegt per javap am Client samt DFU und
Gson:

- etwas hinter dem ersten Dokument, wie bei `StrictJsonParser`;
- `"variants": {}`, `"multipart": []` und eine leere Modellliste;
- ein Gewicht unter 1 oder eines, das keine Zahl ist, und eine Summe der
  Gewichte über 2147483647. Gewichte liest der Client nur in einer Liste,
  das `weight` eines einzelnen Objekts zählt nicht;
- ein Modellname, der kein `Identifier` ist, etwa mit Grossbuchstaben;
- eine Drehung, die modulo 360 nicht 0, 90, 180 oder 270 ist. -90 ist
  270, 90.5 ist 90, wie `intValue` abschneidet. `uvlock` muss ein
  Wahrheitswert sein;
- eine Bedingung `{}`, `OR` oder `AND` neben weiteren Schlüsseln und ein
  leerer Term wie in `"a||b"`. Ein `OR` mit Text statt Liste ist eine
  Eigenschaft namens `OR`, und alte Packs dürfen Zahlen und
  Wahrheitswerte schreiben.

Ein Feld mit `null` zählt wie im Client als fehlend, `"when": null` gilt
also immer. Als Wert einer Variante oder in einer Liste ist `null` ein
Fehler. Zahlen liest der Renderer so, wie sie in der Datei stehen, und
schneidet sie ab wie Gsons `intValue`, auch jenseits von 64 Bit: ein
Gewicht 18446744073709551617 ist 1. Eine Zahl ab 1024 Zeichen macht die
Datei kaputt: so lang ist der Puffer von Gsons `JsonReader`, und nur im
Modus `LENIENT` liest er weiter. Das gilt für Blockstates, Modelle,
`.mcmeta` und Biome. Ebenso kaputt ist `1e10000`: `NumberLimits` lehnt ab
10000 Stellen zwischen letzter Ziffer und Komma ab.

Ein Byte-Order-Mark vorn überspringt Gson, auch in Modellen, `.mcmeta`
und Biomen, und kaputtes UTF-8 wird zu U+FFFD.

Jede Wurzel listet der Renderer einmal auf, so wie der Client ein Pack
(`PathPackResources`). Der Wurzel folgt er, auch über einen Link, und
ebenso jedem Namensraum darin. Die Anfänge der Listen nennt der Client
selbst: `blockstates`, `models` und die Ordner des Block-Atlas, in 26.2
`textures/block` und `textures/entity/conduit`; unter Windows gelten sie
also in jeder Schreibweise. Darunter zählt eine Datei nur, wenn ihr
Name auf der Platte ein `Identifier` ist: unter Windows fände
`block/stone` sonst auch `Stone.json`, das der Client übergeht, und eine
`.mcmeta` gehört nur in genau dieser Schreibweise zur PNG. Einen Link
darunter übergeht der Client, wie `Files.find` ohne `FOLLOW_LINKS`; eine
Junction ist für Java 25, auf dem 26.2 läuft, unter Windows aber ein
Ordner, und dem folgt er; ab Java 26 nicht mehr. Ganz aus lässt er ein
Pack mit einem Link nur im Ordner `resourcepacks` (`DirectoryValidator`),
die Wurzeln hier nennt der Nutzer.

Eine Colormap öffnet der Client direkt, ohne Liste, und folgt dabei jedem
Link; ebenso die beiden einzelnen Texturen des Block-Atlas,
`entity/bell/bell_body` und `entity/enchantment/enchanting_table_book`.
So öffnet der Renderer auch jede andere Textur ausserhalb der Ordner des
Atlas. Der Client zeigte für sie die Missing-Textur, es sei denn, ein Pack
erweitert `atlases/blocks.json`; diese Dateien liest der Renderer nicht.
Andere Ordner unter `textures` listet er wie der Client nicht auf.

Lässt sich der Anfang einer Liste nicht lesen, listet der Client dort
nichts. Fehlt er, fehlt sein Ziel oder ist es kein Ordner, etwa bei einer
Junction ohne Ziel, geschieht das still. Jeden anderen Fehler, etwa bei
einer Junction auf sich selbst oder unter Linux, wenn im Pfad davor eine
Datei steht, schreibt er ins Log, und der Renderer nennt ihn in der
Ausgabe. Ein Fehler tiefer im Baum lässt im Client das Laden der Packs
scheitern und bricht hier den Lauf ab.

Den Rest prüft der Client gegen die Definition des Blocks: welche
Eigenschaften er hat und welche Werte. Die stehen in
`renderer/src/assets/blocks.txt`, 1196 Blöcke aus dem Datengenerator von
26.2. Ein Variantenschlüssel mit unbekannter Eigenschaft oder unbekanntem
Wert fällt weg, nur dieser Eintrag. Zahlen liest `IntegerProperty` mit
`parseInt`, `age=07` ist also `age=7`. Überlappen sich zwei Schlüssel,
bekommt wie im Client der erste gemeinsame Zustand den späteren Eintrag,
und der Rest des späteren fällt weg; dafür behält der Renderer die
Reihenfolge der Datei. Eine Multipart-Bedingung mit unbekannter
Eigenschaft oder unbekanntem Wert dagegen verwirft im Client von 26.2 den
ganzen Block, über alle Packs. So endet etwa eine Mauer aus einem Pack vor
1.16 mit `"north": "true"`. Ob die Assets zu 26.2 gehören, weiss der
Renderer aber nicht; in einer späteren Version gibt es die Eigenschaft
oder den Wert vielleicht. Er vergleicht dort den Text und nennt die Datei
unter „Blockstates“ in der Ausgabe. Für Blöcke und Zustände, die 26.2 nicht
kennt, gibt es kein Vorbild; dort gilt der erste Schlüssel, der als Text
passt.

Fehlt einem Modell sein Parent, oder ist dessen Datei kaputt, setzt der
Client das Missing-Modell an seine Stelle: die eigenen Elemente des Kindes
bleiben, sonst erbt es den Missing-Würfel. Die Blockstate steht dann mit
dem Parent unter „Modelle“ in der Ausgabe, wie „Missing block model“ im
Log des Clients; sonst sähe man einen Tippfehler im `parent` nur an
fehlenden Texturen. 26.2 kennt dabei nur `builtin/missing` und
`builtin/generated`; ein `builtin/entity` aus älteren Packs fehlt.

Modelle liest der Renderer wie `CuboidModel` im Client. Die Textur einer
Fläche ist immer der Name eines Slots, mit oder ohne `#` davor;
`heavy_core` schreibt `"texture": "all"`. Verweise zwischen Slots löst er
bis zum Ende auf, nur ein Zyklus bleibt offen. Hier liest Gson die Felder,
nicht DFU: wo das Modell einen Wert braucht, ist `null` ein Fehler. Eine
Ansicht in `display`, `force_translucent` und eine Seite ohne Fläche
nehmen `null` hin. Kaputt ist ein Modell auch, wenn
`from` oder `to` nicht zwischen -16 und 32 liegt, ein Element keine Seite
hat, eine Seite einen unbekannten Namen trägt, einer Drehung `origin` oder
der Winkel fehlt oder eine Textur kein `Identifier` ist. Das gilt auch für
`display`, `gui_light` und `ambientocclusion`, die der Renderer sonst nicht
braucht. Dafür nimmt der Client Zahlen, wie `intValue` und
`Float.parseFloat` sie lesen: eine Flächendrehung -90 ist 270, und
`"tintindex": 0.0` ist 0. Die Achse `"Y"` gilt als `y` und eine unbekannte
`cullface` als keine. Eine Fläche ohne Ausdehnung fällt weg, bevor der
Client ihre Textur sucht.

Eine `.mcmeta` liest der Renderer wie der Block-Atlas: `animation` und
`texture` je mit ihrem Codec. Was einer davon ablehnt, etwa
`"frametime": 0` oder `"blur": 1`, macht die Textur wie im Client zur
Missing-Textur, und die Ausgabe nennt den Grund. `"width": 16.0` ist 16.
Fehlt eine Bildgrösse, gilt dafür die Seite des Bildes, fehlen beide, für
beide seine kürzere; teilt sie das Bild nicht, ist die Textur ebenso
kaputt. Der
Renderer zeigt das Bild, mit dem der Client beginnt: das erste gültige
aus `frames`. Bleibt nur eines, ist die Textur statisch, und ist das Bild
dann grösser als eines, scheitert im Client der Atlas; der Renderer zeigt
die Missing-Textur.

Alle 1198 Blockstate-Dateien von Vanilla 26.2 und die 39 des
TerraNova-Packs lesen sich so ohne Fehler und ohne verworfenen Eintrag,
und für jeden Zustand jedes Blocks wählt der Renderer damit dasselbe wie
mit dem ersten passenden Schlüssel. Nur eine ganz fehlende
Blockstate-Datei bleibt ein Fehler.

Für eine andere Version wird die Tabelle neu erzeugt, aus dem Server-JAR
dieser Version in einem leeren Verzeichnis; danach kommt `blocks.txt` nach
`renderer/src/assets/`. Sie ist einkompiliert, der Renderer muss danach neu
gebaut werden, und der Test `blocktabelle_aus_26_2` bekommt die Zahlen der
neuen Version. Für 26.2 ergibt das genau die Datei im Repository:

```bash
java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports
python -c "import json; d = json.load(open('generated/reports/blocks.json')); open('blocks.txt', 'w', newline='\n').writelines(' '.join([n.removeprefix('minecraft:')] + [p + '=' + ','.join(v) for p, v in b.get('properties', {}).items()]) + '\n' for n, b in d.items())"
```

## Frontend

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles web/public/tiles
cd web && npm install && npm run dev
```

Der Renderer schreibt die Kacheln direkt dorthin, wo der Devserver sie
ausliefert; damit braucht das Frontend keine Konfiguration. `npm run build`
legt die Seite unter `web/dist` ab, ohne `public/tiles`: dort liegt oft ein
Link auf Hunderte Gigabyte, und Vite folgte ihm beim Kopieren, auch unter
`npm test`. Beim Ausliefern gehören die Kacheln als `tiles/` neben die
Seite, oder `?tiles=` nennt ihren Pfad; statisch ausliefern reicht.

Wer einem langen Render zusehen will, legt die Kacheln woanders ab und
setzt einen Link: unter Windows `mklink /J web\public\tiles D:\tiles`, sonst
`ln -s /pfad/zu/tiles web/public/tiles`. Findet Vite unter `public/` einen
Link, fragt es bei jeder Anfrage die Platte und liefert auch Kacheln aus,
die nach seinem Start entstanden sind, etwa durch `--pyramid`. Aus einem
echten Verzeichnis dort liefert es nur, was beim Start dalag, bis zum
nächsten Neustart. Neue Dateien meldet ihm sonst sein Watcher, und den hat
`vite.config.ts` von den Kacheln abgekoppelt: er beobachtete jede der
Millionen Dateien und verbrennte Kerne, die der Render braucht.

Ohne echte Kacheln zeigt `http://localhost:5173/?tiles=/tiles-demo` einen
kleinen Kachelbaum, der mit im Repository liegt — 7 Dateien, 6,6 kB. Er ist
zugleich das Fixture des Smoke-Tests.

### Was das Frontend tut

Es liest `map.json` und baut daraus ein Koordinatensystem, in dem eine
Karteneinheit ein Pixel der feinsten Stufe ist. Leaflets `CRS.Simple`
rechnet mit `2^zoom`; hier bekommt stattdessen die feinste Stufe den Faktor
1:

```ts
scale: (zoom: number) => 2 ** (zoom - info.maxZoom),
zoom: (scale: number) => Math.log2(scale) + info.maxZoom,
```

Gespiegelt wird nicht: `screen_y` des Renderers zeigt schon nach unten.
Negative Kachelkoordinaten sind damit kein Sonderfall.

Über die feinste gerenderte Stufe hinaus sind zwei weitere Zoomstufen
erlaubt. Dort vergrössert Leaflet nur noch die vorhandenen Kacheln
(`maxNativeZoom`), und `image-rendering: pixelated` hält die Pixelkunst
scharf, statt sie zu verwischen.

Nach unten geht es unter Zoom 0, wenn die ganze Karte dort nicht ins
Fenster passt, etwa nachdem die Welt gewachsen ist. Dann verkleinert
Leaflet die Kacheln von Zoom 0 (`minNativeZoom`), bis alles zu sehen ist.

Mehr ist es nicht: keine Marker, keine Spieler, kein Zustand. Der Browser
bekommt fertige Bilder und ein Koordinatensystem.

## Eingabedaten

`world/`, `assets/`, `vanilla-assets/` und `vanilla-data/` sind in
`.gitignore` — die Testwelt allein ist 2,4 GB. Sie werden dem Renderer über
CLI-Argumente übergeben.

## Entwicklung

```bash
cd renderer
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo nextest run --all-targets
cargo nextest run --all-targets --release
cargo deny check
```

```bash
cd web
npm run check     # tsc --noEmit
npm run lint      # ESLint
npm test          # Playwright, baut vorher und prüft den Build
```

Das Fixture unter `renderer/tests/fixtures/` ist eine 40 KB große Region mit
2×2 echten Terrain-Chunks aus der Zielwelt (Paper 26.2, DataVersion 4903).
Die Sollwerte der Tests stammen aus einem unabhängig geschriebenen
Python-Decoder, damit die Tests nicht dieselbe Annahme prüfen wie der Code.

Beschädigte Regionsdateien lassen sich nicht aus einer echten Welt
extrahieren. `renderer/tests/region_format.rs` baut sie deshalb zur Laufzeit:
ausgelagerte `.mcc`-Chunks, kaputte Längenfelder und Tabelleneinträge,
Paletten ohne Indexdaten, Indizes jenseits der Palette.

Für den Asset-Layer liegt unter `renderer/tests/fixtures/assets-base` und
`assets-overlay` ein kleiner, von Hand geschriebener Assetbaum. Er ist
synthetisch, bildet aber die Formen ab, die eine Bestandsaufnahme über
Vanilla 26.2 und das TerraNova-Pack ergeben hat.

`renderer/tests/metatile.rs` baut aus diesem Assetbaum ganze Welten im
Speicher und rendert sie; `renderer/tests/cli.rs` ruft dafür die echte
Binärdatei auf, weil der Weg über `--center` eine eigene Fehlerquelle ist.
`renderer/tests/tiles.rs` prüft die Naht: jede einzeln gerenderte Kachel gegen
den entsprechenden Ausschnitt eines grossen Renderings. Und `tests/cli.rs`
hält die ganze Exportkette fest — unter anderem, dass jede Kachel einer
gröberen Stufe Pixel für Pixel die Verkleinerung ihrer vier Kinder ist. Dazu gehört ein Goldbild unter
`tests/fixtures/golden/`: jede Änderung an Projektion, Baking, Rasterizer oder
Maleralgorithmus fällt damit auf. Neu erzeugen nach einer gewollten Änderung:

```bash
UPDATE_GOLDEN=1 cargo test --test metatile
```

Fällt der Test, schreibt er das Ist-Bild daneben als `metatile-ist.png`; in CI
liegt es als Artefakt am fehlgeschlagenen Lauf.

## Die Kamera

Fest und orthographisch, alle Faktoren stehen in `render/projection.rs`:

```text
screen_x = (x - z) * scale/2
screen_y = (x + z) * scale/4 - y * scale/2
```

Damit belegt ein voller Würfel genau `scale` mal `scale` Pixel. Sichtbar sind
immer dieselben drei Seiten: oben, Süden (links im Bild) und Osten (rechts).
Die Blickachse ist (1, 1, 1) — Punkte, die sich um ein Vielfaches davon
unterscheiden, landen auf demselben Pixel.

Daraus folgt die Zeichenreihenfolge. Der Metatile-Renderer sortiert erst nach
Höhe `y`, innerhalb einer Höhe nach Tiefe `v = x + z`. Beides ist nötig:

- Verdeckt B den Block A, dann liegt B nie tiefer. Sonst wäre der senkrechte
  Abstand im Bild mindestens eine Blockhöhe, und die Umrisse berührten sich
  höchstens.
- Auf gleicher Höhe verdecken Blöcke einander sehr wohl: der Südnachbar
  `(x, y, z+1)` verdeckt die Südfläche von `(x, y, z)`, der Ostnachbar
  `(x+1, y, z)` die Ostfläche. Dort heisst "verdeckt" genau `v_B > v_A`, denn
  `depth = x + y + z = v + y`.

Zusammen ergibt das eine gültige Reihenfolge, und ein globaler Tiefenpuffer
wird unnötig. Die zweite Regel wegzulassen sieht nicht nach einem Sortierfehler
aus, sondern nach Textur — links läuft `u` aussen, rechts `v`:

![Zeichenreihenfolge](docs/zeichenreihenfolge.png)

Das Muster links sind die Süd- und Ostflächen jedes Blattblocks, die durch den
Block davor schlagen.

Sortiert wird nach Blockwürfeln und nicht nach Blöcken. Der Unterschied zählt
für Modelle, die ihren Würfel verlassen — Feuer ist höher als ein Block. Solche
Sprites zerfallen beim Bauen der Sprite-Tabelle in einen Teil je Würfel, und
jeder Teil wird zu dem Zeitpunkt gezeichnet, der zu seinem eigenen Würfel
gehört. Sonst käme ein zwei Blöcke hohes Modell zu früh, und ein Block
dahinter mit höherem Ursprung übermalte seine obere Hälfte.

Zugeordnet wird über den Bildschirm: die Umrisse benachbarter Würfel kacheln
die Ebene lückenlos, ein Pixel liegt also in genau einem — bis auf die
Blickachse, wo Würfel im Abstand (1, 1, 1) aufeinanderfallen. Dort gewinnt der
vordere, und genau dessen Geometrie hat auch der Tiefenpuffer des Rasterizers
stehen lassen. Ein Modell, das zwei Würfel entlang der Blickachse ausfüllt,
wäre so nicht auflösbar; in Vanilla gibt es keines.

Ein Würfel wird übersprungen, wenn seine drei kamerazugewandten Nachbarn ihn
ganz decken — deren Umrisse setzen genau den eigenen zusammen, mehr nicht.
Der Ost- und der Südnachbar müssen dafür ihren ganzen Umriss deckend füllen,
dem Nachbarn darüber genügt sein Boden: Lava endet bei 8/9 und deckt
trotzdem den Block darunter. Ob ein Sprite deckt, entscheidet sein fertiges
Bild und nicht sein Modell, Pixel für Pixel gegen einen gerasterten vollen
Würfel, damit Glas von selbst herausfällt. Mit einer Pixelbreite Toleranz
fiele der Block unter einer Druckplatte weg, und ihr Rand zeigte den
Hintergrund.

Die Suche nach hineinragenden Nachbarmodellen kostet nichts, solange kein
Modell seinen Würfel verlässt. In einem Ausschnitt mit Feuer kostet sie ein
Nachschlagen je leerem Würfel, rund ein Viertel der Renderzeit.

Weltkoordinaten werden in `f64` projiziert. Minecraft erlaubt knapp 30
Millionen Blöcke in jede Richtung; ab 2²⁴ kann `f32` benachbarte ganzzahlige
Blöcke nicht mehr auseinanderhalten, und zwei Nachbarn landen auf demselben
Pixel.

## Entwurfsregel

Bei jeder neuen Dependency und jedem neuen Feature gilt die Frage: Braucht ein
Offline-Renderer für isometrische Minecraft-Rastertiles das wirklich? Wenn
nein, kommt es nicht dazu.
