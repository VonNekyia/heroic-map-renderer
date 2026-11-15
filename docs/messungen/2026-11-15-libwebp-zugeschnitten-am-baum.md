---
title: libwebp zugeschnitten, Bytes am Baum
description: Ein voller Lauf und ein Update von oben mit Relief auf der Testwelt, master gegen das zugeschnittene kompakte Packen (#417); gleiche Pixel, 0,156 % mehr Bytes, davon zwei Drittel im Relief; ohne --compact jede Datei gleich; Zeiten unter Last, nicht ausgewertet.
date: 2026-11-15
commits: [22875e00, 0c9f42fd]
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/src/cli/pixel.rs
---

# libwebp zugeschnitten, Bytes am Baum

Am ganzen Baum von oben mit Relief sind die kompakten Kacheln mit dem
zugeschnittenen libwebp 0,156 % grösser. Die Pixel sind gleich. Die
Messung an 409 Kacheln sagte 0,03 % voraus. Der Unterschied liegt vor allem
im Relief: Dort wachsen die Bytes um 1,29 %, und das sind 69 % des Zuwachses.
Ohne `--compact` ist jede Datei gleich.

## Aufbau

- **Stände:**
  - master `22875e00` und der Zweig `0c9f42fd` (#417), beide v0.20.2;
  - Release-Build, je eine Binärdatei.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Schalter:** `--camera top-north --scale 4 --relief slope --gpu off`.
  - **kompakt:** dazu `--compact`;
  - **schnell:** ohne.
- **Läufe:**
  - je Stand und Packung ein voller Lauf mit allen Threads, in einen
    leeren Baum;
  - danach ein Update in denselben Baum, nachdem in 16 über die Welt
    verteilten Chunks der oberste feste Block einer Spalte ersetzt oder
    zurückgesetzt wurde, mit neuem Stempel.
- **Vorher dagegen:** [libwebp für Kacheln zuschneiden](2026-11-14-libwebp-zuschneiden.md)
  mass an 409 Kacheln in der CI, ohne Relief.

## Ablauf

- **Ruhe:** am 15.11.2026, 18:49 bis 19:23, unter `messung.lock`. Ab 18:12
  lief ein Spiel, und die Last lag vor den Läufen bei 5 bis 65 %. Die Zeiten
  sind deshalb nicht ausgewertet. Bytes und Pixel hängen nicht davon ab.
- **kompakt:** Jede Kachel beider Bäume wurde dekodiert und Pixel für Pixel
  verglichen. Die Bytes stammen aus den Grössen der Dateien.
- **schnell:** Die Bäume wurden Datei für Datei verglichen.
- **Je Ordner:** die Grössen der Dateien je Zoomstufe und im Relief, in den
  Bäumen nach dem Update.
- **Quelle,** am 15.11.2026: das Messskript und die Dateigrössen.

## Ergebnis

| Packung | Lauf | Kacheln | andere Pixel | Bytes master | Bytes Zweig | Unterschied |
|---|---|---|---|---|---|---|
| kompakt | voll | 56 620 | 0 | 989 088 926 | 990 631 806 | +0,156 % |
| kompakt | Update | 56 617 | 0 | 989 086 632 | 990 629 526 | +0,156 % |

- **schnell:** Im vollen Lauf und nach dem Update waren von 57 004 Dateien
  alle gleich bis auf `map.json`.
- **`map.json`** war bei beiden Packungen anders, aber nur im Salz von
  `world`, das jeder Baum bei seinem ersten Lauf zufällig zieht, siehe
  [Welten und Kennung](../benutzung/welten.md), „Die Kennung“. Sonst war
  keine Datei neben den Kacheln anders.
- **Je Ordner,** kompakt, nach dem Update:

  | Ordner | Dateien | Bytes master | Unterschied |
  |---|---|---|---|
  | Basis, Zoom 8 | 16 127 | 640 842 574 | +520 892 B, +0,081 % |
  | Pyramide, Zoom 0 bis 7 | 5910 | 265 460 484 | −47 960 B, −0,018 % |
  | Relief mit Rest, alle Stufen | 34 580 | 82 783 574 | +1 069 962 B, +1,292 % |
  | zusammen | 56 617 | 989 086 632 | +1 542 894 B, +0,156 % |

  In der Pyramide liegen die einzelnen Stufen zwischen −0,21 % und
  +0,19 %.

## Schluss

- **Die Pixel bleiben gleich,** in jeder kompakten Kachel. Ohne `--compact`
  ändert sich kein Byte.
- **Am Baum kostet der Zuschnitt 0,156 % Bytes,** rund fünfmal so viel wie an
  den 409 Kacheln vorhergesagt:
  - Die Basis wächst um 0,081 %, mehr als die 0,033 % dort.
  - Das Relief wächst um 1,29 %. Die 409 Kacheln hatten kein Relief.
  - Welche der drei Änderungen das Relief wachsen lässt, ist nicht
    gemessen.
- **Offen:**
  - die Zeit, in Ruhe;
  - der Anteil des Kodierens und die eigene Zeit der drei Funktionen unter
    den Bedingungen von
    [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md).
