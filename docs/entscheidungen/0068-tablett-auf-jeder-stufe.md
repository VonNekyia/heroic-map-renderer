---
title: "0068: Tablett auf jeder Stufe"
description: Warum Rahmen, Tisch und Gegenstände des Skins Tablett auf allen Stufen bis zur feinsten sichtbar bleiben und je Ansicht gezeichnet werden, in Leinwänden so gross wie das Fenster mit Überstand, neu nach jedem Zoom und nach einem Zug über den Überstand hinaus; löst in 0063 die Punkte „Einmal für fitZoom“ und „Zoom“ ab.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/tablett.css
---

# 0068: Tablett auf jeder Stufe

Löst in [0063](0063-tablett-als-skin.md) die Punkte „Einmal für
`fitZoom`“ und „Zoom“ ab, also das Ausblenden bis `fitZoom` + 1, und dort
unter „Verworfen“ das Neuzeichnen bei jedem Zoom.

## Anlass

Bis hier blendete das Tablett beim Hineinzoomen aus; auf näheren Stufen
blieb nur die Welt. Der User möchte den Tisch auch auf näheren Stufen sehen
(#112).

## Entscheidung

Entschieden vom Maintainer am 04.10. (#112, issuecomment-5974136222),
weitergegeben vom Reviewer:

- **Sichtbar** bleiben Rahmen, Tisch und Gegenstände auf allen Stufen bis
  zur feinsten und decken voll. Was Gelände deckt, bleibt wie in der
  Gesamtansicht: fern alles unter den Kacheln, nah nur, was Gelände nie
  verdeckt, dazu Saum und Lilie.
- **Heraus** geht es weiter nicht über die Gesamtansicht hinaus.
- **Je Ansicht gezeichnet:** zwei Leinwände so gross wie das Fenster mit
  Überstand, neu nach jedem Zoom und nach einem Zug über den Überstand
  hinaus. Während einer Bewegung gleiten sie wie die Kacheln. Die Kosten
  hängen am Fenster, nicht an der Stufe; es bleibt ein `drawImage` je
  sichtbarer Fläche, ohne Schleife je Pixel.
- **Texturen:** dieselben festen Bilder nach dem nächsten Nachbarn
  ([0066](0066-texturen-des-tabletts.md)); auf näheren Stufen wird ein
  Texel grösser. Die Lage des Texelgitters aus
  [0067](0067-gesamtansicht-zwischen-zwei-stufen.md) gilt weiter; ab etwa
  4 px darf ein Texel um 1 px schwanken.
- **`maxBounds`** umfasst auf jeder Stufe den ganzen Tisch. Es ist das
  Fenster der Gesamtansicht, die den Tisch samt Gegenständen zeigt.

Dazu, vom Frontend:

- **Fest je Fenstergrösse** sind Gesamtansicht, Raster und Teile. Beim
  Zoomen bleibt das Tablett, wie es ist, und wird nur grösser.
- **Überstand** je Seite ein Viertel des Fensters.
- **Marmor** liegt fest auf der Welt und wird mit allem grösser; sein
  Schimmer hängt an der Gesamtansicht.
- **Weiche Schatten** ab 24 px Unschärfe verkleinert gerechnet und
  geglättet vergrössert.

Wie es gebaut ist: [Tablett](../tablett.md), „Zeichnen“.

## Verworfene Alternativen

- **Ausblenden bis `fitZoom` + 1,** wie nach 0063. Der User will den Tisch
  auf jeder Stufe.
- **Einmal für die Gesamtansicht zeichnen und mitwachsen lassen.** Auf der
  feinsten Stufe wäre die Leinwand so gross wie das Tablett dort, bei der
  grossen Welt Hunderttausende Pixel breit.
- **Neu zeichnen während der Bewegung.** Es kostete Bilder beim Ziehen, wie
  schon 0063 verwarf; bis zum Ende der Bewegung reicht der Überstand.
- **Andere Bilder je Stufe.** Auf näheren Stufen würde aus der Pixelkunst
  eine feine Textur, und es gäbe Bilder je Stufe.
- **Schatten in voller Grösse weichzeichnen.** Die Unschärfe wüchse mit der
  Stufe, auf der feinsten der grossen Welt auf Tausende Pixel, und mit ihr
  die Kosten.

## Folgen

- Nach jedem Zoom und nach langen Zügen zeichnet der Skin neu.
- Die Leinwände sind 2,25-mal so gross wie das Fenster, statt 1,08-mal.
- Beim Hinauszoomen fehlt während der Animation am Rand das Tablett, bis
  neu gezeichnet ist, wie Kacheln, die noch laden.
- Die Tests prüfen beide Ebenen auf der Stufe über der Gesamtansicht und
  auf der feinsten, und die Grösse der Leinwand dort.
