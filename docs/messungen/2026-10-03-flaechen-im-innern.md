---
title: Flächen im Innern weich, Kosten
description: Was die weiche Beleuchtung der Flächen im Innern aus #51 kostet, Karte auf der CPU und auf der Grafikkarte und Cinematic, an Stand und Fichtenwald der Testwelt, gegen master.
date: 2026-10-03
commits: [7e067e8, 39c86fb]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
---

# Flächen im Innern weich, Kosten

Messbar teurer werden mit #51 nur die Karte auf der Grafikkarte und
Cinematic am Stand, um 4 bis 5 % und um 3 %, in allen drei Runden. Die
Karte auf der CPU und der Fichtenwald bleiben innerhalb der Streuung. Die
Kacheln wiegen am Stand 0,1 bis 0,7 % mehr, im Fichtenwald mit seinem
Schnee 4,6 bis 5,7 %. Die Spitze des Speichers steigt um 0,3 bis 5,7 %,
mit der Grafikkarte am meisten.

## Aufbau

- **Stände,** Release-Build, je aus eigenem Worktree:
  - **master:** `7e067e8`;
  - **innen:** `39c86fb`, master mit #51.
- **Die Testwelt,** 2:1 bei scale 32 aus `se`, `--native-levels 3`,
  Pyramide, 24 Threads; dieselben Ausschnitte wie in
  [Bits „frei zur Sonne“ am Renderer](2026-10-03-bits-am-renderer.md):
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln.
- **Je Ausschnitt drei Arten:** die Karte mit `--gpu off`, die Karte mit
  `--gpu on` und Cinematic mit `LOOK` und `--gpu off`.
- **Reihe:** die Stände im Wechsel, drei Runden, die Reihenfolge in Runde 2
  umgekehrt, 36 Läufe. Das Bild ändert sich gewollt, ein Vergleich „gleich“
  entfällt.
- **Nach jedem Lauf** wird sein Baum gelöscht und dann 15 s gewartet, siehe
  [Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md). Vor
  jedem Lauf liegt die Last unter 10 %, die Sperrdatei während der ganzen
  Reihe.

## Ablauf

Am 03.10. ab 17:52, mit dem Messskript zu #51 auf dem zu #73: Dauer des
Prozesses, Zeiten der Basis, der nativen Stufen und der Pyramide aus der
Ausgabe des Laufs, Spitze des Arbeitsspeichers aus
`GetProcessMemoryInfo`, Bytes aus den Dateigrössen.

## Ergebnis

Je Ausschnitt und Art die Dauer des Prozesses in den drei Runden, der beste
Lauf je Stand und der Unterschied der besten. Vor jedem Lauf lag die Last
unter 10 %, höchstens 9,8 %; oft wartete das Skript, bis sie fiel, die
Reihe dauerte deshalb 39 statt 25 min.

| Ausschnitt | Art | master, s | innen, s | je Runde | bester Lauf |
|---|---|---|---|---|---|
| Stand | Karte, CPU | 18,18 / 14,44 / 14,50 | 14,49 / 17,22 / 14,94 | −20,3 / +19,3 / +3,0 % | 14,44 → 14,49 s, +0,3 % |
| Stand | Karte, GPU | 13,64 / 13,30 / 13,08 | 13,83 / 13,73 / 14,19 | +1,4 / +3,2 / +8,5 % | 13,08 → 13,73 s, +5,0 % |
| Stand | Cinematic | 45,77 / 46,18 / 45,32 | 47,68 / 47,16 / 46,54 | +4,2 / +2,1 / +2,7 % | 45,32 → 46,54 s, +2,7 % |
| Fichtenwald | Karte, CPU | 8,58 / 6,48 / 6,21 | 6,94 / 6,42 / 6,58 | −19,1 / −0,9 / +6,0 % | 6,21 → 6,42 s, +3,4 % |
| Fichtenwald | Karte, GPU | 6,40 / 6,51 / 6,57 | 6,42 / 6,44 / 6,17 | +0,3 / −1,1 / −6,1 % | 6,40 → 6,17 s, −3,6 % |
| Fichtenwald | Cinematic | 13,11 / 13,20 / 14,20 | 16,08 / 13,19 / 13,91 | +22,7 / −0,1 / −2,0 % | 13,11 → 13,19 s, +0,6 % |

Die Kacheln, je Ausschnitt in jeder Runde gleich viele, 8564 am Stand und
2144 im Fichtenwald, und die Spitze des Arbeitsspeichers, die höchste der
drei Runden:

| Ausschnitt | Art | Bytes master | Bytes innen | Spitze master | Spitze innen |
|---|---|---|---|---|---|
| Stand | Karte, CPU | 586,7 MB | 591,0 MB, +0,7 % | 2,36 GiB | 2,40 GiB, +1,6 % |
| Stand | Karte, GPU | 586,7 MB | 591,0 MB, +0,7 % | 3,21 GiB | 3,39 GiB, +5,5 % |
| Stand | Cinematic | 574,2 MB | 574,7 MB, +0,1 % | 3,36 GiB | 3,42 GiB, +1,7 % |
| Fichtenwald | Karte, CPU | 202,3 MB | 213,9 MB, +5,7 % | 2,01 GiB | 2,07 GiB, +3,0 % |
| Fichtenwald | Karte, GPU | 202,3 MB | 213,9 MB, +5,7 % | 2,54 GiB | 2,69 GiB, +5,7 % |
| Fichtenwald | Cinematic | 212,8 MB | 222,7 MB, +4,6 % | 2,73 GiB | 2,73 GiB, +0,3 % |

## Schluss

- **Messbar** sind nur die Karte auf der Grafikkarte am Stand, +1 bis +9 %
  je Runde, und Cinematic am Stand, +2 bis +4 %. Auf der Grafikkarte trägt
  jede Instanz 104 statt 68 Bytes, und `ecken_at` rechnet mehr Plätze.
- **Innerhalb der Streuung** bleiben die Karte auf der CPU und der
  Fichtenwald: Einzelne Runden reissen um ±20 % aus, die besten Läufe
  liegen bei −3,6 bis +3,4 %.
- **Die Kacheln** wiegen mehr, wo es Flächen im Innern gibt: Im
  Fichtenwald liegt Schnee, die Verläufe auf den Schneedecken kodiert WebP
  mit 5 bis 6 % mehr Bytes.
- **Der Speicher** steigt mit der Grafikkarte um 5,5 bis 5,7 %, mit den
  grösseren Instanzen; sonst um 0,3 bis 3 %.
