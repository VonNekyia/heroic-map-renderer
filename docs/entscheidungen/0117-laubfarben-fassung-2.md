---
title: "0117: Laubfarben in Fassung 2"
description: Warum der Renderer den Vertrag heroicmap:leaf_colors auch in Fassung 2 liest, mit Zweit- und Blütenfarbe je Gruppe, diese prüft, aber noch nicht zeichnet, und warum sie darum weder in den Abdruck noch in einen neuen Zeichenstand gehen.
status: gilt
date: 2026-10-23
issues: []
code:
  - renderer/src/world/chunk.rs
---

# 0117: Laubfarben in Fassung 2

Ergänzt [0081](0081-eigene-laubfarben.md) im Format des Vertrags; alles
andere dort gilt weiter.

## Anlass

Das Plugin, das eigene Laubfarben schreibt, will je Gruppe zwei Farben
mehr ablegen: eine Zweitfarbe und eine Blütenfarbe, für Glanzlichter,
zweifarbiges Laub und Blüten. Der Reviewer stimmte am 23.10. zu. Das Plugin
schreibt Fassung 2 erst, wenn der Renderer sie liest und auf der Produktion
läuft.

## Entscheidung

- **Format:** Fassung 2 ist Fassung 1 mit zwei i32 mehr je Gruppe, zwischen
  Farbe und Zahl der Stellen. Beide tragen Bit 0 bis 23 RGB und Bit 24
  „vorhanden“, die übrigen Bits 0. Der Vertrag steht in
  [Eigene Laubfarben](../benutzung/laubfarben.md), „Format“.
- **Lesen:** `Chunk::decode` liest Fassung 1 und 2. Die neuen Felder prüft
  es wie die Farbe; folgt eins nicht dem Vertrag, gilt der ganze Chunk ohne
  eigene Farben, wie bisher. Eine andere Fassung gilt nicht.
- **Zeichnen:** wie Fassung 1, nur mit der Farbe. Ein Chunk mit Fassung 2
  zeichnet Byte für Byte wie mit Fassung 1 und denselben Farben.
- **Nicht im Abdruck:** Was nicht zeichnet, gehört nicht in den Abdruck.
  So behält ein Chunk seinen Abdruck, wenn das Plugin von Fassung 1 auf 2
  wechselt, und das erste Update danach zeichnet nicht die ganze Welt neu.
- **Kein neuer Zeichenstand:** Das Bild ändert sich nicht.

## Verworfene Alternativen

- **Die neuen Felder gleich in den Abdruck:** Der Wechsel des Plugins auf
  Fassung 2 machte jeden Chunk mit eigenen Farben zu einem geänderten,
  ohne dass sich ein Pixel ändert.
- **Fassung 2 weiter verwerfen, bis die Regel für Zweit- und Blütenfarbe
  feststeht:** Dann könnte das Plugin erst wechseln, wenn auch das Zeichnen
  fertig ist. Zwischen dem Wechsel und einem neuen Renderer fehlten auf der
  Karte alle eigenen Farben.

## Folgen

- **Zeichnet der Renderer die neuen Felder einmal,** gehören sie in den
  Abdruck, und der Zeichenstand des Looks steigt. Das braucht einen vollen
  Lauf, wie jede Änderung am Bild.
- **Die Regel dafür,** Masken und Hash für Glanzlichter, Zweitfarbe und
  Blüten, liefert der Schreiber, sobald sie feststeht. Ob der Renderer sie
  nachbaut, entscheidet der User.
- **Ein Renderer vor 0.15.3** verwirft Fassung 2 als `Fassung 2`, und der
  Chunk zeichnet ohne eigene Farben. Darum wechselt das Plugin erst danach.
