---
title: Boden je Block
description: Was der Boden ohne Laub je Block statt je 4×4 an Platz und an Arbeitsspeicher an der Spitze von --heights kostet, auf der Testwelt mit allen Kernen und mit einem Thread, ungepackt und gepackt im Vorlauf; dazu die Hochrechnung auf die grosse Welt.
date: 2026-10-11
commits: [cc83ff2, KOPF]
code:
  - renderer/src/render/heights.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# Boden je Block

Der Boden je Block belegt auf der Testwelt 11,19 MB statt 1,40 MB je 4×4,
hochgerechnet auf die grosse Welt rund 85 MB. Weil der Vorlauf jede Region
gleich packt, liegt die Spitze von `--heights` mit einem Thread im Mittel
6,5 MiB unter der von vorher, 0,073 statt 0,079 GiB. Mit allen Kernen liegt
sie im Mittel 24,5 MiB darüber, 0,150 statt 0,126 GiB: Je Thread hält der
Vorlauf eine Region ungepackt, nicht je Region der Welt.

## Aufbau

- **Welt:** die Testwelt, 383 Regionen. Der Baum stammt aus einem Export
  über einen Ausschnitt von einem Pixel, in eine eigene Wurzel je Stand:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --tiles <wurzel> --center -64 416 --size 1
  heroic-map-renderer --world ./world --heights <wurzel>/<baum>
  heroic-map-renderer --world ./world --heights <wurzel>/<baum> --threads 1
  ```

- **Stände,** alle als Release-Build:
  - **vorher:** `cc83ff2`, der Boden je 4×4, der Vorlauf hält Höhen und
    Boden ungepackt, 32 KiB je Region und Datei;
  - **je Block, Puffer:** der Boden je Block, der Vorlauf packt jede Region,
    sobald er sie gelesen hat; `Heights::encode` legt dafür erst einen
    Puffer mit allen Werten an, je Block 512 KiB;
  - **je Block, Stücke:** wie davor, aber `encode` packt in Stücken von
    8 KiB. Das ist der Stand dieser PR, `KOPF`.
- **Threads:** ohne `--threads`, also alle Kerne, und mit `--threads 1`.
  Mit einem Thread läuft auch der Vorlauf eines Updates im Plugin, das
  `renderer.threads` von 1 nimmt.
- Keine Zeitmessung, ohne Sperre; vor jedem Lauf wartete das Skript, bis
  die Last unter 10 % lag.

## Ablauf

Am 11.10. von 00:47 bis 01:02, drei Runden, in jeder je Thread-Zahl die drei
Stände, in den Runden 1 und 3 in der Reihenfolge oben, in Runde 2
umgekehrt. Die Zahlen stammen

- für den Speicher an der Spitze aus dem Messskript:
  `PeakWorkingSetSize` aus `GetProcessMemoryInfo` nach dem Ende des
  Prozesses;
- für den Platz aus den Dateigrössen unter `heights/` und `ground/` nach
  dem Lauf, dezimal.

`--heights` liest die ganze Welt und schreibt jede Region, ob schon Dateien
liegen oder nicht; die Läufe schrieben daher über die Dateien des Laufs
davor, ohne sie vorher zu löschen.

## Ergebnis

Speicher an der Spitze, die Spanne über die drei Runden und in Klammern
das Mittel:

| Stand | alle Kerne | `--threads 1` |
|---|---|---|
| vorher | 0,123 bis 0,128 GiB (128,8 MiB) | 0,077 bis 0,081 GiB (81,1 MiB) |
| je Block, Puffer | 0,164 bis 0,168 GiB (169,5 MiB) | 0,074 bis 0,075 GiB (76,0 MiB) |
| je Block, Stücke | 0,140 bis 0,155 GiB (153,3 MiB) | 0,072 bis 0,073 GiB (74,6 MiB) |

Platz nach jedem Lauf, in jeder Runde gleich:

| Stand | `heights/` | `ground/` | `ground/` je Datei |
|---|---|---|---|
| vorher | 1,76 MB | 1,40 MB | 3,7 kB |
| je Block | 1,76 MB | 11,19 MB | 29,2 kB |

383 Dateien je Ordner. Entpackt sind die Höhen in allen Ständen dieselben
Werte, der Boden je Block mit Puffer und mit Stücken auch.

## Schluss

- **Platz:** je Block achtmal so viel wie je 4×4, nicht sechzehnmal; zlib
  packt benachbarte gleiche Werte. Auf die grosse Welt hochgerechnet mit
  dem Verhältnis der Höhen aus
  [2026-09-28, Höhen](2026-09-28-hoehen.md), 13,79 MB zu 1,83 MB, sind das
  rund 85 MB. Vor der Wahl waren 220 bis 250 MB geschätzt.
- **Arbeitsspeicher, gehalten:** Der Vorlauf hält je Region nur noch die
  gepackten Bytes, Höhen und Boden zusammen auf der Testwelt rund 13 MB
  statt 24 MiB ungepackt. Auf der grossen Welt wären es hochgerechnet rund
  100 MB statt 157 MiB je 4×4 ungepackt, und je Block ungepackt 1,23 GiB.
- **Arbeitsspeicher je Thread:** Jeder Thread des Vorlaufs hält die Region,
  die er gerade liest, ungepackt, je Block 512 KiB, dazu den Zustand von
  zlib beim Packen. Das wächst mit den Threads, nicht mit der Welt. Ein
  Puffer für alle Werte in `encode` kostete je Thread weitere 512 KiB; die
  Stücke von 8 KiB sparen sie, mit allen Kernen im Mittel 16 MiB.
- **Nicht gemessen:** die Zeit. Das Packen je Region kommt in den Vorlauf;
  eine Zeitmessung gehört in die Nacht.
