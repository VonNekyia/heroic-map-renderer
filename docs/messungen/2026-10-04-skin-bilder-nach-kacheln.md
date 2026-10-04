---
title: Bilder des Skins nach den Kacheln
description: Lighthouse mit dem Skin Tablett, wenn seine Bilder beim Start laden, nach `load` der Kacheln, zwei Bilder danach oder nach der Meldung, dass eine Kachel als grösstes Element gemalt ist; dazu die Grösse der Bilder.
date: 2026-10-04
commits: [d7a8362]
code:
  - web/skins/tablett/index.ts
  - web/lighthouserc.cjs
---

# Bilder des Skins nach den Kacheln

Laden die Bilder des Skins erst, wenn der Browser eine Kachel als grösstes
Element gemalt meldet, sinkt der LCP mit Skin von 5,5 s auf 1,61 bis
1,69 s, Performance von 0,79 auf 0,99. Nach `load` der Kacheln allein bleibt
er bei 5,5 s. Zwei Bilder nach `load` reichen nur, wenn das Malen früh
kommt: am Zweig von #120 ja, mit der UI aus #135 nicht.

## Aufbau

- Wie in [Skin Tablett](2026-10-03-skin-tablett.md), „Lighthouse“: Lighthouse
  12.6.1 wie in [CI](../entwicklung/ci.md), „Lighthouse“, gegen
  `web/dist-skin` mit dem Demobaum als `tiles/` und `seaLevel` 0 und `area`
  `[-64, -64, 64, 64]` in seiner `map.json`, drei Läufe je Stand, Chromium
  von Playwright.
- Lighthouse drosselt simuliert (`simulate`, 150 ms RTT, 1,6 Mbit/s, CPU 4×).
  Den LCP rechnet es aus den Anfragen, die vor dem gemalten grössten
  Element beginnen; die Last des Rechners ändert ihn kaum, wohl aber, wann
  gemalt wird. Ohne Sperrdatei.
- Stände:
  - **beim Start:** `d7a8362`, die Bilder laden mit dem ersten Zeichnen;
  - **nach `load`:** die Bilder laden, wenn die Ebene der Kacheln zum
    ersten Mal `load` meldet, mit `priority: 'low'`;
  - **zwei Bilder danach:** dazu zwei `requestAnimationFrame`, einmal am
    Zweig von #120 und einmal mit der UI aus #135 darüber;
  - **nach der Meldung:** statt der zwei Bilder ein `PerformanceObserver`
    für `largest-contentful-paint`, bis eine Kachel gemeldet ist.
- 23 Bilder, zusammen 638 282 Byte, gezählt in `web/dist-skin/assets`.

## Ablauf

- Je Stand ein Build mit `SKIN=./skins/tablett npx vite build --outDir
  dist-skin`, dann `npx lhci collect` mit `web/lighthouserc.cjs`, nur mit
  `--outDir dist-skin` im Befehl des Servers. Die Werte aus den Berichten
  `lhr-*.json`: die Phasen des LCP aus `largest-contentful-paint-element`,
  der beobachtete LCP aus `metrics`, der Beginn der Bilder aus
  `network-requests`.
- Der Wert für „beim Start“ stammt aus dem Review zu #135
  (issuecomment-5976899037), gemessen am 04.10. morgens mit 22 Bildern,
  631 KB.

## Ergebnis

| Stand | LCP | Performance | TBT | Bilder beginnen, beobachtet |
|---|---|---|---|---|
| beim Start | 5,5 s | 0,80 | | |
| nach `load` | 5,54 · 5,55 · 5,62 s | 0,74 · 0,79 · 0,79 | 0 · 0 · 268 ms | vor dem LCP |
| zwei Bilder danach, #120 | 1,61 · 1,68 · 1,69 s | 0,99 · 0,99 · 0,99 | 95 · 101 · 102 ms | 0 bis 1 ms nach dem LCP |
| zwei Bilder danach, mit #135 | 5,72 · 5,72 · 5,80 s | 0,76 · 0,77 · 0,77 | 157 · 165 · 182 ms | 100 ms vor dem LCP |
| nach der Meldung, #120 | 1,61 · 1,61 · 1,69 s | 0,99 · 0,99 · 0,99 | 80 · 85 · 105 ms | 101 bis 102 ms nach dem LCP |

In allen Läufen: Barrierefreiheit 1, Best Practices 0,96, SEO 1, CLS 0.
Nach der Meldung hält jede Schwelle aus `web/lighthouserc.cjs`, auch die
Warnung zum LCP bei 4 s.

## Schluss

- Das grösste Element ist eine Kachel. Leaflet blendet sie erst in einem
  Bild nach `load` ein. Beginnen die Bilder davor, rechnet Lighthouse ihre
  638 KB zum LCP.
- Zwei Bilder nach `load` sind ein Wettlauf mit dem ersten Malen: Mit der
  UI aus #135 kam es rund 100 ms später, und die Bilder begannen davor.
- Die Meldung des LCP kommt erst, wenn gemalt ist; danach beginnen die
  Bilder in jedem Lauf rund 100 ms nach dem LCP.
