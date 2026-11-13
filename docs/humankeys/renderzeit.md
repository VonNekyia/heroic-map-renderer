---
title: Wohin die Zeit beim Rendern geht
description: Die Zeit eines Renders, aufgeschlüsselt nach Phasen und Funktionen, für einen vollen Lauf und ein Update von oben und im iso mit Cinematic; was je Phase passiert, welche Funktionen sie tragen und wie viel CPU-Zeit jede braucht; eine Seite, die der Maintainer angefordert hat und die ein Skill erneuert.
code:
  - skills/renderzeit-erneuern/auswerten.py
  - skills/renderzeit-erneuern/phasen.py
  - skills/renderzeit-erneuern/laeufe.py
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/render/heights.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sonne.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
  - renderer/src/world/chunk.rs
  - renderer/src/world/region.rs
---

# Wohin die Zeit beim Rendern geht

Diese Seite zeigt, womit der Renderer seine Zeit verbringt: je Phase, was
dort passiert, welche Funktionen sie tragen und wie viel CPU-Zeit jede
braucht. Gemessen sind ein voller Lauf und ein Update, je von oben und im
iso mit Cinematic. Die Tabellen im Abschnitt Zahlen erzeugt ein Skript aus
einem Profil; unter welchen Bedingungen, steht in der Messung, auf die der
Stand dort verweist.

## Auftrag

Angefordert vom Maintainer am 13.11.2026: „eine Zeitaufschlüsselung was beim
Render exakt wie lange dauert gut granular ggf mit methodennamen“ und „diese
übersicht soll permanent in die doku und ab und zu erneuert werden“. Was für
diese Seite gilt, steht in [`AGENTS.md`](../../AGENTS.md), Regel 28.

## So liest man die Zahlen

- **Die Läufe** rendern die Testwelt mit den Einstellungen, mit denen eine
  Karte im Betrieb läuft: von oben mit Relief, im iso mit Cinematic, beide
  kompakt gepackt und ohne Grafikkarte. Volle Läufe nutzen alle Threads, ein
  Update einen Thread mit niedriger Priorität und 16 geänderten Chunks.
- **CPU-Zeit, nicht Wanduhr:** Ein Profil zählt, in welcher Funktion die
  Kerne rechnen. Mit allen Threads ist die CPU-Zeit ein Vielfaches der Dauer.
  Wie lange der Renderer nach eigener Messung für Vorlauf, Basis und
  Pyramide braucht, steht in der Messung.
- **Eigene Zeit** ist die Zeit in der Funktion selbst, **gesamt** die Zeit
  samt allem, was sie aufruft. Gesamtzeiten überlappen sich und ergeben
  zusammen mehr als 100 %. Gesamt stammt aus den Proben, zu denen das
  Profil die Aufrufer kennt; einem Teil fehlen sie, darum kann gesamt
  knapp unter eigen liegen.
- **Ab 0,5 %** steht eine Funktion einzeln, der Rest einer Phase als Summe.
- **Die Phase folgt dem Namen der Funktion,** nicht ihrem Aufrufer. Zeit im
  Allokator, in der C-Laufzeit und im Kern steht darum in eigenen Zeilen und
  nicht bei der Phase, die sie auslöst.
- **Fremder Code** steht als eigene Zeilen: die Bibliothek, die Kacheln
  packt, der Allokator, das Entpacken, die C-Laufzeit. Vom Kern und seinen
  Treibern kennt das Profil nur Gruppen, keine Funktionen.
- **Das Kodieren der Pyramide** zählt unter „Kodieren“: Die Funktionen sind
  dieselben wie für die Basis. Unter „Pyramide“ stehen das Dekodieren und
  das Verkleinern.

## Was die Zahlen nicht zeigen

- **Gemessen ist unter Windows.** Unter Linux packt die Bibliothek die
  Kacheln nur mit der ersten Stufe von SIMD, siehe
  [Optimierungen](optimierungen.md), „6. SIMD“. Am Packen ändert das
  nichts Messbares, siehe
  [libwebp mit SIMD unter Linux](../messungen/2026-11-13-libwebp-simd-unter-linux.md).
  Ein Profil des ganzen Renders unter Linux gibt es nicht.
- **Der gemessene Build weicht vom Release ab:** Damit das Profil
  Funktionen trennt, verbietet der Skill dem Compiler, eine feste Liste von
  Funktionen einzubetten. Wie viel das an der Dauer ändert, ist nicht
  gemessen.
- **Eingebettete Funktionen** zählen bei ihrem Aufrufer. Das betrifft vor
  allem kleine Funktionen je Pixel und je Block.
- **Der Anfang jedes Laufs fehlt:** Der Profiler hängt sich erst kurz nach
  dem Start an den Prozess.

## Die Phasen

| Phase | Was dort passiert | Wo im Code |
|---|---|---|
| Vorlauf, Assets und Sprites | Liest die Assets, stellt fest, welche Chunks und Kacheln es gibt, und zeichnet je Blockzustand sein Bild vor. | [`tiles.rs`](../../renderer/src/render/tiles.rs), `survey_region`; [`sprites.rs`](../../renderer/src/render/sprites.rs), `SpriteSet`; [`rasterizer.rs`](../../renderer/src/render/rasterizer.rs), `rastern` |
| Lesen und Dekodieren | Liest einen Chunk aus seiner Regionsdatei, entpackt ihn, liest seine Daten und baut daraus die Palette und die Bitmasken. | [`region.rs`](../../renderer/src/world/region.rs), `Region::chunk_nbt`, `decompress`; [`chunk.rs`](../../renderer/src/world/chunk.rs), `Chunk::decode`, `decode_section`; [`metatile.rs`](../../renderer/src/render/metatile.rs), `Loaded::new`, `Masks::of` |
| Licht | Breitet Himmels- und Blocklicht je Chunk aus, wie das Spiel es tut. | [`licht.rs`](../../renderer/src/render/licht.rs), `Ausbreitung::chunk`, `Ausbreitung::breite_aus`; [`metatile.rs`](../../renderer/src/render/metatile.rs), `ChunkCache::licht_slot` |
| Sammeln und Deckung | Sucht je Kachel die Blöcke, von denen etwas zu sehen sein kann, wählt ihr Bild samt Licht und Farbe und ordnet sie von vorn nach hinten. | [`metatile.rs`](../../renderer/src/render/metatile.rs), `von_vorn`, `ChunkCache::sorted_candidates`, `ChunkCache::sprite_at`, `ChunkCache::zelle`, `Deckung` |
| Strahl zur Sonne | Nur im Cinematic: prüft je Pixel, ob die Sonne ihn erreicht. | [`strahl.rs`](../../renderer/src/render/metatile/strahl.rs), `ChunkCache::sonne_im_gang`, `ChunkCache::frei_zur_sonne`; [`sonne.rs`](../../renderer/src/render/sonne.rs), `Sonnenform::trifft` |
| Zeichnen | Legt die Bilder der Blöcke in die Kachel; von oben kommt das Relief dazu, im Cinematic das Licht in HDR mit Ton und Bloom. | [`metatile.rs`](../../renderer/src/render/metatile.rs), `blit_sichtbar`, `mische`, `mit_relief`, `render_hdr`, `blit_hdr`; [`kino.rs`](../../renderer/src/render/kino.rs), `Kino::ton` |
| Kodieren | Packt jede Kachel als WebP. | [`tiles.rs`](../../renderer/src/render/tiles.rs), `encode_webp`; dahinter die Bibliothek libwebp |
| Pyramide | Liest fertige Kacheln wieder ein und verkleinert je vier zu einer Kachel der gröberen Stufe. | [`pyramid.rs`](../../renderer/src/render/pyramid.rs), `merge`, `halbiere`; [`tiles.rs`](../../renderer/src/render/tiles.rs), `decode_webp` |
| Fingerabdruck und Pixel-Hash | Rechnet je Chunk den Abdruck, an dem ein Update Änderungen erkennt, und je Kachel den Hash ihrer Pixel. | [`chunk.rs`](../../renderer/src/world/chunk.rs), `Chunk::abdruck`, `Fnv`; [`pixel.rs`](../../renderer/src/cli/pixel.rs), `Pixel` |
| Höhen und Boden | Schreibt die Höhenkarten, aus denen die Webkarte Koordinaten rechnet. | [`heights.rs`](../../renderer/src/render/heights.rs) |
| Ablegen | Schreibt Kacheln, den Stand und `map.json`. | [`cli.rs`](../../renderer/src/cli.rs), `write_tiles`, `lege_stand_ab` |
| Allokator | Speicher anfordern und freigeben. | mimalloc |
| C-Laufzeit: Kopieren, Füllen, Rechnen | Speicher kopieren und füllen, dazu Rechenfunktionen wie `expf`. | C-Laufzeit im Binär |
| Kern und Dateisystem | Was Windows für den Prozess tut: Dateien lesen und schreiben, Speicherseiten bereitstellen. | – |
| Rest | Was keiner Regel folgt. | – |

## Zahlen

<!-- renderzeit:anfang -->

**Stand:** Messung vom 13.11.2026 an `3100b1b2` (v0.20.0), Bedingungen in [Renderzeit je Funktion](../messungen/2026-11-13-renderzeit-je-funktion.md). Erzeugt von [`auswerten.py`](../../skills/renderzeit-erneuern/auswerten.py), nicht von Hand ändern.

### Anteile je Phase

Anteil an der CPU-Zeit des Laufs.

| Phase | Voller Lauf von oben | Update von oben | Voller Lauf im iso mit Cinematic | Update im iso mit Cinematic |
|---|---|---|---|---|
| Vorlauf, Assets und Sprites | 0,4 % | 1,2 % | 0,1 % | 0,7 % |
| Lesen und Dekodieren | 11,2 % | 14,3 % | 2,4 % | 6,8 % |
| Licht | 18,2 % | 15,6 % | 2,1 % | 3,3 % |
| Sammeln und Deckung | 17,9 % | 15,2 % | 10,4 % | 14,7 % |
| Strahl zur Sonne | – | – | 27,0 % | 37,9 % |
| Zeichnen | 2,4 % | 1,9 % | 15,8 % | 22,2 % |
| Kodieren | 24,6 % | 10,1 % | 29,7 % | 0,8 % |
| Pyramide | 1,5 % | 2,7 % | 1,0 % | 1,3 % |
| Fingerabdruck und Pixel-Hash | 4,8 % | 0,9 % | 0,8 % | 0,5 % |
| Höhen und Boden | 0,3 % | 2,9 % | – | 0,6 % |
| Ablegen | – | 0,1 % | – | 0,1 % |
| Allokator | 2,6 % | 3,8 % | 0,7 % | 1,6 % |
| C-Laufzeit: Kopieren, Füllen, Rechnen | 5,4 % | 6,2 % | 3,8 % | 4,3 % |
| Kern und Dateisystem | 10,5 % | 24,5 % | 6,2 % | 4,5 % |
| Rest | 0,1 % | 0,5 % | – | 0,7 % |
| **CPU-Zeit** | 2663,2 s | 13,9 s | 22796,6 s | 171,8 s |
| **Dauer** | 149,3 s | 6,8 s und 8,2 s | 1161,5 s | 174,2 s |

### Voller Lauf von oben

**Lesen und Dekodieren:** 11,2 % der CPU, 297,5 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `Masks::of` | 2,5 | 2,6 | 65,3 | 68,7 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 2,1 | 2,6 | 55,4 | 70,3 |
| `renderer/src/world/chunk.rs` | `decode_section` | 1,9 | 2,3 | 50,1 | 60,4 |
|  | übrige unter 0,5 % | 4,8 | – | 126,7 | – |

**Licht:** 18,2 % der CPU, 486,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus` | 12,4 | 10,6 | 329,4 | 281,4 |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 4,6 | 15,5 | 122,0 | 412,2 |
| `renderer/src/render/metatile.rs` | `ChunkCache::licht_slot` | 0,9 | 22,3 | 23,2 | 592,7 |
|  | übrige unter 0,5 % | 0,4 | – | 11,4 | – |

**Sammeln und Deckung:** 17,9 % der CPU, 476,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 4,4 | 26,0 | 116,6 | 692,2 |
| `renderer/src/render/metatile.rs` | `umgebung, Closure` | 2,4 | 2,0 | 63,8 | 54,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 1,9 | 34,0 | 50,3 | 906,1 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,4 | 44,0 | 36,4 | 1172,0 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 1,2 | 1,0 | 31,2 | 26,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 1,2 | 2,3 | 30,7 | 61,8 |
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 0,8 | 13,5 | 22,6 | 358,4 |
| `renderer/src/render/metatile.rs` | `ecken_at, Closure` | 0,6 | 0,6 | 17,2 | 14,7 |
| Crate core | `core::hash::sip::write<core::hash::sip::Sip13Rounds>` | 0,6 | – | 17,2 | – |
| `renderer/src/render/metatile.rs` | `tints_at, Closure` | 0,6 | 2,8 | 16,1 | 75,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,5 | 8,8 | 13,7 | 233,5 |
|  | übrige unter 0,5 % | 2,3 | – | 60,3 | – |

**Zeichnen:** 2,4 % der CPU, 62,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `mische` | 0,9 | 0,8 | 25,2 | 21,5 |
| `renderer/src/render/metatile.rs` | `blit_sichtbar` | 0,8 | 1,5 | 21,9 | 40,2 |
| `renderer/src/render/metatile.rs` | `mit_relief` | 0,6 | 0,7 | 14,7 | 17,7 |
|  | übrige unter 0,5 % | 0,0 | – | 1,1 | – |

**Kodieren:** 24,6 % der CPU, 656,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `CalculateBestCacheSize` | 4,9 | 4,4 | 131,6 | 117,8 |
| libwebp (C) | `VP8LHashChainFill` | 4,6 | 6,0 | 123,8 | 159,1 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 3,0 | 3,3 | 80,0 | 87,2 |
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 2,4 | 2,1 | 64,0 | 55,0 |
| libwebp (C) | `VectorMismatch_AVX2` | 1,3 | 1,1 | 34,9 | 29,8 |
| libwebp (C) | `VP8LHistogramStoreRefs` | 1,0 | 0,9 | 25,5 | 22,7 |
| libwebp (C) | `GetBackwardReferences` | 0,9 | 11,4 | 23,8 | 304,4 |
| libwebp (C) | `AnalyzeEntropy` | 0,8 | 0,7 | 20,9 | 18,4 |
| libwebp (C) | `BackwardReferencesLz77` | 0,7 | 0,7 | 19,3 | 18,1 |
| libwebp (C) | `BackwardReferencesRle` | 0,7 | 0,8 | 19,2 | 22,4 |
| libwebp (C) | `ComputeResidualsForTile` | 0,6 | 0,6 | 15,1 | 16,1 |
|  | übrige unter 0,5 % | 3,7 | – | 98,1 | – |

**Pyramide:** 1,5 % der CPU, 39,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `DecodeImageData` | 0,8 | 1,1 | 21,9 | 28,5 |
|  | übrige unter 0,5 % | 0,6 | – | 17,2 | – |

**Fingerabdruck und Pixel-Hash:** 4,8 % der CPU, 129,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/world/chunk.rs` | `Fnv::section` | 3,7 | 3,1 | 97,4 | 82,6 |
| `renderer/src/world/chunk.rs` | `Chunk::abdruck` | 0,8 | 2,7 | 20,1 | 71,5 |
|  | übrige unter 0,5 % | 0,4 | – | 11,6 | – |

**Allokator:** 2,6 % der CPU, 69,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 1,0 | 1,3 | 26,0 | 33,9 |
|  | übrige unter 0,5 % | 1,6 | – | 43,6 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 5,4 % der CPU, 144,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 1,8 | 1,6 | 47,9 | 42,9 |
| C-Laufzeit | `SetSmall8` | 0,9 | 0,8 | 24,6 | 21,5 |
|  | übrige unter 0,5 % | 2,7 | – | 72,2 | – |

**Kern und Dateisystem:** 10,5 % der CPU, 280,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 7,9 | – | 211,0 | – |
| Dateisystem und Platte | – | 1,7 | – | 44,1 | – |
|  | übrige unter 0,5 % | 1,0 | – | 25,5 | – |


### Update von oben

**Vorlauf, Assets und Sprites:** 1,2 % der CPU, 0,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 1,2 | – | 0,2 | – |

**Lesen und Dekodieren:** 14,3 % der CPU, 2,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 3,2 | 5,3 | 0,4 | 0,7 |
| `renderer/src/render/metatile.rs` | `Masks::of` | 2,4 | 3,4 | 0,3 | 0,5 |
| `renderer/src/world/chunk.rs` | `decode_section` | 2,0 | 3,5 | 0,3 | 0,5 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::init_tree` | 0,5 | 0,6 | 0,1 | 0,1 |
| Crate core | `core::str::converts::from_utf8` | 0,5 | 0,6 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 5,6 | – | 0,8 | – |

**Licht:** 15,6 % der CPU, 2,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus` | 10,5 | 12,1 | 1,5 | 1,7 |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 4,3 | 18,4 | 0,6 | 2,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::licht_slot` | 0,5 | 27,2 | 0,1 | 3,8 |
|  | übrige unter 0,5 % | 0,3 | – | 0,0 | – |

**Sammeln und Deckung:** 15,2 % der CPU, 2,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 3,7 | 31,5 | 0,5 | 4,4 |
| `renderer/src/render/metatile.rs` | `umgebung, Closure` | 1,8 | 2,1 | 0,3 | 0,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 1,8 | 40,7 | 0,2 | 5,6 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,0 | 53,8 | 0,1 | 7,5 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 1,0 | 1,2 | 0,1 | 0,2 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 0,9 | 2,6 | 0,1 | 0,4 |
| Crate core | `core::hash::sip::write<core::hash::sip::Sip13Rounds>` | 0,7 | – | 0,1 | – |
| `renderer/src/render/metatile.rs` | `tints_at, Closure` | 0,7 | 3,4 | 0,1 | 0,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 0,6 | 18,5 | 0,1 | 2,6 |
|  | übrige unter 0,5 % | 3,1 | – | 0,4 | – |

**Zeichnen:** 1,9 % der CPU, 0,3 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `mische` | 0,9 | 1,0 | 0,1 | 0,1 |
| `renderer/src/render/metatile.rs` | `blit_sichtbar` | 0,6 | 1,7 | 0,1 | 0,2 |
|  | übrige unter 0,5 % | 0,5 | – | 0,1 | – |

**Kodieren:** 10,1 % der CPU, 1,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `VP8LHashChainFill` | 2,0 | 3,5 | 0,3 | 0,5 |
| libwebp (C) | `CalculateBestCacheSize` | 1,6 | 2,0 | 0,2 | 0,3 |
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 1,4 | 1,6 | 0,2 | 0,2 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 0,9 | 1,5 | 0,1 | 0,2 |
| libwebp (C) | `VectorMismatch_AVX2` | 0,6 | 0,7 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 3,5 | – | 0,5 | – |

**Pyramide:** 2,7 % der CPU, 0,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `DecodeImageData` | 1,7 | 2,7 | 0,2 | 0,4 |
|  | übrige unter 0,5 % | 1,0 | – | 0,1 | – |

**Fingerabdruck und Pixel-Hash:** 0,9 % der CPU, 0,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| ring (Assembler) | `L$oop_shaext` | 0,5 | 0,6 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 0,4 | – | 0,0 | – |

**Höhen und Boden:** 2,9 % der CPU, 0,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::deflate::core::compress_inner` | 2,6 | 3,1 | 0,4 | 0,4 |
|  | übrige unter 0,5 % | 0,3 | – | 0,0 | – |

**Allokator:** 3,8 % der CPU, 0,5 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 1,3 | 2,4 | 0,2 | 0,3 |
|  | übrige unter 0,5 % | 2,5 | – | 0,3 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 6,2 % der CPU, 0,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 1,8 | 2,2 | 0,2 | 0,3 |
| C-Laufzeit | `SetSmall8` | 1,3 | 1,5 | 0,2 | 0,2 |
| C-Laufzeit | `memcmp` | 0,7 | 0,8 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 2,4 | – | 0,3 | – |

**Kern und Dateisystem:** 24,5 % der CPU, 3,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 17,1 | – | 2,4 | – |
| Dateisystem und Platte | – | 5,5 | – | 0,8 | – |
| Laufzeit von Windows | – | 1,0 | – | 0,1 | – |
|  | übrige unter 0,5 % | 0,9 | – | 0,1 | – |

**Rest:** 0,5 % der CPU, 0,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,5 | – | 0,1 | – |


### Voller Lauf im iso mit Cinematic

**Lesen und Dekodieren:** 2,4 % der CPU, 540,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `Masks::of` | 0,6 | 0,7 | 135,4 | 163,0 |
|  | übrige unter 0,5 % | 1,8 | – | 405,6 | – |

**Licht:** 2,1 % der CPU, 480,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus` | 1,4 | 1,4 | 316,5 | 316,8 |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 0,5 | 2,0 | 118,2 | 466,8 |
|  | übrige unter 0,5 % | 0,2 | – | 45,6 | – |

**Sammeln und Deckung:** 10,4 % der CPU, 2372,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 2,2 | 5,8 | 494,5 | 1332,4 |
| `renderer/src/world/chunk.rs` | `Chunk::slot` | 2,0 | 2,0 | 464,8 | 465,2 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,5 | 9,3 | 340,9 | 2130,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::block_at` | 0,9 | 3,2 | 214,4 | 733,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 0,7 | 3,2 | 168,9 | 718,8 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 0,5 | 1,1 | 115,4 | 251,6 |
|  | übrige unter 0,5 % | 2,5 | – | 573,7 | – |

**Strahl zur Sonne:** 27,0 % der CPU, 6153,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::sonne_im_gang` | 8,8 | 27,5 | 1998,3 | 6266,9 |
| `renderer/src/render/sonne.rs` | `Sonnenform::trifft` | 5,7 | 5,8 | 1310,0 | 1318,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_in` | 5,4 | 8,0 | 1233,9 | 1830,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::saeule` | 1,9 | 5,6 | 438,9 | 1272,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sonnen_bits` | 1,8 | 1,9 | 416,5 | 423,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_zur_sonne` | 1,4 | 6,4 | 311,3 | 1462,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::wirkung` | 0,9 | 6,7 | 201,1 | 1524,2 |
| `renderer/src/render/metatile/strahl.rs` | `Gang::springe` | 0,9 | 0,9 | 200,7 | 200,9 |
|  | übrige unter 0,5 % | 0,2 | – | 43,2 | – |

**Zeichnen:** 15,8 % der CPU, 3602,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `render_hdr` | 11,2 | 55,7 | 2549,8 | 12697,8 |
| `renderer/src/render/pyramid.rs` | `to_srgb` | 1,7 | 1,7 | 389,4 | 389,5 |
| `renderer/src/render/metatile.rs` | `Hdr::bild` | 1,1 | 4,6 | 246,7 | 1057,6 |
| `renderer/src/render/kino.rs` | `Kino::ton` | 0,9 | 2,6 | 206,1 | 595,9 |
| `renderer/src/render/kino.rs` | `Kino::bloom` | 0,8 | 0,8 | 172,7 | 178,5 |
|  | übrige unter 0,5 % | 0,2 | – | 37,4 | – |

**Kodieren:** 29,7 % der CPU, 6764,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `CalculateBestCacheSize` | 8,8 | 9,1 | 2017,2 | 2074,8 |
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 4,0 | 4,0 | 906,8 | 914,3 |
| libwebp (C) | `VP8LHashChainFill` | 3,6 | 5,1 | 827,8 | 1159,1 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 2,6 | 3,5 | 603,2 | 787,3 |
| libwebp (C) | `VP8LHistogramStoreRefs` | 1,6 | 1,6 | 353,6 | 359,9 |
| libwebp (C) | `GetBackwardReferences` | 1,2 | 18,2 | 275,3 | 4148,6 |
| libwebp (C) | `BackwardReferencesRle` | 1,1 | 1,4 | 253,5 | 327,7 |
| libwebp (C) | `AnalyzeEntropy` | 1,0 | 1,0 | 219,1 | 224,4 |
| libwebp (C) | `BackwardReferencesLz77` | 0,7 | 0,8 | 158,1 | 183,2 |
| libwebp (C) | `StoreImageToBitMask` | 0,7 | 0,8 | 156,4 | 176,0 |
| libwebp (C) | `GenerateOptimalTree` | 0,7 | 2,2 | 150,4 | 501,6 |
| libwebp (C) | `VectorMismatch_AVX2` | 0,7 | 0,7 | 149,0 | 149,2 |
| libwebp (C) | `BackwardReferencesHashChainFollowChosenPath` | 0,6 | 0,6 | 134,5 | 144,4 |
|  | übrige unter 0,5 % | 2,5 | – | 559,9 | – |

**Pyramide:** 1,0 % der CPU, 237,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/pyramid.rs` | `halbiere` | 0,9 | 0,9 | 197,9 | 197,9 |
|  | übrige unter 0,5 % | 0,2 | – | 39,3 | – |

**Fingerabdruck und Pixel-Hash:** 0,8 % der CPU, 182,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,8 | – | 182,9 | – |

**Allokator:** 0,7 % der CPU, 150,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,7 | – | 150,2 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 3,8 % der CPU, 859,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `Mov0XmmBlocks` | 0,7 | 0,7 | 163,3 | 163,7 |
| C-Laufzeit | `expf` | 0,6 | 0,6 | 133,4 | 133,5 |
|  | übrige unter 0,5 % | 2,5 | – | 563,1 | – |

**Kern und Dateisystem:** 6,2 % der CPU, 1419,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 4,8 | – | 1099,6 | – |
| Dateisystem und Platte | – | 0,7 | – | 165,4 | – |
|  | übrige unter 0,5 % | 0,7 | – | 154,1 | – |


### Update im iso mit Cinematic

**Vorlauf, Assets und Sprites:** 0,7 % der CPU, 1,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,7 | – | 1,2 | – |

**Lesen und Dekodieren:** 6,8 % der CPU, 11,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 1,5 | 2,2 | 2,6 | 3,9 |
| `renderer/src/render/metatile.rs` | `Masks::of` | 1,4 | 1,8 | 2,4 | 3,0 |
| `renderer/src/world/chunk.rs` | `decode_section` | 1,1 | 1,6 | 1,8 | 2,7 |
|  | übrige unter 0,5 % | 2,8 | – | 4,9 | – |

**Licht:** 3,3 % der CPU, 5,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus` | 2,2 | 2,2 | 3,7 | 3,8 |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 0,9 | 3,4 | 1,5 | 5,8 |
|  | übrige unter 0,5 % | 0,2 | – | 0,4 | – |

**Sammeln und Deckung:** 14,7 % der CPU, 25,3 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 2,9 | 11,9 | 5,1 | 20,4 |
| `renderer/src/world/chunk.rs` | `Chunk::slot` | 2,9 | 3,0 | 5,1 | 5,2 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 2,1 | 16,1 | 3,7 | 27,7 |
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 1,1 | 5,0 | 1,8 | 8,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::block_at` | 1,1 | 4,3 | 1,8 | 7,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,8 | 5,6 | 1,3 | 9,7 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 0,7 | 1,6 | 1,2 | 2,7 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 0,6 | 0,6 | 1,0 | 1,0 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 0,6 | 7,2 | 0,9 | 12,4 |
|  | übrige unter 0,5 % | 2,0 | – | 3,4 | – |

**Strahl zur Sonne:** 37,9 % der CPU, 65,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::sonne_im_gang` | 12,5 | 39,0 | 21,5 | 67,0 |
| `renderer/src/render/sonne.rs` | `Sonnenform::trifft` | 8,2 | 8,4 | 14,1 | 14,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_in` | 7,5 | 12,8 | 12,8 | 22,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sonnen_bits` | 3,0 | 3,1 | 5,1 | 5,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::saeule` | 2,2 | 8,6 | 3,8 | 14,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_zur_sonne` | 2,1 | 10,5 | 3,5 | 18,1 |
| `renderer/src/render/metatile/strahl.rs` | `Gang::springe` | 1,3 | 1,3 | 2,2 | 2,2 |
| `renderer/src/render/metatile.rs` | `ChunkCache::wirkung` | 1,0 | 9,4 | 1,6 | 16,2 |
|  | übrige unter 0,5 % | 0,3 | – | 0,4 | – |

**Zeichnen:** 22,2 % der CPU, 38,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `render_hdr` | 15,5 | 83,4 | 26,7 | 143,3 |
| `renderer/src/render/pyramid.rs` | `to_srgb` | 2,7 | 2,8 | 4,6 | 4,7 |
| `renderer/src/render/kino.rs` | `Kino::ton` | 1,5 | 4,3 | 2,6 | 7,3 |
| `renderer/src/render/metatile.rs` | `Hdr::bild` | 1,4 | 6,9 | 2,4 | 11,8 |
| `renderer/src/render/kino.rs` | `Kino::bloom` | 0,9 | 0,9 | 1,6 | 1,6 |
|  | übrige unter 0,5 % | 0,2 | – | 0,4 | – |

**Kodieren:** 0,8 % der CPU, 1,3 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,8 | – | 1,3 | – |

**Pyramide:** 1,3 % der CPU, 2,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/pyramid.rs` | `halbiere` | 1,1 | 1,1 | 1,9 | 1,9 |
|  | übrige unter 0,5 % | 0,2 | – | 0,3 | – |

**Fingerabdruck und Pixel-Hash:** 0,5 % der CPU, 0,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,5 | – | 0,9 | – |

**Höhen und Boden:** 0,6 % der CPU, 1,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::deflate::core::compress_inner` | 0,6 | 0,6 | 1,0 | 1,0 |
|  | übrige unter 0,5 % | 0,1 | – | 0,1 | – |

**Allokator:** 1,6 % der CPU, 2,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 0,7 | 1,0 | 1,1 | 1,7 |
|  | übrige unter 0,5 % | 1,0 | – | 1,7 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 4,3 % der CPU, 7,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 0,9 | 0,9 | 1,5 | 1,6 |
| C-Laufzeit | `expf` | 0,9 | 0,9 | 1,5 | 1,5 |
|  | übrige unter 0,5 % | 2,5 | – | 4,4 | – |

**Kern und Dateisystem:** 4,5 % der CPU, 7,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 3,6 | – | 6,2 | – |
| Dateisystem und Platte | – | 0,6 | – | 1,1 | – |
|  | übrige unter 0,5 % | 0,2 | – | 0,4 | – |

**Rest:** 0,7 % der CPU, 1,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,7 | – | 1,2 | – |

<!-- renderzeit:ende -->

## Erneuern

Der Skill [`renderzeit-erneuern`](../../skills/renderzeit-erneuern/SKILL.md)
misst die Läufe neu, schreibt eine neue Messung und setzt die Tabellen
dieser Seite neu. Die Läufe, die Phasen, die Schwelle und die Spalten
bleiben dabei, wie sie sind.
