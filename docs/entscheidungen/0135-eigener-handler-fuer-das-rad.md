---
title: "0135: Ein eigener Handler für das Rad"
description: Warum die Webkarte das Rad selbst in ganze Stufen umsetzt, statt Leaflets scrollWheelZoom mit seinen Optionen zu nehmen.
status: gilt
date: 2026-11-01
issues: []
code:
  - web/src/rad.ts
  - web/src/main.ts
---

# 0135: Ein eigener Handler für das Rad

## Anlass

Der User am 01.11.: Zoomen auf der Webkarte fühlt sich schlecht an, in 2D und
3D. Die Diagnose fand zwei Fehler an Leaflets `scrollWheelZoom`:

- Während der Animation von 250 ms verwirft er das Delta. Sechs Rasten im
  Takt von 50 ms ergeben zwei Stufen.
- Jedes Delta rundet er auf eine ganze Stufe auf. Ein Touchpad mit 8 Pixeln
  zoomt eine Stufe.

## Entscheidung

`scrollWheelZoom` ist aus. `radZoom` in `web/src/rad.ts` sammelt die Pixel nach
`L.DomEvent.getWheelDelta` und nimmt nur öffentliche Methoden von Leaflet. Die
Regeln stehen in [Frontend](../frontend.md), „Zoom mit dem Rad“. Die Stufen
bleiben ganzzahlig; ein gebrochener Zoom ist eine eigene Entscheidung.

## Verworfene Alternativen

Gemessen auf dem Spiegel, 3D, 6 Rasten zu 120 Pixeln, 1600 × 900.

| Variante | Stufen bei 50 / 100 / 150 / 250 ms Takt | Reaktion | 8 Pixel | 240 Pixel in 3 s |
|---|---|---|---|---|
| Vorgabe | 2 / 3 / 3 / 6 | 52 ms | 1 Stufe | 10 Stufen |
| `wheelDebounceTime: 150` | 4 / 4 / 3 / 6 | 170 ms | 1 Stufe | 8 Stufen |
| `wheelDebounceTime: 300` | 3 / 4 / 4 / 4 | 320 ms | 1 Stufe | 9 Stufen |
| `wheelPxPerZoomLevel: 240` | 2 / 3 / 3 / 6 | 52 ms | 1 Stufe | 10 Stufen |

- **Längeres `wheelDebounceTime`:** Es verliert weiter Stufen und reagiert drei-
  bis sechsmal so spät.
- **`wheelPxPerZoomLevel`:** Es ändert nichts, weil Leaflet mit `zoomSnap` 1
  jedes Delta aufrundet.
- **`zoomAnimation: false`:** Es behebt den ersten Fehler, nimmt aber die
  Animation, und die soll bleiben.
- **`zoomSnap` unter 1:** Das wäre ein gebrochener Zoom, und der ist hier nicht
  entschieden.
- **Private Methoden von Leaflet überschreiben:** Das bräche beim nächsten
  Update still.

## Folgen

- Das Touchpad wird ruhiger: Eine langsame Geste zoomt weniger Stufen als vorher.
- Eine Geste unter einer Stufe wirkt erst 150 ms nach ihrem Ende. Das braucht
  eine einzelne Raste, wenn das Betriebssystem sie klein meldet.
- Den Handler pflegen wir selbst, dafür hängt er an keinem Innenleben von
  Leaflet.
