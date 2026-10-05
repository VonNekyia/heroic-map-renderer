---
title: "0079: Tablett vertagt, vorerst nur Marmor"
description: Warum der Skin Tablett vorerst nur den Marmor um die Karte legt, ohne Rahmen, Tisch und Gegenstände, auch um Welten, die kein Quadrat sind; wie das ganze Tablett als voll.ts erhalten und getestet bleibt und warum es kein eigener Skin ist. Ergänzt 0061 und 0063.
status: gilt
date: 2026-10-05
issues: [167, 112]
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/voll.ts
  - web/skins/tablett/zeichnen.ts
  - web/playwright.config.ts
---

# 0079: Tablett vertagt, vorerst nur Marmor

Ergänzt [0061](0061-tablett-im-frontend.md) und
[0063](0063-tablett-als-skin.md); beide gelten weiter.

## Anlass

Der Maintainer hat am 05.10. das Tablett aus #112 mit Rahmen, Tisch und
Gegenständen vertagt, ebenso die Szene aus Blender
([0074](0074-tablett-aus-blender.md)). Vorerst soll um die Karte nur der
Marmor liegen, so bleibt Raum für spätere Verbesserungen (#167).

## Entscheidung

- **`SKIN=./skins/tablett`** legt nur den Marmor um die Kacheln, wie bisher
  jenseits der Vorlage:
  - an der Welt verankert, er wandert und zoomt mit der Karte;
  - Pixelkunst ohne Glättung, solange ein Block ein Pixel deckt, sonst
    geglättet ([0075](0075-marmor-als-pixelkunst.md));
  - geladen nach den Kacheln, nur `marmor.webp`
    ([0073](0073-bilder-nach-den-kacheln.md)).
- **Kein Quadrat nötig:** `area` und `seaLevel` reichen. Die Lage rechnet
  der Skin wie die des Tischs aus `area`, auch wenn es kein Quadrat ist.
- **Ohne Tablett:** keine nahe Ebene, kein Rahmen, kein Tisch, keine
  Lilien, Gegenstände oder Buchrücken. Gesamtansicht und Grenzen sind die
  der Grundkarte; der Skin gibt keine `ganzeKarte` zurück.
- **Die UI** aus Pergament, Holz und Messing bleibt
  ([0072](0072-ui-in-farben-der-vorlage.md)). Ausweichen muss sie nichts.
- **Das ganze Tablett** bleibt im selben Skin, als zweiter Einstieg
  `web/skins/tablett/voll.ts`, wählbar mit `SKIN=./skins/tablett/voll`.
  Beide Einstiege rufen `skinTablett` aus `index.ts`, einmal mit
  `nurMarmor`. Der Marmor ist dabei die eine Fläche aus `tablett()`, die
  sich wiederholt; gemalt wird er von `ebenen` und `lege` in
  `zeichnen.ts` wie im ganzen Tablett. Die Malerei gibt es so nur einmal.
- **Tests:** Die des ganzen Tabletts laufen weiter, an einem eigenen Build
  mit `voll.ts` (Projekt `tablett` in `web/playwright.config.ts`). Der
  Marmor allein hat seine Tests in `marmor.spec.ts`.

## Verworfene Alternativen

- **Ein eigener kleiner Skin `marmor`:** Er bräuchte Lage, Bild und
  Malerei des Marmors aus dem Tablett. Kopiert gäbe es sie zweimal (Regel
  7), geteilt hinge ein Skin am Code eines anderen; auslagern liesse sich
  keiner mehr allein (0063).
- **Ein Schalter beim Build, etwa eine Variable:** Die Grundkarte gibt dem
  Skin nur Texte aus `SKIN_TEXT_*`; ein Schalter dafür wäre neue
  Schnittstelle für einen Übergang (Regel 22). Der Pfad des Moduls wählt
  schon heute.
- **Den Code des Tabletts löschen:** Der Maintainer will ihn für später
  behalten.
- **Die Tests des Tabletts aussetzen:** Ungetesteter Code verfällt. Ein
  dritter Build kostet die Suite wenige Sekunden.

## Folgen

- Der Skin hat zwei Einstiege, `index.ts` und `voll.ts`. `package.json`
  exportiert weiter nur `index.ts`; `voll` wählt man über den Pfad.
- Die Bilder des Tabletts liegen weiter im Build, geladen wird nur der
  Marmor.
- Wie gross ein Block des Marmors ist, hängt weiter an der Grösse von
  `area`, wie beim Tisch.
- Kommt das Tablett zurück, ist es ein Wechsel des Einstiegs, keine
  Neuentwicklung.
