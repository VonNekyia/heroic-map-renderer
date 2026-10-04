---
title: Tests
description: Wie man die Tests laufen lässt, welche Datei was prüft, woher Fixtures und Sollwerte kommen und wie die Goldbilder Änderungen am Bild auffangen.
code:
  - renderer/tests/assets.rs
  - renderer/tests/cli.rs
  - renderer/tests/common/mod.rs
  - renderer/tests/gpu.rs
  - renderer/tests/heights.rs
  - renderer/tests/kennzahlen.rs
  - renderer/tests/licht.rs
  - renderer/tests/metatile.rs
  - renderer/tests/region_format.rs
  - renderer/tests/render.rs
  - renderer/tests/richtung.rs
  - renderer/tests/tiles.rs
  - renderer/tests/world_reader.rs
  - renderer/tests/fixtures
  - web/tests/smoke.spec.ts
  - web/tests/pick.spec.ts
  - web/skins/tablett/tests/tablett.spec.ts
  - web/skins/tablett/tests/bilder.spec.ts
  - web/skins/tablett/tests/karte.spec.ts
  - web/skins/tablett/tests/auslagern.spec.ts
  - web/playwright.config.ts
  - web/tests/kamera.ts
  - web/tests/seite.spec.ts
---

# Tests

Die Rust-Tests laufen mit `cargo nextest`, in Debug und Release, das
Frontend mit Playwright. Sollwerte stammen aus unabhängigen Quellen: einem
eigenen Python-Decoder, den Klassen des 26.2-Clients oder einer Rechnung
mit den Werten des Spiels, nicht aus dem Code selbst. Goldbilder fangen
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
npm test          # Playwright, baut vorher zweimal: ohne Skin und mit SKIN=./skins/tablett
```

Lighthouse lokal: [CI](ci.md), „Lighthouse“.

Testbauten rechnen mit Optimierung: die Abhängigkeiten mit `opt-level` 2
(`[profile.dev.package."*"]`), das Crate und die Tests mit 1
(`[profile.test]`), beide in `renderer/Cargo.toml`. `debug-assertions` und
`overflow-checks` bleiben an, die Prüfungen des Debug-Builds also auch. Die
Suite braucht so lokal rund 22 statt 90 s, in der CI auf ubuntu 24 statt
186 s; das Bauen kostet in der CI dafür 31 bis 98 s mehr, siehe
[Tests schneller](../messungen/2026-10-02-tests-schneller.md).

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
| `renderer/tests/metatile.rs` | ganze Welten im Speicher, gerendert, samt Goldbildern |
| `renderer/tests/richtung.rs` | die Richtungen der Kamera: Modelle gedreht, Seiten nach der Welt schattiert, auch die eines Blockentities, Licht, Alternativen und Biome aus der Welt; gedrehte Szenen je Kamera aus allen vier Richtungen wie aus der Vorgabe: Treppen, Türen, Zäune, Scheiben, Licht unter einem Dach, Teile in fremden Würfeln, Blockentities, einen Spawner vor vollen Blöcken und Wasser, dazu Eis und Wasser in Stufen, siehe [Richtungen](../renderer/richtungen.md) |
| `renderer/tests/heights.rs` | die Höhen für die Koordinatenanzeige und `projektion.json` für das Frontend |
| `renderer/tests/tiles.rs` | die Naht: jede Kachel gegen den Ausschnitt eines grossen Renderings |
| `renderer/tests/cli.rs` | die ganze Exportkette über das echte Binär |
| `renderer/tests/gpu.rs` | die Karte gegen die CPU, Byte für Byte |
| `renderer/tests/kennzahlen.rs` | nur von Hand, mit der Testwelt: die Kennzahlen des Looks über 24 Ansichten (`kennzahlen_der_ansichten`) und die Bilder des Renderers zu Cinematic (`bilder_zu_cinematic`), siehe unten |
| `renderer/tests/licht.rs` | die Ausbreitung des Lichts gegen einen Lauf von Vanilla 26.2 und an gebauten Welten |
| `renderer/tests/common/mod.rs` | gemeinsame Szenen und Helfer |
| `web/tests/smoke.spec.ts` | das Frontend am Demo-Kachelbaum, dass es `map.json`, `trees.json` und Höhen mit `cache: 'no-cache'` holt, den Stand der Karte aus `Last-Modified`, die Koordinaten mit Maus und Touch über Höhen, die der Test liefert, mit `projection` aus `map.json`, aus jeder Richtung in Weltkoordinaten, ohne Anzeige bei unbekanntem `azimuth` oder `direction`, den Umriss nur beim Tippen, die Adresse, die der Karte folgt, ohne Einträge im Verlauf, den Knopf für die ganze Karte per Maus und Tastatur, das Kopiersymbol für `/tp` mit Maus, Tastatur und Finger samt Rückmeldung ohne Zwischenablage oder bei einem Fehler, den Sprung über einen editierten Wert mit Y aus der Höhenkarte, abgewiesene Eingaben, Abbruch per Escape und Klick daneben und das Minus auf dem Touchscreen, Ziele ab 24 px und 16 px im Feld für Finger, ohne Skin kein Tablett und nur das Bündel der Karte, auch mit quadratischem `area`, Kompass und Umschalter zwischen zwei Bäumen aus `trees.json` mit demselben Block in der Mitte, und dass die Karte unter den strengen Headern aus `preview.headers` in `web/vite.config.ts` ohne Verletzung der Content-Security-Policy läuft |
| `web/tests/seite.spec.ts` | den Kopf der Seite und `robots.txt`, ohne `SITE_URL` am ausgelieferten Build und mit `SITE_URL=https://example.org/karte` an einem eigenen Build |
| `web/skins/tablett/tests/tablett.spec.ts` | Rahmen und Tisch: für jede Kamera und Richtung die Ecken der Oberkante gegen `renderer/tests/fixtures/projektion.json`; dass nichts über den Kacheln Gelände über dem Wasserspiegel deckt, bis fast an die Bauhöhe und an jedem Rand zweier Welten, auch nicht die Kopie des Tischs, ausser den Eckstücken; dass Rahmen und Tisch den Schnitt der Welt zur Kamera ganz decken; dass `grenzen` Rahmen und Lilien genau umfasst; dass auf jedem Pfeiler seine Lilie steht, so gross wie in der Vorlage, nach dem Rahmen gemalt, vor den Kacheln; dass je Ecke ein Eckstück die Ecke der Welt 4,5·w tief deckt, vor den Kacheln, zwischen Holz und Lilien; dass das Licht der Flächen ohne Bild von oben links kommt, die linke nahe Wand in 2:1 und 8:5 rund 1,6-mal so hell wie die rechte; dass nur die fernen Seiten eine Innenseite bis zum Boden haben, hinter der Welt; dass Band, Wand, Eckstücke, die Seiten der Pfeiler zur Kamera und der Tisch ihr Bild in `bilder/` haben; dass die Gesamtansicht das Fenster zu 71 bis 100 % füllt, wo es geht zu 92,5 %, gebrochen nur, wo Leaflet die Kacheln verkleinert, und nie unter der Stufe, auf die Leaflet einpasst; dass ihre Mitte wie in der Vorlage unter der Mitte der Karte liegt und der Rahmen nie aus dem Fenster ragt, von Telefonen bis 4K. Kameras und Einträge des Renderers nimmt es aus `web/tests/kamera.ts` |
| `web/skins/tablett/tests/bilder.spec.ts` | die Bilder im Repository in Node, ohne das Skript: dass jedes Bild in `bilder/` genommen wird und jedes genommene dort liegt; dass Streifen und Eckstücke das Seitenverhältnis ihrer Flächen haben und Lilien und Gegenstände ihre Grösse aus `bilder.ts` |
| `web/skins/tablett/tests/karte.spec.ts` | den Skin im Browser am Build mit ihm: beide Ebenen unter und über den Kacheln, gemalt, ohne Klicks, die Koordinaten durch sie hindurch, sichtbar und voll deckend auf der Stufe über der Gesamtansicht und auf der feinsten, auch an der nahen Ecke, die Gesamtansicht als kleinste Stufe; dass die Leinwand das Fenster mit Überstand ist, auf der feinsten Stufe so gross wie in der Gesamtansicht; dass der Skin nach einem Zoom, einem Zug über den Überstand hinaus und einer neuen Fenstergrösse neu zeichnet, nach einem kurzen Zug nicht; dass der Skin Bilder aus `bilder/` legt, jedes geglättet, und, kommt das Bild des Tischs beim Ziehen, erst danach malt; die Gesamtansicht zwischen zwei Stufen mit 92,5 %, die Leinwand Pixel auf Pixel, geglättete Kacheln und der Weg hinein und heraus; dass sich die Karte hineingezoomt bis über jede Ecke von `area` ziehen lässt; dass ein `area`, das kein Quadrat ist, kein Tablett zeichnet und es in der Konsole sagt |
| `web/skins/tablett/tests/auslagern.spec.ts` | dass die Karte mit einer Kopie des Skins aus einem Ordner ausserhalb des Repositorys baut, der Skin ein eigenes Stück ist und Leaflet nur im Bündel der Karte steht |
| `web/tests/pick.spec.ts` | den Strahl: für 2:1, 8:5, 4:3, 1:1, 5:3, `top`, `top-north` und `north-45` jeder Bildpunkt eines kleinen Geländes gegen den Würfel, den das Zeichnen dort in der Reihenfolge (y, v, u) und nach der Füllregel hinterlässt; dazu die Projektion jeder Kamera und die Kantenpixel auf Oberseiten und an Wänden gegen `renderer/tests/fixtures/projektion.json` des Renderers |

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
`textures/entity`. Der Spawner hat nur das innere, in x gespiegelte
Element von `cube_all_inner_faces` aus 26.2, einfarbig, die untere Schicht
der Mangrovenwurzeln eine eigene Farbe. Das Pack `assets-platten` zeichnet
jeden Zustand der Eichenplatte mit dem Modell der unteren, für zwei
Familien mit demselben Bild, die verschieden decken. Das Pack
`assets-wurzeln` gibt den Mangrovenwurzeln die sechs Schichten aus 26.2,
für ihre Seiten aus jeder Richtung. Biome und Bannermuster für die Tests liegen
unter `renderer/tests/fixtures/data-base`.

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
Referenz ohne jede Abkürzung, in 2:1 bei scale 2, 6 und jedem Vielfachen
von 4 bis 32, dazu bei jeder Kamera der Invarianten. Die Projektionen
rechnet er parallel, sonst bestimmte er allein die Dauer der Suite.

## Kameras

Die Invarianten gelten für jede Kamera. `kameras()` in
`renderer/tests/metatile.rs` liefert sie: 16:9, 8:5, 4:3, 1:1 und `top`
bei scale 32, 5:3 bei scale 30, 1:1 und `top` bei scale 4 und 6, dazu sechs
Paare aus gültigem W:H und scale, gezogen mit fester Saat, damit jeder Lauf
dieselben prüft, ohne 2:1 und ohne eine Kamera zweimal. Genordet kommen
`top-north` und `north-45` je bei 16 und seinen nativen Stufen 8 und 4
dazu, `top-north` bei 6 und 24, `north-45` bei 12 und 48, und je ein
gezogener ungerader scale. Jede läuft aus der Vorgabe. Aus einer der drei
anderen Richtungen, reihum, laufen je Art die mit dem kleinsten scale und
die genordete mit ungeradem: 9:5 bei 18, 1:1, `top`, `top-north` und
`north-45` bei 4 und `top-north` bei 31. So prüft jede Invariante alle
vier Richtungen, schräg wie genordet. Die Spalten dreht `spalten_im_blick`
unabhängig von Kamera und scale, die Drehung je Kamera prüft
`gedrehte_szene_wie_aus_der_vorgabe`, siehe
[Richtungen](../renderer/richtungen.md). Das Bild ist bei
`verdecken_aendert_kein_pixel` und `schneller_weg_gleicht_der_referenz`
das Rechteck um alle Blöcke der Szene, bei jeder Kamera. Je Kamera:

| Test | Prüft |
|---|---|
| `verdecken_aendert_kein_pixel` | Verdecken ist nur eine Abkürzung |
| `schneller_weg_gleicht_der_referenz` | Kandidaten und Bitmasken gegen die Referenz |
| `kein_loch_in_deckendem_gelaende` | kein offener Pixel, auch auf Kanten, die Pixelmitten treffen |
| `hoeher_gesetzt_gleiches_bild` | dieselbe Welt 40 Blöcke höher gibt dasselbe Bild, auch von oben, wo die Referenz dasselbe Band abläuft |
| `kleine_ausschnitte_gleichen_dem_grossen_bild` | Ausschnitte von 128 Pixeln gleichen dem Bild der ganzen Szene, für 2:1 aus allen vier Richtungen, 4:3, `top`, `top-north` und `north-45` bei 16 und `north-45` bei 7, jede aus der Vorgabe und aus einer anderen Richtung, als Karte und mit Cinematic; das ganze Bild ist zweimal gleich; ein Ausschnitt liest nur die Sections und Chunks seines Bands (`y_span`) |
| `cinematic_zeichnet_dieselben_draws_wie_die_karte` | Cinematic zeichnet dieselben Draws wie die Karte, sein Licht ist durch die Lightmap gerechnet ihres, und ein Pixel ist genau da, wo die Karte einen hat; dazu 2:1 bei 4, 16 und 32 |
| `projektion_als_datei_ist_aktuell` (`renderer/tests/heights.rs`) | die Datei für das Frontend, samt Kantenpixeln |

Dazu einzeln: `von_oben_ragt_der_turm_durch_den_teppich` (`metatile.rs`),
`heights_traegt_hoehen_nach` auch für einen Baum von oben und einen in
`north-45`, `genordeter_baum_mit_azimut_und_richtung` und
`kamera_ohne_ganze_pixel_bricht_vor_der_welt_ab` (`cli.rs`). Die Ablage
unter einer Wurzel prüfen `liste_der_baeume_unter_der_wurzel`,
`alte_ablage_nennt_den_ordner`, `baum_statt_wurzel_nennt_die_wurzel`,
`eine_wurzel_eine_welt`, `kaputter_nachbar_wird_uebergangen` und
`andere_richtung_im_ordner_wird_abgelehnt` (`cli.rs`). Welche Richtungen
eine Kamera nimmt, prüfen `richtung_wird_je_kamera_geprueft` (`cli.rs`),
jede Kamera mit jeder ihrer Richtungen, und `falsche_richtung_nennt_die_vier`
(`projection.rs`), die Drehung nach der Tabelle
`richtungen_drehen_die_welt_wie_die_tabelle` (`projection.rs`). Den Lauf
mit `--direction` prüft `richtung_ist_ein_eigener_baum` aus `nw` und `sw`,
`--center` aus jeder Richtung `center_in_der_welt_aus_jeder_richtung`
(`cli.rs`). Die Tabellen der weichen Beleuchtung je Richtung prüfen
`ecken_im_blick_passen_zu_den_nachbarn` (`metatile.rs`) und
`ecken_der_seiten_im_blick` (`rasterizer.rs`), aus `nw` und `sw` mit
festen Zahlen. Cinematic prüfen ausserdem
`hdr_haelt_die_tiefe_der_vordersten_flaeche`,
`hdr_haelt_die_tiefe_des_vorderen_draws`,
`himmelslicht_der_oberwelt_in_jedem_biom` und
`wasser_spiegelt_den_himmel_des_bioms` (`metatile.rs`), mit dem Radius 2
aus `nw`, und `ein_baum_ein_look` (`cli.rs`): Ordner, `look`, `lookHash`, ein look,
den es nicht gibt, und `trees.json`. Die Werte des Looks, das Licht je
Stufe, die Umgebung, den Weissabgleich und den Himmel ohne Biom prüfen die
Unit-Tests in `look.rs` und `kino.rs` gegen Zahlen, die aus den Formeln
des Spiels, aus 0058 und aus 0069 gerechnet sind.

Dazu Unit-Tests in den Quelldateien: die Achse je Kamera, die Regel
„ganze Pixel“ und die genordete Projektion (`projection.rs`), Spalten,
Reihenfolge und Band für beide Azimute (`metatile.rs`), welche Flächen die
Kamera sieht (`rasterizer.rs`), Umriss, Deckung, Licht unbekannter Blöcke,
die Teile je Würfel ohne Naht und ein Modell ganz in einem fremden Würfel
(`sprites.rs`), Schalter und Meldungen (`cli.rs`). Zwei ignorierte Tests
laufen über alle Vanilla-Zustände: die Haarlinien von oben und bei
`north-45` und die Modelle, die in 2:1 ganz in einem fremden Würfel
liegen, siehe [Die Kamera](../renderer/kamera.md), „Von oben“,
„Genordet“ und „Sortiert wird nach Würfeln“.

## Mutationen

Ob ein Test eine Stelle wirklich prüft, zeigt eine Mutation: die Stelle
einzeln falsch machen, die Tests dazu laufen lassen, zurücksetzen. Fällt
kein Test, prüft ihn keiner. Mutationen bauen mit dem Profil `mutation`:
Es rechnet wie `release`, mit `codegen-units = 16` und `incremental`, und
baut nach einer Änderung neu in rund 11 statt 86 s, siehe
[Tests schneller](../messungen/2026-10-02-tests-schneller.md).

```bash
cargo nextest run --cargo-profile mutation -E 'binary(metatile)'
```

Messungen bauen weiter mit `release`.

## Goldbild

Unter `renderer/tests/fixtures/golden/` liegen Goldbilder: jede Änderung an
Projektion, Baking, Rasterizer oder Maleralgorithmus fällt damit auf.
`metatile.png` zeigt 2:1, `metatile-4x3.png`, `metatile-top.png`,
`metatile-top-north.png` und `metatile-north-45.png` die Szene aus
`common::szene` in 4:3, von oben, genordet von oben und in `north-45`,
`metatile-nw.png` dieselbe in 2:1 aus `nw` um die Treppe aus Stein,
`metatile-cinematic.png` sie mit Cinematic in 2:1 aus `se` um dieselbe
Treppe, alle bei scale 16. Der Test vergleicht alle, schreibt zu jedem
abweichenden das Ist-Bild daneben, als `<name>-ist.png`, und fällt erst
dann; in CI liegen sie als Artefakt am fehlgeschlagenen Lauf. Neu erzeugen nach einer gewollten Änderung: Skill
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

`kennzahlen_der_ansichten` in `renderer/tests/kennzahlen.rs` braucht dazu
die Testwelt: Er rendert die 24 Ansichten, an denen 0058 abgestimmt ist,
und schreibt je Pixel die Masken für die Kennzahlen des Looks. Aufruf und
Ergebnis in [Look am Renderer](../messungen/2026-10-03-look-am-renderer.md).
`bilder_zu_cinematic` in derselben Datei rendert die Bilder des Renderers
in [Cinematic](../renderer/cinematic.md) nach `BILDER_AUS`; es ruft der
Skill [`doku-bilder-rendern`](../../skills/doku-bilder-rendern/SKILL.md).
