---
title: "0033: Tönung beim Zeichnen statt Fassungen je Biom"
description: Warum gefärbte Sprites eine Tönungskarte tragen und die Farbe des Bioms je Block erst beim Zeichnen dazukommt, statt je Biom eine Fassung zu rastern.
status: gilt
date: 2026-09-27
issues: [21]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/tint.rs
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
---

# 0033: Tönung beim Zeichnen statt Fassungen je Biom

Löst
[0011: Färbung als Sprite-Fassung](0011-faerbung-als-sprite-fassung.md)
ab.

## Anlass

#21: Übergänge zwischen Biomen wie im Spiel. Der Client mischt die Farbe
eines Blocks über die 25 Blöcke um ihn, und mit dem Zoom verlaufen die
Grenzen blockgenau. Längs jeder Grenze hat damit fast jeder gefärbte Block
eine eigene Farbe, und welche vorkommen, steht erst beim Zeichnen fest. Eine
vorab gerasterte Fassung je Farbe geht nicht mehr auf.

## Entscheidung

Ein Sprite mit gefärbten Flächen trägt den Teil ohne Farbe als Bild und je
Pixel eine Tönungskarte: den Anteil der Farbe des Blocks und den des
Wassers, je Kanal ein Byte. Die Karte entsteht aus drei Rastern desselben
Modells, in Schwarz und je einer Farbe in Weiss. Beim Zeichnen kommen die
gemischten Farben des Blocks dazu, ganzzahlig und gleich auf CPU und Karte,
vor Licht und weicher Beleuchtung. Feste Farben wie Fichte, Birke und Seerose
bleiben im Sprite. Siehe [Biomfarben](../renderer/biomfarben.md), „Tönung
beim Zeichnen“.

Welche Blöcke gefärbt werden und woher ihre Farbe kommt, steht weiter im
Code wie in `BlockColors` (`source_of` in `colors.rs`), nicht in den
Assets: Minecraft verdrahtet es dort, in den Assets steht es nicht. Das
übernimmt diese Entscheidung aus 0011.

## Verworfene Alternativen

- **Fassungen je Farbe weiterführen:** Die Mischfarben stehen erst beim
  Zeichnen fest, der Vorlauf kennt sie nicht, und ihre Zahl wächst mit jeder
  Grenze.
- **Ein Bit je Pixel „gefärbt“ und das Bild in Weiss:** falsch, wo sich
  gefärbte und ungefärbte Flächen in einem Pixel treffen. Unter der halb
  durchsichtigen Wasseroberfläche würde Seegras blau, am Rand der Auflage
  eines Grasblocks die Erde grün.
- **Ein Anteil je Pixel statt je Kanal:** derselbe Fehler, wo gefärbte und
  ungefärbte Texel verschiedene Farben haben, wie Erde und Auflage.
- **Die Karte mischt die Biome selbst:** Sie hat keine Chunks; das Biom
  eines Blocks und die 25 Nachbarn liest die CPU ohnehin beim Aufstellen der
  Zeichenliste.

## Folgen

- Je gefärbtem Block mischt die CPU bis zu zwei Farben über 25 Blöcke; das
  Biom eines Blocks rechnet sie einmal und behält es je Chunk und Höhe.
  Zusammen mit der Tönung kostet das auf einem Thread 7 % je Kachel, auf 24
  Threads 6 bis 7 % der Rate, siehe
  [2026-09-27, Biomübergänge](../messungen/2026-09-27-biomuebergaenge.md).
- Die Sprite-Tabelle braucht keine Fassung je Biom mehr, und der Vorlauf
  keine Biome je Blockstate. Ein gefärbtes Sprite trägt dafür 8 Bytes je
  Pixel mehr. Auf der grossen Welt hat die Tabelle bei scale 32 7489 statt
  26 341 Sprites; die Spitze des Speichers bis zur fertigen Tabelle
  halbiert sich. Die Spitze des ganzen Laufs steigt dagegen um 0,05 bis
  0,1 GiB, vermutlich durch die Biome je Block im Cache jedes Threads.
- Eine Instanz auf der Karte hat 40 statt 32 Bytes: die beiden Farben.
- Die drei Raster tragen das Licht des Blocks. Das Leuchten gehört deshalb
  zum Schlüssel einer Familie, sonst trüge ein gefluteter Block unter seiner
  Oberfläche das Licht eines anderen Zustands mit demselben Modell.
- Gegenüber einem Raster in der Farbe liegt ein Kanal höchstens um 2
  daneben, meist höchstens um 1: Das Raster rundet an jeder Schicht, die
  Karte einmal je Pixel. Gemessen an allen Vanilla-Blöcken, siehe
  [Biomfarben](../renderer/biomfarben.md), „Tönung beim Zeichnen“.
