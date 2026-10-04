---
title: "0071: Tisch und Gegenstände im Bezugsrahmen"
description: Warum der Skin Tablett den Tisch und die Füsse der Gegenstände mit der Umkehrung seiner eigenen Projektion im Bezugsrahmen auf die Platte legt, 8:5 aus se in der Gesamtansicht im Fenster der Vorlage, statt über die Homographie der Vorlage, und warum der Tisch sich über die Vorlage hinaus gespiegelt fortsetzt statt dunkel auszulaufen; löst darin 0070 ab.
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

## Entscheidung

Vom Reviewer am 04.10. vorgeschlagen und so gebaut:

- **Bezugsrahmen:** 8:5 aus se, die Gesamtansicht im Fenster der Vorlage,
  1491 × 1055 px. Ein Punkt der Vorlage kommt mit der Umkehrung dieser
  Projektion auf die Platte (`aufDiePlatte` in `tablett.ts`). Im
  Bezugsrahmen liegt er so genau dort, wo die Vorlage ihn zeigt.
- **Tisch:** das Bild ist die Vorlage selbst, mit Marmor unter Tablett und
  Gegenständen; seine Ecken sind die Ecken der Vorlage, auf die Platte
  gebracht. Ohne Entzerrung.
- **Gegenstände:** Ihr Fuss ist ein Punkt der Vorlage, in `bilder.ts`.
- **Jenseits der Vorlage** setzt sich der Tisch fort, an jeder Kante
  gespiegelt, so weit die Leinwand reicht. So passt jede Kante, und in
  keiner Kamera und keinem Fenster endet der Tisch.
- **Der Rahmen** folgt weiter der Karte, wie in 0070.

## Verworfene Alternativen

- **Die Homographie der Vorlage,** wie bisher: bis 45 px daneben, gemessen
  per Phasenkorrelation.
- **Ein nahtloses Stück Marmor jenseits der Vorlage:** Holzränder und
  angeschnittene Gegenstände endeten am Rand der Vorlage mitten auf dem
  Tisch.
- **Ein grösseres Bild des Tischs, im Skript gespiegelt:** mehr Bytes für
  dasselbe Bild, und doch nur so weit, wie es reicht. Zur Laufzeit legt der
  Skin so viele Kacheln, wie die Leinwand braucht.
- **Grund jenseits der Vorlage,** wie bisher: In 1:1, von oben und
  genordet war ein Viertel bis ein Drittel des Fensters schwarz.

## Folgen

- Im Bezugsrahmen liegen alle fünf Gegenstände bei 0 px Versatz; ein Test
  hält sie auf 3 px, den Tisch auf 1 px.
- In anderen Kameras und Fenstern treffen sich jenseits der Vorlage
  gespiegelte Holzränder zu Ecken, und Gegenstände, die der Rand der
  Vorlage schneidet, laufen dort aus.
- In anderen Kameras ist der Marmor so verzerrt wie die Platte im Bild,
  in 2:1 etwa in t gestaucht; man merkt es nicht.
- Der Tisch kostet je Leinwand bis neun Aufrufe von `drawImage` statt einen;
  gemessen ist das nicht, gezeichnet wird nur nach Zoom und Zug.
