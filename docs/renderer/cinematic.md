---
title: Cinematic
description: Wie --cinematic dieselbe Karte im Licht des Spiels in HDR zeichnet - Sprites ohne Schattierung nach Richtung, das Licht der Lightmap je Ecke aus Umgebung, Himmels- und Blocklicht, die Farbe des Himmels je Biom, die Sonne mit hartem Schatten aus einem Strahl je Texel, Bodenpflanzen, die nur dämpfen, Wasser, Weissabgleich, Belichtung und Kurve aus 0058.
code:
  - renderer/src/render/look.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/projection.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sonne.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tint.rs
  - renderer/src/assets/colors.rs
  - renderer/src/assets/dimension.rs
  - renderer/src/assets/mod.rs
  - renderer/src/cli.rs
---

# Cinematic

`--cinematic` zeichnet mit `--tiles` oder `--render` dieselbe Karte im
Licht des Spiels in HDR, mit Weissabgleich, Belichtung und Kurve nach
[0058](../entscheidungen/0058-look-von-cinematic.md). Kacheln landen in
einem eigenen Baum samt nativen Stufen und Pyramide, siehe
[`map.json`](../benutzung/map-json.md), „Liste der Bäume“. Cinematic nimmt
dieselben Kandidaten, dieselbe Deckungsmaske und dieselben Draws wie die
Karte, also dasselbe Pixelraster bei jeder Kamera, Richtung und jedem
scale; anders ist nur das Licht je Pixel, entschieden in
[0053](../entscheidungen/0053-cinematic-als-schalter-der-karte.md). Es
zeichnet immer die CPU. Phase 1 (#72) brachte das Licht des Spiels, Phase 2
(#73) Sonne, Schatten, Bodenpflanzen, Wasser, Leuchten, Wärme nach Biom und
Bloom.

## Werte des Looks

Alle Werte stehen benannt an einer Stelle, `LOOK` in
[`renderer/src/render/look.rs`](../../renderer/src/render/look.rs):

| Wert | in `Look` | genutzt |
|---|---|---|
| Stärke des Himmelslichts | `himmel`: 3 | ja |
| Anteil der Farbe des Himmels am Himmelslicht, der Rest ist Nebel | `himmel_anteil`: 0,75 | ja |
| Stärke des Blocklichts | `block`: 1,5 | ja |
| Leuchten: Stärke, ab und bis zu welcher Helligkeit eines Texels | `leuchten`: 2, `leuchten_ab`: 0,25, `leuchten_voll`: 0,75 | ja |
| Sonne: Stärke, Farbe linear, Höhe, waagrecht von links zur Kamera hin | `sonne`: 3, `sonne_farbe`: (1; 0,93; 0,83), `sonne_hoehe`: 48,47°, `sonne_seite`: 8,75° | ja |
| Wie weit ein Strahl zur Sonne reicht, in Blöcken entlang des Strahls | `sonne_weite`: 128 | ja |
| So viel Sonne lässt eine Bodenpflanze durch | `pflanzen`: 0,5 | ja |
| Wasser: F0 der Spiegelung, Anteil der Deckkraft seiner Textur, Dichte | `wasser_spiegel`: 0,04, `wasser_textur`: 0,6, `wasser_dichte`: 8 | ja |
| Wärme: so viel stärker wird der Weissabgleich höchstens, ab und bis zu welcher Temperatur | `waerme`: 0,5, `waerme_von`: 0,5, `waerme_bis`: 1,0 | ja |
| Belichtung | `belichtung`: 0,25 | ja |
| Kurve: gerade bis, flach ab | `knie`: 0,8, `flach`: 1,2 | ja |
| Bloom: Stärke, σ in Blöcken | `bloom`: 1, `bloom_breite`: 0,25 | ja |

- **Herkunft:** alle aus 0058, bis auf `sonne_weite`, `wasser_textur`,
  `leuchten_ab` und `leuchten_voll`. 0058 sagt „nur die hellen Texel“; ab
  0,25 und ganz ab 0,75 im hellsten Kanal leuchteten sie im Prototyp, an
  dem 0058 abgestimmt ist.
  Bis 128 Blöcke weit reichte der Strahl zur Sonne im Prototyp, an dem 0056
  den Preis gemessen hat, siehe
  [Gang zur Sonne in Stufen](../messungen/2026-10-02-gang-zur-sonne-in-stufen.md).
  Mit 0,6 ihrer Deckkraft deckte die Textur des Wassers im Prototyp, in
  allen Bildern, an denen 0058 abgestimmt ist.
- **Fingerabdruck:** Jeder Baum mit Cinematic hält die Werte und den Stand
  des Verfahrens als `lookHash` in `map.json`. Wie er gerechnet wird, wann
  er sich ändert und wann ein Lauf deshalb abbricht, steht in
  [`map.json`](../benutzung/map-json.md), „Look“.
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
  ihnen, siehe „Sonne“. Das prüft `geometrie_der_vorderen_flaeche` in
  `rasterizer.rs` an zwei Flächen eines Modells, die sich decken.
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

Aus den drei Werten rechnet Cinematic das Licht je Kanal (`Kino::licht` in
[`renderer/src/render/kino.rs`](../../renderer/src/render/kino.rs)), wie
das Spiel an jeder Ecke einer Seite, einmal je Draw (`EckenLicht` in
`metatile.rs`). Ein Pixel auf einer Seite der AO-Karte mischt das Licht
ihrer vier Ecken mit denselben Anteilen wie die Karte, ungerundet; ein
Pixel ohne Seite bekommt das Licht des Blocks. Über eine Fläche verläuft so
das fertige Licht wie im Spiel, denn `terrain.vsh` liest die Lightmap je
Ecke (`sample_lightmap`). Das prüft `licht_zwischen_den_ecken_wie_im_spiel`
in `metatile.rs`.

`Licht = Schatten / 255 · (Umgebung + Himmelslicht des Bioms · H(Himmel) + B(Block))`

- **Umgebung:** `ambient_light_color` des Dimensionstyps, die
  `lightmap.fsh` unter jedes Licht legt, roh wie dort und ohne Stärke aus
  dem Look. Im Nether und im Ende ist `sky_light_factor` 0; ohne Blocklicht
  ist sie dort das ganze Licht.
- **H:** je Stufe `getBrightness` mal `sky_light_factor` und
  `sky_light_color` der Dimension, mal `himmel`.
- **B:** je Stufe `getBrightness` mit `BlockFactor` in der Farbe
  `BlockLightColor`, mal `block`, beides wie in
  [Wasser und Licht](wasser-und-licht.md), „Blocklicht“. Die Farbe mischt
  Cinematic wie das Spiel aus den rohen Werten von `block_light_tint`;
  linear wird erst das Ergebnis.
- **Wie `lightmap.fsh`, nur getrennt:** dieselbe Kurve `getBrightness` wie
  die Lightmap der Karte, siehe [Wasser und Licht](wasser-und-licht.md),
  „Helligkeit wie im Spiel“. Ohne Begrenzung auf 1, denn die Kurve aus 0058
  fängt helles Licht auf. Ohne den Schritt zu `notGamma`: Er ist die
  Helligkeit aus den Optionen des Spiels (`BrightnessFactor`). Wie hell
  Cinematic zeichnet, legen Belichtung und Kurve fest, abgestimmt am
  Prototyp aus #89, der ihn nicht hat.
- **Zwischen zwei Stufen** linear gemischt, wie das Spiel die Lightmap
  gefiltert liest (`Lightmap::linear`).
- **Linear:** Die Farben des Lichts rechnet Cinematic aus sRGB in lineares
  Licht um, wie der Prototyp: `sky_light_color`, die Farben des Himmels
  und die fertig gemischte Farbe des Blocklichts. Nur die Umgebung bleibt
  roh, siehe „Was bleibt eine Näherung“.

## Farbe des Himmels

Das Himmelslicht hat die Farbe des Himmels am Block, eine Wahl aus 0053
und 0058; was das Spiel tut, steht unter „Was bleibt eine Näherung“.

- **Je Biom** aus seinen `attributes`, `minecraft:visual/sky_color` und
  `fog_color`, gelesen wie `EnvironmentAttributeMap.CODEC` in 26.2,
  belegt per javap. Wie ein Biom gelesen wird: [Biomfarben](biomfarben.md),
  „Biome lesen“.
- **Ohne Angabe im Biom** gilt die des Dimensionstyps, ohne Angabe dort die
  Vorgabe aus `EnvironmentAttributes`; die Werte stehen in
  [Dimensionstypen](dimensionstypen.md), „Was der Renderer liest“.
- **Ein Modifikator** statt einer Farbe lässt die des Dimensionstyps. Ein
  Lauf mit `--cinematic` nennt das Biom beim Start.
- **Himmelslicht:** Nebel und Himmel linear gemischt, der Himmel zum Anteil
  `himmel_anteil`, siehe „Werte des Looks“.
- **Gemischt** wie die Biomfarben über die Blöcke im Quadrat mit dem Radius
  aus `--biome-blend`, auf der Höhe des Blocks (`ChunkCache::himmel_at`),
  aber linear und ungerundet. So mischt 0058 auch die Wärme; Kacheln
  bekommen keine Nähte. Ein Test färbt mit Biomen aus eigener `sky_color`,
  mit dem Radius 2 aus `nw`, an der Grenze das Mittel der 25 Blöcke
  (`biom_faerbt_das_himmelslicht`).
- **Für das Wasser** dazu Himmel und Nebel getrennt und
  `water_fog_color`, ebenso gemischt (`Himmelsfarben`), siehe „Wasser“.

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
- **So weit sie durchkommt:** mal dem, was der Strahl zur Sonne von ihr
  übrig lässt, siehe „Schatten“.

## Schatten

Je Pixel, auf den die Sonne scheint, geht ein Strahl zur Sonne, entschieden
in [0056](../entscheidungen/0056-exakter-strahl-zur-sonne.md). Er gibt 0
hinter einer deckenden Stelle, sonst 1, je Bodenpflanze auf dem Weg mal
0,5 (`ChunkCache::sonne` in
[`renderer/src/render/metatile/strahl.rs`](../../renderer/src/render/metatile/strahl.rs)).

- **Wo er beginnt** (`startpunkt` in
  [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)):
  am Punkt der vordersten Fläche im Pixel, aus dem Bildpunkt und der Tiefe
  des Sprites zurückgerechnet (`Projection::punkt`), ein Tausendstel vor
  der Fläche. Auf einer achsparallelen Fläche liegt er in der Mitte seines
  Sechzehntels Block (`texel_mitte`): Alle Pixel auf einem Texel bekommen
  denselben Strahl, er wird einmal gerechnet. Auf schrägen Flächen je
  Pixel.
- **Was deckt** (`Sonnenform` in
  [`renderer/src/render/sonne.rs`](../../renderer/src/render/sonne.rs)):
  je Alternative die Dreiecke ihres Modells im Blick, mit dem Alpha-Test
  ihrer Schicht. Ausgeschnitten deckt ein Texel ab der Schwelle des
  Alpha-Tests, gemischt nur mit Alpha 255 (`sonnenschwelle`). Laub deckt
  also nach seinen Löchern, Glas nur, wo seine Texel ganz decken.
- **Was durchlässt:** Flächen aus Wasser fehlen ganz, Wasser hält die
  Sonne nie auf. Ein Block ohne deckenden Texel ebenso.
- **Lava** reicht unter derselben bis zur Kante, und ihre Flächen entfallen
  zu derselben und vor einer vollen Seite, wie
  `LiquidBlockRenderer.shouldRenderFace`.
- **Modelle, die aus ihrem Würfel ragen,** prüft er in jedem Würfel, in den
  ihre Hülle reicht (`Sonnenform::zellen`), einmal je Strahl.
- **Wie weit:** bis `sonne_weite`, 128 Blöcke entlang des Strahls.
- **Der Test einer Zelle:** erst gegen die Hülle des Modells, dann Dreieck
  für Dreieck, beidseitig; der erste deckende Treffer genügt. Gerechnet
  relativ zum Block, also gleich, in welcher Zelle er geprüft wird.

### Der schnelle Gang

`ChunkCache::sonne_gang` geht den Strahl Zelle für Zelle durch das Gitter
im Blick, springt aber über, was nichts aufhält:

- **Je Chunk eine Säule** (`Saeule`), sobald ein Strahl ihn betritt: ihre
  Decke, die oberste Zelle mit Block oder hineinragendem Modell. Darüber
  springt der Strahl zum Rand des Chunks. Dafür lädt der Chunk-Cache die
  Chunks rundum, aus denen Modelle hineinragen können; das Band wächst
  nicht im Voraus.
- **Je Section Bits** (`Bits`), sobald ein Strahl sie betritt, einmal je
  Section: die Zellen mit Arbeit, die vollen deckenden Würfel und die
  Zellen, in die ein Modell eines Nachbarn ragt; dazu je Würfel aus
  4 × 4 × 4 Zellen ein Bit. Durch eine Section und einen Würfel ohne Arbeit
  springt er hinaus.
- **Ein voller deckender Würfel** (`Sonnenform::wuerfel`: alle sechs Seiten
  ganz von einer deckenden Fläche belegt) hält ihn ohne Test auf.
- **Gleich dem Bezug:** `ChunkCache::sonne_bezug` prüft jede Zelle bis zur
  Weite mit jedem Block, dessen Modell hineinragen kann. Beide geben
  dasselbe, denn ein Block, den der Strahl nicht trifft, ändert nichts, ob
  er geprüft wird oder nicht. Das prüft `schneller_gang_gleicht_dem_bezug`
  in `renderer/tests/metatile.rs` Bit für Bit am HDR-Puffer, an Szenen mit
  Wasser, Lava, Laub, Glas und Modellen, die aus ihrem Würfel ragen.

Getestet: einzelne Strahlen durch Würfel, Laub, Wasser, Glas, Pflanze und
Überhang (`strahlen_zur_sonne`), die Lage des Schattens eines Würfels im
Bild (`wuerfel_wirft_seinen_schatten`).

## Wasser

Ein Pixel, dessen vorderstes Fragment Wasser ist (`Geometrie::wasser`, sein
Alpha), mischt sich wie im Prototyp aus #89 von vorn nach hinten
(`mische_wasser` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)):

1. **Spiegelung:** der Anteil nach Fresnel (Schlick, F0 0,04) aus Blick und
   Normale, mit dem Himmel in der gespiegelten Richtung, zum Horizont hin in
   der Farbe des Nebels (`Kino::spiegel`). Aus 2:1 sind das von oben rund
   5 %.
2. **Textur:** Vom Rest deckt die Textur des Wassers mit 0,6 ihres Alphas,
   im Licht des Wassers und der Sonne.
3. **Im selben Sprite dahinter,** etwa ein gefluteter Block an der
   Oberfläche, folgt ohne Strecke in seinem Licht.
4. **Darunter** dämpft das Wasser den Pixel darunter je Kanal nach der
   Strecke bis zu ihm, `exp(−σ · Strecke)`, und füllt mit `water_fog_color`
   im Himmelslicht. σ kommt aus der Farbe des Wassers: Kanäle, die sie
   schwächer trägt, dämpft es stärker, geteilt durch die Dichte 8
   (`Kino::wasser_dichte`). Tieferes Wasser ist so dunkler und blauer.

- **Die Strecke** kommt aus der Tiefe je Pixel (`Hdr::tiefe`): die des
  Wassers weniger die des vordersten Pixels darunter, geteilt durch die
  Länge der Blickachse.
- **Wasser und Rest** trennt die Tönungskarte, wie für das Licht des
  Wassers unter „Zeichnen in HDR“.
- **Spiegelung und Streulicht** liegen im Himmelslicht des Wassers: In
  einer Höhle spiegelt Wasser keinen hellen Himmel.
- **Im Licht des Blocks,** ohne die Ecken der weichen Beleuchtung: Wasser
  hat keine Seite in der AO-Karte, denn das Spiel zeichnet Flüssigkeiten
  ohne sie, siehe [Weiche Beleuchtung](weiche-beleuchtung.md).
- **Alpha** wie bei der Karte; das Streulicht füllt nur, wo darunter etwas
  deckt. Über leerem Grund, etwa am Rand der geladenen Chunks, gibt es
  keine Strecke: Dort mischt Wasser wie jede andere Fläche.
- Getestet: `tieferes_wasser_ist_dunkler` in `renderer/tests/metatile.rs`,
  `wasser_spiegelt_und_dampft` in `renderer/src/render/kino.rs`.

## Bodenpflanzen

Eine Bodenpflanze dämpft den Strahl zur Sonne auf `pflanzen`, 0,5, einmal
je Block, statt ihn zu decken, wie in 0058:

- **Welche:** wessen Modelle jeder Alternative über ihre `parent`-Kette von
  einer Vorlage des Spiels erben (`Assets::bodenpflanze` in
  [`renderer/src/assets/mod.rs`](../../renderer/src/assets/mod.rs)): dem
  Kreuz (`block/cross`, `block/tinted_cross`, `block/cross_emissive`), den
  Ebenen der Feldfrüchte (`block/crop`), den Blütenteppichen
  (`block/flowerbed_*`), dem Laub am Boden (`block/template_leaf_litter_*`)
  und der Vorlage des Seegrases (`block/template_seagrass`). In 26.2 sind
  das 85 Blöcke, gezählt an den Modellen des Client: 84 ganz, dazu die
  untere Hälfte der Sonnenblume; ihre Blüte oben hat ein eigenes Modell.
- **Nicht** dämpft die Pflanze, auf der der Strahl beginnt, und von ihr
  aus der Block darüber, wenn er ihre obere Hälfte ist
  (`Family::obere_haelfte`).
- **Licht:** Flächen ohne `shade` bekommen das Licht einer Fläche nach
  oben, siehe „Sonne“.
- Getestet: `bodenpflanze_nach_der_vorlage` in `renderer/tests/assets.rs`
  und die Pflanze in `strahlen_zur_sonne`.

## Leuchten

Ein Block, der selbst leuchtet, bringt sein Leuchten zu jedem Pixel, wie im
Prototyp aus #89 (`mische_hdr` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)):

- **Wie viel:** die Farbe linear mal `leuchten`, mal seiner Stufe
  `getLightEmission` / 15 (`leuchten.txt`, siehe
  [Erzeugte Tabellen](../entwicklung/tabellen.md)), mal wie stark der Texel
  leuchtet. Es kommt zum Licht dazu, wie Sonne und Himmel.
- **Nur helle Texel** (`Look::leuchtet`): nach dem hellsten Kanal der
  Farbe, linear, 0 bis `leuchten_ab`, dann weich bis 1 bei `leuchten_voll`.
  Eine Laterne leuchtet so in ihrem Licht, nicht in ihrem Gestell.
- **Unter Wasser** leuchtet ein gefluteter Block im selben Sprite ebenso.
- Getestet: `leuchten_nach_der_helligkeit` in `look.rs`,
  `nur_helle_texel_leuchten` in `renderer/tests/metatile.rs` an einem Block
  mit halb heller, halb dunkler Textur.

## Wärme

Der Weissabgleich wird je Pixel nach der Temperatur des Bioms stärker, wie
in 0058 (`Look::waerme`, `Kino::ton`):

- **Je Kanal** `1 + (v − 1) · w`, mit `v` aus „Zeichnen in HDR“, und `w`
  von 1 bis `waerme_von`, dann gerade bis 1 + `waerme` bei `waerme_bis`.
  Was das je Biom heisst, steht in 0058, „Weissabgleich im Einzelnen“.
- **Die Temperatur** ist `temperature` des Bioms
  (`BiomeColors::temperatur`), ohne Definition die von `plains`, 0,8.
  Gemischt wird sie wie die Farben des Himmels (`Himmelsfarben`), erst aus
  dem Mittel kommt `w`.
- **Je Pixel** gilt die Wärme des vordersten gezeichneten Pixels, aus dem
  Biom seines Blocks (`Hdr::waerme`).
- Getestet: `waerme_nach_der_temperatur` in `look.rs`, `ton_mit_waerme` in
  `kino.rs` und die Wärme je Pixel in `biom_faerbt_das_himmelslicht`, an
  der Grenze aus dem Mittel der Temperatur.

## Bloom

Aus dem Leuchten kommt ein Schein, wie in 0058 (`Kino::bloom`, `Hdr::bild`
in [`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)):

- **Die Quelle** ist das Leuchten je Pixel (`Hdr::leuchten`), linear,
  vormultipliziert und gemischt wie die Farbe. Wasser dämpft es wie die
  Farbe darunter. Je Pixel abgeglichen mit dessen Wärme, mal `bloom`.
- **Unscharf** mit drei Kastenfiltern je Achse, erst senkrecht, dann
  waagrecht, nahe an einer Gaussglocke mit σ = `bloom_breite` · scale, wie
  im Prototyp aus #89 (`unscharf` in `kino.rs`): Breite √(4σ² + 1),
  gerundet und ungerade, Radius r die Hälfte davon, abgerundet. Bei scale
  32 ist r = 8.
- **Dazu** vor Belichtung und Kurve, so abgeglichen wie die Farbe. Auf
  einen Pixel ohne Block fällt kein Schein, er bleibt durchsichtig.
- **Ohne Nähte:** `render_area_with` rendert um jede Kachel einen Rand von
  3r Pixeln mit, so weit reichen die drei Filter; bei scale 32 sind das 24
  Pixel. Im Rand rechnet er keine Strahlen zur Sonne, dort zählt nur das
  Leuchten. Die Filter rechnen in Festkomma mit 24 Bit nach dem Komma: Die
  gleitende Summe ist exakt, und ein Ausschnitt gibt dieselben Bits wie das
  grosse Bild.
- Getestet: `unscharf_wie_im_prototyp` mit Sollwerten aus der
  Nachbearbeitung des Prototyps zu #89, `unscharf_im_ausschnitt_gleich` und
  `bloom_radius_nach_dem_scale` in `kino.rs`; `bloom_um_das_leuchten` und
  `wasser_daempft_das_leuchten` in `renderer/tests/metatile.rs`, dazu
  `kleine_ausschnitte_gleichen_dem_grossen_bild`.

## Zeichnen in HDR

Der dritte Durchgang von `render_area_with` zeichnet mit dem Look in HDR
(`render_hdr_with`, `blit_hdr`), dieselben sichtbaren Pixel wie für die
Karte, siehe [Der Weg einer Kachel](renderpfad.md), „Blit“:

- **Farbe:** erst in den Farben des Bioms wie bei der Karte (`tinted`),
  dann linear mal ihr Licht, dem des Spiels und der Sonne. Liegt das Wasser
  eines Blocks in einem anderen Licht als der Block, bekommt der Anteil des
  Wassers an der Farbe dessen Licht, wie bei der Karte (`tinted_im_licht`).
- **Mischen:** vormultipliziert über den Pixel darunter, wie `over`.
- **Tiefe je Pixel** (`Hdr::tiefe`): die des vordersten gezeichneten Pixels,
  aus der Tiefe des Blockursprungs und der Geometrie des Sprites, ohne
  Pixel −∞; für das Wasser. Das prüfen
  `hdr_haelt_die_tiefe_der_vordersten_flaeche` und, mit zwei Draws auf
  einem Pixel, `hdr_haelt_die_tiefe_des_vorderen_draws`.
- **Ton am Ende** (`Hdr::bild`, `Kino::ton`): die Farbe mal Weissabgleich,
  dazu der Bloom, mal Belichtung, dann je Kanal die Kurve aus 0058, dann
  sRGB. Ein Pixel ohne Block bleibt durchsichtig.
- **Weissabgleich:** je Kanal `v` aus `Look::weissabgleich`; er macht eine
  weisse Fläche nach oben in Sonne und Himmel der Oberwelt farblos, auch in
  anderen Dimensionen. Je Pixel verstärkt ihn die Wärme, siehe „Wärme“.
- **Alpha:** Cinematic rundet erst am Ende, die Karte nach jeder Schicht.
  Über Durchscheinendem weicht Alpha deshalb um höchstens eins ab; ein Pixel
  ist genau da, wo die Karte einen hat.
- **Native Stufen und Pyramide** laufen wie bei der Karte, ohne eigenen
  Code. Die Pyramide mittelt die fertigen Kacheln, siehe
  [Zoomstufen](../benutzung/zoomstufen.md), „Verkleinern“.

Getestet: das Goldbild `metatile-cinematic.png`; jeder Ausschnitt gleicht
dem grossen Bild, zweimal gleich; Pyramide und native Stufen über einem Baum
mit Cinematic in `renderer/tests/cli.rs`. Was ein Baum mit Cinematic
kostet, steht in [Was ein Lauf kostet](../benutzung/kosten.md),
„Cinematic“.

## Was bleibt eine Näherung

- **Die Farbe des Himmels:** Das Spiel beleuchtet Blöcke nie in ihr. Die
  Lightmap nimmt `sky_light_color`; `sky_color` und `fog_color` nimmt das
  Spiel nur für die Kuppel des Himmels und den Nebel, gemischt um die
  Kamera. Cinematic färbt das Himmelslicht mit ihnen, eine Wahl aus 0053
  und 0058, und nimmt sie am Block, denn eine Karte hat keine Kamera in der
  Welt.
- **Die Umgebung roh:** Jede andere Farbe des Lichts rechnet Cinematic
  linear, `ambient_light_color` nimmt es roh, wie `lightmap.fsh` sie
  addiert. Linear wäre sie im Nether rund ein Siebtel so hell, und Nether
  und Ende lägen ohne Blocklicht fast schwarz. Gerechnet für eine weisse
  Textur ohne Himmels- und Blocklicht zeigt Cinematic in der Oberwelt 0,08
  bis 0,11 von ihr, das Spiel 0,09. Im Nether sind es 0,16 bis 0,26 und
  im Ende 0,23 bis 0,29, im Spiel mit der Helligkeit 0,5 dort 0,26 bis 0,38
  und 0,45 bis 0,50.
- **Ohne Begrenzung und `notGamma`:** Die Lightmap des Spiels begrenzt
  jedes Licht auf höchstens 1 und hebt es nach der Helligkeit aus den
  Optionen. Cinematic lässt beides weg, siehe „Licht in HDR“.
- **`getBrightness` auf linearem Licht:** Das Spiel multipliziert die
  Lightmap mit der Textur in sRGB, Cinematic mit der Farbe in linearem
  Licht, wie der Prototyp. Unterschiede im Licht wirken so flacher: Halbes
  Licht zeigt Cinematic mit rund 0,7 der Helligkeit, das Spiel mit 0,5.
- **Wasser im eigenen Licht nach seinem Anteil an der Farbe:** Der Anteil
  kommt aus der Tönungskarte in sRGB, wie bei der Karte. Das Spiel mischt
  das Wasser als eigene Fläche über das Modell.
- **Wasser nur bis zum nächsten Pixel:** Die Strecke reicht bis zum
  vordersten Pixel darunter, auch wenn dazwischen Luft liegt, etwa hinter
  einer Wassersäule. Der Prototyp verliess das Wasser an seiner Rückseite;
  die zeichnet der Rasterizer nicht.
- **Kein Schein über leerem Grund:** Auf Pixel ohne Block, etwa am Rand
  der Welt, fällt kein Bloom; sie bleiben durchsichtig, wie bei der Karte.
- **Schatten bis 128 Blöcke:** Ein Block, der weiter entlang des Strahls
  steht, also gut 95 Blöcke höher, wirft keinen Schatten mehr. Das Spiel
  hat keine Schatten der Sonne; der Prototyp, an dem 0056 den Preis
  gemessen hat, reichte so weit.
