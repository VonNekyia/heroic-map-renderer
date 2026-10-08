---
title: Nachverdichten
description: Was --compact-tree an einem schnellen Baum der Testwelt mit einem und mit vier Threads kostet, was ein zweiter Aufruf kostet, und dass danach jede Kachel dieselben Bytes hat wie der kompakte Export und dieselbe Zeit wie vorher.
date: 2026-10-08
commits: [cca8a7a]
code:
  - renderer/src/cli/verdichten.rs
---

# Nachverdichten

`--compact-tree` packt die 3086 Kacheln eines schnellen Baums der Testwelt
auf einem Thread in 41,7 s kompakt, rund 13,5 ms je Kachel, mit 4 Threads
in 19,6 s. Danach hat jede Kachel Byte für Byte dieselben Bytes wie nach
einem Export mit `--compact` und dieselbe Zeit wie vorher. Ein zweiter
Aufruf erkennt jede Kachel an ihrem Hash und braucht 3,3 s.

## Aufbau

- **Bäume:** aus der Reihe in
  [2026-10-08, Kompakt packen](2026-10-08-kompakt.md), mit 4 Threads und 3
  nativen Stufen:
  - der schnelle Baum aus dem ersten Lauf, 3086 Kacheln, als Kopie mit
    allen Zeiten;
  - zum Vergleich der kompakte Baum aus dem ersten Lauf derselben Reihe.
- **Stand:** `cca8a7a`, Release-Build.
- **Befehl:**

  ```bash
  heroic-map-renderer --compact-tree <kopie>/2x1-se --threads 1
  ```

## Ablauf

- **Läufe:** je eine frische Kopie für 1 und für 4 Threads. Danach ein
  zweiter Aufruf über die Kopie mit einem Thread.
- **Ruhe:** davor die Last unter 10 % und 15 s Pause; die ganze Reihe unter
  der Sperre, ohne laufenden Minecraft-Client.
- **Quelle,** am 08.10. abends:
  - das Messskript: Wanduhr, CPU-Zeit und Spitze des Arbeitsspeichers;
  - SHA-256 und Zeit jeder Kachel vorher und nachher;
  - die Zeile `Verdichtet:` der Ausgabe.

## Ergebnis

| Lauf | Wanduhr | CPU | Spitze | Ausgabe |
|---|---|---|---|---|
| 1 Thread | 41,7 s | 41,2 s | 0,026 GiB | 3086 Kacheln, 108,4 statt 154,0 MiB |
| 4 Threads | 19,6 s | 46,1 s | 0,039 GiB | ebenso |
| zweiter Aufruf, 1 Thread | 3,3 s | 3,3 s | 0,022 GiB | 0 Kacheln, 3086 schon kompakt |

- **Bytes:** In beiden Kopien gleicht jede Kachel der des kompakten
  Exports.
- **Zeiten:** Jede Kachel hat nachher dieselbe Zeit wie vorher.
- **Je Kachel:** Dekodieren und kompakt Kodieren kosten rund 13,5 ms. Ein
  zweiter Aufruf dekodiert nur, rund 1,1 ms.

## Schluss

- **Grosse Welt, hochgerechnet:** 3,3 Mio. Kacheln zu 13,5 ms sind rund 12
  CPU-Stunden. Mit `--threads 1 --low-priority` ist das gut ein halber Tag
  auf einem Kern, einmal.
- **Speicher:** Die Spitze bleibt unter 0,05 GiB, denn je Thread liegt nur
  eine Kachel und der Block ihrer Hashes im Speicher.
- **Ein Abbruch** kostet nur die Blöcke in Arbeit. Ein neuer Aufruf
  dekodiert den Rest, kodiert aber nur, was noch nicht kompakt ist.
- **Entscheidung:** [0093](../entscheidungen/0093-nachverdichten.md).
