---
title: Licht je Spalte
description: Was das Licht je Spalte statt der Ausbreitung der einfarbigen Ansicht über die ganze Testwelt an Zeit und Arbeitsspeicher spart und wie viele Pixel es ändert, je drei Läufe im Wechsel gegen den Stand davor.
date: 2026-10-10
commits: [812197f, 8c98c14]
code:
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
---

# Licht je Spalte

Mit Licht je Spalte statt der Ausbreitung braucht `--flat` über die ganze
Testwelt 35 % weniger Zeit und 32 % weniger CPU, 45 s statt 69 s. Der
Arbeitsspeicher an der Spitze bleibt gleich. 10,5 % der Pixel der Basis
ändern sich, die Hälfte davon um höchstens 4 von 255, fast alle werden
dunkler, vor allem an den Rändern der Baumkronen.

## Aufbau

- **Stände,** Release-Build:
  - vorher `812197f`: `--flat` in Bändern, Licht ausgebreitet, dieselben
    Bytes wie master;
  - nachher `8c98c14`: Licht je Spalte.
- **Welt und Befehl** wie in
  [2026-10-10, Einfarbige Ansicht](2026-10-10-einfarbige-ansicht.md): die
  ganze Testwelt, `--flat`, `--gpu off`, Threads nach Vorgabe, jeder Lauf
  frisch in ein leeres Verzeichnis.

## Ablauf

- **Folge:** vorher, nachher, im Wechsel, je drei Läufe, am 10.10. von 12:17
  bis 12:26.
- **Ruhe:** vor jedem Lauf die Last unter 10 % und 15 s Pause; die Reihe
  unter der Sperre, ohne laufenden Minecraft-Client. Die Last vor den Läufen
  lag bei 3 bis 9 %.
- **Quelle:** das Messskript misst Wanduhr, CPU-Zeit und die Spitze des
  Arbeitsspeichers und vergleicht die Basis beider Bäume Pixel für Pixel.

## Ergebnis

| Stand | Wanduhr | CPU | Spitze | Platz |
|---|---|---|---|---|
| vorher, `812197f` | 69,36 s, 70,14 s, 68,30 s | 1244 s, 1232 s, 1230 s | 1,52 bis 1,56 GiB | 91,53 MB |
| nachher, `8c98c14` | 45,27 s, 45,12 s, 45,79 s | 842 s, 840 s, 854 s | 1,50 bis 1,54 GiB | 92,40 MB |

- **Zeit:** −35 % an der Wanduhr, −32 % CPU; die Spannen trennen sich weit.
- **Speicher:** gleich. Warum, ist nicht gemessen; die weiche Beleuchtung
  am Rand eines Chunks liest weiter das Licht des Nachbarn.
- **Platz:** +0,96 %.

### Pixel

Die Basis, Stufe 6, 1153 Kacheln mit 75 563 008 Pixeln:

| Abweichung, höchster Kanal | Pixel | Anteil der geänderten |
|---|---|---|
| 1 | 2 018 149 | 25 % |
| bis 4 | 4 155 284 | 52 % |
| bis 16 | 7 113 103 | 90 % |
| bis 64 | 7 918 667 | 99,99 % |
| über 64 | 900 | 0,01 % |
| zusammen | 7 919 567 | 10,5 % aller Pixel, in 1143 Kacheln |

- **Wo:** fast nur im Wald, als Ränder um die Kronen. Die weiche
  Beleuchtung einer Oberseite liest auch die Zellen unter dem Laub der
  Nachbarspalte; mit Ausbreitung kam dorthin Licht von der Seite, je Spalte
  nicht mehr.
- **Richtung:** Im Ausschnitt von `docs/bilder/einfarbig.webp`, 512 × 512
  Blöcke, sind 20 591 Pixel anders, 19 662 davon dunkler; 2938 um mehr als
  8, 580 um mehr als 16.
- **Auf den gröberen Stufen** weniger: 9,0 % auf Stufe 5, 0,75 % auf
  Stufe 0.

## Schluss

- **Initial und live:** ein Drittel schneller, weil das Ausbreiten wegfällt
  und mit ihm die Nachbarn der Nachbarn.
- **Bild:** dunklere Ränder um Baumkronen, meist um wenige Stufen; der
  Schein von Leuchtendem unter Wasser fehlt.
- **Speicher und Platz:** gleich.
