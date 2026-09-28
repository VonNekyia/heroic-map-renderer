---
title: Der Weg einer Kachel
description: Vom Vorlauf bis zur WebP-Datei - Streifen und Cache je Thread, Bitmasken der Sections, Kandidaten, Blit, Kodieren und Speicher, mit den Messungen dazu.
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/heights.rs
  - renderer/src/render/metatile.rs
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
welche Blockstates vorkommen, und welche Kacheln überhaupt etwas zeigen
(`survey` in
[`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)). Erst
danach steht die Sprite-Tabelle, und erst dann kann parallel gerendert
werden, denn sonst müsste jeder Worker sie unter einer Sperre füllen, siehe
[0003](../entscheidungen/0003-vorlauf-vor-dem-rendern.md). Die Kachelmenge
kommt aus den belegten Sections, nicht aus der Welthöhe.

Die Welt wird deshalb mehrmals durchlaufen: vom Vorlauf, für die Höhen,
von der Basis und von jeder nativen Stufe, bei scale 32 mit allen dreien
also sechsmal. Die Höhen brauchen einen eigenen Durchgang nach der
Sprite-Tabelle, denn erst sie sagt, welcher Block ein Sprite bekommt, siehe
[map.json](../benutzung/map-json.md), „Höhen“. In jedem Durchgang dekodiert
jeder Thread seine Chunks selbst.

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
bei scale 4 rund 250 Chunks. Über 65 536 Kacheln lädt
eine Kachel der Basis im Mittel gut zwei Chunks neu; bei etwa einer Kachel
je Chunk wird dort jeder Chunk rund zweimal dekodiert. Siehe
[0025](../entscheidungen/0025-streifen-und-cache-je-thread.md).

Ohne Cache über Kacheln hinweg dekodierte jede Kachel ihre gut hundert
Chunks neu, einfädig 15 von 64 ms, siehe
[2026-09-23, Phasen je Kachel](../messungen/2026-09-23-phasen-je-kachel.md).

Der Cache hält je Section und Paletteneintrag den Familienindex, die
Blockstate wird einmal je Section gehasht statt einmal je Block, und der
Nachschlag merkt sich den letzten Chunk, statt je Block zu hashen.

## Bitmasken

Jede Section hält je Spalte ein 16-Bit-Wort je Eigenschaft (Bit = y):
"vorhanden", "deckend", "deckt den Boden", "Wasser", "Lava", "nur Wasser",
"nur Lava", "lose" und "hat Teile in Nachbarwürfeln", dazu für die weiche
Beleuchtung "dunkelt ab" und "nimmt die Sicht" (`Masks` in `metatile.rs`,
die Ebenen `PRESENT` bis `VIEW`). Verdeckt ist ein Block, wenn die Nachbarn
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
sechzehn Zeichenlisten.

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
