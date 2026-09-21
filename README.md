# TerraNova Map Renderer

Isometrischer Offline-Renderer für Minecraft-Java-Welten. Liest Weltdaten und
ein Resourcepack, rendert die Welt mit fester isometrischer Kamera und gibt
WebP-Rastertiles aus, die ein schlankes Leaflet-Frontend anzeigt.

```
Minecraft World + Resource Pack  ->  Rust Renderer  ->  WebP Tiles  ->  Leaflet
```

Der Browser rendert keine Minecraft-Geometrie, sondern nur fertige Rasterkacheln.

## Stand

Schritt 1 von 8: **Welt-Reader**. Der Renderer kann `.mca`-Regionen lesen,
Chunks dekodieren und Blockstates sowie Biome auflösen. Es wird noch nichts
gerendert.

| Schritt | Inhalt | Status |
|---------|--------|--------|
| 1 | Welt-Reader (Region, Chunk, Palette) | **fertig** |
| 2 | Resourcepack: Blockstates, Models, Texturen | offen |
| 3 | Model-Baking und Iso-Sprite-Rasterizer | offen |
| 4 | Metatile-Renderer | offen |
| 5 | Rayon-Parallelisierung, Tiles, WebP | offen |
| 6 | Zoom-Pyramide und `map.json` | offen |
| 7 | Frontend (Vite, TypeScript, Leaflet) | offen |
| 8 | Modelle und Transparenz im Detail | offen |

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

Ein Durchlauf über die gesamte Testwelt:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --scan
```

```
Scan:       316223 Chunks in 174.8 s (1809 Chunks/s), 0 Fehler
```

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

## Entwurfsregel

Bei jeder neuen Dependency und jedem neuen Feature gilt die Frage: Braucht ein
Offline-Renderer für isometrische Minecraft-Rastertiles das wirklich? Wenn
nein, kommt es nicht dazu.
