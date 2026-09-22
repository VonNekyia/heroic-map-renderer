# TerraNova Map Renderer

Isometrischer Offline-Renderer für Minecraft-Java-Welten. Liest Weltdaten und
ein Resourcepack, rendert die Welt mit fester isometrischer Kamera und gibt
WebP-Rastertiles aus, die ein schlankes Leaflet-Frontend anzeigt.

```
Minecraft World + Resource Pack  ->  Rust Renderer  ->  WebP Tiles  ->  Leaflet
```

Der Browser rendert keine Minecraft-Geometrie, sondern nur fertige Rasterkacheln.

## Stand

Schritt 5 von 8: **Kacheln**. Die Welt wird parallel in WebP-Kacheln
gerendert, die ein Browser einzeln nachladen kann. Zoomstufen und Frontend
fehlen noch.

![Karte](docs/map.png)

900 mal 900 Pixel um (-64, 416), scale 16, 292 Chunks, 2,1 s einkernig.

| Schritt | Inhalt | Status |
|---------|--------|--------|
| 1 | Welt-Reader (Region, Chunk, Palette) | **fertig** |
| 2 | Resourcepack: Blockstates, Models, Texturen | **fertig** |
| 3 | Model-Baking und Iso-Sprite-Rasterizer | **fertig** |
| 4 | Metatile-Renderer | **fertig** |
| 5 | Rayon-Parallelisierung, Tiles, WebP | **fertig** |
| 6 | Zoom-Pyramide und `map.json` | offen |
| 7 | Frontend (Vite, TypeScript, Leaflet) | offen |
| 8 | Modelle und Transparenz im Detail | offen |

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
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --render docs/map.png --center -64 416 --size 900 --scale 16
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
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --tiles ./tiles --scale 16
```

```
Vorlauf:    316223 Chunks in 10.7 s, 3110 Blockstates, 73920 Kacheln
            3057 Sprites bei scale 16
            82 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
            200/73920 Kacheln
            400/73920 Kacheln
```

Der Vorlauf liest jeden Chunk einmal und beantwortet zwei Fragen auf einmal:
welche Blockstates vorkommen, und welche Kacheln überhaupt etwas zeigen. Erst
danach steht die Sprite-Tabelle — und erst dann kann parallel gerendert
werden, denn sonst müsste jeder Worker sie unter einer Sperre füllen. Die
Chunks werden deshalb zweimal gelesen; der Vorlauf kostet 11 Sekunden für die
ganze Welt.

Gerendert wird mit Rayon über die Kacheln. Jeder Worker hält seinen eigenen
Chunk- und Regionscache, geteilt wird nur die unveränderliche Sprite-Tabelle.

`--center` und `--size` schränken auf einen Ausschnitt ein:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --tiles ./tiles --center -64 416 --size 2048
```

```
Vorlauf:    8192 Chunks in 0.4 s, 266 Blockstates, 72 Kacheln
Kacheln:    72 geschrieben, 0 leer, 256x256 px, 24 Threads
            9.4 MB in 1.5 s (46 Kacheln/s, 134 kB je Kachel) -> ./tiles
```

Der Ausschnitt wird dabei auf ganze Kacheln aufgerundet, bevor der Vorlauf
irgendetwas ausschliesst — ausgegeben werden immer vollständige Kacheln, also
muss auch der Vorlauf sie vollständig abdecken. Umgekehrt sammelt er
Blockstates nur aus Chunks, die tatsächlich in eine ausgegebene Kachel fallen:
ein kleiner Ausschnitt braucht deshalb keine Assets für Blöcke am anderen Ende
der Welt.

Die Kacheln liegen als `tiles/<x>/<y>.webp`; beide Koordinaten dürfen negativ
sein, weil der Blockursprung mitten in der Welt liegt. Die Zoomstufe kommt in
Schritt 6 dazu. Wird eine Kachel bei einem erneuten Lauf leer, löscht der
Export die alte Datei — sonst zeigte die Karte weiter, was inzwischen
abgerissen wurde.

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

WebP wird **verlustfrei** geschrieben. Minecraft-Texturen sind Pixelkunst mit
wenigen flachen Farben; verlustbehaftet würde daraus Matsch, und an den
Kachelrändern sähe man die Artefakte im Raster. Gegenüber PNG spart
verlustfreies WebP auf diesem Inhalt 20 bis 40 Prozent — dieselbe Kachel wiegt
als PNG 173 kB und als WebP 108 kB.

### Wasser fehlt

Flüssigkeiten haben kein Blockmodell — Minecraft zeichnet sie über einen
eigenen Pfad. Der Renderer überspringt sie deshalb, und Ozeane erscheinen als
nackter Meeresboden. In einer Nahaufnahme fällt das kaum auf, über der ganzen
Welt sehr:

![Übersicht](docs/map-wide.png)

Die grauen Flächen sind Ozean, das Blau oben rechts ist Eis — Eis ist ein
gewöhnlicher Block und wird gezeichnet. Wasser steht in Schritt 8.

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

## Eingabedaten

`world/`, `assets/` und `vanilla-assets/` sind in `.gitignore` — die Testwelt
allein ist 2,4 GB. Sie werden dem Renderer über CLI-Argumente übergeben.

## Entwicklung

```bash
cd renderer
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo nextest run --all-targets
cargo nextest run --all-targets --release
cargo deny check
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
den entsprechenden Ausschnitt eines grossen Renderings. Dazu gehört ein Goldbild unter
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
