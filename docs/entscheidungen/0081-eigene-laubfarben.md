---
title: "0081: Eigene Laubfarben als Tönung aus dem PDC"
description: Warum eigene Laubfarben im PersistentDataContainer des Chunks liegen und als Tönung statt der Biomfarbe wirken, warum Fichte und Birke nur bedingt eine Tönungskarte bekommen und warum „hell“ vorerst nichts ändert.
status: gilt
date: 2026-10-06
issues: [156]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
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
  Biomfarbe, auf der CPU wie auf der Karte, ohne neue Sprites.
- **Fichte und Birke** tragen ihre feste Farbe im Bild. Findet der Vorlauf
  eine eigene Farbe auf ihnen, rastert `SpriteSet::toenbares_festes_laub`
  ihre Familien mit Tönungskarte neu; ohne eigene Farbe gilt dort die feste.
  Sonst bleibt alles, wie es war.
- **„Hell“** ändert vorerst nichts: Die hellere Textur des Clients hat der
  Renderer nicht, und wie viel heller sie ist, ist nicht bekannt.
- **Update:** Die Farben gehen in den Abdruck des Chunks ein, nur wenn es
  welche gibt; ein Chunk ohne behält seinen Abdruck.

## Verworfene Alternativen

- **Je Farbe eine eigene Familie mit gebackener Farbe,** wie die Muster der
  Banner: Jede verschiedene Farbe hiesse ein neues Rastern je Stufe; mit
  einer Farbe je Blatt wären das Tausende.
- **Fichte und Birke immer mit Tönungskarte:** Über die Karte gemischt
  weicht ihre Farbe um bis zu 1 je Kanal vom Bild ab. Jede Karte mit Fichte
  oder Birke änderte sich dann, auch ohne eigene Farben (Regel 22).
- **„Hell“ schätzen,** etwa die Tönung zu Weiss hin aufhellen: Ohne die
  Textur wäre jeder Wert geraten.

## Folgen

- Liegt eine eigene Farbe auf Fichte oder Birke, weicht das übrige Fichten-
  und Birkenlaub dieser Karte um bis zu 1 je Kanal ab.
- Laub mit „hell“ zeichnet dunkler als im Spiel.
- Ein Plugin, das die Bytes bei jedem Laden neu schreibt, macht jeden Chunk
  ungespeichert; das Update liest ihn dann umsonst. Das steht im Vertrag.
