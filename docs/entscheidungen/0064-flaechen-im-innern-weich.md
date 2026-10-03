---
title: "0064: Flächen im Innern weich wie das Spiel"
description: Warum der Renderer Flächen im Innern eines Blocks weich beleuchtet wie das Spiel, mit sechs Plätzen in der AO-Karte und den Ecken aller Plätze in der Instanz auf der Karte.
status: gilt
date: 2026-10-03
issues: [51]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/assets/blockstate.rs
---

# 0064: Flächen im Innern weich wie das Spiel

## Anlass

Bis hier lag eine Fläche, die nicht auf dem Rand ihres Blocks liegt, flach
im Licht der eigenen Zelle, siehe
[0040](0040-licht-selbst-ausbreiten.md). Das Spiel beleuchtet auch sie
weich (`BlockModelLighter.prepareQuadAmbientOcclusion`). Am meisten fehlte
das auf Schneedecken, Trampelpfaden, Ackerboden und den Oberseiten unterer
Platten und Treppen: Neben einer Mauer oder zwischen Gras blieben sie hell.
Issue #51.

## Entscheidung

Der Renderer rechnet Flächen im Innern nach den Regeln des Spiels, siehe
[Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Die Regeln des
Spiels“ und „Licht an den Ecken“:

- **Sechs Plätze.** Die AO-Karte trägt je Pixel einen Platz statt einer
  Seite: je Seite im Blick einen für Flächen auf dem Rand und einen für
  Flächen im Innern (`AO_PLAETZE`, `ao_face`). Die Seite einer Fläche ist
  die Richtung des Spiels für ihr Viereck (`findClosestDirection`).
- **Nur, was zu sehen ist.** `ecken_at` rechnet die Plätze, die das Sprite
  zeigt (`SpriteSet::plaetze`).
- **Die Mitte aus Bit 2** von `schatten.txt`: Nicht `isLightPermeable` ist
  in 26.3 genau `isSolidRender`, geprüft über alle Zustände
  (`sicht_ist_solid_render`).
- **Durch das Wasser.** Das Wort der AO-Karte kommt vom vordersten
  Fragment, das nicht von einer Flüssigkeit ist: Den Anteil des Wassers
  beleuchtet die Tönungskarte, der Rest ist, was durch das Wasser zu sehen
  ist. Sonst läge die Oberseite einer gefluteten Platte unter ihrer
  Wasserfläche flach im Licht der eigenen Zelle.
- **Die Ecken in der Instanz.** Eine Instanz auf der Karte trägt je Kanal
  sechs Wörter statt drei, 104 statt 68 Bytes.

## Verworfene Alternativen

- **Drei Plätze, die Flächen im Innern auf dem Platz ihrer Seite.** Eine
  Treppe zeigt oben ihre obere Stufe auf dem Rand und ihre untere im
  Innern, mit verschiedenen Ecken. Mit einem Platz je Seite bekäme eine
  von beiden das Licht der anderen.
- **Die Ecken in einem eigenen Puffer**, mit einem Index in der Instanz:
  Instanzen ohne Ecken würden kleiner. Das kostet eine Bindung mehr und
  einen Umweg je Pixel. Der Puffer der Instanzen fasst mit 104 Bytes noch
  rund 2,6 Millionen Instanzen je Durchgang; sechzehn Kacheln brauchen
  weit weniger.
- **Ein eigenes Bit für `isSolidRender`.** Aus `Schatten.java` erzeugt
  stimmte es in allen 1092 Ziffern von `schatten.txt` mit Bit 2 überein.
  Es wäre eine zweite Ebene der Bitmasken für dieselbe Antwort.
- **Teilflächen bilinear wie das Spiel.** Das Spiel mischt die vier Werte an
  den Ecken einer Teilfläche nach ihrer Lage (`facePartial`). Das bliebe
  ein Verlauf je Fläche statt je Seite, mehr Daten je Pixel. Es bleibt eine
  Näherung wie in 0040.

## Folgen

- Mehr Sprites tragen Ecken, und mehr Pixel mischen das Licht ihrer Ecken.
  Ein Lauf dauert im Median 3 bis 4 % länger am Stand der Testwelt, mit
  Cinematic im Fichtenwald 5,4 %; die Kacheln wiegen bis 5,7 % mehr,
  siehe [2026-10-03, Flächen im Innern weich, Kosten](../messungen/2026-10-03-flaechen-im-innern.md).
- Das Bild ändert sich: Goldbild, Doku-Bilder und die Kennzahlen der
  Ansichten sind neu.
- Flächen, deren Seite der Blick nicht zeigt, liegen weiter flach, siehe
  [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Was bleibt eine
  Näherung“.
