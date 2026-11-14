---
title: Optimierungen
description: Welche bekannten Techniken für Tempo und Speicher der Renderer und sein Build nutzen, welche fehlen, welche verworfen und welche offen sind, je mit Stelle im Code und Beleg; eine Seite, die der Maintainer angefordert hat.
code:
  - renderer/Cargo.toml
  - renderer/build.rs
  - renderer/segmentheap.manifest
  - renderer/src/main.rs
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli/verdichten.rs
  - renderer/src/render/gpu.rs
  - renderer/src/render/heights.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
  - renderer/src/world/chunk.rs
  - renderer/src/world/palette.rs
  - renderer/src/world/region.rs
  - renderer/vendor/libwebp-sys/build.rs
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
---

# Optimierungen

Eine Liste bekannter Techniken, die ein Programm schneller oder sparsamer
machen, und was der Renderer und sein Build davon nutzen. Je Technik steht
ihr Stand, die Stelle im Code und der Beleg. Die Zahlen stehen nicht hier,
sondern in den verlinkten Entscheidungen und Messungen.

## Auftrag

Angefordert vom Maintainer am 13.11.2026: „eine zentrale liste mit best
practice optimierungen welche die wir schon haben“, dazu die Techniken
Daten am Stück statt je Element auf dem Heap, Nummern statt Strings und
Freigeben am Stück. Die Gliederung in acht Abschnitte mit je einem Satz und
einer Tabelle hat er am selben Tag bestätigt. Was für diese Seite gilt,
steht in [`AGENTS.md`](../../AGENTS.md), Regel 28.

## So liest man die Tabellen

- **Stand:** `da` heisst, der Renderer nutzt die Technik. `fehlt` heisst,
  er nutzt sie nicht, und niemand hat darüber entschieden. `verworfen`
  heisst, sie ist geprüft und abgelehnt. `offen` heisst, sie passt auf eine
  Stelle, ist aber weder gebaut noch entschieden.
- **Stelle im Code:** die Datei und der Name darin. Ein Strich steht, wo es
  keine Stelle gibt.
- **Beleg:** die Entscheidung oder Messung. „nicht gemessen“ heisst, es
  gibt für diese Zeile keine eigene Zahl.

## 1. Build und Compiler

Der Compiler entscheidet, wie schnell derselbe Quelltext läuft. Diese
Zeilen sagen, was der Release-Build einschaltet.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Höchste Optimierungsstufe für alles, was je Pixel arbeitet | da | [`Cargo.toml`](../../renderer/Cargo.toml), `[profile.release]`, `opt-level` | [Weitergabe des Binärs](../entwicklung/weitergabe.md) |
| Eine Übersetzungseinheit: Der Compiler sieht den ganzen eigenen Code auf einmal | da | [`Cargo.toml`](../../renderer/Cargo.toml), `codegen-units` | [Weitergabe des Binärs](../entwicklung/weitergabe.md) |
| Optimieren über die Grenzen der Bibliotheken hinweg (LTO) | fehlt | – | Grösse in [Grösse des Binärs](../messungen/2026-10-05-binaergroesse.md); Zeit nicht gemessen |
| Optimieren nach einem aufgezeichneten Lauf (PGO) | fehlt | – | nicht gemessen |
| Neuere Befehlssätze der CPU für den eigenen Code | fehlt | – | nicht gemessen |
| Abbrechen statt Abwickeln bei einer Panik | verworfen | [`cli.rs`](../../renderer/src/cli.rs), `ohne_panik` | [Weitergabe des Binärs](../entwicklung/weitergabe.md) |
| Grösse vor Tempo, wo nichts je Pixel läuft: Grafikstapel und Kommandozeile | da | [`Cargo.toml`](../../renderer/Cargo.toml), `[profile.release.package.*]` | [Grösse des Binärs](../messungen/2026-10-05-binaergroesse.md) |

## 2. Speicher und Allokation

Speicher anfordern und freigeben kostet Zeit, und viele Threads bremsen
sich dabei gegenseitig.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Ein Allokator, der viele Threads nicht ausbremst | da | [`main.rs`](../../renderer/src/main.rs), `GLOBAL` | [0020](../entscheidungen/0020-mimalloc.md) |
| Ein Heap, der Speicher behält, für die C-Bibliothek, die Kacheln packt | da | [`build.rs`](../../renderer/build.rs), [`segmentheap.manifest`](../../renderer/segmentheap.manifest) | [0029](../entscheidungen/0029-segment-heap-fuer-libwebp.md) |
| Puffer wiederverwenden, statt sie je Kachel oder Chunk neu anzulegen | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Deckung::new` und `ChunkCache` mit dem Feld `ausbreitung`; [`kino.rs`](../../renderer/src/render/kino.rs), `Bloompuffer` | nicht gemessen |
| Freigeben am Stück: Eine ganze Struktur fällt mit einem Schritt weg, statt Element für Element | fehlt | – | nicht gemessen |

## 3. Datenlayout

Wie Daten im Speicher liegen, entscheidet, wie viel davon in den schnellen
Zwischenspeicher der CPU passt und wie viele Schritte ein Zugriff braucht.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Bitmasken je Spalte: sechzehn Blöcke mit wenigen Wortoperationen statt eines Besuchs je Block | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Masks` | [0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md), [Bitmasken](../messungen/2026-09-25-bitmasken.md) |
| Ganz leere und ganz volle Ebenen der Masken teilen sich einen Platz | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Masks::packe`, `Masks::ebene` | [Masken teilen](../messungen/2026-10-25-masken-teilen.md) |
| Licht je Section verdichtet: ein Wert, wo alle Zellen gleich sind | da | [`licht.rs`](../../renderer/src/render/licht.rs), `SectionLicht`, `verdichte` | nicht gemessen |
| Deckung je Pixel als ein Bit | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Deckung` | [0026](../entscheidungen/0026-deckungsmaske.md) |
| Pixel-Hashes als ein festes Feld | da | [`pixel.rs`](../../renderer/src/cli/pixel.rs), `Pixel` | [Pixel-Hashes als Feld](../messungen/2026-10-25-pixel-hashes-als-feld.md) |
| Die gepackten Indizes der Welt bleiben gepackt | da | [`palette.rs`](../../renderer/src/world/palette.rs), `PackedIndices` | nicht gemessen |
| Nummern statt Strings: Familie, Leuchten und Biom je Paletteneintrag einmal je Chunk nachgeschlagen, danach nur noch Nummern | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Loaded::new` | nicht gemessen |
| Nummern statt Strings für die Palette selbst | offen | [`palette.rs`](../../renderer/src/world/palette.rs), `BlockState`; [`chunk.rs`](../../renderer/src/world/chunk.rs), `decode_section` | Anteil der betroffenen Funktionen in [Wohin die Zeit beim Rendern geht](renderzeit.md); der Gewinn selbst nicht gemessen |
| Daten am Stück statt je Element auf dem Heap: Heute hält jeder Paletteneintrag seinen Namen und seine Eigenschaften einzeln | offen | [`palette.rs`](../../renderer/src/world/palette.rs), `BlockState` | nicht gemessen |

## 4. Threads

Mehrere Kerne rechnen zugleich, solange sie sich nicht um dieselben Daten
streiten.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Kacheln parallel über alle Kerne | da | [`cli.rs`](../../renderer/src/cli.rs), `run`, `write_tiles` | nicht gemessen |
| Streifen: Jeder Thread zeichnet zusammenhängende Kacheln und hält seinen eigenen Cache der Chunks | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `ChunkCache`, `streifenbreite` | [0025](../entscheidungen/0025-streifen-und-cache-je-thread.md), [Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md) |
| Eigene Threads nur zum Schreiben | verworfen | – | [0027](../entscheidungen/0027-keine-schreibthreads.md) |

## 5. Arbeit vermeiden

Am schnellsten ist die Arbeit, die gar nicht anfällt.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Vorlauf: erst feststellen, welche Kacheln es gibt, dann zeichnen | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `survey_region` | [0003](../entscheidungen/0003-vorlauf-vor-dem-rendern.md) |
| Verdeckte Blöcke früh aussortieren | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `ChunkCache::expose`, `Exposed` | [0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md) |
| Von vorn nach hinten zeichnen und Verdecktes auslassen | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `Deckung::zeichne`, `blit_sichtbar` | [0026](../entscheidungen/0026-deckungsmaske.md) |
| Zeichnen auf der Grafikkarte, wo es eine gibt | da | [`gpu.rs`](../../renderer/src/render/gpu.rs), `Gpu` | [0023](../entscheidungen/0023-zeichnen-auf-der-grafikkarte.md), [Grafikkarte](../messungen/2026-09-26-grafikkarte.md) |
| Die feinen Stufen der Pyramide im Speicher bauen, während die Basis entsteht | da | [`cli.rs`](../../renderer/src/cli.rs), `ImSpeicher` | [0042](../entscheidungen/0042-feine-stufen-im-speicher.md) |
| Nur prüfen, ob etwas die Sonne verdeckt, wo etwas im Weg sein kann | da | [`strahl.rs`](../../renderer/src/render/metatile/strahl.rs), `ChunkCache::frei_zur_sonne` | [Bits frei zur Sonne](../messungen/2026-10-03-bits-frei-zur-sonne.md) |
| Updates: nur zeichnen, wo sich die Welt geändert hat | da | [`stand.rs`](../../renderer/src/render/stand.rs); [`chunk.rs`](../../renderer/src/world/chunk.rs), `Chunk::abdruck` | [0062](../entscheidungen/0062-updates-nach-stempel-und-fingerabdruck.md) |
| Das Gebiet eines Updates endet unter der Änderung | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `gebiet_der_aenderungen` | [0127](../entscheidungen/0127-grenze-nach-unten.md), [Grenze nach unten](../messungen/2026-10-24-grenze-nach-unten.md) |
| Kacheln mit gleichen Pixeln nicht neu packen | da | [`pixel.rs`](../../renderer/src/cli/pixel.rs), `Pixel` | [0091](../entscheidungen/0091-gleiche-pixel-nicht-kodieren.md) |
| Die Pyramide nur über Kindern mit neuen Pixeln neu bauen | da | [`cli.rs`](../../renderer/src/cli.rs), `pyramide_nur_ueber_neuen` | [0124](../entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md), [Pyramide nur über neuen Kindern](../messungen/2026-10-24-pyramide-nur-ueber-neuen.md) |
| Den Stand nur für Regionen schreiben, die anders sind | da | [`cli.rs`](../../renderer/src/cli.rs), `lege_stand_ab` | [0126](../entscheidungen/0126-stand-je-region.md) |
| Licht, Farben und Masken über die nativen Stufen teilen | verworfen | – | [Arbeit je Block über die nativen Stufen](../versuche/arbeit-je-block.md) |

## 6. SIMD

Ein einzelner Befehl der CPU rechnet mehrere Werte auf einmal. Das gibt es
je nach Alter der CPU in mehreren Stufen.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Im eigenen Code | fehlt | – | nicht gemessen |
| Beim Packen der Kacheln unter Windows: alle Stufen im Binär, gewählt zur Laufzeit | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `richte_libwebp_ein` | [0028](../entscheidungen/0028-libwebp-statt-image.md), [Kompakt packen](../messungen/2026-10-08-kompakt.md) |
| Beim Packen der Kacheln unter Linux: nur die erste Stufe | da | [`build.rs`](../../renderer/vendor/libwebp-sys/build.rs), `setup_build` | [0028](../entscheidungen/0028-libwebp-statt-image.md) |
| Unter Linux die höheren Stufen für das ganze Binär einschalten | verworfen | [`build.rs`](../../renderer/vendor/libwebp-sys/build.rs), `setup_build` | [0028](../entscheidungen/0028-libwebp-statt-image.md) |
| Unter Linux die höheren Stufen nur für ihre Dateien, gewählt zur Laufzeit | verworfen | [`build.rs`](../../renderer/vendor/libwebp-sys/build.rs), `do_build` | [libwebp mit SIMD unter Linux](../messungen/2026-11-13-libwebp-simd-unter-linux.md) |

## 7. Grenzprüfungen und direkter Code

Die Sprache prüft jeden Zugriff auf ein Feld. Das kostet wenig, wo der
Compiler die Prüfung beweisen und weglassen kann.

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Grenzprüfungen bleiben an; ungeprüfter Code steht im Renderpfad nur an der Grenze zur C-Bibliothek | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `encode_webp`, `decode_webp` | nicht gemessen |
| Prüfungen aus den Schleifen je Pixel herausheben | offen | [`metatile.rs`](../../renderer/src/render/metatile.rs), `blit_sichtbar`, `blit_hdr` | Anteil der Funktionen in [Wohin die Zeit beim Rendern geht](renderzeit.md); der Gewinn selbst nicht gemessen |
| Kleine Funktionen je Pixel gezielt einbetten | da | [`metatile.rs`](../../renderer/src/render/metatile.rs), `faktor`, `anteile`, `mische`, `mische_hdr` | nicht gemessen |
| Direkte statt dynamischer Aufrufe beim Zeichnen der Karte | da | – | nicht gemessen |
| Direkter statt dynamischer Aufruf der Sonne je Pixel im Cinematic | offen | [`metatile.rs`](../../renderer/src/render/metatile.rs), `blit_hdr`, Parameter `sonne` | nicht gemessen |
| Der Compiler weiss, dass sich zwei Zugriffe nicht überlappen | da | – | nicht gemessen |

## 8. Kodieren und Ablegen

Jede Kachel wird gepackt und als Datei geschrieben. Wie viel Zeit das
braucht, steht in [Wohin die Zeit beim Rendern geht](renderzeit.md).

| Technik | Stand | Stelle im Code | Beleg |
|---|---|---|---|
| Verlustfreies WebP statt PNG | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `encode_webp` | [0004](../entscheidungen/0004-webp-verlustfrei.md) |
| Die C-Bibliothek des Formats statt eines Encoders in Rust | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `encode_webp` | [0028](../entscheidungen/0028-libwebp-statt-image.md), [libwebp](../messungen/2026-09-27-libwebp.md) |
| Schnell packen, aber Wiederholungen in der ganzen Kachel finden | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `encode_webp` | [0090](../entscheidungen/0090-webp-mit-quality-75.md), [WebP mit quality 75](../messungen/2026-10-08-webp-quality-75.md) |
| Kompakt packen auf Wunsch: kleinere Dateien für mehr Zeit | da | [`tiles.rs`](../../renderer/src/render/tiles.rs), `Packen` | [0092](../entscheidungen/0092-kompakt-ohne-vorhersage.md), [Kompakt packen](../messungen/2026-10-08-kompakt.md) |
| Beim kompakten Packen die Grösse des Farb-Caches festlegen, statt sie je Kachel zu suchen | offen | [`backward_references_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c), `CalculateBestCacheSize` | [Farb-Cache fest statt gesucht](../messungen/2026-11-13-farbcache-fest-statt-suchen.md), [libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md) |
| Beim kompakten Packen RLE nicht als zweite Art neben LZ77 probieren | offen | [`vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c), `EncoderAnalyze` | [libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md) |
| Beim kompakten Packen die Analyse der Entropie auslassen, wo ihr Ergebnis feststeht | offen | [`vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c), `EncoderAnalyze`, `AnalyzeEntropy` | [libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md) |
| Beim kompakten Packen weniger Aufwand: quality 51, weniger Felder für Histogramme, ohne TraceBackwards | offen | [`tiles.rs`](../../renderer/src/render/tiles.rs), `kodiere`; [`vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c), `GetHistoBits`; [`backward_references_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c), `GetBackwardReferences` | [libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md) |
| Schnell packen ohne räumliche Vorhersage und mit Farb-Cache | offen | [`vp8l_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c), `EncoderAnalyze`; [`backward_references_enc.c`](../../renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c), `GetBackwardReferencesLowEffort` | [libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md) |
| Near-lossless: Pixel vor dem Packen leicht runden, im verlustfreien Format | offen | – | [Near-lossless und verlustbehaftetes WebP](../messungen/2026-11-14-near-lossless-und-verlustbehaftet.md) |
| Verlustbehaftetes WebP | verworfen | – | [0004](../entscheidungen/0004-webp-verlustfrei.md), [Near-lossless und verlustbehaftetes WebP](../messungen/2026-11-14-near-lossless-und-verlustbehaftet.md) |
| Chunkdaten mit einer schnelleren Bibliothek entpacken | offen | [`region.rs`](../../renderer/src/world/region.rs), `decompress` | [0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md), [Entpacken und Höhen mit zlib-rs und fdeflate](../messungen/2026-11-14-entpacken-und-hoehen.md) |
| Höhen mit einer schnelleren Bibliothek packen | offen | [`heights.rs`](../../renderer/src/render/heights.rs), `Heights::encode` | [Entpacken und Höhen mit zlib-rs und fdeflate](../messungen/2026-11-14-entpacken-und-hoehen.md) |
| Einen fertigen Baum nachträglich kompakt packen | da | [`verdichten.rs`](../../renderer/src/cli/verdichten.rs), `verdichte` | [0093](../entscheidungen/0093-nachverdichten.md), [Nachverdichten](../messungen/2026-10-08-nachverdichten.md) |
| Den Kachelordner vom Echtzeitschutz ausnehmen, nur mit Zustimmung | da | [`cli.rs`](../../renderer/src/cli.rs), `defender_exclusion` | [0022](../entscheidungen/0022-defender-ausnahme-nur-mit-zustimmung.md), [Echtzeitschutz](../messungen/2026-09-26-echtzeitschutz.md) |
