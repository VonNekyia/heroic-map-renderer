---
title: "0081: Eigene Laubfarben als Tönung aus dem PDC"
description: Warum eigene Laubfarben im PersistentDataContainer des Chunks liegen und als Tönung statt der Biomfarbe wirken, warum Fichte und Birke an solchen Stellen eine eigene Familie mit Tönungskarte bekommen und warum „hell“ vorerst nichts ändert.
status: gilt
date: 2026-10-06
issues: [156]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
---

# 0081: Eigene Laubfarben als Tönung aus dem PDC

## Anlass

Bäume eines Plugins tragen eigene Farben (#156). Der Maintainer entschied
am 05.10.: Die Farben kommen über einen Datenvertrag in den Chunk-Daten,
nicht über eine API im Code. Der Vertrag steht in
[Eigene Laubfarben](../benutzung/laubfarben.md).

## Entscheidung

- **Lesen:** `Chunk::decode` liest `ChunkBukkitValues.heroicmap:leaf_colors`
  nach Fassung 1. Was dem Vertrag nicht folgt, gilt für den ganzen Chunk
  nicht; der Vorlauf nennt es, der Lauf bricht nicht ab.
- **Zeichnen:** Die Farbe ist eine Tönung und ersetzt in `tints_at` die
  Biomfarbe auf tönbarem Laub. Sie läuft über die Tönungskarte wie die
  Biomfarbe, auf der CPU wie auf der Karte. Laub mit Biomfarbe hat seine
  Tönungskarte schon und braucht keine neuen Sprites.
- **Fichte und Birke** tragen ihre feste Farbe im Bild. Je Familie, auf der
  der Vorlauf eine eigene Farbe findet, legt `SpriteSet::add_laub` eine
  zweite mit Tönungskarte an, wie die Varianten der Blockentities. Der
  Renderpfad nimmt sie nur an Stellen mit eigener Farbe; jede andere Stelle
  zeichnet Byte für Byte wie ohne eigene Farben.
- **„Hell“** ändert vorerst nichts. Über die Tönungskarte geht es nicht:
  Die Karte trägt je Pixel nur den Anteil der Farbe, keine andere Textur.
  Der Weg wäre derselbe wie für Fichte und Birke, eine eigene Familie mit
  hellerer Textur. Am 06.10. auf mehr als einen halben Tag geschätzt;
  darum vorerst ohne Wirkung, mit eigenem Issue.
- **Update:** Die Farben gehen in den Abdruck des Chunks ein, nur wenn es
  welche gibt; ein Chunk ohne behält seinen Abdruck.

## Verworfene Alternativen

- **Je Farbe eine eigene Familie mit gebackener Farbe,** wie die Muster der
  Banner: Jede verschiedene Farbe hiesse ein neues Rastern je Stufe; mit
  einer Farbe je Blatt wären das Tausende.
- **Fichte und Birke immer mit Tönungskarte:** Über die Karte gemischt
  weicht ihre Farbe um bis zu 1 je Kanal vom Bild ab. Jede Karte mit Fichte
  oder Birke änderte sich dann, auch ohne eigene Farben (Regel 22).
- **Fichte und Birke neu rastern, sobald der Vorlauf irgendwo eine eigene
  Farbe auf ihnen findet:** Dann wiche alles Fichten- und Birkenlaub ab,
  und ob, hinge daran, welche Chunks der Lauf liest. Ein Update, dessen
  Gebiet die Vorgabe nicht enthält, zeichnete anders als ein voller Lauf.
- **„Hell“ schätzen,** etwa die Tönung zu Weiss hin aufhellen: Ohne die
  Textur wäre jeder Wert geraten.

## Folgen

- Je Familie von Fichten- oder Birkenlaub mit eigener Farbe rastert jede
  Stufe eine Familie mehr.
- Laub mit „hell“ zeichnet dunkler als im Spiel.
- Ein Plugin, das die Bytes bei jedem Laden neu schreibt, macht jeden Chunk
  ungespeichert; das Update liest ihn dann umsonst. Das steht im Vertrag.
