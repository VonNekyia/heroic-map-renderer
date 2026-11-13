---
title: "0140: Das Rad zoomt wieder mit Leaflet"
description: Warum die Webkarte das Rad wieder Leaflets eigenem scrollWheelZoom mit seinen Vorgaben überlässt, statt einer eigenen Logik; was die Probe auf dem Spiegel verglichen hat, was verworfen ist, und wie die Formen auch dann nicht springen, wenn ein Zoom im Bild nach dem Ende des vorigen beginnt; löst 0135 und 0136 ab.
status: gilt
date: 2026-11-13
issues: []
code:
  - web/src/main.ts
  - web/src/ebenen.ts
---

# 0140: Das Rad zoomt wieder mit Leaflet

Löst [0135](0135-eigener-handler-fuer-das-rad.md) und
[0136](0136-rad-spielt-nichts-nach.md) ab. Die Regeln im Einzelnen stehen in
[Frontend](../frontend.md), „Zoom mit dem Rad“.

## Anlass

Der User am 13.11.2026 nach der Probe mit fester Stufe je Raste: „das zoomen
könnte sich etwas mehr an meine maus anpassen wenn ich schnell scrolle mehr und
weniger schnell langsamer“. Danach war ihm jede der eigenen Varianten entweder
zu schnell oder zu langsam.

Eine Probe auf dem Spiegel bot ihm Varianten über `rad=` in der Adresse:

| Variante | Regel |
|---|---|
| a | 0136: höchstens eine Stufe je Ereignis, danach eine, solange das Rad dreht |
| b | wie a, aber bis zu 4 gezählte Stufen danach |
| c | 40 ms sammeln, dann alles auf einmal, der Rest folgt immer |
| d | wie c, aber die erste Raste sofort |
| e | Leaflets eigener `scrollWheelZoom` mit seinen Vorgaben |
| f | wie b, hinein höchstens bis zur feinsten gerenderten Stufe |

Seine Wahl, wörtlich: „e passt“.

Abgenommen hat er auf dem Spiegel am 13.11. mit E und den Fixes aus
[0137](0137-pfade-erst-nach-dem-zoom.md) und
[0139](0139-striche-halten-ihre-breite.md), wörtlich: „Yeah the border is calm“
zur Randlinie im 3D beim Zoomen und „Yeah it stays calm“ zu den Regionen beim
schnellen Zoomen.

## Entscheidung

- **Das Rad zoomt mit Leaflets `scrollWheelZoom`,** mit Leaflets Vorgaben:
  `wheelDebounceTime` 40 ms, `wheelPxPerZoomLevel` 60. Die eigene Logik in
  `rad.ts` fällt weg.
- **Die Formen springen auch dann nicht, wenn ein Zoom im Bild nach dem Ende
  des vorigen beginnt.** Nach jedem `zoomend` liest ein Microtask den Stil der
  SVGs der Formen. Er läuft nach `moveend`, wenn der Renderer seinen Massstab
  zurückgesetzt hat. So rechnet der Browser den Massstab 1, und der Übergang
  des nächsten Zooms beginnt dort, gleich ob ihn das Rad, ein Knopf, die
  Tastatur oder das Zoomrechteck startet.
- Das ersetzt die Folgestufe des Rads ein Bild nach `zoomend` aus
  [0137](0137-pfade-erst-nach-dem-zoom.md); der Rest von 0137 gilt.

## Verworfene Alternativen

- **a bis d und f:** Der User hat e gewählt. Die eigene Logik folgte dem Tempo
  des Rads entweder nur bei langem Drehen (a, b, f) oder zoomte nach einem
  kurzen schnellen Dreh noch rund eine halbe Sekunde nach (c, d).
- **Eine kürzere Animation als 250 ms:** Leaflet setzt je Animation einen
  Zeitgeber von 250 ms und löscht ihn nicht. Endet die Animation früher und
  beginnt die nächste, beendet er sie. Gemessen nach 48 ms bei 200 ms Dauer und
  nach 113 ms bei 150 ms.
- **Das Springen am Rad beheben,** wie in 0137 mit der Folgestufe ein Bild
  später: hilft nur dem eigenen Handler, nicht Leaflets Rad, den Knöpfen oder
  der Tastatur.

## Folgen

- **Was 0135 behoben hatte, gilt wieder:** Leaflet verliert Rasten während
  einer Animation, rundet eine angefangene Stufe auf, und `getWheelDelta` teilt
  in Chromium unter Windows durch die Skalierung. Der User hat das so gewählt.
- **Springen unter Dauerfeuer am Rad,** headless, die Region im iso. Je Lauf
  viermal 1 s hinein und hinaus, je Takt zwei Läufe. Gezählt sind Animationen,
  deren Übergang des SVG nicht bei 1 beginnt:

  | Takt der Rasten | Animationen je Lauf | ohne das Lesen | mit |
  |---|---|---|---|
  | 16 ms | 8 | 0, 0 | 0, 0 |
  | 30 ms | 8 | 0, 0 | 0, 0 |
  | 50 ms | 16 | 3, 2 | 0, 0 |
  | 80 ms | 14 | 2, 1 | 0, 0 |

- Je Zoom ein erzwungenes Rechnen des Stils der SVGs der Ebenen.
- **Nicht belegt:** was ein echtes Rad oder Touchpad je Raste meldet. Die Tests
  erzeugen ihre Ereignisse selbst.
