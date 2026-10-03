---
title: Updates
description: Wie --update nur zeichnet, wo sich die Welt seit dem letzten vollen Lauf geändert hat, mit dem Stand je Baum, Zeitstempeln und Fingerabdrücken je Chunk, dem Gebiet einer Änderung, Abbruch und --resume, und wann ein voller Lauf nötig ist.
code:
  - renderer/src/cli.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
  - renderer/src/world/region.rs
  - renderer/src/world/chunk.rs
---

# Updates

`--update` zeichnet nur die Kacheln neu, deren Welt sich seit dem letzten
vollen Lauf oder Update des Baums geändert hat. Dazu legt jeder volle Lauf
über die ganze Welt den Stand des Baums ab: je Chunk den Zeitstempel aus dem
Kopf seiner Regionsdatei und einen Fingerabdruck dessen, was der Renderer
aus ihm zeichnet. Ein Update vergleicht den Stand mit der Welt, liest nur
Chunks mit neuem Stempel und zeichnet um jeden geänderten Chunk ein Gebiet
neu. Das Ergebnis gleicht Byte für Byte einem vollen Lauf über dieselbe
Welt (`update_gleicht_einem_vollen_lauf` in
[`renderer/tests/cli.rs`](../../renderer/tests/cli.rs)). Entschieden in
[0061](../entscheidungen/0061-updates-nach-stempel-und-fingerabdruck.md).

## Ein Update

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles ./tiles --update
```

Vor dem Vorlauf sagt der Lauf, was er gefunden hat:

```
Update:     2 Chunks geändert, 6 Kacheln von 256x256 px bei scale 4 im Gebiet, in 0.0 s
```

Die Kacheln im Gebiet liegen auf der gröbsten nativen Stufe, hier bei drei
nativen Stufen und scale 32. Danach läuft der Export wie ein Ausschnitt über
dieses Gebiet: Vorlauf, Basis, native Stufen, Pyramide, Entfernen,
`map.json`. Am Ende steht der neue Stand:

```
Stand:      4 Chunks -> ./tiles/2x1-se/stand.bin
```

Hat sich nichts geändert, schreibt der Lauf nur den Stand mit den neuen
Stempeln und sagt `Update:     nichts zu zeichnen`.

- **Gleiche Einstellungen:** Kamera, Richtung, scale, native Stufen, Radius
  der Mischung und `look` kommen aus dem Baum wie bei jedem Lauf in einen
  bestehenden Baum, siehe [map.json](map-json.md).
- **Nicht mit `--size`:** Ein Update hat sein Gebiet aus den Änderungen.
- **Mit `--prune`:** Wie ein Ausschnitt im Gebiet des Updates, siehe
  [Kacheln exportieren](kacheln.md), „Kacheln ohne Chunk: `--prune`“.

## Der Stand

Jeder Baum hat seinen Stand in `stand.bin` neben `map.json`. Ihn schreibt
nur ein voller Lauf über die ganze Welt oder ein Update, und zwar ganz am
Ende, nach `map.json`; getauscht wie jede Datei, siehe
[0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md).
Ein Ausschnitt schreibt keinen. Bricht ein Lauf vorher ab, gilt der alte
Stand, und das nächste Update zeichnet dieselben Stellen noch einmal.

Der Stand enthält die Stempel, wie sie zu Beginn des Laufs waren, vor dem
Vorlauf gelesen. Schreibt der Server einen Chunk während des Laufs, liest
der Lauf am Ende die Köpfe noch einmal: Jeder Chunk mit anderem Stempel
steht als unbekannt im Stand und gilt beim nächsten Update als geändert,
über die volle Höhe.

Vor der ersten Kachel legt der Lauf den angefangenen Stand als
`stand-neu.bin` ab, wie `map.json`. Ihn braucht nur `--resume`, siehe unten;
am Ende fällt er weg.

| Teil | Bytes |
|---|---|
| Kopf: `HMRSTAND`, Fassung 1, Art (voller Lauf oder Update), Fingerabdruck des Renderers und der Assets, Zahl der Regionen | 33 |
| je Region: x, z | 8 |
| je Chunk, 1024 je Region: Art des Inhalts mit einem Bit für den Stempel, Zeit, Eintrag der Tabelle, Fingerabdruck, höchster Block | 19 |

Eine Region kostet 19 464 Byte, eine Welt mit 2,5 Millionen Chunks also
mindestens rund 47 MB je Baum. Alles in Little Endian, `Stand::als_bytes` in
[`renderer/src/render/stand.rs`](../../renderer/src/render/stand.rs).

## Was als geändert gilt

Zwei Stufen, damit ein Update nicht jeden Chunk dekodiert:

1. **Der Stempel** aus dem Kopf der Regionsdatei, `Region::stempel` in
   [`renderer/src/world/region.rs`](../../renderer/src/world/region.rs): die
   Zeit in Sekunden und der Eintrag der Tabelle mit Sektor und Länge. Der
   Lauf liest nur die zwei Sektoren des Kopfs je Region. Gleicht der Stempel
   dem im Stand, gilt der Chunk als gleich, ohne dass er gelesen wird.
   Verglichen wird auf Gleichheit, nicht auf später; die Uhren von Server und
   Renderer spielen so keine Rolle, und eine kopierte Welt behält ihre
   Stempel.
2. **Der Fingerabdruck,** `Chunk::abdruck` in
   [`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs), nur
   für Chunks mit neuem Stempel: FNV-1a mit 64 Bit über die Blöcke und Biome
   jeder Section, als Läufe gleicher Werte der Reihe nach, und über die
   Daten der Blockentities, die das Bild ändern. Die Ordnung der Palette und
   die Namen ihrer Felder zählen nicht: Ein Chunk, den der Server nur von
   26.2 auf 26.3 umschreibt, bleibt gleich. Dazu kommt das y seines höchsten
   Blocks, der nicht Luft ist.

Ein Chunk, der nicht fertig erzeugt ist, zeichnet der Renderer nicht, siehe
[Welten und Kennung](welten.md), „Nicht fertig erzeugte Chunks“. Zwischen
ihm und keinem Chunk ändert sich darum nichts.

Belegt per javap an `RegionFile` aus dem Client 26.2 und aus Paper 26.3:

- **`write`** legt den Chunk in neue Sektoren, setzt danach den Eintrag der
  Tabelle und die Zeit (`getTimestamp`: `Util.getEpochMillis` durch 1000)
  und schreibt dann den Kopf. Wer den Kopf liest, findet zu einem neuen
  Eintrag also schon die neuen Daten.
- **`clear`** setzt den Eintrag auf 0 und die Zeit auf jetzt.
- **Paper** setzt beim Reparieren einer kaputten Regionsdatei
  (`recalculateHeader`) die Zeit jedes Chunks darin auf jetzt; das Update
  liest die Region dann ganz und vergleicht die Fingerabdrücke.

Ein Stempel heisst „geschrieben“, nicht „geändert“: Vanilla und Paper
schreiben jeden Chunk neu, den ein Spieler geladen hat, auch ohne Änderung.
Das steht an #100 mit den Belegen. Deshalb der Fingerabdruck.

## Wo ein Update zeichnet

Um jeden geänderten Chunk ein Gebiet, `gebiet_der_aenderungen` in
[`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs):

- **Daneben:** 16 Blöcke rundum, also ein Chunk. So weit reichen Licht (15)
  und weiche Beleuchtung (1), die Mischung der Biome (höchstens 7) liegt
  darin.
- **In der Höhe:** von der Unterkante der Dimension bis 16 Blöcke über dem
  höchsten Block der 3 × 3 Chunks um ihn: für ihn selbst der höhere aus
  altem und neuem Stand, für die Nachbarn der neue; über einen Chunk, von
  dem der Stand nichts weiss, die Oberkante. Nach unten bleibt es die volle
  Höhe, denn Himmelslicht fällt in einer Spalte beliebig tief. Nach oben
  reicht der höhere der beiden Stände, sonst bliebe die Spitze eines
  abgerissenen Turms stehen, und die Nachbarn zählen mit: Wird ein Chunk
  fertig, fällt er weg oder wechselt er sein Biom, ändern sich an einem
  Nachbarn Licht und Farbe bis zu dessen höchstem Block (`mit_nachbarn` in
  `renderer/src/render/stand.rs`).
- **Mit Cinematic** dazu jeder Chunk, dessen Strahlen zur Sonne den
  geänderten lesen, wie im Vorlauf umgekehrt, siehe
  [Cinematic](../renderer/cinematic.md), „Der Vorlauf“, und der Rand des
  Bloom auf jeder nativen Stufe.
- Projiziert samt der Reserve für überstehende Sprites, dann aufgerundet auf
  Kacheln der gröbsten nativen Stufe wie bei einem Ausschnitt, siehe
  [Kacheln exportieren](kacheln.md), „Ein Ausschnitt“.

Die Ränder prüft `gebiet_reicht_16_bloecke_um_die_aenderung` in
`tiles.rs` an jeder Ecke jedes Blocks bis 16 daneben und darüber.
`update_reicht_so_weit_wie_die_aenderung` zeichnet je eine Änderung allein
in einem Chunk: Licht im Chunk daneben, Schatten im Himmelslicht weit
darunter, Licht über dem höchsten Block, einen Chunk, der fertig wird,
und einen, der nur sein Biom wechselt, je neben einem hohen Nachbarn, und
mit Cinematic den Schatten eines Turms bis vier Chunks weit, aus `se`,
`nw` und `north-45`.

Alle Stücke zusammen sind das Gebiet des Laufs, eine Menge solcher Kacheln
(`Gebiet` in `tiles.rs`). Der Vorlauf liest nur die Chunks, deren Spalte es
berührt. Kacheln, in die ein Chunk früher reichte, der noch da ist, zeichnet
der Lauf neu; leer verschwinden sie. Kacheln eines Chunks, der ganz fehlt,
bleiben ohne `--prune` stehen, siehe [Kacheln exportieren](kacheln.md),
„Leer gewordene Kacheln“ (`update_laesst_kacheln_fehlender_chunks_stehen`).

## Abbruch und `--resume`

Bricht ein Update ab, bleibt der Stand der alte. Ein neues Update findet
dieselben Änderungen wieder, dazu neuere, und zeichnet sie noch einmal
(`abgebrochenes_update_laesst_den_stand_stehen`). Wer zusieht, sieht während
des Laufs alte neben neuen Kacheln, jede ganz, wie bei einem vollen Lauf.

`--update --resume` setzt ein abgebrochenes Update fort. Es behält nur
Basiskacheln, die jünger sind als dessen angefangener Stand, ohne die
frischen der letzten zwei Minuten wie
[0019](../entscheidungen/0019-resume-behaelt-die-basiskacheln.md); ältere im
Gebiet zeichnet es neu (`update_setzt_mit_resume_fort`). Am Ende gilt der
angefangene Stand, mit allem als unbekannt, was der Server seitdem schrieb.

Ein voller Lauf mit `--resume` nimmt ebenso den angefangenen Stand des
abgebrochenen Laufs, siehe [Pyramide und Fortsetzen](pyramide-und-resume.md).
Fehlt er oder stammt er von einem Lauf der anderen Art, schreibt der Lauf
keinen Stand und sagt es; `--update` braucht dann einen vollen Lauf.

## Anderer Renderer, andere Assets

Ein Update mischte sonst alte und neue Kacheln. Der Stand trägt deshalb zwei
Fingerabdrücke, und `--update` bricht vor der ersten Kachel ab, wenn einer
nicht passt (`update_braucht_den_stand_und_dieselben_assets`):

- **Der Renderer:** FNV-1a über seine ausführbare Datei. Jeder neue Build
  zählt als anders, auch einer, der dasselbe Bild zeichnet.
- **Assets und Daten:** je Wurzel von `--assets` und `--data` ihre Nummer,
  dann je Datei darunter der Pfad, die Grösse und die Zeit der letzten
  Änderung. Den Inhalt liest er nicht; eine kopierte Datei hat eine neue
  Zeit und zählt als anders.

Danach zeichnet ein voller Lauf alles neu, und `--update` geht wieder.

## Was ein Update nicht bemerkt

- **Ein Werkzeug, das einen Chunk ohne neue Zeit an derselben Stelle
  überschreibt.** Das Spiel tut das nicht, siehe oben.
- **Ein Stand aus einem anderen Baum,** von Hand kopiert.

## Kosten

Noch nicht gemessen.
