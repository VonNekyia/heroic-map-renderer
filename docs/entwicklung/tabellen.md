---
title: Erzeugte Tabellen
description: Die Tabellen aus dem Spiel, blocks.txt, leuchten.txt, licht.txt, schatten.txt, nachbarn.txt, seiten.txt, blockentities.txt und dimensionstypen.txt - was darin steht, gegen welche Version, welche Tests sie festhalten und wie man sie neu erzeugt.
code:
  - renderer/src/assets/blocks.txt
  - renderer/src/assets/leuchten.txt
  - renderer/src/assets/licht.txt
  - renderer/src/assets/schatten.txt
  - renderer/src/assets/nachbarn.txt
  - renderer/src/assets/seiten.txt
  - renderer/src/assets/blockentities.txt
  - renderer/src/assets/dimensionstypen.txt
  - renderer/src/assets/Leuchten.java
  - renderer/src/assets/Licht.java
  - renderer/src/assets/Schatten.java
  - renderer/src/assets/Nachbarn.java
  - renderer/src/assets/Seiten.java
  - renderer/src/assets/Blockentities.java
  - renderer/src/assets/Dimensionstypen.java
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/blockentity.rs
  - renderer/src/assets/dimension.rs
---

# Erzeugte Tabellen

Was Minecraft im Code verdrahtet und der Renderer braucht, steht in acht
Tabellen unter `renderer/src/assets/`, sechs aus dem Server-JAR von 26.3,
zwei aus dem Client-JAR, und ins Binär einkompiliert (`blockstate.rs`,
`blockentity.rs`, `dimension.rs`). Von Hand werden sie nie geändert; für
eine andere Version erzeugt sie der Skill
[`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md) neu,
danach muss der Renderer neu gebaut werden.

## Die Tabellen

Haben alle Zustände eines Blocks dasselbe Zeichen, steht es einmal.

| Datei | Inhalt | Quelle | Stand |
|---|---|---|---|
| [`blocks.txt`](../../renderer/src/assets/blocks.txt) | jeder Block mit seinen Eigenschaften und Werten | `generated/reports/blocks.json` des Datengenerators | 1286 Blöcke aus 26.3 |
| [`leuchten.txt`](../../renderer/src/assets/leuchten.txt) | je Zustand in der Reihenfolge von `getPossibleStates` eine Ziffer 0 bis f für `getLightEmission`, mit `emissiveRendering` dieselbe Stufe als Buchstabe g bis v; Blöcke, die nie leuchten, fehlen | `Leuchten.java` | 109 Blöcke aus 26.3 |
| [`licht.txt`](../../renderer/src/assets/licht.txt) | je Zustand in der Reihenfolge von `getPossibleStates` sieben Zeichen: `getLightDampening` (0, 1 oder f), dann je Richtung von `Direction.values()` die Fläche, mit der er das Licht an dieser Seite aufhält, zur Basis 36: 0 keine, 1 die ganze Seite, ab 2 eine Teilfläche. Am Ende je Achse die Paare aus Teilflächen, die zusammen eine Seite decken, als `paar <achse> <a> <b>`; Blöcke, die das Licht nirgends aufhalten, fehlen | `Licht.java` | 933 Blöcke mit 21 426 Zuständen und 288 Paare aus 26.3 |
| [`schatten.txt`](../../renderer/src/assets/schatten.txt) | je Zustand in der Reihenfolge von `getPossibleStates` eine Ziffer, Bit 1 für `getShadeBrightness` 0,2, Bit 2 für nicht `isLightPermeable`, Bit 4 für `isCollisionShapeFullBlock`; Blöcke ohne Bit fehlen | `Schatten.java` | 558 Blöcke aus 26.3 |
| [`nachbarn.txt`](../../renderer/src/assets/nachbarn.txt) | je Block mit eigenem `skipRendering` eine Zeile: der Name, die Regel `gleich`, `senkrecht` oder `verbunden` und bei `verbunden` womöglich der Tag seiner Gruppe. Wasser und Lava fehlen, Laub mit den Vorgaben des Spiels auch | `Nachbarn.java`, mit den Tags des Spiels | 59 Blöcke aus 26.3 |
| [`seiten.txt`](../../renderer/src/assets/seiten.txt) | je Zustand in der Reihenfolge von `getPossibleStates` zwei Hexziffern: die Seiten, an denen `getFaceOcclusionShape` genau `Shapes.block()` ist, ein Bit je Richtung von `Direction.values()`, 1 unten bis 20 Osten; Blöcke, die nirgends voll decken, fehlen | `Seiten.java` | 518 Blöcke aus 26.3, 3920 Zustände mit mindestens einer vollen Seite, 2862 mit allen sechs |
| [`dimensionstypen.txt`](../../renderer/src/assets/dimensionstypen.txt) | die Vorgaben der vier Attribute der Lightmap und der drei Farben des Himmels aus `EnvironmentAttributes`; je Dimensionstyp des Spiels `has_skylight`, `cardinal_light` und die Attribute, die er setzt. Das Format steht im Kopf von `Dimensionstypen.java` | `Dimensionstypen.java` mit dem Client-JAR, über `VanillaRegistries.createWorldLookup` | 4 Typen aus 26.3 |
| [`blockentities.txt`](../../renderer/src/assets/blockentities.txt) | je Zustand eines Blocks mit Blockentity-Renderer, was das Spiel aus Modellen zeichnet: Flächen, Lage, Textur, Schicht, Farbe; dazu die Farbstoffe, die Scherben, die Regel und die Muster des Spiels für Banner. Das Format steht im Kopf von `Blockentities.java` | `Blockentities.java` mit dem Client-JAR | 26.3, Zahlen in [Blockentities](../renderer/blockentities.md), „Die Tabelle“ |

`blocks.txt` prüft Variantenschlüssel und Multipart-Bedingungen, siehe
[Blockstates](../renderer/blockstates.md). `leuchten.txt` gibt das
Blocklicht, siehe [Wasser und Licht](../renderer/wasser-und-licht.md),
„Blocklicht“. `licht.txt` sagt, wie die Blöcke das Licht beim Ausbreiten
aufhalten, siehe [Wasser und Licht](../renderer/wasser-und-licht.md).
`schatten.txt` sagt, welche Blöcke weich abdunkeln, welche
die Sicht nehmen, vor welchen also auch eine Fläche im Innern in der Mitte
das Licht ihrer eigenen Zelle nimmt, und bei welchen jede ebene Fläche im
Licht der Zelle davor liegt, siehe
[Weiche Beleuchtung](../renderer/weiche-beleuchtung.md).
Andere Werte als 0,2 und 1 gibt `getShadeBrightness` in 26.3 nicht zurück.
`nachbarn.txt` sagt, welche Flächen zu gleichen Nachbarn entfallen, siehe
[Sprites und Deckung](../renderer/sprites-und-deckung.md), „Flächen zu
gleichen Nachbarn“. `seiten.txt` sagt, vor welchen Seiten die Flächen jedes
Nachbarn entfallen, siehe [Sprites und
Deckung](../renderer/sprites-und-deckung.md), „Flächen vor einem vollen
Nachbarn“.
`blockentities.txt` gibt Truhen, Bannern und den übrigen Blockentities ihr
Bild, siehe [Blockentities](../renderer/blockentities.md).
`dimensionstypen.txt` gibt jeder Dimension ihren Typ, siehe
[Dimensionstypen](../renderer/dimensionstypen.md).

## Tests, die sie festhalten

- `blocktabelle_aus_26_3` hält die Zahlen von `blocks.txt` fest und bekommt
  mit einer neuen Version deren Zahlen.
- `leuchten_wie_im_spiel`, `licht_wie_im_spiel` und `schatten_wie_im_spiel`
  prüfen Stufen, Flächen und Bits einzelner Blöcke aus den Tabellen und
  die Zahl der Paare.
- `nachbarn_wie_im_spiel` hält die Zahl der Blöcke in `nachbarn.txt` fest
  und prüft Paare, für die `skipRendering` in 26.2 so antwortet.
- `seiten_wie_im_spiel` hält die Zahl der Blöcke in `seiten.txt` fest und
  prüft die Bits einzelner Zustände, darunter die, für die
  `Block.shouldRenderFace` in 26.2 eine Fläche weglässt oder zeichnet.
- `tabelle_wie_im_spiel` prüft, dass jeder Verweis in `blockentities.txt`
  auf etwas zeigt, das es gibt, und jeder Block so viele Bilder hat wie
  Zustände oder eines; dazu die Zahl der Blöcke und Farbstoffe und die Regel
  der Muster. `bild_je_zustand` und `zuordnung_je_zustand` prüfen einzelne
  Zustände.
- `tabelle_wie_im_spiel` in `dimension.rs` hält jeden der vier
  Dimensionstypen und die Vorgaben fest.

## Eine neue Version

Für 26.3 ergibt der Skill genau die Dateien im Repository. Für eine andere
Version gilt ein Beleg erst, wenn ihn jemand dort geprüft hat, siehe Skill
[`spielverhalten-belegen`](../../skills/spielverhalten-belegen/SKILL.md).
Warum nur 26.x: [0015](../entscheidungen/0015-nur-welten-ab-26-1.md).
