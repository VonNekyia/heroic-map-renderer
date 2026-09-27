---
title: Phasen je Kachel, erste Runde der Umbauten
description: Wohin die Zeit einer Kachel ging, vor und nach Cache je Stapel, Bitmasken samt mimalloc, Flächen überspringen und Sammeln nur im Band.
date: 2026-09-23
commits: [4fbb51c, aabf997, 1b5b283, fa615dd, bd0e73f, 93faf81]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/main.rs
  - renderer/src/world/palette.rs
---

# Phasen je Kachel, erste Runde der Umbauten

Einfädig brauchte eine Kachel am Ende 6,3 statt 64 ms, auf 24 Threads
schaffte der Renderer 813 statt 159 Kacheln/s, Byte für Byte dasselbe Bild.
Gemessen in der ersten Fassung von #10, vor den Regeln von #9, auf der
grossen Welt; wie viel davon nach dem Hochziehen blieb, steht in
[2026-09-25, Bitmasken](2026-09-25-bitmasken.md), dort auf der Testwelt.

## Aufbau

- Welt: die grosse Welt, scale 32, wie `aabf997` schreibt; jeder Messlauf
  dieser Nacht las sie. [2026-09-25, Bitmasken](2026-09-25-bitmasken.md) mass
  Ausschnitte derselben Grösse auf der Testwelt; Zahl für Zahl vergleichen
  lassen sich die beiden Tabellen deshalb nicht.
- Einfädig ein 4096er-Ausschnitt, je Kachel; mit 12 und 24 Threads ein
  8192er-Ausschnitt.
- Stände nacheinander: ursprünglich `4fbb51c`, Cache je Stapel `aabf997`,
  Bitmasken und mimalloc `1b5b283`, Flächen überspringen `fa615dd`, Sammeln
  nur im Band `bd0e73f`, alle vom 23.09. nachts.
- Die Spalte „Sammeln“ kam mit `93faf81` (Paletten ohne HashMap) ins
  README. `bd0e73f` selbst nennt 6,6 ms und dieselben 813 Kacheln/s,
  `93faf81` rund drei Prozent weniger; ob die 6,3 ms schon `93faf81`
  enthalten, ist nicht festgehalten.
- Wie oft je Stand gemessen wurde und ob die Stände abwechselnd liefen, ist
  nicht festgehalten.

## Ablauf

Je Stand die Phasen einer Kachel einfädig gemessen, dazu die Rate mit 12
und 24 Threads. Nach jedem Umbau der 8192er-Ausschnitt Byte für Byte gegen
den Stand davor.

## Ergebnis

| Phase | ursprünglich | Cache je Stapel | Bitmasken | Flächen | Sammeln |
|---|---|---|---|---|---|
| Blöcke finden und Sprite wählen | 39 ms | 20 ms | 4,5 ms | 4,5 ms | 3,3 ms |
| Chunks laden und dekodieren | 15 ms (106 Chunks) | 1 ms (6,5) | 1,5 ms (9) | 1,5 ms | 1,5 ms |
| Sprites zeichnen | 9 ms | 9 ms | 8 ms | ~3 ms | ~3 ms |
| WebP kodieren | 0,5 ms | 0,5 ms | 0,6 ms | 0,6 ms | 0,6 ms |
| gesamt, ein Kern | 64 ms | 30 ms | 13 ms | 7,8 ms | 6,3 ms |
| 12 Threads, 8192er-Ausschnitt | | | 411 Kacheln/s | 620 | 700 |
| 24 Threads, 8192er-Ausschnitt | 159 Kacheln/s | 318 | 532 | 731 | 813 |

- mimalloc: ohne ihn schaffte der Stand mit Bitmasken auf 24 Threads 245
  statt 532 Kacheln/s; ein Chunk brauchte parallel sechsmal so lang wie
  allein.
- Flächen überspringen: Ein sichtbarer Block zeichnete alle drei Flächen,
  70-fach überzeichnet, auf flachem Gelände zwei von drei umsonst.
- Sammeln: Die Schleife über alle 24 Sections aller Band-Chunks kostete
  1,3 ms, mehr als das Auswerten der Masken selbst.

Byte für Byte gleich nach jedem Umbau: der 8192er-Ausschnitt, alle 1393
Dateien.

## Schluss

Der grösste Posten war das Suchen und Wählen je Block, dann das
Dekodieren. Auf 24 Threads gewinnen die Umbauten weniger als auf einem
Kern, denn dort teilen sich die Threads Kerne und Speicherbandbreite.
Entscheidungen: [0020](../entscheidungen/0020-mimalloc.md),
[0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md).
