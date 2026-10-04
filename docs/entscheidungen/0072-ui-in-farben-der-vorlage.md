---
title: "0072: UI des Tabletts in Farben der Vorlage"
description: Warum die UI mit dem Skin Tablett auf Pergament, Holz und Messing liegt, in Farben, die in der Vorlage gemessen sind, über die CSS-Variablen der Grundkarte und einen Verlauf als Rand aus Messing, ohne Bilddatei, und in der Gesamtansicht den Gegenständen ausweicht; verworfen sind ein erzeugtes Bild als Rahmen der Knöpfe, ein Ausschnitt der Vorlage, alles auf Holz und feste Plätze nach der Vorlage.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/src/style.css
  - web/src/skin-api.ts
  - web/skins/tablett/tablett.css
  - web/skins/tablett/index.ts
---

# 0072: UI des Tabletts in Farben der Vorlage

## Anlass

Nach dem Nachtrag des Maintainers folgt auch die UI dem Skin, in Holz,
Messing und Pergament (#112, issuecomment-5969255392). Die Grundkarte gibt
dafür CSS-Variablen, der Skin setzt sie in seinem Stylesheet und darf Bilder
für die Knöpfe ergänzen. Ein erster Entwurf legte die Knöpfe in ein Bild aus
dem Skript der Texturen; dieses Skript gibt es seit
[0070](0070-bilder-aus-der-vorlage.md) nicht mehr.

## Entscheidung

Festgelegt am 04.10. mit der PR zur UI (#112):

- **Grundkarte:** Farben, Rand, Radius, Schrift und Abstand der UI
  sind CSS-Variablen in `:root` von `web/src/style.css`, mit den Werten von
  vor den Variablen, auch für gesperrte Knöpfe von Leaflet. Ihre Namen nennt
  `UiVariable` in `web/src/skin-api.ts`; `VERSION` ist 2.
- **Farben aus der Vorlage,** je Stelle der Median: Pergament im Licht,
  Tinte der Skizze, Holz der Wand, Messing des Bands, Rot der Bücher. Die
  Tabelle steht in [Tablett](../tablett.md), „UI“.
- **Pergament mit Tinte** für Leiste, Stand und Kompass; **Holz in Messing**
  für Knöpfe und Umschalter, mit Schrift in hellem Messing.
- **Ohne Bilddatei:** Den Rand aus Messing zeichnet ein Verlauf als
  `border-image`, hell oben links, dunkel unten rechts.
- **Neben den Gegenständen,** nach Befund 4 des Reviews zu #120
  (issuecomment-5976163815): In der Gesamtansicht weicht jede Ecke der UI
  Gegenständen und Lilien entlang ihres Rands aus, gerechnet aus der Lage
  ihrer Bilder.

## Verworfene Alternativen

- **Ein erzeugtes Bild als Rahmen,** 12 px mit 3 px Messing, wie im ersten
  Entwurf. Sein Skript löst 0070 ab; ein eigenes Skript nur für dieses Bild
  wäre mehr Code als die Regel in CSS.
- **Ein Ausschnitt der Vorlage als Rahmen.** Die Vorlage hat keine Knöpfe,
  und vom Band der Oberkante bliebe auf 3 px nur seine Farbe, wie im
  Verlauf.
- **Alles auf Holz mit heller Schrift,** wie im ersten Entwurf. Der Nachtrag
  nennt Pergament, und Tinte auf Pergament liest sich wie die Skizze in der
  Vorlage, mit 6,4:1.
- **Ein Ring mit Licht am Kompass.** Der Kompass dreht sich mit Norden, das
  Licht drehte mit.
- **Der Fokus in den Farben des Skins auch an der Karte selbst.** Tinte sähe
  man auf dem dunklen Tisch nicht; die Karte behält den Fokus des Browsers.
- **Feste Plätze nach der Vorlage,** etwa in Prozent des Fensters. In der
  Vorlage stehen die Gegenstände in den Ecken; in breiteren Fenstern liegt
  sie mit Rand in der Mitte, und Plätze, die dort frei sind, decken in
  anderen etwas.

## Folgen

- Ein Skin für die Schnittstelle 1 bleibt aus; der einzige ist das Tablett.
- Die Knöpfe sind eckig: Leaflet rundet den ersten und letzten selbst, der
  Skin hebt das auf.
- Mehr Variablen heissen eine neue `VERSION`; wer eine braucht, ergänzt sie
  in `style.css` und `skin-api.ts` zugleich, ein Smoke-Test vergleicht
  beide.
