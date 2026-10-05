---
title: Vollrender der grossen Welt mit Cinematic
description: Der erste Vollrender der grossen Welt mit Cinematic, Kamera 8:5 bei scale 32 mit einer nativen Stufe, über ein Rechteck aus 1564 × 1564 Chunks, mit Live-Ansicht. Dauer je Stufe, Kacheln, Grösse, was die Live-Ansicht kostete, die Drosselung der Maschine am Abend und der Vergleich mit der Hochrechnung.
date: 2026-10-04
commits: [4d1abbc]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/pyramid.rs
---

# Vollrender der grossen Welt mit Cinematic

Die grosse Welt mit Cinematic, 8:5 bei scale 32 und einer nativen Stufe,
brauchte 5 h 26 min für 222,8 GB: die Basis 3 h 58 min für 3,07 Mio.
Kacheln, 215 Kacheln/s, die native Stufe 1 h 26 min. Ohne die Live-Ansicht
nebenher wäre die Basis rund 7 % schneller gewesen, 231 Kacheln/s. Der
Lauf lag in der Hochrechnung von 4,3 bis 8,3 h und knapp unter der Grösse
von 224 bis 258 GB.

## Aufbau

- **Welt:** die grosse Welt, 26.2, mit ihren eigenen Biomdaten neben denen
  von Vanilla. Gezeichnet ein Rechteck mit `--area`, auf ganze Chunks
  gerundet 1564 × 1564 Chunks, 626 km²; darin 2 427 101 fertig erzeugte
  Chunks und 15 245 nicht fertig erzeugte, die der Lauf nicht zeichnet.
- **Stand:** master `4d1abbc`, mit den Flächen im Innern aus #51, der
  Sicht in der Ecke nach der Version der Welt aus #111 und `--area` aus
  #115. Release-Build, alle 24 Threads.
- **Ansicht:** Cinematic mit `LOOK`, Kamera 8:5 aus `se`, scale 32, eine
  native Stufe bei scale 16. Cinematic zeichnet auf der CPU, `--gpu off`.
  Die weiche Beleuchtung wie in 26.2, nach der Datenversion der Welt.
- **Assets:** die aus dem Client von 26.2, dazu das Overlay unter
  `./assets`.
- Der Kachelordner lag in einem Verzeichnis, das vom Echtzeitschutz
  ausgenommen ist, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Den
  Hinweis dazu gibt die Ausgabe trotzdem aus.
- **Live-Ansicht:** Während der Basis rief ein Treiber `--pyramid` über den
  Baum, 150 s nach dem Start und danach 5 min nach jedem Ende, 37 Aufrufe,
  zusammen 48 min. Ein Devserver lieferte die Kacheln über einen Link aus,
  siehe [Das Frontend](../frontend.md), „Einem Render zusehen“.
- Während des Laufs lag die Sperrdatei, es lief kein Build und kein Test.

Der Befehl, aus der Wurzel des Repositorys:

```bash
renderer/target/release/heroic-map-renderer --world <grosse Welt> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --data <ihre Biomdaten> --tiles <ordner> --camera 8:5 --direction se --scale 32 --native-levels 1 --cinematic --gpu off --area <Rechteck>
```

## Ablauf

Am 03.10. von 19:30:13 bis 00:56:39. Die Ausgabe des Laufs und die des
Treibers hat ein Filter Zeile für Zeile mit der Uhrzeit versehen; alle
Zeiten und Zahlen stammen aus diesen beiden Logs. Die Ausgabe zählt MB
binär, hier in GB umgerechnet; die Grössen sind Summen der Dateigrössen.
Die Spitze des Arbeitsspeichers ist nicht gemessen.

Die Kosten der Live-Ansicht: Die Basis meldet sich alle 200 Kacheln mit
Uhrzeit. Fiel die Mitte eines Abschnitts in einen Aufruf von `--pyramid`
aus dem Log des Treibers, zählt er zu „mit“, sonst zu „ohne“.

## Ergebnis

| | Kacheln | Grösse | Dauer | Kacheln/s |
|---|---|---|---|---|
| Vorlauf | 2 427 101 Chunks, 3 076 640 Kacheln geplant | | 68 s | |
| Höhen | 2500 Regionen | 13,6 MB | 1 s | |
| Basis, Zoom 11 | 3 076 640: 3 068 949 geschrieben, 7 691 leer | 165,5 GB, 53,9 kB je Kachel | 14 294 s, 3 h 58 min | 215 |
| nativ, Zoom 10, scale 16 | 768 442 | 41,3 GB | 5189 s, 1 h 26 min | 148 |
| Pyramide, Zoom 0 bis 9 | 257 817, davon 241 227 schon während der Basis | 16,0 GB | 22 s | |
| zusammen | | 222,8 GB | 5 h 26 min | |

Die Basis mit und ohne `--pyramid` nebenher:

| | Kacheln | Dauer | Kacheln/s |
|---|---|---|---|
| während eines Aufrufs | 436 640 | 2876 s | 152 |
| sonst | 2 639 800 | 11 417 s | 231 |

Mit 231 Kacheln/s hätte die Basis 13 305 s gebraucht statt 14 293 s, 6,9 %
weniger.

## Gegen die Hochrechnung

- **Dauer:** Die Hochrechnung aus den Faktoren gegen die Karte nannte 3,6
  bis 6,9 h, dazu für die Drosselung am Abend rund 20 % mehr, 4,3 bis
  8,3 h. Gemessen sind 5 h 26 min.
- **Grösse:** hochgerechnet 224 bis 258 GB, gemessen 222,8 GB.
- **Drosselung:** Am Abend brauchte master am Stand der Testwelt 37 %
  (Karte) und 17 % (Cinematic) länger als am Morgen. Ein A/B zeigte die
  Maschine als Ursache: Dieselbe Binärdatei war abends 20 % langsamer, und
  unter voller Last fiel die Leistung der CPU in 10 s von 77 auf 69 % des
  Nenntakts, wie bei Wärme. Der Code kostete nur 0,7 s im Start je Lauf,
  mit #117, siehe #123. Mit `--area` rechnet der Lauf diese Hülle nicht.
- **Native Stufe:** 0,36 der Basis. Gerechnet waren 0,33 bis 0,48.

## Schluss

- Ein Vollrender mit Cinematic über die grosse Welt passt in eine Nacht,
  mit Live-Ansicht in 5 h 26 min.
- Während eines Aufrufs von `--pyramid` schafft die Basis 34 % weniger,
  mit #21 war es rund die Hälfte. Über den Lauf kostet die Live-Ansicht
  rund 7 %; das hängt auch daran, wie oft der Treiber ruft. Vermutlich ist
  es weniger als mit #21, weil `--pyramid` seit #38 schneller von der
  Platte baut und Cinematic je Kachel länger rechnet als die Karte, sodass
  das Verkleinern weniger ins Gewicht fällt; für sich gemessen ist das
  nicht.
- Die native Stufe schafft 148 Kacheln/s, gut zwei Drittel der Basis.
