---
title: "0088: Helles Laub mit den Farben des Spiels"
description: Warum Laub mit Bit 24 einer eigenen Laubfarbe die Farben seiner Textur nach einer festen Tabelle tauscht, wie der Client es tut, statt der Näherung aus 0081; woher die Tabelle kommt, warum die helle Fassung eine eigene Familie ohne dark_cutout ist und welche Wege verworfen sind. Löst 0081 im Punkt „hell“ ab.
status: gilt
date: 2026-10-06
issues: [179, 156]
code:
  - renderer/src/assets/laubkopie.rs
  - renderer/src/assets/hell.txt
  - renderer/src/assets/laubtabellen.py
  - renderer/src/assets/texture.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/tiles.rs
---

# 0088: Helles Laub mit den Farben des Spiels

Löst [0081](0081-eigene-laubfarben.md) im Punkt „hell“ ab.

## Anlass

Bit 24 einer eigenen Laubfarbe heisst im Spiel: eine hellere Blatttextur.
Seit 0081 zeichnete der Renderer das Bit nicht, und entschieden war eine
Näherung über den Grauwert. Am 06.10. stand fest, dass ein Mod das Bit
wirklich setzt; der Reviewer entschied, wie der Client zu rechnen statt
genähert (#179).

## Entscheidung

- **Der Tausch des Clients:** Für helles Laub tauscht der Client in der
  Blatttextur Farben nach einer festen Tabelle, mit einer Atlasquelle
  `paletted_permutations`. Je Texel nach `PalettedPermutations` im Client,
  per javap am Client-JAR von 26.2 belegt (`createPaletteMapping`): ohne
  Deckung bleibt es, sonst sucht er die Farbe ohne Deckung in der Tabelle,
  eine fehlende bleibt, die Deckung ist die des Texels mal die der neuen
  Farbe durch 255. Die neuen Farben sind ganz deckend. Getönt wird danach
  wie jede Laubfarbe.
- **Die Tabelle** steht in `hell.txt`, je Blatttextur der sieben Sorten
  die Farben alt zu hell. `laubtabellen.py` erzeugt sie aus den Texturen des
  Client-JARs: je Kanal `60 + ch / top · 195`, `top` der höchste Kanal über
  alle deckenden Farben der Textur, halbe Werte zur geraden Zahl. Aus den
  Texturen von 26.2 ergibt das Farbe für Farbe die Tabelle, die das Spiel
  in 26.3 tauscht. Der Renderer nimmt die Tabelle, nicht die Formel: Eine
  Textur aus einem Resourcepack mit anderen Farben wird nicht hell, wie im
  Spiel.
- **Eine eigene Familie** je Laub mit Bit 24, mit der hellen Fassung der
  Textur, Fichte und Birke dazu mit Tönungskarte. Der Renderpfad nimmt sie
  nur an Stellen mit dem Bit; jede andere Stelle zeichnet Byte für Byte wie
  zuvor.
- **Ohne `dark_cutout`:** Die helle Fassung legt das Spiel ohne `.mcmeta`
  an, darum füllt sie ihre Löcher nicht dunkel.
- **Kommt keine Farbe der Tabelle in der Textur vor,** etwa aus einem
  Resourcepack, gibt es keine helle Familie, und das Bit zeichnet wie ohne.

Wie es wirkt, steht in [Eigene Laubfarben](../benutzung/laubfarben.md),
„Wirkung“.

## Verworfene Alternativen

- **Die Näherung aus 0081,** den Grauwert von 100…190 auf 160…255 abbilden:
  Sie trifft die Farben des Spiels nicht, und Karte und Spiel sähen
  verschieden aus.
- **Die Tabelle zur Laufzeit aus den Texturen der Assets rechnen:** Dann
  würde auch ein Resourcepack hell, das im Spiel nicht hell wird.
- **„Hell“ in `tints_at`:** Die Tönungskarte trägt schon Grauwert mal
  Seitenschatten; der Tausch wirkt auf den Texel davor, wie schon in 0081.

## Folgen

- Je Familie von Laub mit Bit 24 rastert jede Stufe eine Familie mehr.
- Neue Blattsorten, etwa Azalee oder Kirsche, brauchen eine eigene Zeile in
  `hell.txt` mit der Tabelle, die das Spiel für sie tauscht.
- Eine neue Version mit anderen Blatttexturen braucht eine neu erzeugte
  `hell.txt`, Skill
  [`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md).
