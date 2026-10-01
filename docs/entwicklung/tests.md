---
title: Tests
description: Wie man die Tests laufen lässt, welche Datei was prüft, woher Fixtures und Sollwerte kommen und wie das Goldbild Änderungen am Bild auffängt.
code:
  - renderer/tests/assets.rs
  - renderer/tests/cli.rs
  - renderer/tests/common/mod.rs
  - renderer/tests/gpu.rs
  - renderer/tests/licht.rs
  - renderer/tests/metatile.rs
  - renderer/tests/region_format.rs
  - renderer/tests/render.rs
  - renderer/tests/tiles.rs
  - renderer/tests/world_reader.rs
  - renderer/tests/fixtures
  - web/tests/smoke.spec.ts
  - web/tests/pick.spec.ts
  - web/tests/seite.spec.ts
---

# Tests

Die Rust-Tests laufen mit `cargo nextest`, in Debug und Release, das
Frontend mit Playwright. Sollwerte stammen aus unabhängigen Quellen: einem
eigenen Python-Decoder, den Klassen des 26.2-Clients oder einer Rechnung
mit den Werten des Spiels, nicht aus dem Code selbst. Ein Goldbild fängt
jede Änderung am Bild. Was die CI davon laufen lässt, steht in
[CI](ci.md).

## Laufen lassen

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

Lighthouse lokal: [CI](ci.md), „Lighthouse“.

Ein Test läuft nur in Release: `eimer_zaehlen_wie_die_binaersuche` in
`pyramid.rs` prüft jeden f32 von 0 bis 1, gut eine Milliarde Werte, in
rund zwei Sekunden. Im Debug-Build dauerte er zu lange und trägt dort
`#[cfg_attr(debug_assertions, ignore = …)]`.

## Welche Datei was prüft

| Datei | Prüft |
|---|---|
| `renderer/tests/world_reader.rs` | den Welt-Reader am Fixture einer echten Region |
| `renderer/tests/region_format.rs` | beschädigte Regionsdateien, zur Laufzeit gebaut |
| `renderer/tests/assets.rs` | den Asset-Layer am synthetischen Assetbaum |
| `renderer/tests/render.rs` | Projektion, Baking und Rasterizer zusammen: von der Blockstate bis zu den Pixeln des Sprites |
| `renderer/tests/metatile.rs` | ganze Welten im Speicher, gerendert, samt Goldbild |
| `renderer/tests/tiles.rs` | die Naht: jede Kachel gegen den Ausschnitt eines grossen Renderings |
| `renderer/tests/cli.rs` | die ganze Exportkette über das echte Binär |
| `renderer/tests/gpu.rs` | die Karte gegen die CPU, Byte für Byte |
| `renderer/tests/licht.rs` | die Ausbreitung des Lichts gegen einen Lauf von Vanilla 26.2 und an gebauten Welten |
| `renderer/tests/common/mod.rs` | gemeinsame Szenen und Helfer |
| `web/tests/smoke.spec.ts` | das Frontend am Demo-Kachelbaum, die Koordinaten mit Maus und Touch über Höhen, die der Test liefert, den Umriss nur beim Tippen, und dass die Karte unter den strengen Headern aus `preview.headers` in `web/vite.config.ts` ohne Verletzung der Content-Security-Policy läuft |
| `web/tests/seite.spec.ts` | den Kopf der Seite und `robots.txt`, ohne `SITE_URL` am ausgelieferten Build und mit `SITE_URL=https://example.org/karte` an einem eigenen Build |
| `web/tests/pick.spec.ts` | den Strahl: jeder Bildpunkt eines kleinen Geländes gegen den Würfel, den das Zeichnen dort hinterlässt, gerechnet mit der Projektion aus [Die Kamera](../renderer/kamera.md), die der Test an `renderer/tests/fixtures/projektion.json` des Renderers prüft |

Dazu stehen Unit-Tests in den Quelldateien selbst, unter `mod tests`.

## Fixtures

Das Fixture unter `renderer/tests/fixtures/` ist eine 40 KB grosse Region
mit 2×2 echten Terrain-Chunks aus der Testwelt (DataVersion 4903, 26.2).
Die Sollwerte der Tests stammen aus einem unabhängig geschriebenen
Python-Decoder, damit die Tests nicht dieselbe Annahme prüfen wie der Code.

Für das Licht liegen unter `renderer/tests/fixtures/licht/` 4×4 Chunks
der Testwelt, x und z von -18 bis -15. Ein Vanilla-Server 26.2 hat ihr
gespeichertes Licht gelöscht (`--forceUpgrade --eraseCache`), sie neu
beleuchtet und gespeichert, darüber gebaute Szenen: Tunnel mit Glowstone
hinter Platten und getöntem Glas, Löcher im Dach mit Platten und Treppe,
ein Becken mit Seelaterne und Magma, eine Fackel neben Schnee und
Ackerboden, eine Säule aus Laub. `licht_wie_im_spiel` vergleicht jede
Zelle der 2×2 Chunks in der Mitte, deren Rand von 14 Blöcken im Fixture
liegt: in jeder Section mit gespeichertem Array die Stufe darin; in einer
Section ohne Array, neben der Blöcke stehen, 0, denn dort hält das Spiel
das Licht im Speicher und lässt nur das leere Array weg
(`SerializableChunkData.copyOf`). Sections ganz ohne Speicher zählen
nicht, dort liegt keine Zelle vor einer Fläche.

Beschädigte Regionsdateien lassen sich nicht aus einer echten Welt
extrahieren. `renderer/tests/region_format.rs` baut sie deshalb zur
Laufzeit: ausgelagerte `.mcc`-Chunks, kaputte Längenfelder und
Tabelleneinträge, Paletten ohne Indexdaten, Indizes jenseits der Palette.

Für den Asset-Layer liegt unter `renderer/tests/fixtures/assets-base` und
`assets-overlay` ein kleiner, von Hand geschriebener Assetbaum. Er ist
synthetisch, bildet aber die Formen ab, die eine Bestandsaufnahme über
Vanilla 26.2 und das TerraNova-Pack ergeben hat. Für die Blockentities
bringt er Truhe, Banner und Krug mit einem Modell nur aus der
Partikeltextur wie im Spiel und kleine, selbst gemalte Texturen unter
`textures/entity`. Biome und Bannermuster für die Tests liegen unter
`renderer/tests/fixtures/data-base`.

## Welten im Speicher

`renderer/tests/metatile.rs` baut aus diesem Assetbaum ganze Welten im
Speicher und rendert sie; `renderer/tests/cli.rs` ruft dafür die echte
Binärdatei auf, weil der Weg über `--center` eine eigene Fehlerquelle ist.
`renderer/tests/tiles.rs` prüft die Naht: jede einzeln gerenderte Kachel
gegen den entsprechenden Ausschnitt eines grossen Renderings. Und
`tests/cli.rs` hält die ganze Exportkette fest, unter anderem, dass jede
Kachel einer gröberen Stufe Pixel für Pixel die Verkleinerung ihrer vier
Kinder ist.

`schneller_weg_gleicht_der_referenz` rendert eine Szene über mehrere Chunks,
Biome und Sections Byte für Byte gegen `render_area_without_culling`, die
Referenz ohne jede Abkürzung, bei scale 2, 6 und jedem Vielfachen von 4
bis 32.

## Goldbild

Unter `renderer/tests/fixtures/golden/` liegt ein Goldbild: jede Änderung an
Projektion, Baking, Rasterizer oder Maleralgorithmus fällt damit auf. Fällt
der Test, schreibt er das Ist-Bild daneben als `metatile-ist.png`; in CI
liegt es als Artefakt am fehlgeschlagenen Lauf. Neu erzeugen nach einer
gewollten Änderung: Skill
[`goldbild-erneuern`](../../skills/goldbild-erneuern/SKILL.md).

## GPU-Tests

Die Tests, die eine Karte brauchen, überspringen sich ohne Karte und sagen
es. In der CI laufen sie auf Software-Adaptern, und dort ist ein fehlender
Adapter ein Fehler, siehe [Grafikkarte](../benutzung/grafikkarte.md),
„Adapter und Backends“.

## Sollwerte aus dem Spiel

Ein Test, der Verhalten des Spiels festschreibt, nennt die Herkunft des
Werts in einem Satz. Wie man den Beleg holt: Skill
[`spielverhalten-belegen`](../../skills/spielverhalten-belegen/SKILL.md).
Wo eine Rechnung zu viele Fälle hat, um sie von Hand nachzurechnen, gibt
ein kleines Java-Programm die Werte aus den Klassen des Spiels selbst aus:
für Biomzoom, Seed und Sumpfrauschen
[`renderer/src/world/Biomwerte.java`](../../renderer/src/world/Biomwerte.java),
siehe [Biomfarben](../renderer/biomfarben.md), „Belege“.

## Tests mit den Vanilla-Assets

Die CI hat keine Vanilla-Assets. Ein Test, der sie braucht, trägt
`#[ignore]` und liest ihre Wurzeln aus der Umgebung:
`toenungskarte_an_allen_vanilla_bloecken` in `sprites.rs` prüft die
Tönungskarte an allen Blöcken, die gefärbt oder geflutet sein können, mit
den Wurzeln wie `--assets` als Pfadliste in `ASSETS`. Der Aufruf steht am
Test, das Ergebnis in [Biomfarben](../renderer/biomfarben.md), „Tönung beim
Zeichnen“.
