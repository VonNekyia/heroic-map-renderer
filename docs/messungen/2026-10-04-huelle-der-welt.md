---
title: Hülle der Welt schneller
description: Was die Hülle der fertig erzeugten Chunks aus #117 einen Lauf an der Testwelt kostet, vor und nach #123, an einem kleinen Ausschnitt ohne --area, damit der Start ins Gewicht fällt.
date: 2026-10-04
commits: [4d1abbc, 06c7539]
code:
  - renderer/src/world/mod.rs
---

# Hülle der Welt schneller

Mit #123 braucht ein kleiner Lauf an der Testwelt im Median 0,70 s statt
1,12 s, 0,42 s oder 37 % weniger. Das Rechteck in `map.json` ist in allen
zehn Läufen dasselbe. Die Hülle (`World::huelle`) rechnet jeder Lauf ohne
`--area` im Start, siehe [`map.json`](../benutzung/map-json.md).

## Aufbau

- **Welt:** die Testwelt, 26.2.
- **Stände:** master `4d1abbc` und #123 `06c7539`, je ein Release-Build,
  alle 24 Threads.
- **Lauf:** die Karte, ein kleiner Ausschnitt ohne `--area`, damit der
  Start den Lauf bestimmt:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <leer> --center -64 416 --size 2048 --scale 32 --gpu off
  ```

- **Reihe:** fünf Runden, je Runde beide Stände, in Runde 3 umgekehrt. Jeder
  Lauf in einen leeren Ordner, danach gelöscht und 15 s Pause.
- **Ruhe:** unter der Sperrdatei; die Last vor jedem Lauf zwischen 0,8 und
  7,6 %.
- **Gemessen:** die Wanduhr des ganzen Prozesses.

## Ergebnis

| Runde | master | #123 |
|---|---|---|
| 1 | 2,00 s | 0,74 s |
| 2 | 1,13 s | 0,72 s |
| 3 | 1,12 s | 0,70 s |
| 4 | 1,12 s | 0,70 s |
| 5 | 1,12 s | 0,70 s |
| Median | 1,12 s | 0,70 s |

- **Runde 1** war für master der erste Lauf der Reihe, vermutlich mit
  kaltem Cache des Systems; der Median lässt ihn aussen vor. Ab Runde 2
  streut jeder Stand um höchstens 0,03 s.
- **Rechteck:** in jedem Lauf `[-13072, -10208, 11104, 10496]`.

## Schluss

- #123 nimmt jedem Lauf ohne `--area` an der Testwelt 0,42 s. Der Researcher
  hatte für die Hülle 0,7 s je Lauf gefunden. Wie sich die übrigen 0,70 s
  auf Hülle und Lauf verteilen, ist nicht gemessen.
- Mit `--area` rechnet ein Lauf keine Hülle. Einen Vollrender betrifft das
  nur einmal im Start.
