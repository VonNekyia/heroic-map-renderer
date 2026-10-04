---
title: "0076: Wärmer in Cinematic"
description: Warum Cinematic überall etwas wärmer abgleicht, ohne dass kalte Biome warm werden - die Wärme aus 0069 um 0,05 höher über den neuen Wert waerme_grund, also neutral 1,05, ganz warm 1,3, ganz kühl 0,9; warum nicht 1,35. Löst 0069 in den Werten der Wärme ab.
status: gilt
date: 2026-10-04
issues: [124]
code:
  - renderer/src/render/look.rs
  - renderer/src/render/kino.rs
---

# 0076: Wärmer in Cinematic

## Anlass

Mit den Werten aus [0069](0069-ein-himmelslicht-und-kaelte.md) wirkten
Wüsten in den Bäumen der grossen Welt mit Cinematic nicht mehr warm. Der
Maintainer wollte alles etwas wärmer, Wüsten wieder warm, kalte Biome
weiter kalt.

## Entscheidung

Der Weissabgleich wirkt zwischen Kälte und Wärme mit dem neuen Wert
`waerme_grund` statt mit 1. Wärme und Kälte rechnen von dort aus, mit
denselben Abständen und Temperaturen wie in 0069:

`w = waerme_grund + waerme · clamp((t − waerme_von) / (waerme_bis − waerme_von), 0, 1) − kaelte · clamp((kaelte_von − t) / (kaelte_von − kaelte_bis), 0, 1)`

Mit `waerme_grund` 1,05 liegt jede Stufe aus 0069 um 0,05 höher: Wüste
1,3, neutral 1,05, Schnee 0,9. Gewählt hat der Maintainer am 04.10.

| Wert | in `Look` | 0069 |
|---|---|---|
| Abgleich zwischen Kälte und Wärme | `waerme_grund`: 1,05 | 1, fest |
| Wärme höchstens, dazu | `waerme`: 0,25 | gleich |
| warm ab, ganz warm ab | `waerme_von`: 0,5, `waerme_bis`: 1,0 | gleich |
| Kälte höchstens, ab | `kaelte`: 0,15 | gleich |
| kühl unter, ganz kühl ab | `kaelte_von`: 0,15, `kaelte_bis`: 0 | gleich |

**Wirkung je Biom**, `w` nach der rohen `temperature`:

| Biome | `t` | 0069 | 0076 |
|---|---|---|---|
| verschneite Ebenen, Hänge und Gipfel, Eisspitzen, gefrorener Fluss und gefrorenes Meer | bis 0 | 0,85 | 0,9 |
| verschneiter Strand | 0,05 | 0,9 | 0,95 |
| windige Hügel, Taiga, Meer, Flüsse, Wiesen | 0,15 bis 0,5 | 1 | 1,05 |
| Wald | 0,7 | 1,1 | 1,15 |
| Ebenen, Strände | 0,8 | 1,15 | 1,2 |
| Savanne, Wüste, Badlands | ab 1,0 | 1,25 | 1,3 |

**Im Ton:** Weiss in linearem Licht 1, Weissabgleich der Oberwelt,
Belichtung 0,25, in sRGB (Test `ton_mit_waerme` in `kino.rs`):

| | 0069 | 0076 |
|---|---|---|
| Schnee | [144, 137, 121] | [145, 137, 120] |
| neutral | [145, 137, 117] | [146, 137, 116] |
| Wüste | [147, 137, 112] | [148, 137, 111] |

- **Kalt bleibt kalt:** Schnee liegt mit 0,9 weiter unter 1, also kühler
  als der Abgleich allein, und 0,15 unter neutral wie in 0069.
- **Alles etwas wärmer:** Jedes Biom bekommt 0,05 mehr. Die Abstände
  zwischen kalt, neutral und warm bleiben wie in 0069.
- **Kein Überlaufen:** Eine weisse Fläche voll zur Sonne im vollen
  Himmelslicht bleibt mit 1,3 bei [254, 252, 239], siehe
  [Cinematic](../renderer/cinematic.md), „Wärme“.

## Verworfene Alternativen

- **Wüste 1,35**, also `waerme` 0,3. So hatte der Maintainer zuerst
  gewählt. Am Renderer erreicht dann eine weisse Fläche voll zur Sonne im
  Rot 255, [255, 252, 237] (Test `weisse_flaeche_in_voller_sonne`). Der
  Maintainer hat darauf 1,3 genommen.
- **Nur `waerme` höher, neutral 1:** Wüsten würden wärmer, alles andere
  bliebe wie in 0069. Der Maintainer wollte alles etwas wärmer.
- **Die Wärme 1,5 aus [0058](0058-look-von-cinematic.md):** Rot erreicht
  schon bei fast weissen Texeln 255, in Wüste, Savanne, Badlands und auf
  den steinigen Gipfeln.
- **Die Stufen als drei Werte für die Enden:** `waerme` und `kaelte`
  bleiben Abstände zur Mitte wie in 0069. So kommt nur ein Wert dazu, und
  mit `waerme_grund` 1 ist die Formel die aus 0069.

## Folgen

- **Neu rendern:** Ein Wert mehr, also ein anderer `lookHash`. Bäume mit
  Cinematic nimmt ein Lauf erst nach dem Löschen wieder an, siehe
  [`map.json`](../benutzung/map-json.md), „Look“. `VERFAHREN` bleibt 3:
  Der neue Wert geht mit seinem Namen in den Fingerabdruck ein.
- **Auch die Mitte ist warm:** Mit 1,05 ist eine weisse Fläche in Sonne und
  Himmel der Oberwelt in einem neutralen Biom nicht mehr ganz farblos,
  sondern leicht warm.
- **Nicht neu gemessen:** die Zahlen des Researchers zu 0069, Blau einer
  weissen Fläche unter den Himmeln aus dem Datapack der grossen Welt, und
  die Kennzahlen und Grenzen aus 0058 und
  [0060](0060-grenze-schatten-sonne-am-renderer.md).
