---
title: "0002: Deckend entscheidet das fertige Sprite"
description: Warum am gerasterten Bild und nicht am Modell entschieden wird, ob ein Block deckt, Pixel für Pixel und ohne Toleranz.
status: gilt
date: 2026-09-22
issues: [4, 8]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0002: Deckend entscheidet das fertige Sprite

## Anlass

Neun von zehn nicht-leeren Blöcken liegen unter der Oberfläche. Verdeckte
Würfel sollen wegfallen, und dafür muss feststehen, welche Blöcke decken.

## Entscheidung

Ob ein Sprite deckt, entscheidet sein fertiges Bild: Füllt es den Umriss
seines Würfels lückenlos mit Alpha 255, Pixel für Pixel gegen einen
gerasterten vollen Würfel? Glas fällt damit von selbst heraus. Ein
verdeckter Block fällt nur weg, wenn sein Sprite Pixel für Pixel in diesem
Umriss bleibt. Siehe [Sprites und Deckung](../renderer/sprites-und-deckung.md).

## Verworfene Alternativen

- **Am Modell oder mit einer gepflegten Blockliste.** Ein Modell sagt nichts
  über durchsichtige Texel, und eine Liste müsste jemand mit jedem Pack
  pflegen.
- **Eine Pixelbreite Toleranz.** Dann fiele der Block unter einer
  Druckplatte weg, und ihr Rand zeigte den Hintergrund.

## Folgen

- Schilder, Weizen, Rote Bete, Schienen, Feuer und das Lesepult legen je
  nach scale ein paar Pixel neben den Umriss; sie werden immer gezeichnet.
- Nachbarn verdecken nur bei Vielfachen von 4 als scale; bei anderen liegen
  Blöcke auf halben Pixeln, und ihre Umrisse schliessen nicht lückenlos an.
