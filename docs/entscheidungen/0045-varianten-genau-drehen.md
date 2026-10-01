---
title: "0045: Varianten genau um Vielfache von 90 Grad drehen"
description: Warum der Baker Varianten und Elemente um Vielfache von 90 Grad ohne sin und cos rechnet, wie die Matrix des Spiels, statt die Füllregel des Rasterizers zu ändern.
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
(`sin_cos` in `baker.rs`). Dieselbe Funktion dreht die Elemente, also sind
auch dort Vielfache von 90 Grad genau. Wie das Spiel dreht, steht in
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
- **Elemente um 180 Grad mit den Werten des Spiels.** JOML rechnet dort
  sin = ±8,7e-8, und eine Ecke liegt im Spiel bis zu 1,4e-6 eines Blocks
  neben ihrem Platz, weit unter einem Pixel. Im Renderer liegen die Ecken
  eines Blocks aber genau auf Pixelgrenzen, und dort entscheidet die
  Abweichung die Füllregel: Mit diesem sin verschiebt sich der Rahmen
  eines vollen Würfels bei scale 4 wieder um ein Pixel, wie in #61.

## Folgen

- Elemente um ±90 und ±270 Grad liegen genau wie im Spiel.
- Elemente um ±180 Grad liegen genau, im Spiel bis zu 1,4e-6 daneben.
- Andere Winkel, etwa 22,5, 45 oder 67,5 Grad, laufen weiter über
  `sin_cos`. Dort sorgt `EDGE_ON` für Flächen parallel zur Blickrichtung,
  siehe [Nähte](../renderer/naehte.md).
- `lock_uv` rundet nicht mehr.
