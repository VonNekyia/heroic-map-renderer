---
title: Viele Nadeln und Banner
description: Stresstest der Ebenen mit 1000 Bannern und 1000 Nadeln, dicht und weit, bei dpr 1 und 3, CPU 1× und 4×; Frames, lange Aufgaben, Heap, DOM-Knoten und Pixel der Leinwände beim Laden, Ziehen, Rad und Pinch, dazu eine Chrome-Spur des Ziehens und der Versuch, die Icons als Leinwand ohne Beschleunigung oder als <img> zu zeichnen.
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
nicht in JS, sondern in `Commit` und `Layerize`, 69 % des Hauptthreads.
Eigene Ebenen sind die Leinwände nicht, Chromium zeigt nur 4. Im Versuch
bringt eine Leinwand ohne Beschleunigung nichts, `<img>` für die Banner
−30 % p95; beides bleibt weit über 33 ms.

**Richtiggestellt am 24.10.:** „weit“ ist hier nicht weit. Am Start liegen
1834 der 2000 Orte im Fenster, 92 %, nicht 179. Die 179 waren die Zahl am
Ende, nach dem Pinch, und die Variante war daran geeicht. Alle Zahlen hier
gelten also für eine Ansicht, in der fast alles sichtbar ist. Die Reihe mit
wirklich weiter Ansicht folgt mit der Entscheidung zu Markern nur im
Ausschnitt.

## Aufbau

- **Szene:** gebaut von `web/tests/messung-viele-orte.spec.ts` auf dem
  Demobaum, iso 2:1. Zwei Ebenen: 1000 Banner mit `design`, Sprite 23 × 51
  aus einem Satz, Name im Bogen; 1000 Nadeln mit Symbol 16 × 16 und Namen.
  Jeder Ort mit Tafel, also bedienbar, und mit `y`, ohne Höhen.
- **Varianten:**
  - `dicht`: Raster 45 × 45 mit 4 Blöcken Abstand; am Start alle 2000 im
    Ausschnitt.
  - `weit`: 40 Blöcke Abstand, Zoom −4; am Start 1834 im Fenster, 92 %.
    Gedacht war eine weite Ansicht, siehe die Richtigstellung oben.
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

Am 24.10. unter `messung.lock`, ohne Spiel, an `vite preview`. Richtiggestellt:
Es war nicht das eigene, das an `::1` lauschte, sondern das, welches
Playwright mit `PW_PORT` selbst startete, mit einem Build des Arbeitsbaums.
Der war in dieser Reihe master, die Zahlen gelten also für master. Je
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
- **„dicht“ und „weit“ kosten fast gleich viel.** Beide haben fast alle
  Orte im Fenster, 2000 und 1834.
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

- **`Commit` und `Layerize`** nehmen zwei Drittel. Das sind die Schritte,
  in denen Chromium aus dem Gezeichneten die Ebenen bildet und an den
  Compositor übergibt.
- **Keine Ebene je Icon:** `LayerTree` zählt in jedem Fall 4 Ebenen,
  auch mit 2000 Leinwänden; siehe „Versuch V1 und V2“. Die Arbeit wächst
  also mit den Elementen und ihren Leinwänden, nicht mit den Ebenen. Jedes
  Icon setzt Leaflet mit eigener Transformation; dass das der Treiber ist,
  ist vermutet, nicht gemessen.
- **JS** ist klein, fast nur Leaflet, darunter `_updateOpacity` der
  Kacheln.

## Versuch V1 und V2

Zwei kleine Wege, gemessen in „weit“ und „dicht“ bei dpr 3 und CPU 4×, je
drei Läufe gegen master, in derselben Ansicht wie oben mit fast allen Orten
im Fenster. Auch hier lief der Build des Arbeitsbaums, den Playwright
startete; die Wege schaltete die Adresse, `?versuch=`. Der Code liegt unter dem Tag
`versuch/icons-leinwand-img`, siehe
[Icons als Leinwand oder Bild](../versuche/icons-leinwand-oder-bild.md).

- **V1:** die Leinwand mit `getContext('2d', { willReadFrequently: true })`,
  also ohne Beschleunigung.
- **V2:** Banner als `<img>` mit der Adresse ihres Sprites, Nadeln weiter
  als Leinwand. Die CSP erlaubt Bilder nur von `'self'`; eine Nadel wird
  eingefärbt gezeichnet und hat keine Adresse.

Median aus drei Läufen, am selben Tag nach der Reihe oben, die Wege
nacheinander: erst master, dann V1, dann V2, je 7 min. master liegt hier
höher als oben; verglichen wird nur innerhalb dieser Tabelle.

| Weg | Variante | p95 Ziehen | lang Ziehen | p95 Pinch | lang Pinch | Leinwand MiB |
|---|---|---|---|---|---|---|
| master | weit | 167 | 11 420 (412) | 466 | 2 672 | 66,3 |
| V1 | weit | 150 | 11 165 (417) | 433 | 2 923 | 66,3 |
| V2 | weit | **117** | **9 305** (439) | 467 | 2 841 | **26,1** |
| master | dicht | 167 | 12 908 (477) | 567 | 2 936 | 66,3 |
| V1 | dicht | 167 | 11 884 (442) | 517 | 3 606 | 66,3 |
| V2 | dicht | **117** | **8 985** (407) | 500 | 3 313 | **26,1** |

Dazu je ein Lauf mit Spur und `LayerTree` in „weit“. `LayerTree` bremst
selbst; die Zeiten gelten nur untereinander.

| Weg | Ebenen | alle Aufgaben ms | `Commit` | `Layerize` | p95 Ziehen |
|---|---|---|---|---|---|
| master | 4 | 15 623 | 5 858 | 4 773 | 183 |
| V1 | 4 | 14 762 | 5 536 | 4 536 | 183 |
| V2 | 4 | 11 017 | 2 728 | 4 003 | 100 |
| V1 + V2 | 4 | 11 649 | 2 773 | 4 351 | 117 |

- **V1 bringt nichts,** innerhalb des Rauschens. Der Hinweis
  `willReadFrequently` ändert an den Ebenen nichts, es bleiben 4.
- **V2** halbiert `Commit`, senkt p95 beim Ziehen um 30 % und die Leinwand
  um die 40 MiB der Banner. Der Pinch bleibt gleich.
- **Beide bleiben über 33 ms.** Auch V1 + V2 zusammen kommt nicht darunter.
  Nach dem Plan ist damit V3 dran, nur Marker im Ausschnitt; dafür kommt
  zuerst ein Plan.
- **Eine erste Reihe zu V3** ist verworfen: Alle ihre Läufe trafen denselben
  Build.

## Nebenbei gerechnet

- **C3, `api/sicht.json` mit ETag und 304: gerechnet, verworfen.**
  - Das Plugin schreibt die Datei ohne Bewegung alle 5 s neu, mit neuem
    `jetzt` als Herzschlag. Die Karte fragt alle 2 s. Ohne Bewegung wären so
    höchstens etwa 60 % der Abrufe ein 304, mit Bewegung fast keiner.
  - Gespart würde nur der Körper, rund 100 B je Spieler: bei 20 Spielern
    etwa 0,6 KB/s je Zuschauer.
  - Dafür müssten ETag und 304 neben das `no-store` aus
    [0107](../entscheidungen/0107-aktionsebenen.md). Das lohnt sich nicht.
