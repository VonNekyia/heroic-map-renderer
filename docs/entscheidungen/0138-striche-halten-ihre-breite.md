---
title: "0138: Striche halten ihre Breite im Zoom"
description: Warum Ränder und Linien der Ebenen in einer Zoom-Animation über einen Faktor am SVG gegengerechnet werden, statt mit dem SVG zu wachsen und am Ende zurückzuspringen; was verworfen ist und was es kostet.
status: gilt
date: 2026-11-13
issues: []
code:
  - web/src/ebenen.ts
  - web/src/formen.ts
---

# 0138: Striche halten ihre Breite im Zoom

## Anlass

Der User am 13.11.2026 nach [0137](0137-pfade-erst-nach-dem-zoom.md): „sie
flackern halt aber glitchen nicht mehr“.

In der Zoom-Animation skaliert Leaflet das äussere `<svg>` jeder Ebene per
CSS-`scale`. Ränder, Linien und ihr Strichmuster wuchsen damit mit, je Schritt
auf das Doppelte. Im ersten Bild nach dem Ende sprangen sie zurück.

Gemessen headless an Bildfolgen über drei Schritte: in 2D ein Rand von 2 px
auf 3–4 px und zurück, im iso der Lauf über die schräge Kante von 3 px auf
6–7 px und zurück.

## Entscheidung

- **Ein Faktor je SVG:** die registrierte Eigenschaft `--strich`, eine Zahl,
  vererbt, Ausgang 1.
- **Jeder Pfad mit Strich trägt seine Breite als `calc(<breite>px * var(--strich))`.**
  Ebenso Muster und Versatz. Die Werte kommen aus den Attributen, die Leaflet
  setzt (`halteStrich` in `formen.ts`). Geschrieben wird einmal je `setze`,
  nicht je Animation.
- **In der Animation läuft der Faktor** am SVG von 1 auf 1 / Massstab. Dauer
  und Kurve kommen aus dem berechneten Übergang des SVG selbst. Die
  Stützstellen liegen bei je 10 % Fortschritt auf dem Kehrwert des Massstabs.
  So ist die Breite auf dem Schirm genau, nicht nur ungefähr.
- **Bei `zoomend` endet die Animation des Faktors,** im selben Task wie das
  Zurücksetzen des Renderers bei `moveend`.
- **Ohne `CSS.registerProperty`** bleibt der Faktor 1, und alles ist wie
  vorher.

## Verworfene Alternativen

- **`vector-effect: non-scaling-stroke`:** wirkt nicht. Das `scale` liegt auf
  dem äusseren `<svg>`, ausserhalb des Koordinatensystems, das `vector-effect`
  kennt. In der Bildfolge gab es Bild für Bild dieselben Breiten wie ohne.
- **Die Striche in der Animation ausblenden:** flackert selbst.
- **Die Breite je Pfad überblenden,** mit Leaflets Kurve von der Breite auf
  Breite / Massstab: Je Animation ein Schreiben je Pfad, und auf dem Schirm in
  der Mitte bis zu 12,5 % daneben.

## Folgen

- Im Test bleiben Breite, Muster und Versatz jedes Strichs auf dem Schirm in
  jedem Bild einer Kette von zwei Schritten auf 5 % am Wert des Attributs. Im
  iso läuft eine gestrichelte Route über einen Wall, also mit einem verdeckten
  Lauf (1,5 px, `3 4`) und einem sichtbaren mit Versatz; in 2D eine
  gestrichelte Region.
- Der User am 13.11.: Es flackert die Randlinie, während des Zoomens, im 3D.
  Band, Fläche und Verschwinden hat er nicht angekreuzt.
- In der Bildfolge bleibt der Rand in 2D in der Animation bei 1–2 px und im
  iso bei 2–4 px, statt mitzuwachsen.
- Je SVG einer Ebene eine Web-Animation je Zoom-Animation.
- **Nicht belegt:** der Zoom mit zwei Fingern. Leaflet skaliert dort je Bild
  ohne `zoomanim`, der Faktor bleibt 1, und die Striche wachsen mit wie
  bisher.
- Die Bänder der Wand wechseln ihre Zahl weiter je Schritt. Das regelt diese
  Entscheidung nicht.
