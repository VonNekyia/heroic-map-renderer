---
title: "0096: Formen und Kartenschrift der Ebenen im Browser"
description: Warum die Webkarte Regionen, Kreise und Linien als SVG-Pfade je Ebene zeichnet, Flächen als Umriss der sichtbaren Felder statt als Dreiecke mit Maske, die Sichtbarkeit in einem Durchgang rechnet, die Höhen einmal je Ebene lädt und die Kartenschrift als SVG-Text in der unveränderten Schrift setzt; mit den Kosten nach Regel 26.
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
Ein Kreis mit 2000 Blöcken Radius hat rund 785 000 Felder von 4 × 4 Blöcken.

## Entscheidung

- **SVG je Ebene:** Leaflet zeichnet Flächen, Ränder und Linien als SVG in
  einem Pane je Ebene, `z-index` 410 + Rang, unter den Nadeln (510 + Rang).
  Klicks gehen durch das SVG; nur Flächen mit Namen oder Tafel fangen sie.
- **Flächen als Umriss:** Geschnitten wird an den Linien durch die Mitten
  der Zellen in Felder. Die sichtbaren Felder ganz drinnen ergeben einen
  Umriss; Felder mit Rand geben ihre Stücke. Alles kommt in einen Pfad,
  gerade/ungerade, einmal gefüllt, ohne Vereinfachen.
  - Warum der Umriss reicht: Was sichtbar ist, deckt auf dem Schirm genau
    einen Punkt des Geländes. Die Projektion ist dort umkehrbar, also
    umschliesst der projizierte Umriss das Bild der Felder.
  - Der Umriss trägt jede Ecke der Felder, so folgt er dem Gelände. Seine
    Kanten laufen von Mitte zu Mitte der Zellen; dort ist `H` linear, die
    Projektion einer Kante also gerade. Kanten der Stücke mit Rand tragen
    dazu ihre Mitte.
  - Die Punkte wachsen mit dem Umfang, nicht mit der Fläche.
- **Sichtbarkeit in einem Durchgang:** je Linie zur Kamera, mit laufendem
  Maximum von `H + Steigung · Abstand` über die Mitten der Felder davor und
  je die halbe Zelle vor ihnen. Das sind dieselben Punkte, die ein einzelner
  Punkt in Schritten einer halben Zelle abtastet; das Ergebnis gleicht
  `verdeckt` an jeder Mitte. Es kostet einmal je Feld, nicht je Feld und
  Schritt.
- **Höhen einmal je Ebene:** Die Regionen aller Formen und Schriften einer
  Ebene laden zusammen, nur innerhalb von `area`. Für eine Fläche ihr
  Rechteck, für Ränder und Linien nur die Regionen entlang des Zugs, je
  samt dem Streifen zur Kamera. Das Ergebnis hält die Karten selbst und
  fällt nach dem Zeichnen weg; der Cache der Höhen fasst nur 64 Regionen.
- **Kartenschrift als SVG-Text:** je Schriftzug ein SVG mit `textPath`, in
  IM FELL English SC als TTF, unverändert, geladen mit `FontFace`. Die Höhe
  der Grossbuchstaben ist die Oberkante des „H“: 1384 von 2048 Einheiten.

## Verworfene Alternativen

- **Leinwand von Leaflet (`L.canvas`):** Leaflet prüft Treffer je
  Leinwand, und jede liegt über der ganzen Karte. Die oberste gäbe einen
  Klick nicht an die Fläche einer tieferen Ebene weiter.
- **Dreiecke mit Maske,** wie `ebenen.md` sie als einen Weg beschreibt:
  dasselbe Bild, aber Punkte je Feld und eine eigene Leinwand für die Maske.
- **Felder je Reihe zu Streifen** (der erste Stand dieser PR): Auf
  unebenem Grund behält jede Kante ihre Ecken, geschätzt bis zu 1,5 Mio.
  Punkte für den Kreis oben.
- **Vereinfachen, durch Leaflet oder vorher:** Jeder Ring würde für sich
  vereinfacht; gemeinsame Kanten liefen auseinander, und gerade/ungerade
  machte aus der Überlappung ein Loch.
- **Je Feld den Strahl abgehen:** rund 785 000 Felder mal etwa 50 Schritte.
- **Höhen je Form:** Jede Form lud ihre Regionen neu, der Cache verdrängte
  sie gleich wieder.
- **`sCapHeight` der Schrift** (1417): Es liegt über den flachen
  Grossbuchstaben H, E und Z (1384); die Schrift stünde 2,4 % zu klein.
- **WOFF2 statt TTF:** Die OFL nennt einen Wechsel des Formats eine
  geänderte Version, und die darf den reservierten Namen nicht tragen. Die
  Schrift kommt unverändert mit, so wie gewählt.
- **Je Zeichen ein gedrehtes Element:** mehr DOM, Sperrung und Kontur selbst
  gerechnet; `textPath` macht beides im Browser.

## Folgen

- **Kosten nach Regel 26:** Die Messung im Browser folgt in dieser PR.
- Gerechnet wird einmal je `version`; ein Zoom projiziert nur neu, und nur
  die Striche über verdeckte Stücke und die Schrift setzen sich neu.
- Ein Feld, dessen Mitte verdeckt ist, fehlt ganz, auch wenn eine Ecke
  sichtbar wäre. Der Fehler ist höchstens eine Zelle breit. Am Rand des
  Verdeckten kann der Umriss sich darum leicht überschneiden; gerade/
  ungerade zeigt dort ein kleines Loch.
- Die Schrift kostet 184 KB, geladen erst mit dem ersten Schriftzug. Ihre
  Lizenz steht in `lizenzen.txt` und `NOTICE`.
