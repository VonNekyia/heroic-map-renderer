---
title: Texturen des Tabletts
description: Was die Texturen des Skins Tablett beim Laden kosten, erstes Bild, Worker und Tausch mit CPU 1× und 4×, die Arbeit des Workers bei drei Fenstergrössen, das Ziehen mit dem kleineren Rand gegen den Stand vor den Texturen, Bündel und Lighthouse am Build mit Skin.
date: 2026-10-03
commits: [9ce8744, 40e9ec2]
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/stoffe.ts
  - web/skins/tablett/werkstatt.ts
  - web/skins/tablett/zeichnen.ts
---

# Texturen des Tabletts

Mit Texturen zeichnet der Skin das erste Bild in Flächenfarben in 6 ms, bei
CPU 4× in 29 ms, und tauscht nach rund 0,2 s die Bilder mit Texturen ein.
Die Arbeit dafür macht ein Worker: 0,15 s bei CPU 1×, bei CPU 4× rund
0,37 bis 0,49 s je nach Fenster. Beim Ziehen kostet der Skin bei CPU 4×
noch 3,6 statt 8,1 ms je Bewegung der Maus; kein Bild fällt aus.

## Aufbau

- Stände: `9ce8744`, master mit dem Skin nach #119, Flächenfarben und ein
  Viertel Rand je Seite; `40e9ec2`, dieser Zweig, mit Texturen, Worker und
  2 % Rand. Dazu `40e9ec2` ohne `SKIN` als Grundkarte. Gebaut mit
  `npx vite build` und `SKIN=./skins/tablett npx vite build --outDir
  dist-skin` (Vite 8.3.0).
- Wie in [Skin Tablett](2026-10-03-skin-tablett.md): die Previews der
  Builds, der Demobaum aus `web/public/tiles-demo` mit `seaLevel` 0 und
  `area` `[-64, -64, 64, 64]` per Route in `map.json`, ein Fenster von
  1491 × 1055, Chromium von Playwright, kopflos, die Drossel per CDP
  (`Emulation.setCPUThrottlingRate`).
- Die Messskripte lagen lokal, nicht im Repository.

## Ablauf

- **Laden:** die Dauer von `performance.measure('tablett: zeichnen')`, das
  erste Bild, und `'tablett: texturen'`, vom Beginn des Zeichnens bis die
  Bilder getauscht sind. Dessen `detail` nennt die Zeit im Worker und den
  Tausch auf dem Hauptthread.
- **CDP drosselt keine Worker:** `Emulation.setCPUThrottlingRate` an einer
  Sitzung des Workers lehnt Chromium ab („Operation is only supported for
  pages, not workers“), und die Zeit im Worker blieb bei CPU 4× gleich.
  Deshalb zusätzlich nachgestellt: den Auftrag an den Worker abgefangen und
  dieselbe Arbeit, `marmorKachel`, `marmor`, `rechne` für jede Fläche und
  `ebenen`, auf dem gedrosselten Hauptthread derselben Seite gerechnet,
  im Devserver, frisch geladen, ohne vorheriges Aufwärmen.
- **Fenstergrössen:** nachgestellt bei 1280 × 800, 1491 × 1055 und
  1920 × 1080, an der Testwelt in 8:5 von Südost bei scale 16, mit `--area`
  als Quadrat.
- **Ziehen:** wie dort, 120 Bewegungen der Maus mit gedrückter Taste, je 2
  und 1,2 Pixel, 16 ms auseinander, auf `fitZoom`; gemessen die Abstände
  der Bilder und die Dauer der ganzen Bewegung.
- Abwechselnd ohne Skin, master und dieser Zweig, je drei frische Seiten,
  die Sperrdatei gelegt. Fremde Last vor der Reihe 14 %, Programme im
  Hintergrund; keine Builds oder Tests.
- Eine erste Reihe am selben Tag fiel in Builds und Tests einer anderen
  Sitzung und ist verworfen.
- **Lighthouse:** wie in [CI](../entwicklung/ci.md), „Lighthouse“, aber
  gegen `web/dist-skin` mit dem Demobaum als `tiles/` und `seaLevel` und
  `area` in seiner `map.json`, drei Läufe.

## Ergebnis

Laden, ms, je drei Läufe:

| CPU | Stand | erstes Bild | Texturen getauscht nach | im Worker | Tausch |
|---|---|---|---|---|---|
| 1× | master | 8,3 · 7,6 · 7,8 | | | |
| 1× | dieser Zweig | 6,3 · 6,3 · 6,0 | 195 · 191 · 195 | 151 · 147 · 153 | 0,1 · 0,1 · 0 |
| 4× | master | 35,4 · 37,7 · 34,7 | | | |
| 4× | dieser Zweig | 30,7 · 29,2 · 26,7 | 401 · 388 · 366 | 187 · 204 · 172, ungedrosselt | 0,4 · 0,2 · 0,2 |

Die Arbeit des Workers nachgestellt, ms, am Demobaum:

| CPU | Grund des Marmors | Adern und Platte | Flächen, 152 109 Pixel | beide Ebenen malen | zusammen |
|---|---|---|---|---|---|
| 1× | 8 · 8 · 7 | 13 · 12 · 12 | 75 · 73 · 77 | 42 · 41 · 41 | 138 · 134 · 137 |
| 4× | 32 · 28 · 33 | 39 · 39 · 43 | 231 · 229 · 228 | 164 · 164 · 167 | 466 · 459 · 471 |

Nachgestellt bei CPU 4× an der Testwelt, ms:

| Fenster | Pixel mit Textur | zusammen |
|---|---|---|
| 1280 × 800 | 118 650 | 382 · 378 · 369 |
| 1491 × 1055 | 124 240 | 411 · 422 · 441 |
| 1920 × 1080 | 136 182 | 494 · 489 · 470 |

Ziehen, Dauer der Bewegung in ms. Der Abstand der Bilder lag in allen
Läufen im Median bei 16,7 ms, im p95 bei 16,7 bis 16,8 ms, nie über
16,8 ms:

| CPU | ohne Skin | master | dieser Zweig |
|---|---|---|---|
| 1× | 4 276 · 4 163 · 4 196 | 4 361 · 4 324 · 4 273 | 4 229 · 4 216 · 4 154 |
| 4× | 4 382 · 4 411 · 4 432 | 5 464 · 5 403 · 5 260 | 4 860 · 4 875 · 4 788 |

Bündel, Byte, gzip mit `gzip -9`:

| Stand | JS des Skins | gzip |
|---|---|---|
| master | 8 464 | 3 935 |
| dieser Zweig | 12 327 + 9 813 für den Worker | 5 594 + 4 666 |

Ohne Skin bleibt das JS der Karte bei 161 676 Byte, mit Skin bei 163 477;
das CSS des Skins bei 193 Byte. Bilder für Texturen: keine, 0 KB.

Lighthouse mit Skin: Performance 0,98 · 1 · 0,99, Barrierefreiheit, Best
Practices und SEO je 1, CLS 0, TBT 131 · 14 · 65 ms, LCP rund 1,69 s. Alle
Schwellen aus `web/lighthouserc.cjs` halten.

## Schluss

- Das erste Bild bleibt unter 10 ms, bei CPU 4× unter 31 ms; die Texturen
  kosten den Hauptthread darüber hinaus nur den Tausch, unter 0,5 ms.
- Die Arbeit des Workers wächst mit den Pixeln mit Textur: bei CPU 4× rund
  0,37 s bei 1280 × 800 und 0,49 s bei 1920 × 1080. Am meisten kosten die
  Flächen, dann das Malen beider Ebenen.
- Beim Ziehen bei CPU 4×: master rund (5 376 − 4 408) / 120 ≈ 8,1 ms je
  Bewegung, dieser Zweig rund (4 841 − 4 408) / 120 ≈ 3,6 ms. Auf
  `fitZoom` bewegt sich die Karte nicht mehr, die Leinwände sind halb so
  gross wie vorher. Bei CPU 1× liegt der Unterschied in der Streuung.
- Lighthouse hält wie vor den Texturen.
