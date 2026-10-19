---
title: Zeit der Bänder der einfarbigen Ansicht
description: Wie lange --flat mit Bändern von 64, 32 und 16 Pixeln für den ersten Render über die ganze Testwelt und für ein Update nach 15 Chunks braucht, unter der Sperre, je drei Läufe im Wechsel, mit der Spitze des Arbeitsspeichers; danach gewählt 16 Pixel.
date: 2026-10-19
commits: [4cb7a89]
code:
  - renderer/src/render/metatile.rs
---

# Zeit der Bänder der einfarbigen Ansicht

Ein Update braucht mit Bändern von 64, 32 und 16 Pixeln gleich lang, rund
3,6 s. Der erste Render über die ganze Testwelt ist mit 64 und 32 Pixeln
gleich schnell, mit 16 Pixeln rund 4 % langsamer. Die Spitze des
Arbeitsspeichers liegt mit 16 Pixeln 24 bis 31 % unter der mit 64. Gewählt
sind 16 Pixel.

## Aufbau

- **Stand:** dieselben drei Binärs wie in
  [2026-10-19, Bänder der einfarbigen Ansicht](2026-10-19-baender-der-einfarbigen-ansicht.md),
  Byte für Byte: master `4cb7a89`, Release-Build, für 32 und 16 Pixel nur
  `FLACH_BAND` in `renderer/src/render/metatile.rs` geändert.
- **Welt:** eine Kopie der ganzen Testwelt, `--gpu off`, Threads nach
  Vorgabe:

  ```bash
  heroic-map-renderer --world ./welt --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --flat --tiles <baum>
  ```

- **A, erster Render:** jeder Lauf frisch in ein leeres Verzeichnis.
- **B, Update:** In der Kopie fehlt in 16 über die Welt verteilten Regionen
  der Chunk in der Mitte, wie Teil D in
  [2026-10-04, Updates](2026-10-04-updates.md). Dann `--update` in
  den Baum des letzten Laufs aus A; vor jedem Lauf kommen `stand.bin` und
  `map.json` des Baums zurück, so sieht jeder Lauf dieselben Änderungen.
  Der Renderer meldet 15 geänderte Chunks und 60 Kacheln im Gebiet.

## Ablauf

- **Folge:** am 19.10. von 15:43 bis 15:59, drei Runden, die Stände im
  Wechsel 64, 32, 16, in der zweiten Runde umgekehrt; erst alle Läufe von
  A, dann die von B.
- **Ruhe:** unter der Sperre, nach 10 min Ruhe, ohne Spiel und ohne Server
  am Rechner. Vor jedem Lauf wartet das Messskript, bis die Last unter 10 %
  liegt, dann 15 s.
- **Quelle:** das Messskript; die Zeit als Wanduhr von Start bis Ende des
  Prozesses, die Spitze als `PeakWorkingSetSize` über
  `GetProcessMemoryInfo` nach seinem Ende.

## Ergebnis

**A, erster Render,** je drei Läufe:

| | 64 Pixel | 32 Pixel | 16 Pixel |
|---|---|---|---|
| Median | 42,56 s | 42,78 s | 44,44 s |
| Spanne | 42,09–44,27 s | 39,48–42,88 s | 43,64–45,61 s |
| Spitze | 1,53–1,57 GiB | 1,21–1,24 GiB | 1,06–1,14 GiB |

**B, Update nach 15 Chunks,** je drei Läufe:

| | 64 Pixel | 32 Pixel | 16 Pixel |
|---|---|---|---|
| Median | 3,70 s | 3,58 s | 3,56 s |
| Spanne | 3,50–3,74 s | 3,33–3,64 s | 3,30–3,64 s |
| Spitze | 1,49–1,56 GiB | 1,23–1,25 GiB | 1,12–1,16 GiB |

- **Bytes:** Die 1670 Kacheln der ersten Runde von A sind bei 32 und 16
  Pixeln Byte für Byte dieselben wie bei 64.

## Schluss

- **Spitze,** je im Median: 32 gegen 64 Pixel −20 % in A und −17 % in B;
  16 gegen 32 noch einmal −14 % und −9 %; 16 gegen 64 −31 % und −24 %.
- **32 gegen 64:** gleiche Zeit, die Spannen überlappen.
- **16 gegen 32:** beim Update gleich; beim ersten Render im Median 4 %
  mehr, und seine Spanne liegt ganz über der von 32.
- **Gewählt: 16 Pixel,** nach Regel 26 in `AGENTS.md`: Das Live-Rendern
  wiegt schwerer und leidet nicht, der Arbeitsspeicher ist knapp, der erste
  Render wiegt weniger.
