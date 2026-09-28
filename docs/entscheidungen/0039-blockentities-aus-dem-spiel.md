---
title: "0039: Blockentities aus den Renderern des Spiels"
description: Warum der Renderer Truhen, Banner, Köpfe, Krüge und die übrigen Blöcke mit Blockentity-Renderer aus einer Tabelle zeichnet, die die Renderer des Spiels selbst schreiben, und Muster und Scherben erst beim Rendern dazunimmt.
status: gilt
date: 2026-09-28
issues: [30]
code:
  - renderer/src/assets/Blockentities.java
  - renderer/src/assets/blockentities.txt
  - renderer/src/assets/blockentity.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/world/chunk.rs
---

# 0039: Blockentities aus den Renderern des Spiels

Ergänzt [0038: Flächen mit Löchern werden ausgeschnitten](0038-cutout-wie-im-spiel.md):
Der Alpha-Test läuft je Abtastpunkt, vor dem Mitteln; für Blockflächen aus
Vanilla ohne Wirkung, siehe [Rastern ohne Nähte](../renderer/naehte.md),
„Ausgeschnitten statt gemischt“.

## Anlass

84 Blöcke haben in 26.2 kein Blockmodell mit Elementen, das Spiel zeichnet
sie mit ihrem Blockentity-Renderer: Truhen, Shulkerkisten, Banner, Köpfe,
Verzierte Krüge, der Aquisator, Kupfergolemstatuen. Auf der Karte blieben
sie leer (#30). Der User hat am 28.09. entschieden, dass ihre Modelle aus
dem Client-JAR kommen und das Projekt sie selbst erzeugt.

## Entscheidung

`Blockentities.java` lässt jeden Blockentity-Renderer des Clients 26.2
jeden Zustand zeichnen, mit einem Collector, der jedes `submitModel`
mitschreibt, und zeichnet wie `ModelFeatureRenderer.prepareModel`. Die
Tabelle `blockentities.txt` hält je Zustand die Flächen im Raum ihres
Modells fest, dazu die Matrix, die Textur, die Schicht mit dem, was ihre
Pipeline festlegt, und die Farbe. Der Renderer backt daraus Flächen mit
eigener Schicht: Alpha-Test, Rückseiten und Licht wie für Entity-Modelle.

Bannermuster und Scherben kommen beim Rendern aus `block_entities`, nach
Regeln, die der Generator am Spiel prüft und in die Tabelle schreibt: Welche
Zeichnung die Grundlage der Muster ist und wie viele Lagen das Spiel
zeichnet, welche Zeichnung welchen Platz in `sherds` trägt, und die Muster
des Spiels. Je Familie und Daten entsteht beim Aufbau der Sprite-Tabelle
eine eigene Familie. Einzelheiten in
[Blockentities](../renderer/blockentities.md).

## Verworfene Alternativen

- **Modelle von Hand als Blockmodell-JSON:** nachgeschrieben statt aus
  dem Spiel; jede Lage, jede Pose und jede Textur wäre eine eigene Stelle,
  an der es falsch werden kann.
- **Modelle aus anderen Werkzeugen übernehmen:** Sie stammen nicht aus dem
  Spiel; vom User am 28.09. verworfen.
- **Nur die Modellteile mit ihren Lagen backen**, `LayerDefinition` und
  `modelTransformation` je Renderer. Welche Teile ein Renderer zeichnet, mit
  welcher Textur, Farbe und Pose, entscheidet sein `submit`: das Buch nur
  auf dem Lesepult mit Buch, die Truhe nach Art und Material, die
  Shulkerkiste mit der Textur ihrer Farbe. Das nachzubauen hiesse, jeden
  Renderer nachzuschreiben.
  Mitschreiben, was er abgibt, nimmt seine Logik, wie sie ist.
- **Die Tabelle als Blockmodell-JSON** für den vorhandenen Baker: Die
  Schicht ginge verloren. Blockmodelle kennen nur die Schichten der Blöcke,
  mit dem Alpha-Test 0,5 statt 0,1, immer mit Culling und im Licht der
  Blockseiten.
- **Bilder mit Mustern und Scherben schon in der Tabelle:** jeder
  Farbstoff mit jedem Muster in bis zu 16 Lagen, dazu die Scherben auf vier
  Plätzen; das sind mehr Bilder, als je in einer Welt vorkommen. Beim
  Rendern entstehen nur die, die es gibt.
- **Licht wie für Blockseiten** (`CardinalLighting`): Das Spiel zeichnet
  Entity-Modelle anders, die Seiten nach Osten und Westen etwa mit 0,50
  statt 0,6.

## Folgen

- Die Tabelle ist einkompiliert; für eine neue Version schreibt der
  Generator sie neu, mit dem Client-JAR und Java 25, Skill
  [`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md).
- Alles steht still, zur Zeit 0; was nicht aus einem Modell kommt, fehlt,
  siehe [Blockentities](../renderer/blockentities.md), „Was fehlt“.
- Jede Kombination aus Familie und Daten in einer Welt ist eine Familie mit
  eigenen Sprites. Was das Lesen der Daten kostet, steht in
  [2026-09-28, Blockentities](../messungen/2026-09-28-blockentities.md).
