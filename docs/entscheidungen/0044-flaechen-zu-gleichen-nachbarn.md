---
title: "0044: Flächen zu gleichen Nachbarn nach der Regel des Spiels"
description: Warum der Renderer die Regel von skipRendering aus einer Tabelle des Spiels kennt, die cullface mit der Variante dreht und die Fassungen je Familie vorab rastert.
status: gilt
date: 2026-10-01
issues: [58]
code:
  - renderer/src/assets/Nachbarn.java
  - renderer/src/assets/nachbarn.txt
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/baker.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0044: Flächen zu gleichen Nachbarn nach der Regel des Spiels

## Anlass

#58: Eis, Glas, Scheiben und Gitter wirkten auf der Karte zu deckend. Der
Renderer zeichnete jede Fläche zur Kamera, auch die zwischen zwei gleichen
Blöcken. Durch eine Eisdecke sah die Karte drei Schichten, das Spiel eine.
Es lässt diese Flächen über `skipRendering` weg, siehe
[Sprites und Deckung](../renderer/sprites-und-deckung.md), „Flächen zu
gleichen Nachbarn“.

## Entscheidung

- **Die Regel kommt aus dem Spiel.** `Nachbarn.java` wertet `skipRendering`
  für jeden Block aus, dessen Klasse es überschreibt, an allen Paaren aus
  Zustand, Nachbarzustand und Richtung, mit den Tags des Spiels. Es schreibt
  je Block eine von drei Regeln in `nachbarn.txt` und bricht ab, wenn keine
  passt. Der Renderer kennt nur diese drei Regeln (`Nachbarregel` in
  `blockstate.rs`).
- **Die `cullface` dreht sich mit der Variante,** wie `Direction.rotate` im
  Spiel (`rotate_face` in `baker.rs`).
- **Die Fassungen sind vorab gerastert,** je Maske über die Seiten, zu denen
  eine Fläche zur Kamera ihre `cullface` hat und die Regel wirken kann, mit
  einer Flüssigkeit im Block zusammen mit deren Maske. Beim Zeichnen wählt
  `sprite_at` nur noch aus.
- **Die Fassungen gehören der Familie,** nicht dem Sprite.

## Verworfene Alternativen

- **Eine feste Liste der Blöcke im Code.** Sie müsste mit jeder Version
  jemand nachziehen. Den Tag `bars` bildete sie nur nach, und die Grenze
  zwischen gleichen Blöcken und Gruppen stünde nirgends geprüft.
- **Die Fassungen am Sprite, wie bei Wasser.** Zwei Familien teilen sich
  ein Sprite, wenn ihre Bilder Pixel für Pixel gleich sind. Ihre Modelle
  können dabei verschiedene `cullface` haben. Am Sprite gälte dann die
  Fassung der ersten Familie für beide.
- **Jede Maske über alle sechs Seiten.** Bei Eis wären das 64 statt 8
  Fassungen, und die meisten sähen gleich aus. Flächen, die die Kamera
  nicht sieht, und Seiten, zu denen die Regel nie wirkt, ändern nichts.
- **Die Flächen beim Zeichnen weglassen,** statt vorab zu rastern. Dann
  rasterte jeder Block mit Regel beim Zeichnen sein Modell neu. Der
  Renderpfad kopiert bisher nur fertige Pixel, siehe
  [Sprites und Deckung](../renderer/sprites-und-deckung.md), „Die
  Sprite-Tabelle“.
- **Den ersten Fall von `shouldRenderFace` mitnehmen,** die volle Seite des
  Nachbarn. Zur Kamera hin übermalt der Nachbar die Fläche ohnehin. Er
  zählt nur für innere Flächen, deren `cullface` von der Kamera weg weist,
  und gehört nicht zu #58.

## Folgen

- Eine siebte Tabelle, die mit jeder Spielversion neu erzeugt wird, siehe
  [Erzeugte Tabellen](../entwicklung/tabellen.md).
- Mehr Sprites: je Alternative eine Fassung je Maske über ihre Seiten,
  geflutet je Maske der Flüssigkeit noch einmal. Bei Eis sind das 8, bei
  Pulverschnee 64, denn seine inneren Schichten zeigen mit `cullface` zu
  allen sechs Seiten. Ein Pack mit inneren Flächen bringt je Alternative
  bis zu 64, geflutet bis zu 512. Dazu je gezeichnetem Block mit Regel ein
  Nachschlag je Seite, bei Eis drei, bei Mangrovenwurzeln zwei, bei
  Pulverschnee sechs. Am Stand und an einer Eisszene der Testwelt sind
  das 3 bis 4 % mehr Sprites und keine messbare Zeit, siehe
  [2026-10-01, Flächen zu gleichen Nachbarn](../messungen/2026-10-01-flaechen-zu-gleichen-nachbarn.md).
- Mitten in einer Eismasse fällt der Block weg, ohne Pixel.
- Innere Flächen vor einem vollen Nachbarn bleiben eine Näherung, siehe
  [Sprites und Deckung](../renderer/sprites-und-deckung.md), „Flächen zu
  gleichen Nachbarn“.
