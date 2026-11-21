---
title: "0146: Umpacken mit --repack"
description: Warum --repack einen fertigen Baum von maximal oder maximal_compact auf jede Stufe umpackt, ohne Welt und Assets, warum es mit zusammengelegten Farben die Eltern neu baut, an der Zeit statt am Hash fortsetzt, was es ablehnt und warum --compact-tree jetzt --repack mit maximal_compact ist.
status: gilt
date: 2026-11-19
issues: [421]
code:
  - renderer/src/cli/verdichten.rs
  - renderer/src/cli.rs
---

# 0146: Umpacken mit --repack

## Anlass

Ein Betreiber soll einen grossen Baum auf eine kleinere Stufe bringen, ohne
einen vollen Lauf über die Welt (#421). Bisher packte nur `--compact-tree`
einen fertigen Baum um, und nur von `maximal` nach `maximal_compact`, siehe
[0093](0093-nachverdichten.md). Die Stufen stehen in
[0143](0143-stufen-farben-zusammenlegen.md), die Gebiete in
[0145](0145-gebiete-mit-eigener-stufe.md).

## Entscheidung

- **`--repack <baum> --quality <stufe>`** packt einen fertigen Baum um, ohne
  Welt und Assets. Danach gleicht er Byte für Byte einem vollen Lauf mit
  dieser Stufe und denselben Gebieten. Je Kachel gilt die höchste Stufe aus
  dem Ziel und den gemerkten Gebieten.
- **Nur von `maximal` oder `maximal_compact` aus.** Ein Baum mit
  zusammengelegten Farben hat die Pixel davor nicht mehr; zurück führt nur
  ein voller Lauf mit `--quality`.
- **`--compact-tree` ist `--repack` mit `maximal_compact`,** mit demselben
  Code. Es nimmt jetzt auch Bäume mit Gebieten; das löst den Punkt
  „`--compact-tree` lehnt einen Baum mit Gebieten ab“ aus 0145 ab.
- **Verlustfrei nach verlustfrei** bleibt wie in 0093: jede Stufe der
  Pyramide neu gepackt, dieselben Pixel, jede Kachel mit ihrer alten Zeit.
- **Mit zusammengelegten Farben:**
  - Basis und native Stufen packt es aus den Pixeln der Datei, mit der
    Vorstufe wie ein Lauf. Der Hash gilt den Pixeln davor, mit dem Präfix
    des Ziels.
  - Eine so gepackte Kachel bekommt die Zeit des Umpackens.
  - Am Ende baut es jede Elternkachel über den nativen Stufen aus ihren
    Kindern neu. Die nativen Stufen lässt die Pyramide dabei, wie sie
    sind; sie sind gezeichnet, nicht verkleinert.
- **`repack-offen.json`** mit Quelle, Ziel und Beginn liegt, solange ein
  Umpacken mit zusammengelegten Farben nicht fertig ist. Ein neuer Aufruf
  mit demselben Ziel setzt fort, einer mit einem anderen bricht ab. Jeder
  volle Lauf räumt die Datei weg.
- **Erst `repack-offen.json`, dann `map.json`:** Endet der Aufruf
  dazwischen, nennt der Baum noch die Quelle, und das Fortsetzen trägt das
  Ziel nach. In der anderen Reihenfolge bliebe ein Baum, der das Ziel nennt,
  aber ganz verlustfrei ist, und den `--repack` dann ablehnt. Nennt
  `map.json` beim Fortsetzen das Ziel noch nicht, beginnt das Umpacken neu:
  Ein Update dazwischen packte noch mit der Quelle. Die Stufe des Baums
  sagt dann `map.json`, denn ein voller Lauf kann seine schon eingetragen
  haben, bevor er die Datei wegräumte.
- **Was schon umgepackt ist, sagt die Zeit:** Eine Kachel, die jünger ist
  als der Beginn, schrieb das Umpacken oder ein Update mit dem Ziel; sie
  bleibt. Eine ältere ist noch verlustfrei.
- **Ablehnen,** bevor es etwas schreibt:
  - wenn `gebiete-neu.json` liegt: Ein abgebrochener Lauf kann andere
    Gebiete haben;
  - mit zusammengelegten Farben, wenn `stand-neu.bin` liegt;
  - mit Gebieten ohne `stand.bin`, wie `--pyramid`.
- **Kein `--quality-areas` an `--repack`.** Es packt nach den gemerkten
  Gebieten; eine neue Datei setzt das nächste Update um.

## Verworfene Alternativen

- **Eltern einzeln umpacken:** Ein Lauf mittelt die Eltern aus den
  zusammengelegten Kindern. Eltern aus ihren eigenen Pixeln
  zusammenzulegen gäbe andere Bytes als ein voller Lauf.
- **Am Hash fortsetzen:** Ein Abbruch verliert die Hashes der Blöcke in
  Arbeit, und Bäume ohne Hashes gibt es. Eine Kachel ohne Hash, deren
  Farben schon zusammengelegt sind, packte ein zweiter Aufruf noch einmal.
  Die Pixel blieben dabei gleich, denn ein zweites Zusammenlegen ändert
  nichts (`zusammenlegen_zweimal_wie_einmal`). Ihr Hash gälte dann aber den
  zusammengelegten Pixeln statt denen davor: Der nächste Lauf erkennte die
  Kachel nicht und packte sie umsonst neu.
- **Die alte Zeit behalten wie `--compact-tree`:** Dann liesse sich eine
  umgepackte Kachel beim Fortsetzen nicht von einer verlustfreien
  unterscheiden. Die Pixel ändern sich ja.
- **Den Eltern trauen, wenn ihre Kinder älter sind:** Eine native Kachel
  in einem Gebiet bleibt verlustfrei mit alter Zeit, ihre Eltern hingen
  dann an der Packung von vorher. Alle Eltern neu zu bauen kostet an der
  Testwelt Sekunden, siehe [Umpacken](../benutzung/umpacken.md).
- **`--quality-areas` an `--repack`:** Der Abbruch bräuchte dann auch die
  Gebiete des laufenden Umpackens. Das Plugin braucht den Schalter dort
  nicht.

## Folgen

- Ein Baum mit Gebieten lässt sich wieder verdichten.
- Jeder Spieler mit dem Mod lädt einen umgepackten Baum zum Download einmal
  neu, wie nach einem vollen Lauf mit anderer Stufe, siehe 0143.
- Wer einen halb umgepackten Baum kopiert und den Dateien dabei neue Zeiten
  gibt, macht jede Kachel jünger als den Beginn. Das Fortsetzen hielte dann
  alles für umgepackt; nur ein voller Lauf macht den Baum wieder richtig,
  siehe [Umpacken](../benutzung/umpacken.md), „Abbruch“.
