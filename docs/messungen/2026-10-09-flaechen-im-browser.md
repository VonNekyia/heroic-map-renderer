---
title: Flächen im Browser
description: Was ein Kreis mit 2000 Blöcken Radius auf der Webkarte kostet, nur mit Rand, gefüllt und gefüllt mit Tafel und Schrift, von oben und im iso; Zeit bis zum Pfad, JS-Heap nach der Speicherbereinigung, Spitze beim Laden und Punkte, in Chromium unter Playwright.
date: 2026-10-09
commits: [a1f6995]
code:
  - web/src/gelaende.ts
  - web/src/formen.ts
  - web/src/ebenen.ts
  - web/tests/messung-flaechen.spec.ts
---

# Flächen im Browser

Ein gefüllter Kreis mit 2000 Blöcken Radius steht im iso nach rund 0,66 s
auf der Karte. Er hält danach 11,1 MiB im JS-Heap und hat 73 886 Punkte; die
Spitze beim Laden liegt bei 12 bis 26 MiB. Nur mit Rand sind es rund
0,16 s und 1,6 MiB, von oben unter 20 ms und unter 1 MiB. Mit Tafel und
Schrift bleibt der Heap gleich: Die Höhen fallen nach dem Zeichnen weg.

## Aufbau

- **Stand:** `a1f6995`, Runde 2 der PR zu #219, Teil 4. Gebaut von
  Playwright wie die Tests, `vite build` und `vite preview`.
- **Browser:** Chromium aus Playwright, headless, Fenster 1280 × 720.
- **Welt:** der Demobaum mit Höhen und Ebenen, die das Skript liefert,
  `heightsCell` 4, `seaLevel` 0, ohne `area`. Das Gelände ist hügelig,
  `64 + 30 · sin(i/7) · cos(j/9)` je Zelle, so hat „verdeckt“ zu tun.
- **Form:** ein Kreis um (−2500, −2500) mit 2000 Blöcken Radius, Rand 2 px.
  - **Rand:** nur der Rand;
  - **gefüllt:** dazu `fill` mit Alpha;
  - **gefüllt, Tafel, Schrift:** dazu `name`, eine Tafel und ein Schriftzug
    mit 40 Blöcken Höhe.
- **Ansichten:** von oben (`top`, genordet, 16 px je Block) und iso 2:1,
  scale 16. Blick auf die Mitte des Kreises, sechs Stufen unter der
  feinsten.
- **Befehl:**

  ```bash
  MESSUNG=1 MESSUNG_LAST=20 npx playwright test --project=grund tests/messung-flaechen.spec.ts
  ```

## Ablauf

- Je Ansicht die drei Formen abwechselnd, je drei Läufe. Vor jedem Lauf
  lädt die Seite neu, die Wahl der Ebenen ist gelöscht, die Ebene aus.
- **Last:** Vor jedem Lauf wartet das Skript, bis die Last der Maschine
  unter 20 % liegt. Die Grundlast ohne die Sitzungen lag bei 15 bis 17 %,
  von einem Chat-Programm und einer Treibersoftware im Hintergrund. Die
  Grenze von 20 % statt 10 % hat der Reviewer für diese Reihe festgelegt,
  wie für den Benchmark am selben Abend. Vor den Läufen gemessen: 1 bis
  16 %.
- **Zeit:** im Browser von dem Klick, der die Ebene einschaltet, bis ein
  `MutationObserver` den Pfad mit `d` sieht: gefüllt die Fläche, sonst der
  Rand. Darin: Datei der Ebene laden, Höhen laden, rechnen, zeichnen.
- **Heap:** `Runtime.getHeapUsage` über das Chrome DevTools Protocol, vor
  und nach dem Lauf je nach `HeapProfiler.collectGarbage`. Die Spitze:
  derselbe Wert alle 50 ms während des Laufs, ohne Bereinigung.
- **Punkte:** die Befehle `M` und `L` aller Pfade der Ebenen auf dem Schirm.
  Leaflet vereinfacht Ränder je Stufe, Flächen nicht.
- Zahlen aus der Ausgabe des Skripts vom 09.10.2026, 20:33 bis 20:35.
  Ein Lauf davor hing bei der zweiten Form: Ohne gelöschte Wahl stand die
  Ebene schon an, und der Klick schaltete sie aus. Das Skript löscht die
  Wahl jetzt vor jedem Laden.

## Ergebnis

Je drei Läufe. Heap als Zuwachs gegen vor dem Lauf, in MiB.

| Ansicht | Form | Zeit in ms | Heap danach | Spitze beim Laden | Punkte |
|---|---|---|---|---|---|
| von oben | Rand | 11, 11, 15 | 0,5, 0,4, 0,4 | 0,1, 0,1, 0,1 | 57 |
| von oben | gefüllt | 15, 14, 18 | 0,8, 0,8, 0,8 | 0,1, 0,1, 0,1 | 2 175 |
| von oben | gefüllt, Tafel, Schrift | 17, 19, 16 | 0,7, 0,8, 0,8 | 0,0, 0,1, 0,1 | 2 177 |
| iso 2:1 | Rand | 174, 162, 156 | 1,6, 1,6, 1,6 | 8,0, 0,9, 1,8 | 400 |
| iso 2:1 | gefüllt | 678, 661, 635 | 11,1, 11,1, 11,1 | 12,5, 20,9, 25,9 | 73 886 |
| iso 2:1 | gefüllt, Tafel, Schrift | 672, 689, 697 | 11,2, 11,2, 11,2 | 25,7, 21,7, 12,1 | 74 389 |

### Nach B1, zwei Arrays statt drei

Derselbe Aufbau, Stand nach `a1a94b8` mit zwei statt drei `Float64Array` in
`sichtbareFelder`, gemessen am 09.10.2026, 20:39 bis 20:40.

| Ansicht | Form | Zeit in ms | Heap danach | Spitze beim Laden | Punkte |
|---|---|---|---|---|---|
| von oben | Rand | 11, 15, 17 | −0,1, 0,4, 0,4 | 0,0, 0,1, 0,1 | 57 |
| von oben | gefüllt | 20, 20, 22 | 0,8, 0,1, 0,8 | 0,1, 0,0, 0,1 | 2 175 |
| von oben | gefüllt, Tafel, Schrift | 23, 20, 21 | 0,8, 0,8, 0,8 | 0,1, 0,1, 0,1 | 2 177 |
| iso 2:1 | Rand | 186, 183, 181 | 1,6, 1,6, 1,6 | 6,3, 7,9, 5,1 | 400 |
| iso 2:1 | gefüllt | 747, 731, 733 | 11,1, 11,1, 11,1 | 25,7, 25,6, 25,6 | 73 886 |
| iso 2:1 | gefüllt, Tafel, Schrift | 752, 750, 771 | 11,2, 11,2, 11,2 | 27,3, 26,5, 11,7 | 74 389 |

- Die Zeiten liegen 10 % über der ersten Reihe, auch von oben, wo
  `sichtbareFelder` nicht läuft. Das ist Streuung zwischen zwei Reihen, kein
  Unterschied des Codes; verglichen wird nur innerhalb einer Reihe.
- Die Spitze sinkt nicht messbar: Sie streut mit der Speicherbereinigung um
  mehr als die 8 bis 9 MB des dritten Arrays.
- **Höhenkarten zählen,** wie im Review vorgeschlagen, ging nicht:
  `Runtime.queryObjects` fand in diesem Chromium weder `Int16Array` noch
  `ArrayBuffer`, auch nicht zwei Proben, die der Test selbst anlegte. Ob die
  Höhen wegfallen, zeigt darum der Heap: Hielte die Ebene sie fest, lägen
  die 17 Regionen über den 64 des Caches mit rund 0,5 MiB mehr im Heap. Mit
  Tafel und Schrift sind es in beiden Reihen 0,1 MiB.

## Schluss

- **Tragbar:** Eine Fläche dieser Grösse kostet im iso einmal je
  `version` rund 0,7 s und hält 11 MiB. Gerechnet wird nur beim Laden einer
  Ebene, ein Zoom projiziert nur neu.
- **Die Punkte** wachsen mit dem Umfang des Sichtbaren, nicht mit der Fläche:
  73 886 statt geschätzt bis zu 1,5 Mio. für Felder je Reihe. Auf dem
  hügeligen Gelände trägt der Rand des Verdeckten einen grossen Teil dazu.
- **Die Höhen fallen weg:** 81 Regionen sind rund 2,5 MiB. Mit Tafel und
  Schrift, deren Closures sie früher festhielten, liegt der Heap danach nur
  0,1 MiB höher als ohne, das ist die Tafel selbst und die Schrift.
- **Die Spitze** streut mit der Speicherbereinigung des Browsers, 12 bis
  26 MiB im iso gefüllt.
- Eine Grenze für gefüllte Flächen im Format braucht es danach nicht.
