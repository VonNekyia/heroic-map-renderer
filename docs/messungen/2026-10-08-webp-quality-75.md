---
title: WebP mit quality 75
description: Grösse und Dauer, wenn libwebp die Kacheln mit method 0 und quality 75 statt auf Stufe 0 packt, an der Testwelt mit nativen Stufen, mit Cinematic und auf einem Thread, dazu die Hochrechnung für die grosse Welt.
date: 2026-10-08
commits: [a11017a, deeddfe]
code:
  - renderer/src/render/tiles.rs
---

# WebP mit quality 75

Mit method 0 und quality 75 werden die Basiskacheln ×0,79 so gross, die
nativen Stufen ×0,85, Pixel für Pixel gleich. Auf einem Thread kostet das
rund 0,35 ms je Kachel, ein voller Lauf mit nativen Stufen auf 4 Threads
6 % mehr Zeit. Für die grosse Welt sind das hochgerechnet rund 150 statt
188 GB.

## Aufbau

- **Welt:** die Testwelt, der Ausschnitt um (-64, 416) aus
  [Was ein Lauf kostet](../benutzung/kosten.md), scale 32, 2:1 aus `se`,
  `--gpu off`, Release-Build:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --center -64 416 --scale 32 --gpu off --tiles <leer> <Versuch>
  ```

- **Versuche:**

  | Versuch | Schalter | Kacheln |
  |---|---|---|
  | 4 Threads | `--size 10240 --native-levels 3 --threads 4` | 2304 Basis, 756 nativ, 26 Pyramide |
  | Cinematic | `--size 10240 --threads 4 --cinematic` | 1600 Basis, 603 Pyramide |
  | ein Thread | `--size 8192 --threads 1` | 1024 Basis, 401 Pyramide |

- **Stände:** master `a11017a` mit Stufe 0 gegen `deeddfe` mit method 0 und
  quality 75.

## Ablauf

- **Reihenfolge:** abwechselnd master und `deeddfe`. Je drei Läufe mit 4
  Threads und auf einem Thread, je einer mit Cinematic, der nur für die
  Grösse.
- **Jeder Lauf:**
  - frisch in ein leeres Verzeichnis, danach dessen Baum gelöscht und 15 s
    Pause;
  - davor die Last unter 10 %;
  - die ganze Reihe unter der Sperre, ohne laufenden Minecraft-Client.
- **Quelle,** am 08.10.:
  - das Messskript: Wanduhr, CPU-Zeit des Prozesses und Spitze des
    Arbeitsspeichers;
  - die Dateigrössen der Bäume, dezimal;
  - die Kachelzahlen aus der Ausgabe der Läufe.
- **Pixel:** Jede Kachel des ersten Laufs je Stand ist mit Pillow 12.2
  dekodiert und RGBA für RGBA gegen den anderen Stand verglichen. Das sind
  alle 3086 Kacheln mit 4 Threads und alle 2203 mit Cinematic.

## Ergebnis

| | master | quality 75 | Faktor |
|---|---|---|---|
| 4 Threads, Basis | 148,8 MB, 64,6 kB je Kachel | 118,2 MB, 51,3 kB je Kachel | ×0,79 |
| dito, native Stufen | 49,2 MB | 42,1 MB | ×0,85 |
| dito, Pyramide | 1,20 MB | 1,20 MB | ×1,00 |
| dito, zusammen | 199,2 MB | 161,5 MB | ×0,81 |
| dito, Wanduhr | 11,54, 11,67, 11,74 s | 12,26, 12,38, 12,42 s | +6 % |
| dito, CPU | 40,7 bis 41,2 s | 43,1 bis 43,8 s | +6 % |
| dito, Spitze | 0,57 bis 0,60 GiB | 0,58 bis 0,60 GiB | gleich |
| Cinematic, Basis | 101,1 MB | 83,3 MB | ×0,82 |
| dito, Pyramide | 37,8 MB | 34,1 MB | ×0,90 |
| dito, Wanduhr | 16,1 s | 16,5 s | |
| ein Thread, Basis | 75,2 MB | 61,4 MB | ×0,82 |
| dito, Pyramide | 27,8 MB | 25,1 MB | ×0,90 |
| dito, Wanduhr | 11,43, 11,69, 11,80 s | 12,02, 12,16, 12,17 s | +4 % |
| dito, CPU | 11,2 bis 11,6 s | 11,8 bis 12,1 s | +4 % |

- **Pixel:** Alle 3086 und alle 2203 Kacheln sind gleich.
- **Bytes:** Die drei Läufe eines Stands gaben jeweils Byte für Byte
  denselben Baum.
- **Je Kachel:** Auf einem Thread kostet quality 75 im Median 0,47 s
  Wanduhr und 0,5 s CPU mehr für 1425 kodierte Kacheln. Das sind rund
  0,35 ms je Kachel. Mit 4 Threads und nativen Stufen sind es 2,6 s CPU für
  3086 Kacheln, rund 0,8 ms je Kachel.

## Schluss

- **Weniger als erwartet:** Auf einem anderen Rechner gab das an #205
  ×0,83 für rund 1 ms je Kachel. Hier sind es ×0,79 für 0,35 bis 0,8 ms.
- **Hochrechnung grosse Welt:** Mit diesen Faktoren werden aus 188 GB rund
  150 GB:
  - Basis 142,2 GB ×0,79;
  - native Stufen 44,6 GB ×0,85;
  - Pyramide 1,1 GB ×0,90.

  Ihre 3,3 Mio. Kacheln kosten 20 bis 45 CPU-Minuten mehr. Mit 24 Threads
  sind das 1 bis 2 min auf rund 95 min.
- **Entscheidung:** [0090](../entscheidungen/0090-webp-mit-quality-75.md).
