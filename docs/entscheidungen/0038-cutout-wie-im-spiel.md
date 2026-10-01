---
title: "0038: Flächen mit Löchern werden ausgeschnitten"
description: Warum der Renderer Flächen, deren Texturausschnitt Löcher, aber keine halb durchsichtigen Texel hat, nach dem Mitteln mit dem Alpha-Test des Spiels ausschneidet, statt sie zu mischen, und dark_cutout über die Farbe der Löcher nachbildet.
status: gilt
date: 2026-09-28
issues: [33]
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/assets/texture.rs
---

# 0038: Flächen mit Löchern werden ausgeschnitten

Die feste Blickachse ist seit [0051](0051-kameras-und-richtungen.md) die
Achse der Kamera des Laufs.

## Anlass

Laub sah auf der Karte milchig aus, am stärksten die Oberseiten der
Fichtennadeln, und Randpixel von Pflanzen waren halb durchsichtig (#33).
Der Renderer mittelte jede Textur mit Löchern über den Pixel und mischte das
Mittel wie eine durchscheinende Textur. Das Spiel mischt solche Flächen
nicht, es schneidet sie aus.

## Entscheidung

Der Renderer legt jede Fläche wie das Spiel in eine Schicht. Was
durchscheint, mischt er weiter; jede andere Fläche schneidet er nach dem
Mitteln aus wie `cutout_terrain`: Ein Pixel deckt ganz, wenn mehr als die
Hälfte der Abtastpunkte deckt, sonst gar nicht, und bei genau der Hälfte
entscheidet das Texel in der Pixelmitte. Bei `dark_cutout` zählen die
Löcher mit der Farbe mit, die das Spiel in sie schreibt. Regeln und Belege
stehen in [Rastern ohne Nähte](../renderer/naehte.md), „Ausgeschnitten
statt gemischt“.

Durch einige Prozent der Pixel einer Krone sieht man damit den Boden, unter
der festen Blickachse mehr als mit der Kamera des Spiels. Vom User am 28.09.
entschieden.

Zu [0008](0008-sprite-kanten-nicht-glaetten.md): Die Textur wird weiter über
den Pixel gemittelt, neu ist nur der Test danach. Teildeckung im Alpha, die
0008 an gemeinsamen Kanten vermeiden wollte, gibt es bei Flächen mit
Löchern damit gar nicht mehr.

## Verworfene Alternativen

- **Weiter mischen wie bisher:** das milchige Laub aus #33.
- **Nur `>= 0,5` wie der Shader, ohne eigenen Fall für genau die Hälfte.**
  Bei zwei mal zwei Abtastpunkten deckte dann jeder Pixel, den zwei von vier
  treffen, und Laub würde deutlich dichter als seine Textur.
- **Nur das Texel in der Pixelmitte.** Bei scale 32 fast dasselbe; auf den
  nativen Stufen zählte aber aus vier mal vier oder acht mal acht Texeln
  nur eines, mit Aliasing.
- **Die Mip-Stufen des Spiels nachbauen**, `MipmapGenerator` mit
  `scaleAlphaToCoverage`. Das hielte auf den nativen Stufen die Deckung,
  braucht aber je Textur eine Stufe je scale und dafür einen zweiten Weg
  durch das Abtasten. Wie weit das Laub dort dichter wird, steht unter
  „Folgen“.
- **Deckungsmasken je Abtastpunkt statt Alpha**, wie Mehrfachabtastung auf
  der Grafikkarte: ändert den Blit und die Karte und kostet Speicher.
- **Laub deckend, wie mit „Laub ausschneiden: aus“:** nicht die
  Voreinstellung des Spiels, `Options.cutoutLeaves` ist wahr.

## Folgen

- Von den Blocktexturen in Vanilla schneidet der Renderer 517 aus: Laub,
  Pflanzen, Glas, Schienen. 47 scheinen durch und mischen weiter, etwa
  Buntglas, Eis und Wasser, ohne die zehn Abbaustufen. 695 decken ganz.
- Laub, Pflanzen und Schienen bekommen harte Kanten; das Goldbild ist neu,
  304 Pixel an den Kanten der Overlay-Streifen.
- Auf den nativen Stufen wird Laub dichter als im Spiel, gemessen in
  [2026-09-28, Cutout](../messungen/2026-09-28-cutout.md), dort auch
  Grösse und Dauer.
