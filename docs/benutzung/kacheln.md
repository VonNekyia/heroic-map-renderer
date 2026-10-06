---
title: Kacheln exportieren
description: Was ein Lauf mit --tiles tut, wie ein Ausschnitt gerundet wird, wie --area ein Rechteck der Welt wählt, wo Kacheln, Höhen und der Stand liegen, wann der Export Kacheln entfernt, auch mit --prune, dass er Dateien mit gleichen Bytes liegen lässt, und wie --progress json den Fortschritt meldet.
code:
  - renderer/src/cli.rs
  - renderer/src/world/mod.rs
  - renderer/src/world/region.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/render/heights.rs
---

# Kacheln exportieren

`--tiles` rendert die Welt als verlustfreie WebP-Kacheln von 256 mal 256
Pixeln, stapelt die gröberen Zoomstufen darüber und schreibt `map.json`.
`--tiles` nennt die Wurzel; jeder Baum liegt darunter in seinem Ordner,
siehe „Wo die Kacheln liegen“.
Ein Vorlauf liest dafür jeden Chunk einmal, dann rendern alle Threads die
Basis in Streifen. `--center` und `--size` schränken auf einen Ausschnitt
ein, der in einen bestehenden Baum passt; `--update` auf die Stellen, die
sich geändert haben, siehe [Updates](updates.md). Der Ablauf steht in `write_tiles` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs), der Vorlauf in `survey` in
[`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs). Was
ein Lauf kostet: [Was ein Lauf kostet](kosten.md).

## Die ganze Welt

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles
```

```
GPU:        <Name der Karte> (Vulkan)

Vorlauf:    249103 Chunks in 5.5 s, 3107 Blockstates, 280630 Kacheln
            67120 Chunks nicht fertig erzeugt, nicht gezeichnet
            4953 Sprites bei scale 32, davon 2437 Fassungen
            30 Modelle ragen über ihren Block hinaus, Würfel {[0, -1, 0], [0, 1, 0]}
Höhen:      383 Regionen, 1.7 MB in 0.2 s
            200/280630 Kacheln
            400/280630 Kacheln
```

Danach stapelt der Lauf die gröberen Zoomstufen darüber und schreibt
`map.json`.

Der Vorlauf beantwortet zwei Fragen auf einmal: welche Blockstates
vorkommen, und welche Kacheln überhaupt etwas zeigen. Er kostet für die
ganze Welt 5 bis 11 Sekunden. Chunks, die das Spiel nicht fertig erzeugt
hat, übergeht er und nennt ihre Zahl, siehe
[Welten und Kennung](welten.md), „Nicht fertig erzeugte Chunks“. Eine
Fassung ist jedes Sprite, das nicht
selbst Alternative einer Blockstate ist: eines je Maske verdeckter
Flüssigkeitsflächen, dazu die Streifen an Wasserstufen. Wie
Vorlauf und Renderlauf zusammenspielen und warum die Welt dafür mehrmals
durchlaufen wird, steht in
[Der Weg einer Kachel](../renderer/renderpfad.md).

## Ein Ausschnitt

`--center` und `--size` schränken auf einen Ausschnitt ein:

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles --center -64 416 --size 2048 --native-levels 3
```

```
GPU:        <Name der Karte> (Vulkan)

Vorlauf:    788 Chunks in 0.1 s, 247 Blockstates, 256 Kacheln
            570 Sprites bei scale 32, davon 323 Fassungen
            1 Modelle ragen über ihren Block hinaus, Würfel {[0, 1, 0]}
Höhen:      4 Regionen, 0.0 MB in 0.0 s
            200/256 Kacheln
            256/256 Kacheln
Kacheln:    256 geschrieben, 0 leer, 256x256 px, 24 Threads + GPU
            23.7 MB in 0.3 s (794 Kacheln/s, 95 kB je Kachel)
Zoom  9:     64 Kacheln nativ bei scale 16 + GPU, 6.0 MB
Zoom  8:     16 Kacheln nativ bei scale 8 + GPU, 1.4 MB
Zoom  7:     4 Kacheln nativ bei scale 4 + GPU, 0.4 MB
            7.7 MB in 0.6 s (138 Kacheln/s), in Bändern aus 1 Kachel bei scale 4
Zoom  6:     2 Kacheln
...
Pyramide:   9 Kacheln, 0.2 MB in 0.0 s
Karte:      Zoom 0..10, 256 Basiskacheln, -10240/0 bis -6144/4096 px -> ./tiles/2x1-se/map.json
```

MB und kB zählt die Ausgabe binär, 2^20 und 2^10 Byte, siehe
[Was ein Lauf kostet](kosten.md). Die Rate eines so kurzen Laufs sagt wenig;
die Basis braucht hier eine Drittelsekunde. Wie stark sie streut, steht in
[2026-09-27, Biomübergänge](../messungen/2026-09-27-biomuebergaenge.md),
„Die Beispielausgabe“. Die Ausgabe oben ist ein Lauf aus
[2026-10-01, Native Stufen in Bändern](../messungen/2026-10-01-native-stufen-in-baendern.md),
„Beispiel“; dort stand „aus 1 Kachel“ noch in der Mehrzahl. Die
nativen Stufen nennen ihre Zeit zusammen, in der Zeile unter ihnen, ab zwei
Stufen mit der Grösse der Bänder, siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Native Stufen in
Bändern“.

Der Ausschnitt wird aufgerundet, bevor der Vorlauf irgendetwas
ausschliesst, und zwar auf ganze Kacheln der gröbsten nativen Stufe
(`Gebiet` in [`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)): mit
drei Stufen bei scale 32 auf 2048 Pixel, aus 2048 mal 2048 werden hier 4096
mal 4096. Die nativen Stufen zeigen ganze Elternkacheln, und alle Stufen
sollen denselben Stand der Welt zeigen: sonst stünde ein Neubau neben dem
Ausschnitt nur auf den gröberen. Umgekehrt sammelt der Vorlauf Blockstates
nur aus Chunks, die tatsächlich in eine ausgegebene Kachel fallen, und was
die gerundete Fläche gar nicht berühren kann, dekodiert er nicht einmal:
hier 788 Chunks statt der 8192 aller Regionen, die sie schneiden. Ein
kleiner Ausschnitt braucht deshalb keine Assets für Blöcke am anderen Ende
der Welt; fehlt eines in seiner Fläche, bricht der Lauf ab, bevor er die
erste Kachel schreibt. Mit `--cinematic` liest er dazu die Chunks, durch
die ein Strahl zur Sonne läuft, siehe [Cinematic](../renderer/cinematic.md),
„Der Vorlauf“.

## Ein Rechteck der Welt: `--area`

`--area X0 Z0 X1 Z1` zeichnet nur ein Rechteck der Welt, etwa ohne
verirrte Chunks weit draussen. Die zwei Ecken sind Blöcke, beide gehören
dazu, in beliebiger Reihenfolge. Der Lauf rundet nach aussen auf ganze
Chunks, `--area 0 0 20 5` zeichnet die Chunks (0, 0) und (1, 0), x von 0
bis 31 und z von 0 bis 15:

- **Chunks ausserhalb** liest der Lauf nicht, weder im Vorlauf noch beim
  Zeichnen, noch für das Licht: Sie fehlen wie nie erzeugte, siehe
  [Welten und Kennung](welten.md), „Nicht fertig erzeugte Chunks“. Der
  Rand des Rechtecks sieht deshalb aus wie der Rand einer Welt.
- **Zoomstufen:** Die Nummerierung hängt am Rechteck statt an allen
  Regionen, siehe [Zoomstufen](zoomstufen.md).
- **Mit `--center` und `--size`** zeichnet der Lauf, was in beiden liegt.
- **Es gehört zum Baum** wie der Radius der Mischung, siehe
  [map.json](map-json.md), „Die Welt“: Ein Lauf ohne `--area` in einen
  Baum mit Rechteck nimmt es aus `map.json`, auch mit `--resume`. Ein
  anderes Rechteck bricht ab, bevor der Lauf einen Chunk liest, ebenso
  `--area` auf einem Baum ohne.
- **Die Weltgrenze** liest der Lauf aus `data/minecraft/world_border.dat`,
  gesucht wie der Seed, siehe [Welten und Kennung](welten.md), „Wo der
  Seed steht“. Ist sie kleiner als die Vorgabe des Spiels, 59 999 968
  Blöcke, nennt er ohne `--area` das Rechteck um sie als fertiges
  `--area …`. Er wendet sie nicht an.

`rechteck_gehoert_zum_baum` und `wasserspiegel_und_weltgrenze` in
`renderer/tests/cli.rs` prüfen das.

## Wo die Kacheln liegen

Unter der Wurzel von `--tiles` liegt jeder Baum in seinem eigenen Ordner,
daneben `trees.json` und die Höhen, die alle Bäume teilen. Wie die Ordner
heissen, auch mit `--cinematic`, steht in [map.json](map-json.md), „Liste
der Bäume“, die Höhen unter „Höhen“. In einem Baum liegen die Kacheln als
`<z>/<x>/<y>.webp`; x und y dürfen negativ sein, weil der Blockursprung
mitten in der Welt liegt. Neben `map.json` liegt der Stand für Updates,
`stand.bin`, siehe [Updates](updates.md), „Der Stand“, und das Manifest mit
Grösse und ETag jeder Kachel, `manifest`, siehe [Plugin](../plugin.md),
„Manifest“. Jede Kachel, jede
Datei der Höhen, `map.json`, der Stand, das Manifest und `trees.json` entstehen erst als
eigene Datei daneben und werden dann getauscht: Ein Leser sieht nie eine
halbe Datei, siehe
[0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md).

Eine Kachel muss Pixel für Pixel dem entsprechenden Ausschnitt eines
grossen Renderings gleichen, sonst stünden im Browser Kanten dazwischen.
Neun Kacheln nebeneinander, die Grenzen rot eingezeichnet:

![Kacheln](../bilder/kacheln.png)

## Leer gewordene Kacheln

Wird eine Kachel bei einem erneuten Lauf leer, löscht der Export die alte
Datei, auf jeder Stufe, sonst zeigte die Karte weiter, was inzwischen
abgerissen wurde. Nur eine native Elternkachel, unter der eine Kachel
stehen bleibt, bleibt durchsichtig stehen, siehe unten.

Das gilt für jede Kachel, die der Lauf zeichnet. Eine Kachel, in die kein
Block mehr reicht, etwa über einem abgerissenen Turm, zeichnet er nur, wenn
er weiss, dass ein Chunk dorthin reichte, der noch da ist: Ein Update weiss
es aus dem Stand, ein voller Lauf, wenn der Baum schon einen Stand hat. Er
zeichnet dann die vorhandenen Kacheln im Gebiet jedes geänderten Chunks,
der noch etwas zeichnet, wie ein Update, siehe [Updates](updates.md), „Wo
ein Update zeichnet“; leer verschwinden sie. Ohne Stand bleibt so eine
Kachel stehen wie eine ohne Chunk, siehe unten
(`voller_lauf_mit_stand_raeumt_abgerissenes_weg`). Ebenso bleibt eine
Kachel stehen, die im Gebiet eines gelöschten Chunks leer wird: Der Lauf
weiss nicht, ob sie auch ihn zeigte; `--prune` räumt sie weg.

## Kacheln ohne Chunk: `--prune`

Eine Basiskachel, in die kein Block mehr reicht und die der Lauf nicht
zeichnet, etwa weil ein Editor den Chunk zurückgesetzt hat, entfernt der
Export nur mit `--prune`, dann auf jeder Stufe. Im Gebiet eines Chunks, der
im Stand stand und jetzt fehlt, gilt das auch für ein Update.
Bis zum Ende der Pyramide läuft ein Lauf mit dem Schalter wie einer
ohne ihn; erst dann nimmt er diese Kacheln heraus und setzt die Stufen über
ihnen ohne sie neu zusammen. Ohne den Schalter zählt er sie und lässt sie
stehen; nur wo der Lauf eine native Elternkachel ohnehin neu rendert, fehlt
dort schon, was sie zeigen. Die Elternkachel bleibt dann durchsichtig
stehen, damit keine Kachel ohne Eltern dasteht. Einer Teilkopie der Welt
fehlt vieles, und ein Lauf mit `--prune` leerte über ihr den Baum: der
Schalter gehört nur an die vollständige Welt. Der Lauf nennt deshalb vor
der ersten Kachel, wie viele Kacheln es trifft, von wie vielen. Ein
Ausschnitt sucht nur in seiner gerundeten Fläche. Mit `--prune` läuft er
auch dann, wenn der Vorlauf dort gar nichts mehr findet, und auch, wenn
dort schon aufgeräumt ist.

Die Höhen einer Region, deren Regionsdatei fehlt, entfernt ebenfalls nur
`--prune`, am Ende des Laufs mit den Kacheln, soweit der Lauf die Region
läse. Die Höhen eines verschwundenen Chunks in einer Region, die es noch
gibt, schreibt dagegen jeder Lauf leer, der ihn liest, siehe
[map.json](map-json.md), „Höhen“.

## Wann entfernt wird

Entfernt wird erst am Ende des Laufs, auf allen Stufen, auch was nur leer
geworden ist, von der gröbsten Stufe bis zur Basis. Bis dahin zeigt eine
Kachel, die beim Rendern oder in der Pyramide leer geworden ist, schon
nichts mehr, der Lauf überschreibt sie durchsichtig. Bricht er vorher ab,
hat er nichts gelöscht, mit `--resume` nur die frischen Basiskacheln (siehe
[Pyramide und Fortsetzen](pyramide-und-resume.md)), und auch ein späterer
Ausschnitt holt nichts Abgerissenes in eine Elternkachel zurück.

Über den Kacheln ohne Chunk hat ein Lauf mit `--prune` bis zum Ende der
Pyramide nur verändert, was auch ein Lauf ohne ihn verändert hätte. Danach
setzt er die verkleinerten Stufen über ihnen ohne sie neu zusammen. Was
dabei leer wird, entfernt er erst am Ende, mit diesen Kacheln und ihren
nativen Vorfahren, unter denen nichts bleibt. Bricht er dazwischen ab,
zeigen die neu zusammengesetzten Kacheln schon den aufgeräumten Stand. Die
Basis, native Kacheln, die dieser Lauf nicht gerendert hat, und was leer
geworden ist, zeigen noch den alten. Ein Lauf mit `--prune` über dieselbe
Fläche bringt den Baum in Ordnung, einer ohne den Schalter nicht immer.

Bricht er beim Entfernen ab, fehlen feineren Kacheln die Eltern. Jeder Lauf
sucht solche Kacheln, soweit sie seine Fläche berühren, und baut ihnen die
Eltern neu, auch einer, dessen Vorlauf dort nichts mehr findet; einer mit
`--prune` räumt dann auch die Kacheln ohne Chunk weg.

## Gleiche Bytes bleiben liegen

Hat eine Datei schon dieselben Bytes, schreibt der Export sie nicht neu
(`lege_ab` in [`renderer/src/cli.rs`](../../renderer/src/cli.rs)). Sie
behält ihre Zeit und damit das ETag aus Grösse und Zeit, das ein Server
ausliefert (#151, #154): Ein Browser bekommt `304`, und ein Abgleich lädt
sie nicht noch einmal. Ohne das bekäme nach einem vollen Lauf über einen
bestehenden Baum, etwa mit einem neuen Binär, jede Kachel ein neues ETag.

- **Wie:** erst die Grösse aus den Metadaten, nur bei gleicher Grösse die
  Bytes. Ändert sich viel, reicht meist die Grösse.
- **Kosten:** keine. Ein voller Lauf über einen bestehenden Baum der
  Testwelt, in dem sich nichts geändert hat, brauchte in der Basis 2 bis 3 %
  weniger als mit Neuschreiben, siehe
  [Gleiche Bytes liegen lassen](../messungen/2026-10-05-gleiche-bytes.md).
- **Die Ausgabe** nennt, was liegen blieb:
  `Kacheln:    0 geschrieben, 17820 gleich geblieben, 344 leer, …`.
- **Was:** Kacheln jeder Stufe, die Basis, die nativen Stufen und die
  Pyramide, dazu die Höhen. `map.json`, `stand.bin` und `trees.json`
  schreibt der Renderer immer neu.
- **Mit vorgegebener Zeit,** wie `--pyramid` seine Kacheln stempelt
  ([0017](../entscheidungen/0017-pyramide-vergleicht-zeiten.md)), bekommt
  eine gleiche Datei nur diese Zeit, ihre Bytes bleiben.
- **`--pyramid` danach:** Hat ein Kind neue Bytes, die Elternkachel aber
  trotzdem dieselben, bleibt sie mit ihrer alten Zeit liegen und ist älter
  als ihr Kind. Der nächste `--pyramid` baut sie einmal nach und stempelt
  sie; danach nicht wieder, siehe
  [Pyramide und Fortsetzen](pyramide-und-resume.md), „Was neu gebaut wird“.
- **`--resume`** erkennt eine liegen gelassene Basiskachel nicht an ihrer
  Zeit. Ein Lauf mit angefangenem Stand trägt sie deshalb in ein Protokoll
  ein, siehe [Pyramide und Fortsetzen](pyramide-und-resume.md),
  „Fortsetzen: `--resume`“.
- Getestet: `voller_lauf_laesst_gleiche_dateien_liegen` in
  `renderer/tests/cli.rs`. Ein voller Lauf über einen bestehenden Baum, in
  dem sich ein Chunk geändert hat: Jede Datei mit gleichen Bytes behält ihre
  Zeit, jede andere bekommt eine neue, und am Ende stehen dieselben Bytes da
  wie nach einem Lauf in einen leeren Baum.

## Fortschritt als JSON: `--progress`

Mit `--progress json` meldet ein Export seinen Fortschritt als JSON-Zeilen
auf stdout, für Programme wie das Plugin, auch im Vorlauf, für jede
native Stufe und die Pyramide. Die Zeilen `n/N Kacheln` fallen dann weg, alle übrigen bleiben
Text. Mit Text melden die nativen Stufen erst, wenn sie fertig sind. Welche
Zeilen kommen und was in ihnen steht:
[Plugin](../plugin.md), „Fortschritt als JSON“.

## Weitere Schalter beim Export

- `--native-levels`: [Zoomstufen](zoomstufen.md)
- `--resume`: [Pyramide und Fortsetzen](pyramide-und-resume.md)
- `--update`: [Updates](updates.md)
- `--gpu`: [Grafikkarte](grafikkarte.md)
- `--defender-exclusion`: [Echtzeitschutz](echtzeitschutz.md)

## Was bleibt eine Näherung

- **Ohne `--prune` bleiben Kacheln verschwundener Chunks stehen.** Rendert
  der Lauf eine native Elternkachel ohnehin neu, zeigt sie die Welt ohne
  den Chunk, die Basis darunter noch mit ihm; rendert sie leer, bleibt sie
  durchsichtig stehen. Die Ausgabe nennt den Schalter.
- **Ohne Stand bleiben Kacheln stehen, in die ein Chunk nicht mehr reicht,**
  etwa über einem abgerissenen Turm, bis `--prune`. Jeder volle Lauf über
  die ganze Welt schreibt den Stand, danach zeichnet jeder Lauf sie.
