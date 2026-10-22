---
title: "0116: Nadeln und Banner unter 16 Blöcken je Pixel"
description: Warum Nadeln und Banner auf der Webkarte nur bis 16 Blöcke je CSS-Pixel ihre volle Grösse behalten und darunter je Stufe auf die Hälfte schrumpfen, Bild und Name, bis der Name unter 6 Pixel fiele; warum die Grenze am Massstab hängt statt am Zoom; löst die feste Grösse aus 0097 für die Webkarte ab.
status: gilt
date: 2026-10-23
issues: []
code:
  - web/src/ebenen.ts
---

# 0116: Nadeln und Banner unter 16 Blöcken je Pixel

## Anlass

Der User auf der Webkarte, Baum `top-north-s` (scale 4, von oben), am
22.10.: Bis Zoom −6 stehen die Stadtnamen gut, danach überlappen sie und
müssten kleiner werden. Bis dahin galt aus
[0097](0097-banner-feste-groesse-tafel-beim-zeigen.md), „Feste Grösse“:
Nadeln und Banner samt Namen sind auf jeder Stufe gleich gross. Weit
draussen rücken die Orte auf dem Schirm zusammen, die Bilder nicht.

## Entscheidung

Vorschlag des Reviewers vom 22.10., umgesetzt für die Webkarte.

- **Grenze am Massstab:** Volle Grösse, solange ein Block auf dem Schirm
  mindestens 1/16 CSS-Pixel breit ist, also bis 16 Blöcke je Pixel. Auf
  `top-north-s` ist das Zoom −6: 4 Pixel je Block auf der feinsten Stufe,
  mal 2⁻⁶. Auf einem anderen Baum liegt sie bei demselben Massstab, nicht
  bei derselben Zahl des Zooms. Die Breite eines Blocks ist die längere
  seiner Kanten in x und z, `blockBreite`.
- **Darunter je Stufe die Hälfte,** Bild und Name zusammen: So bleibt die
  Anordnung von dort, wo die Namen noch stehen, und es kommen keine neuen
  Überlappungen dazu. Das Bild ohne Glätten, auf ganze Pixel des Geräts
  gerundet, aus 32 × 64 also 16 × 32 und 8 × 16. Der Name, gerade oder im
  Bogen, wird um den Fuss mit kleiner.
- **Untergrenze:** Fiele der Name unter 6 Pixel Höhe, fehlt er; ab dieser
  Stufe werden Nadel und Banner nicht kleiner. Mit Schrift von 16 Pixel ist
  das die zweite Stufe unter der Grenze: 16, 8, dann 4 und ohne Namen.
- **Die Tafel** bleibt, wie sie ist; sie steht über dem kleineren Icon wie
  sonst über dem grossen.
- **Über dem Massstab** gilt 0097 weiter: Pixel auf Pixel in der Grösse
  des Bilds, der Name 16 Pixel.
- **Der Mod** hat noch die feste Grösse; ob dort dieselbe Regel gilt, folgt
  nach der Webkarte.

## Verworfene Alternativen

- **Die Grenze als Zahl des Zooms:** Jeder Baum hat seinen `scale` und
  seine Kamera; Zoom −6 hiesse auf einem Baum mit scale 16 etwas anderes.
- **Stufenlos kleiner:** Das Bild verlöre seine Pixel, und die Anordnung
  änderte sich mit jedem Bruchteil des Zooms.
- **Namen ausblenden, die sich überlappen:** bräuchte eine Prüfung jedes
  Paars bei jedem Zoom, und welcher Name bleibt, wäre Willkür.
- **Weiter die feste Grösse:** Das ist der Befund des Users.

## Folgen

- Bei jedem Zoom über eine Stufe bekommen Nadeln und Banner ein neues Icon;
  neu geladen wird nichts.
- Ergänzt 0097: „Feste Grösse“ gilt auf der Webkarte nur bis zur Grenze.
