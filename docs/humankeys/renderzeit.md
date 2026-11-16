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

**Stand:** Messung vom 15.11.2026 an `51b8c144` (v0.20.2), Bedingungen in [Renderzeit je Funktion](../messungen/2026-11-15-renderzeit-je-funktion.md). Erzeugt von [`auswerten.py`](../../skills/renderzeit-erneuern/auswerten.py), nicht von Hand ändern.

### Anteile je Phase

Anteil an der CPU-Zeit des Laufs.

| Phase | Voller Lauf von oben | Update von oben | Voller Lauf im iso mit Cinematic | Update im iso mit Cinematic |
|---|---|---|---|---|
| Vorlauf, Assets und Sprites | 0,5 % | 1,5 % | 0,1 % | 1,3 % |
| Lesen und Dekodieren | 13,7 % | 18,3 % | 2,8 % | 8,5 % |
| Licht | 14,2 % | 12,2 % | 1,6 % | 1,9 % |
| Sammeln und Deckung | 17,1 % | 14,8 % | 12,0 % | 14,1 % |
| Strahl zur Sonne | – | – | 30,5 % | 35,2 % |
| Zeichnen | 2,9 % | 2,4 % | 18,2 % | 21,4 % |
| Kodieren | 21,8 % | 8,9 % | 20,5 % | 1,2 % |
| Pyramide | 1,9 % | 3,3 % | 1,2 % | 1,5 % |
| Fingerabdruck und Pixel-Hash | 5,9 % | 1,1 % | 0,9 % | 0,6 % |
| Höhen und Boden | 0,4 % | 4,0 % | – | 1,1 % |
| Ablegen | – | 0,2 % | – | 0,2 % |
| Allokator | 3,0 % | 4,2 % | 0,7 % | 2,0 % |
| C-Laufzeit: Kopieren, Füllen, Rechnen | 6,6 % | 7,3 % | 4,4 % | 4,8 % |
| Kern und Dateisystem | 12,1 % | 21,3 % | 6,9 % | 4,6 % |
| Rest | 0,1 % | 0,6 % | – | 1,5 % |
| **CPU-Zeit** | 2318,0 s | 9,7 s | 20895,7 s | 70,8 s |
| **Dauer** | 115,1 s | 6,2 s und 6,0 s | 980,4 s | 74,2 s |

### Voller Lauf von oben

**Lesen und Dekodieren:** 13,7 % der CPU, 316,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `Masks::of` | 3,1 | 3,1 | 72,2 | 72,6 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 2,5 | 3,1 | 58,8 | 71,7 |
| `renderer/src/world/chunk.rs` | `decode_section` | 2,3 | 2,7 | 54,3 | 62,1 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::apply_match` | 0,5 | 0,6 | 12,0 | 14,5 |
| Crate core | `core::str::converts::from_utf8` | 0,5 | 0,4 | 11,8 | 9,7 |
|  | übrige unter 0,5 % | 4,6 | – | 107,8 | – |

**Licht:** 14,2 % der CPU, 328,5 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus<true>` | 7,6 | – | 175,1 | – |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 5,6 | 12,1 | 130,7 | 280,6 |
|  | übrige unter 0,5 % | 1,0 | – | 22,8 | – |

**Sammeln und Deckung:** 17,1 % der CPU, 396,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 2,3 | 21,3 | 52,4 | 493,7 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 2,1 | 29,7 | 48,7 | 687,8 |
| `renderer/src/render/metatile.rs` | `umgebung, Closure` | 1,8 | 1,5 | 41,6 | 34,6 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,6 | 41,5 | 37,7 | 961,9 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 1,4 | 1,2 | 33,1 | 27,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 1,4 | 2,8 | 32,5 | 64,3 |
| Crate core | `core::hash::sip::write<core::hash::sip::Sip13Rounds>` | 0,8 | – | 19,1 | – |
| `renderer/src/render/metatile.rs` | `ecken_aus, Closure` | 0,8 | 0,7 | 18,3 | 15,2 |
| `renderer/src/render/metatile.rs` | `tints_at, Closure` | 0,8 | 3,4 | 17,5 | 78,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 0,7 | 15,7 | 17,0 | 363,7 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,6 | 10,4 | 15,0 | 241,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::expose` | 0,6 | 0,5 | 13,6 | 11,6 |
| `renderer/src/world/chunk.rs` | `Chunk::slot` | 0,5 | 0,4 | 12,3 | 10,2 |
|  | übrige unter 0,5 % | 1,6 | – | 38,1 | – |

**Zeichnen:** 2,9 % der CPU, 67,3 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `mische` | 1,1 | 0,9 | 26,4 | 21,9 |
| `renderer/src/render/metatile.rs` | `blit_sichtbar` | 1,0 | 1,8 | 23,7 | 41,6 |
| `renderer/src/render/metatile.rs` | `mit_relief` | 0,7 | 0,8 | 15,9 | 18,4 |
|  | übrige unter 0,5 % | 0,1 | – | 1,4 | – |

**Kodieren:** 21,8 % der CPU, 504,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `VP8LHashChainFill` | 5,3 | 7,0 | 123,5 | 161,8 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 4,0 | 4,3 | 93,7 | 100,3 |
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 3,0 | 2,5 | 70,6 | 59,1 |
| libwebp (C) | `VectorMismatch_AVX2` | 1,5 | 1,2 | 34,5 | 28,7 |
| libwebp (C) | `BackwardReferencesLz77` | 0,9 | 0,8 | 20,7 | 18,9 |
| libwebp (C) | `GenerateOptimalTree` | 0,8 | 1,5 | 17,8 | 35,1 |
| libwebp (C) | `ComputeResidualsForTile` | 0,7 | 0,7 | 16,2 | 16,8 |
| libwebp (C) | `VP8LHistogramStoreRefs` | 0,7 | 0,6 | 15,5 | 13,7 |
| libwebp (C) | `BackwardReferencesHashChainFollowChosenPath` | 0,6 | 0,6 | 14,2 | 13,9 |
| libwebp (C) | `GetBackwardReferences` | 0,5 | 7,1 | 12,5 | 164,0 |
| libwebp (C) | `StoreImageToBitMask` | 0,5 | 0,5 | 11,9 | 11,1 |
|  | übrige unter 0,5 % | 3,2 | – | 73,5 | – |

**Pyramide:** 1,9 % der CPU, 43,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `DecodeImageData` | 1,1 | 1,3 | 24,9 | 31,0 |
|  | übrige unter 0,5 % | 0,8 | – | 18,3 | – |

**Fingerabdruck und Pixel-Hash:** 5,9 % der CPU, 136,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/world/chunk.rs` | `Fnv::section` | 4,5 | 3,7 | 105,4 | 85,6 |
| `renderer/src/world/chunk.rs` | `Chunk::abdruck` | 0,8 | 3,2 | 19,2 | 74,9 |
| ring (Assembler) | `L$oop_shaext` | 0,5 | 0,4 | 12,0 | 10,0 |
|  | übrige unter 0,5 % | 0,0 | – | 0,2 | – |

**Allokator:** 3,0 % der CPU, 69,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 1,1 | 1,4 | 25,0 | 31,8 |
|  | übrige unter 0,5 % | 1,9 | – | 44,0 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 6,6 % der CPU, 152,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 2,2 | 2,0 | 51,8 | 45,2 |
| C-Laufzeit | `SetSmall8` | 1,0 | 0,9 | 23,4 | 20,0 |
| C-Laufzeit | `memcmp` | 0,5 | 0,4 | 12,3 | 10,2 |
| C-Laufzeit | `Mov0XmmBlocks` | 0,5 | 0,4 | 11,9 | 9,9 |
|  | übrige unter 0,5 % | 2,3 | – | 52,7 | – |

**Kern und Dateisystem:** 12,1 % der CPU, 280,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 9,2 | – | 212,7 | – |
| Dateisystem und Platte | – | 1,9 | – | 44,9 | – |
|  | übrige unter 0,5 % | 1,0 | – | 23,2 | – |


### Update von oben

**Vorlauf, Assets und Sprites:** 1,5 % der CPU, 0,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 1,5 | – | 0,1 | – |

**Lesen und Dekodieren:** 18,3 % der CPU, 1,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 4,3 | 6,4 | 0,4 | 0,6 |
| `renderer/src/render/metatile.rs` | `Masks::of` | 2,9 | 3,7 | 0,3 | 0,4 |
| `renderer/src/world/chunk.rs` | `decode_section` | 2,7 | 4,1 | 0,3 | 0,4 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::init_tree` | 0,7 | 0,7 | 0,1 | 0,1 |
| Crate cesu8 | `cesu8::from_cesu8_internal` | 0,7 | 1,4 | 0,1 | 0,1 |
| Crate core | `core::str::converts::from_utf8` | 0,7 | 0,7 | 0,1 | 0,1 |
| Crate miniz_oxide | `miniz_oxide::inflate::core::apply_match` | 0,5 | 1,1 | 0,1 | 0,1 |
| Crate serde_core | `serde_core::de::impls::deserialize::visit_seq<world::chunk::PaletteEntry,fastnbt::de::L...` | 0,5 | – | 0,0 | – |
|  | übrige unter 0,5 % | 5,2 | – | 0,5 | – |

**Licht:** 12,2 % der CPU, 1,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus<true>` | 6,4 | – | 0,6 | – |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 5,3 | 13,6 | 0,5 | 1,3 |
|  | übrige unter 0,5 % | 0,6 | – | 0,1 | – |

**Sammeln und Deckung:** 14,8 % der CPU, 1,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 2,3 | 25,1 | 0,2 | 2,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 1,7 | 33,8 | 0,2 | 3,3 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 1,4 | 1,4 | 0,1 | 0,1 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,3 | 48,6 | 0,1 | 4,7 |
| `renderer/src/render/metatile.rs` | `umgebung, Closure` | 1,2 | 1,3 | 0,1 | 0,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 1,0 | 3,0 | 0,1 | 0,3 |
| Crate core | `core::hash::sip::write<core::hash::sip::Sip13Rounds>` | 0,8 | – | 0,1 | – |
| `renderer/src/render/metatile.rs` | `tints_at, Closure` | 0,8 | 3,8 | 0,1 | 0,4 |
| `renderer/src/render/metatile.rs` | `ecken_aus, Closure` | 0,7 | 0,7 | 0,1 | 0,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,6 | 13,4 | 0,1 | 1,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::expose` | 0,5 | 0,5 | 0,0 | 0,1 |
|  | übrige unter 0,5 % | 2,6 | – | 0,3 | – |

**Zeichnen:** 2,4 % der CPU, 0,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `mische` | 1,0 | 1,1 | 0,1 | 0,1 |
| `renderer/src/render/metatile.rs` | `blit_sichtbar` | 0,8 | 1,9 | 0,1 | 0,2 |
| `renderer/src/render/metatile.rs` | `mit_relief` | 0,5 | 0,8 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 0,0 | – | 0,0 | – |

**Kodieren:** 8,9 % der CPU, 0,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `VP8LHashChainFill` | 2,3 | 3,7 | 0,2 | 0,4 |
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 1,6 | 1,7 | 0,2 | 0,2 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 1,2 | 1,7 | 0,1 | 0,2 |
| libwebp (C) | `VectorMismatch_AVX2` | 0,7 | 0,8 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 3,1 | – | 0,3 | – |

**Pyramide:** 3,3 % der CPU, 0,3 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `DecodeImageData` | 2,1 | 3,0 | 0,2 | 0,3 |
|  | übrige unter 0,5 % | 1,1 | – | 0,1 | – |

**Fingerabdruck und Pixel-Hash:** 1,1 % der CPU, 0,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| ring (Assembler) | `L$oop_shaext` | 0,7 | 0,8 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 0,4 | – | 0,0 | – |

**Höhen und Boden:** 4,0 % der CPU, 0,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::deflate::core::compress_inner` | 3,5 | 3,8 | 0,3 | 0,4 |
|  | übrige unter 0,5 % | 0,4 | – | 0,0 | – |

**Allokator:** 4,2 % der CPU, 0,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 1,4 | 2,4 | 0,1 | 0,2 |
|  | übrige unter 0,5 % | 2,8 | – | 0,3 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 7,3 % der CPU, 0,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 2,1 | 2,3 | 0,2 | 0,2 |
| C-Laufzeit | `SetSmall8` | 1,4 | 1,5 | 0,1 | 0,1 |
| C-Laufzeit | `memcmp` | 0,7 | 0,7 | 0,1 | 0,1 |
| C-Laufzeit | `memcpy_repmovs` | 0,5 | 0,6 | 0,1 | 0,1 |
|  | übrige unter 0,5 % | 2,6 | – | 0,3 | – |

**Kern und Dateisystem:** 21,3 % der CPU, 2,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 14,9 | – | 1,4 | – |
| Dateisystem und Platte | – | 4,8 | – | 0,5 | – |
| Laufzeit von Windows | – | 0,8 | – | 0,1 | – |
|  | übrige unter 0,5 % | 0,7 | – | 0,1 | – |

**Rest:** 0,6 % der CPU, 0,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,6 | – | 0,1 | – |


### Voller Lauf im iso mit Cinematic

**Lesen und Dekodieren:** 2,8 % der CPU, 578,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `Masks::of` | 0,7 | 0,9 | 150,3 | 179,6 |
|  | übrige unter 0,5 % | 2,1 | – | 428,5 | – |

**Licht:** 1,6 % der CPU, 339,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus<true>` | 0,8 | – | 169,2 | – |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 0,6 | 1,6 | 130,2 | 333,0 |
|  | übrige unter 0,5 % | 0,2 | – | 39,7 | – |

**Sammeln und Deckung:** 12,0 % der CPU, 2503,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 2,5 | 6,8 | 528,4 | 1422,6 |
| `renderer/src/world/chunk.rs` | `Chunk::slot` | 2,4 | 2,4 | 494,7 | 495,9 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 1,8 | 9,9 | 370,2 | 2064,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::block_at` | 1,1 | 3,8 | 237,9 | 794,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 0,6 | 1,3 | 127,9 | 276,9 |
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 0,6 | 2,6 | 126,4 | 539,6 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,6 | 2,8 | 118,8 | 575,5 |
|  | übrige unter 0,5 % | 2,4 | – | 498,7 | – |

**Strahl zur Sonne:** 30,5 % der CPU, 6377,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::sonne_im_gang` | 9,4 | 31,2 | 1963,6 | 6515,0 |
| `renderer/src/render/sonne.rs` | `Sonnenform::trifft` | 6,8 | 6,9 | 1426,3 | 1438,0 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_in` | 6,3 | 9,4 | 1317,7 | 1954,8 |
| `renderer/src/render/metatile.rs` | `ChunkCache::saeule` | 2,2 | 6,4 | 461,7 | 1345,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sonnen_bits` | 2,1 | 2,2 | 443,6 | 451,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_zur_sonne` | 1,6 | 7,5 | 331,9 | 1557,3 |
| `renderer/src/render/metatile/strahl.rs` | `Gang::springe` | 1,0 | 1,0 | 212,9 | 213,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::wirkung` | 0,8 | 7,7 | 173,6 | 1617,0 |
|  | übrige unter 0,5 % | 0,2 | – | 46,2 | – |

**Zeichnen:** 18,2 % der CPU, 3796,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `render_hdr` | 12,8 | 62,8 | 2673,8 | 13122,1 |
| `renderer/src/render/pyramid.rs` | `to_srgb` | 2,0 | 2,0 | 420,2 | 421,3 |
| `renderer/src/render/metatile.rs` | `Hdr::bild` | 1,2 | 5,4 | 258,0 | 1128,3 |
| `renderer/src/render/kino.rs` | `Kino::ton` | 1,1 | 3,1 | 221,9 | 643,7 |
| `renderer/src/render/kino.rs` | `Kino::bloom` | 0,9 | 0,9 | 183,3 | 189,7 |
|  | übrige unter 0,5 % | 0,2 | – | 39,0 | – |

**Kodieren:** 20,5 % der CPU, 4287,7 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| libwebp (C) | `GetCombinedEntropyUnrefined_C` | 4,8 | 4,8 | 999,4 | 1008,7 |
| libwebp (C) | `VP8LHashChainFill` | 3,9 | 5,8 | 821,4 | 1221,4 |
| libwebp (C) | `BackwardReferencesHashChainDistanceOnly` | 3,1 | 4,0 | 640,4 | 844,0 |
| libwebp (C) | `GenerateOptimalTree` | 1,4 | 3,3 | 294,4 | 687,9 |
| libwebp (C) | `VP8LHistogramStoreRefs` | 1,0 | 1,0 | 211,3 | 217,0 |
| libwebp (C) | `BackwardReferencesLz77` | 0,8 | 1,0 | 171,4 | 201,1 |
| libwebp (C) | `StoreImageToBitMask` | 0,8 | 0,9 | 167,5 | 190,6 |
| libwebp (C) | `BackwardReferencesHashChainFollowChosenPath` | 0,7 | 0,9 | 153,1 | 190,5 |
| libwebp (C) | `VectorMismatch_AVX2` | 0,7 | 0,7 | 141,3 | 141,7 |
| libwebp (C) | `GetBackwardReferences` | 0,6 | 7,5 | 117,6 | 1571,5 |
|  | übrige unter 0,5 % | 2,7 | – | 569,8 | – |

**Pyramide:** 1,2 % der CPU, 257,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/pyramid.rs` | `halbiere` | 1,0 | 1,0 | 214,0 | 214,7 |
|  | übrige unter 0,5 % | 0,2 | – | 43,6 | – |

**Fingerabdruck und Pixel-Hash:** 0,9 % der CPU, 192,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/world/chunk.rs` | `Fnv::section` | 0,5 | 0,5 | 106,6 | 103,3 |
|  | übrige unter 0,5 % | 0,4 | – | 85,4 | – |

**Allokator:** 0,7 % der CPU, 153,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 0,7 | – | 153,9 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 4,4 % der CPU, 929,5 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `Mov0XmmBlocks` | 0,9 | 0,9 | 180,8 | 181,4 |
| C-Laufzeit | `expf` | 0,7 | 0,7 | 145,2 | 145,5 |
| C-Laufzeit | `qsort` | 0,6 | 0,9 | 130,6 | 192,3 |
| C-Laufzeit | `MoveSmall4` | 0,5 | 0,6 | 112,0 | 132,1 |
|  | übrige unter 0,5 % | 1,7 | – | 360,9 | – |

**Kern und Dateisystem:** 6,9 % der CPU, 1447,6 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 5,4 | – | 1137,7 | – |
| Dateisystem und Platte | – | 0,8 | – | 175,0 | – |
|  | übrige unter 0,5 % | 0,6 | – | 135,0 | – |


### Update im iso mit Cinematic

**Vorlauf, Assets und Sprites:** 1,3 % der CPU, 0,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 1,3 | – | 0,9 | – |

**Lesen und Dekodieren:** 8,5 % der CPU, 6,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::inflate::core::decompress_fast` | 2,2 | 3,2 | 1,6 | 2,3 |
| `renderer/src/render/metatile.rs` | `Masks::of` | 1,5 | 1,8 | 1,0 | 1,3 |
| `renderer/src/world/chunk.rs` | `decode_section` | 1,3 | 1,9 | 0,9 | 1,3 |
|  | übrige unter 0,5 % | 3,6 | – | 2,6 | – |

**Licht:** 1,9 % der CPU, 1,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/licht.rs` | `Ausbreitung::breite_aus<true>` | 1,0 | – | 0,7 | – |
| `renderer/src/render/licht.rs` | `Ausbreitung::chunk` | 0,8 | 2,0 | 0,6 | 1,4 |
|  | übrige unter 0,5 % | 0,1 | – | 0,1 | – |

**Sammeln und Deckung:** 14,1 % der CPU, 10,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/world/chunk.rs` | `Chunk::slot` | 3,0 | 3,0 | 2,1 | 2,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::slot` | 3,0 | 12,7 | 2,1 | 9,0 |
| `renderer/src/render/metatile.rs` | `von_vorn` | 2,0 | 14,6 | 1,4 | 10,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::block_at` | 1,0 | 4,2 | 0,7 | 3,0 |
| `renderer/src/render/metatile.rs` | `ChunkCache::zelle` | 0,7 | 3,3 | 0,5 | 2,3 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sorted_candidates` | 0,7 | 6,3 | 0,5 | 4,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::biome_of` | 0,6 | 1,5 | 0,4 | 1,0 |
| `renderer/src/world/biomzoom.rs` | `zoom` | 0,6 | 0,6 | 0,4 | 0,4 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sprite_at` | 0,5 | 5,2 | 0,4 | 3,7 |
|  | übrige unter 0,5 % | 2,0 | – | 1,4 | – |

**Strahl zur Sonne:** 35,2 % der CPU, 25,0 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `ChunkCache::sonne_im_gang` | 11,1 | 36,5 | 7,9 | 25,8 |
| `renderer/src/render/sonne.rs` | `Sonnenform::trifft` | 8,4 | 8,6 | 6,0 | 6,1 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_in` | 6,9 | 12,0 | 4,9 | 8,5 |
| `renderer/src/render/metatile.rs` | `ChunkCache::sonnen_bits` | 3,0 | 3,1 | 2,1 | 2,2 |
| `renderer/src/render/metatile.rs` | `ChunkCache::frei_zur_sonne` | 2,0 | 9,9 | 1,4 | 7,0 |
| `renderer/src/render/metatile.rs` | `ChunkCache::saeule` | 1,6 | 8,0 | 1,1 | 5,7 |
| `renderer/src/render/metatile/strahl.rs` | `Gang::springe` | 1,4 | 1,4 | 1,0 | 1,0 |
| `renderer/src/render/metatile.rs` | `ChunkCache::wirkung` | 0,7 | 9,3 | 0,5 | 6,6 |
|  | übrige unter 0,5 % | 0,2 | – | 0,2 | – |

**Zeichnen:** 21,4 % der CPU, 15,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/metatile.rs` | `render_hdr` | 14,8 | 77,8 | 10,5 | 55,1 |
| `renderer/src/render/pyramid.rs` | `to_srgb` | 2,9 | 2,9 | 2,0 | 2,1 |
| `renderer/src/render/kino.rs` | `Kino::ton` | 1,5 | 4,4 | 1,1 | 3,1 |
| `renderer/src/render/metatile.rs` | `Hdr::bild` | 1,1 | 6,7 | 0,8 | 4,8 |
| `renderer/src/render/kino.rs` | `Kino::bloom` | 0,9 | 0,9 | 0,6 | 0,7 |
|  | übrige unter 0,5 % | 0,2 | – | 0,1 | – |

**Kodieren:** 1,2 % der CPU, 0,9 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
|  | übrige unter 0,5 % | 1,2 | – | 0,9 | – |

**Pyramide:** 1,5 % der CPU, 1,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| `renderer/src/render/pyramid.rs` | `halbiere` | 1,1 | 1,1 | 0,8 | 0,8 |
|  | übrige unter 0,5 % | 0,4 | – | 0,3 | – |

**Fingerabdruck und Pixel-Hash:** 0,6 % der CPU, 0,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| ring (Assembler) | `L$oop_shaext` | 0,6 | 0,6 | 0,4 | 0,4 |
|  | übrige unter 0,5 % | 0,0 | – | 0,0 | – |

**Höhen und Boden:** 1,1 % der CPU, 0,8 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate miniz_oxide | `miniz_oxide::deflate::core::compress_inner` | 0,9 | 1,0 | 0,7 | 0,7 |
|  | übrige unter 0,5 % | 0,1 | – | 0,1 | – |

**Allokator:** 2,0 % der CPU, 1,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| mimalloc (C) | `mi_malloc_aligned` | 0,8 | 1,2 | 0,6 | 0,9 |
|  | übrige unter 0,5 % | 1,2 | – | 0,8 | – |

**C-Laufzeit: Kopieren, Füllen, Rechnen:** 4,8 % der CPU, 3,4 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| C-Laufzeit | `MoveSmall4` | 1,0 | 1,0 | 0,7 | 0,7 |
| C-Laufzeit | `expf` | 0,8 | 0,9 | 0,6 | 0,6 |
|  | übrige unter 0,5 % | 3,0 | – | 2,1 | – |

**Kern und Dateisystem:** 4,6 % der CPU, 3,2 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Kern von Windows | – | 3,2 | – | 2,3 | – |
| Dateisystem und Platte | – | 0,9 | – | 0,6 | – |
|  | übrige unter 0,5 % | 0,4 | – | 0,3 | – |

**Rest:** 1,5 % der CPU, 1,1 s.

| Datei | Symbol | eigene % | gesamt % | eigene s | gesamt s |
|---|---|---|---|---|---|
| Crate core | `core::iter::traits::iterator::Iterator::any::check, Closure<i32,render::tiles::beruehrt...` | 1,1 | – | 0,8 | – |
|  | übrige unter 0,5 % | 0,4 | – | 0,3 | – |

<!-- renderzeit:ende -->

## Erneuern

Der Skill [`renderzeit-erneuern`](../../skills/renderzeit-erneuern/SKILL.md)
misst die Läufe neu, schreibt eine neue Messung und setzt die Tabellen
dieser Seite neu. Die Läufe, die Phasen, die Schwelle und die Spalten
bleiben dabei, wie sie sind.
