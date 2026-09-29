---
title: "0040: Licht selbst ausbreiten, je Chunk mit Rand"
description: Warum der Renderer Himmels- und Blocklicht wie das Spiel selbst ausbreitet, je Chunk in einem Fenster mit 14 Blöcken Rand, statt das gespeicherte Licht zu lesen, und damit zeichnet wie das Spiel.
status: gilt
date: 2026-09-29
issues: [34]
code:
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/render/gpu.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/licht.txt
  - renderer/src/assets/Licht.java
---

# 0040: Licht selbst ausbreiten, je Chunk mit Rand

## Anlass

#34: Höhleneingänge, Überhänge und der Boden unter dichtem Laub lagen so
hell wie freies Feld, und was leuchtet, leuchtete nur selbst: Der Schein
einer Kolonie Meeresgurken auf dem Sand fehlte. Der Renderer zählte das
Licht nur, wo Wasser steht, und gab Blocklicht nur dem Block, der
leuchtet.

## Entscheidung

Der Renderer breitet Himmels- und Blocklicht selbst aus, nach den Regeln
von 26.2 und den Werten aus `licht.txt` und `leuchten.txt`: skalar, mit
einem Eimer je Stufe. Er rechnet je Chunk in einem Fenster aus 44 × 44
Spalten, 14 Blöcke Rand an jeder Seite, so weit wie Licht reicht, wenn
ein Chunk zum ersten Mal Licht braucht, und behält es mit dem Chunk im
Cache des Threads. Ein Chunk, der fehlt oder nicht fertig ist, lässt kein
Licht herein. Siehe [Wasser und Licht](../renderer/wasser-und-licht.md),
„Licht ausbreiten“.

Gezeichnet wird mit diesem Licht wie im Spiel: Volle Würfel bekommen es an
den Ecken jeder Seite, gemischt wie `smoothBlend`, die Lightmap linear
gefiltert; Flüssigkeiten das hellere ihrer Zelle und der darüber, das
Wasser eines gefluteten Blocks ebenso; alles andere das seiner Zelle. Das
Sprite trägt dafür kein Licht mehr. Siehe
[Wasser und Licht](../renderer/wasser-und-licht.md), „Welches Licht ein
Block bekommt“, und [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md),
„Licht an den Ecken“. Das löst
[0030](0030-licht-je-block.md) ab.

## Verworfene Alternativen

- **Das gespeicherte Licht lesen:** In der grossen Welt fehlt es in rund
  drei Vierteln der Chunks, der DataFixer `ChunkDeleteLightFix` des Spiels
  hat es beim Upgrade auf 26.2 gelöscht, und unfertige Chunks am Rand
  tragen keines. Als Orakel für die Tests taugt ein Lauf von Vanilla.
- **Inkrementell über den Cache, ohne Rand:** Braucht am Streifenrand
  dieselben Nachbarn, dazu vorläufiges Licht und Nachrechnen, wenn ein
  Nachbar kommt; gerechnet mindestens so teuer wie das Fenster im
  Streifen. Nur nach einer Messung, so festgelegt.
- **Ein Fenster, das dem Streifen folgt:** gerechnet rund 11 % statt
  27 % mehr für die Basis, hängt aber an der Zeilenfolge des Streifens, am
  Stehlen mitten im Streifen, an `--render` in Stücken und an schmalen
  Streifen kleiner Ausschnitte. Es ersetzt nur den Bau des Fensters und
  kann später folgen, wenn die Minuten zählen.
- **Bitparallel über Spaltenmasken:** gemessen langsamer als skalar,
  0,47 statt 0,37 ms Himmel je Chunk mit Rand.
- **Blöcke aus mehreren Chunks:** Im Streifen kaum billiger als je Chunk,
  weil ein Block fast immer in zwei Streifen liegt.
- **Nur Schatten unter Überhängen:** nicht wie im Spiel, und kaum
  billiger.

Die Zahlen hat der Researcher im Prototyp gemessen und gerechnet, siehe
#34.

## Folgen

- Jede Zelle hat das Licht, das Vanilla 26.2 speichert; gegen einen Lauf
  des Spiels weicht keine ab.
- Das Licht eines Chunks braucht seine acht Nachbarn: Der Cache lädt am
  Rand eines Streifens mehr Chunks.
- Eine Tabelle mehr aus dem Spiel, `licht.txt`, und die Stufe voll heller
  Blöcke in `leuchten.txt`.
- Eine Instanz auf der Karte trägt je Farbkanal das Licht an den Ecken
  und das Licht des Wassers, 68 statt 40 Bytes.
