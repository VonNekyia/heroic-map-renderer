---
title: "0031: Eigene Tabellen statt der Masken für die weiche Beleuchtung"
description: Warum die weiche Beleuchtung abdunkelnde und sichtnehmende Blöcke aus schatten.txt nimmt und als eigene Bitebenen führt, statt „deckend“ aus den Masken.
status: gilt
date: 2026-09-27
issues: [15, 18]
code:
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/schatten.txt
  - renderer/src/assets/Schatten.java
  - renderer/src/render/metatile.rs
---

# 0031: Eigene Tabellen statt der Masken für die weiche Beleuchtung

## Anlass

#15 wollte für die weiche Beleuchtung die vollen Blöcke aus den Masken in
`metatile.rs` nehmen, also „deckend“.

## Entscheidung

Das Spiel fragt nicht, ob ein Block deckt, sondern `getShadeBrightness` und
`isViewBlocking` samt `getLightDampening`. Beides steht je Zustand in
`renderer/src/assets/schatten.txt`, aus dem Spiel gelesen von
`Schatten.java`, und liegt als die Ebenen `DARK` und `VIEW` neben `SOLID` in
den Bitmasken der Sections, auch für Blöcke ohne Familie. `umgebung` holt
die 31 Blöcke, nach denen die drei Seiten fragen, aus 15 Spalten, je Spalte
aus einem Wort. Siehe [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md).

## Verworfene Alternativen

- **„Deckend“ aus den Masken.** Das deckt sich nicht mit dem Spiel: Laub
  dunkelt ab und deckt nicht, Glas deckt und dunkelt nicht ab, Seelensand
  und Schlamm dunkeln trotz kleinerer Form ab.
- **Je Block nachschlagen**, die erste Runde von #18: `ao_at` fragte je
  Block bis zu 39-mal `schatten_at` und dreimal `family_at`, für 31
  verschiedene Blöcke. Mit den Ebenen kosten die Ecken 0,06 statt 0,19 ms je
  Kachel, siehe
  [2026-09-27, Weiche Beleuchtung](../messungen/2026-09-27-weiche-beleuchtung.md).

## Folgen

- Eine weitere Tabelle aus dem Spiel, neu zu erzeugen mit jeder Version,
  siehe [Erzeugte Tabellen](../entwicklung/tabellen.md).
- Eine Section ohne Familie behält ihre Masken, wenn darin etwas abdunkelt
  oder die Sicht nimmt.
