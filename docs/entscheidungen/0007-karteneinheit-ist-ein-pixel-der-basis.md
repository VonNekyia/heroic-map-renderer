---
title: "0007: Eine Karteneinheit ist ein Pixel der feinsten Stufe"
description: Warum das Frontend Leaflets CRS.Simple so umstellt, dass die feinste Stufe den Faktor 1 hat, und nicht spiegelt.
status: gilt
date: 2026-09-22
issues: [7]
code:
  - web/src/main.ts
---

# 0007: Eine Karteneinheit ist ein Pixel der feinsten Stufe

## Anlass

Schritt 7: Leaflet braucht ein Koordinatensystem für die Kacheln aus
`map.json`, dessen `bounds` in Pixeln der feinsten Stufe stehen.

## Entscheidung

Eine Karteneinheit ist ein Pixel der feinsten Stufe:
`scale(zoom) = 2^(zoom − maxZoom)`, `zoom(scale) = log2(scale) + maxZoom`.
Gespiegelt wird nicht, denn `screen_y` des Renderers zeigt schon nach
unten. Siehe [Frontend](../frontend.md), „Das Koordinatensystem“.

## Verworfene Alternativen

- **`CRS.Simple` unverändert.** Es rechnet mit `2^zoom`, Zoom 0 bekäme den
  Faktor 1, und jede Koordinate hinge an der Zahl der Stufen.

## Folgen

- `bounds` aus `map.json` passen ohne Umrechnung; negative
  Kachelkoordinaten sind kein Sonderfall.
- Über der feinsten Stufe sind zwei weitere Zoomstufen erlaubt; dort
  vergrössert Leaflet die vorhandenen Kacheln (`maxNativeZoom`).
- Eine Koordinatenanzeige gibt es nicht: vom Bildpunkt zurück auf eine
  Blockkoordinate zu rechnen verlangte eine angenommene Höhe.
