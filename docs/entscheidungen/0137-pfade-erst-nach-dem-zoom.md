---
title: "0137: Pfade erst nach dem Zoom"
description: Warum die Webkarte die Formen einer Ebene ab dem Start einer Zoom-Animation für das Ziel rechnet, aber erst nach ihrem Ende setzt, statt 150 ms nach dem Zoom, und warum eine Folgestufe des Rads ein Bild nach dem Ende beginnt; was verworfen ist und was es kostet; ergänzt 0111.
status: gilt
date: 2026-11-13
issues: []
code:
  - web/src/ebenen.ts
  - web/src/rad.ts
---

# 0137: Pfade erst nach dem Zoom

Ergänzt [0111](0111-geometrie-nach-bildschirmfehler.md) darum, wann die Karte
beim Zoomen rechnet und setzt. Die Regeln im Einzelnen stehen in
[Frontend](../frontend.md), „Ebenen“.

## Anlass

Der User am 13.11.2026: „regionen flackern beim reinzoomen und dass sich das
zoomen zu langsam anfühlt“, „man merkt es mehr beim schnellen zoomen“. Danach
genauer: „flackern ist falsch ausgedrückt eher spontanes skalieren also von groß
zu klein es skaliert nicht smooth“.

Nach 0111 rechnete die Karte 150 ms nach `zoomend` oder `moveend` neu und
setzte das Ergebnis sofort. Dreht das Rad weiter, beginnt die nächste Animation
ein Bild nach `zoomend` und dauert 250 ms, nach
[0136](0136-rad-spielt-nichts-nach.md). Der Timer feuerte also mitten in ihr.

In der Animation hat Leaflet 1.9.4 Zoom und Ursprung schon am Ziel. Der Renderer
behält `viewBox` und Rahmen der alten Stufe bis `moveend`. Gesetzte Pfade lagen
deshalb bis zum Ende der Animation versetzt und rund doppelt so gross, dann
sprangen sie auf die richtige Grösse zurück. Dasselbe
gilt für eine Ebene, deren Datei in einer Animation kommt.

Dazu stand nach jedem Schritt 150 ms plus Rechenzeit die Geometrie der alten
Stufe, im iso etwa die Wand mit zu wenigen Bändern.

Ein zweiter Sprung zeigte sich erst in Bildfolgen: Am Start einer verketteten
Animation stand die Region in einem Bild auf Endgrösse und wuchs dann nicht
mehr, während die Kacheln weiterliefen. Der Renderer setzt bei `moveend` seinen
Massstab zurück. Das Rad startete die nächste Stufe aus `zoomend`, Leaflet also
im nächsten Bild, ohne dass der Browser dazwischen den Stil rechnete. Der
Übergang des SVG begann deshalb am Ziel. Headless, 3 Läufe mit je 2
Folgestufen: in 3 von 6 kein Übergang, das SVG stand schon auf 2. In den
Bildfolgen sprang es auf master in 3 von 4 Läufen.

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
- **Eine Folgestufe des Rads beginnt ein Bild nach `zoomend`.** So rechnet der
  Browser den zurückgesetzten Massstab, und der Übergang beginnt bei 1.
- **Der Nachlauf von 150 ms bleibt** für Verschieben und als Sicherung nach
  dem Zoom. Läuft die Rechnung für dieselbe Stufe und dasselbe Fenster schon,
  beginnt er sie nicht noch einmal.

## Nicht belegt

- **Zoom mit zwei Fingern:** nur am Code von Leaflet 1.9.4 gelesen
  (`TouchZoom`), nicht im Test.
- **Bleibt `zoomend` aus,** bleibt `zoomt` wahr. Dann warten neue Rechnungen
  und neu geladene Ebenen bis zum Ende des nächsten Zooms. Leaflet beendet jede
  Animation spätestens nach 250 ms mit einem eigenen Zeitgeber.

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
- Eine Folgestufe des Rads beginnt 16–33 ms später, headless 579–597 ms statt
  563–578 ms nach der ersten Raste. Mit ihr in 6 von 6 Folgestufen ein
  Übergang von 1 an, in 4 Bildfolgen kein Sprung.
- Die Rechnung läuft jetzt in der Animation. Sie rechnet nach 0123 in
  Stücken von 8 ms, die Animation selbst ist ein Übergang in CSS.
