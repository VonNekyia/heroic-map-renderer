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

900 mal 900 Pixel um (-64, 416), scale 16, 292 Chunks, 3,2 s einkernig —
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

Beide Weltlayouts werden erkannt: das klassische `world/region` und das seit
Minecraft 26.1 genutzte `world/dimensions/minecraft/overworld/region`.

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

Einzelne Blockstates als Sprites rastern:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --assets ./vanilla-assets --assets ./assets --scale 64 --sprite docs/sprites.png --block stone --block "grass_block[snowy=false]" --block "furnace[facing=east,lit=false]"
```

![Sprites](docs/sprites.png)

Einen Weltausschnitt rendern:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render docs/map.png --center -64 416 --size 900 --scale 16
```

```
Render:     292 Chunks im Ausschnitt, 292 generiert, 258 Blockstates, 235 Sprites
            900x900 px um (-64, 416) bei scale 16 in 2.1 s -> docs/map.png
```

`--center` nennt die Blockspalte, die in der Bildmitte landet, `--scale` die
Pixelbreite eines Blocks. Gesucht wird nur, was im Bild landen kann: der
sichtbare Bereich ist ein schmales diagonales Band in x und z, kein Rechteck.
Wer stattdessen die Hüllbox nähme, läse für einen 1024er Ausschnitt rund das
Sechzehnfache an Chunks.

### Die ganze Welt als Kacheln

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles
```

```
Vorlauf:    316223 Chunks in 10.7 s, 3110 Blockstates, 73920 Kacheln
            3057 Sprites bei scale 16
            82 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
            200/73920 Kacheln
            400/73920 Kacheln
```

Danach stapelt der Lauf die gröberen Zoomstufen darüber und schreibt
`map.json`.

Der Vorlauf liest jeden Chunk einmal und beantwortet zwei Fragen auf einmal:
welche Blockstates vorkommen, und welche Kacheln überhaupt etwas zeigen. Erst
danach steht die Sprite-Tabelle — und erst dann kann parallel gerendert
werden, denn sonst müsste jeder Worker sie unter einer Sperre füllen. Die
Chunks werden deshalb zweimal gelesen; der Vorlauf kostet 11 Sekunden für die
ganze Welt.

Gerendert wird mit Rayon über Stapel aufeinanderfolgender Kacheln. Jeder
Stapel hält seinen Chunk- und Regionscache, geteilt wird nur die
unveränderliche Sprite-Tabelle. Kacheln untereinander teilen sich fast alle
Chunks; der Cache lädt je Kachel nur die paar neuen am unteren Rand.

`--center` und `--size` schränken auf einen Ausschnitt ein:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --tiles ./tiles --center -64 416 --size 2048
```

```
Vorlauf:    8192 Chunks in 0.4 s, 266 Blockstates, 72 Kacheln
Kacheln:    72 geschrieben, 0 leer, 256x256 px, 24 Threads
            9.4 MB in 1.7 s (44 Kacheln/s, 134 kB je Kachel)
Zoom  8:     25 Kacheln
Zoom  7:     9 Kacheln
Zoom  6:     4 Kacheln
...
Pyramide:   45 Kacheln, 3.1 MB in 0.0 s
Karte:      Zoom 0..9, -4864/256 bis -2816/2560 px -> ./tiles/map.json
```

Der Ausschnitt wird dabei auf ganze Kacheln aufgerundet, bevor der Vorlauf
irgendetwas ausschliesst — ausgegeben werden immer vollständige Kacheln, also
muss auch der Vorlauf sie vollständig abdecken. Umgekehrt sammelt er
Blockstates nur aus Chunks, die tatsächlich in eine ausgegebene Kachel fallen:
ein kleiner Ausschnitt braucht deshalb keine Assets für Blöcke am anderen Ende
der Welt.

Alte Spielstände tragen alte Blocknamen: ein Chunk speichert die Namen der
Version, die ihn zuletzt geladen hat, und ein Server voller nie wieder
betretener Chunks hat davon ganze Jahrgänge. Minecraft biegt das beim Laden
mit dem DataFixer gerade; der Renderer hat dafür eine Tabelle der
Umbenennungen, bei denen nur der Name wechselte (`grass_path` → `dirt_path`,
`grass` → `short_grass`, `chain` → `iron_chain`). Blöcke, die auch danach
kein Asset haben — aus einem Mod, aus einer Umbenennung ohne Eintrag —,
bleiben leer wie Luft. Der Lauf zählt sie nach dem Vorlauf auf, statt an
ihnen zu scheitern; ein einzelner unbekannter Block darf keinen Render von
Stunden abbrechen.

Die Kacheln liegen als `tiles/<z>/<x>/<y>.webp`; x und y dürfen negativ sein,
weil der Blockursprung mitten in der Welt liegt. Wird eine Kachel bei einem
erneuten Lauf leer, löscht der Export die alte Datei — auf jeder Stufe, sonst
zeigte die Karte weiter, was inzwischen abgerissen wurde.

`--resume` setzt einen abgebrochenen Lauf fort: vorhandene Basiskacheln
bleiben stehen, gerendert wird nur, was fehlt, und die Zoomstufen entstehen
danach über allen Basiskacheln. Für eine veränderte Welt ist das der falsche
Schalter — dann rendert erst ein Lauf ohne ihn die alten Kacheln neu.

### Zoomstufen

Gerendert wird nur die feinste Stufe. Jede gröbere entsteht aus vier Kacheln
der darunterliegenden, auf die halbe Kantenlänge gestaucht — die Welt wird
dafür kein zweites Mal angefasst. Für den Ausschnitt oben kosten alle Stufen
zusammen 3,1 MB gegenüber 9,4 MB für die Basis, also das erwartete Drittel.

![Zoomstufen](docs/zoomstufen.png)

Gemittelt wird mit vormultipliziertem Alpha. Geradeaus gemittelt zögen
durchsichtige Pixel ihre Farbe in die Nachbarn, und jede Kante gegen Luft
bekäme einen dunklen Saum — auf einer Karte voller Blattwerk wäre das überall
zu sehen.

Die Nummerierung hängt an der **Welt**, nicht am Ausschnitt: `maxZoom` kommt
aus der Ausdehnung aller Regionsdateien, und dafür wird kein einziger Chunk
gelesen.

Ein nachgerenderter Ausschnitt passt damit in einen bestehenden Kachelbaum.
Welche Kinder in eine Elternkachel gehören, entscheidet dabei die Platte und
nicht der laufende Export: die Geschwister ausserhalb des Ausschnitts liegen
ja weiterhin da. Und `map.json` beschreibt den ganzen Baum, nicht den letzten
Lauf. An einer unveränderten Welt ändert ein Nachrendern deshalb keine einzige
Datei.

Gemittelt wird in linearem Licht, nicht in sRGB-Werten: die sind
gammakodiert, ihr Mittel ist zu dunkel, und jede Stufe verdunkelt weiter.
Halb Schwarz, halb Weiss ergibt so 188 statt 128.

Verkleinern mittelt Nachbarblöcke ineinander; zwei Stufen unter der Basis
ist ein Block noch acht Pixel breit, und Blockkanten werden zu Verläufen.
Wer die Kanten länger scharf haben will, lässt mit `--native-levels N` die
ersten N gröberen Stufen aus der Welt rendern, mit Sprites in der
kleineren Grösse; das geht, solange ein Block noch zwei Pixel breit ist.
Der Preis ist ehrlich hoch: jede native Stufe ist ein weiterer Durchlauf
durch die Welt und kostet etwa so viel wie die Basis, denn der Renderer
zahlt je Block, nicht je Pixel. Deshalb ist die Vorgabe 0.

#### Pyramide nachbauen, Karte während des Renders ansehen

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --tiles ./tiles --pyramid
```

`--pyramid` rendert nichts. Es baut die gröberen Stufen und `map.json` aus
den Basiskacheln, die auf der Platte liegen — und zwar nur über
Basiskacheln, die jünger sind als ihre Elternkachel. Der Aufruf lässt sich
deshalb wiederholen, während ein Vollrender noch Stunden läuft: die Karte
im Browser zeigt, was fertig ist, und wächst mit jedem Aufruf. `--scale`
muss zum laufenden Render passen, die Zoomstufen kommen wie immer aus der
Ausdehnung der Welt.

### `map.json`

```json
{
  "tileSize": 256,
  "scale": 16,
  "minZoom": 0,
  "maxZoom": 9,
  "tiles": "{z}/{x}/{y}.webp",
  "bounds": [-4864, 256, -2816, 2560]
}
```

`bounds` ist der belegte Bereich auf der feinsten Stufe in Pixeln, als
`[links, oben, rechts, unten]`. Die Projektion selbst steht nicht drin: sie
hängt allein an `scale`, und die Formel gehört in den Renderer, nicht in eine
Datei.

Eine Kachel muss Pixel für Pixel dem entsprechenden Ausschnitt eines grossen
Renderings gleichen, sonst stünden im Browser Kanten dazwischen. Neun Kacheln
nebeneinander, die Grenzen rot eingezeichnet:

![Kacheln](docs/kacheln.png)

### Was das kostet

Der Vorlauf über die ganze Welt ist gemessen, der Vollrender hochgerechnet:
abgebrochen nach 6746 Kacheln und 801 MB, statt eine Stunde Plattenplatz zu
verbrennen.

| `--scale` | Vorlauf | Kacheln | je Kachel | hochgerechnet |
|-----------|---------|---------|-----------|---------------|
| 16 | 10,7 s | 73 920 | 134 kB | ~9 GB |
| 8 | 6,4 s | 18 951 | 137 kB | ~2,5 GB |

Eine Kachel ist immer 256x256 px, und ihr Inhalt ist bei scale 8 genauso dicht
wie bei 16 — sie zeigt nur viermal so viel Welt. Der Massstab wirkt also rein
über die Kachelzahl.

Der erste Vollrender einer grossen Welt hat die Rechnung geerdet:
Interconnect, 2,5 Millionen Chunks (30 GB), scale 32, gemessen vor dem
Umbau weiter unten.

| | |
|---|---|
| Vorlauf | 259 s |
| Basiskacheln | 2 504 461, rund 120 kB je Kachel, also ~300 GB |
| Rate | 44 Kacheln/s auf 24 Threads, davon nur 9 Kerne frei |
| Basisstufe | rund 15 Stunden |

Das ist keine Eigenschaft der Welt, sondern des Renderers. Eine Kachel
ist ein schräger Schnitt durch die volle Bauhöhe von 384 Blöcken: rund
320 000 Blockpositionen, gut hundert Chunks, und neun von zehn nicht-leeren
Blöcken liegen unter der Oberfläche. Gemessen an einem 4096er-Ausschnitt um
(0, 0), einfädig, je Kachel:

| Phase | ursprünglich | Cache je Stapel | Bitmasken | Flächen | Sammeln |
|---|---|---|---|---|---|
| Blöcke finden und Sprite wählen | 39 ms | 20 ms | 4,5 ms | 4,5 ms | 3,3 ms |
| Chunks laden und dekodieren | 15 ms (106 Chunks) | 1 ms (6,5) | 1,5 ms (9) | 1,5 ms | 1,5 ms |
| Sprites zeichnen | 9 ms | 9 ms | 8 ms | ~3 ms | ~3 ms |
| WebP kodieren | 0,5 ms | 0,5 ms | 0,6 ms | 0,6 ms | 0,6 ms |
| gesamt, ein Kern | 64 ms | 30 ms | 13 ms | 7,8 ms | 6,3 ms |
| 12 Threads, 8192er-Ausschnitt | | | 411 Kacheln/s | 620 | 700 |
| 24 Threads, 8192er-Ausschnitt | 159 Kacheln/s | 318 | 532 | 731 | 813 |

Keiner der Umbauten ändert einen Pixel: der 8192er-Ausschnitt ist nach jedem
Byte für Byte gleich, alle 1393 Dateien.

**Cache je Stapel.** Der Chunk-Cache lebt je Stapel aufeinanderfolgender
Kacheln statt je Kachel, und der Nachschlag merkt sich den letzten Chunk,
statt je Block zweimal zu hashen.

**Bitmasken statt Blockbesuche.** Der Renderer lief über jede Position im
Band, fragte je Block die Familie ab und für jeden nicht-leeren Block drei
Nachbarn — und wählte für neun von zehn erst das Sprite, bevor er merkte,
dass der Block verdeckt ist. Jetzt hält jede Section je Spalte ein 16-Bit-Wort
(Bit = y) für "vorhanden", "deckend", "volles Wasser", "ragt heraus":
verdeckt ist ein Block, wenn die Nachbarn nach +x, +y und +z deckend sind,
und das ist je Spalte eine Handvoll Wortoperationen für sechzehn Blöcke auf
einmal — nach +y ein Shift, an den Rändern kommt das Bit aus der Section
darüber oder dem Nachbarchunk. Für volles Wasser zählt gleiches Wasser als
Deckung, damit das Innere eines Ozeans gar nicht erst zur Sprite-Wahl kommt.
Aus den Masken fallen die Kandidaten heraus, ohne dass Luft je angefasst
wird; sortiert nach `(y, v, u)` sind sie genau die Zeichenreihenfolge des
Maleralgorithmus. Der zweite Durchgang zeichnet nur noch.

**mimalloc.** Parallel dauerte ein Chunk-Dekodieren sechsmal so lang wie
allein: der Windows-Heap serialisiert die vielen kleinen Allokationen des
NBT-Lesers über 24 Threads. Mit mimalloc hat jeder Thread seinen Heap; das
war der Unterschied zwischen 245 und 532 Kacheln/s.

**Flächen überspringen.** Ein sichtbarer Block zeichnete alle drei Flächen,
auch die, die der deckende Nachbar gleich übermalt: 70-fach überzeichnet,
auf flachem Gelände zwei von drei Flächen umsonst. Jetzt kennt der Renderer
je Pixelposition eines Sprites, in welchem Nachbarumriss sie liegt — eine
Tabelle je Projektion, nicht je Sprite —, und lässt die Pixel aus, deren
Nachbar deckend ist und in derselben Kachel gezeichnet wird. Der setzt sie
danach ohnehin auf Alpha 255; was vorher dort stand, ist egal. Genommen
wird nur der Umriss ohne seinen Pixelrand, denn nur innen garantiert
`covers_cell` das Alpha. Dazu schreibt der Blit deckende Pixel direkt
statt durch `over`.

**Sammeln nur, wo das Band hinreicht.** Die Sammelschleife lief je Kachel
über alle 24 Sections aller gut hundert Band-Chunks, 256 Spalten je
Section — 1,3 ms, mehr als das Auswerten der Masken selbst. Das Band
erreicht in einem Chunk aber nur rund 36 Höhen, also drei Sections; die
Umkehrung von `v_window` grenzt sie ein, und ein Flag je Section sagt, ob
überhaupt ein Kandidat drinsteht. Sections ohne Wasser, lose oder
herausragende Familien bauen nur die zwei Masken, die sie brauchen.

Drei Dinge, die gemessen nichts gebracht haben und deshalb nicht im Code
sind: Zeilenspannen im Blit samt `memcpy` deckender Zeilen (pixelgleich,
aber nicht schneller — der Blit wartet auf Sprite-Pixel aus dem Speicher,
nicht auf die Schleife), der Nachbar auf der Blickachse als vierte
Deckungsrichtung (kommt zu selten vor, die Prüfung je Pixel kostet mehr),
und `zlib-rs` statt `miniz` zum Entpacken der Chunks (kein Unterschied).

Was bleibt, verteilt sich: Chunks dekodieren, die Kandidaten aus den
Masken, die Sprite-Wahl, das Zeichnen. Auf 24 Threads sind es 5-mal so
viele Kacheln je Sekunde wie am Anfang, auf einem Kern 10-mal; die
Differenz ist Hyperthreading auf 12 Kernen plus das, was 24 Threads sich
an Speicherbandbreite teilen.

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
flacheren. Quellen und fallendes Wasser füllen den Block ganz; Minecraft
lässt sie einen Pixel tiefer enden, doch ohne Nachbarschaftswissen bekäme
sonst jede Schicht eines Ozeans eine Fuge.

![Übersicht](docs/map-wide.png)

Die Wassertextur ist grau und durchscheinend. Ihre Farbe kommt aus
`water_color` des Bioms. Flächen zwischen zwei Wasserblöcken werden nicht
gezeichnet — wie im Spiel: ein Sprite kennt seine Nachbarn zwar nicht,
aber der Renderer, und er wählt je Block die Fassung ohne die Flächen zu
gleichem Wasser daneben und darüber. Sonst läge in jedem Becken Wasser
über Wasser, die Deckkraft stiege an jeder Blockgrenze, und der Grund
schimmerte durch ein Raster. Ein Wasserblock mitten im Ozean hat danach
keine Fläche mehr und kostet nichts.

Die Oberfläche trägt dafür die Deckkraft aller Schichten darunter. Eine
Schicht der Wassertextur lässt 29 Prozent durch, zwei noch 9, vier noch
unter 1: durch einen Block Wasser sieht man den Grund, durch vier nicht
mehr. Der Renderer zählt je Oberflächenblock die Wasserblöcke darunter
und nimmt die Fassung mit dem entsprechend hochgerechneten Alpha — ohne
das sähe ein Ozean aus wie ein Meeresboden hinter Milchglas, mit
sichtbarem Kies in jeder Tiefe. Im Spiel erledigt das der Unterwassernebel.

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

Durchsichtige Flächen mischen sich seit diesem Schritt auch innerhalb
eines Sprites: die Flächen werden von hinten nach vorne gezeichnet, und
ein durchscheinendes Texel legt sich über das, was schon da ist. Vorher
gewann der Tiefenpuffer, und ein gefluteter Zaun war ein Wasserwürfel ohne
Zaun.

### Keine Nähte

Sprite-Kanten werden nicht geglättet. Die Geometrie wird nur im
Pixelmittelpunkt geprüft, damit jeder Pixel genau einer Fläche gehört und
Nachbarflächen nahtlos aneinanderstossen. Geglättete Kanten trügen
Teildeckung im Alpha, und beim Zusammensetzen der Sprites könnte niemand
mehr unterscheiden, ob zwei Nachbarflächen dasselbe Pixel teilen oder ob
eine durch die andere scheint: ein Wasserbecken bekam an jeder Blockgrenze
eine hellere Naht, ein Boden aus deckenden Blöcken dunkle Linien — bis
Schritt 8 hatte das Goldbild sie. Die Textur dagegen wird über den Pixel
gemittelt, sonst fiele auf einer acht Pixel breiten Seitenfläche jeder
zweite Texel weg.

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
Position: `Mth.getSeed(x, y, z)` als Saat für `java.util.Random`, der erste
`nextLong` gekürzt auf 32 Bit, davon der Betrag modulo Gesamtgewicht.
Der Renderer rechnet genau das nach, geprüft gegen ein echtes
`java.util.Random`. Damit sieht Sand aus wie im Spiel statt wie eine
Tapete, und die Wahl hängt weder von der Kachel noch vom Thread ab. Alle
Alternativen sind vorab gerastert; der Renderpfad rechnet je Block nur
die Saat.

Ein Durchlauf über die gesamte Testwelt, der jeden Chunk dekodiert, jede
vorkommende Blockstate auflöst und sie rastert:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --scan
```

```
Scan:       316223 Chunks in 80.8 s (3911 Chunks/s), 0 Fehler
            3110 verschiedene Blockstates
Assets:     3110 Blockstates aufgelöst in 1.0 s, 0 ungelöst
            10 Blöcke ohne Modell: air, cave_air, water, lava, bubble_column,
            chest, decorated_pot, skeleton_skull, brown_wall_banner,
            white_wall_banner
Sprites:    3069 gerastert bei scale 16 in 0.1 s (25608/s)
            2.3 MB Sprite-Pixel, größtes: minecraft:fire[...] (16x20)
Texturen:   734 geladen, 0 fehlen
```

Blöcke ohne Modell zeichnet Minecraft über Entity-Modelle oder als
Flüssigkeit — beides kennt V1 noch nicht.

## Frontend

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles web/public/tiles
cd web && npm install && npm run dev
```

Der Renderer schreibt die Kacheln direkt dorthin, wo Vite sie ausliefert;
damit braucht das Frontend keine Konfiguration. `npm run build` legt alles
unter `web/dist` ab, statisch ausliefern reicht.

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

Ein Würfel wird übersprungen, wenn seine drei kamerazugewandten Nachbarn volle,
deckende Blöcke sind — deren Umrisse setzen genau den eigenen zusammen, mehr
nicht. Ob ein Sprite "deckend" ist, entscheidet sein fertiges Bild und nicht
sein Modell, damit Glas von selbst herausfällt.

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
