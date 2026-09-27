---
title: "0016: Native Stufen nur auf ganzen Pixeln und nur auf Wunsch"
description: Warum gröbere Zoomstufen nur mit --native-levels aus der Welt gerendert werden, nur solange jeder Block auf ganzen Pixeln liegt, und die Zahl zum Baum gehört.
status: gilt
date: 2026-09-25
issues: [8, 9]
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/pyramid.rs
---

# 0016: Native Stufen nur auf ganzen Pixeln und nur auf Wunsch

## Anlass

Das Verkleinern mittelt Nachbarblöcke ineinander; die Karte verschwamm beim
Herauszoomen früh. #8 renderte deshalb bei scale 32 die Stufen 16, 8 und 4
immer aus der Welt. Der erste Vollrender der grossen Welt zeigte den Preis:
Die drei kosten zusammen fast noch einmal die Basis, bei knapp 16 Stunden
Basisstufe rund 14 Stunden mehr.

## Entscheidung

`--native-levels N` rendert die ersten N gröberen Stufen aus der Welt, mit
Sprites in dieser Grösse, solange der scale der Stufe durch vier teilbar
ist; die Vorgabe ist 0 (#9). Die Zahl gehört zum Baum: `map.json` hält sie
als `nativeLevels` fest. Ein Ausschnitt wird auf ganze Kacheln der gröbsten
nativen Stufe aufgerundet. Siehe [Zoomstufen](../benutzung/zoomstufen.md).

## Verworfene Alternativen

- **Immer alle nativen Stufen**, wie in #8: siehe Anlass.
- **Native Stufen bis scale 2**, die vierte Stufe der ersten Fassung in #8:
  Jede zweite Blockreihe liegt dort auf einem halben Pixel, das Runden kippte
  an Bildzeile 0, und benachbarte Reihen überdeckten sich ganz;
  durchscheinendes Wasser mischte dort doppelt.
- **Einen Ausschnitt nicht runden.** Die erste Fassung machte ausserhalb des
  Ausschnitts alles zu Luft, die zweite zeigte einen Neubau neben dem
  Ausschnitt nur auf den gröberen Stufen.

## Folgen

- Wer schärfere Kanten will, zahlt je Stufe einen Weltdurchlauf; zusammen
  brauchen die drei Stufen bei scale 32 etwa so lange wie die Basis.
- Ein Baum aus #8 nennt `nativeLevels` nicht; ein Lauf ohne den Schalter
  bricht dort ab und fragt nach der Zahl.
- Mit nativen Stufen braucht ein Ausschnitt mehr Welt als sich selbst.
