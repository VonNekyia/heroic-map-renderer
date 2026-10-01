---
title: Der Weg einer Kachel
description: Vom Vorlauf bis zur WebP-Datei - Streifen und Cache je Thread, Bitmasken der Sections, Kandidaten, Blit, Kodieren und Speicher, mit den Messungen dazu.
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/heights.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/sprites.rs
  - renderer/src/world/chunk.rs
  - renderer/src/world/palette.rs
  - renderer/src/main.rs
  - renderer/build.rs
  - renderer/segmentheap.manifest
---

# Der Weg einer Kachel

Eine Kachel ist ein schräger Schnitt durch die volle Bauhöhe von 384
Blöcken: rund 320 000 Blockpositionen, gut hundert Chunks, und neun von zehn
nicht-leeren Blöcken liegen unter der Oberfläche. Der Renderer fasst deshalb
weder Luft noch Verdecktes an: Ein Vorlauf baut die Sprite-Tabelle, dann
rendern die Threads in Streifen mit warmem Cache, Bitmasken je Section
liefern die Kandidaten, eine Deckungsmaske siebt sie, und libwebp packt das
Ergebnis. Keiner dieser Umbauten hat einen Pixel geändert; die Messungen
stehen unten. Die Stationen stehen in `cli.rs` (`write_tiles`, `rendere`,
`verteile`) und
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs).

## Vorlauf

Der Vorlauf liest jeden Chunk einmal und beantwortet zwei Fragen auf einmal:
welche Blockstates vorkommen, samt den Daten der Blockentities, die ein Bild
ändern, und welche Kacheln überhaupt etwas zeigen (`survey` in
[`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)). Erst
danach steht die Sprite-Tabelle, und erst dann kann parallel gerendert
werden, denn sonst müsste jeder Worker sie unter einer Sperre füllen, siehe
[0003](../entscheidungen/0003-vorlauf-vor-dem-rendern.md). Die Kachelmenge
kommt aus den belegten Sections, nicht aus der Welthöhe. Chunks, die nicht
fertig erzeugt sind, zählt er und übergeht sie, siehe
[Welten und Kennung](../benutzung/welten.md), „Nicht fertig erzeugte
Chunks“.

Die Welt wird deshalb mehrmals durchlaufen: vom Vorlauf, von der Basis und
von jeder nativen Stufe, bei scale 32 mit allen dreien also fünfmal. In
jedem Durchgang dekodiert jeder Thread seine Chunks selbst. Die Höhen für
die Koordinatenanzeige liest der Vorlauf mit, aus der Heightmap jedes
Chunks, siehe [map.json](../benutzung/map-json.md), „Höhen“. Wie Muster und
Scherben in die Sprite-Tabelle kommen, steht in
[Blockentities](blockentities.md), „Daten aus dem Chunk“.

## Streifen und Cache je Thread

Gerendert wird in Streifen, Zeile für Zeile, bei scale 32 bis zu acht
Kacheln breit, auf der Basis und auf jeder nativen Stufe
(`streifenbreite`). Jeder Thread bekommt ein zusammenhängendes Stück, nimmt
es von vorn und holt sich, wenn er fertig ist, die hintere Hälfte des
grössten, das noch übrig ist (`verteile` in `cli.rs`). Gestohlen wird erst,
wenn vom grössten Stück noch vier Streifenbreiten übrig sind, bei scale 32
also 32 Kacheln: Wer stiehlt, fängt kalt an. Seine erste Kachel lädt bei
scale 32 rund 160 Chunks, so viel wie gut ein Dutzend warme; vier Kacheln
laden kalt 173 Chunks, warm sind es drei je Kachel. Kalt fängt ein Thread
sonst nur am Anfang an und an jeder Streifengrenze in seinem Stück. Rayon
zerteilt die Reihe dagegen schon beim Verteilen in viele kleine Stücke;
1024 Kacheln auf 24 Threads luden damit je Kachel doppelt so viele Chunks
wie in festen Stapeln.

Dabei behält jeder Thread seinen Chunk- und Regionscache und seinen
Zeichner für die Karte über den ganzen Lauf; geteilt wird nur die
unveränderliche Sprite-Tabelle. Eine Zeile teilt sich fast alle Chunks mit
der darüber; der Cache lädt je Zeile nur die paar neuen am unteren Rand.
Wie breit ein Streifen ist, hängt an den Kacheln je Thread: breite laden je
Kachel weniger nach, ihre erste Zeile aber mehr. Im Mittel am wenigsten
lädt, wer den Streifen etwa so breit macht wie die Wurzel aus einem Zehntel
der Kacheln je Thread (`breite_der_streifen`): zwei Spalten bei den 1024
Kacheln eines 8192er-Ausschnitts auf 24 Threads, acht, so viel wie der
Cache hält, bei einer ganzen Welt. Die Breite ist eine Zweierpotenz, damit
Geschwister ab zwei Spalten im selben Streifen liegen. Bei einer Spalte, ab
scale 4 oder unter rund 20 Kacheln je Thread, liegen sie eine Spalte
auseinander; zwei Spalten hielten dort je Thread eine Kachel mehr im Cache,
bei scale 4 rund 250 Chunks. Siehe
[0025](../entscheidungen/0025-streifen-und-cache-je-thread.md).

Ohne Cache über Kacheln hinweg dekodierte jede Kachel ihre gut hundert
Chunks neu, einfädig 15 von 64 ms, siehe
[2026-09-23, Phasen je Kachel](../messungen/2026-09-23-phasen-je-kachel.md).

Der Cache hält je Section und Paletteneintrag den Familienindex, die
Blockstate wird einmal je Section gehasht statt einmal je Block, und der
Nachschlag merkt sich den letzten Chunk, statt je Block zu hashen.

Dazu hält er je Chunk sein ausgebreitetes Licht (`ChunkLicht` in
[`renderer/src/render/licht.rs`](../../renderer/src/render/licht.rs)), je
Section ein Byte je Zelle oder eines für alle, gerechnet, wenn der Chunk
zum ersten Mal Licht braucht (`licht_slot`), siehe
[Wasser und Licht](wasser-und-licht.md), „Licht ausbreiten“. Dafür lädt er
die acht Nachbarn mit. Am Rand eines Streifens fragen die Ecken der
weichen Beleuchtung auch nach dem Licht von Chunks, die der Streifen nie
zeichnet, und deren Nachbarn sind ein zweiter Ring. Den fasst danach keine
Kachel mehr an: `next_tile` verwirft ihn, sobald er eine Zeile lang nicht
gebraucht wurde, und kommt die nächste Zeile wieder an diesen Rand, lädt sie
ihn neu. Die Zahlen oben, Chunks je Kachel und je Thread, sind von vor dem
Licht. Seitdem dekodiert ein Lauf der Testwelt über alle Stufen 6 bis 9 %
öfter, um (-64, 416) 10,6 statt 9,8 Mal je Chunk; das Rechnen des Lichts
kostet mehr als das Lesen, siehe
[2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md).

Doppelt dekodiert wird nur an den Grenzen der Streifen: Ein Chunk, der in
zwei Streifen reicht, wird in jedem geladen, und innerhalb eines Streifens
verdrängt der Cache nichts, was er noch braucht. Wie oft das vorkommt,
hängt an der Breite der Streifen und daran, wie die Threads sie gehen,
gezählt in
[2026-09-29, Doppelte Arbeit an Streifengrenzen](../messungen/2026-09-29-streifengrenzen.md):

- **Vollrender:** Jeder Thread geht mehrere ganze Streifen zu 8 Spalten
  nacheinander. Auf der Basis wird jeder Chunk im Mittel 1,73-mal
  dekodiert, vor dem Licht 1,56-mal, und sein Licht 1,31-mal gerechnet.
- **Ausschnitte mit allen Threads:** schmalere Streifen und kalte Starts
  mitten im Streifen. Über 65 536 Kacheln der grossen Welt lud eine Kachel
  vor dem Licht im Mittel 2,0 Chunks neu, bei etwa einer Kachel je Chunk
  also rund zwei Dekodierungen je Chunk. Am Stand der Testwelt mit 4
  Spalten sind es mit Licht 2,86 Dekodierungen und 1,68 Lichtrechnungen.
  Was an Ausschnitten gemessen ist, überzeichnet deshalb, was Dekodieren
  und Licht am Vollrender kosten.

Zwei Hebel dagegen sind durchgerechnet und verworfen:

- **Streifen zu 16 Spalten** statt 8 (`streifenbreite`): 1,31 statt 1,73
  Dekodierungen und 1,13 statt 1,31 Lichtrechnungen je Chunk, gerechnet
  rund 4 % weniger CPU-Zeit. Dafür hält jeder Thread gut 340 Chunks mehr
  im Cache, bis rund 100 MB.
- **Ein Cache für alle Threads:** Er hilft nur, wenn die Threads
  benachbarte Streifen Zeile für Zeile nebeneinander gehen. Im Vollrender
  braucht die Chunks an einer Grenze derselbe Thread erst einen ganzen
  Streifen später wieder, und kein anderer Thread zur selben Zeit; so lange
  hält sie auch ein geteilter Cache nicht. Die Threads im Gleichschritt
  über benachbarte Streifen zu führen, baut Verteilung und Cache um und
  spart gerechnet höchstens 7 %, wenn gar nichts mehr doppelt gerechnet
  wird.

## Bitmasken

Jede Section hält je Spalte ein 16-Bit-Wort je Eigenschaft (Bit = y):
"vorhanden", "deckend", "deckt den Boden", "Wasser", "Lava", "nur Wasser",
"nur Lava", "lose" und "hat Teile in Nachbarwürfeln", dazu für die weiche
Beleuchtung "dunkelt ab" und "nimmt die Sicht", für die Ausbreitung des
Lichts "dämpft" und "dicht" und für beides "voll hell" (`Masks` in
`metatile.rs`, die Ebenen `PRESENT` bis `VOLL`). Diese fünf gelten auch für
Blöcke ohne Familie. Die Ausbreitung nimmt ausserdem je Section zwei Listen
mit: die Blöcke mit einer Fläche, die Licht an einer Seite aufhält
(`formen`), und die, die leuchten, mit ihrer Stufe (`quellen`). Verdeckt ist ein Block, wenn die Nachbarn
nach +x und +z deckend sind und der nach +y seinen Boden deckt, siehe
[Sprites und Deckung](sprites-und-deckung.md), „Verdeckte Würfel“, und das
ist je Spalte eine Handvoll Wortoperationen für sechzehn Blöcke auf einmal:
nach +y ein Shift, an den Rändern kommt das Bit aus der Section darüber oder
dem Nachbarchunk.

Reine Flüssigkeit, Wasser wie Lava, fällt ausserdem weg, wo über ihr
dieselbe steht und sie seitlich an dieselbe mit derselben darüber oder an
Deckendes grenzt: dann bleibt von ihr keine Fläche und kein Streifen, und
das Innere eines Ozeans oder Lavasees kommt gar nicht erst zur Sprite-Wahl.
Seitlich genügt ein deckender Nachbar, denn mit derselben Flüssigkeit
darüber reicht die Seitenfläche bis zur Kante, und der Nachbar übermalt sie
danach. Lava deckt selbst nur bei scale 4; bei anderen fiele ohne diese
Regel kein Lavablock weg. Ein Dach statt derselben Flüssigkeit darüber
genügt nicht, denn ohne sie endet die Oberfläche bei 8/9 und ragt in die
Seiten hinein.

Aus den Masken fallen die Kandidaten heraus, ohne dass Luft je angefasst
wird; sortiert nach `(y, v, u)` sind sie genau die Zeichenreihenfolge des
Maleralgorithmus. Die Masken entstehen je Klasse gleicher Bits, Luft, Stein,
Wasser: je Block ein OR, danach setzt sich jede Eigenschaft aus den Klassen
zusammen. Siehe
[0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md).

## Kandidaten

Die Sammelschleife lief je Kachel über alle 24 Sections aller gut hundert
Band-Chunks, 256 Spalten je Section. Das Band erreicht in einem Chunk aber
nur rund 36 Höhen, also drei Sections; die Umkehrung von `v_window` grenzt
sie ein, und ein Flag je Section sagt, ob überhaupt ein Kandidat
drinsteht. Entscheidend bleibt die Prüfung je Block; das spart nur die
Schleife über Sections, die das Band gar nicht berührt.

Das Band um die Kachel hat drei Blöcke Reserve für Modelle, die aus ihrem
Würfel ragen (`BLEED_BLOCKS`). Für alles, was im Umriss seines Würfels
bleibt, zählt der Kasten dieses Umrisses: Kandidaten, die die Kachel gar
nicht berühren, fallen weg. Im Band lag sonst mehr als die Hälfte der
Kandidaten neben der Kachel, und jeder bekam eine Sprite-Wahl. Lose Familien
und Teile in fremden Würfeln behalten das Band.

Danach siebt die Deckungsmaske, siehe
[Sprites und Deckung](sprites-und-deckung.md), „Deckungsmaske“.

## Blit

Der Blit zeichnet die sichtbaren Pixel jedes Draws und mischt mit `over`,
ganzzahlig, auf der CPU wie im Shader der Karte; deckende Pixel schreibt er
direkt statt durch `over` (`mische`). Vorher tönt er jeden Pixel eines
Sprites mit Tönungskarte in den Farben seines Blocks (`tinted`), siehe
[Biomfarben](biomfarben.md), „Tönung beim Zeichnen“, und multipliziert ihn
dann mit dem Licht des Blocks und dem Faktor der weichen Beleuchtung, siehe
[Wasser und Licht](wasser-und-licht.md) und
[Weiche Beleuchtung](weiche-beleuchtung.md). Auf der Karte: siehe
[Grafikkarte](../benutzung/grafikkarte.md).

## Grosse Ausschnitte

`--render` rendert alles über 1024 Pixel Kantenlänge in Stücken und setzt
sie zusammen (`STUECK`), sonst hielte die Deckungsmaske die sichtbaren Pixel
des ganzen Bilds bis zum Schluss. Bei `--render --size 16384` braucht der
Lauf damit gut 2 GiB statt 3,3 bis 5,3, gemessen bei scale 32 und 16, siehe
[2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md).
Bei scale 8 kämen ohne Stücke gerechnet rund 5 GiB dazu. Am Code stand dafür
„1 bis 14 GB mehr, je nach scale“; woher die 14 kommen, ist nicht
festgehalten.

## Kodieren

WebP wird **verlustfrei** geschrieben, mit libwebp auf Stufe 0 (`encode_webp`
in `tiles.rs`), siehe [0004](../entscheidungen/0004-webp-verlustfrei.md) und
[0028](../entscheidungen/0028-libwebp-statt-image.md). Minecraft-Texturen
sind Pixelkunst mit wenigen flachen Farben; verlustbehaftet würde daraus
Matsch, und an den Kachelrändern sähe man die Artefakte im Raster. libwebp
nutzt Palette, Farbcache und Rückverweise; Stufe 0 ist die schnellste,
höhere sparen wenig und kosten ein Vielfaches. Die Kacheln werden ein
Drittel so gross wie mit dem einfachen Encoder aus `image`, auf dichtem Land
halb so gross, über Ozean ein Viertel bis ein Achtel, Pixel für Pixel
gleich.

`exact` behält die Farbe voll durchsichtiger Pixel, sonst setzt libwebp sie
auf 0 und die Kachel käme nur fast zurück. `libwebp-sys` baut libwebp aus
dem mitgelieferten C-Quelltext. Dafür braucht es einen C-Compiler, unter
Windows den von Visual Studio, den Rust dort ohnehin verlangt, unter Linux
gcc oder clang. libwebp selbst steht unter BSD-3-Clause.

Dekodiert wird ebenfalls mit libwebp, `decode_webp` in `tiles.rs`: die
Pyramide liest so ihre Kinder, siehe
[Zoomstufen](../benutzung/zoomstufen.md), „Von der Platte“.

Das erste Kodieren und Dekodieren im Prozess läuft allein: `encode_webp` und
`decode_webp` warten dafür auf ein `std::sync::Once` in
`richte_libwebp_ein`, das einmal ein Bild mit 16 × 16 Pixeln kodiert und
wieder dekodiert. Dabei richtet libwebp seine Tabellen für SSE2 und AVX2
ein. In libwebp 1.6.0, das `libwebp-sys` 0.14.4 mitbringt, geschieht das
unter Windows ohne Sperre (`WEBP_DSP_INIT` in `src/dsp/cpu.h`), und MSVC
baut den AVX2-Code immer ein (`WEBP_MSC_AVX2`). `VP8LEncDspInitSSE2`
kopiert am Ende die Tabelle `VP8LPredictorsSub` in `VP8LPredictorsSub_SSE`,
für das Dekodieren `VP8LDspInitSSE2` ebenso `VP8LPredictorsAdd` in
`VP8LPredictorsAdd_SSE`. Über diese Kopien rechnen die AVX2-Prädiktoren den
Rest einer Zeile, der kürzer als 8 Pixel ist. Richten zwei Threads zugleich
ein, kann eine Kopie schon die AVX2-Einträge des anderen enthalten. Dann
ruft sich ein AVX2-Prädiktor für den Rest selbst auf, und das bleibt so,
bis der Prozess endet: Im Debug-Build, also in den Tests, bricht er mit
einem Stack Overflow ab, im Release-Build hängt der Thread ohne Meldung.
Offen ist das Fenster, solange mehrere Threads ihr erstes Bild kodieren
oder dekodieren, also zu Beginn jedes Laufs. Mit `--pyramid` dekodieren
sie zuerst, ebenso ein Export, der keine Basiskachel mehr rendert, etwa
`--resume` nach einem Abbruch in der Pyramide.
Unter Linux sperrt libwebp mit einem Mutex, und `libwebp-sys` baut es dort
ohne AVX2.

libwebp hat beides nach 1.6.0 behoben, in den Commits `54f23b0`, eine Sperre
für `WEBP_DSP_INIT` unter Windows, und `de6aee4`, die Zuweisungen an die
`_SSE`-Tabellen umgestellt. In einem Release sind sie noch nicht. Bringt ein
`libwebp-sys` ein neueres libwebp mit, ist zu prüfen, ob
`richte_libwebp_ein` das Warten noch braucht. Es kostet ein kleines Bild je
Prozess und danach ein atomares Lesen je Kachel.

## Speicher

Der Rust-Teil allokiert über mimalloc: Parallel dauerte ein
Chunk-Dekodieren sonst sechsmal so lang wie allein, weil der Windows-Heap
die vielen kleinen Allokationen des NBT-Lesers über 24 Threads
serialisiert, siehe [0020](../entscheidungen/0020-mimalloc.md).

libwebp holt sich je Kachel rund 2 MB über `malloc` der C-Laufzeit, nicht
über mimalloc, und der gewöhnliche Heap von Windows gibt sie beim Freigeben
ans System zurück: gut 500 Seitenfehler je Kachel, die sich auf vielen
Threads stauen. Unter Windows bekommt das Binär deshalb ein Manifest mit dem
Segment-Heap, siehe [`renderer/build.rs`](../../renderer/build.rs) und
[0029](../entscheidungen/0029-segment-heap-fuer-libwebp.md), und der behält
den Speicher.

Jeder Thread hält eine Zeile seines Streifens im Cache, auf der grossen
Welt gemessen höchstens 430 bis 520 Chunks bei scale 32 und 825 bei
scale 4, samt dem Viertel Spielraum aus `CACHE_CHUNKS`; mit Karte dazu
sechzehn Zeichenlisten. Gemessen vor dem Licht: Dazu kommen jetzt der
zweite Ring am Rand des Streifens, siehe „Streifen und Cache je Thread“,
und je Chunk mit Licht bis 4 KB je Section, in der nicht jede Zelle
dasselbe Licht hat. Auf der Testwelt liegt die Spitze damit auf 24 Threads
0,4 bis 0,8 GiB höher, im Median 0,5 bis 0,65 GiB, siehe [2026-09-29, Licht ausbreiten](../messungen/2026-09-29-licht-ausbreiten.md).

Die Sprite-Tabelle teilen sich alle Threads. Fast jedes Sprite hat eine
AO-Karte, 4 Bytes je Pixel wie das Bild, siehe
[Weiche Beleuchtung](weiche-beleuchtung.md), „Was es kostet“.

## Was die Zeit bringt, gemessen

| Messung | Was |
|---|---|
| [2026-09-22, Erster Vollrender](../messungen/2026-09-22-erster-vollrender.md) | die Ausgangslage auf der grossen Welt |
| [2026-09-23, Phasen je Kachel](../messungen/2026-09-23-phasen-je-kachel.md) | Cache, Bitmasken, mimalloc, Flächen, Sammeln, je Phase |
| [2026-09-25, Bitmasken](../messungen/2026-09-25-bitmasken.md) | vorher gegen nachher auf den Regeln von #9 |
| [2026-09-26, Grafikkarte](../messungen/2026-09-26-grafikkarte.md) | CPU gegen Karte |
| [2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md) | Streifen, Kandidaten, Deckungsmaske |
| [2026-09-27, libwebp](../messungen/2026-09-27-libwebp.md) | Grösse und Dauer des Kodierens, Segment-Heap |

Was nach den Umbauten bleibt, verteilt sich: Chunks dekodieren, die
Kandidaten aus den Masken, die Sprite-Wahl, das Zeichnen, das Kodieren.
Auf einem Kern gewinnen die Umbauten mehr als auf 24 Threads, denn dort
teilen sich die Threads Kerne und Speicherbandbreite.

## Nicht im Code

Gemessen und verworfen, jeweils mit Grund in der Entscheidung:

- Zeilenspannen im Blit samt `memcpy` deckender Zeilen, der Nachbar auf der
  Blickachse als vierte Deckungsrichtung, `zlib-rs` statt `miniz` und ein
  eigener serde-Visitor für die Properties:
  [0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md);
- eigene Threads zum Schreiben:
  [0027](../entscheidungen/0027-keine-schreibthreads.md);
- Rayon mit einem Cache je Thread und der Verteiler der ersten Fassung:
  [0025](../entscheidungen/0025-streifen-und-cache-je-thread.md).
