---
title: Genordete Kameras
description: Was top-north und north-45 aus #67 bei scale 16 am Stand und im Fichtenwald der Testwelt kosten, gegen 2:1 bei scale 32, ohne und mit Karte, je Spalte der Welt, und dass 2:1 Byte für Byte gleich bleibt.
date: 2026-10-02
commits: [4654d8d, d93682d, bbcb829]
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# Genordete Kameras

2:1 bleibt mit #67 an Stand und Fichtenwald Byte für Byte gleich: 8564 und
2144 Kacheln, keine anders. Auch Zeit und Speicher von 2:1 liegen im
Wechsel gegen master in der Streuung. Bei scale 16 belegt eine
Oberseite genordet so viele Pixel wie in 2:1 bei scale 32, und eine
Basiskachel zeigt gleich viele Spalten der Welt. Je Spalte kostet
`north-45` an Bytes das 0,95- bis 0,98-fache von 2:1. An Zeit liegt seine
Basis im Median bei 0,89 bis 0,97; am Stand streuen einzelne Läufe aber von
0,87 bis 1,29, ob es dort einen Unterschied gibt, lässt sich nicht sagen.
`top-north` packt sich kleiner, das 0,53- bis 0,75-fache an Bytes. An Zeit
liegt es im Fichtenwald bei 0,68 bis 0,69; am Stand, mit 1,08 und 1,17 im
Median und 0,81 bis 1,42 je Lauf, lässt sich das nicht sagen. Im ganzen
Lauf waren beide schneller als 2:1 und brauchten weniger Speicher: an der
Spitze 0,94 bis 2,00 GiB gegen 2,02 bis 3,21 GiB. Bei `north-45` kommt
das zum grössten Teil nicht aus der Basis: Der ganze Lauf liegt im Median
bei 0,73 bis 0,84 von 2:1, die Basis je Basiskachel bei 0,89 bis 0,97.
Genordet gibt es bei scale 16 zwei native Stufen statt drei, und der
Ausschnitt wird auf ein feineres Raster gerundet.

## Aufbau

- Stände, Release-Build:
  - **A:** `4654d8d`, master mit #79;
  - **B:** `d93682d`, mit #67 nach dem Merge von master samt #80, je
    Kamera mit `--camera`;
  - **B2:** `bbcb829`, B mit der ersten Runde des Reviews, nur für die
    Serie „2:1 gegen master“. Sie ändert in 2:1 bei scale 32 nichts am
    Weg einer Kachel.
- Die Testwelt, `--native-levels 3`, Pyramide, 24 Threads. 2:1 bei
  scale 32, `top-north` und `north-45` bei scale 16. Mehr native Stufen, als
  eine Kamera hergibt, heisst alle, die sie hergibt: 2:1 bei 32 hat drei
  (16, 8, 4), genordet bei 16 zwei (8, 4), siehe
  [Die Kamera](../renderer/kamera.md), „Ganze Pixel“.
  - **Stand:** um (-64, 416) mit `--size 18432`, wie in
    [2026-10-01, Kameras](2026-10-01-kameras.md);
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, ebenso.
- Gleiche Bildfläche je Kamera: `--size` bleibt in Pixeln gleich. Der Lauf
  rundet den Ausschnitt auf ganze Kacheln der gröbsten nativen Stufe auf,
  siehe [Kacheln exportieren](../benutzung/kacheln.md). In 2:1 sind das je
  8 Basiskacheln, genordet je 4; deshalb hat genordet weniger
  Basiskacheln.
- Je Spalte: Eine Basiskachel von 256 Pixeln zeigt in 2:1 bei scale 32
  wie genordet bei scale 16 256 Spalten der Welt. „Je Spalte gegen 2:1“ ist
  deshalb der Wert der Basis je Basiskachel geteilt durch den von 2:1.
- Nicht derselbe Boden: Das Fenster von 2:1 steht in der Welt um 45°
  gedreht, das genordete achsparallel. Nur rund zwei Drittel der Spalten
  aus 2:1 liegen auch im genordeten Fenster, im Review gerechnet 67 % am
  Stand und 65 % im Fichtenwald. „Je Spalte“ mischt also Kamera und
  Gelände.
- Serien:
  - **Gleich:** A und B in 2:1 bei scale 32 je ein Lauf, `--gpu off`, alle
    Kacheln per SHA-256 verglichen;
  - **Ohne Karte:** B in 2:1, `top-north` und `north-45` im Wechsel,
    `--gpu off`, drei Runden, die Reihenfolge je Runde umgekehrt;
  - **Mit Karte:** ebenso mit `--gpu on`;
  - **2:1 gegen master:** A und B2 in 2:1 bei 32 im Wechsel, ohne und
    mit Karte, drei Runden, die Reihenfolge je Runde umgekehrt. Nach jedem
    Lauf wird sein Baum gelöscht und dann 15 s gewartet. Ohne die Pause
    hängen nach #56 einzelne Schreibvorgänge der Kacheln, und die Basis
    wird langsam. Die Serien „ohne Karte“ und „mit Karte“ liefen noch ohne
    Pause.
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf ging in ein frisches
  Verzeichnis, das vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Last: Nebenher liefen keine Builds, Tests oder Messungen.
  - Gleich: vor der Serie 16 %, danach 6 %.
  - Ohne Karte: über 20 s davor im Mittel 7,7 %, direkt davor 5 %, danach
    0 %.
  - Mit Karte: über 20 s davor im Mittel 2,3 %, direkt davor 0 %, danach
    15 %.
  - 2:1 gegen master: über 20 s davor im Mittel 5,6 %, direkt davor 6 %,
    danach 3 %.
- Ein erster Durchgang von „ohne“ und „mit Karte“ lief über einen Neustart
  der Arbeitsumgebung und ohne die Prüfung der Last vor „mit Karte“. Er ist
  verworfen, und beide Serien sind danach neu gemessen. „Gleich“ lief
  davor und bleibt.

Der Befehl, aus der Wurzel des Repositorys, für den Fichtenwald in
`north-45` ohne Karte:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -2712 -3297 --size 8192 --scale 16 --native-levels 3 --gpu off --camera north-45
```

## Ablauf

Die Dauer der Basis und der nativen Stufen stammt aus der Ausgabe des
Laufs. Für die nativen Stufen gilt die Zeile, die die Zeit für alle
zusammen nennt. Welche Stufen nativ waren, steht in den Zeilen „Kacheln
nativ bei scale“. Die Dauer des ganzen Laufs misst das Messskript. Die
Spitze des Speichers ist `PeakWorkingSetSize` des Prozesses. Kacheln und
Bytes sind die Dateien unter `--tiles`, Bytes dezimal; die Bytes der Basis
sind die der Stufe `maxZoom`. In den Tabellen steht der Median, dahinter die
Spanne. Der Lauf nennt die Zeit der Basis auf 0,1 s genau; bei 1,0 bis
2,2 s im Fichtenwald sind das allein bis ±5 %. Gemessen am 02.10.2026.

## Ergebnis

### Gleich

| Ausschnitt | Kacheln A | Kacheln B | anders |
|---|---|---|---|
| Stand | 8564 | 8564 | 0 |
| Fichtenwald | 2144 | 2144 | 0 |

### 2:1 gegen master

| Karte | Ausschnitt | Stand | Basiskacheln | Basis | ganzer Lauf | Spitze | Kacheln | Bytes |
|---|---|---|---|---|---|---|---|---|
| ohne | Stand | A | 6400 | 4,9 s (4,9–5,0) | 11,3 s (11,3–11,3) | 2,37 GiB (2,34–2,40) | 8564 | 586,7 MB |
|  |  | B2 | 6400 | 5,0 s (4,8–5,1) | 11,4 s (11,1–11,6) | 2,40 GiB (2,39–2,40) | 8564 | 586,7 MB |
|  | Fichtenwald | A | 1600 | 1,8 s (1,8–1,8) | 4,8 s (4,7–5,1) | 2,03 GiB (2,00–2,05) | 2144 | 202,3 MB |
|  |  | B2 | 1600 | 1,8 s (1,7–1,8) | 4,7 s (4,5–4,8) | 2,06 GiB (2,04–2,08) | 2144 | 202,3 MB |
| mit | Stand | A | 6400 | 3,8 s (3,8–3,9) | 9,8 s (9,6–9,8) | 3,17 GiB (3,16–3,29) | 8564 | 586,7 MB |
|  |  | B2 | 6400 | 3,9 s (3,8–3,9) | 9,6 s (9,6–9,8) | 3,21 GiB (3,18–3,25) | 8564 | 586,7 MB |
|  | Fichtenwald | A | 1600 | 1,5 s (1,5–1,6) | 4,3 s (4,3–4,3) | 2,72 GiB (2,71–2,73) | 2144 | 202,3 MB |
|  |  | B2 | 1600 | 1,5 s (1,5–1,6) | 4,4 s (4,3–4,4) | 2,72 GiB (2,71–2,73) | 2144 | 202,3 MB |

### 24 Threads ohne Karte

| Ausschnitt | Kamera | scale | nativ bei scale | Basiskacheln | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|---|---|---|
| Stand | 2:1 | 32 | 16, 8, 4 | 6400 | 5,2 s (5,0–5,3) | 5,2 s (5,2–5,3) | 11,8 s (11,5–12,0) | 2,40 GiB (2,36–2,44) |
|  | top-north | 16 | 8, 4 | 5472 | 4,8 s (3,6–5,0) | 3,0 s (3,0–3,1) | 8,4 s (7,5–9,0) | 0,94 GiB (0,91–0,94) |
|  | north-45 | 16 | 8, 4 | 5472 | 4,3 s (4,3–5,0) | 3,9 s (3,9–4,6) | 9,9 s (9,2–10,0) | 1,36 GiB (1,34–1,36) |
| Fichtenwald | 2:1 | 32 | 16, 8, 4 | 1600 | 2,0 s (1,9–2,0) | 2,3 s (2,2–2,3) | 5,2 s (5,0–5,3) | 2,02 GiB (2,01–2,02) |
|  | top-north | 16 | 8, 4 | 1296 | 1,1 s (1,0–1,1) | 0,9 s (0,9–0,9) | 2,7 s (2,5–2,8) | 1,14 GiB (1,12–1,16) |
|  | north-45 | 16 | 8, 4 | 1296 | 1,5 s (1,4–2,2) | 1,5 s (1,4–1,5) | 3,9 s (3,6–4,4) | 1,44 GiB (1,39–1,46) |

### 24 Threads mit Karte

| Ausschnitt | Kamera | scale | nativ bei scale | Basiskacheln | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|---|---|---|
| Stand | 2:1 | 32 | 16, 8, 4 | 6400 | 4,7 s (4,2–4,8) | 5,0 s (4,7–5,1) | 11,6 s (10,7–11,7) | 3,21 GiB (3,20–3,24) |
|  | top-north | 16 | 8, 4 | 5472 | 4,7 s (3,6–5,7) | 2,7 s (2,6–2,7) | 8,5 s (7,6–9,7) | 1,55 GiB (1,51–1,56) |
|  | north-45 | 16 | 8, 4 | 5472 | 3,9 s (3,5–5,2) | 3,6 s (3,6–3,8) | 9,3 s (8,7–10,3) | 2,00 GiB (1,99–2,01) |
| Fichtenwald | 2:1 | 32 | 16, 8, 4 | 1600 | 1,8 s (1,8–2,1) | 2,4 s (2,1–2,5) | 5,6 s (5,2–5,8) | 2,59 GiB (2,54–2,61) |
|  | top-north | 16 | 8, 4 | 1296 | 1,0 s (1,0–1,0) | 0,8 s (0,8–0,9) | 2,9 s (2,9–3,1) | 1,53 GiB (1,50–1,55) |
|  | north-45 | 16 | 8, 4 | 1296 | 1,3 s (1,3–1,3) | 1,4 s (1,4–1,5) | 4,1 s (4,0–4,7) | 1,90 GiB (1,90–1,93) |

### Grösse und je Spalte

Die Kacheln und Bytes waren in jedem Lauf einer Kamera gleich, ohne wie
mit Karte.

| Ausschnitt | Kamera | scale | Kacheln | Bytes | Basiskacheln | Bytes der Basis | Bytes je Spalte gegen 2:1 | Zeit ohne Karte | Zeit mit Karte |
|---|---|---|---|---|---|---|---|---|---|
| Stand | 2:1 | 32 | 8564 | 586,7 MB | 6400 | 439,1 MB | 1,00 | 1,00 | 1,00 |
|  | top-north | 16 | 7326 | 369,7 MB | 5472 | 283,0 MB | 0,75 | 1,08 | 1,17 |
|  | north-45 | 16 | 7326 | 485,6 MB | 5472 | 369,7 MB | 0,98 | 0,97 | 0,97 |
| Fichtenwald | 2:1 | 32 | 2144 | 202,3 MB | 1600 | 155,9 MB | 1,00 | 1,00 | 1,00 |
|  | top-north | 16 | 1745 | 86,6 MB | 1296 | 67,3 MB | 0,53 | 0,68 | 0,69 |
|  | north-45 | 16 | 1745 | 156,3 MB | 1296 | 120,6 MB | 0,95 | 0,93 | 0,89 |

## Schluss

- **2:1 ändert sich nicht.** Die Kacheln an Stand und Fichtenwald sind
  Byte für Byte gleich, und Zeit und Spitze liegen im Wechsel gegen
  master in der Streuung: die Basis am Stand 4,9 gegen 5,0 s ohne und 3,8
  gegen 3,9 s mit Karte, im Fichtenwald gleich.
- **Mit der Pause streut die Basis kaum.** In 2:1 bei 32 lagen alle Läufe
  je Stand und Karte innerhalb von 0,3 s. Die Serien ohne Pause liefen zu
  einer anderen Zeit; ein Vergleich mit ihnen trägt nicht. Was die Pause
  an derselben Stelle bringt, misst #56, siehe
  [2026-10-02, Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md).
- **`north-45` bei 16 kostet je Spalte etwa wie 2:1 bei 32.** Seine
  Oberseite hat so viele Pixel wie die von 2:1. Ein voller Würfel belegt
  aber 512 Pixel, Oberseite und Südwand, gegen 768 in 2:1 mit Süd- und
  Ostwand. Bytes und Zeit der Basis liegen im Median bei 0,89 bis 0,98, am
  Stand mit der Streuung aus der Einleitung.
- **`top-north` zeigt nur Oberseiten.** Wie `top` aus
  [2026-10-01, Kameras](2026-10-01-kameras.md) packt es sich kleiner,
  im Fichtenwald auf etwa die Hälfte. Seine Basis am Stand streut zwischen
  zwei Zuständen, 3,6 und rund 5 s, wie die Basis in
  [2026-09-29, Pyramide von der Platte und im Speicher](2026-09-29-pyramide-platte-und-speicher.md).
  Ob es dort einen Unterschied zu 2:1 gibt, lässt sich nicht sagen.
- **Weniger native Stufen:** Genordet bei 16 gibt es nur 8 und 4, native
  Stufen gehen bis scale 4. Die nativen Stufen sind deshalb kürzer, und
  eine Stufe mehr wird verkleinert statt gerendert. Das ist keine Rundung;
  gerundet wird nur der Ausschnitt.
- **Die Spitze des Speichers** fällt bei beiden genordeten Kameras, mit und
  ohne Karte. Das passt dazu, dass eine native Stufe und je Ausschnitt
  Basiskacheln wegfallen. Es passt auch zu den Chunks je Band: Diagonal ist
  ein Band ein langer schräger Streifen. Genordet reicht es bei `north-45`
  nur nach Norden in die Tiefe, bei `top-north` gar nicht. Einzeln gemessen
  ist das nicht.
