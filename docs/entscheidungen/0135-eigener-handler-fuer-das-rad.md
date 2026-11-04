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
`L.DomEvent.getWheelDelta`, in Chromium unter Windows im Pixelmodus `-deltaY / 2`,
und nimmt nur öffentliche Methoden von Leaflet. Die
Regeln stehen in [Frontend](../frontend.md), „Zoom mit dem Rad“. Die Stufen
bleiben ganzzahlig; ein gebrochener Zoom ist eine eigene Entscheidung.

## Eine Raste in Chromium unter Windows

Aus dem Quelltext von Chromium, Stand `3be30cc8a45e`:

1. `WebMouseWheelEventBuilder::Build` in
   `ui/events/blink/web_input_event_builders_win.cc`: Ticks sind `wParam / 120`,
   `delta_y` ist Ticks × Zeilen je Raste × 100/3.
2. `ScaleEvent` in `components/input/input_router_impl.cc` multipliziert `delta_y`
   und Ticks mit dem Faktor der Skalierung.
3. Der Konstruktor von `WheelEvent` in
   `third_party/blink/renderer/core/events/wheel_event.cc` teilt beide durch
   `devicePixelRatio`.

Die Skalierung hebt sich also auf: Bei 3 Zeilen je Raste ist `deltaY` 100 und
`wheelDeltaY` 120, geteilt durch einen Browser-Zoom über die Skalierung hinaus.
Leaflets `getWheelDelta` teilt unter Windows trotzdem durch 2 × `devicePixelRatio`.
Eine Raste ergibt damit weniger als 50 Pixel, und schnelle Rasten gingen wieder
verloren.

Gemessen auf einem Windows-Runner der CI, Stand 2026-11-04: Chromium aus
Playwright, sichtbar, eigenes Profil, `--force-device-scale-factor`, 3 Zeilen je
Raste ausgelesen, echte `WM_MOUSEWHEEL` mit ±120 per `PostMessage`, zwei Läufe:

| Skalierung | `devicePixelRatio` | `deltaY` | `deltaMode` | `wheelDeltaY` | `getWheelDelta` |
|---|---|---|---|---|---|
| 100 % | 1 | ±100 | 0 | ∓120 | ∓50 |
| 125 % | 1,25 | ±100 | 0 | ∓120 | ∓40 |
| 150 % | 1,5 | ±100 | 0 | ∓120 | ∓33,3 |

An das Kindfenster ergaben 3 Nachrichten 6 Ereignisse, an das Hauptfenster 3.
Die Werte je Ereignis sind gleich. 200 % und der Browser-Zoom von 125 % liefen
nicht durch und sind nur gerechnet: 25 Pixel und 32 Pixel.

## Nicht belegt

- Mit 6 Zeilen und mehr je Raste ist `deltaY / 2` mindestens 100, also 2 Stufen
  je Raste.
- Bei Browser-Zoom über 100 % liegt eine Raste unter 50 und läuft über die Regel
  mit den 150 ms, statt sofort zu zoomen.
- Was ein Touchpad unter Windows meldet, ist nicht gelesen.

## Verworfene Alternativen

Gemessen an der grossen Welt, 3D, 6 Rasten zu 120 Pixeln, 1600 × 900, Chromium
unter Windows, einzelne Läufe, Stand 01.11.2026. Die Rasten und Pixel sind
erzeugt, nicht von einem echten Rad.

| Variante | Stufen bei 50 / 100 / 150 / 250 ms Takt | Reaktion | 8 Pixel | 240 Pixel in 3 s |
|---|---|---|---|---|
| Vorgabe | 2 / 3 / 3 / 6 | 52 ms | 1 Stufe | 10 Stufen |
| `wheelDebounceTime: 150` | 4 / 4 / 3 / 6 | 170 ms | 1 Stufe | 8 Stufen |
| `wheelDebounceTime: 300` | 3 / 4 / 4 / 4 | 320 ms | 1 Stufe | 9 Stufen |
| `wheelPxPerZoomLevel: 240` | 2 / 3 / 3 / 6 | 52 ms | 1 Stufe | 10 Stufen |
| dieser Handler | 6 / 6 / 6 / 6 | 4 ms | 1 Stufe nach 150 ms Ruhe | 2 Stufen |

- **Längeres `wheelDebounceTime`:** Es verliert weiter Stufen und reagiert drei-
  bis sechsmal so spät.
- **`wheelPxPerZoomLevel`:** Es ändert nichts, weil Leaflet mit `zoomSnap` 1
  jedes Delta aufrundet.
- **`zoomAnimation: false`:** Es behebt den ersten Fehler, nimmt aber die
  Animation, und die soll bleiben.
- **`zoomSnap` unter 1:** Das wäre ein gebrochener Zoom, und der ist hier nicht
  entschieden.
- **`wheelDeltaY` statt `deltaY`:** Es hinge nicht an den Zeilen je Raste. Aber
  jedes Pinch-Update meldet ±120 (`components/input/touchpad_pinch_event_queue.cc`),
  am Mac liefe ein Trackpad deutlich schneller
  (`components/input/web_input_event_builders_mac.mm`), und `page.mouse.wheel` wie
  `new WheelEvent` treffen es nicht wie ein echtes Ereignis
  (`content/browser/devtools/protocol/input_handler.cc`).
- **Private Methoden von Leaflet überschreiben:** Das bräche beim nächsten
  Update still.

## Folgen

- Das Touchpad wird ruhiger: Eine langsame Geste zoomt weniger Stufen als vorher.
- Eine Geste unter einer Stufe wirkt erst 150 ms nach ihrem Ende. Das braucht
  eine einzelne Raste, wenn das Betriebssystem sie klein meldet.
- Den Handler pflegen wir selbst, dafür hängt er an keinem Innenleben von
  Leaflet.
