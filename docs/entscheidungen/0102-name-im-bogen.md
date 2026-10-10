---
title: "0102: Der Name eines Banners im Bogen"
description: Warum der Name unter einem Banner auf einem Bogen läuft, wie Radius, Lage und Sperrung aus Schriftgrösse und Höhe des Banners folgen, warum der Bogen im schrägen Satz mit der Unterkante des Tuchs dreht und warum Nadeln gerade bleiben; ergänzt 0100.
status: gilt
date: 2026-10-10
issues: [262]
code:
  - docs/bilder/quellen/name-bogen/name-bogen.html
---

# 0102: Der Name eines Banners im Bogen

## Anlass

Der User will den Namen unter einem Banner gebogen, für das Aussehen, auf
Webkarte und im Mod gleich (#262). Bisher stand er gerade unter dem Fuss,
nach [0100](0100-der-renderer-zeichnet-die-banner.md) im schrägen Satz
parallel zur Unterkante des Tuchs gedreht.

## Entscheidung

Die Masse stehen in [Ebenen](../benutzung/ebenen.md), „Nadeln und Banner“
unter „Zeichnen“. Im Kern:

- **Ein Kreisbogen unter dem Fuss,** nach unten gewölbt, jedes Zeichen
  aufrecht zum Bogen wie die Kartenschrift an ihrem Pfad.
- **Radius aus der Höhe des Banners,** `2 · h`. Lange Namen öffnen den Bogen
  bis 120°, darüber wird er flacher.
- **Sperrung** `0,125 · s` zwischen den Zeichen, 2 Pixel bei 16; der User
  wählte sie aus 0, 1, 1,6 und 2 Pixeln am Bild.
- **Im schrägen Satz dreht der ganze Bogen** um den Winkel aus `satz.json`
  um den Fuss. Der User hatte keine Vorliebe; es gilt der Vorschlag.
- **Nadeln bleiben gerade,** Wahl des Users.

Gewählt hat der User an Bildern wie
[`name-bogen.png`](../bilder/name-bogen.png), gezeichnet mit derselben
Regel.

## Verworfene Alternativen

- **Im schrägen Satz nur gebogen, nicht gedreht:** Der Bogen stünde
  waagrecht unter einem Tuch, dessen Unterkante fällt, und wirkte schief
  gegen das Banner.
- **Im schrägen Satz gerade und gedreht,** wie 0100 es vorsah: nicht, was
  der User will, einen Bogen.
- **Auch Nadeln im Bogen:** Wegpunkte stehen oft dicht; gerade Namen
  bleiben dort ruhiger, und der Bogen bleibt das Zeichen einer Stadt.
- **Ein fester Radius:** Ein kurzer Name stünde in einem engen Bogen, ein
  langer liefe um das Banner herum. Mit der Grenze von 120° bleibt jeder
  lesbar.

## Folgen

- **Ergänzt 0100** im Punkt „Der Name“: Statt gerade parallel zur
  Unterkante läuft er im Bogen, der mit ihr dreht. Der Rest von 0100 gilt.
- **Webkarte und Mod** bauen je eine PR. Die Webkarte zeichnet den Bogen als
  SVG-Pfad mit `textPath`, wie ihre Kartenschrift; der Mod setzt die Zeichen
  einzeln gedreht, wie seine Kartenschrift. Bilder beider nebeneinander zur
  Abnahme.
- **Kosten** nach Regel 26: keine Renderzeit, kein Platz; je Banner ein
  Pfad in der Ansicht.
