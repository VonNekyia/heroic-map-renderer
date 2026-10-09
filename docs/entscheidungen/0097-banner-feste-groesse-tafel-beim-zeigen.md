---
title: "0097: Banner für Orte, feste Grösse, Tafel beim Zeigen"
description: Warum Ebenen ein Banner als eigenes Objekt bekommen, warum Nadeln und Banner auf jeder Stufe gleich gross bleiben statt nach Zoom gestuft, und warum die Tafel schon beim Zeigen erscheint; löst die Stufen und die Tafel nur beim Anklicken aus 0095 ab.
status: gilt
date: 2026-10-10
issues: [219]
code:
  - web/src/ebenen.ts
---

# 0097: Banner für Orte, feste Grösse, Tafel beim Zeigen

## Anlass

Der User will drei Änderungen an den Ebenen (#219), für Webkarte und Mod:

- **Städte als Banner:** Eine Stadt zeigt das Banner ihrer Nation, eine
  Stadt ohne Nation ein weisses. Die Nadel bleibt für Wegpunkte.
- **Feste Grösse:** Nadeln und Banner bleiben beim Hinauszoomen gleich
  gross. Das kehrt eine Entscheidung aus [0095](0095-ebenen.md) um: Dort
  wurden sie nach Ortstyp und Zoom kleiner und verschwanden zuletzt.
- **Tafel beim Zeigen:** Die Tafel erscheint schon, wenn der Zeiger auf
  einer Nadel, einem Banner oder einer Fläche mit Tafel ruht. Ein Klick hält
  sie offen, auf dem Telefon reicht Tippen.

## Entscheidung

Das Format steht in [Ebenen](../benutzung/ebenen.md). Im Kern:

- **Ein neues Objekt `banner`:** `at`, `y`, `image`, `name`, `panel` und
  `dimension` wie bei der Nadel. Das Bild liegt wie jedes Bild der Ebene
  unter `images/`, PNG oder WebP `VP8L`, höchstens 32 × 64 Pixel. Die
  Ansicht zeichnet es Pixel auf Pixel, nie skaliert, unten mittig auf dem
  Ort. Jedes Banner bringt sein Bild mit; für eine Stadt ohne Nation
  schickt das Plugin ein weisses.
- **Feste Grösse:** Nadeln und Banner stehen auf jeder Stufe in der Grösse
  ihres Bilds, in Pixeln der Ansicht. Die Stufen nach der Breite eines
  Blocks fallen weg. `size` bleibt die feste Wahl der Nadel zwischen
  `large`, `medium` und `small`. Der Name steht immer darunter.
- **Tafel beim Zeigen:**
  - Ruht der Zeiger 150 ms auf einem Ziel mit Tafel, erscheint sie.
  - Verlässt er Ziel und Tafel, schliesst sie nach 300 ms; dazwischen kann
    er in die Tafel wandern, etwa zum Scrollen.
  - Ein Klick hält sie offen, bis zum Schliessknopf, Escape oder einem
    Klick daneben.
  - Ohne Zeiger, auf dem Telefon, öffnet Tippen sie wie bisher. Per Tastatur
    öffnet Enter sie wie bisher.

## Verworfene Alternativen

- **Das Banner als Symbol der Nadel:** Symbole sind 16 × 16 oder 9 × 9
  Pixel. Ein Banner von 22 × 40 passte nur verkleinert, und Pixelkunst
  verliert dabei ihre Pixel.
- **Banner skaliert auf eine feste Höhe:** Gleich hohe Banner sähen
  einheitlicher aus, aber jedes Bild würde unscharf oder verlöre Zeilen.
  Pixel auf Pixel bleibt die Regel wie bei Symbolen und Bildern der Tafel.
- **Ein weisses Banner aus der Ansicht, wenn das Bild fehlt:** Jede Ansicht
  bräuchte dasselbe Ersatzbild, und es wiche vom Stil des Plugins ab. Das
  Plugin kennt die Nation und schickt das passende Bild, auch das weisse.
- **Die Stufen behalten:** Sie halten die Gesamtansicht einer grossen Welt
  frei; das war der Grund in 0095. Der User will trotzdem feste Grössen.
- **Die Tafel sofort beim Zeigen:** Fährt die Maus über viele Nadeln, blitzt
  eine Tafel nach der anderen auf. 150 ms Ruhe genügen, damit nur die Tafel
  erscheint, auf der der Zeiger stehen bleibt.
- **Die Tafel gleich beim Verlassen schliessen:** Der Weg vom Ziel in die
  Tafel führt oft über einen Spalt; sie schlösse, bevor der Zeiger ankommt.

## Folgen

- **Abgelöst in 0095:** die Stufen der Nadel beim Hinauszoomen und die Tafel
  nur beim Anklicken. Der Rest von 0095 gilt.
- **Volle Karte:** In der Gesamtansicht einer grossen Welt können Nadeln und
  Banner die Karte decken. Abhilfe ist, Ebenen auszuschalten.
- **Grenzen:** Ein Banner ist höchstens 32 × 64 Pixel. Nadeln und Banner
  zählen zusammen gegen die 1000 einer Ebene. Ein Banner ohne gültiges Bild
  übergeht jede Ansicht mit Meldung, wie jede Verletzung einer Grenze.
- **Plugin und Mod:**
  - Das Plugin prüft Format und Grösse des Bilds beim Aufruf.
  - Der Mod liest dieselben Bilder und zeichnet Banner wie seine Wegpunkte,
    in Einheiten seiner Oberfläche.
  - Eine Ebene mit `permission` hat vorerst keine Bilder, also auch keine
    Banner.
- **Prüfen:** Jede Ansicht prüft diese Grenzen mit Tests:
  - ein Banner von 32 × 64 Pixeln erscheint, eines von 33 × 64 oder 32 × 65
    nicht, mit Meldung;
  - ein Bild ausserhalb von `images/` oder in einem anderen Format holt sie
    nicht;
  - Nadeln und Banner haben auf der feinsten Stufe und weit draussen
    dieselbe Grösse;
  - die Tafel erscheint nach 150 ms Ruhe und schliesst 300 ms nach dem
    Verlassen, hält per Klick, öffnet per Tippen und per Enter.
