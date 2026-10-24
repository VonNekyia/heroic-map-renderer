---
title: Relief von oben
description: Was --relief slope und shadows auf einer Stadt der grossen Welt bei top-north scale 4 an Zeit, Arbeitsspeicher und Platz kostet, gegen die Karte ohne Relief, mit 16 Threads und mit einem, dazu die Kacheln je geändertem Chunk.
date: 2026-10-24
commits: []
code:
  - renderer/src/render/metatile/relief.rs
---

# Relief von oben

Mit den Höhen aus dem Cache der Kachel kostet das Relief beim ersten Render
nichts Messbares. Auf einem Thread kostet es je Kachel 2 % mit `slope` und
5 % mit `shadows`. Die Kacheln werden 13 bis 19 % grösser. Bei Updates
trifft `shadows` 36 % mehr Kacheln.

## Aufbau

- **Welt:** die Kopie der grossen Welt, 26.2, um eine ihrer Städte.
- **Kamera:** `top-north`, Richtung `s`, scale 4.
- **Bau:** ohne Grafikkarte (`--gpu off`), ohne native Stufen,
  `--low-priority`, Release-Build.
- **Erster Render:** 3072 × 3072 Blöcke um die Stadt, 2401 Basiskacheln,
  16 Threads.
- **Wie live:** 1024 × 1024 Blöcke der Innenstadt, 289 Basiskacheln,
  1 Thread.
- **Stand:** der Versuch, aus dem das Relief stammt. Werte, Rechnung und die
  Höhen aus dem Cache der Kachel sind dieselben wie im Code. Er legte das
  Relief in `render_area_with` auf, der Code danach in `zeichne`; ohne
  Grafikkarte ist das derselbe Weg.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel.
- **Reihenfolge:** A ohne Relief, B `slope`, C `shadows`, im Wechsel, je
  zwei Runden. Jeder Lauf schrieb in einen frischen Baum, der danach
  gelöscht wurde, mit 5 s Pause.
- **Zeit und Kacheln je Sekunde** aus der Ausgabe des Laufs, der Zeile
  `… MB in … s`.
- **Die Spitze des Arbeitsspeichers** aus `GetProcessMemoryInfo` des
  beendeten Prozesses.
- **Die Bytes** aus den Dateigrössen des Baums.
- **Gestört:** In der zweiten Runde mit einem Thread lief kurz ESLint
  daneben. Sie zählt nicht.

## Ergebnis

**Erster Render,** 16 Threads, 2401 Kacheln:

| Stand | Kacheln/s, Runde 0 / 1 | Spitze Working Set | je Basiskachel | Baum |
|---|---|---|---|---|
| A ohne | 132 / 154 | 0,87–0,92 GiB | 26,9 kB | 95,6 MB |
| B `slope` | 130 / 165 | 0,86–0,89 GiB | 30,5 kB | 108,1 MB |
| C `shadows` | 149 / 152 | 0,87–0,89 GiB | 30,9 kB | 109,6 MB |

**Wie live,** 1 Thread, 289 Kacheln der Innenstadt:

| Stand | Runde 0 | Runde 1, gestört | je Basiskachel |
|---|---|---|---|
| A ohne | 13,5 s, 46,7 ms je Kachel | 16,5 s | 37,8 kB |
| B `slope` | 13,8 s, 47,8 ms | 17,2 s | 44,3 kB |
| C `shadows` | 14,2 s, 49,1 ms | 26,1 s | 44,9 kB |

- **Spitze Working Set** mit einem Thread: 0,10–0,11 GiB in allen drei.
- **Kacheln je geändertem Chunk,** gerechnet für einen einzelnen Chunk bei
  scale 4, eine Kachel 64 Blöcke breit, ohne die Reserve für überstehende
  Sprites:
  - ohne Relief und mit `slope` im Mittel 2,25, denn ein Update zeichnet
    ohnehin 16 Blöcke um den Chunk;
  - mit `shadows` 3,06, denn es reicht 32 Blöcke nach Südosten.

## Schluss

- **Mit 16 Threads** streut die Zeit um mehr als 10 %, mehr als jeder
  Unterschied.
- **Auf einem Thread** kostet `slope` 2 % und `shadows` 5 % je Kachel. Mit den
  Kacheln je Chunk kostet ein Update mit `slope` rund 3 % mehr, mit `shadows`
  rund 43 % mehr.
- **Der Platz** wächst um 13 bis 19 %, mehr, wo das Gelände mehr Relief hat.
- **Der Arbeitsspeicher** bleibt gleich.
