---
title: "0032: Weiche Beleuchtung zuerst nur für volle Würfel"
description: Warum die weiche Beleuchtung zuerst nur Modelle aus vollen Seiten abdunkelt und Teilflächen und Licht je Ecke später kommen.
status: gilt
date: 2026-09-27
issues: [15, 18]
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
  - renderer/src/assets/model.rs
---

# 0032: Weiche Beleuchtung zuerst nur für volle Würfel

## Anlass

#15: Das Spiel zeichnet Blöcke in der Voreinstellung weich beleuchtet. Für
eine volle Seite (`faceCubic`) rechnet es je Ecke einen Wert aus vier
Nachbarn; Teilflächen rechnet es mit `facePartial` und den Gewichten aus
`SizeInfo`, und an jeder Ecke mischt es auch das Licht der vier Blöcke.

## Entscheidung

Eine AO-Karte bekommt nur ein Modell, das ganz aus vollen Seiten besteht,
das es mit `ambientocclusion` erlaubt und das nicht leuchtet. Dann gehört
jeder Pixel des Sprites genau einer Seite mit vier Ecken. Jeder Block liegt
in einem Licht aus `light_at`. Siehe
[Weiche Beleuchtung](../renderer/weiche-beleuchtung.md).

## Verworfene Alternativen

- **Auch die vollen Seiten eines Modells mit Teilflächen.** Dann gehörte
  nicht mehr jeder Pixel des Sprites einer Seite mit vier Ecken.
- **Teilflächen gleich mit**, Punkt 3 im Issue: offen für einen nächsten
  Schritt.
- **Licht je Ecke** (`LightCoordsUtil.smoothBlend`): aussen vor, wie im
  Issue.

## Folgen

- Schneedecken, Ackerboden, Trampelpfade, Treppen, Platten und Zäune bleiben
  ohne weiche Beleuchtung; als Nachbarn zählen sie mit ihrem Wert.
- Unter Wasser und an der Kante eines Überhangs verläuft das Licht nicht.
- Eine Kachel wiegt ein Fünftel bis ein Viertel mehr, und auf einem Thread
  braucht sie ein Siebtel länger, siehe
  [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md).
