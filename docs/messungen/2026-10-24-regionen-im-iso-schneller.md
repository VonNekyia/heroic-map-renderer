---
title: Regionen im iso schneller
description: Rechnen der Ansichten, lange Tasks, Bildzeit, JS-Heap und Anfragen der Höhen der Webkarte mit den Ebenen eines Servers auf der grossen Welt, iso gegen 2D, vor und nach 0123; je drei Läufe, mit Profil.
date: 2026-10-24
commits: [76ea0cc, 89c9e86]
code:
  - web/src/ebenen.ts
  - web/src/formen.ts
  - web/src/gelaende.ts
  - web/src/hoehen.ts
---

# Regionen im iso schneller

Nach [0123](../entscheidungen/0123-formen-im-iso-gemerkt-je-stufe.md)
blockieren die Ebenen im iso die Seite beim Ziehen und Zoomen im Mittel
noch 21 ms statt 3 952 ms, rund 190-mal weniger, und keine Aufgabe dauert
länger als 63 ms statt 924–1 271 ms. Das Rechnen der Ansichten sinkt von
3 828 ms auf 454 ms, 8,4-mal. Jede Region Höhen lädt einmal statt bis zu
fünfmal. Die Bildzeit p99 liegt mit 44 ms beim Rauschen dieser Maschine,
wie in 2D.

## Aufbau

- **Welt:** die grosse Welt, zwei Bäume aus ihrer `trees.json`: iso 4:3
  aus Südost mit `cinematic`, `groundCell` 1, und 2D von oben genordet.
  Kacheln, `layers.json`, Ebenen und Höhen vom Webserver des Plugins auf
  demselben Rechner, nur gelesen.
- **Ebenen:** die vier Ebenen des Servers, alle an: Regionen, 34 Formen mit
  456 Punkten, die über rund 21 000 Blöcke reichen; Kreise, Städte,
  Schiffsrouten.
- **Seite:** ein lokaler Build ohne Minify, damit das Profil Namen trägt.
  Playwright liefert ihn aus und holt alles andere vom Webserver. Mit dem
  Routing schaltet Chromium seinen HTTP-Cache ab; die Zahl der Anfragen
  stimmt, Bytes und 304 nicht.
- **Fenster** 1600 × 900, `devicePixelRatio` 1, Chromium aus Playwright,
  headless. `zoom=-3`, die Mitte im Gebiet der Regionen.
- **Bewegung:** zehnmal 300 px ziehen, hin und her, je 600 ms Pause; dann
  fünfmal mit dem Mausrad eine Stufe hinaus und wieder hinein, je 800 ms.
  Gemessen nur während der Bewegung, nach dem Laden.
- **Werte:**
  - Rechnen: Stichproben des CPU-Profils mit 100 µs, die `ansichtJetzt`
    oder `zeichneAnsicht` auf dem Stapel haben. Nach 0123 rechnet
    `zeichneAnsicht` in Stücken; deren Stapel beginnt nach `await` neu,
    darum beide.
  - Lange Tasks aus `PerformanceObserver`.
  - Bildzeiten aus `requestAnimationFrame`.
  - Heap aus `performance.memory` am Ende.
- **Stände:** vorher `76ea0cc` (master, Basis des Zweigs), nachher
  `89c9e86`.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Die Stände abwechselnd, je drei
Läufe, 15 s Pause dazwischen. Skript `m7/messe.mjs` des Frontends, nicht im
Repo.

Ein Lauf schlug fehl: vorher, Lauf 1, 2D wartete nach dem Laden über 60 s
auf die erste Kachel. Von 2D vorher zählen darum zwei Läufe.

## Ergebnis

Mittel aus drei Läufen, dahinter die Spanne.

| | iso vorher | iso nachher | 2D vorher | 2D nachher |
|---|---|---|---|---|
| Rechnen der Ansichten | 3 828 ms (2 652–5 431) | 454 ms (412–520) | 20 ms | 6 ms |
| Lange Tasks, Summe | 3 952 ms (2 740–5 629) | 21 ms (0–63) | 0 | 192 ms (0–576) |
| Längste Aufgabe | 1 053 ms (924–1 271) | 21 ms (0–63) | 0 | 192 ms (0–576) |
| Bildzeit p99 | 78 ms (67–100) | 44 ms (33–50) | 42 ms | 44 ms |
| JS-Heap am Ende | 94 MB (50–123) | 83 MB (64–109) | 11 MB | 10 MB |
| Anfragen `heights` | 1 422–2 250 auf 496 Regionen | 340–363, je Region eine | 2 | 2 |
| Anfragen `ground` | 42–105 auf 29 Regionen | 0 | – | – |

Die eine lange Aufgabe in 2D nachher, Lauf 1, gehört nicht zu den Ebenen:
In diesem Lauf rechneten sie 9 ms; die Läufe 2 und 3 haben keine.

## Schluss

- **Das Blockieren war das Lag.** Die Caches der Höhen hielten weniger, als
  eine Ansicht braucht, und jede Ansicht lud und entpackte die Karten neu.
  Dazu rechnete jede Ansicht alle Ränder neu, mit einem Strahl je Punkt.
- **Gemerkt je Stufe, Caches nach Budget, die Probe billiger, Blöcke im
  Strahl:** Danach rechnet in dieser Bewegung nur noch das erste
  Hinauszoomen.
- **Offen:** Das Rechnen beim Zoomwechsel kostet rund 0,4 s, in Stücken
  ohne lange Aufgabe; ein Worker nähme es vom Hauptfaden. Das p99 von rund
  45 ms ist hier das Rauschen der Maschine.
