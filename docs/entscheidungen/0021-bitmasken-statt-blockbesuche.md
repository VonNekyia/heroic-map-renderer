---
title: "0021: Bitmasken statt Blockbesuche"
description: Warum die Kandidaten einer Kachel aus Bitmasken je Section kommen, ohne Luft und Verdecktes anzufassen, und was dabei gemessen nichts brachte.
status: gilt
date: 2026-09-26
issues: [10]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/world/palette.rs
---

# 0021: Bitmasken statt Blockbesuche

## Anlass

Der Renderer lief über jede der rund 317 000 Positionen im Band einer
Kachel, 268 000 davon Luft, fragte je Block die Familie ab und für jeden
nicht-leeren Block drei Nachbarn. Für neun von zehn wählte er erst das
Sprite, bevor er merkte, dass der Block verdeckt ist. Der erste Vollrender
der grossen Welt brauchte so für die Basis knapp 16 Stunden.

## Entscheidung

Jede Section hält je Spalte ein 16-Bit-Wort je Eigenschaft (Bit = y):
vorhanden, deckend, deckt den Boden, Wasser, Lava, nur Wasser, nur Lava,
lose, Teile in Nachbarwürfeln. Verdeckt ist ein Block, wenn die Nachbarn
nach +x und +z deckend sind und der nach +y seinen Boden deckt; das sind je
Spalte ein paar Wortoperationen für sechzehn Blöcke. Reine Flüssigkeit im
Inneren fällt weg. Die Kandidaten fallen aus den Masken, sortiert nach
`(y, v, u)` in Zeichenreihenfolge. Die Masken entstehen je Klasse gleicher
Bits, mit einem OR je Block. `render_area_without_culling` bleibt als
Referenz ohne jede Abkürzung. Siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Bitmasken“.

## Verworfene Alternativen

Gemessen, und weil sie nichts brachten, nicht im Code:

- **Zeilenspannen im Blit samt `memcpy` deckender Zeilen:** pixelgleich,
  aber nicht schneller. Der Blit wartet auf Sprite-Pixel aus dem Speicher,
  nicht auf die Schleife.
- **Der Nachbar auf der Blickachse als vierte Deckungsrichtung:** kommt zu
  selten vor, und die Prüfung je Pixel kostet mehr, als sie spart.
- **`zlib-rs` statt `miniz`** zum Entpacken der Chunks: kein Unterschied.
- **Ein eigener serde-Visitor für die Properties:** ohne messbaren Gewinn,
  nach dem Review zurückgenommen.

## Folgen

- Einfädig 10-mal so schnell, auf 24 Threads 5,5-mal, Byte für Byte dasselbe
  Bild, siehe [2026-09-25, Bitmasken](../messungen/2026-09-25-bitmasken.md).
- Jede Eigenschaft, die über Verdeckung, Licht oder weiche Beleuchtung
  entscheidet, wird eine weitere Ebene; so kamen `DARK` und `VIEW` dazu,
  siehe [0031](0031-eigene-tabellen-statt-der-masken.md).
- Ein Dach über reiner Flüssigkeit genügt nicht: ohne dieselbe Flüssigkeit
  darüber endet die Oberfläche bei 8/9 und ragt in die Seiten.
