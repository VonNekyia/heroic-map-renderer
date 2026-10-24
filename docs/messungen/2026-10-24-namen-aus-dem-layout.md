---
title: Namen aus dem Layout
description: V4 aus dem Stresstest der Ebenen: ausgeblendete Namen ganz aus dem Layout statt nur unsichtbar, gemessen weit draussen mit 2000 Orten bei dpr 3 und CPU 4×, gegen master; dazu die Kontrolle mit sichtbaren Namen und zwei verworfene Anläufe, deren Szene fehlerhaft war.
date: 2026-10-24
commits: [97d5307]
code:
  - web/src/ebenen.ts
  - web/src/style.css
  - web/tests/messung-viele-orte.spec.ts
---

# Namen aus dem Layout

Weit draussen blendet die Karte die Namen nach
[0119](../entscheidungen/0119-namen-stufenlos-icons-fest.md) aus. Nimmt sie
sie danach ganz aus dem Layout (`display: none`), statt sie nur unsichtbar
zu lassen, fallen beim Ziehen die langen Aufgaben um 41 %, mit dem Rad
hinein um 61 %. Über die Schwelle hinaus kostet der Pinch 15 % mehr. Mit
sichtbaren Namen ändert sich nichts. Übernommen, so entschieden vom
Reviewer am 24.10.

## Aufbau

- **Szene:** `web/tests/messung-viele-orte.spec.ts`, Variante `fern`.
  - 1000 Banner und 1000 Nadeln mit Namen, 400 Blöcke auseinander, eine
    grosse Welt.
  - Grenzen ±1 048 576 px und `minZoom` −12, Start bei Zoom −10. Alle 2000
    liegen im Fenster, die Namen sind ausgeblendet: Beide Panes tragen
    `namen-weg`.
  - Dazu `voll` als Kontrolle: Zoom −4, Namen sichtbar.
- **Stände:**
  - master: `web/src` von `97d5307`.
  - V4: dazu `display: none` an `namen-weg` mit
    `transition-behavior: allow-discrete`. Übernommen ist ein anderer Weg,
    `namen-aus` nach der Dauer per JS; weit draussen ist der Zustand
    derselbe, `display: none`.
- **Browser:** Chromium aus Playwright, headless, 1280 × 800, dpr 3, CPU 4×,
  je Lauf ein frischer Kontext; je Stand ein `vite preview` an 127.0.0.1,
  Playwright mit eigener Konfiguration ohne `webServer`.
- **Werte:** wie in [Viele Nadeln und Banner](2026-10-24-viele-nadeln-und-banner.md),
  hinein und hinaus getrennt.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. `fern` in acht Runden, `voll`
in drei, je Runde beide Stände im Wechsel.

## Ergebnis

Median, lange Aufgaben in ms, in Klammern die Spanne:

| Ansicht | Stand | Läufe | Laden | p95 Ziehen | lang Ziehen | lang Rad hinein | lang Pinch hinein | lang Pinch hinaus |
|---|---|---|---|---|---|---|---|---|
| fern | master | 8 | 2 892 | 100 | 7 666 (6 773–8 051) | 2 278 | 3 862 | 2 297 |
| fern | V4 | 8 | 2 495 | **58** | **4 544** (3 463–7 245) | **896** | 3 628 | **2 647** |
| voll | master | 3 | 4 061 | 117 | 9 348 (8 952–10 050) | 858 | 896 | 1 058 |
| voll | V4 | 3 | 3 926 | 117 | 9 381 (9 267–10 128) | 884 | 889 | 1 040 |

- **Ziehen** weit draussen: −41 %, p95 von 100 auf 58 ms. Sieben von acht
  Läufen mit V4 liegen unter dem besten von master.
- **Rad hinein:** −61 %, die Spannen trennen sich ganz.
- **Pinch hinein:** gleich.
- **Pinch hinaus:** +15 %, sieben von acht Läufen über dem Median von
  master. Vermutet, nicht gemessen: Der Pinch kreuzt die Schwelle zum
  Ausblenden, und 2000 Namen wechseln zwischen `display: none` und sichtbar;
  der Browser legt sie neu aus.
- **Kontrolle `voll`:** kein Unterschied; dort sind die Namen sichtbar.

## Verworfene Anläufe

Zwei Anläufe vorher hingen; beide waren Fehler der Szene, kein Ergebnis.

- **Zoom −11 bei Grenzen ±65 536:** Der kleinste Zoom der Karte folgt aus
  den Grenzen (`fitZoom`), die Seite klemmte auf −8. Dort blenden die Namen
  noch nicht aus.
- **Grenzen ±1 048 576 ohne tiefe Pyramide:** Der Demobaum hat die Stufen 0
  bis 2. Weit draussen kachelte Leaflet das Fenster mit verkleinerten
  Kacheln der Stufe 0, Hunderttausenden Elementen, bis die Seite abstürzte.
  Eine echte Karte hat ihre Pyramide bis dorthin; die Szene hat jetzt
  `minZoom` −12.
