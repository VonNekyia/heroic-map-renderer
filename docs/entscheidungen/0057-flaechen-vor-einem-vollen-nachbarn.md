---
title: "0057: Flächen vor einem vollen Nachbarn nach der Tabelle des Spiels"
description: Warum der Renderer aus einer Tabelle des Spiels weiss, wo ein Block voll deckt, und eine Familie nur zu Seiten fragt, deren Flächen der Nachbar nicht ohnehin übermalt.
status: gilt
date: 2026-10-02
issues: [64]
code:
  - renderer/src/assets/Seiten.java
  - renderer/src/assets/seiten.txt
  - renderer/src/assets/blockstate.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0057: Flächen vor einem vollen Nachbarn nach der Tabelle des Spiels

## Anlass

#64: Das Spiel lässt eine Fläche mit `cullface` weg, wenn der Nachbar
dorthin voll deckt, im ersten Fall von `Block.shouldRenderFace`. Der
Renderer zeichnete sie. Zu sehen war das an den inneren Schichten der
Mangrovenwurzeln auf Schlamm und an den Innenwänden eines Spawners. Seit
[0044](0044-flaechen-zu-gleichen-nachbarn.md) stand der Fall als Näherung
unter „Folgen“, mit der Begründung, er zähle nur für innere Flächen, deren
`cullface` von der Kamera weg weist.

## Entscheidung

- **Wo ein Block voll deckt, kommt aus dem Spiel.** `Seiten.java` fragt je
  Zustand und Richtung, ob `getFaceOcclusionShape` genau `Shapes.block()`
  ist, und schreibt `seiten.txt`. Die Seiten gehören zum Schlüssel der
  Familie.
- **Eine Familie fragt nur zu Seiten, die etwas ändern können:** wo eine
  Fläche, die die Kamera sieht, ihre `cullface` hat und nicht zu dieser
  Seite zeigt und auf der Wand dorthin oder dahinter im Würfel des
  Nachbarn liegt (`auf_der_wand`). Liegt sie so, übermalt sie der volle
  Nachbar ohnehin. Beschrieben in
  [Sprites und Deckung](../renderer/sprites-und-deckung.md), „Flächen vor
  einem vollen Nachbarn“.
- **Dieselben Fassungen wie in 0044.** Die Seiten aus beiden Fällen bilden
  eine Maske; `sprite_at` setzt ein Bit, wenn der Nachbar voll deckt oder
  die Regel aus `nachbarn.txt` greift.

## Verworfene Alternativen

- **`licht.txt` nehmen.** Dort steht eine Seite nur bei Blöcken mit
  `useShapeForLightOcclusion`, Stein hat keine. Und „die ganze Seite“ heisst
  dort geometrisch voll. Das Spiel vergleicht hier das Objekt: Eine Treppe
  ist an ihrer Rückseite geometrisch voll, aber nie `Shapes.block()`, und
  lässt die Schicht der Mangrovenwurzeln stehen. Eine Probe gegen 26.2 hat
  das bestätigt.
- **Die Formen selbst nachrechnen.** Dafür bräuchte der Renderer die
  Formen aller Zustände und die Regel von `calculateFace` samt Teilungen.
  Die Tabelle trägt das Ergebnis in 10 kB.
- **Nur Seiten, die von der Kamera weg weisen,** wie #64 vorschlug. Beim
  Spawner zeigt die Nordwand innen zur Kamera und trägt die `cullface`
  Süden; ein voller Block im Süden lässt sie im Spiel weg, übermalt sie aber
  nicht. Entscheidend ist, ob die Fläche auf der Wand liegt.
- **Jede Seite mit `cullface`.** Dann hätte jeder volle Würfel Fassungen zu
  oben, Süden und Osten, achtmal so viele Sprites, und kein Pixel würde
  anders.
- **Genau prüfen, ob der Nachbar die Fläche übermalt,** über ihr Bild im
  Sechseck des Nachbarn. Für 26.2 ändert das nichts: Nur beim Trichter
  bekäme eine Seite keine Fassungen mehr.

## Folgen

- Eine achte Tabelle, die mit jeder Spielversion neu erzeugt wird, siehe
  [Erzeugte Tabellen](../entwicklung/tabellen.md).
- Mehr Fassungen je Alternative, aus `se`: Mangrovenwurzeln 16 statt 4,
  geflutet 128 statt 32; Spawner und Prüfungsspawner 8, der Trichter 2.
  Pulverschnee hatte über seine Regel schon alle sechs Seiten.
- Mehr Nachschläge je gezeichnetem Block, aus `se`: Mangrovenwurzeln 4
  statt 2, Spawner und Prüfungsspawner 3, der Trichter 1.
- Die Tabelle wächst: In den 17 Läufen der Doku-Bilder der Testwelt sind es
  1 bis 16 Sprites mehr, 0,6 bis 2,4 %. Für jeden anderen Block kostet
  `sprite_at` einen Vergleich wie vorher; eine Zeitreihe braucht es
  nicht.
- Der Trichter bekommt Fassungen, die nichts ändern: Das Innere seiner
  Schale übermalt der Block darüber.
- Ein Block, den 26.2 nicht kennt, deckt nirgends.
- Der dritte Fall, der Vergleich der Formen, fehlt weiter. In 26.2 ändert er
  kein Pixel, siehe [Sprites und Deckung](../renderer/sprites-und-deckung.md),
  „Flächen zu gleichen Nachbarn“.
