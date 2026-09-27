---
title: "0010: Tiefe entlang des Blickstrahls"
description: Wie die Oberfläche die Deckkraft aller Wasserschichten hinter ihr trug, gezählt entlang des Blickstrahls; abgelöst durch Licht je Block.
status: abgelöst durch 0030
date: 2026-09-25
issues: [8]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# 0010: Tiefe entlang des Blickstrahls

Abgelöst durch [0030: Licht je Block beim Zeichnen](0030-licht-je-block.md).

## Anlass

Nur innere Flächen wegzulassen ([0009](0009-wasserflaechen-je-block.md))
hätte einen Ozean in einen Meeresboden hinter Milchglas verwandelt: Eine
Schicht Wasser lässt 29 Prozent durch, und die sah man dann in jeder Tiefe.
Im Spiel erledigt das der Unterwassernebel.

## Entscheidung

Je Oberflächenblock zählt der Renderer die Wasserblöcke hinter ihr auf der
Diagonale (x−1, y−1, z−1), der Richtung des Blickstrahls, höchstens vier,
und nimmt die Fassung, deren Oberfläche das Alpha von so vielen Schichten
trägt: `255 − 255 · (1 − 180/255)^d`, aus der Textur gerechnet. Die Zählung
endet an einem Block, der mehr als die Hälfte der Oberseite deckt, durch
die die Strahlen eintreten. Die hochgerechnete Deckkraft gilt nur für
Pixel, hinter denen das Sprite selbst nichts hat.

## Verworfene Alternativen

- **Senkrecht in der eigenen Spalte zählen**, die erste Fassung: Kelp,
  Riffe und Wracks knapp unter einer tiefen Oberfläche verschwanden unter
  Alpha 253, obwohl der Strahl nur einen Block Wasser kreuzt.
- **Der ganze Umriss eines Blocks hält den Strahl auf:** Eine untere Platte
  beendete die Zählung, obwohl drei von fünf Strahlen über sie hinweggehen.

## Folgen

- Unter einer Quelle verschwand ein dünnes Modell einen Block unter der
  Oberfläche vor tiefem Wasser fast; von ihm blieb unter 1 statt 29 Prozent
  sichtbar.
- Was einen Block tief lag, stand hell und blass neben fast deckendem
  Wasser, Pfosten und Wracks in eckigen Flecken. Das war der Anlass für #14
  und [0030](0030-licht-je-block.md).
- Die Tiefenfassungen je Oberfläche vergrösserten die Sprite-Tabelle.
