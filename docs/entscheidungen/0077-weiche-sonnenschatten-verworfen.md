---
title: "0077: Weiche Sonnenschatten verworfen"
description: Warum Cinematic beim harten Sonnenschatten aus 0056 bleibt. Weiche Schatten über eine Sonnenscheibe von 0,53° oder 3° sind am Prototyp geprüft, kaum sichtbar oder teuer, und verworfen. Was sie kosten würden und was günstiger wäre, falls es wieder Thema wird.
status: gilt
date: 2026-10-04
issues: []
code:
  - renderer/src/render/metatile/strahl.rs
---

# 0077: Weiche Sonnenschatten verworfen

## Anlass

Der User fragte, ob die Sonnenschatten in Cinematic nicht leicht ausblenden
sollten. Die Sonne ist eine Scheibe von rund 0,53°. Ein Schatten wird darum
mit dem Abstand zum Werfer weicher, sein Halbschatten ist rund 1 % des
Abstands breit. Cinematic zeichnet ihn hart: ein exakter Strahl zur Mitte
der Sonne je Texel, der 0 oder 1 gibt
([0056](0056-exakter-strahl-zur-sonne.md), im Look
[0058](0058-look-von-cinematic.md)).

Geprüft am 04.10. an einem Prototyp, Zeiten und Zahlen in
[Weiche Sonnenschatten am Prototyp](../messungen/2026-10-04-weiche-sonnenschatten.md).

## Entscheidung

Cinematic bleibt beim harten Schatten aus 0056. Weiche Schatten über eine
Sonnenscheibe gibt es nicht, weder physikalisch noch gewollt weicher. 0056
und 0058 bleiben, wie sie sind. Der User hat am 04.10. nach den Bildern und
den Zeiten entschieden.

## Verworfene Alternativen

Gemessen ist an einem Einzelbild der Testwelt, Cinematic 4:3 aus `se`,
scale 24, am Prototyp. Je Pixel gehen 16 Strahlen über die Scheibe, ihre
Richtungen fest als Vogel-Spirale, dasselbe Muster für jedes Pixel. Die
Faktoren sind die Zeit des Bildes gegen den harten Schatten.

- **Physikalisch, Scheibe 0,53°.** Kaum zu sehen: Halbschatten von 1 bis 4
  Pixeln an Eiche, Haus und Hang. Kostet das 2,2- bis 4,3-Fache, ein
  perfekter Detektor das 1,5-Fache.
- **Gewollt weicher, Scheibe 3°.** Deutlich zu sehen: 9 bis 24 Pixel.
  Kostet das 3,7- bis 5,2-Fache, ein perfekter Detektor das 2,6-Fache.
- **Warum es teuer ist,** auch wenn nur Pixel am Rand mehr Strahlen
  bekommen: Schattenkanten liegen im Spiel überall auf Ebene der Texel, an
  Löchern im Laub, an Gras und an Stufen im Gelände. Ein Detektor aus dem
  harten Schatten muss 35 bis 41 % der Pixel mit Sonne markieren, um jeden
  sichtbaren Halbschatten zu treffen. Unter Wasser muss er den Grund mit
  ansehen.
- **Gestaffelt,** 4 bis 16 Strahlen je Pixel nach der Breite des
  Halbschattens, gibt die unteren Faktoren. Es weicht aber an 1 bis 2 % der
  Pixel sichtbar vom Bild mit 16 Strahlen ab.
- **Die Grösse spricht nicht dagegen:** Die Kacheln wachsen um 0,3 und
  1,0 %, weil die Richtungen fest sind und nichts rauscht.

## Folgen

- Die Schatten bleiben hart und exakt, ohne Mehrkosten.
- Übertragen auf den Vollrender 4:3 der grossen Welt mit 2 h 43 min hätte
  0,53° rund 5 h 50 min bis 11 h 40 min gekostet, 3° rund 10 h 10 min bis
  14 h 10 min. Das ist eher zu hoch gerechnet, siehe die Messung.
- **Falls es wieder Thema wird:**
  - Günstiger, aber nicht gemessen: den harten Sonnenwert im Bild
    weichzeichnen, mit einem Radius aus dem Abstand zum Werfer und nur über
    dieselbe Fläche. Das kostet keinen Strahl mehr, ist an Kanten der
    Geometrie aber eine Näherung.
  - Mit Strahlen bräuchte es zwei Durchgänge je Metatile: erst hart, mit
    dem Abstand zum Werfer je Fragment, dann eine Randmaske im Bild und dort
    die Strahlen über die Scheibe.
    - Der Rand der Metatile wüchse mit dem breitesten Halbschatten.
    - Die Sonnenform rechnete ihre Dreiecke für jede Richtung vorab.
    - Bits „frei zur Sonne“ für den Kegel der Scheibe sparten Strahlen, wo
      nichts in der Nähe liegt.
