---
title: "0012: Varianten aus der Position wie im Client"
description: Warum der Renderer die Alternative einer Variantenliste aus der Position würfelt wie der 26.2-Client, statt immer die erste zu nehmen.
status: gilt
date: 2026-09-25
issues: [2, 8]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/mod.rs
---

# 0012: Varianten aus der Position wie im Client

## Anlass

Seit Schritt 2 gewann immer die erste Alternative einer Variantenliste. 34
Blockstates liegen als Liste vor, Sand, Stein, Erde, Grasblock in vier
Drehungen; das Review von #8 führte die Sandtapete darauf zurück.

## Entscheidung

Der Renderer rechnet `Mth.getSeed(x, y, z)` nach, setzt damit einen
Nachbau von `SingleThreadedRandomSource`, zieht `nextInt(Gesamtgewicht)`
wie `WeightedList.getRandomOrThrow` im 26.2-Client und zählt die Gewichte
der Reihe nach ab. Obere Hälften von Doppelpflanzen und Türen würfeln mit
der Position der unteren, das Fussende eines Betts mit der des Kopfendes,
wie `getSeed` in `DoublePlantBlock`, `DoorBlock` und `BedBlock`. Fehlt einer
Alternative das Modell, zeichnet der Renderer den Missing-Würfel und behält
ihr Gewicht. Siehe [Varianten aus der Position](../renderer/varianten.md).

## Verworfene Alternativen

- **Immer die erste Alternative**, wie seit Schritt 2: deterministisch, aber
  eine Tapete.
- **Das Verfahren älterer Versionen, der erste `nextLong`**, die erste
  Fassung in #8: Sie traf an 256 Positionen nur 65-mal die Drehung des
  Spiels, also Zufall.
- **Eine kaputte Alternative weglassen.** Dann würfelten auch die intakten
  Positionen anders als im Client.

## Folgen

- Alle Alternativen sind vorab gerastert; der Renderpfad rechnet je Block
  nur die Saat, bei einer einzigen Alternative nicht einmal die.
- Die Wahl hängt weder von der Kachel noch vom Thread ab.
- Multipart-Listen, nur bei Bambus, Chorus und Feuer, würfeln nicht; dort
  gilt der erste Eintrag.
