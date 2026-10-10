---
title: Schalter und Beispiele
description: Alle Schalter von heroic-map-renderer mit einer Zeile, die Aufrufe, die keine Kacheln schreiben, mit Beispielausgabe, und was Threads und niedrige Priorität am Betriebssystem setzen.
code:
  - renderer/src/cli.rs
  - renderer/src/main.rs
---

# Schalter und Beispiele

`heroic-map-renderer` ist ein einziges Binär; welche Arbeit es tut, entscheiden
die Schalter. `--tiles` exportiert Kacheln, `--pyramid` baut nur Zoomstufen
nach, `--heights` trägt nur die Höhen nach, `--render`, `--sprite`,
`--block`, `--at` und `--scan` helfen beim Ansehen und Prüfen. Die Schalter stehen in `Args` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs); `--help` nennt dieselben
Texte. Ohne Schalter startet unter Windows an einer Konsole der
[Assistent](assistent.md).

## Alle Schalter

| Schalter | Wirkung | Mehr |
|---|---|---|
| `--world DIR` | Weltwurzel mit `level.dat` oder eine Dimension darin | [Welten und Kennung](welten.md) |
| `--assets DIR` | Asset-Wurzel, mehrfach, spätere überschreiben frühere | [Assets und Biomdaten](assets.md) |
| `--data DIR` | Datenwurzel mit Biomdefinitionen und Bannermustern, mehrfach | [Assets und Biomdaten](assets.md) |
| `--download-client-jar` | Assets und Daten aus dem Client-Jar von Mojang, vor allen `--assets` und `--data`; lädt es einmal in den Cache. Der Schalter ist die Zustimmung | [Assets und Biomdaten](assets.md), „Von Mojang laden“ |
| `--client-version VERSION`, `--cache-dir DIR` | nur mit `--download-client-jar`: die Version statt der aus `level.dat`, der Ordner des Caches | [Assets und Biomdaten](assets.md), „Von Mojang laden“ |
| `--at X Y Z` | die Blockstate an dieser Weltkoordinate ausgeben | unten |
| `--block BLOCKSTATE` | eine Blockstate auflösen, mehrfach | unten |
| `--sprite DATEI` | die Blockstates aus `--block` als Sprites in eine PNG rastern | unten |
| `--scale N` | Pixelbreite eines Blocks, ab 4 (scale 1 nur mit `--flat`), Vorgabe 32, bei `top-north` und `north-45` 16; jede Blockecke muss bei der Kamera auf ganzen Pixeln liegen, in 2:1 ein Vielfaches von 4 | [Kamera](../renderer/kamera.md), „Ganze Pixel“ |
| `--camera KAMERA` | diagonal `W:H` schräg von 2:1 bis 1:1 oder `top` von oben, genordet `top-north` von oben oder `north-45` schräg von Süden, Vorgabe `2:1`; jede Kamera schreibt in ihren eigenen Baum | [Kamera](../renderer/kamera.md), „Kameras“ |
| `--direction RICHTUNG` | wo die Kamera steht: diagonal `se`, `sw`, `nw` oder `ne`, genordet `s`, `w`, `n` oder `e`, Vorgabe `se` und `s` | [Kamera](../renderer/kamera.md), „Richtungen“ |
| `--biome-blend N` | wie weit Gras, Laub und Wasser über Biomgrenzen gemischt werden, 0 bis 7 Blöcke wie der Biomübergang im Spiel, Vorgabe 2; ein bestehender Kachelbaum behält seinen | [Biomfarben](../renderer/biomfarben.md), [map.json](map-json.md) |
| `--render DATEI` | einen Weltausschnitt in eine PNG rendern | unten |
| `--cinematic` | mit `--render` oder `--tiles`: im Licht des Spiels in HDR zeichnen statt als Karte, in einen eigenen Baum; zeichnet auf der CPU | [Cinematic](../renderer/cinematic.md) |
| `--flat` | mit `--render` oder `--tiles`: die einfarbige Ansicht, `top-north` bei scale 1 mit einer Farbe je Block und Relief, in einen eigenen Baum; nicht mit `--scale`, `--camera`, `--direction`, `--cinematic` oder `--native-levels`; zeichnet auf der CPU | [Die einfarbige Ansicht](../renderer/einfarbig.md) |
| `--center X Z` | der Punkt der Welt in der Bildmitte, Vorgabe `0 0` | unten |
| `--size N` | Kantenlänge des Ausschnitts in Pixeln, ab 1; für `--render` Vorgabe 1024, ohne Angabe deckt `--tiles` die ganze Welt | [Kacheln exportieren](kacheln.md) |
| `--area X0 Z0 X1 Z1` | nur dieses Rechteck der Welt zeichnen, zwei inklusive Ecken in Blöcken, nach aussen auf ganze Chunks gerundet; ein Kachelbaum behält seins | [Kacheln exportieren](kacheln.md), „Ein Rechteck der Welt: `--area`“ |
| `--scan` | jeden Chunk dekodieren, auch die nicht fertig erzeugten, mit `--assets` die Blockstates der übrigen auflösen und rastern | unten |
| `--tiles DIR` | die Welt als WebP-Kacheln exportieren; `DIR` ist die Wurzel, jeder Baum liegt darunter in seinem Ordner | [Kacheln exportieren](kacheln.md) |
| `--prune` | mit `--tiles`: Kacheln entfernen, die kein Chunk mehr berührt, und Höhen von Regionen ohne Regionsdatei | [Kacheln exportieren](kacheln.md) |
| `--native-levels N` | mit `--tiles`: so viele gröbere Stufen aus der Welt rendern, Vorgabe 0 | [Zoomstufen](zoomstufen.md) |
| `--compact` | mit `--tiles`: kompakt packen, rund die Hälfte der Bytes für ein Mehrfaches der Zeit beim Kodieren; ein neuer Baum merkt es sich, ein bestehender behält seine Packung | [map.json](map-json.md), „Packen“ |
| `--resume` | mit `--tiles`: einen abgebrochenen Lauf fortsetzen | [Pyramide und Fortsetzen](pyramide-und-resume.md) |
| `--update` | mit `--tiles`, ohne `--size`: nur zeichnen, wo sich die Welt seit dem letzten vollen Lauf oder Update geändert hat | [Updates](updates.md) |
| `--gpu auto\|on\|off` | mit `--tiles`: die Grafikkarte zeichnet, Vorgabe `auto`; mit `--cinematic` und `--flat` immer die CPU; mit `--threads` nie ein Software-Adapter | [Grafikkarte](grafikkarte.md) |
| `--progress text\|json` | mit `--tiles`: den Fortschritt als `n/N Kacheln` oder als JSON-Zeilen melden, Vorgabe `text` | [Kacheln exportieren](kacheln.md), „Fortschritt als JSON: `--progress`“; Vertrag in [Plugin](../plugin.md), „Fortschritt als JSON“ |
| `--threads N` | so viele Threads für jede Phase, ab 1; geht `RAYON_NUM_THREADS` vor; ohne Angabe so viele, wie es logische CPUs gibt | unten, „Threads und Priorität“ |
| `--low-priority` | mit niedrigster Priorität laufen, damit etwa ein Server daneben vorgeht | unten, „Threads und Priorität“ |
| `--estimate` | mit `--tiles`: nur schätzen, wie viele Kacheln, wie viel Platz und wie lange der Lauf braucht und ob der Platz reicht; schreibt unter `--tiles` nichts | [Was ein Lauf kostet](kosten.md), „Schätzen: `--estimate`“ |
| `--defender-exclusion` | mit `--tiles`, nur unter Windows: eine Ausnahme im Echtzeitschutz setzen | [Echtzeitschutz](echtzeitschutz.md) |
| `--pyramid DIR` | Zoomstufen und `map.json` aus den Basiskacheln nachbauen, ohne Welt und Assets; daneben nur `--threads`, `--low-priority` und `--manifest` | [Pyramide und Fortsetzen](pyramide-und-resume.md) |
| `--compact-tree DIR` | einen fertigen Baum kompakt nachpacken, wie mit `--compact`, ohne Welt und Assets; jede Kachel behält ihre Zeit; daneben nur `--threads`, `--low-priority` und `--manifest` | [Kacheln exportieren](kacheln.md), „Nachverdichten“ |
| `--manifest` | mit `--tiles`, `--pyramid` oder `--compact-tree`: am Ende das Manifest des Baums schreiben, je Kachel Grösse und ETag, für den Download; ohne entfernt ein Lauf, der Kacheln schreibt, ein altes | [Plugin](../plugin.md), „Manifest“ |
| `--serve DIR` | die Wurzel von `--tiles` unter `/tiles/` ausliefern, mit `--web` die Karte dazu; daneben nur `--threads`, `--low-priority` und seine eigenen Schalter | [Server](server.md) |
| `--web DIR`, `--listen ADRESSE:PORT`, `--max-connections N`, `--header-timeout S`, `--max-header-bytes N`, `--max-headers N`, `--write-timeout S`, `--exit-with-stdin`, `--tls-cert DATEI`, `--tls-key DATEI`, `--secret-file DATEI`, `--site-url URL`, `--site-title TEXT`, `--site-description TEXT`, `--site-image PFAD` | nur mit `--serve`: Seite, Adresse, Grenzen am offenen Netz, Ende mit stdin, HTTPS, Download, Angaben der Seite | [Server](server.md), „Aufruf“ |
| `--heights DIR` | die Höhen für die Koordinatenanzeige in einen bestehenden Baum schreiben, ohne zu rendern; `DIR` ist der Ordner des Baums; braucht nur `--world` | [map.json](map-json.md), „Höhen“ |

## Einen Block ansehen: `--at`

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
Biom der Zelle:            minecraft:forest
Biom des Blocks:           minecraft:forest
```

`Biom der Zelle` ist das gespeicherte Biom der Zelle aus 4×4×4 Blöcken,
`Biom des Blocks` das Biom des Blocks nach dem Zoom des Spiels; gemischt
wird darüber erst beim Zeichnen. Ohne Seed in der Welt fehlt die zweite
Zeile. Siehe [Biomfarben](../renderer/biomfarben.md), „Biom je Block“.

Ist der Chunk nicht fertig erzeugt, sagt eine Zeile unter seinem Status,
dass der Renderer ihn nicht zeichnet. Welche Welten und welche Chunks der
Renderer liest, steht in [Welten und Kennung](welten.md).

## Eine Blockstate auflösen: `--block`

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

Ein Block mit Blockentity-Renderer nennt dazu die Flächen, die das Spiel
aus dessen Modell zeichnet, und ihre Texturen. Die Truhe hat ein
Blockmodell ohne Elemente, siehe [Blockentities](../renderer/blockentities.md):

```
minecraft:chest[facing=north,type=single,waterlogged=false]
  minecraft:block/chest
      0 Elemente, 0 Flächen
  Blockentity: 18 Flächen
      minecraft:entity/chest/normal
```

## Sprites rastern: `--sprite`

Einzelne Blockstates als Sprites rastern, hier die zwanzig aus dem Bild:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --assets ./vanilla-assets --assets ./assets --scale 64 --sprite docs/bilder/sprites.png \
  --block "stone" --block "grass_block[snowy=false]" --block "oak_log[axis=y]" --block "crafting_table" --block "glass" \
  --block "furnace[facing=north,lit=false]" --block "furnace[facing=east,lit=false]" --block "furnace[facing=south,lit=false]" --block "furnace[facing=west,lit=false]" --block "torch" \
  --block "oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]" --block "oak_stairs[facing=north,half=top,shape=straight,waterlogged=false]" \
  --block "oak_slab[type=bottom,waterlogged=false]" --block "oak_fence[north=true,east=true,south=false,west=false,waterlogged=false]" \
  --block "cobblestone_wall[east=none,north=low,south=low,up=true,waterlogged=false,west=none]" \
  --block "oak_door[facing=east,half=lower,hinge=left,open=false,powered=false]" --block "lily_pad" \
  --block "oak_leaves[distance=1,persistent=false,waterlogged=false]" --block "short_grass" \
  --block "oak_hanging_sign[attached=false,rotation=3,waterlogged=false]"
```

![Sprites](../bilder/sprites.png)

## Einen Ausschnitt rendern: `--render`

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render docs/bilder/map.png --center -64 416 --size 900 --scale 16
```

```
Render:     390 Chunks gelesen, 215 Blockstates, 536 Sprites
            900x900 px bei (-4290, 958) und scale 16 in 0.3 s -> docs/bilder/map.png
```

`--center` nennt den Punkt (X, 0, Z) der Welt, die Ecke der Blockspalte
(X, Z) mit kleinstem x und z; er landet aus jeder Richtung in der
Bildmitte. `--scale` nennt die Pixelbreite eines Blocks, `--size` die
Kantenlänge, mindestens 1. Ein Punkt in Höhe y liegt schräg y · b Pixel
über dem Punkt in Höhe 0 darunter; welcher Punkt einer Oberfläche dann in
der Mitte zu sehen ist, hängt an a und damit an der Kamera, siehe
[Kamera](../renderer/kamera.md), „Projektion“. Von oben liegt der Punkt
selbst in der Mitte, die Ecke von vier Spalten. Wer bei jeder Kamera
denselben Punkt in der Mitte will, rechnet `--center` je Kamera und
Richtung, wie `mitte` in
[`skills/doku-bilder-rendern/bilder-rendern.py`](../../skills/doku-bilder-rendern/bilder-rendern.py). Die
Ausgabe nennt die linke obere Bildecke in Pixeln. Die Sprite-Tabelle kommt
aus demselben Vorlauf wie beim Kachelexport, nur über den Ausschnitt, und
der dekodiert nur, was im Bild landen kann: der sichtbare Bereich ist
diagonal schräg ein schmales diagonales Band in x und z, kein Rechteck,
genordet ein Rechteck. Wer
stattdessen die Hüllbox nähme, läse in 2:1 für einen 1024er Ausschnitt rund
das Sechzehnfache an Chunks.

Alles über 1024 Pixel Kantenlänge rendert `--render` in Stücken, siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Grosse Ausschnitte“.

## Die ganze Welt prüfen: `--scan`

Ein Durchlauf über die gesamte Testwelt, der jeden Chunk dekodiert, auch
die nicht fertig erzeugten, und die Blockstates der übrigen auflöst und
rastert:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --scan
```

```
Scan:       316223 Chunks in 56.4 s (5611 Chunks/s), 0 Fehler
            davon 67120 nicht fertig erzeugt, der Renderer zeichnet sie nicht
            3107 verschiedene Blockstates
            96 Banner mit Mustern, 2073 Krüge mit Scherben, 20 verschiedene samt Block
Assets:     3107 Blockstates aufgelöst in 0.4 s, 0 ungelöst
            2 Blöcke ohne Modell:
            minecraft:air
            minecraft:cave_air
Sprites:    3093 gerastert bei scale 32 in 0.5 s (5913/s)
            9.1 MB Sprite-Pixel, größtes: minecraft:brain_coral_fan[waterlogged=true] (46x31)
            1 Blöcke sind aus dieser Blickrichtung unsichtbar: minecraft:fire

Texturen:   722 geladen, 0 fehlen
```

MB zählt die Ausgabe binär, 2^20 Byte.

- **Banner und Krüge:** Die Zeile zählt Blockentities, deren Daten ihr Bild
  ändern. „Verschieden samt Block“ zählt Paare aus Blockstate und Daten: So
  viele Familien baut eine Sprite-Tabelle höchstens dazu, jede native Stufe
  ihre eigene, siehe [Blockentities](../renderer/blockentities.md), „Im
  Renderpfad“.
- **Blöcke ohne Modell:** Truhen, Banner und die übrigen Blockentities
  stehen nicht in der Liste, sie bekommen ihr Bild aus dem Blockentity.
  Wasser fehlt, weil der Renderer es im Code baut. Beides steht in
  [Modelle und Texturen](../renderer/modelle-und-texturen.md),
  „Was kein Blockmodell hat“.

## Threads und Priorität

`--threads` und `--low-priority` gelten für jeden Aufruf, auch für
`--pyramid`. Beides setzt der Renderer am Anfang, vor dem ersten Thread:
Neue Threads erben die Priorität, und rayon legt seinen Pool beim ersten
Gebrauch an.

- **`--threads N`:** Der Pool von rayon hat `N` Threads. Er trägt jede
  Phase, Vorlauf, Basis, native Stufen und Pyramide; libwebp startet keine
  eigenen. Die Zeile „Kacheln:“ nennt die Zahl. Die Kacheln sind mit einem
  Thread bytegleich zu denen mit allen, das prüft
  `ein_thread_mit_niedriger_prioritaet_gleicht_dem_vollen_lauf` in
  [`renderer/tests/cli.rs`](../../renderer/tests/cli.rs).
- **`--low-priority`:** Die Zeile „Priorität:“ sagt, was gesetzt ist.

| System | Rechenzeit | I/O und Speicher |
|---|---|---|
| Windows | `IDLE_PRIORITY_CLASS` | nichts |
| Linux | `SCHED_IDLE`, wenn das nicht geht `nice 19` | I/O-Klasse idle über `ioprio_set` |
| andere Unix | `nice 19` | nichts |

Den Hintergrundmodus von Windows, der auch I/O und Speicher senkt, setzt
der Renderer nicht. Er kostete einen Lauf mit einem Thread 9 bis 35 % und
schützte die Tickzeit eines Servers nicht besser als IDLE allein, siehe
[0080](../entscheidungen/0080-ohne-hintergrundmodus.md).

Was der Schalter nicht abschirmt, steht in [Was ein Lauf kostet](kosten.md),
„Neben einem Server“.

Ein Software-Adapter wie WARP oder lavapipe verteilt sich auf alle Kerne,
gleich wie viele Threads der Renderer hat. Mit `--threads` nimmt ihn
deshalb auch `--gpu on` nicht, siehe [Grafikkarte](grafikkarte.md),
„Adapter und Backends“. Der Treiber einer echten Karte startet eigene
Threads; sie erben die Priorität.
