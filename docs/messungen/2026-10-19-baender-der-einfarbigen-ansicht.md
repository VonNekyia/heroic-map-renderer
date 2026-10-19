---
title: Bänder der einfarbigen Ansicht
description: Was die Höhe der Bänder von --flat an der Spitze des Arbeitsspeichers ausmacht, über die ganze Testwelt mit 64, 32 und 16 Pixeln, gegen die Karte in top-north bei scale 4; die Kacheln bleiben Byte für Byte gleich, die Zeit ist hier nicht gemessen.
date: 2026-10-19
commits: [4cb7a89]
code:
  - renderer/src/render/metatile.rs
---

# Bänder der einfarbigen Ansicht

Mit Bändern von 32 statt 64 Pixeln sinkt die Spitze von `--flat` über die
ganze Testwelt von 1,52 bis 1,54 auf 1,22 bis 1,23 GiB, fast auf die 1,13
bis 1,15 GiB der Karte in `top-north` bei scale 4. Mit 16 Pixeln sind es
1,05 bis 1,08 GiB. Jede Kachel bleibt Byte für Byte gleich. Die Zeit misst
diese Reihe nicht.

## Aufbau

- **Stand:** master `4cb7a89`, Release-Build. Für 32 und 16 Pixel nur
  `FLACH_BAND` in `renderer/src/render/metatile.rs` geändert, sonst
  dasselbe Binär.
- **Welt:** die ganze Testwelt wie am 10.10., `--gpu off`, Threads nach
  Vorgabe, jeder Lauf frisch in ein leeres Verzeichnis:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --scale 4 --camera top-north --tiles <baum>
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --flat --tiles <baum>
  ```

## Ablauf

- **Folge:** am 19.10. von 12:50 bis 13:07, Karte und `--flat` mit 64
  Pixeln im Wechsel, je zweimal, dann zweimal 32 und zweimal 16, vor jedem
  Lauf 15 s Pause.
- **Ruhe:** keine. Speicher an der Spitze ist keine Zeitmessung; andere
  Sitzungen arbeiteten am Rechner. Die Wanduhr steht nur zur Orientierung.
- **Quelle:** das Messskript liest nach dem Ende jedes Laufs
  `PeakWorkingSetSize` des Prozesses über `GetProcessMemoryInfo`, die
  Bytes der Kacheln vergleicht es als SHA-256 je Datei.

## Ergebnis

| Lauf | Spitze | Wanduhr, nur zur Orientierung |
|---|---|---|
| Karte, `top-north` 4 | 1,13 / 1,15 GiB | 168 / 107 s |
| `--flat`, 64 Pixel | 1,54 / 1,52 GiB | 45 / 48 s |
| `--flat`, 32 Pixel | 1,23 / 1,22 GiB | 49 / 48 s |
| `--flat`, 16 Pixel | 1,08 / 1,05 GiB | 51 / 52 s |

- **Bytes:** Bei 32 und 16 Pixeln ist jede der 1670 Kacheln dieselbe wie
  bei 64. Anders sind nur `map.json` und Dateien unter `pixel/`, und die
  sind es auch zwischen zwei Läufen desselben Binärs.
- **Je Thread:** Mit den Threads nach Vorgabe, 24, spart ein halbes Band
  rund 13 MB je Thread.

## Schluss

- **Ursache:** Den Rest über der Karte macht die Höhe des Bands. Jeder
  Thread hält das laufende und das vorige Band samt Rand im Cache.
- **Mit 32 Pixeln** liegt `--flat` an der Spitze beim 1,07-Fachen der Karte
  statt beim 1,34-Fachen.
- **Offen ist die Zeit,** für den ersten Render und für ein Update: Bei 16
  Pixeln lag die Wanduhr in beiden Läufen etwas höher, ohne Ruhe am
  Rechner. Das klärt eine Reihe unter der Sperre.
