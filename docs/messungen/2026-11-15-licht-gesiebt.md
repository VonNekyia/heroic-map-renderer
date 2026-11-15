---
title: Licht gesiebt
description: Die Ausbreitung des Lichts nach Abstand zur Mitte gesiebt (#418) gegen master, auf der Testwelt von oben; jede Datei gleich, im Profil die halbe eigene Zeit von breite_aus und 6,4 % weniger CPU-Zeit, an der Wanduhr rund 5 % schneller.
date: 2026-11-15
commits: [22875e00, fba227ae]
code:
  - renderer/src/render/licht.rs
---

# Licht gesiebt

Gesiebt braucht `Ausbreitung::breite_aus` in einem vollen Lauf von oben
49,8 % weniger eigene Zeit, 176,7 s statt 351,9 s. Der Lauf braucht damit
6,4 % weniger CPU-Zeit. An der Wanduhr ist er rund 5 % schneller. Jede
Datei des Baums gleicht der von master.

## Aufbau

- **Stände:** master `22875e00` und der Zweig `fba227ae` (#418), beide
  v0.20.2.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Bytevergleich**, erstes Fenster:
  - `--camera top-north --scale 4 --relief slope --gpu off`, Release-Build;
  - je Stand ein voller Lauf in einen leeren Baum, dann ein Update in
    denselben Baum. Vorher wurde in 16 über die Welt verteilten Chunks der
    oberste feste Block einer Spalte zurückgesetzt, mit neuem Stempel.
- **Profil**, zweites Fenster:
  - je Stand ein voller Lauf unter den Bedingungen von
    [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md), also
    dieselben Schalter mit `--compact`;
  - Build mit Zeilentabellen und `#[inline(never)]` vor den Funktionen aus
    `MARKEN`, Proben 1000 je Sekunde.
- **Wanduhr**, zweites Fenster:
  - die Schalter des Bytevergleichs, Release-Build ohne Profiler, je ein
    voller Lauf in einen leeren Baum;
  - zwei Durchgänge, je master, #418, #419. Die Zeiten von #419 stehen in
    seiner eigenen Messung.

## Ablauf

- **Erstes Fenster,** am 15.11.2026 von 19:23 bis 20:01, unter
  `messung.lock`:
  - Die Bäume wurden Datei für Datei verglichen.
  - Ab 18:12 lief ein Spiel, und die Last lag vor den Läufen bei 8 bis
    48 %. Die Zeiten dieses Fensters sind deshalb nicht ausgewertet.
- **Zweites Fenster,** am 15.11.2026 von 20:31 bis 21:26, unter
  `messung.lock`:
  - Es lief kein Spiel. Vor jedem Lauf lag die Last bei höchstens 10 %.
  - Daneben lief ein kleiner Webserver im Leerlauf.
  - Erst kamen die Profile aller Stände, dann die Wanduhr.
- **Quelle,** am 15.11.2026: die Ausgabe der Läufe, das Messskript und
  `auswerten.py` aus
  [`skills/renderzeit-erneuern`](../../skills/renderzeit-erneuern/SKILL.md),
  ohne neue Tabellen auf der Seite der Renderzeit.

## Ergebnis

### Bytes

- Nach dem vollen Lauf waren alle 57 004 Dateien gleich, nach dem Update
  alle 57 001, ausser `map.json`.
- In `map.json` unterschied sich nur das Salz von `world`, das jeder Baum
  bei seinem ersten Lauf zufällig zieht, siehe
  [Welten und Kennung](../benutzung/welten.md), „Die Kennung“.

### Profil

Ein Lauf je Stand. Die eigene Zeit ist über alle Threads summiert, der
Anteil bezieht sich auf die CPU-Zeit des Laufs.

| | master | Zweig |
|---|---|---|
| Dauer unter dem Profiler | 145,3 s | 133,0 s |
| CPU-Zeit | 2811,6 s | 2630,5 s (−6,4 %) |
| Licht | 518,7 s, 18,5 % | 342,8 s, 13,0 % (−33,9 %) |
| `Ausbreitung::breite_aus` | 351,9 s, 12,5 % | 176,7 s, 6,7 % (−49,8 %) |
| `Ausbreitung::chunk` | 130,0 s, 4,6 % | 129,5 s, 4,9 % |

Die anderen Phasen ändern sich zusammen um 5 s.

### Wanduhr

| Durchgang | master | Zweig | Unterschied |
|---|---|---|---|
| 1 | 116,8 s | 109,1 s | −6,6 % |
| 2 | 110,3 s | 101,6 s | −7,9 % |

- **Drift:** Der zweite Durchgang ist bei jedem Stand 5,6 bis 6,9 %
  schneller als der erste.
- **Die Reihenfolge hilft dem Zweig.** In jedem Durchgang lief er rund
  2 min nach master. Ein Teil der Drift fällt so in seinen Vorteil. Ohne
  sie ist er eher 5 % schneller. Das passt zu den 6,4 % weniger CPU-Zeit im
  Profil.

## Schluss

- **Jede Datei bleibt gleich,** im vollen Lauf und im Update.
- **Die eigene Zeit von `breite_aus` halbiert sich.** Die Herleitung in #418
  sagte 20 bis 45 % voraus, gemessen sind 49,8 %.
- **Der ganze Lauf von oben** braucht 6,4 % weniger CPU-Zeit und an der
  Wanduhr rund 5 % weniger.
- **Im iso mit Cinematic** ist Licht unter 5 % der CPU-Zeit, siehe
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md). Dort
  bringt das Sieben entsprechend weniger; gemessen ist es nicht.
- **Grenzen:**
  - Das Profil ist ein Lauf je Stand, die Wanduhr zwei Läufe je Stand.
  - Alles lief unter Windows.
