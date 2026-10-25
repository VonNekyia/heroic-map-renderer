---
title: Updates
description: Wie --update nur zeichnet, wo sich die Welt seit dem letzten vollen Lauf geändert hat, mit dem Stand je Baum, Zeitstempeln und Fingerabdrücken je Chunk, dem offenen Gebiet abgebrochener Läufe und Ausschnitte, dem Gebiet einer Änderung, Abbruch und --resume, und wann ein voller Lauf nötig ist.
code:
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
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
[0062](../entscheidungen/0062-updates-nach-stempel-und-fingerabdruck.md).

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
Stempeln und sagt `Update:     nichts zu zeichnen`. Ausgenommen ist ein
Baum aus `top-north`, dessen Pyramide noch mittelt: Dann baut das Update
sie ganz neu und sagt es mit `Verkleinern:`, siehe
[map.json](map-json.md), „Verkleinern“.

- **Gleiche Einstellungen:** Kamera, Richtung, scale, native Stufen, Radius
  der Mischung und `look` kommen aus dem Baum wie bei jedem Lauf in einen
  bestehenden Baum, siehe [map.json](map-json.md). Hat die Welt seitdem
  die Version gewechselt, bricht das Update ab wie jeder Lauf, siehe
  [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Welten aus
  26.2“.
- **Nicht mit `--size`:** Ein Update hat sein Gebiet aus den Änderungen.
- **Mit `--prune`:** Wie ein Ausschnitt im Gebiet des Updates, siehe
  [Kacheln exportieren](kacheln.md), „Kacheln ohne Chunk: `--prune`“.

## Der Stand

Jeder Baum hat seinen Stand neben `map.json`: den Kopf `stand.bin` und je
Region eine Datei in `stand/`, Fassung 2 aus
[0126](../entscheidungen/0126-stand-je-region.md). Ihn schreibt nur ein
voller Lauf über die ganze Welt oder ein Update, und zwar ganz am Ende,
nach `map.json`. Ein Ausschnitt schreibt keinen.

- **Nur, was anders ist:** Ein Lauf schreibt die Regionen, deren Einträge
  anders sind als auf der Platte, jede getauscht wie in
  [0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md),
  danach den Kopf. Zuletzt fallen die Regionen weg, die der Kopf nicht mehr
  nennt (`lege_stand_ab` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs)).
- **Abbruch dazwischen:** Dann gilt der alte Kopf, und einige Regionen
  tragen schon den neuen Stand. Der Stand entsteht erst nach den Kacheln,
  eine neue Region nennt also nie mehr, als die Kacheln zeigen. Das nächste
  Update zeichnet höchstens mehr und gleicht danach einem vollen Lauf, auch
  mit `--resume` (`stand_je_region_nach_abbruch`).
- **Ohne Änderung:** Findet ein Update dieselben Stempel und Inhalte wie im
  alten Stand, schreibt es ihn nicht neu: `Stand:      unverändert ->
  …/stand.bin`. Der alte gilt weiter, mit Bytes und Zeit. Was ein
  abgebrochener Lauf hinterliess, `stand-neu.bin` samt `stand-neu/`,
  `stand-neu-liegen.bin` und `gebiet-offen.bin`, räumt es trotzdem weg
  (`entferne_angefangene`). In Fassung 1 sparte das an der Testwelt 7,5 MB
  und 0,08 s je Update.
- **Neue Stempel ohne neuen Inhalt** sind keine Änderung zum Zeichnen,
  aber eine am Stand: Deren Regionen schreibt das Update dann, sonst läse
  das nächste dieselben Chunks wieder.
- **Fassung 1,** alles in `stand.bin`, liest der Renderer weiter. Sie gilt
  in jeder Region als geändert: Der erste fertige Lauf schreibt alle in
  Fassung 2, auch ein Update ohne Änderung
  (`stand_der_fassung_1_gilt_weiter`). Ein älterer Renderer liest Fassung 2
  nicht; wer zurückgeht, braucht einen vollen Lauf.
- **Geprüft** in `update_ohne_aenderung_schreibt_den_stand_nicht` in
  `renderer/tests/cli.rs`.

Kacheln ohne neuen Stand zeichnen ein Ausschnitt, ein Lauf, der abbricht,
und einer mit `--resume`, der keinen Stand schreibt. Damit das nächste
Update keine davon übersieht, hält jeder Lauf vor der ersten Kachel fest,
was er zeichnen kann:

- **Ein Update und ein Ausschnitt** legen ihr Gebiet ab, siehe „Das offene
  Gebiet“. Der Stand bleibt dabei, wie er ist.
- **Ein voller Lauf** liest ohnehin alles. Er macht im alten Stand jeden
  Chunk unbekannt, den der Stand kennt oder der jetzt in der Welt steht
  (`Stand::unbekannt_wo`). Bricht er ab, zeichnet das nächste Update also
  alles.

Der Stand enthält die Stempel, wie sie zu Beginn des Laufs waren, vor dem
Vorlauf gelesen. Schreibt der Server einen Chunk während des Laufs, liest
der Lauf am Ende die Köpfe noch einmal: Jeder Chunk mit anderem Stempel
steht als unbekannt im Stand und gilt beim nächsten Update als geändert,
über die volle Höhe.

Vor der ersten Kachel legt der Lauf den angefangenen Stand ab: den Kopf
`stand-neu.bin` und in `stand-neu/` nur die Regionen, die anders sind als
in `stand.bin`; die anderen liest `--resume` aus `stand/`. Einen älteren
angefangenen Stand räumt er vorher weg. Daneben liegt das Protokoll der
Kacheln, die er liegen lässt, siehe
[Pyramide und Fortsetzen](pyramide-und-resume.md), „Fortsetzen:
`--resume`“. Das alles braucht nur `--resume`, siehe unten; am Ende fällt
es weg.

Der Kopf, `stand.bin` und `stand-neu.bin`:

| Teil | Bytes |
|---|---|
| `HMRSTAND`, Fassung 2, Art (voller Lauf oder Update), Fingerabdruck des Renderers und der Assets, Zahl der Regionen | 33 |
| je Region: x, z | 8 |

Eine Region, `stand/<x>.<z>.bin`:

| Teil | Bytes |
|---|---|
| `HMRREGIO`, Fassung 2 | 12 |
| je Chunk, 1024, nach z, dann x: Art des Inhalts mit einem Bit für den Stempel, Zeit, Eintrag der Tabelle, Fingerabdruck, höchster Block | 19 |
| dazu je Chunk für die Grenze nach unten K als i8, ohne Tiefe `0x80`, und H als u64, ohne 0; siehe „Die Grenze nach unten“ | 9 |

- **Grösse:** Eine Region kostet 28 684 Byte, eine Welt mit 2,5 Millionen
  Chunks also mindestens rund 70 MB je Baum. Im Speicher hält der Stand je
  Chunk die 28 Bytes.
- **Einmal im Speicher:** Ein Update hält den alten und den neuen Stand
  nicht nebeneinander; `Stand::vergleiche` ersetzt den alten Region für
  Region. Ein voller Lauf liest den alten Stand für die Änderungen und vor
  der ersten Kachel noch einmal, statt ihn dazwischen zu halten. Ein Update
  und ein Ausschnitt prüfen vor der ersten Kachel nur, ob es `stand.bin`
  gibt.
- **Fassung 1** hatte alles in `stand.bin`: hinter jeder Lage einer Region
  ihre Chunks zu 19 Bytes, 47 MB für dieselbe Welt.
- **Was das Plugin liest,** steht in [Plugin](../plugin.md), „Was das
  Plugin vom Renderer nutzt“.
- Alles in Little Endian, `Stand::kopf_bytes` und `Stand::region_bytes` in
  [`renderer/src/render/stand.rs`](../../renderer/src/render/stand.rs).

## Das offene Gebiet

Ein Update und ein Ausschnitt legen vor der ersten Kachel ihr Gebiet als
`gebiet-offen.bin` neben `stand.bin` ab: die Kacheln der gröbsten nativen
Stufe, die sie zeichnen können, vereinigt mit denen, die schon darin
stehen. Entschieden in [0112](../entscheidungen/0112-abgebrochene-laeufe-legen-ihr-gebiet-ab.md), #314.

- **Das nächste Update** zeichnet diese Kacheln noch einmal, dazu das Gebiet
  seiner Änderungen. Die Zeile `Update:` nennt sie:

  ```
  Update:     2 Chunks geändert, dazu 6 Kacheln aus abgebrochenen Läufen und Ausschnitten, 6 Kacheln von 256x256 px bei scale 4 im Gebiet, in 0.0 s
  ```

- **Ein Chunk, der zurückwechselt,** wie ein Ofen, der wieder ausgeht,
  gleicht wieder dem Stand. Die Kacheln, die ein abgebrochener Lauf mit dem
  brennenden Ofen zeichnete, zeichnet das Update trotzdem neu, weil sie
  offen liegen (`zurueckgewechselter_chunk_wird_gezeichnet`, nach einem
  abgebrochenen Update und nach einem Ausschnitt). Eine Kachel darin, in die
  kein Block mehr reicht, zeichnet es wie eine über einem abgerissenen Turm
  und entfernt sie leer, siehe [Kacheln exportieren](kacheln.md), „Leer
  gewordene Kacheln“.
- **Es wächst nicht:** Bricht auch das nächste Update ab, liegen danach
  dieselben Kacheln offen und die seiner neuen Änderungen, nicht mehr.
  Bis v0.13.0 machte jeder Lauf stattdessen im alten Stand jeden Chunk
  unbekannt, den er las, samt Strahlen zur Sonne und Nachbarn. Das nächste
  Update las um jeden davon wieder so viel. Mit Cinematic wuchs es so mit
  jedem Abbruch um ein Vielfaches
  (`abbrueche_blaehen_das_naechste_update_nicht_auf`).
- **Am Ende** eines vollen Laufs und eines Updates fällt die Datei weg,
  zusammen mit `stand-neu.bin`.
- **Lässt sich die Datei nicht lesen** oder gilt sie für eine andere Zahl
  nativer Stufen, bricht `--update` ab und verlangt einen vollen Lauf.
- **Aus älteren Versionen** stehen in `stand.bin` vielleicht noch viele
  Chunks als unbekannt. Das erste Update, das fertig wird, räumt sie weg.

| Teil | Bytes |
|---|---|
| Kopf: `HMROFFEN`, Fassung 1 | 12 |
| die Zahl der nativen Stufen über der Basis, die Zahl der Kacheln, je als u32 | 8 |
| je Kachel x und y als i32 | 8 |

Alles in Little Endian, `lege_offen_ab` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs).

## Was als geändert gilt

Zwei Stufen, damit ein Update nicht jeden Chunk dekodiert:

1. **Der Stempel** aus dem Kopf der Regionsdatei, `Region::stempel` in
   [`renderer/src/world/region.rs`](../../renderer/src/world/region.rs): die
   Zeit in Sekunden und der Eintrag der Tabelle mit Sektor und Länge. Der
   Lauf liest nur die zwei Sektoren des Kopfs je Region; ist die Datei
   kürzer, etwa weil der Server sie eben anlegt, gilt der Rest als leer.
   Gleicht der Stempel
   dem im Stand, gilt der Chunk als gleich, ohne dass er gelesen wird.
   Verglichen wird auf Gleichheit, nicht auf später; die Uhren von Server und
   Renderer spielen so keine Rolle, und eine kopierte Welt behält ihre
   Stempel.
2. **Der Fingerabdruck,** `Chunk::abdruck` in
   [`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs), nur
   für Chunks mit neuem Stempel: FNV-1a mit 64 Bit über die Blöcke und Biome
   jeder Section, als Läufe gleicher Werte der Reihe nach, über die Daten
   der Blockentities, die das Bild ändern, und über die eigenen
   Laubfarben, siehe [Eigene Laubfarben](laubfarben.md), „Update“. Die
   Ordnung der Palette und
   die Namen ihrer Felder zählen nicht: Ein Chunk, den der Server nur von
   26.2 auf 26.3 umschreibt, bleibt gleich. Dazu kommt das y seines höchsten
   Blocks, der nicht Luft ist.

Ein Chunk, der nicht fertig erzeugt ist, zeichnet der Renderer nicht, siehe
[Welten und Kennung](welten.md), „Nicht fertig erzeugte Chunks“. Zwischen
ihm und keinem Chunk ändert sich darum nichts.

Belegt per javap an `RegionFile` aus dem Client 26.2 und aus Paper 26.3:

- **Der Konstruktor** liest den Kopf in einen Puffer von 8192 Byte voller
  Nullen (`ByteBuffer.allocateDirect`). Ist die Datei kürzer, warnt er
  „Region file {} has truncated header“ und nimmt den Rest als leer.
  `Region::stempel` tut dasselbe.
- **`write`** legt einen Chunk in der Regionsdatei in neue Sektoren, setzt
  danach den Eintrag der Tabelle und die Zeit (`getTimestamp`:
  `Util.getEpochMillis` durch 1000) und schreibt dann den Kopf. Wer den Kopf
  liest, findet zu einem neuen Eintrag also schon die neuen Daten.
- **Ausgelagerte Chunks,** ab 256 Sektoren: `write` schreibt die Daten
  zuerst in eine temporäre Datei, dann einen Stummel in die Regionsdatei
  und den Kopf. Erst danach verschiebt der `CommitOp` die temporäre Datei
  nach `c.<x>.<z>.mcc` (`Files.move`). Zwischen Kopf und Verschieben steht
  der neue Stempel also neben dem alten Inhalt, siehe „Was ein Update nicht
  bemerkt“.
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
  Höhe, denn Himmelslicht fällt in einer Spalte beliebig tief, ausser auf
  der Karte unter dem tiefsten ersten dichten Block, siehe „Die Grenze nach
  unten“. Nach oben
  reicht der höhere der beiden Stände, sonst bliebe die Spitze eines
  abgerissenen Turms stehen, und die Nachbarn zählen mit: Wird ein Chunk
  fertig, fällt er weg oder wechselt er sein Biom, ändern sich an einem
  Nachbarn Licht und Farbe bis zu dessen höchstem Block (`mit_nachbarn` in
  `renderer/src/render/stand.rs`).
- **Mit Cinematic** dazu jeder Chunk, dessen Strahlen zur Sonne den
  geänderten lesen, wie im Vorlauf umgekehrt, siehe
  [Cinematic](../renderer/cinematic.md), „Der Vorlauf“, und der Rand des
  Bloom auf jeder nativen Stufe.
- **Mit `--relief`** dazu die Chunks bis 2 nach Südosten: So weit fällt
  der Schatten einer Spalte, siehe [Relief von oben](../renderer/relief.md),
  „Schatten“. Das gilt auch für `slope`, dessen Gefälle nur einen Block
  braucht, denn die Schicht trägt den Schatten, siehe ebenda, „Die
  Schicht“.
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
`nw` und `north-45`. Mit Relief prüfen
`gebiet_mit_schatten_reicht_32_bloecke_nach_suedosten` das Gebiet und
`relief_update_reicht_so_weit_wie_der_schatten` in
`renderer/tests/cli.rs` ein Update gegen einen vollen Lauf, mit dem
Schatten eines Turms jenseits der 16 Blöcke, mit `slope` und mit
`shadows`.

Alle Stücke zusammen sind das Gebiet des Laufs, eine Menge solcher Kacheln
(`Gebiet` in `tiles.rs`). Der Vorlauf liest nur die Chunks, deren Spalte es
berührt. Kacheln, in die ein Chunk früher reichte, der noch da ist, zeichnet
der Lauf neu; leer verschwinden sie. Kacheln eines Chunks, der ganz fehlt,
bleiben ohne `--prune` stehen, siehe [Kacheln exportieren](kacheln.md),
„Leer gewordene Kacheln“ (`update_laesst_kacheln_fehlender_chunks_stehen`).

## Die Grenze nach unten

Auf der Karte ohne Cinematic und ohne Relief beginnt das Gebiet eines
geänderten Chunks erst bei `min(16·K_alt, E_neu) − 16`, wenn seine
Sections unter K gleich geblieben sind. Entschieden in
[0127](../entscheidungen/0127-grenze-nach-unten.md), dort auch, warum es
reicht.

- **E** ist das tiefste y, an dem in einer Spalte des Chunks von oben der
  erste dichte Block steht, einer, der das Licht ganz aufhält
  (`Chunk::erster_dichter` in
  [`renderer/src/world/chunk.rs`](../../renderer/src/world/chunk.rs)). Hat
  eine Spalte keinen, gibt es kein E und keine Grenze.
- **K** ist die Section von E, **H** der Abdruck der Sections unter K
  (`Chunk::abdruck_unter`). Beide stehen im Stand, siehe „Der Stand“.
- **Die Bedingung** prüft `grenze_nach_unten` in `stand.rs`: Der alte Stand
  kennt K und H, der Chunk hat jetzt ein E, und seine Sections unter dem
  alten K haben wieder H. Ein Update liest dafür jeden Chunk mit dem alten
  K.
- **Ohne Tiefe** bleibt die volle Höhe: für einen Chunk, den der Server
  während des Laufs schrieb, und für einen aus einem Stand der Fassung 1,
  bis ein Lauf ihn wieder liest.
- **Wie viel:** An der Testwelt hat ein Update mit 16 Änderungen 1144
  statt 1968 Kacheln im Gebiet und braucht 10,4 statt 13,4 s; es schreibt
  dieselben 13 Kacheln. Der Vorlauf des vollen Laufs braucht 3,1 s mehr.
  Gemessen in
  [2026-10-24, Grenze nach unten](../messungen/2026-10-24-grenze-nach-unten.md).
- **Geprüft** in `grenze_nach_unten_gleicht_einem_vollen_lauf` in
  `renderer/tests/cli.rs`: ein Berg aus Stein mit einem Schacht in die Halle
  des Chunks daneben, der Reihe nach ein Block oben, Glas, Wasser und Laub
  über E und weg, Leuchtstein unter K, der Schacht auf und zu und eine
  Spalte ohne dichten Block; nach jedem Schritt gleicht das Update einem
  vollen Lauf.

## Die Liste der Basis

Ein Update liest die Basiskacheln nur in den Spalten seines Gebiets, nicht
die ganze Basis. Die Grenzen in `map.json` führt es aus der alten fort,
erweitert um die Kacheln, die es zeichnet.

- **Schrumpfen:** Entfernt es eine Kachel auf dem alten Rand, können die
  Grenzen kleiner werden. Dann liest es nach dem Lauf doch die ganze Basis.
  So gleicht `map.json` Byte für Byte einem vollen Lauf
  (`update_liest_die_basis_nur_im_gebiet` in `renderer/tests/cli.rs`).
- **Ganz liest** ein Update die Basis ausserdem mit `--resume`, beim
  Umstellen, ohne Grenzen in der alten `map.json` und nach einem
  abgebrochenen Lauf, dessen Gebiet offen liegt; ein voller Lauf und ein
  Ausschnitt immer.
- **Die Zahl:** Kennt das Update nur sein Gebiet, nennt die Zeile `Karte:`
  die Basiskacheln darin, `… Basiskacheln im Gebiet, …`, ebenso `tiles` in
  der Zeile `done` von `--progress json`.
  Das Plugin liest beides nicht, siehe [Plugin](../plugin.md), „Fortschritt
  als JSON“.
- **Ausserhalb des Gebiets** weiss das Update nicht, welche Basiskacheln
  dastehen. Eine Elternkachel, deren Kind dort liegt, baut es darum von der
  Platte statt im Speicher (`lag_da` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs)).
- **Kosten:** Die ganze Liste kostete an der Testwelt 0,21–0,24 s je
  Update, siehe
  [2026-10-24, Updates je Phase](../messungen/2026-10-24-updates-je-phase.md).

## Gleiche Pixel

Die meisten Kacheln im Gebiet eines Updates zeigen danach dieselben Pixel
wie vorher. Jeder Lauf hält deshalb je Kachel jeder Stufe einen Hash ihrer
Pixel fest und kodiert keine Kachel, die schon dieselben Pixel zeigt
(`schreibe_oder_lass` in [`renderer/src/cli.rs`](../../renderer/src/cli.rs),
`Pixel` in [`renderer/src/cli/pixel.rs`](../../renderer/src/cli/pixel.rs)).
Sie bleibt liegen wie eine mit gleichen Bytes, mit ihrer Zeit und ihrem
ETag, siehe [Kacheln exportieren](kacheln.md), „Gleiche Bytes bleiben
liegen“. Entschieden in
[0091](../entscheidungen/0091-gleiche-pixel-nicht-kodieren.md).

- **Der Hash:** SHA-256 über die RGBA-Bytes der Kachel, davon die ersten 16
  Bytes.
- **Wann er gilt:**
  - nur für die Datei, wie ein Lauf sie hinterliess, mit derselben Grösse
    und derselben Zeit der letzten Änderung auf die Nanosekunde;
  - nur für denselben Kodierstand, `KODIERSTAND` in
    [`renderer/src/cli/pixel.rs`](../../renderer/src/cli/pixel.rs). Er
    steigt, wenn sich am Kodieren etwas ändert: libwebp oder sein Patch,
    die Vorgaben in `encode_webp` oder `Packen`. Was gezeichnet wird, zählt
    nicht, siehe
    [0098](../entscheidungen/0098-der-zeichenstand-statt-des-builds.md).

  Schrieb jemand anders die Datei, kodiert der nächste Lauf sie wie ohne
  Hash und merkt sich den neuen. Das gilt für einen abgebrochenen Lauf,
  `--pyramid` und ein Werkzeug, und ebenso, wenn sie nur eine andere Zeit
  bekam.
- **Jeder Lauf** liest und schreibt die Hashes der Kacheln, die er ablegt,
  auch ein Ausschnitt und `--resume`. Geschrieben wird am Ende. Bricht ein
  Lauf ab, fehlen nur seine Hashes.
- **Die Pyramide eines Updates** baut die Eltern von der Platte nur über
  Kindern ohne Treffer im Hash: über neuen Pixeln, über entfernten Kindern,
  über Kindern, die ein abgebrochener Lauf schrieb oder `--resume` liegen
  lässt, und über Waisen. Ein leeres Kind ohne Datei vorher baut keine. Die
  Ausgabe sagt es: `Pyramide:   … über N Kacheln mit gleichen Pixeln nicht
  neu gebaut`. Ein voller Lauf und ein Update, das umstellt, bauen jede
  Elternkachel. Entschieden in
  [0124](../entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md). An
  der Testwelt mit 16 Änderungen baut die Pyramide 560 statt 970 Kacheln,
  in 0,6 statt 2,6 bis 3,0 s, gemessen in
  [2026-10-24, Pyramide nur über neuen Kindern](../messungen/2026-10-24-pyramide-nur-ueber-neuen.md).
- **Ein anderer Kodierstand** kodiert jede Kachel neu, die ein Lauf
  ablegt. Ein Block eines alten Builds, bis v0.5.0 mit dem Fingerabdruck
  der ausführbaren Datei im Kopf, gilt als Kodierstand 1.
- **Die Ausgabe** nennt am Ende, wie viele Kacheln der Lauf nicht kodiert
  hat:

  ```
  Pixel:      113 Kacheln mit gleichen Pixeln nicht kodiert, 4 Blöcke -> ./tiles/2x1-se/pixel
  ```

- **Wo:** `pixel/` im Baum, je Stufe ein Ordner `<z>`, je Block aus 32 × 32
  Kacheln eine Datei `<bx>.<by>.bin`, mit `bx` = x div 32 und `by` =
  y div 32. Ein Lauf liest einen Block erst, wenn er eine Kachel darin
  ablegt, und schreibt nur geänderte. Ein Update berührt so nur die Blöcke
  um sein Gebiet. Eine Blockdatei, die sich nicht lesen lässt oder einen
  anderen Kodierstand hat, gilt als leer.

| Teil einer Blockdatei | Bytes |
|---|---|
| Kopf: `HMRPIXEL`, Fassung 1, Kodierstand als u64, Zahl der Einträge | 24 |
| je Kachel: Platz `y · 32 + x` im Block, Hash, Grösse, Zeit in ns | 30 |

Alles in Little Endian, nach Platz geordnet.

## Abbruch und `--resume`

Bricht ein Update ab, bleibt der alte Stand, wie er war, und sein Gebiet
liegt offen, siehe „Das offene Gebiet“. Ein neues Update zeichnet es noch
einmal, dazu neuere Änderungen (`abgebrochenes_update_legt_sein_gebiet_ab`). Wer
zusieht, sieht während des Laufs alte neben neuen Kacheln, jede ganz, wie
bei einem vollen Lauf.

`--update --resume` setzt ein abgebrochenes Update fort. Es behält nur
Basiskacheln, die jünger sind als dessen angefangener Stand, ohne die
frischen der letzten zwei Minuten wie
[0019](../entscheidungen/0019-resume-behaelt-die-basiskacheln.md); ältere im
Gebiet zeichnet es neu (`update_setzt_mit_resume_fort`). Am Ende gilt der
angefangene Stand, mit allem als unbekannt, was der Server seitdem schrieb.

Ein voller Lauf mit `--resume` nimmt ebenso den angefangenen Stand des
abgebrochenen Laufs und behält nur Basiskacheln, die jünger sind als er,
siehe [Pyramide und Fortsetzen](pyramide-und-resume.md). Wie ohne
`--resume` sucht er die Änderungen gegen `stand.bin` und zeichnet Kacheln
neu, in die ein Chunk nicht mehr reicht
(`voller_lauf_mit_resume_raeumt_abgerissenes_weg`).

Stammt der angefangene Stand eines vollen Laufs von einem Renderer, der
anders zeichnet, von anderen Assets oder einem Update, läuft der volle Lauf
wie ohne `--resume` und sagt es:

```
Stand:      ./tiles/2x1-se/stand-neu.bin stammt von einem anderen Build des Renderers, der anders zeichnet, oder von anderen Assets
Stand:      dieser Lauf rendert alles wie ohne --resume und schreibt am Ende einen Stand
```

Er rendert jede Basiskachel neu, legt einen neuen angefangenen Stand ab und
schreibt am Ende `stand.bin` (`resume_mit_fremdem_stand_rendert_alles`).
Fortgesetzt behielte er Kacheln, ohne zu wissen, wie sie gezeichnet sind,
und schriebe keinen Stand: Das nächste `--update` zeichnete alles noch
einmal.

Fehlt der angefangene Stand eines vollen Laufs, oder passt der eines Updates
nicht, schreibt der Lauf keinen Stand und sagt es. Ein Update legt sein
Gebiet dann offen ab, ein voller Lauf macht in `stand.bin` alles unbekannt:
Das nächste `--update` zeichnet es noch einmal, nach einem vollen Lauf also
alles. Ohne `stand.bin` braucht `--update` einen
vollen Lauf.

## Anderer Renderer, andere Assets

Ein Update mischte sonst alte und neue Kacheln. Der Stand trägt deshalb zwei
Fingerabdrücke, und `--update` bricht vor der ersten Kachel ab, wenn einer
nicht passt (`update_braucht_den_stand_und_dieselben_assets`):

- **Der Renderer:** FNV-1a über den Zeichenstand des Looks und die
  eingebauten Tabellen, je Tabelle ihr Name und ihre Zeilen ohne `\r`
  (`fingerabdruck_des_renderers` in
  [`renderer/src/render/stand.rs`](../../renderer/src/render/stand.rs)). Je
  Wert von `look` in `map.json`, `map`, `cinematic` und `flat`, gibt es
  einen Zeichenstand. Ein neuer Build, der einen Look gleich zeichnet, hat
  für ihn denselben Abdruck: Ein Release, das nur `--flat` anders zeichnet,
  verlangt nur für Bäume mit `look` `flat` einen vollen Lauf. Wann ein
  Zeichenstand steigt, steht in
  [0098](../entscheidungen/0098-der-zeichenstand-statt-des-builds.md) und
  [0101](../entscheidungen/0101-zeichenstand-je-look.md).
  - **Mit Relief** gehen dazu sein Name und `ZEICHENSTAND_RELIEF` ein, ab
    0122 1, seit der Schicht 2 ([0132](../entscheidungen/0132-relief-als-schicht.md)).
    Ohne Relief bleibt der Abdruck, wie er war. Steigt er, rendern nur Bäume
    mit Relief neu, siehe [0122](../entscheidungen/0122-relief-von-oben.md).
  - **Ein Stand von v0.4.0 oder v0.5.0** trägt noch den Fingerabdruck der
    ausführbaren Datei für Linux oder Windows. Er gilt als Zeichenstand 1 mit den
    Tabellen von v0.5.0, `ZEICHENSTAND_1`: Ein Update nimmt ihn an, solange
    der Renderer noch so zeichnet, und schreibt dann den neuen
    (`update_nimmt_den_stand_alter_builds`). Ebenso ein angefangener Stand.
  - **Die Zeichenstände** bisher:

    | ab | `map` | `cinematic` | `flat` | was anders zeichnet |
    |---|---|---|---|---|
    | 0098, wie v0.4.0 und v0.5.0 | 1 | 1 | – | – |
    | #243 | 2 | 2 | – | der Grund unter Mangrovenwurzeln, siehe [Sprites und Deckung](../renderer/sprites-und-deckung.md), „Verdeckte Würfel“ |
    | #250, wie v0.7.0 | 3 | 3 | 3 | das Licht von `--flat`, siehe [Die einfarbige Ansicht](../renderer/einfarbig.md), „Licht je Spalte“; bis 0101 ein Zeichenstand für alle |
    | #257 | 3 | 3 | 4 | ein Schritt Licht von der Seite in `--flat`, ebenda |
    | #295 | 4 | 3 | 5 | Bodenpflanzen liegen von oben flach, siehe [Die Kamera](../renderer/kamera.md), „Pflanzen von oben“ |

- **Assets und Daten:** je Wurzel von `--assets` und `--data` und je
  Datapack der Welt, das etwas trägt, ihre Nummer, dann je Datei darunter
  der Pfad, die Grösse und die Zeit der letzten Änderung (#288). Ein Datapack
  zählt mit den Dateien, die der Renderer aus ihm liest: Eine geänderte
  Funktion zählt nicht, ein geändertes Biom schon, siehe
  [Assets](assets.md), „Datapacks der Welt“. Ein Test hält den Wert für
  feste Dateien als Konstante fest (`fingerabdruck_der_dateien_bleibt`). Den Inhalt liest er nicht; eine kopierte Datei hat eine neue
  Zeit und zählt als anders.
- **Der Cache des Client-Jars** aus `--download-client-jar` trägt je Datei
  die Bauzeit des Jars: Neu ausgepackt bleibt der Fingerabdruck gleich. Wer
  von eigenen Wurzeln auf den Cache wechselt, bekommt einen neuen und
  einmal einen vollen Lauf, siehe [Assets](assets.md), „Von Mojang laden“.

Danach zeichnet ein voller Lauf alles neu, und `--update` geht wieder.

## Relief wechseln

Das Relief eines Baums steht in `map.json` unter `relief`, siehe
[Relief von oben](../renderer/relief.md). Einen anderen Wert nimmt nur ein
voller Lauf an (`pruefe_relief` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs)):

- **Ein Update und ein Ausschnitt** mit einem anderen Relief brechen ab,
  bevor sie einen Chunk lesen. Sonst mischten sie Kacheln mit und ohne
  Relief. Die Meldung, etwa: „…/map.json gehört zu einem Baum ohne Relief,
  dieser Lauf zeichnet mit Relief slope. Das Relief wechselt nur ein voller
  Lauf ohne --update und --size, er zeichnet den Baum ganz neu.“
- **Ein voller Lauf** wechselt es und zeichnet jede Kachel neu, auch mit
  `--resume`: „Relief:     …/map.json gehört zu einem Baum ohne Relief,
  dieser Lauf zeichnet mit Relief slope und jede Kachel neu“. Danach geht
  `--update` wieder.
- **Ohne Relief** nimmt der volle Lauf die Schicht in `relief` weg.
- **Geprüft** in `relief_wechselt_nur_mit_vollem_lauf` in
  `renderer/tests/cli.rs`.

## Was ein Update nicht bemerkt

- **Ein Werkzeug, das einen Chunk ohne neue Zeit an derselben Stelle
  überschreibt.** Das Spiel tut das nicht, siehe oben.
- **Eine Kachel, die ein Werkzeug mit anderen Bytes, aber derselben Grösse
  und derselben Zeit auf die Nanosekunde zurücklegt.** Sie bleibt stehen,
  bis sich ihre Pixel ändern, siehe „Gleiche Pixel“.
- **Ein Stand aus einem anderen Baum,** von Hand kopiert.
- **Ein vergessener Zeichenstand:** Zeichnet ein neuer Build anders, ohne
  dass der Zeichenstand seines Looks stieg, nimmt `--update` den alten
  Stand an. Es
  zeichnet dann nur, wo sich die Welt änderte, und alte Kacheln stehen
  neben neuen. Ebenso bleiben mit einem vergessenen Kodierstand Kacheln in
  der alten Kodierung liegen. Das fängt nur das Review, siehe
  [0098](../entscheidungen/0098-der-zeichenstand-statt-des-builds.md).
- **Ein ausgelagerter Chunk, gelesen zwischen Kopf und Verschieben,** siehe
  oben: Liest der Lauf den neuen Stempel und noch vor dem Verschieben den
  Inhalt aus der alten `.mcc`, nennt der Stand den neuen Stempel mit dem
  alten Inhalt. Bis der Server den Chunk wieder schreibt, sieht kein Update
  den neuen. Das Fenster ist klein: Das Spiel verschiebt gleich nach dem
  Kopf, und der Lauf liest den Inhalt nach den Köpfen aller Regionen. Gibt
  es noch keine `.mcc`, liest der Lauf nach einer Pause neu, siehe
  [Welten und Kennung](welten.md), „Während der Server schreibt“; dort steht
  auch, was er sonst halb geschrieben sehen kann.

## Kosten

An der Testwelt bei scale 8, gemessen in
[2026-10-04, Updates, Kosten](../messungen/2026-10-04-updates.md):

| | Dauer |
|---|---|
| voller Lauf ohne Stand, master | 84,9 s |
| voller Lauf mit Stand | 89,6 s, davon 4,4 s mehr im Vorlauf für den Fingerabdruck |
| Update ohne Änderung | 0,44 s |
| Update, 10 240 Chunks mit neuem Stempel, gleicher Inhalt | 1,3 s |
| Update, 16 verteilte Chunks gelöscht, 130 Kacheln im Gebiet | 3,0 s |

- **Neue Stempel** kosten Lesen und Fingerabdruck der Chunks, je Region in
  einem Thread, die Regionen parallel.
- **Unbekannte Chunks** zeichnet ein Update über die volle Höhe mit ihren
  Nachbarn. Das erste Update nach einem langen vollen Lauf auf einem Server
  mit Spielern ist darum grösser: Jeder Chunk, den der Server während des
  Laufs schrieb, ist unbekannt. Nach einem Ausschnitt oder einem Abbruch
  zeichnet das erste dessen Gebiet noch einmal, siehe „Das offene Gebiet“.
- **Ein Gebiet** kostet, was ein Ausschnitt dieser Grösse kostet, dazu rund
  2 s für Assets und Sprites.
- **Der Stand** hat 28 Bytes je Chunk, an der Testwelt gerechnet rund
  11 MB. Ein Update schreibt davon nur die Regionen, die sich geändert
  haben, siehe „Der Stand“.
- **Mit einem Thread,** wie das Plugin es startet, brauchte das Update mit
  16 gelöschten Chunks 14,3 s. Mit den Hashes der Pixel waren es 13,9 s;
  198 von 388 Kacheln im Gebiet kodierte es nicht. Die Hashes haben rund
  30 Byte je Kachel, an der Testwelt 0,73 MB. Gemessen in
  [2026-10-08, Gleiche Pixel nicht kodieren](../messungen/2026-10-08-gleiche-pixel.md).
