---
title: "0013: scale 32 als Standard"
description: Warum die Vorgabe für --scale 32 ist, warum nur Vielfache von 4 gelten und warum der scale zum Kachelbaum gehört.
status: gilt
date: 2026-09-25
issues: [8]
code:
  - renderer/src/render/projection.rs
  - renderer/src/cli.rs
---

# 0013: scale 32 als Standard

## Anlass

`scale` ist die Breite des ganzen Würfels, eine Seitenfläche ist halb so
breit. Bei scale 16 hat sie acht Pixel für sechzehn Texel: Die Hälfte der
Texturzeilen fehlt.

## Entscheidung

Die Vorgabe ist scale 32 (`Projection::DEFAULT_SCALE`); erst dann ist die
Textur vollständig zu sehen. `--scale` nimmt nur Vielfache von 4. Der scale
gehört zum Baum: `map.json` hält ihn fest, und ein Lauf mit anderem scale
bricht ab, bevor er einen Chunk liest. Siehe
[Die Kamera](../renderer/kamera.md).

## Verworfene Alternativen

- **scale 16 mit mehr Browser-Zoom oder `maxNativeZoom`.** Das hilft nicht:
  die fehlenden Texel sind nicht gerendert, das Review von #8 hat es
  vorgerechnet.
- **Andere scales als Vielfache von 4.** Die Projektion setzt Blöcke in
  Schritten von scale/4 Pixeln; sonst läge jede zweite Blockreihe auf einem
  halben Pixel, und benachbarte Reihen überdeckten sich.

## Folgen

- Viermal so viele Kacheln wie bei scale 16: für die Testwelt rund 300 000
  statt 74 000. `--scale 16` bleibt die sparsame Wahl.
- Ein vergessenes `--scale` beim Nachrendern in einen mit 16 gebauten Baum
  hätte ihn zerlegt; deshalb die Prüfung gegen `map.json`.
- Ein Baum mit scale 2, 6 oder 10 aus einem älteren Stand lässt sich nicht
  fortsetzen.
