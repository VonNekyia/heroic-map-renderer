---
title: Viele Nadeln und Banner
description: Stresstest der Ebenen mit 1000 Bannern und 1000 Nadeln, dicht und weit, bei dpr 1 und 3, CPU 1× und 4×; Frames, lange Aufgaben, Heap, DOM-Knoten und Pixel der Leinwände beim Laden, Ziehen, Rad und Pinch, dazu eine Chrome-Spur des Ziehens.
date: 2026-10-24
commits: [2731158]
code:
  - web/src/ebenen.ts
  - web/tests/messung-viele-orte.spec.ts
---

# Viele Nadeln und Banner

Mit 2000 Orten ruckelt die Karte beim Ziehen und beim Pinch, schon bei
CPU 1×. In der Zelle, an der wir entscheiden, „weit“ bei dpr 3 und CPU 4×,
liegt p95 beim Ziehen bei 150 ms, und die Leinwände der Icons belegen
66 MiB. Alle drei Schwellen aus dem Plan sind überschritten. Die Zeit geht
nicht in JS, sondern in den Compositor: Jede Leinwand ist eine eigene
Ebene, `Commit` und `Layerize` nehmen 69 % des Hauptthreads.

## Aufbau

- **Szene:** gebaut von `web/tests/messung-viele-orte.spec.ts` auf dem
  Demobaum, iso 2:1. Zwei Ebenen: 1000 Banner mit `design`, Sprite 23 × 51
  aus einem Satz, Name im Bogen; 1000 Nadeln mit Symbol 16 × 16 und Namen.
  Jeder Ort mit Tafel, also bedienbar, und mit `y`, ohne Höhen.
- **Varianten:**
  - `dicht`: Raster 45 × 45 mit 4 Blöcken Abstand; am Start alle 2000 im
    Ausschnitt.
  - `weit`: 40 Blöcke Abstand; am Start 179 im Ausschnitt, 9 %. So sieht
    es auf einer echten Karte aus.
  - `weit-ohne-namen`: wie `weit`, die Banner ohne Namen.
- **Ohne Kacheln:** Jede Kachel ist 404. Es zählt nur, was die Orte kosten.
- **Browser:** Chromium aus Playwright, headless, 1280 × 800, je Lauf ein
  frischer Kontext mit `deviceScaleFactor` 1 oder 3 und Touch. CPU-Bremse
  über CDP, `Emulation.setCPUThrottlingRate`.
- **Abläufe je Lauf:**
  1. Laden bis 2000 Icons im DOM.
  2. Ziehen 480 × 180 px in 60 Schritten zu 33 ms.
  3. Rad: drei Stufen hinein, drei hinaus.
  4. Pinch per CDP `Input.synthesizePinchGesture`, ×2 und ×0,5.
- **Werte:**
  - Frames: Abstände der `requestAnimationFrame`.
  - Lange Aufgaben aus `PerformanceObserver`. Ein Abschnitt reicht bis 600 ms
    nach seinem Ende.
  - JS-Heap nach GC, DOM-Knoten aus `Memory.getDOMCounters`.
  - Leinwand: Summe Breite · Höhe · 4 aller Leinwände der Icons.
  - „Im Ausschnitt“ zählt am Ende, nach dem Pinch. Unter der Bremse endet
    der Pinch weiter draussen, darum stehen dort mehr.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel, am eigenen `vite preview`. Je
Zelle drei Läufe hintereinander, 13 min. Danach eine Chrome-Spur nur des
Ziehens (`SPUR`), drei Läufe in der Zelle „weit, dpr 3, CPU 4×“. Die Spur
bremst selbst, ihre Zeiten liegen darum über denen der Reihe.

## Ergebnis

Median aus drei Läufen. „lang“: Summe der langen Aufgaben im Abschnitt, in
Klammern die längste. p95 der Frames in ms.

| Variante | dpr | CPU | Laden ms | lang Laden | p95 Ziehen | lang Ziehen | p95 Rad | p95 Pinch | lang Pinch | Leinwand MiB | DOM-Knoten |
|---|---|---|---|---|---|---|---|---|---|---|---|
| dicht | 1 | 1× | 1 320 | 883 | 83 | 519 (256) | 17 | 67 | 520 | 7,4 | 15 106 |
| dicht | 1 | 4× | 5 384 | 4 450 | 150 | 10 810 (511) | 33 | 533 | 3 081 | 7,4 | 15 215 |
| dicht | 3 | 1× | 1 325 | 965 | 67 | 594 (269) | 17 | 67 | 488 | 66,3 | 15 106 |
| dicht | 3 | 4× | 4 859 | 4 115 | 167 | 11 079 (398) | 17 | 550 | 3 069 | 66,3 | 15 215 |
| weit | 1 | 1× | 1 171 | 812 | 50 | 380 (208) | 17 | 67 | 277 | 7,4 | 15 119 |
| weit | 1 | 4× | 4 405 | 3 704 | 133 | 9 990 (364) | 17 | 433 | 2 457 | 7,4 | 15 215 |
| weit | 3 | 1× | 1 309 | 939 | 50 | 544 (294) | 17 | 67 | 386 | 66,3 | 15 119 |
| **weit** | **3** | **4×** | **4 814** | **4 070** | **150** | **11 082 (395)** | **17** | **450** | **2 927** | **66,3** | **15 215** |
| weit-ohne-namen | 1 | 1× | 871 | 637 | 33 | 548 (255) | 17 | 50 | 76 | 7,4 | 9 118 |
| weit-ohne-namen | 1 | 4× | 3 668 | 2 983 | 83 | 6 237 (260) | 17 | 350 | 2 368 | 7,4 | 9 240 |
| weit-ohne-namen | 3 | 1× | 1 142 | 821 | 50 | 317 (134) | 17 | 50 | 239 | 66,3 | 9 118 |
| weit-ohne-namen | 3 | 4× | 4 181 | 3 454 | 133 | 8 437 (382) | 17 | 383 | 2 644 | 66,3 | 9 227 |

Der JS-Heap liegt überall bei 9,3 bis 9,6 MB.

- **Schwellen** aus dem Plan, in der Zelle „weit, dpr 3, CPU 4×“:
  - p95 über 33 ms beim Ziehen oder Zoomen: Ziehen 150 ms, Pinch 450 ms.
  - Lange Aufgaben über 200 ms: die längste beim Ziehen 395 ms.
  - Leinwand über 64 MiB: 66,3 MiB.

  Alle drei sind überschritten.
- **Das Rad** bleibt glatt, p95 17 ms, auch unter der Bremse.
- **„dicht“ und „weit“ kosten fast gleich viel,** obwohl am Start 2000 statt
  179 Orte im Ausschnitt liegen. Es zählt, wie viele im DOM stehen, nicht wie
  viele man sieht.
- **Die Namen** kosten rund ein Drittel der DOM-Knoten, 15 119 gegen 9 118,
  und beim Ziehen unter der Bremse rund 25 % der langen Aufgaben.
- **Die Leinwand** ist von der Variante unabhängig und wächst mit k². k =
  max(1, round(dpr)), bei dpr 3 also 9 × 7,4 MiB.

## Chrome-Spur des Ziehens

Hauptthread, „weit, dpr 3, CPU 4×“, ein Lauf von dreien, die anderen
gleich verteilt:

| Ereignis | ms | Anteil |
|---|---|---|
| alle Aufgaben (`RunTask`) | 15 916 | 100 % |
| `Commit` | 5 886 | 37 % |
| `Layerize` | 4 971 | 31 % |
| JS (`FunctionCall`) | 932 | 6 % |
| `UpdateLayoutTree` | 728 | 5 % |
| `HitTest` | 642 | 4 % |
| `Paint` | 448 | 3 % |
| `Layout` | 440 | 3 % |

- **Der Compositor** nimmt zwei Drittel. Jede Leinwand eines Icons ist in
  Chromium eine eigene Ebene, und jede Bewegung baut die Liste der Ebenen
  neu und übergibt sie.
- **JS** ist klein, fast nur Leaflet, darunter `_updateOpacity` der
  Kacheln.
- **Folgerung:** Den grössten Hebel hat, was die Zahl der Ebenen senkt.
  Weniger Marker im DOM tut das, oder Icons, die keine eigene Ebene
  sind, etwa `<img>` mit einem Sprite für alle oder eine Leinwand ohne
  Beschleunigung. Welcher Weg, entscheidet ein Versuch.

## Nebenbei gerechnet

- **C3, `api/sicht.json` mit ETag und 304: gerechnet, verworfen.**
  - Das Plugin schreibt die Datei ohne Bewegung alle 5 s neu, mit neuem
    `jetzt` als Herzschlag. Die Karte fragt alle 2 s. Ohne Bewegung wären so
    höchstens etwa 60 % der Abrufe ein 304, mit Bewegung fast keiner.
  - Gespart würde nur der Körper, rund 100 B je Spieler: bei 20 Spielern
    etwa 0,6 KB/s je Zuschauer.
  - Dafür müssten ETag und 304 neben das `no-store` aus
    [0107](../entscheidungen/0107-aktionsebenen.md). Das lohnt sich nicht.
