---
title: "0037: Chunks ab dem Status light"
description: Warum der Renderer Chunks erst ab dem Status minecraft:light liest und die übrigen wie fehlende behandelt, statt jeden Chunk zu lesen oder erst ab full wie der Client.
status: gilt
date: 2026-09-28
issues: [32]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/world/region.rs
---

# 0037: Chunks ab dem Status light

## Anlass

Am Rand jedes erzeugten Gebiets zeigte die Karte, was das Spiel nie zeigt:
einen kahlen Streifen ohne Bäume, Seen und Schnee, am Ost- und Südrand
einen Schnitt bis zum Grundgestein und unter tiefem Wasser helle Linien im
Licht 14. Der Renderer las jeden Chunk, der in der Regionsdatei stand, auch
die, die das Spiel angefangen, aber nicht fertig erzeugt hatte (#32).

## Entscheidung

Der Renderer liest nur Chunks ab dem Status `minecraft:light`, also `light`,
`spawn` und `full`. Die übrigen gelten als fehlend, für Vorlauf, Render und
Höhen. Was dazu zählt, sagt eine Stelle, `Chunk::is_generated`.
`Region::chunk` lässt die übrigen weg; darüber lesen der Render und die
Biome. Nur wer sie zählt oder nennt, liest sie mit `stored_chunk`: der
Vorlauf, `--at` und `--scan`.
Vom User am 28.09. entschieden.

Ab `light` setzt die Erzeugung keinen Block mehr, belegt per javap am
Client 26.2 (`ChunkPyramid.GENERATION_PYRAMID`):

- Der Schritt `light` verlangt die Nachbarn im Radius 1 mindestens in
  `initialize_light`, also hinter `features`.
- Nur `features` schreibt über den eigenen Chunk hinaus, einen Chunk weit
  (`blockStateWriteRadius(1)`). Die Schritte danach setzen keinen Radius,
  der Standard −1 erlaubt gar keinen Block.

## Verworfene Alternativen

- **Nur `minecraft:full`, genau wie der Client.** `ChunkHolder.getChunkToSend`
  gibt ihm nur fertige Chunks heraus. Chunks mit `spawn`, deren Blöcke
  längst feststehen, fehlten dann als Löcher. An den Rändern beider Welten,
  an denen die Fehler auffielen, ergibt `light` dieselbe Kachelmenge wie
  `full`.
- **Jeden Chunk lesen, wie bisher:** die Fehler oben.
- **Die Prüfung bei jedem Aufrufer.** Vorlauf, Render, Höhen und `--scan`
  lesen Chunks an eigenen Stellen; eine, die die Prüfung vergisst, sähe
  eine andere Welt als die übrigen. Deshalb lässt `Region::chunk` sie
  selbst weg, und ungefiltert liest nur, wer sie zählen muss.

## Folgen

- Die Karte endet am letzten Chunk ab `light`. Der Vorlauf nennt die Zahl
  der übergangenen Chunks. Auf der Testwelt fallen 4,2 % der Basiskacheln
  weg.
- Der Schnitt am Ost- und Südrand bleibt, 16 bis 32 Blöcke weiter innen.
  Ihn abzudunkeln gehört zur Ausbreitung des Lichts (#34). Vom User am
  28.09. entschieden.
- Am neuen Rand mischt die Biomfarbe mit plains, wie neben jedem fehlenden
  Chunk.
- Ein Baum aus einem früheren Stand zeigt die Fehler am Rand, bis ein
  Export ohne `--resume` den Rand neu rendert, siehe
  [Welten und Kennung](../benutzung/welten.md), „Nicht fertig erzeugte
  Chunks“.
