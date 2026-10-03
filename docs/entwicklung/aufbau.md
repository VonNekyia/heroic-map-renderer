---
title: Aufbau des Codes
description: Welche Datei was tut, vom Welt-Reader über Assets und Sprites bis zu Kacheln, Pyramide, Grafikkarte und Frontend, und wie die Daten durchlaufen.
code:
  - renderer/src/lib.rs
  - renderer/src/main.rs
  - renderer/src/cli.rs
  - renderer/src/world/mod.rs
  - renderer/src/assets/mod.rs
  - renderer/src/render/mod.rs
  - renderer/Cargo.toml
  - web/src/main.ts
---

# Aufbau des Codes

Ein Rust-Crate unter `renderer/` mit einer Bibliothek (`terranova_render`)
und einem Binär, dazu das Frontend unter `web/`. Die Daten laufen in einer
Richtung: Welt und Assets lesen, Sprites vorab rastern, Kacheln rendern und
kodieren, Zoomstufen stapeln, im Browser anzeigen. Die Bibliothek hat drei
Module, `world`, `assets` und `render`; die Kommandozeile in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs) verbindet sie.

```
Minecraft World + Resource Pack  ->  Rust Renderer  ->  WebP Tiles  ->  Leaflet
```

## `renderer/src/world/`: die Welt lesen

| Datei | Inhalt |
|---|---|
| `mod.rs` | `World`: Weltwurzel, Dimension, Regionen, Seed, siehe [Welten und Kennung](../benutzung/welten.md) |
| `region.rs` | Regionsdateien: Chunk-Tabelle, Sektoren, ausgelagerte `.mcc`-Chunks |
| `chunk.rs` | ein Chunk aus NBT: Sections, Blöcke, Biome, Blockentities mit Daten |
| `palette.rs` | `BlockState` und die gepackten Paletten-Indizes einer Section |
| `biomzoom.rs`, `Biomwerte.java` | das Biom je Block wie `BiomeManager.getBiome` und die Sollwerte dafür aus dem Spiel, siehe [Biomfarben](../renderer/biomfarben.md) |

## `renderer/src/assets/`: das Resourcepack

| Datei | Inhalt |
|---|---|
| `mod.rs` | `Assets`: Wurzeln stapeln, Blockstates zu gebackenen Modellen auflösen, Missing-Würfel |
| `pack.rs` | eine Wurzel auflisten wie der Client, siehe [Packs und Wurzeln](../renderer/packs.md) |
| `blockstate.rs` | Blockstate-Dateien lesen, dazu die Tabellen aus dem Spiel, siehe [Blockstates](../renderer/blockstates.md) und [Erzeugte Tabellen](tabellen.md) |
| `model.rs` | Modelle lesen wie `CuboidModel`, siehe [Modelle und Texturen](../renderer/modelle-und-texturen.md) |
| `texture.rs` | Texturen und `.mcmeta` |
| `baker.rs` | Elemente zu Vierecken backen: Drehungen, `uvlock`, Flüssigkeitsflächen |
| `fluid.rs` | Wasser und Lava als Würfel, siehe [Wasser und Licht](../renderer/wasser-und-licht.md) |
| `blockentity.rs` | was das Spiel für Truhen, Banner und die übrigen Blockentities aus Modellen zeichnet, aus `blockentities.txt`, mit Mustern und Scherben; die Bannermuster des Spiels und der Datenwurzeln, siehe [Blockentities](../renderer/blockentities.md) |
| `colors.rs` | Biomfarben, siehe [Biomfarben](../renderer/biomfarben.md) |
| `dimension.rs` | der Typ der gezeichneten Dimension, aus `dimensionstypen.txt` und den Datenwurzeln, siehe [Dimensionstypen](../renderer/dimensionstypen.md) |
| `noise.rs` | das Rauschen des Sumpfgrases, siehe [Biomfarben](../renderer/biomfarben.md), „Sumpfgras“ |
| `*.txt`, `*.java` | die aus dem Spiel erzeugten Tabellen und ihre Generatoren, siehe [Erzeugte Tabellen](tabellen.md) |

## `renderer/src/render/`: Bilder machen

| Datei | Inhalt |
|---|---|
| `projection.rs` | die Kameras und ihre Projektion, siehe [Die Kamera](../renderer/kamera.md) |
| `rasterizer.rs` | ein gebackenes Modell zu einem Sprite rastern, Helligkeit, AO-Karte, siehe [Rastern ohne Nähte](../renderer/naehte.md) |
| `sprites.rs` | die Sprite-Tabelle: Familien, Fassungen, Varianten, siehe [Sprites und Deckung](../renderer/sprites-und-deckung.md) |
| `metatile.rs` | eine Kachel rendern: Chunk-Cache, Bitmasken, Kandidaten, Deckungsmaske, Licht, weiche Beleuchtung, Biom je Block, Blit, siehe [Der Weg einer Kachel](../renderer/renderpfad.md) |
| `look.rs` | die Werte des Looks von Cinematic an einer Stelle und ihr Fingerabdruck, siehe [Cinematic](../renderer/cinematic.md), „Werte des Looks“ |
| `kino.rs` | das Licht von Cinematic in HDR, die Sonne, Spiegelung und Dichte des Wassers, Bloom, Wärme und der Ton am Ende, siehe [Cinematic](../renderer/cinematic.md) |
| `sonne.rs` | was ein Block dem Strahl zur Sonne in den Weg stellt und der Test eines Strahls dagegen, siehe [Cinematic](../renderer/cinematic.md), „Schatten“ |
| `metatile/strahl.rs` | der Strahl zur Sonne durch den Chunk-Cache: schneller Gang und langsamer Bezug, siehe [Cinematic](../renderer/cinematic.md), „Der schnelle Gang“ |
| `licht.rs` | Himmels- und Blocklicht ausbreiten wie das Spiel, siehe [Wasser und Licht](../renderer/wasser-und-licht.md), „Licht ausbreiten“ |
| `tint.rs` | die Farben der Biome und ihre Mischung über Biomgrenzen, siehe [Biomfarben](../renderer/biomfarben.md) |
| `tiles.rs` | Kachelraster, Vorlauf (`survey`), WebP (`encode_webp`) |
| `pyramid.rs` | Zoomstufen verkleinern, `map.json`, Kennung der Welt |
| `gpu.rs`, `gpu.wgsl` | Zeichnen auf der Grafikkarte, siehe [Grafikkarte](../benutzung/grafikkarte.md) |

## Binär und Bau

| Datei | Inhalt |
|---|---|
| `main.rs` | Einstieg, mimalloc als Allokator |
| `cli.rs` | Schalter, Export, Pyramide, Fortsetzen, Echtzeitschutz, siehe [Schalter und Beispiele](../benutzung/schalter.md) |
| `lib.rs` | die drei Module der Bibliothek |
| `build.rs`, `segmentheap.manifest` | unter Windows das Manifest mit dem Segment-Heap |
| `Cargo.toml`, `deny.toml` | Abhängigkeiten und ihre Lizenzen |

## `web/`: das Frontend

| Datei | Inhalt |
|---|---|
| `src/main.ts` | Leaflet, Koordinatensystem aus `map.json`, Höhenkarten, Anzeige der Koordinaten, siehe [Frontend](../frontend.md) |
| `src/pick.ts` | welcher Block an einem Bildpunkt zu sehen ist, und sein Umriss, siehe [Frontend](../frontend.md), „Koordinaten“ |
| `src/style.css`, `index.html` | die Seite |
| `vite.config.ts` | Devserver und Build ohne `public/tiles` |
| `public/tiles-demo/` | ein kleiner Kachelbaum, Fixture des Smoke-Tests |
| `tests/smoke.spec.ts`, `tests/pick.spec.ts`, `playwright.config.ts` | der Smoke-Test und der Strahl |

## Wie der Code entstand

Der Plan hatte acht Schritte, alle fertig; danach kam je Thema eine PR.

| PR | Inhalt | Entscheidungen |
|---|---|---|
| #1 | Schritt 1: Welt-Reader (Region, Chunk, Palette) | |
| #2 | Schritt 2: Resourcepack, Blockstates, Modelle, Texturen | |
| #3 | Schritt 3: Model-Baking und isometrischer Sprite-Rasterizer | |
| #4 | Schritt 4: Metatile-Renderer | [0001](../entscheidungen/0001-zeichenreihenfolge-statt-tiefenpuffer.md), [0002](../entscheidungen/0002-deckend-entscheidet-das-bild.md) |
| #5 | Schritt 5: Kacheln, parallel, als WebP | [0003](../entscheidungen/0003-vorlauf-vor-dem-rendern.md), [0004](../entscheidungen/0004-webp-verlustfrei.md) |
| #6 | Schritt 6: Zoompyramide und `map.json` | [0005](../entscheidungen/0005-zoomstufen-haengen-an-der-welt.md) |
| #7 | Schritt 7: Frontend mit Vite, TypeScript und Leaflet | [0006](../entscheidungen/0006-kacheln-unter-web-public.md), [0007](../entscheidungen/0007-karteneinheit-ist-ein-pixel-der-basis.md) |
| #8 | Schritt 8: Modelle und Transparenz im Detail | 0008 bis 0014 |
| #9 | Erster Vollrender: Pyramide aus Kacheln, native Stufen auf Wunsch, Live-Ansicht | 0015 bis 0018 |
| #10 | Renderer fünf- bis zehnmal so schnell | 0019 bis 0022 |
| #11 | Kacheln auf der Grafikkarte | [0023](../entscheidungen/0023-zeichnen-auf-der-grafikkarte.md), [0024](../entscheidungen/0024-vulkan-zuerst-ohne-gl.md) |
| #12 | Die grossen Posten, zweite Runde | 0025 bis 0027 |
| #16 | Kacheln verlustfrei mit libwebp (#13) | [0028](../entscheidungen/0028-libwebp-statt-image.md), [0029](../entscheidungen/0029-segment-heap-fuer-libwebp.md) |
| #17 | Wasser im Licht des Spiels (#14) | [0030](../entscheidungen/0030-licht-je-block.md) |
| #18 | Weiche Beleuchtung wie im Spiel (#15) | [0031](../entscheidungen/0031-eigene-tabellen-statt-der-masken.md), [0032](../entscheidungen/0032-weiche-beleuchtung-zuerst-fuer-volle-wuerfel.md) |
| #20 | Regeln in `AGENTS.md`, Workflows in `skills/` | |

Alle Entscheidungen stehen in [`docs/index.md`](../index.md).

## Neue Abhängigkeiten und Features

Für jede neue Abhängigkeit und jedes neue Feature gilt die Entwurfsregel in
[`AGENTS.md`](../../AGENTS.md), „Entwurf“.
