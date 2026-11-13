---
title: "0137: Pfade erst nach dem Zoom"
description: Warum die Webkarte die Formen einer Ebene ab dem Start einer Zoom-Animation für das Ziel rechnet, aber erst nach ihrem Ende setzt, statt 150 ms nach dem Zoom; was verworfen ist und was es kostet; ergänzt 0111.
status: gilt
date: 2026-11-13
issues: []
code:
  - web/src/ebenen.ts
---

# 0137: Pfade erst nach dem Zoom

Ergänzt [0111](0111-geometrie-nach-bildschirmfehler.md) darum, wann die Karte
beim Zoomen rechnet und setzt. Die Regeln im Einzelnen stehen in
[Frontend](../frontend.md), „Ebenen“.

## Anlass

Der User am 13.11.2026: „regionen flackern beim reinzoomen und dass sich das
zoomen zu langsam anfühlt“, „man merkt es mehr beim schnellen zoomen“.

Nach 0111 rechnete die Karte 150 ms nach `zoomend` oder `moveend` neu und
setzte das Ergebnis sofort. Dreht das Rad weiter, beginnt die nächste Animation
ein Bild nach `zoomend` und dauert 250 ms, nach
[0136](0136-rad-spielt-nichts-nach.md). Der Timer feuerte also mitten in ihr.

In der Animation hat Leaflet 1.9.4 Zoom und Ursprung schon am Ziel. Der Renderer
behält `viewBox` und Rahmen der alten Stufe bis `moveend`. Gesetzte Pfade lagen
deshalb bis zum Ende der Animation versetzt und rund doppelt so gross. Dasselbe
gilt für eine Ebene, deren Datei in einer Animation kommt.

Dazu stand nach jedem Schritt 150 ms plus Rechenzeit die Geometrie der alten
Stufe, im iso etwa die Wand mit zu wenigen Bändern.

## Entscheidung

- **Von `zoomstart` bis `zoomend` setzt die Karte keinen Pfad,** weder aus
  einer neuen Rechnung noch beim Laden einer Ebene. Das gilt auch beim Zoom mit
  zwei Fingern: Leaflet bewegt den Zoom dort je Bild ohne `zoomanim`, und der
  Renderer ist genauso alt.
- **Gesetzt wird nach `moveend`,** im selben Task wie `zoomend` und vor dem
  nächsten Bild. Dann hat der Renderer die neue `viewBox`.
- **Gerechnet wird ab `zoomanim`,** denn erst dort steht das Ziel fest. Gleich
  danach setzt Leaflet Zoom und Ursprung darauf. Beim Zoom mit zwei Fingern
  rechnet der Nachlauf nach `zoomend`.
- **Der Nachlauf von 150 ms bleibt** für Verschieben und als Sicherung nach
  dem Zoom. Läuft die Rechnung für dieselbe Stufe und dasselbe Fenster schon,
  beginnt er sie nicht noch einmal.

## Verworfene Alternativen

- **Nur den Timer bei `zoomstart` löschen:** Dann flackert nichts mehr, aber
  die neue Stufe springt weiter 150 ms nach dem Ende ein.
- **Ein Ergebnis aus der Animation verwerfen:** verliert den Vorsprung der
  Rechnung ab `zoomanim`, und eine Ebene, deren Datei in der Animation kommt,
  käme erst mit dem nächsten Laden.

## Folgen

- Die neue Stufe steht im Test 0 ms nach dem Ende der Animation, wenn die
  Rechnung in 250 ms fertig ist, statt mindestens 150 ms danach.
- **Rechnungen:** Gezählt mit `?punkte` in der Testwelt, headless, 3 Läufe, eine
  Ebene mit 5 Regionen im iso. Je Zug von 4 Rasten hinein von Stufe −2 bis 2
  und zurück, im Abstand von 200 ms, gab es je Richtung 4 begonnene und 4
  gesetzte Rechnungen statt 2–3. Mehr Stufen hat der Demobaum nicht. Jede Stufe
  wird jetzt einmal gerechnet und gesetzt, vorher fielen Zwischenstufen aus.
- **Höhen:** Kein Abruf von `heights` oder `ground` im Zug, vorher wie
  nachher. Beim Hineinzoomen liegt jedes Zwischenfenster im Fenster davor,
  dessen Höhen schon im Cache sind. Beim Hinauszoomen liegt es im Fenster
  danach, das ohnehin geladen wird.
- Die Rechnung läuft jetzt in der Animation. Sie rechnet nach 0123 in
  Stücken von 8 ms, die Animation selbst ist ein Übergang in CSS.
