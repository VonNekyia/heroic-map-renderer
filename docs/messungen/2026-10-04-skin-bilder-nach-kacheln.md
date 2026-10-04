---
title: Bilder des Skins nach den Kacheln
description: Lighthouse mit dem Skin Tablett, wenn seine Bilder beim Start laden, nach `load` der Kacheln und nach dem Bild, in dem die Kacheln gemalt sind; dazu die Grösse der Bilder.
date: 2026-10-04
commits: [d7a8362]
code:
  - web/skins/tablett/index.ts
  - web/lighthouserc.cjs
---

# Bilder des Skins nach den Kacheln

Laden die Bilder des Skins erst nach dem Bild, in dem die Kacheln gemalt
sind, sinkt der LCP mit Skin von 5,5 s auf 1,61 bis 1,69 s, Performance von
0,79 auf 0,99. Nach `load` der Kacheln allein bleibt er bei 5,5 s.

## Aufbau

- Wie in [Skin Tablett](2026-10-03-skin-tablett.md), „Lighthouse“: Lighthouse
  12.6.1 wie in [CI](../entwicklung/ci.md), „Lighthouse“, gegen
  `web/dist-skin` mit dem Demobaum als `tiles/` und `seaLevel` 0 und `area`
  `[-64, -64, 64, 64]` in seiner `map.json`, drei Läufe je Stand, Chromium
  von Playwright.
- Lighthouse drosselt simuliert (`simulate`, 150 ms RTT, 1,6 Mbit/s, CPU 4×).
  Den LCP rechnet es aus den Anfragen, die vor dem gemalten grössten
  Element beginnen; die Last des Rechners ändert ihn kaum. Ohne Sperrdatei.
- Stände:
  - **beim Start:** `d7a8362`, die Bilder laden mit dem ersten Zeichnen;
  - **nach `load`:** die Bilder laden, wenn die Ebene der Kacheln zum
    ersten Mal `load` meldet, mit `priority: 'low'`;
  - **nach dem gemalten Bild:** dazu zwei `requestAnimationFrame`.
- 23 Bilder, zusammen 638 282 Byte, gezählt in `web/dist-skin/assets`.

## Ablauf

- Je Stand ein Build mit `SKIN=./skins/tablett npx vite build --outDir
  dist-skin`, dann `npx lhci collect` mit `web/lighthouserc.cjs`, nur mit
  `--outDir dist-skin` im Befehl des Servers. Die Werte aus den Berichten
  `lhr-*.json`, die Phasen des LCP aus `largest-contentful-paint-element`.
- Der Wert für „beim Start“ stammt aus dem Review zu #135
  (issuecomment-5976899037), gemessen am 04.10. morgens mit 22 Bildern,
  631 KB.

## Ergebnis

| Stand | LCP | Performance | TBT | Render Delay |
|---|---|---|---|---|
| beim Start | 5,5 s | 0,80 | | |
| nach `load` | 5,54 · 5,55 · 5,62 s | 0,74 · 0,79 · 0,79 | 0 · 0 · 268 ms | 3,3 bis 4,4 s |
| nach dem gemalten Bild | 1,61 · 1,68 · 1,69 s | 0,99 · 0,99 · 0,99 | 95 · 101 · 102 ms | 1,0 bis 1,2 s |

In allen Läufen: Barrierefreiheit 1, Best Practices 0,96, SEO 1, CLS 0.
Nach dem gemalten Bild hält jede Schwelle aus `web/lighthouserc.cjs`, auch
die Warnung zum LCP bei 4 s.

## Schluss

- Das grösste Element ist eine Kachel. Leaflet blendet sie erst im Bild
  nach `load` ein. Beginnen die Bilder bei `load`, beginnen sie vor dem
  LCP, und Lighthouse rechnet ihre 638 KB zu ihm.
- Zwei `requestAnimationFrame` nach `load` reichen: Danach beginnt keine
  Anfrage des Skins mehr vor dem LCP.
