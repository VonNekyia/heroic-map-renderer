---
title: Ecken nach Masken
description: Die weiche Beleuchtung liest nur die Zellen und Spalten der gezeigten Plätze (#419), gegen master auf der Testwelt von oben und im iso; jede Datei gleich, im Profil von oben zelle −56 % und umgebung −37 %, CPU-Zeit −2,8 %, an der Wanduhr von oben rund 1 bis 3 % und im iso 1,6 % schneller.
date: 2026-11-15
commits: [22875e00, f2495994]
code:
  - renderer/src/render/metatile.rs
  - skills/renderzeit-erneuern/phasen.py
---

# Ecken nach Masken

Liest die weiche Beleuchtung nur die Zellen und Spalten, nach denen die
gezeigten Plätze fragen, dann braucht `ChunkCache::zelle` in einem vollen
Lauf von oben 56 % weniger eigene Zeit und `umgebung` 37 % weniger.
Sammeln und Deckung werden um 20 % kürzer, der ganze Lauf braucht 2,8 %
weniger CPU-Zeit. An der Wanduhr ist er von oben rund 1 bis 3 % schneller,
im iso 2:1 1,6 %. Jede Datei gleicht der von master.

## Aufbau

- **Stände:** master `22875e00` und der Zweig `f2495994` (#419), beide
  v0.20.2.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Schalter:**
  - **von oben:** `--camera top-north --scale 4 --relief slope --gpu off`;
  - **iso:** `--camera 2:1 --scale 8 --gpu off`.
- **Bytevergleich**, erstes Fenster:
  - von oben und im iso, Release-Build;
  - je Stand ein voller Lauf in einen leeren Baum, dann ein Update in
    denselben Baum. Vorher wurde in 16 über die Welt verteilten Chunks der
    oberste feste Block einer Spalte zurückgesetzt, mit neuem Stempel.
- **Profil**, zweites Fenster:
  - je Stand ein voller Lauf von oben unter den Bedingungen von
    [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md), also
    mit `--compact`;
  - Build mit Zeilentabellen und `#[inline(never)]` vor den Funktionen aus
    `MARKEN`, Proben 1000 je Sekunde.
- **Wanduhr**, zweites Fenster, Release-Build ohne Profiler, je ein voller
  Lauf in einen leeren Baum:
  - **von oben:** zwei Durchgänge, je master, #418, #419; die Zeiten von
    #418 stehen in seiner eigenen Messung;
  - **iso:** zwei Durchgänge, je master, #419.

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
  - `ecken_aus` ordnet `phasen.py` seit diesem Zweig Sammeln und Deckung zu,
    wie vorher `ecken_at`.

## Ergebnis

### Bytes

- **Von oben:** Nach dem vollen Lauf waren alle 57 004 Dateien gleich, nach
  dem Update alle 57 001, ausser `map.json`.
- **Im iso:** Nach dem vollen Lauf und nach dem Update waren alle 24 737
  Dateien gleich, ausser `map.json`.
- In `map.json` unterschied sich nur das Salz von `world`, das jeder Baum
  bei seinem ersten Lauf zufällig zieht, siehe
  [Welten und Kennung](../benutzung/welten.md), „Die Kennung“.

### Profil, von oben

Ein Lauf je Stand. Die eigene Zeit ist über alle Threads summiert, der
Anteil bezieht sich auf die CPU-Zeit des Laufs.

| | master | Zweig |
|---|---|---|
| Dauer unter dem Profiler | 145,3 s | 144,6 s |
| CPU-Zeit | 2811,6 s | 2732,6 s (−2,8 %) |
| Sammeln und Deckung | 508,6 s, 18,1 % | 406,9 s, 14,9 % (−20,0 %) |
| `ChunkCache::zelle` | 124,7 s, 4,44 % | 54,4 s, 1,99 % (−56,4 %) |
| `umgebung`, Closure | 68,1 s, 2,42 % | 43,2 s, 1,58 % (−36,6 %) |
| `ecken_at` / `ecken_aus`, Closure | 18,4 s | 18,6 s |
| `ChunkCache::licht_slot` | 24,9 s, 0,89 % | 11,4 s, 0,42 % |

`licht_slot` sinkt mit, weil `zelle` es seltener aufruft. Die Ausbreitung
des Lichts selbst bleibt gleich.

### Wanduhr

| Durchgang | von oben, master | von oben, Zweig | iso, master | iso, Zweig |
|---|---|---|---|---|
| 1 | 116,8 s | 111,7 s (−4,4 %) | 92,4 s | 91,2 s (−1,3 %) |
| 2 | 110,3 s | 104,7 s (−5,1 %) | 92,2 s | 90,5 s (−1,8 %) |

- **Von oben, Drift:** Der zweite Durchgang ist bei jedem Stand 5,6 bis
  6,9 % schneller als der erste. Der Zweig lief in jedem Durchgang rund
  4 min nach master, und die Drift kommt ihm zugute. Ohne sie ist er eher
  1 bis 3 % schneller. Das passt zu den 2,8 % weniger CPU-Zeit im Profil.
- **Im iso** gibt es keine Drift: master 92,4 und 92,2 s. Im Mittel ist der
  Zweig 1,6 % schneller.

## Schluss

- **Jede Datei bleibt gleich,** von oben und im iso, im vollen Lauf und im
  Update.
- **Das Profil zeigt den Rückgang klar:** `zelle` −56 %, `umgebung` −37 %,
  Sammeln und Deckung −20 %. Von oben liest ein Sprite mit nur der Oberseite
  10 der 27 Zellen, siehe
  [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Beim Zeichnen“.
- **Der ganze Lauf** wird klein schneller: 2,8 % CPU-Zeit von oben, an der
  Wanduhr 1 bis 3 % von oben und 1,6 % im iso. Im iso zeigt ein Sprite oft
  drei Seiten und liest 20 der 27 Zellen; dort spart es weniger.
- **Grenzen:**
  - Das Profil ist ein Lauf je Stand und nur von oben, die Wanduhr zwei
    Läufe je Stand.
  - Alles lief unter Windows.
