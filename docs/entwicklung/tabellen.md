---
title: Erzeugte Tabellen
description: Die drei Tabellen aus dem Spiel, blocks.txt, leuchten.txt und schatten.txt - was darin steht, gegen welche Version, welche Tests sie festhalten und wie man sie neu erzeugt.
code:
  - renderer/src/assets/blocks.txt
  - renderer/src/assets/leuchten.txt
  - renderer/src/assets/schatten.txt
  - renderer/src/assets/Leuchten.java
  - renderer/src/assets/Schatten.java
  - renderer/src/assets/blockstate.rs
---

# Erzeugte Tabellen

Was Minecraft im Code verdrahtet und der Renderer braucht, steht in drei
Tabellen unter `renderer/src/assets/`, erzeugt aus dem Server-JAR von 26.2
und ins Binär einkompiliert (`blockstate.rs`). Von Hand werden sie nie
geändert; für eine andere Version erzeugt sie der Skill
[`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md) neu,
danach muss der Renderer neu gebaut werden.

## Die Tabellen

Haben alle Zustände eines Blocks dasselbe Zeichen, steht es einmal.

| Datei | Inhalt | Quelle | Stand |
|---|---|---|---|
| [`blocks.txt`](../../renderer/src/assets/blocks.txt) | jeder Block mit seinen Eigenschaften und Werten | `generated/reports/blocks.json` des Datengenerators | 1196 Blöcke aus 26.2 |
| [`leuchten.txt`](../../renderer/src/assets/leuchten.txt) | je Zustand in der Reihenfolge von `getPossibleStates` eine Ziffer 0 bis f für `getLightEmission` oder ein `x` für `emissiveRendering`; Blöcke, die nie leuchten, fehlen | `Leuchten.java` | 109 Blöcke aus 26.2 |
| [`schatten.txt`](../../renderer/src/assets/schatten.txt) | je Zustand in der Reihenfolge von `getPossibleStates` eine Ziffer, Bit 1 für `getShadeBrightness` 0,2, Bit 2 für `isViewBlocking` mit `getLightDampening` > 0; Blöcke ohne Bit fehlen | `Schatten.java` | 491 Blöcke aus 26.2 |

`blocks.txt` prüft Variantenschlüssel und Multipart-Bedingungen, siehe
[Blockstates](../renderer/blockstates.md). `leuchten.txt` gibt das
Blocklicht, siehe [Wasser und Licht](../renderer/wasser-und-licht.md),
„Blocklicht“. `schatten.txt` sagt, welche Blöcke weich abdunkeln und welche
die Sicht nehmen, siehe [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md).
Andere Werte als 0,2 und 1 gibt `getShadeBrightness` in 26.2 nicht zurück.

## Tests, die sie festhalten

- `blocktabelle_aus_26_2` hält die Zahlen von `blocks.txt` fest und bekommt
  mit einer neuen Version deren Zahlen.
- `leuchten_wie_im_spiel` und `schatten_wie_im_spiel` prüfen Stufen und Bits
  einzelner Blöcke aus den Tabellen.

## Eine neue Version

Für 26.2 ergibt der Skill genau die Dateien im Repository. Für eine andere
Version gilt ein Beleg erst, wenn ihn jemand dort geprüft hat, siehe Skill
[`spielverhalten-belegen`](../../skills/spielverhalten-belegen/SKILL.md).
Warum nur 26.x: [0015](../entscheidungen/0015-nur-welten-ab-26-1.md).
