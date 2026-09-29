---
title: "0030: Licht je Block beim Zeichnen"
description: Warum unter Wasser jeder Block beim Zeichnen in seinem Licht liegt, statt dass eine Oberfläche die Deckkraft aller Schichten dahinter trägt; abgelöst durch das Licht aus der Ausbreitung.
status: abgelöst durch 0040
date: 2026-09-27
issues: [14, 17]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/render/gpu.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/leuchten.txt
---

# 0030: Licht je Block beim Zeichnen

Abgelöst durch
[0040: Licht selbst ausbreiten, je Chunk mit Rand](0040-licht-selbst-ausbreiten.md):
Das Licht zählt nicht mehr `light_at` aus den Blöcken darüber, es kommt aus
der Ausbreitung, und volle Würfel bekommen es an jeder Ecke. Die eine
Oberfläche und die Multiplikation beim Blit bleiben.

Löst
[0010: Tiefe entlang des Blickstrahls](0010-tiefe-entlang-des-blickstrahls.md)
ab.

## Anlass

#14: Wasser deckte zu stark, und an Holzbauten unter Wasser standen blasse
Flecken. Die Oberfläche trug die Deckkraft aller Schichten dahinter, so als
läge je Block Wasser eine weitere Oberfläche darüber. Im Spiel sieht man
durch genau eine Oberfläche, und jeder Block Wasser nimmt eine Stufe
Himmelslicht.

## Entscheidung

Man sieht durch genau eine Oberfläche mit Alpha 180. Darunter bestimmt
`light_at` für jeden Block, in welchem Himmelslicht das Spiel ihn zeichnet,
und was selbst leuchtet, bringt sein Blocklicht mit. Der Blit multipliziert
jeden Pixel je Kanal mit der Helligkeit dazu, ganzzahlig wie das Mischen,
auf der CPU wie im Shader. Über dem Grund D ergibt die Oberfläche W so α ·
W + (1 − α) · b · D. Siehe
[Wasser und Licht](../renderer/wasser-und-licht.md).

## Verworfene Alternativen

- **Das Licht in der Oberfläche**, wie im Issue geplant: Tiefenstufe d des
  Oberflächen-Sprites mit Deckkraft 1 − 0,294 · b(14 − d), gezählt je Hälfte
  der Oberseite. So zuerst gebaut, mit Erfolg für ebenen Grund, aber:
  - Eine Oberfläche trägt ein Licht für alles hinter ihr. Kelp knapp unter
    der Oberfläche lag im Licht des Grundes, über 20 Blöcken Wasser bei 3 %
    statt 24 %; im Spiel sieht man Tangwälder von oben.
  - 15 Tiefen je Oberfläche, je Hälfte und je Biom: Für die ganze grosse
    Welt waren es 165 212 statt 56 761 Sprites, die Tabelle brauchte 34
    statt 12 s.
- **Vier statt fünf Wörter je Instanz**, die Helligkeit aus einer Tabelle:
  brachte auf der Karte nichts (2246 und 2187 Kacheln/s bei gleichen
  Pixeln).
- **Eine Abkürzung, die an Land gar nicht zählt:** brachte auf einem Thread
  nichts (5,59 statt 5,56 ms).

## Folgen

- Eine Multiplikation je Pixel für Blöcke unter Wasser, auf CPU und Karte,
  und ein weiteres Wort je Instanz im Shader.
- Die Sprite-Tabelle der grossen Welt hat bei scale 32 26 341 statt 56 761
  Sprites und steht in 5,3 statt 11,8 s.
- Die Kacheln werden grösser, weil man jetzt ins Wasser sieht; das zeigt das
  Spiel ebenso. Siehe
  [2026-09-27, Wasser im Licht](../messungen/2026-09-27-wasser-im-licht.md).
- Näherungen: eine Zahl je Block, Licht von der Seite nur an zwei Stellen,
  Luft und Glas unter Wasser ohne Verlust, Blocklicht nur am Block selbst,
  kein Flackern; siehe die Seite.
