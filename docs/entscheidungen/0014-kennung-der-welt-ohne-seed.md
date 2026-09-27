---
title: "0014: Die Kennung der Welt verrät den Seed nicht"
description: Warum map.json statt des Seeds ein Salz und einen 2^20-mal verketteten SipHash von Seed und Dimension trägt.
status: gilt
date: 2026-09-25
issues: [8]
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
  - renderer/src/world/mod.rs
---

# 0014: Die Kennung der Welt verrät den Seed nicht

## Anlass

Ein Kachelbaum muss erkennen, ob ein Lauf zu seiner Welt und Dimension
gehört; sonst lägen Kacheln zweier Welten in einem Baum. `map.json` liegt
aber öffentlich neben den Kacheln.

## Entscheidung

`world` in `map.json` ist ein Salz, das der Baum bei seinem ersten Lauf
zufällig bekommt, und SipHash-2-4 von Seed und Dimension mit diesem Salz,
2^20-mal verkettet (`world_id`). Ohne Weltwurzel oder Seed steht
`"world": null` da, und ein solcher Baum nimmt keine Welt mit Kennung auf.
Siehe [Welten und Kennung](../benutzung/welten.md).

## Verworfene Alternativen

- **Den Seed selbst.** Mit ihm fände jeder Besucher Strukturen und Erze
  ohne zu suchen.
- **Einen einzelnen Hash.** Vanilla zieht Zufallsseeds mit
  `LegacyRandomSource`, 48 Bit Zustand, also aus nur 2^48 Werten; alle
  durchzuprobieren dauerte Stunden bis Tage.
- **`DefaultHasher`.** Sein Algorithmus darf sich mit jeder Rust-Version
  ändern, und jeder bestehende Baum gälte dann als fremd. SipHash steht
  deshalb von Hand im Code.
- **Nur den Seed ohne Dimension.** Die Dimensionen einer Welt tragen meist
  denselben Seed; der Nether käme in den Baum der Oberwelt.
- **Ein Hash ohne Salz.** Dann liessen sich die Kennungen aller Seeds einmal
  vorab rechnen, für jeden Baum zugleich; mit dem Salz kostet jeder Baum den
  Aufwand von vorn.

## Folgen

- Der Export zahlt 16 ms je Lauf; durchzuprobieren kostet 2^68 Aufrufe je
  Baum für einen Zufallsseed.
- Ein Seed aus einem Text hat nur 2^32 Werte, 2^52 Aufrufe: den schützt die
  Kennung für Stunden bis Tage. Ein eingetippter Seed wie 12345 oder einer
  aus einer öffentlichen Liste steht in jedem Wörterbuch und kostet einen
  Versuch von 16 ms.
- Zwei Welten ohne Kennung kann der Renderer nicht auseinanderhalten.
