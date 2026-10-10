---
title: Einfarbige Ansicht
description: Was --flat über die ganze Testwelt an Zeit, Platz und Arbeitsspeicher kostet, gegen die Karte mit Texturen in top-north bei scale 4, je drei Läufe im Wechsel.
date: 2026-10-10
commits: [e931896]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# Einfarbige Ansicht

Über die ganze Testwelt braucht `--flat` 35 % weniger Zeit und 91 % weniger
Platz als die Karte in `top-north` bei scale 4: 57 bis 59 s statt 87 bis
91 s, 91,5 MB statt 1055,0 MB. An der Spitze braucht es aber 3,3-mal so
viel Arbeitsspeicher, 3,8 statt 1,2 GiB.

## Aufbau

- **Stand:** `e931896`, Release-Build, mit #245 darunter.
- **Welt:** die ganze Testwelt, 316 223 Chunks, `--gpu off`, Threads nach
  Vorgabe, jeder Lauf frisch in ein leeres Verzeichnis:

  ```bash
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --scale 4 --camera top-north --tiles <baum>
  heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --flat --tiles <baum>
  ```

## Ablauf

- **Folge:** Karte, flat, im Wechsel, je drei Läufe, am 10.10. von 10:20
  bis 10:29.
- **Ruhe:** vor jedem Lauf die Last unter 10 % und 15 s Pause; die Reihe
  unter der Sperre, ohne laufenden Minecraft-Client.
- **Quelle:** das Messskript misst Wanduhr, CPU-Zeit und die Spitze des
  Arbeitsspeichers des Prozesses und zählt die Bytes je Stufe aus den
  Dateien.

## Ergebnis

| Ansicht | Wanduhr | CPU | Spitze | Kacheln | Platz |
|---|---|---|---|---|---|
| Karte, `top-north` 4 | 87,42 s, 90,68 s, 90,91 s | 1767 s bis 1811 s | 1,14 bis 1,19 GiB | 22 037 | 1055,0 MB |
| `--flat` | 57,34 s, 59,01 s, 57,84 s | 1160 s bis 1175 s | 3,82 bis 3,85 GiB | 1670 | 91,5 MB |

- **Zeit:** −35 % an der Wanduhr im Mittel, −35 % CPU. Die Spannen trennen
  sich.
- **Platz:** ×0,087. Die Karte hat die Stufen 0 bis 8, die einfarbige
  Ansicht 0 bis 6: Mit einem Pixel je Block deckt eine Kachel 16-mal so
  viele Blöcke. In derselben Zeile der Tabelle unten deckt eine Kachel
  gleich viele Blöcke.
- **Bytes:** In jedem Lauf einer Ansicht Byte für Byte gleich.

### Grösse je Stufe

Bytes dezimal, aus den Dateien.

| Stufe | Karte | `--flat` |
|---|---|---|
| 8 | 714,51 MB | – |
| 7 | 233,25 MB | – |
| 6 | 73,58 MB | 60,18 MB |
| 5 | 23,50 MB | 21,61 MB |
| 4 | 7,14 MB | 6,79 MB |
| 3 | 2,12 MB | 2,04 MB |
| 2 | 0,63 MB | 0,62 MB |
| 1 | 0,21 MB | 0,21 MB |
| 0 | 0,07 MB | 0,07 MB |
| zusammen | 1055,00 MB | 91,53 MB |

## Schluss

- **Zeit:** Die Ansicht zeichnet 16-mal weniger Pixel, ist aber nur ein
  Drittel schneller. Lesen und Dekodieren der Chunks bleiben gleich.
- **Platz:** ×0,087 für den ganzen Baum. Auf Stufe 6, wo beide gleich
  viele Blöcke je Kachel zeigen, ×0,82.
- **Arbeitsspeicher:** mehr als das Dreifache an der Spitze. Warum, ist
  offen; #247 klärt es, bevor die Ansicht als leicht gilt.
