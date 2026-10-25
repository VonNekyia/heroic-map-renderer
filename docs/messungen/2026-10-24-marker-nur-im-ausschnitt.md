---
title: Marker nur im Ausschnitt
description: master gegen Marker nur im Fenster mit Rand (V3, 0128) und V3 mit Bannern als <img> (V2), mit 1000 Bannern und 1000 Nadeln bei dpr 3 und CPU 4×, in einer weiten Ansicht mit 9 % der Orte im Fenster, einer vollen mit 92 % und einer dichten mit allen; Frames und lange Aufgaben beim Laden, Ziehen, Rad und Pinch, hinein und hinaus getrennt.
date: 2026-10-24
commits: [d25e1f0, 53ad53c]
code:
  - web/src/ebenen.ts
  - web/tests/messung-viele-orte.spec.ts
---

# Marker nur im Ausschnitt

In einer weiten Ansicht, 179 der 2000 Orte im Fenster, fällt p95 beim
Ziehen bei dpr 3 und CPU 4× von 117 auf 33 ms, wenn nur Marker im Fenster
mit Rand im DOM stehen. Die langen Aufgaben beim Ziehen fallen von 9,2 s auf
0,6 s, Rad und Pinch von rund 500 auf 33 bis 67 ms, das Laden von 3,9 auf
1,3 s. Mit Bannern als `<img>` dazu halbiert sich p95 noch einmal. Liegen
fast alle Orte im Fenster, hilft das Ausdünnen beim Ziehen nicht.

## Aufbau

- **Szene:** wie in
  [Viele Nadeln und Banner](2026-10-24-viele-nadeln-und-banner.md), „Aufbau“,
  mit `web/tests/messung-viele-orte.spec.ts`. Je 1000 Banner und Nadeln mit
  Namen und Tafel, iso 2:1, ohne Kacheln.
- **Ansichten,** Raster 45 × 45:
  - `weit`: 40 Blöcke Abstand, Zoom −2; am Start 179 im Fenster, 9 %.
  - `voll`: 40 Blöcke Abstand, Zoom −4; 1834 im Fenster, 92 %. Das ist das
    „weit“ der ersten Reihe.
  - `dicht`: 4 Blöcke Abstand, Zoom −4; alle 2000 im Fenster.
- **Stände:**
  - master: `web/src` von `d25e1f0`.
  - V3: Marker nur im Ausschnitt nach
    [0128](../entscheidungen/0128-marker-nur-im-ausschnitt.md), `53ad53c`.
  - V3 + V2: dazu Banner als `<img>`, der Versuch unter dem Tag
    `versuch/icons-leinwand-img`, geschaltet mit `?versuch=v2`.
- **Browser:** Chromium aus Playwright, headless, 1280 × 800, dpr 3, Touch,
  CPU 4× über CDP.
- **Abläufe je Lauf:** Laden, bis die Zahl der Icons 1 s steht; Ziehen
  480 × 180 px in 60 Schritten; Rad drei Rasten hinein, dann drei hinaus;
  Pinch ×2, dann ×0,5. Jeder Abschnitt reicht bis 600 ms nach seinem Ende.
- **Werte:** wie in der ersten Reihe. Dazu die Icons im Fenster am Start und
  die im DOM nach dem Laden. Die Leinwand zählt nur Icons im DOM am Ende;
  gebaute, aber ausgehängte Marker behalten ihre Leinwand und fehlen in der
  Zahl.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Je Stand ein Build, je ein
`vite preview` an 127.0.0.1. Playwright lief mit einer eigenen
Konfiguration ohne `webServer`, damit sicher der ausgelieferte Build
antwortet. Drei Runden, je Runde jeder Stand einmal in jeder Ansicht, 13
min.

## Ergebnis

Median aus drei Läufen. „lang“: Summe der langen Aufgaben, in Klammern die
längste. p95 der Frames in ms.

| Ansicht | Stand | im Fenster | im DOM geladen | Laden ms | p95 Ziehen | lang Ziehen | p95 Rad hinein / hinaus | p95 Pinch hinein / hinaus | längste Rad / Pinch hinaus | Leinwand MiB |
|---|---|---|---|---|---|---|---|---|---|---|
| weit | master | 179 | 2 000 | 3 875 | 117 | 9 249 (660) | 467 / 467 | 483 / 550 | 490 / 502 | 66,3 |
| weit | **V3** | 179 | **387** | **1 266** | **33** | **593 (124)** | **33 / 33** | **67 / 67** | **68 / 68** | **12,5** |
| weit | V3 + V2 | 179 | 387 | 1 102 | 17 | 187 (66) | 33 / 33 | 50 / 66 | 64 / 53 | 4,7 |
| voll | master | 1 834 | 2 000 | 4 986 | 167 | 11 621 (417) | 17 / 17 | 433 / 517 | 63 / 500 | 66,3 |
| voll | V3 | 1 834 | 2 000 | 4 590 | 167 | 11 916 (654) | 17 / 17 | 200 / 166 | 0 / 150 | 39,6 |
| voll | V3 + V2 | 1 834 | 2 000 | 3 527 | 100 | 8 815 (379) | 17 / 17 | 167 / 167 | 0 / 154 | 15,4 |
| dicht | master | 2 000 | 2 000 | 5 244 | 167 | 12 044 (447) | 33 / 17 | 467 / 517 | 60 / 509 | 66,3 |
| dicht | V3 | 2 000 | 2 000 | 4 545 | 167 | 12 002 (734) | 50 / 17 | 450 / 300 | 60 / 287 | 55,2 |
| dicht | V3 + V2 | 2 000 | 2 000 | 3 720 | 100 | 8 594 (335) | 33 / 17 | 367 / 283 | 0 / 267 | 25,0 |

- **„weit“ entscheidet,** so sieht eine echte Karte aus. master bleibt dort
  über der Schwelle von 33 ms, V3 liegt mit 33 ms genau darauf.
- **Hinauszoomen** mit V3: Die längste Aufgabe dauert 68 ms, obwohl der Bau
  in Stücken läuft. Ein Teil davon ist Leaflets eigenes Zoomen.
- **„voll“ und „dicht“:** Beim Ziehen bringt V3 nichts, alle Orte stehen
  ohnehin im DOM. Der Pinch wird in „voll“ billiger, weil er hinein den
  Rahmen verkleinert.
- **V2 dazu** senkt p95 beim Ziehen in jeder Ansicht weiter, in „weit“ auf
  17 ms, in „voll“ von 167 auf 100 ms.
- **„lang Ziehen“** zählt bei master und in „voll“ und „dicht“ mehr als die
  2 s, die das Ziehen ohne Bremse dauert. Der Abschnitt dauert unter der
  Bremse länger; vermutlich nimmt Playwright jeden Schritt erst an, wenn
  die Seite ihn verarbeitet hat.
