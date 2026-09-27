---
title: Varianten aus der Position
description: Wie der Renderer die Alternative einer Variantenliste aus der Blockposition würfelt, genau wie der 26.2-Client, auch für beide Hälften eines Doppelblocks.
code:
  - renderer/src/render/sprites.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/mod.rs
---

# Varianten aus der Position

34 Vanilla-Blockstates liegen als Liste vor, Sand, Stein, Erde, Grasblock in
vier Drehungen. Welche ein Block bekommt, würfelt Minecraft aus seiner
Position, und der Renderer rechnet genau das nach (`Family::pick`, `seed`
und `java_next_int` in
[`renderer/src/render/sprites.rs`](../../renderer/src/render/sprites.rs)).
Damit sieht Sand aus wie im Spiel statt wie eine Tapete, und die Wahl hängt
weder von der Kachel noch vom Thread ab. Belegt gegen die Klassen des
26.2-Clients, siehe
[0012](../entscheidungen/0012-varianten-aus-der-position.md).

## Wie gewürfelt wird

`ModelBlockRenderer` sät seinen Zufallsgenerator mit `Mth.getSeed(x, y, z)`,
`nextInt` über das Gesamtgewicht zieht eine Zahl, wie
`WeightedList.getRandomOrThrow`, und die Gewichte werden der Reihe nach
abgezählt, bis sie verbraucht ist. Bis 1.21.4 nahm Minecraft stattdessen
`abs((int) nextLong()) % total`. Geprüft an sieben Positionen,
auch an den Extremwerten von `int`, mit vier Gewichtslisten, gegen die
Klassen des 26.2-Clients. Alle Alternativen sind vorab gerastert; der
Renderpfad rechnet je Block nur die Saat, bei einer einzigen Alternative
nicht einmal die.

## Doppelblöcke

Obere Hälften von Doppelpflanzen und Türen nehmen die Position der unteren,
das Fussende eines Betts die des Kopfendes: `DoublePlantBlock`, `DoorBlock`
und `BedBlock` überschreiben `getSeed`, per javap am 26.2-Client, und beide
Hälften passen so immer zusammen (`seed_offset`).

## Kaputte Alternativen

Fehlt einer Alternative das Modell, zeichnet der Renderer dort wie das Spiel
den Missing-Würfel, und ihr Gewicht bleibt, wie im Client, der jeden
Verweis für sich auflöst. Fiele sie weg, würfelte `nextInt` auch an den
intakten Positionen anders als im Client. Ein Pack mit einem Tippfehler
bricht so keinen Lauf ab. Das gilt für jeden kaputten
Verweis: auch wenn alle Alternativen einer Blockstate kaputt sind, und für
den einen kaputten Teil eines Multipart-Modells, jeweils mit der Drehung
des Eintrags. Wie der Renderer Blockstate-Dateien liest, steht in
[Blockstates](blockstates.md).

## Was bleibt eine Näherung

- **Multipart-Listen ohne Zufall.** Nur Bambus, Chorus und Feuer haben in
  `multipart` Listen; dort gilt je Fall der erste Eintrag. Die
  Variantenlisten, das, was Sand und Stein betrifft, würfeln.
