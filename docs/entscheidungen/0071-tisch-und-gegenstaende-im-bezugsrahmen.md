---
title: "0071: Tisch und Gegenstände im Bezugsrahmen"
description: Warum der Skin Tablett den Tisch und die Füsse der Gegenstände mit der Umkehrung seiner eigenen Projektion im Bezugsrahmen auf die Platte legt, 8:5 aus se in der Gesamtansicht im Fenster der Vorlage, statt über die Homographie der Vorlage, und warum jenseits der Vorlage nur Marmor liegt, in den sie kurz ausläuft, statt dass sie sich gespiegelt fortsetzt oder dunkel endet; löst darin 0070 ab.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/bilder.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/werkzeug/ausschnitte.py
---

# 0071: Tisch und Gegenstände im Bezugsrahmen

Löst in [0070](0070-bilder-aus-der-vorlage.md) ab, wie Tisch und Gegenstände
auf die Platte kommen und was jenseits der Vorlage liegt.

## Anlass

Im Review zu #120 (issuecomment-5976163815) lagen Gegenstände und Tisch in
der Gesamtansicht bis 45 px neben der Vorlage, die Armillarsphäre bei
+45, +47 px, die Kerze bei +15, −21 px. Die Vorlage ist keine
Parallelprojektion: Was über ihre Homographie auf die Platte kam und mit
unserer Kamera zurück, wich umso mehr ab, je weiter es von der Mitte lag.
Wo das Bild des Tischs endete, war es schwarz, in der Gesamtansicht als
Streifen am Rand, in 1:1, von oben und genordet auf einem Viertel bis
Drittel des Fensters.

In der Nachprüfung (issuecomment-5976810750, Befund 7) machte das
gespiegelte Fortsetzen die anderen Kameras zum Kaleidoskop: Holzränder
trafen sich zu V und Pfeilen, in 1:1 aus ne standen oben rechts zwei
Leuchter, einer davon halb, ein Rest des Gegenstands im Bild des Tischs.

## Entscheidung

Vom Reviewer am 04.10. vorgeschlagen und so gebaut, das Jenseits nach
Befund 7 in der Nacht von ihm gewählt:

- **Bezugsrahmen:** 8:5 aus se, die Gesamtansicht im Fenster der Vorlage,
  1491 × 1055 px. Ein Punkt der Vorlage kommt mit der Umkehrung dieser
  Projektion auf die Platte (`aufDiePlatte` in `tablett.ts`). Im
  Bezugsrahmen liegt er so genau dort, wo die Vorlage ihn zeigt.
- **Tisch:** das Bild ist die Vorlage selbst; seine Ecken sind die Ecken der
  Vorlage, auf die Platte gebracht. Ohne Entzerrung.
- **Gegenstände:** Ihr Fuss ist ein Punkt der Vorlage, in `bilder.ts`. Im
  Bild des Tischs bleibt kein Stück von ihnen: Wo sie stehen, füllt das
  Skript die Farbe ringsum, ohne Spiegelbild.
- **Jenseits der Vorlage nur Marmor:** ein Quadrat aus Flicken der Platte
  der Vorlage, das sich nahtlos wiederholt, unter dem Tisch über die ganze
  Ebene. Marmor hat keine Richtung; das Auge sucht dort keine Kante.
- **Am Rand der Vorlage** laufen Holzrand, Tuch und alles andere 24 px weit
  in den Marmor aus, angeschnittene Gegenstände 8 px. Der Auslauf liegt
  ausserhalb der Vorlage: So bleibt der Bezugsrahmen Pixel auf Pixel.
- **Der Rahmen** folgt weiter der Karte, wie in 0070.

## Verworfene Alternativen

- **Die Homographie der Vorlage:** bis 45 px daneben, gemessen per
  Phasenkorrelation.
- **Die Vorlage an jeder Kante gespiegelt fortgesetzt:** In anderen Kameras
  und breiteren Fenstern ein Kaleidoskop aus Holzrändern, Schnitzwerk und
  Resten von Gegenständen (Befund 7).
- **Der Auslauf innerhalb der Vorlage:** Der Bezugsrahmen zeigte an den
  Rändern des Fensters Marmor, wo die Vorlage Holz zeigt.
- **Ein einzelnes Stück Marmor, wiederholt,** wie bisher unter dem
  Tablett: Das Stück von 160 × 65 px ergab ein sichtbares Raster, und
  grösser als rund 130 px im Quadrat ist keine saubere Stelle der Vorlage.
- **Grund jenseits der Vorlage:** In 1:1, von oben und genordet war ein
  Viertel bis ein Drittel des Fensters schwarz.

## Folgen

- Im Bezugsrahmen liegen alle fünf Gegenstände bei 0 px Versatz; ein Test
  hält sie auf 3 px, den Tisch auf 1 px.
- In anderen Kameras und Fenstern endet die Vorlage mit geradem, weichem
  Rand im Marmor, und Gegenstände, die ihr Rand schneidet, enden dort.
- Wo die Vorlage Gegenstände zeigt, liegt im Bild des Tischs ein glatter
  Fleck in der Farbe ringsum. In anderen Kameras sieht man ihn neben dem
  Gegenstand, etwa hinter der Kerze.
- In anderen Kameras ist der Marmor so verzerrt wie die Platte im Bild,
  in 2:1 etwa in t gestaucht; man merkt es nicht.
- Zwei Bilder statt einem: Der Marmor kommt mit 122 KB dazu, das Bild des
  Tischs wird mit 296 KB kleiner als vorher mit 380 KB.
