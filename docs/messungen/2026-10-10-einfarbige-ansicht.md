---
title: Einfarbige Ansicht
description: Was --flat über die ganze Testwelt an Zeit, Platz und Arbeitsspeicher kostet, gegen die Karte mit Texturen in top-north bei scale 4, je drei Läufe im Wechsel.
date: 2026-10-10
commits: [e931896, 812197f]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# Einfarbige Ansicht

Über die ganze Testwelt braucht `--flat` 35 % weniger Zeit und 91 % weniger
Platz als die Karte in `top-north` bei scale 4: 57 bis 59 s statt 87 bis
91 s, 91,5 MB statt 1055,0 MB. An der Spitze brauchte es zuerst 3,3-mal so
viel Arbeitsspeicher, 3,8 statt 1,2 GiB. In Bändern von vier Zeilen Chunks
sind es 1,55 GiB, rund 1,3-mal die Karte, bei gleichen Bytes. Die −35 %
sind ohne Bänder gemessen; mit Bändern lässt sich kein Unterschied zeigen.
Lesen und Licht sind 61 % der CPU von `--flat`.

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

| Ansicht | Wanduhr | CPU | Spitze | Kacheln, alle Stufen | Platz |
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

## Der Cache

Am selben Tag von 10:49 bis 11:17, mit Zählern auf Zeit, die nur für diese
Läufe in den Quellen lagen. Je ein Lauf, Bytes in jedem wie oben.

| Lauf | Chunks je Cache, höchstens | Ladungen | Licht gerechnet | Spitze | Wanduhr |
|---|---|---|---|---|---|
| Karte, `top-north` 4 | 310 | 526 569 | 373 513 | 1,14 GiB | 86,43 s |
| `--flat` | 778 | 374 443 | 281 415 | 3,81 GiB | 55,74 s |
| `--flat`, die alte Kachel nicht behalten | 400 | 453 816 | 315 041 | 2,45 GiB | 61,81 s |
| `--flat` in Bändern (`812197f`) | 412 | 374 406 | 281 412 | 1,56 GiB | 60,66 s |

- **Ursache:** Bei scale 1 deckt eine Kachel 16 × 16 Chunks, samt Rand und
  Licht rund 400. Der Cache behielt die der letzten Kachel mit, obwohl die
  nächste nur eine Zeile davon teilt.
- **Die alte Kachel nicht behalten** senkt die Spitze nur auf 2,45 GiB und
  lädt die geteilte Zeile neu: 21 % mehr Ladungen.
- **In Bändern** von 64 Pixeln beginnt jedes Band im Cache wie eine Kachel;
  Ladungen und Licht bleiben wie ohne. Ohne Zähler 1,55 und 1,56 GiB an der
  Spitze.
- **Ladungen je Chunk:** die Karte 1,67, `--flat` 1,18.

### Bänder im Wechsel

Von 11:21 bis 11:33, ohne Zähler, Bytes in jedem Lauf wie oben. Die Last
vor den Läufen lag bei 3 bis 9 %.

| Stand | Wanduhr | CPU | Spitze |
|---|---|---|---|
| ohne Bänder, `e931896` | 66,49 s, 63,34 s, 64,37 s | 1199 s, 1188 s, 1192 s | 3,73 bis 3,82 GiB |
| mit Bändern, `812197f` | 71,02 s, 64,43 s, 79,96 s | 1197 s, 1197 s, 1276 s | 1,52 bis 1,54 GiB |

- **Speicher:** −60 % an der Spitze.
- **Zeit:** In zwei Läufen gleich viel CPU, im dritten 7 % mehr. Ein
  Unterschied ist so in keiner Richtung belegt; die Läufe streuen hier mehr
  als am Vormittag.

### CPU je Phase

Aus den Läufen mit Zählern ohne Bänder, Summe über alle Threads. Gezählt
sind Lesen und Dekodieren je geladenem Chunk, die Ausbreitung des Lichts,
die Kandidaten je Section, `von_vorn` als Ganzes, das Zeichnen und das
Kodieren; der Rest von `von_vorn` ist die Differenz.

| Phase | Karte | `--flat` |
|---|---|---|
| Lesen und Dekodieren | 445 s | 278 s |
| Licht ausbreiten | 546 s | 401 s |
| Kandidaten | 14 s | 10 s |
| Rest von `von_vorn`: Sammeln, Sortieren, Deckung, Sprites | 441 s | 345 s |
| Zeichnen | 55 s | 11 s |
| Kodieren | 156 s | 15 s |
| Rest: Pyramide, Höhen, Schreiben | 116 s | 62 s |
| zusammen | 1773 s | 1122 s |

## Schluss

- **Zeit:** Die Ansicht zeichnet 16-mal weniger Pixel, ist aber nur ein
  Drittel schneller. Lesen und Licht hängen an den Chunks, nicht an den
  Pixeln, und sind 61 % der CPU von `--flat`. Zeichnen und Kodieren sind
  schon bei der Karte nur 12 %.
- **Platz:** ×0,087 für den ganzen Baum. Auf Stufe 6, wo beide gleich
  viele Blöcke je Kachel zeigen, ×0,82.
- **Arbeitsspeicher:** in Bändern 1,55 GiB, rund 1,3-mal die Karte.
