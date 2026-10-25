---
title: Pixel-Hashes als Feld
description: Was R2, die Pixel-Hashes als Feld je Block und früh geschriebene Blöcke, im ersten Render der Testwelt am Arbeitsspeicher ändert, an der Spitze und am Ende jeder Phase, mit allen Threads und mit einem Thread über den Ausschnitt aus der Messung zum Speicher im ersten Render.
date: 2026-10-25
commits: [c94548a, 08dc11e]
code:
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# Pixel-Hashes als Feld

An der Spitze ändert R2 auf der Testwelt nichts Messbares: 3,363 gegen
3,365 GiB mit allen Threads, 0,489 gegen 0,491 GiB mit einem Thread. Mit
einem Thread liegt der Speicher spät in der Basis 3 bis 11 MB tiefer. Die
Pixel-Hashes der Testwelt sind klein, rund 18 MB; ihr Gewinn zeigt sich erst
auf einer grossen Welt mit einem Thread.

## Aufbau

- **Welt und Baum** wie in
  [2026-10-24, Speicher im ersten Render](2026-10-24-speicher-im-ersten-render.md):
  eine Kopie der Testwelt, 26.2, 2:1 aus `se`, scale 32, ohne native Stufen,
  `--gpu auto --low-priority`, jeder Lauf in einen frischen Baum.
- **Läufe:**
  - der volle Lauf mit allen Threads, 274 273 Basiskacheln;
  - der Ausschnitt mit `--threads 1`, `--center 0 0 --size 49152`,
    34 445 Basiskacheln.
- **Stände**, Release-Builds:
  - A: master `c94548a`.
  - B: `08dc11e`, R2.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel, ohne
  Server des Spiegels. Vor jedem Lauf die Lastprüfung: Sie lag bei 1 bis 3 %
  und stand im Protokoll, bevor der Lauf begann.
- **Reihenfolge:** A voll, B voll, A Ausschnitt, B Ausschnitt; je ein Lauf.
- **Speicher:** alle 0,5 s Working Set und privater Speicher des laufenden
  Prozesses aus `GetProcessMemoryInfo`, die Spitze dazu am Ende.
- **Phasen:** Die Zeilen `Kacheln:`, `Pyramide:` und `Pixel:` der Ausgabe
  bekamen ihre Zeit. Am Ende einer Phase steht der letzte Wert davor.

## Ergebnis

Privater Speicher in GiB:

| | A, alle Threads | B, alle Threads | A, 1 Thread | B, 1 Thread |
|---|---|---|---|---|
| Spitze | 3,363 | 3,365 | 0,489 | 0,491 |
| Mitte der Basis | 3,130 | 3,164 | 0,475 | 0,467 |
| bei 90 % der Basis | 3,152 | 3,135 | 0,477 | 0,466 |
| am Ende der Basis | 3,261 | 3,178 | 0,482 | 0,479 |
| nach der Pyramide | 1,588 | 1,432 | 0,376 | 0,366 |
| nach den Hashes, `Pixel:` | 1,555 | 1,362 | 0,376 | 0,366 |

- **Zeit:** voll 239,0 gegen 250,4 s, Ausschnitt 274,9 gegen 270,0 s; je ein
  Lauf, darum nicht belastbar.
- **Blöcke:** In beiden Ständen schrieb der Lauf 591 Blöcke der Hashes,
  im Ausschnitt 88.

## Schluss

- **An der Spitze** wirkt R2 auf der Testwelt nicht, wie im Plan gerechnet:
  Die Hashes halten dort rund 18 MB, unter 1 % der Spitze.
- **Mit einem Thread** liegt B spät in der Basis 3 bis 11 MB tiefer, in der
  Richtung, die das frühe Schreiben erwarten lässt.
- **Mit allen Threads** liegen nach der Pyramide 0,16 bis 0,19 GiB weniger
  im Speicher. Das ist mehr, als die Hashes ausmachen, und fällt in eine
  Phase, in der grosse Puffer frei werden; in je einem Lauf ist es nicht
  R2 zuzuschreiben.
- **Auf der grossen Welt mit einem Thread** hielte A am Ende der Basis rund
  120 MB Hashes, gerechnet aus 3,3 Mio. Kacheln zu rund 48 Bytes; B nur die
  Blöcke einer Spalte. Gemessen ist das dort nicht.
