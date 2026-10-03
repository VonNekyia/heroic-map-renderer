---
title: Skin Tablett
description: Was der Skin Tablett kostet - Bündel ohne und mit Skin gegen den Stand vor #112, das einmalige Zeichnen für fitZoom, die Bildzeit beim Ziehen mit CPU 1× und 4× und Lighthouse am Build mit Skin.
date: 2026-10-03
commits: [08a50c9]
code:
  - web/src/main.ts
  - web/vite.config.ts
  - web/skins/tablett/index.ts
---

# Skin Tablett

Ohne Skin ist das Bündel 64 Byte grösser als vor #112 (161 676 statt
161 612 Byte JS). Mit Skin zeichnet er einmal in 7,4 bis 8,8 ms, bei CPU 4×
in 33 bis 38 ms. Beim Ziehen fällt kein Bild aus, auch bei CPU 4× nicht;
die Eingabe kostet dort mit Skin aber rund 9 ms mehr je Bewegung der Maus.

## Aufbau

- Stände: `08a50c9`, der letzte Stand vor #112; der Zweig dieser PR ohne
  `SKIN` und mit `SKIN=./skins/tablett`, gebaut mit `npx vite build`
  (Vite 8.3.0).
- Für die Zeiten: die Previews beider Builds auf dem Rechner des Frontends,
  der Demobaum aus `web/public/tiles-demo` mit `seaLevel` 0 und `area`
  `[-64, -64, 64, 64]` per Route in `map.json`, ein Fenster von
  1491 × 1055, Chromium von Playwright, kopflos. Die Drossel per CDP,
  `Emulation.setCPUThrottlingRate`.
- Das Messskript lag lokal, nicht im Repository.

## Ablauf

- **Bündel:** ein Build je Stand, die Dateien unter `assets/` gezählt, gzip
  mit `gzip -9`. Builds streuen nicht. Gezählt am Stand mit `skin?.()` in
  `start`; die Zeiten davor, ohne diese zwei Byte.
- **Zeichnen:** die Dauer von `performance.measure('tablett: zeichnen')` im
  Skin, nach dem Laden der Seite.
- **Ziehen:** 120 Bewegungen der Maus mit gedrückter Taste, je 2 und 1,2
  Pixel, 16 ms auseinander. Gemessen die Abstände zwischen den Aufrufen von
  `requestAnimationFrame` und die Dauer der ganzen Bewegung.
- Ohne und mit Skin abwechselnd, je drei frische Seiten, die Sperrdatei
  gelegt. Fremde Last 11 bis 26 % (`LoadPercentage`), andere Sitzungen
  liefen weiter.
- **Gegenversuche** bei CPU 4×, nur mit Skin: die Leinwände vor dem Ziehen
  per `display: none` verborgen; `image-rendering: auto`; `will-change:
  transform`.
- **Lighthouse:** wie in [CI](../entwicklung/ci.md), „Lighthouse“, aber
  gegen `web/dist-skin` mit dem Demobaum als `tiles/` und `seaLevel` und
  `area` in seiner `map.json`, drei Läufe.

## Ergebnis

Bündel, Byte:

| Stand | JS der Karte | gzip | JS des Skins | gzip | CSS des Skins |
|---|---|---|---|---|---|
| vor #112, `08a50c9` | 161 612 | 48 038 | | | |
| ohne Skin | 161 676 | 48 055 | | | |
| mit Skin | 163 477 | 48 860 | 8 464 | 3 935 | 193 |

Das CSS der Karte ist in allen drei Ständen dieselbe Datei, 16 306 Byte.
Ohne Skin entsteht kein Stück für `virtual:skin`. Mit Skin wächst das
Bündel der Karte um 1 801 Byte, vor allem um den Helfer, mit dem Vite einen
dynamischen Import samt CSS lädt. Texturen: keine, 0 KB.

Zeichnen, ms, zwei Reihen:

| CPU | Läufe | Median |
|---|---|---|
| 1× | 8,8 · 7,6 · 8,1 · 7,5 · 7,4 · 7,6 | 7,6 |
| 4× | 33,2 · 37,3 · 36,5 · 38,4 · 35,6 · 36,3 | 36,4 |

Ziehen, zweite Reihe. Abstand der Bilder in ms, Median und p95 je Lauf;
Dauer der Bewegung in ms:

| CPU | Stand | Median | p95 | längstes Bild | Dauer |
|---|---|---|---|---|---|
| 1× | ohne | 16,7 | 16,7–16,8 | 16,8–33,3 | 4 212 · 4 095 · 4 137 |
| 1× | mit | 16,7 | 16,7–16,8 | 16,8 | 4 232 · 4 258 · 4 248 |
| 4× | ohne | 16,7 | 16,7 | 16,8 | 4 498 · 4 503 · 4 383 |
| 4× | mit | 16,7 | 16,7–16,8 | 16,8–50,1 | 5 744 · 5 476 · 5 501 |

Die erste Reihe zeigt dieselben Bildzeiten. Ein einzelnes langes Bild kam
einmal ohne und einmal mit Skin vor.

Gegenversuche bei CPU 4×, Dauer der Bewegung in ms:

| Variante | mit Skin | ohne Skin in derselben Reihe |
|---|---|---|
| Leinwände verborgen | 4 426 · 4 354 · 4 368 | 4 483 · 4 378 · 4 497 |
| `image-rendering: auto` | 5 388 · 5 375 · 5 302 | |
| `will-change: transform` | 6 277 · 5 529 · 5 434 | |

Lighthouse mit Skin: Performance 0,97 · 0,99 · 1, Barrierefreiheit, Best
Practices und SEO je 1, CLS 0, TBT 164 · 66 · 33 ms, LCP rund 1,69 s. Alle
Schwellen aus `web/lighthouserc.cjs` halten.

## Schluss

- Ohne Skin bleibt das Bündel, wie es vor #112 war. Die 64 Byte stecken in
  `start` und `fitZoom`, die jetzt mit den Grenzen der ganzen Karte statt
  mit `bounds` rechnen; der Aufruf des Skins fällt weg.
- Das Zeichnen kostet einmal unter 10 ms, bei CPU 4× unter 40 ms, und nur
  beim Laden und bei einer neuen Fenstergrösse.
- Beim Ziehen hält die Karte mit Skin 60 Bilder je Sekunde, auch bei CPU 4×.
- Die Mehrarbeit beim Ziehen, bei CPU 4× rund (5 574 − 4 461) / 120 ≈ 9 ms
  je Bewegung, kommt von den beiden sichtbaren Leinwänden: Verborgen ist
  sie weg. Pixelig oder nicht und `will-change` ändern daran nichts. Bei
  CPU 1× sind es rund 0,8 ms. Kleiner würde sie mit kleineren Bildern, also
  weniger Rand über das Fenster hinaus.
