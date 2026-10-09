---
title: "0096: Formen und Kartenschrift der Ebenen im Browser"
description: Warum die Webkarte Regionen, Kreise und Linien als SVG-Pfade je Ebene zeichnet, Flächen als Streifen je Reihe statt als Dreiecke mit Maske, und die Kartenschrift als SVG-Text in der unveränderten Schrift.
status: gilt
date: 2026-10-09
issues: [219]
code:
  - web/src/gelaende.ts
  - web/src/formen.ts
  - web/src/schrift.ts
  - web/src/ebenen.ts
---

# 0096: Formen und Kartenschrift der Ebenen im Browser

## Anlass

#219, Teil 4: Die Webkarte zeigt Regionen, Kreise, Linien und Kartenschrift.
[0095](0095-ebenen.md) und [Ebenen](../benutzung/ebenen.md), „Zeichnen“,
legen fest, was jede Ansicht zeichnet: auf dem Gelände, ohne doppeltes Alpha,
ohne Verdecktes. Offen war, womit der Browser es zeichnet und was es kostet.

## Entscheidung

- **SVG je Ebene:** Leaflet zeichnet Flächen, Ränder und Linien als SVG in
  einem Pane je Ebene, `z-index` 410 + Rang, unter den Nadeln (510 + Rang).
  Das Pane lässt Klicks durch; nur Flächen mit Namen oder Tafel fangen sie.
- **Flächen als Streifen:** Geschnitten wird an den Linien durch die Mitten
  der Zellen. Felder ganz drinnen fassen sich je Reihe zu einem Streifen
  zusammen, dessen Umriss jede Ecke der Felder trägt; Felder mit Rand geben
  ihre Stücke. Alles kommt in einen Pfad, gerade/ungerade, einmal gefüllt.
  Nach der Projektion fällt weg, was weniger als 0,25 Pixel der feinsten
  Stufe beiträgt.
- **Höhen je Form:** Jede Form hält die Höhenkarten ihres Ausschnitts selbst,
  samt dem Streifen zur Kamera, aus dem Gelände sie verdecken kann. Der
  Cache der Höhen fasst 64 Regionen, ein Kreis mit 2000 Blöcken braucht mehr.
- **Kartenschrift als SVG-Text:** je Schriftzug ein SVG mit `textPath`, in
  IM FELL English SC als TTF, unverändert, geladen mit `FontFace`. Die Höhe
  der Grossbuchstaben ist die Oberkante des „H“: 1384 von 2048 Einheiten.

## Verworfene Alternativen

- **Leinwand von Leaflet (`L.canvas`):** Leaflet prüft Treffer je
  Leinwand, und jede liegt über der ganzen Karte. Die oberste gäbe einen
  Klick nicht an die Fläche einer tieferen Ebene weiter.
- **Dreiecke mit Maske,** wie `ebenen.md` sie als einen Weg beschreibt:
  dasselbe Bild, aber jedes Feld mindestens zwei Dreiecke und eine eigene
  Leinwand für die Maske statt eines Pfads von Leaflet.
- **Jedes Feld einzeln:** Die Punkte wüchsen mit der Fläche, bei einem
  Kreis mit 2000 Blöcken Radius rund 785 000 Felder. Der Streifen hat auf
  ebenem Grund nach dem Vereinfachen vier Ecken.
- **`sCapHeight` der Schrift** (1417): Es liegt über den flachen
  Grossbuchstaben H, E und Z (1384); die Schrift stünde 2,4 % zu klein.
- **WOFF2 statt TTF:** Die OFL nennt einen Wechsel des Formats eine
  geänderte Version, und die darf den reservierten Namen nicht tragen. Die
  Schrift kommt unverändert mit, so wie gewählt.
- **Je Zeichen ein gedrehtes Element:** mehr DOM, Sperrung und Kontur selbst
  gerechnet; `textPath` macht beides im Browser.

## Folgen

- Gerechnet wird einmal je `version`; ein Zoom skaliert nur, nur die
  Striche über verdeckte Stücke und die Schrift setzen sich neu.
- Ein Feld, dessen Mitte verdeckt ist, fehlt ganz, auch wenn eine Ecke
  sichtbar wäre. Der Fehler ist höchstens eine Zelle breit.
- Wo sich Stücke auf dem Schirm überlappen, etwa an einem Hang, der von der
  Kamera wegfällt, kann gerade/ungerade ein Loch zeigen. Solche Stellen
  liegen meist im Verdeckten.
- Die Schrift kostet 184 KB, geladen erst mit dem ersten Schriftzug. Ihre
  Lizenz steht in `lizenzen.txt` und `NOTICE`.
- Was eine grosse Fläche im Browser an Zeit und Speicher kostet, misst
  eine eigene Reihe nach Regel 26.
