---
title: Vollrender der grossen Welt, 4:3 bei scale 24
description: Der Vollrender der grossen Welt mit Cinematic in 4:3 bei scale 24 ohne native Stufe, über dasselbe Rechteck wie der Lauf in 8:5. Dauer, Rate, Kacheln und Grösse je Stufe, wie viel die Pyramide ohne native Stufe wiegt, und die Schätzung vor dem Start gegen das Ergebnis.
date: 2026-10-04
commits: [f7b9ba3]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/pyramid.rs
---

# Vollrender der grossen Welt, 4:3 bei scale 24

Die grosse Welt mit Cinematic, 4:3 bei scale 24 ohne native Stufe,
brauchte 2 h 43 min für 160,2 GB: die Basis 2 h 40 min für 2,07 Mio.
Kacheln, 217 Kacheln/s, die Pyramide 74 s. Ohne native Stufe trägt die
Pyramide auch Zoom 10 und wiegt 37 % der Basis. Im Lauf in 8:5 mit einer
nativen Stufe wogen native Stufe und Pyramide zusammen 35 %, die Pyramide
allein 10 %. Angesagt waren 2,5 bis 3 h und 135 bis 140 GB: Die Dauer
traf, die Grösse lag 14 % über der oberen Ansage. Fortsetzung von
[Vollrender mit Cinematic](2026-10-04-vollrender-cinematic.md).

## Aufbau

- **Welt:** die grosse Welt, 26.2, mit ihren eigenen Biomdaten neben denen
  von Vanilla. Dasselbe Rechteck wie in
  [Vollrender mit Cinematic](2026-10-04-vollrender-cinematic.md):
  1564 × 1564 Chunks, darin 2 427 101 fertig erzeugte.
- **Stand:** master `f7b9ba3`, mit einem Himmelslicht für alle Biome aus
  #124, dem Treiber für `--pyramid` aus #126, dem Stand für Updates aus
  #129 und den Hebeln 1, 2 und 4 aus #118. Release-Build, alle 24
  Threads.
- **Ansicht:** Cinematic mit `LOOK`, Kamera 4:3 aus `se`, scale 24, ohne
  native Stufe: 4:3 hat bei scale 24 keine, siehe
  [Kamera](../renderer/kamera.md), „Ganze Pixel“. Cinematic zeichnet auf
  der CPU, `--gpu off`.
- **Assets:** die aus dem Client von 26.2, dazu das Overlay unter
  `./assets`.
- Der Kachelordner lag in einem Verzeichnis, das vom Echtzeitschutz
  ausgenommen ist, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- **Live-Ansicht:** Ein Treiber rief `--pyramid` über den Baum, 150 s nach
  dem Start und danach 5 min nach jedem Ende: 27 Aufrufe während der
  Basis, zusammen 618 s, und einer nach dem Ende. Ein Devserver lieferte
  die Kacheln über einen Link aus, siehe [Das Frontend](../frontend.md),
  „Einem Render zusehen“.
- **Nebenher:** Anders als beim Lauf in 8:5 durfte gebaut werden. Die
  Sperrdatei sagte „Builds erlaubt“, und das Frontend baute und testete
  während des Laufs, auf Wunsch des Users. Die Rate ist darum nicht ganz
  mit der des Laufs in 8:5 vergleichbar.

Der Befehl, aus der Wurzel des Repositorys:

```bash
renderer/target/release/heroic-map-renderer --world <grosse Welt> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --data <ihre Biomdaten> --tiles <ordner> --camera 4:3 --direction se --scale 24 --native-levels 0 --cinematic --gpu off --area <Rechteck>
```

## Ablauf

Am 04.10. von 11:54:54 bis 14:37:50. Die Ausgabe des Laufs und die des
Treibers hat ein Filter Zeile für Zeile mit der Uhrzeit versehen; alle
Zeiten und Zahlen stammen aus diesen beiden Logs. Die Ausgabe zählt MB
binär, hier in GB umgerechnet; die Grössen sind Summen der Dateigrössen.
Die Spitze des Arbeitsspeichers ist nicht gemessen.

Die Schätzung vor dem Start stammt aus den ersten 1,5 min der Basis: der
Rate und der mittleren Grösse der ersten 26 000 Kacheln, Dateigrössen.

## Ergebnis

| | Kacheln | Grösse | Dauer | Kacheln/s |
|---|---|---|---|---|
| Vorlauf | 2 427 101 Chunks, 2 073 988 Kacheln geplant | | 118 s | |
| Höhen | 2500 Regionen | 13,6 MB | 1,5 s | |
| Basis, Zoom 11 | 2 073 988: 2 068 641 geschrieben, 5 347 leer | 117,0 GB, 56,5 kB je Kachel | 9 571 s, 2 h 40 min | 217 |
| Pyramide, Zoom 0 bis 10 | 692 365, davon 648 302 schon während der Basis | 43,3 GB | 74 s | |
| zusammen | | 160,2 GB | 2 h 43 min | |

Die Pyramide je Stufe: Zoom 10 518 275 Kacheln, Zoom 9 130 027, Zoom 8
32 787, Zoom 7 8 312, Zoom 6 2 151, darüber 813.

## Die Pyramide ohne native Stufe

| | 4:3, scale 24, ohne native Stufe | 8:5, scale 32, eine native Stufe |
|---|---|---|
| Basis | 117,0 GB | 165,5 GB |
| Zoom 10 | in der Pyramide, 518 275 Kacheln | native Stufe, 768 442 Kacheln, 41,3 GB, 25 % der Basis |
| Zoom 0 bis 9 | in der Pyramide | Pyramide, 16,0 GB, 10 % der Basis |
| über der Basis zusammen | 43,3 GB, 37 % der Basis | 57,3 GB, 35 % der Basis |
| Dauer über der Basis | 74 s | 1 h 26 min, fast alles native Stufe |

- **Die Pyramide allein** ist zwischen den Läufen nicht vergleichbar: In
  8:5 trug die native Stufe Zoom 10. Zusammen wiegen die Stufen über der
  Basis in beiden Läufen gut ein Drittel der Basis.
- **Die Zeit** über der Basis liegt ohne native Stufe bei 74 s: Die
  Pyramide setzt Zoom 10 aus der Basis zusammen, die native Stufe rendert
  ihn aus der Welt.

## Gegen die Schätzung

| | vorher grob | angesagt nach dem Vorlauf | gemessen |
|---|---|---|---|
| Dauer | 2,5 bis 3 h | Basis 2,5 bis 2,6 h, geplant bis 3 h | 2 h 43 min, Basis 2 h 40 min |
| Grösse | rund 150 GB | 135 bis 140 GB | 160,2 GB |

- **Dauer:** Die ersten 1,5 min liefen mit 220 bis 230 Kacheln/s, der
  ganze Lauf mit 217. Die Ansage traf.
- **Grösse:** Die ersten Kacheln hatten im Mittel 59 kB, der ganze Lauf
  56,5 kB; die Basis lag mit 117,0 GB unter der Ansage von rund 125 GB. Die
  Pyramide war mit 10 % der Basis angesetzt, nach dem Lauf in 8:5. Ohne
  native Stufe sind es 37 %, siehe oben; daher die 14 % zu wenig.
- **Die Live-Ansicht** kostete hier, wie in 8:5, Zeit in der Basis; wie
  viel, ist nicht ausgezählt, weil die Builds des Frontends sich mit ihr
  mischen.

## Schluss

- Ein Vollrender der grossen Welt mit Cinematic in 4:3 bei scale 24 ohne
  native Stufe kostet rund 2 h 45 min und 160 GB.
- Für eine Hochrechnung ohne native Stufe die Pyramide mit gut einem
  Drittel der Basis ansetzen, nicht mit dem Zehntel aus einem Lauf mit
  nativer Stufe.
