---
title: "0045: Varianten genau um Vielfache von 90 Grad drehen"
description: Warum der Baker die Drehung einer Variante ohne sin und cos rechnet, wie die Matrix des Spiels, statt die Füllregel des Rasterizers zu ändern.
status: gilt
date: 2026-10-01
issues: [61]
code:
  - renderer/src/assets/baker.rs
---

# 0045: Varianten genau um Vielfache von 90 Grad drehen

## Anlass

#61: Der Baker drehte die Varianten über `sin_cos` in `f32`, und eine Ecke
lag danach rund 3e-7 neben ihrem Platz. In 2:1 war das fast unsichtbar. In
einer Draufsicht, wie sie für die Kameras geplant ist, fielen Kanten von
zufällig gedrehtem Sand und Gras zwischen zwei Nachbarn, und durch die
Lücke schien dunkler Boden.

## Entscheidung

Die Drehung einer Variante rechnet der Baker wie das Spiel als
Vierteldrehungen: sin und cos sind 0 oder ±1, und das Ergebnis ist genau
(`viertel_drehen` in `baker.rs`). Wie das Spiel dreht, steht in
[Modelle und Texturen](../renderer/modelle-und-texturen.md), „Drehung der
Varianten“.

## Verworfene Alternativen

- **Eine inklusive Füllregel im Rasterizer.** Sie schloss die Lücken im
  Prototyp der Draufsicht auch. Dann nähmen aber zwei Flächen einen Pixel
  auf ihrer gemeinsamen Kante beide, und Durchscheinendes bekäme dort zwei
  Schichten, siehe [Nähte](../renderer/naehte.md), „Füllregel“.
- **Weiter über `sin_cos`, mit Runden danach.** So machte es `lock_uv` mit
  Tausendsteln. Die Grenze fürs Runden ist willkürlich, und das Spiel
  rundet nicht, seine Matrix ist genau.

## Folgen

- Die Drehung der Elemente bleibt bei `sin_cos`, denn 22,5 und 45 Grad
  sind nicht genau darstellbar. Dort sorgt `EDGE_ON` für Flächen parallel
  zur Blickrichtung, siehe [Nähte](../renderer/naehte.md).
- `lock_uv` rundet nicht mehr.
