---
title: "0136: Das Rad spielt nichts nach"
description: Warum die Webkarte Rasten während einer Animation verfallen lässt und ein Ereignis höchstens eine Stufe zoomt, statt jede Raste abzuspielen.
status: abgelöst durch 0140
date: 2026-11-05
issues: []
code:
  - web/src/main.ts
---

# 0136: Das Rad spielt nichts nach

Abgelöst durch [0140](0140-rad-von-leaflet.md): Das Rad zoomt wieder mit Leaflet.

## Anlass

Der User am 05.11.2026, nach dem Stand von
[0135](0135-eigener-handler-fuer-das-rad.md) mit einer echten Maus: „Die website
ist schnell aber scrolle ich bei meiner arbeitsmaus rein scrollt es sehr oft“ und
„wenn ich nicht mehr scrolle auch nicht nachträglich reinscrollen“.

Beides folgt aus 0135, die jede Raste zählte:

- Stufen warteten während der Animation und folgten danach, also auch lange nach
  der letzten Raste.
- Eine Raste ab 100 Pixeln ergab 2 Stufen, und eine Stufe ist die doppelte Grösse.

„Sechs Rasten sind sechs Stufen“ war ein Ziel aus der Diagnose, kein Wunsch des
Users.

## Entscheidung

- Ein Ereignis ergibt höchstens eine Stufe.
- Während einer Animation merkt sich `radZoom` nur die letzte Richtung, nichts
  zum Abspielen.
- Beim Ende der Animation folgt eine Stufe nur, wenn das letzte Ereignis höchstens
  150 ms her ist. Sonst verfällt sie.

Die sofortige Reaktion, das Sammeln kleiner Deltas und der Zweig für Chromium unter
Windows bleiben, wie in 0135.

## Sechs Rasten im Takt

Gerechnet, nicht gemessen, mit 250 ms Animation:

| Takt | 50 ms | 100 ms | 150 ms | 250 ms |
|---|---|---|---|---|
| Stufen | 2 | 3 | 4 | 6 |

## Nicht belegt

- Sendet ein Maustreiber nach dem Anhalten selbst noch Ereignisse, gilt das Rad als
  drehend, und die Karte zoomt weiter. Was die Maus des Users meldet, ist nicht
  gelesen.
- Wie sich ein Touchpad jetzt anfühlt, hat noch niemand geprüft.

## Verworfene Alternativen

- **Eine Stufe aufheben und immer folgen lassen:** Sie begänne bis 250 ms nach der
  letzten Raste und liefe noch 250 ms. Genau das soll wegfallen.
- **Die laufende Animation abbrechen:** Dafür hat Leaflet keine öffentliche Methode,
  und private überschreiben wir nicht, siehe 0135.
- **Gebrochener Zoom:** Eine Raste zöge dann weniger als die doppelte Grösse. Das
  bleibt eine eigene Entscheidung, siehe 0135.

## Folgen

- Nach der letzten Raste läuft die Karte noch bis rund 400 ms: Eine Stufe beginnt
  bis 150 ms danach, ihre Animation dauert 250 ms.
- Zwei Rasten unter 100 ms Abstand ergeben eine Stufe. Die zweite kommt dann mehr
  als 150 ms vor dem Ende der Animation und ist zu alt.
- Das Rad zoomt höchstens 4 Stufen je Sekunde, so schnell es auch dreht.
- Das Rad springt nicht mehr um mehrere Stufen. Einen Sprung gibt es noch über das
  Zoomrechteck mit gehaltener Umschalttaste.
