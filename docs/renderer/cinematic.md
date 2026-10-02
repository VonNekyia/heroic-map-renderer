---
title: Cinematic
description: Wie --cinematic dieselbe Karte im Licht des Spiels in HDR zeichnet - Sprites ohne Schattierung nach Richtung, Himmels- und Blocklicht getrennt an den Ecken, die Farbe des Himmels je Biom, Weissabgleich, Belichtung und Kurve aus 0058. Stand Phase 1, ohne Sonne.
code:
  - renderer/src/render/look.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tint.rs
  - renderer/src/assets/colors.rs
  - renderer/src/assets/dimension.rs
  - renderer/src/cli.rs
---

# Cinematic

`--cinematic` zeichnet mit `--tiles` oder `--render` dieselbe Karte im
Licht des Spiels in HDR, mit Weissabgleich, Belichtung und Kurve nach
[0058](../entscheidungen/0058-look-von-cinematic.md). Kacheln landen in
einem eigenen Baum `<kamera>-<richtung>-cinematic`, samt nativen Stufen und
Pyramide. Cinematic nimmt dieselben Kandidaten, dieselbe Deckungsmaske und
dieselben Draws wie die Karte, also dasselbe Pixelraster bei jeder Kamera,
Richtung und jedem scale; anders ist nur das Licht je Pixel, entschieden in
[0053](../entscheidungen/0053-cinematic-als-schalter-der-karte.md). Es
zeichnet immer die CPU. Phase 1 (#72) brachte das Licht des Spiels, Phase 2
(#73) bringt Sonne, Schatten, Wasser, Leuchten, Wärme nach Biom und Bloom;
bis jetzt davon die Sonne nach dem Winkel zur Fläche, ohne Schatten.

## Werte des Looks

Alle Werte stehen benannt an einer Stelle, `LOOK` in
[`renderer/src/render/look.rs`](../../renderer/src/render/look.rs):

| Wert | in `Look` | genutzt |
|---|---|---|
| Stärke des Himmelslichts | `himmel`: 3 | ja |
| Anteil der Farbe des Himmels am Himmelslicht, der Rest ist Nebel | `himmel_anteil`: 0,75 | ja |
| Stärke des Blocklichts | `block`: 1,5 | ja |
| Sonne: Stärke, Farbe linear, Höhe, waagrecht von links zur Kamera hin | `sonne`: 3, `sonne_farbe`: (1; 0,93; 0,83), `sonne_hoehe`: 48,47°, `sonne_seite`: 8,75° | ja |
| Belichtung | `belichtung`: 0,25 | ja |
| Kurve: gerade bis, flach ab | `knie`: 0,8, `flach`: 1,2 | ja |

- **Herkunft:** alle aus 0058, bis auf `himmel_anteil`. 0058 sagt nur „in
  der Farbe des Himmels“. Der Prototyp aus #89, an dem 0058 abgestimmt
  ist, nimmt für das Licht auf einer Fläche nach oben die Farbe des Nebels
  und des Himmels, linear gemischt mit 0,75 Himmel.
- **Fingerabdruck:** Jeder Baum mit Cinematic hält die Werte als
  `lookHash` in `map.json`; mit anderen bricht ein Lauf ab. Wie er
  gerechnet wird, steht in [`map.json`](../benutzung/map-json.md), „Look“.
- **Ändern:** die Werte in `LOOK` ändern, den Fingerabdruck im Test
  `fingerabdruck_der_werte_aus_0058` nachziehen, die Bäume mit Cinematic
  neu rendern. Die neuen Werte hält eine Entscheidung fest, die 0058
  ablöst.

## Sprites für Cinematic

Nur mit dem Schalter backt die Sprite-Tabelle eigene Sprites
(`SpriteSet::build_mit_licht` mit dem Look, `rastern` mit `kino`):

- **Ohne Schattierung nach Richtung:** jede Fläche mit dem Faktor 1, auch
  Flächen aus Blockentity-Modellen und die Seiten von Flüssigkeiten. Die
  Faktoren der Karte stehen in [Dimensionstypen](dimensionstypen.md),
  „Schattierung nach Richtung“.
- **Geometrie je Pixel** (`Sprite::geometrie`): vom vordersten Fragment
  die Tiefe entlang der Blickachse relativ zum Ursprung des Blocks, die
  Normale der Fläche im Blick und ob sie `shade` hat. Die Sonne rechnet mit
  ihnen, siehe „Sonne“.
- **Sonst wie die Karte:** dieselben Fragmente, Füllregel, Alpha-Tests,
  AO-Karte und Tönungskarte. Ein Sprite deckt also genau die Pixel, die das
  der Karte deckt; das prüft `cinematic_ohne_schattierung_mit_geometrie`.

## Licht an den Ecken

Welches Licht ein Block bekommt und wie die weiche Beleuchtung an den
Ecken rechnet, bleibt wie bei der Karte, siehe
[Wasser und Licht](wasser-und-licht.md) und
[Weiche Beleuchtung](weiche-beleuchtung.md), „Licht an den Ecken“. Nur
legen `licht_fuer` und `ecken_at` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
statt der Helligkeit der Lightmap je Ecke drei Werte ab
(`kino_kanaele`):

- das Himmelslicht in Sechzehnteln einer Stufe, wie es `smooth_blend`
  liefert, 0 bis 240;
- das Blocklicht ebenso;
- den Schatten der weichen Beleuchtung in 255steln.

Durch die Lightmap gerechnet ergeben sie genau das Licht der Karte, an
jeder Ecke und für das Wasser. Das prüft
`cinematic_zeichnet_dieselben_draws_wie_die_karte` in
`renderer/tests/metatile.rs` bei jeder Kamera und Richtung der Invarianten,
zusammen mit denselben Draws.

## Licht in HDR

An einem Pixel auf einer Seite der AO-Karte mischt Cinematic die drei Werte
mit denselben Anteilen der Ecken wie die Karte, aber ungerundet; sonst gilt
das Licht des Blocks. Daraus das Licht je Kanal (`Kino::licht` in
[`renderer/src/render/kino.rs`](../../renderer/src/render/kino.rs)):

`Licht = Schatten / 255 · (Himmelslicht des Bioms · H(Himmel) + B(Block))`

- **H:** je Stufe `getBrightness(s)` mal `sky_light_factor` und
  `sky_light_color` der Dimension mal 3 (`himmel`).
- **B:** je Stufe `getBrightness(b)` mal 1,4 (`BlockFactor`) in der Farbe
  zwischen `block_light_tint` und Weiss, gemischt mit `0,9 · (2b − 1)²`,
  mal 1,5 (`block`).
- **Wie `lightmap.fsh`, nur getrennt:** dieselbe Kurve `getBrightness` und
  dieselbe Mischung der Farbe wie die Lightmap der Karte, siehe
  [Wasser und Licht](wasser-und-licht.md), „Helligkeit wie im Spiel“. Ohne
  Begrenzung auf 1 und ohne den Schritt zu `notGamma`, denn HDR braucht
  keinen Platz in acht Bit; ohne die Umgebungsfarbe, siehe „Was bleibt eine
  Näherung“.
- **Zwischen zwei Stufen** linear gemischt, wie das Spiel die Lightmap
  gefiltert liest (`Lightmap::linear`).
- **Linear:** Alle Farben, auch `block_light_tint` und
  `sky_light_color`, rechnet Cinematic aus sRGB in lineares Licht um, wie
  der Prototyp aus #89.

## Farbe des Himmels

Das Himmelslicht hat die Farbe des Himmels am Block:

- **Je Biom** aus seinen `attributes`, `minecraft:visual/sky_color` und
  `fog_color`, gelesen wie `EnvironmentAttributeMap.CODEC` in 26.2,
  belegt per javap. Wie ein Biom gelesen wird: [Biomfarben](biomfarben.md),
  „Biome lesen“.
- **Ohne Angabe im Biom** gilt die des Dimensionstyps, ohne Angabe dort die
  Vorgabe aus `EnvironmentAttributes`; die Werte stehen in
  [Dimensionstypen](dimensionstypen.md), „Was der Renderer liest“. In der
  Oberwelt ist das Himmel `#78a7ff` und Nebel `#c0d8ff`.
- **Ein Modifikator** statt einer Farbe lässt die des Dimensionstyps. Der
  Lauf nennt das Biom beim Start.
- **Himmelslicht** = Nebel + (Himmel − Nebel) · 0,75, linear, siehe „Werte
  des Looks“.
- **Gemischt** wie die Biomfarben über die Blöcke im Quadrat mit dem Radius
  aus `--biome-blend`, auf der Höhe des Blocks (`ChunkCache::himmel_at`),
  aber linear und ungerundet. So mischt 0058 auch die Wärme; Kacheln
  bekommen keine Nähte. Ein Test färbt mit einem Biom aus eigener
  `sky_color` (`biom_faerbt_das_himmelslicht`).
- **`water_fog_color`** liest der Renderer schon, Cinematic nutzt sie erst
  mit dem Wasser in #73.

## Sonne

Die Sonne steht fest zur Kamera (`Look::sonne_im_blick`): 48,47° über dem
Horizont, waagrecht von links um 8,75° zur Kamera hin. Links heisst im Blick
diagonal (−1, 0, 1)/√2, genordet (−1, 0, 0); zur Kamera hin (1, 0, 1)/√2
und (0, 0, 1). Aus jeder Richtung steht sie also gleich zum Bild.

- **Licht nach dem Winkel** (`Kino::sonnenlicht`): Farbe mal Stärke 3 mal
  dem Kosinus zwischen Normale und Sonne, abgewandt nichts. Eine Fläche
  ohne `shade` bekommt das Licht einer Fläche nach oben, wie in 0058.
- **Wo:** nur, wo der Dimensionstyp Himmelslicht zeigt, `sky_light_factor`
  über 0; im Nether und im Ende scheint sie nicht.
- **Ohne Schatten der weichen Beleuchtung:** Der Schatten an den Ecken
  dunkelt das Licht des Spiels, nicht die Sonne.

## Zeichnen in HDR

Der dritte Durchgang von `render_area_with` zeichnet mit dem Look in HDR
(`render_hdr_with`, `blit_hdr`), dieselben sichtbaren Pixel wie für die
Karte, siehe [Der Weg einer Kachel](renderpfad.md), „Blit“:

- **Farbe:** erst in den Farben des Bioms wie bei der Karte (`tinted`),
  dann linear mal ihr Licht, dem des Spiels und der Sonne. Liegt das Wasser eines Blocks in einem anderen
  Licht als der Block, bekommt der Anteil des Wassers an der Farbe dessen
  Licht, wie bei der Karte (`tinted_im_licht`).
- **Mischen:** vormultipliziert über den Pixel darunter, wie `over`.
- **Tiefe je Pixel** (`Hdr::tiefe`): die des vordersten gezeichneten Pixels,
  aus der Tiefe des Blockursprungs und der Geometrie des Sprites, ohne
  Pixel −∞; für #73.
- **Ton am Ende** (`Hdr::bild`, `Kino::ton`): die Farbe mal Weissabgleich
  und Belichtung 0,25, dann je Kanal die Kurve aus 0058, dann sRGB. Ein
  Pixel ohne Block bleibt durchsichtig.
- **Weissabgleich:** je Kanal `v` aus `Look::weissabgleich`; er macht eine
  weisse Fläche nach oben in Sonne und Himmel der Oberwelt farblos, auch in
  anderen Dimensionen. Die Wärme nach Biom aus 0058 kommt in #73.
- **Alpha:** Cinematic rundet erst am Ende, die Karte nach jeder Schicht.
  Über Durchscheinendem weicht Alpha deshalb um höchstens eins ab; ein Pixel
  ist genau da, wo die Karte einen hat.
- **Native Stufen und Pyramide** laufen wie bei der Karte, ohne eigenen
  Code. Die Pyramide mittelt die fertigen Kacheln, siehe
  [Zoomstufen](../benutzung/zoomstufen.md), „Verkleinern“.

Getestet: das Goldbild `metatile-cinematic.png`; jeder Ausschnitt gleicht
dem grossen Bild, zweimal gleich; Pyramide und native Stufen über einem Baum
mit Cinematic in `renderer/tests/cli.rs`.

## Was bleibt eine Näherung

- **Keine Umgebungsfarbe:** Die Lightmap des Spiels legt
  `ambient_light_color` auf jedes Licht, in der Oberwelt `#0a0a0a`.
  Cinematic lässt sie weg, wie der Prototyp, an dem 0058 abgestimmt ist.
  Ohne jedes Licht ist ein Pixel schwarz statt fast schwarz.
- **Ohne Begrenzung und `notGamma`:** Die Lightmap des Spiels hält jedes
  Licht unter 1. In HDR wird es heller, die Kurve fängt es auf.
- **Die Farbe des Himmels je Block:** Das Spiel nimmt sie an der Kamera,
  räumlich gemischt um den Spieler. Eine Karte hat keine Kamera in der
  Welt; der Block steht für sie.
- **Wasser im eigenen Licht nach seinem Anteil an der Farbe:** Der Anteil
  kommt aus der Tönungskarte in sRGB, wie bei der Karte. Das Spiel mischt
  das Wasser als eigene Fläche über das Modell.
