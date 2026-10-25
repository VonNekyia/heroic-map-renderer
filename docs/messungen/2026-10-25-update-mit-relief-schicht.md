---
title: Update mit der Schicht des Reliefs
description: Was die Schicht des Reliefs samt Rest ein Update mit einem Thread und den vollen Lauf kostet, gegen eingebackenes slope ohne Schicht, und wie gross Schicht und Rest auf der ganzen Testwelt bei top-north scale 4 sind.
date: 2026-10-25
commits: [c94548a, 6a4bde1]
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/cli.rs
---

# Update mit der Schicht des Reliefs

Mit Schicht und Rest dauert ein Update mit 16 geänderten Chunks auf einem
Thread 5,81 statt 5,28 s, +10 %. Die Schicht kostet 88,0 MB, 6,7 % des
Baums mit `slope`. Der Rest kostet 60,3 MB, 4,6 % des Baums; ihn haben
9 660 der 16 127 Basiskacheln.

## Aufbau

- **Welt:** eine Kopie der Testwelt, 26.2.
- **Kamera:** `top-north`, Richtung `s`, scale 4, `--relief slope`,
  `--gpu auto`, `--low-priority`, ohne native Stufen, Release-Build.
- **Stände:**
  - A: master `c94548a`, `slope` eingebacken, ohne Schicht.
  - B: `6a4bde1`, dazu die Schicht in `relief/` und der Rest in
    `relief/rest/`.
- **Läufe:** je ein voller Lauf mit allen Threads in einen eigenen Baum,
  dann drei Änderungen an denselben 16 Chunks, je in einer anderen Region:
  - der oberste feste Block wird Gold, dann wieder wie vorher, dann wieder
    Gold;
  - nach jeder Änderung ein Update mit `--threads 1` auf beiden Bäumen, in
    der Reihenfolge A B, B A, A B.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel. Kein
  Server des Spiegels lief.
- **Lastprüfung vor jedem Lauf:**
  - Die Auslastung der CPU, dreimal im Abstand von 2 s, dazu die Prozesse
    mit der meisten CPU-Zeit in diesen Sekunden, stand im Protokoll, bevor
    der Lauf begann.
  - Über 10 % hätte der Lauf gewartet. Sie lag vor jedem Lauf zwischen 1
    und 5 %, vor dem ersten bei 0 bis 3 %.
- **Zeit** von aussen gemessen. **Spitze** des privaten Speichers und des
  Working Sets aus `GetProcessMemoryInfo` des beendeten Prozesses.
- **Bytes** aus den Dateigrössen des Baums B nach seinem vollen Lauf.

## Ergebnis

**Updates,** 16 Chunks, ein Thread:

| Stand | Zeit | Mittel | Kacheln im Gebiet | geschrieben, gleich | Spitze privat |
|---|---|---|---|---|---|
| A | 5,42 / 5,27 / 5,15 s | 5,28 s | 64 | 16, 46 | 0,45–0,46 GiB |
| B | 5,87 / 5,78 / 5,79 s | 5,81 s | 74 | 16, 56 | 0,48 GiB |

- **Die zehn Kacheln mehr** liegen im Ring des Schattens: B zeichnet auch
  mit `slope` so weit, wie der Schatten fällt. Ihre Pixel blieben gleich,
  sie wurden nicht kodiert.
- **Die Pyramide der Schicht** baute in diesen Updates keine Kachel neu:
  Gold ersetzt den obersten Block, die Höhen bleiben gleich. Die des Rests
  baute 31 bis 33 Kacheln neu, 0,1 MB; Gold ist hell und wird abgeschnitten.

**Voller Lauf,** alle Threads, je einer:

| Stand | Zeit | Spitze privat | Spitze Working Set |
|---|---|---|---|
| A | 90,3 s | 3,64 GiB | 2,66 GiB |
| B | 106,1 s | 3,89 GiB | 2,80 GiB |

- **Die Pyramiden** von Schicht und Rest bauten in B 3,0 und 1,8 s, von
  der Platte.

**Grössen** des Baums B:

| Teil | Dateien | Bytes | gegen den Baum |
|---|---|---|---|
| Baum mit `slope` | 22 037 | 1 313,5 MB | |
| Schicht `relief/` | 22 037 | 88,0 MB | 6,7 % |
| Rest `relief/rest/` | 14 459 | 60,3 MB | 4,6 % |

- **Rest gegen Schicht:** 69 %.
- **Basiskacheln mit Rest:** 9 660 von 16 127, 60 %. Abgeschnitten werden
  2,4 % der Pixel, gezählt in
  [0132](../entscheidungen/0132-relief-als-schicht.md), aber über die
  meisten Kacheln verstreut.

## Schluss

- **Ein Update** kostet mit Schicht und Rest auf einem Thread 10 % mehr,
  0,5 s bei 16 Chunks, und rund 0,02 GiB mehr Speicher. Das liegt über der
  Streuung von A, 0,27 s, aber unter den 15 %, ab denen nur die Schicht im
  Ring neu gerechnet würde.
- **Ein voller Lauf** dauerte in je einem Lauf 17 % länger und braucht
  0,25 GiB mehr an der Spitze.
- **Der Rest** ist mit 4,6 % des Baums grösser als gedacht; er steckt in
  60 % der Basiskacheln.
