---
title: "0069: Ein Himmelslicht und Kälte in Cinematic"
description: Warum Cinematic in jedem Biom mit dem Himmelslicht der Oberwelt beleuchtet und die Farben des Himmels je Biom nur noch dem Wasser gibt, und warum die Wärme schmaler ist und eine kalte Seite hat. Löst 0058 in der Farbe des Himmelslichts und im Weissabgleich nach Biom ab.
status: gilt
date: 2026-10-04
issues: [124]
code:
  - renderer/src/render/look.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/metatile.rs
---

# 0069: Ein Himmelslicht und Kälte in Cinematic

## Anlass

#124: Am Vollrender der grossen Welt vom 03.10. wirkte Schnee oft warm.
Warm, normal und kalt lagen zu weit auseinander.

- **Abgleich und Licht passten nicht zusammen.** Der Weissabgleich `v`
  aus [0058](0058-look-von-cinematic.md) macht eine weisse Fläche in Sonne
  und Himmelslicht der Oberwelt farblos. Das Himmelslicht hatte nach 0058
  aber die Farbe des Himmels je Biom.
- **Datapacks geben Biomen andere Himmel.** Die Biome der grossen Welt
  geben Schnee einen grauen Himmel und Badlands einen beigen. Der feste
  Abgleich zog dann Blau aus einem Licht, das kaum blau ist. Was vom Himmel
  beleuchtet wird, kippte nach Orange, vor allem Schatten und Seiten. Die
  Verhältnisse je Biom, überschlagen, stehen in #124.
- **Keine kalte Seite.** Die Wärme aus 0058 reichte bis 1,5. Schnee blieb
  nur klar, nie kühl.

## Entscheidung

Weg 1 aus #124 und die Wärme enger, mit einer kalten Seite. Gewählt hat
der Reviewer in der Nacht vom 04.10. an den Vergleichsbildern des
Researchers, der Maintainer hat ihm die Wahl überlassen und kann die Werte
noch ändern (#124, issuecomment-5974735230).

- **Ein Himmelslicht:** In jedem Biom und jeder Dimension hat das
  Himmelslicht die Farbe aus Himmel und Nebel der Oberwelt, der Himmel zu
  0,75, wie in 0058. Auf genau dieses Licht ist der Abgleich `v`
  gerechnet. Eine weisse Fläche nach oben in Sonne und Himmel ist so in
  jedem Biom farblos; die Farbe des Lichts kommt nur noch aus der
  Temperatur.
- **Die Farben des Himmels je Biom** bleiben für das Wasser: `sky_color`
  und `fog_color` für die Spiegelung, `water_fog_color` für die Farbe nach
  der Strecke.
- **Wärme:** je Kanal `1 + (v − 1) · w` wie in 0058. Neu ist die kalte
  Seite:

  `w = 1 + waerme · clamp((t − waerme_von) / (waerme_bis − waerme_von), 0, 1) − kaelte · clamp((kaelte_von − t) / (kaelte_von − kaelte_bis), 0, 1)`

  `t` ist die `temperature` des Bioms, gemischt wie in 0058.

| Wert | in `Look` | 0058 |
|---|---|---|
| Wärme höchstens | `waerme`: 0,25 | 0,5 |
| warm ab, ganz warm ab | `waerme_von`: 0,5, `waerme_bis`: 1,0 | gleich |
| Kälte höchstens | `kaelte`: 0,15 | keine |
| kühl unter, ganz kühl ab | `kaelte_von`: 0,15, `kaelte_bis`: 0 | keine |

**Wirkung je Biom:**
- **Kühl, `w` 0,85:** ab 0 abwärts, also die verschneiten Ebenen, Hänge
  und Gipfel, Eisspitzen, gefrorene Flüsse und Meere. Ein verschneiter
  Strand (0,05) bekommt 0,9.
- **Klar, `w` 1:** von 0,15 bis 0,5, also windige Hügel, Taiga, Meer,
  Flüsse und Wiesen.
- **Warm:** Wald (0,7) 1,1, Ebenen und Strände (0,8) 1,15; ab 1,0 1,25,
  also Savanne, Wüste und Badlands.
- **Kühler unter 1:** Mit `w` unter 1 nimmt der Abgleich dem Himmelslicht
  weniger Blau. 0058 verwarf einen Abgleich unter 1 für die ganze Welt,
  weil er alles blau macht; hier gilt er nur in den kalten Biomen.

**Warum diese Werte:** gemessen vom Researcher am Renderer, eine weisse
Fläche nach oben in voller Sonne, Himmelslicht 15, ohne AO, mit den
Himmeln aus dem Datapack der grossen Welt. Die Zahl ist Blau in sRGB nach
dem Ton; je höher, desto kühler.

| Fassung | Schnee | `plains` | Wüste, Badlands |
|---|---|---|---|
| 0058 | 210 | 228 | 219, 184 |
| Weg 1 | 240 | 228 | 219 |
| Weg 1, `waerme` 0,35 | 240 | 232 | 226 |
| **Weg 1, `waerme` 0,25, `kaelte` 0,15** | **245** | **234** | **230** |
| Weg 1, `waerme` 0,25, `kaelte` 0,30 | 248 | 234 | 230 |
| Weg 1, `waerme` 0,15 | 240 | 237 | 234 |

- **Weg 1** nimmt das Datapack aus dem Licht. Schnee wird weiss mit blauen
  Schatten, fast wie ohne Datapack; Badlands verlieren den orangen
  Schatten. In der grossen Welt weicht auf 39,5 % des Landes das
  Himmelslicht des Bioms mehr als 15 % von dem der Oberwelt ab.
- **`waerme` 0,25:** Warme Biome bleiben wärmer als `plains`, aber Weiss in
  der Wüstensonne kippt nicht mehr ins Gelbe.
- **`kaelte` 0,15:** Schnee wirkt kalt statt nur klar. Spiegelbildlich zur
  Wärme: Badlands bekommen ein Viertel mehr Abgleich, Schnee 15 % weniger.
- **`kaelte_von` 0,15:** Unter dieser Temperatur fällt im Spiel Schnee
  statt Regen. `Biome.warmEnoughToRain` vergleicht `getTemperature` am Block
  mit `>= 0.15f`, `coldEnoughToSnow` ist das Gegenteil; Client 26.2, per
  javap. Die kalte Seite trifft so die Biome, in denen es schneit. Das
  Spiel nimmt dort die Temperatur nach der Höhe, Cinematic die rohe des
  Bioms, siehe [Cinematic](../renderer/cinematic.md), „Wärme“.
- **Geordnet:** 245, 234 und 230 nach der Temperatur, statt 184 bis 228
  mit Schnee wärmer als die Ebene.

## Verworfene Alternativen

- **Abgleich je Pixel auf das gemischte Himmelslicht** (Weg 2 aus #124).
  Der Himmel jedes Bioms würde farblos. Das Bild wäre fast dasselbe wie
  mit einem Himmelslicht, kostet aber eine Rechnung je Pixel mehr.
- **Die Farbe des Himmels je Biom behalten und nur die Wärme enger.**
  Schnee unter einem grauen Himmel bliebe warm, denn warm wird er allein
  über den Himmel, nicht über die Temperatur.
- **`waerme` 0,15:** Wüste und Ebene fast gleich, 234 gegen 237. Die Wärme,
  die 0058 wollte, ginge verloren.
- **`waerme` 0,35:** wärmer, Weiss in der Wüste 226; möglich, wenn der
  Maintainer mehr Wärme will.
- **`kaelte` 0,30:** sonniger Schnee wird sichtbar bläulich, 248.
- **`kaelte_von` 0,05:** träfe nur Schnee. Mit 0,15 trifft die kalte Seite
  auch Biome knapp darunter leicht, im Datapack der grossen Welt eine
  Kieswüste (0,14) und eine Taiga (0,13).

## Folgen

- **Neu rendern:** Die Werte sind andere, und das Verfahren hat den Stand
  3, also ändert sich `lookHash`. Bäume mit Cinematic nimmt ein Lauf erst
  nach dem Löschen wieder an, siehe
  [`map.json`](../benutzung/map-json.md), „Look“.
- **Ein Biom färbt das Licht nur noch über seine Temperatur.** Ein Himmel
  aus einem Datapack ändert das Licht nicht mehr, nur das Wasser. Auch im
  Spiel ohne Datapack beleuchten Biome mit eigenem Himmel jetzt wie
  plains; ihr Himmel lag dort nah an dem der Oberwelt.
- **Andere Dimensionen** mit Himmelslicht beleuchten mit dem Himmel der
  Oberwelt. Nether und Ende haben kein Himmelslicht.
- **Weiter eine Wahl, nicht das Spiel:** Das Spiel beleuchtet Blöcke mit
  `sky_light_color` allein, siehe [Cinematic](../renderer/cinematic.md),
  „Was bleibt eine Näherung“.
- **Nicht neu gemessen:** Die Kennzahlen und Grenzen aus 0058 und
  [0060](0060-grenze-schatten-sonne-am-renderer.md) sind mit diesen Werten
  nicht neu gemessen.
