---
title: "0015: Nur Welten ab 26.1"
description: Warum der Renderer nur Welten ab Minecraft 26.1 liest und ältere Welten vorher mit dem Server von 26.2 hochgezogen werden.
status: gilt
date: 2026-09-25
issues: [9]
code:
  - renderer/src/world/mod.rs
  - renderer/src/assets/blocks.txt
---

# 0015: Nur Welten ab 26.1

## Anlass

Der erste Vollrender einer echten Welt stiess auf Blocknamen älterer
Versionen. Die erste Fassung von #9 löste sie über eine Tabelle auf und
liess unbekannte Blöcke leer; #8 hatte Code für `DIM-1`, `DIM1` und den Seed
aus `level.dat`.

## Entscheidung

Das Projekt unterstützt Welten ab 26.1, Kompatibilität gilt nur für 26.x.
Eine ältere Welt wird vorher mit dem Server von Minecraft 26.2 und
`--forceUpgrade` hochgezogen. Ein Block ohne Asset bricht den Lauf vor der
ersten Kachel ab. Siehe [Welten und Kennung](../benutzung/welten.md).

## Verworfene Alternativen

- **Alte Blocknamen über eine Tabelle auflösen und unbekannte Blöcke leer
  lassen**, die erste Fassung von #9: zurückgenommen. Der Server von 26.2
  baut Verzeichnisse, Seed und Chunks selbst um, und ein leerer Block
  versteckte ein fehlendes Asset.
- **`DIM-1`, `DIM1` und den Seed aus `level.dat` weiter lesen:** Code, den
  nur ältere Welten brauchten; entfernt.

## Folgen

- Die Tabellen aus dem Spiel (`blocks.txt`, `leuchten.txt`, `schatten.txt`)
  und alle Belege gelten für 26.2. Eine neue Version braucht neue Tabellen
  und neue Belege, siehe [Erzeugte Tabellen](../entwicklung/tabellen.md).
- Ohne Seed oder Weltwurzel nennt die Meldung den Weg über
  `--forceUpgrade`.
