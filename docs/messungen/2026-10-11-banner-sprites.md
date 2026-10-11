---
title: Sprites der Banner
description: Was --banners für 200 Entwürfe in 4 schrägen Sätzen und oben kostet, 2000 Sprites, kalt in ein leeres --out und noch einmal mit passenden Stempeln, auf einem Thread mit niedrigster Priorität wie im Plugin; Zeit, CPU, Speicher an der Spitze und Platz, gegen die Schätzung in 0100.
date: 2026-10-11
commits: [27f9488]
code:
  - renderer/src/cli/banner.rs
  - renderer/src/render/banner.rs
---

# Sprites der Banner

`--banners` zeichnet 2000 Sprites kalt in 6,28 bis 6,47 s auf einem Thread,
rund 2,9 ms je Sprite über dem Aufruf. Mit passenden Stempeln braucht
derselbe Aufruf 0,62 bis 0,63 s und zeichnet nichts. Die Spitze liegt unter
20 MiB, auf der Platte sind es 3,89 MB, 1,9 kB je Sprite.

## Aufbau

- **Ebene** `messung:staedte` mit 200 Entwürfen: die Grundfarbe reihum aus
  den 16 Farbstoffen, je 1 bis 6 Lagen aus den Mustern in
  `blockentities.txt`, ohne Namen.
- **Sätze:** eine Wurzel mit `trees.json` aus 4 Bäumen im Look der Karte,
  `2:1` `se`, `2:1` `nw`, `4:3` `se` und `1:1` `se`, dazu `oben`. Je Entwurf
  ohne und mit Krone, zusammen 200 × 5 × 2 = 2000 Sprites.
- **Befehl,** wie das Plugin ihn ruft:

  ```bash
  heroic-map-renderer --banners staedte.json --out <out> --tiles <wurzel> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --threads 1 --low-priority
  ```

- Release-Build von `27f9488`, master mit #274 und #275, der Stand von
  v0.9.0 ohne die Nummer.
- Unter der Sperre, nach den Zeitmessungen des Researchers und von Mod 2.
  Vor jedem Lauf lief kein Spiel, und die Last lag unter 10 %.

## Ablauf

Am 11.10. von 02:57:40 bis 02:59:08 drei Wiederholungen: je ein Lauf kalt
in ein leeres `--out`, gleich danach einer in dasselbe `--out`, dessen
Stempel passen; 15 s Pause zwischen den Wiederholungen. Die Zahlen stammen
aus dem Messskript:

- Wanduhr des Prozesses;
- CPU aus `GetProcessTimes`, Kernel und User;
- Speicher an der Spitze, `PeakWorkingSetSize` aus `GetProcessMemoryInfo`
  nach dem Ende des Prozesses;
- Dateien und Bytes unter `--out` nach dem Lauf, und die letzte Zeile der
  Ausgabe.

## Ergebnis

| Lauf | Wanduhr | CPU | Spitze | neu gezeichnet |
|---|---|---|---|---|
| kalt | 6,28 bis 6,47 s | 6,03 bis 6,31 s | 0,019 GiB (19,5 bis 19,8 MiB) | 2000 Sprites |
| warm | 0,62 bis 0,63 s | 0,59 bis 0,62 s | 0,018 GiB (18,5 bis 18,6 MiB) | keins |

- **Meldung:** kalt `{"changed":["messung:staedte"],"failed":[]}`, warm
  `{"changed":[],"failed":[]}`.
- **Platz:** 2010 Dateien mit 3,89 MB, die 2000 Sprites und je Satz
  `.stempel` und `satz.json`; 1,9 kB je Sprite.

## Schluss

- **Je Sprite:** kalt weniger warm sind im Mittel 5,74 s für 2000 Sprites, also
  2,9 ms je Sprite. Ändert sich ein Entwurf, zeichnet `--banners` ihn in
  den 5 Sätzen mit und ohne Krone neu, 10 Sprites, rund 30 ms, dazu der
  Aufruf von gut 0,6 s.
- **Gegen die Schätzung in
  [0100](../entscheidungen/0100-der-renderer-zeichnet-die-banner.md):**
  - Der erste Render liegt mit 6,3 bis 6,5 s über den geschätzten unter
    5 s.
  - Ein Sprite kostet rund 2,9 ms statt unter 1 ms.
  - Der Platz ist 1,9 statt rund 1 kB je Sprite.
  - Der Aufruf ohne neues Bild liegt mit 0,6 s unter den geschätzten 1 bis
    2 s Start.
- **Arbeitsspeicher:** unter 20 MiB, auf einem Thread.
- **Neben dem Server:** Alles läuft auf einem Thread mit niedrigster
  Priorität. Auch der erste Render aller 2000 Sprites belegt einen Kern für
  gut 6 s.
