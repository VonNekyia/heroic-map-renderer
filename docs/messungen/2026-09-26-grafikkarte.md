---
title: CPU gegen Grafikkarte
description: Wie viel schneller die Kacheln mit einer eigenständigen Grafikkarte über Vulkan werden, einfädig und auf 12 und 24 Threads, auf der Testwelt und der grossen Welt.
date: 2026-09-26
commits: [d958b78, 6f0e365]
code:
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
---

# CPU gegen Grafikkarte

Mit der Karte ist ein Kern gut anderthalbmal so schnell, 24 Threads ein
Fünftel bis ein Drittel: auf der grossen Welt 872 bis 946 statt 712 bis
750 Kacheln/s. Seit der Deckungsmaske ist die CPU ohne Karte fast so schnell
wie mit, siehe
[2026-09-27, Die grossen Posten, zweite Runde](2026-09-27-grosse-posten-zweite-runde.md).

## Aufbau

- Eine eigenständige Grafikkarte über Vulkan.
- Testwelt: dieselben Ausschnitte wie in
  [2026-09-25, Bitmasken](2026-09-25-bitmasken.md), scale 32.
- Grosse Welt: ein Ausschnitt mit 65 536 Basiskacheln auf Land, scale 32,
  24 Threads.
- Stände: master mit #10 (`d958b78`) und #11 nach der ersten Runde des
  Reviews, mit `--gpu off` und mit Karte; die Zahlen kamen mit `6f0e365`
  ins README. Die zweite und dritte Runde änderten den Zeichenweg nicht.

## Ablauf

Alle Fassungen am selben Tag abwechselnd. Auf der Testwelt jeweils das
beste von drei Läufen, auf der grossen Welt alle drei.

## Ergebnis

| | master | #11, CPU | #11, mit Karte |
|---|---|---|---|
| ein Kern, 4096er-Ausschnitt | 8,1 ms je Kachel | 7,0 ms | 4,5 ms |
| 12 Threads, 8192er-Ausschnitt | 568 Kacheln/s | 650 | 858 |
| 24 Threads, 8192er-Ausschnitt | 690 | 674 | 917 |
| 24 Threads, grosse Welt, 65 536 Basiskacheln | 655 / 675 / 706 | 750 / 712 / 725 | 946 / 915 / 872 |

- Ein ganzer Lauf über den grossen Ausschnitt, mit Vorlauf und Pyramide,
  brauchte mit der Karte 120 bis 125 s statt 135 bis 142.
- Auf der Testwelt dauert die Basis nur 1 bis 2 s; dort schwankten die
  Läufe um bis zu 20 %.
- Speicher an der Spitze, 24 Threads: 1,5 statt 1,0 GB bei scale 32, mit
  drei nativen Stufen bis hinunter zu scale 4 1,8 statt 1,1 GB.
- Die Adaptersuche kostet unter `auto` 0,1 s je Lauf, findet sie nichts
  Passendes, 0,4 s.
- Jede geschriebene Kachel ging durch den Echtzeitschutz.

Byte für Byte gleich: Auf einem Ausschnitt der grossen Welt mit 4608
Basiskacheln, drei nativen Stufen und Pyramide sind alle 6164 Kacheln
gleich mit master, die der CPU wie die der Karte; nur `map.json` weicht ab,
dort das Salz der Kennung. Am Kopf von #11 noch einmal: CPU gegen Karte und
Karte gegen den Stand der ersten Runde, je alle 6164 gleich.

## Schluss

Die Karte ersetzt nur den Blit; Dekodieren, Kandidaten, Sprite-Wahl und
WebP bleiben auf der CPU. Eine Onboard-Grafik ist nicht gemessen; dort
zeigt `--gpu off` gegen `--gpu auto`, ob der Standard passt. Entscheidung:
[0023](../entscheidungen/0023-zeichnen-auf-der-grafikkarte.md).
