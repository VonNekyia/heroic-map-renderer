---
title: "0011: Färbung als Sprite-Fassung"
description: Warum Biomfarben vorab je Biom als Fassung gerastert werden statt beim Zeichnen zu multiplizieren, und warum die Farbtabelle im Code steht.
status: abgelöst durch 0033
date: 2026-09-25
issues: [8]
code:
  - renderer/src/assets/colors.rs
  - renderer/src/render/sprites.rs
---

# 0011: Färbung als Sprite-Fassung

Abgelöst durch
[0033: Tönung beim Zeichnen statt Fassungen je Biom](0033-toenung-beim-zeichnen.md).

## Anlass

Gras, Laub und Wasser haben graue Texturen; ihre Farbe hängt am Biom.

## Entscheidung

Eine gefärbte Blockstate bekommt je Biom eine vorab gerasterte Fassung;
gleiche Farben teilen sich das Sprite. Gebaut wird je Familie nur für die
Biome, mit denen sie im Vorlauf eine Section teilt. Welche Blöcke gefärbt
werden, steht wie in `BlockColors` fest im Code (`colors.rs`), beschränkt
auf das, was auf einer Karte Fläche macht. Siehe
[Biomfarben](../renderer/biomfarben.md).

## Verworfene Alternativen

- **Die Farbe beim Zeichnen multiplizieren.** Der Renderpfad sollte Pixel
  kopieren und sonst nichts.
- **Die Farbtabelle aus den Assets.** Dort steht sie nicht; Minecraft
  verdrahtet sie im Code.

## Folgen

- Die Tabelle wächst mit den Biomen. Die grosse Welt kam mit den
  Wasserfassungen von damals auf 428 498 Sprites mit geschätzt rund 1,5 GB
  an Pixeln, bevor geteilte Familien, ein Biomindex, der Biomfilter und
  geteilte Einträge sie verkleinerten.
- Seit [0030](0030-licht-je-block.md) multipliziert der Blit doch, mit dem
  Licht und der weichen Beleuchtung; die Farbe bleibt eine Fassung.
- Übergänge zwischen Biomen mischt der Renderer nicht; die Farbe gilt je
  Section und Biom.
