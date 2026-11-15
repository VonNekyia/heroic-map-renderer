---
title: libwebp zugeschnitten am Baum
description: Das zugeschnittene kompakte Packen (#417) gegen master auf der Testwelt von oben mit Relief; gleiche Pixel, 0,156 % mehr Bytes, davon zwei Drittel im Relief; im Profil 26,7 % weniger Zeit fürs Kodieren und 5,2 % weniger CPU-Zeit, an der Wanduhr rund 8 % schneller.
date: 2026-11-15
commits: [22875e00, 0c9f42fd]
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/src/cli/pixel.rs
---

# libwebp zugeschnitten am Baum

Mit dem zugeschnittenen libwebp braucht ein voller Lauf von oben mit Relief
und `--compact` 26,7 % weniger Zeit fürs Kodieren und 5,2 % weniger
CPU-Zeit. An der Wanduhr ist er rund 8 % schneller. Die Pixel bleiben
gleich.

Die Bytes der Kacheln wachsen um 0,156 %. Vorhergesagt waren an 409 Kacheln
0,03 %. Der Unterschied liegt vor allem im Relief: Dort wachsen die Bytes um
1,29 %, und das sind 69 % des Zuwachses. Ohne `--compact` ist jede Datei
gleich.

## Aufbau

- **Stände:** master `22875e00` und der Zweig `0c9f42fd` (#417), beide
  v0.20.2.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Schalter:** `--camera top-north --scale 4 --relief slope --gpu off`.
  - **kompakt:** dazu `--compact`;
  - **schnell:** ohne.
- **Bytes**, erstes Fenster:
  - je Stand und Packung ein voller Lauf mit allen Threads, in einen leeren
    Baum, Release-Build;
  - danach ein Update in denselben Baum. Vorher wurde in 16 über die Welt
    verteilten Chunks der oberste feste Block einer Spalte ersetzt oder
    zurückgesetzt, mit neuem Stempel.
- **Profil**, zweites Fenster:
  - je Stand ein voller Lauf, kompakt, unter den Bedingungen von
    [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md);
  - Build mit Zeilentabellen und `#[inline(never)]` vor den Funktionen aus
    `MARKEN`, Proben 1000 je Sekunde.
- **Wanduhr**, zweites Fenster: kompakt, voller Lauf, Release-Build ohne
  Profiler, abwechselnd master, Zweig, master, Zweig.
- **Vorher dagegen:** [libwebp für Kacheln zuschneiden](2026-11-14-libwebp-zuschneiden.md)
  mass an 409 Kacheln in der CI, ohne Relief.

## Ablauf

- **Erstes Fenster,** am 15.11.2026 von 18:49 bis 19:23, unter
  `messung.lock`:
  - Ab 18:12 lief ein Spiel, und die Last lag vor den Läufen bei 5 bis
    65 %. Die Zeiten dieses Fensters sind deshalb nicht ausgewertet. Bytes
    und Pixel hängen nicht davon ab.
  - **kompakt:** Jede Kachel beider Bäume wurde dekodiert und Pixel für
    Pixel verglichen. Die Bytes stammen aus den Grössen der Dateien.
  - **schnell:** Die Bäume wurden Datei für Datei verglichen.
  - **Je Ordner:** die Grössen der Dateien je Zoomstufe und im Relief, in
    den Bäumen nach dem Update.
- **Zweites Fenster,** am 15.11.2026 von 20:31 bis 21:26, unter
  `messung.lock`:
  - Es lief kein Spiel. Vor jedem Lauf lag die Last bei höchstens 10 %.
  - Daneben lief ein kleiner Webserver im Leerlauf.
  - Erst kamen die Profile aller Stände, dann die Wanduhr.
- **Quelle,** am 15.11.2026: die Ausgabe der Läufe, das Messskript, die
  Dateigrössen und `auswerten.py` aus
  [`skills/renderzeit-erneuern`](../../skills/renderzeit-erneuern/SKILL.md),
  ohne neue Tabellen auf der Seite der Renderzeit.

## Ergebnis

### Bytes

| Packung | Lauf | Kacheln | andere Pixel | Bytes master | Bytes Zweig | Unterschied |
|---|---|---|---|---|---|---|
| kompakt | voll | 56 620 | 0 | 989 088 926 | 990 631 806 | +0,156 % |
| kompakt | Update | 56 617 | 0 | 989 086 632 | 990 629 526 | +0,156 % |

- **schnell:** Im vollen Lauf und nach dem Update waren von 57 004 Dateien
  alle gleich bis auf `map.json`.
- **`map.json`** war bei beiden Packungen anders. Es unterschied sich aber
  nur im Salz von `world`, das jeder Baum bei seinem ersten Lauf zufällig
  zieht, siehe [Welten und Kennung](../benutzung/welten.md), „Die Kennung“.
  Sonst war ausser den Kacheln keine Datei anders.
- **Je Ordner,** kompakt, nach dem Update:

  | Ordner | Dateien | Bytes master | Unterschied |
  |---|---|---|---|
  | Basis, Zoom 8 | 16 127 | 640 842 574 | +520 892 B, +0,081 % |
  | Pyramide, Zoom 0 bis 7 | 5910 | 265 460 484 | −47 960 B, −0,018 % |
  | Relief mit Rest, alle Stufen | 34 580 | 82 783 574 | +1 069 962 B, +1,292 % |
  | zusammen | 56 617 | 989 086 632 | +1 542 894 B, +0,156 % |

  In der Pyramide liegen die einzelnen Stufen zwischen −0,21 % und
  +0,19 %.

### Profil

Ein Lauf je Stand. Die eigene Zeit ist über alle Threads summiert, der
Anteil bezieht sich auf die CPU-Zeit des Laufs.

| | master | Zweig |
|---|---|---|
| Dauer unter dem Profiler | 145,3 s | 135,3 s |
| CPU-Zeit | 2811,6 s | 2665,0 s (−5,2 %) |
| Kodieren | 704,6 s, 25,1 % | 516,5 s, 19,4 % (−26,7 %) |
| `CalculateBestCacheSize` | 148,9 s, 5,30 % | 0,0 s |
| `BackwardReferencesRle` | 20,8 s, 0,74 % | 0,0 s |
| `AnalyzeEntropy` | 22,5 s, 0,80 % | 6,5 s, 0,24 % |

`AnalyzeEntropy` bleibt bei Bildern mit Palette, siehe
[0142](../entscheidungen/0142-kompakt-ohne-suche-rle-und-analyse.md).

### Wanduhr

| Durchgang | master | Zweig | Unterschied im Paar |
|---|---|---|---|
| 1 | 148,1 s | 133,5 s | −9,9 % |
| 2 | 141,5 s | 127,5 s | −9,9 % |

- **Drift:** Der zweite Durchgang ist bei beiden Ständen rund 4,5 %
  schneller als der erste.
- Der Zweig lief in jedem Paar nach master, die Drift hilft ihm also. Sein
  erster Lauf lag zwischen den beiden master-Läufen. Gegen deren Mittel,
  144,8 s, ist er 7,8 % schneller.

## Schluss

- **Die Pixel bleiben gleich,** in jeder kompakten Kachel. Ohne `--compact`
  ändert sich kein Byte.
- **Die Zeit:**
  - Die drei Funktionen, die der Zuschnitt auslässt, brauchten 192,2 s von
    2811,6 s CPU-Zeit. Übrig bleiben 6,5 s.
  - Das Kodieren wird um 26,7 % kürzer. An den 409 Kacheln waren es 31 %.
  - Der ganze Lauf braucht 5,2 % weniger CPU-Zeit und an der Wanduhr rund
    8 % weniger.
- **Die Bytes:** Am Baum kostet der Zuschnitt 0,156 %, rund fünfmal so viel
  wie an den 409 Kacheln vorhergesagt.
  - Die Basis wächst um 0,081 %, mehr als die 0,033 % dort.
  - Das Relief wächst um 1,29 %. Die 409 Kacheln hatten kein Relief, darum
    sah die Vorhersage es nicht.
  - Welche der drei Änderungen das Relief wachsen lässt, ist nicht gemessen.
- **Grenzen:**
  - Das Profil ist ein Lauf je Stand, die Wanduhr zwei Läufe je Stand.
  - Alles lief unter Windows und nur von oben, nicht im iso.
