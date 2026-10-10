---
title: Updates
description: Wie --update nur zeichnet, wo sich die Welt seit dem letzten vollen Lauf geändert hat, mit dem Stand je Baum, Zeitstempeln und Fingerabdrücken je Chunk, dem Gebiet einer Änderung, Abbruch und --resume, und wann ein voller Lauf nötig ist.
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

Jeder Baum hat seinen Stand in `stand.bin` neben `map.json`. Ihn schreibt
nur ein voller Lauf über die ganze Welt oder ein Update, und zwar ganz am
Ende, nach `map.json`; getauscht wie jede Datei, siehe
[0018](../entscheidungen/0018-dateien-tauschen-statt-ueberschreiben.md).
Ein Ausschnitt schreibt keinen.

Kacheln ohne neuen Stand zeichnen ein Ausschnitt, ein Lauf, der abbricht,
und einer mit `--resume`, der keinen Stand schreibt. Damit `stand.bin` nie
einen Inhalt nennt, den eine Kachel vielleicht nicht zeigt, macht jeder
Lauf vor der ersten Kachel im alten Stand unbekannt, was er zeichnen kann:
jeden Chunk, den er liest, auch für einen Strahl zur Sonne, samt seinen
acht Nachbarn; ein voller Lauf also alle (`Stand::unbekannt_wo`). Das gilt
für jeden Chunk, den der Stand kennt oder der jetzt in der Welt steht. Das
nächste Update zeichnet dieses Gebiet über die volle Höhe noch einmal, auch
wenn ein Chunk darin wieder seinen alten Inhalt hat, wie ein Ofen, der
wieder ausgeht (`zurueckgewechselter_chunk_wird_gezeichnet`, nach einem
abgebrochenen Update und nach einem Ausschnitt).

Der Stand enthält die Stempel, wie sie zu Beginn des Laufs waren, vor dem
Vorlauf gelesen. Schreibt der Server einen Chunk während des Laufs, liest
der Lauf am Ende die Köpfe noch einmal: Jeder Chunk mit anderem Stempel
steht als unbekannt im Stand und gilt beim nächsten Update als geändert,
über die volle Höhe.

Vor der ersten Kachel legt der Lauf den angefangenen Stand als
`stand-neu.bin` ab, wie `map.json`, und daneben das Protokoll der Kacheln,
die er liegen lässt, siehe [Pyramide und Fortsetzen](pyramide-und-resume.md),
„Fortsetzen: `--resume`“. Beides braucht nur `--resume`, siehe unten;
am Ende fällt beides weg.

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

Bricht ein Update ab, bleibt der alte Stand, sein Gebiet darin unbekannt,
siehe „Der Stand“. Ein neues Update zeichnet das Gebiet noch einmal, dazu
neuere Änderungen (`abgebrochenes_update_macht_sein_gebiet_unbekannt`). Wer
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
nicht, schreibt der Lauf keinen Stand und sagt es. In `stand.bin` ist sein
Gebiet dann unbekannt: Das nächste `--update` zeichnet es noch einmal, nach
einem vollen Lauf also alles. Ohne `stand.bin` braucht `--update` einen
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

- **Assets und Daten:** je Wurzel von `--assets` und `--data` ihre Nummer,
  dann je Datei darunter der Pfad, die Grösse und die Zeit der letzten
  Änderung. Den Inhalt liest er nicht; eine kopierte Datei hat eine neue
  Zeit und zählt als anders.
- **Der Cache des Client-Jars** aus `--download-client-jar` trägt je Datei
  die Bauzeit des Jars: Neu ausgepackt bleibt der Fingerabdruck gleich. Wer
  von eigenen Wurzeln auf den Cache wechselt, bekommt einen neuen und
  einmal einen vollen Lauf, siehe [Assets](assets.md), „Von Mojang laden“.

Danach zeichnet ein voller Lauf alles neu, und `--update` geht wieder.

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
  Laufs schrieb, ist unbekannt. Ebenso das erste nach einem Ausschnitt oder
  einem Abbruch, über dessen Gebiet.
- **Ein Gebiet** kostet, was ein Ausschnitt dieser Grösse kostet, dazu rund
  2 s für Assets und Sprites.
- **`stand.bin`** hat rund 24 Bytes je Chunk, an der Testwelt 7,5 MB.
- **Mit einem Thread,** wie das Plugin es startet, brauchte das Update mit
  16 gelöschten Chunks 14,3 s. Mit den Hashes der Pixel waren es 13,9 s;
  198 von 388 Kacheln im Gebiet kodierte es nicht. Die Hashes haben rund
  30 Byte je Kachel, an der Testwelt 0,73 MB. Gemessen in
  [2026-10-08, Gleiche Pixel nicht kodieren](../messungen/2026-10-08-gleiche-pixel.md).
