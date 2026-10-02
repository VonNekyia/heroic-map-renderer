---
title: "0053: Cinematic als Schalter der Karte"
description: Warum Cinematic kein eigenes Backend bekommt, sondern im Raster der Karte zeichnet, mit einem Strahl zur Sonne je Texel, als eigener Kachelbaum mit derselben Kamera, und warum Showcase und die Strahlen zum Himmel wegfallen.
status: gilt
date: 2026-10-02
issues: [72, 73]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
---

# 0053: Cinematic als Schalter der Karte

[0054](0054-baeume-unter-einer-wurzel.md) löst einen Teil von „Schalter
`--cinematic`“ ab: Ein Lauf mit dem anderen `look` schreibt in den Ordner
mit oder ohne den Anhang `-cinematic` und bricht nur ab, wenn in diesem
schon ein Baum mit dem anderen steht.

[0056](0056-exakter-strahl-zur-sonne.md) löst drei Stellen ab:

- das Ziel von höchstens 0,5 µs je Strahl zur Sonne unter „Entscheidung“;
- die Schattenkarte als Ausweg unter „Verworfene Alternativen“;
- die Kosten eines Cinematic-Baums unter „Folgen“, gerechnet mit diesem
  Ziel.

Der exakte Strahl je Texel bleibt, bei rund 0,7 bis 0,9 µs. Was ein
Cinematic-Baum damit kostet, steht in 0056.

## Anlass

[0046](0046-drei-renderarten.md) plante drei Renderarten mit zwei
Backends: die Karte im Raster, Cinematic und Showcase als Einzelbilder aus
einem neuen Strahlen-Backend. Am 01.10. spät hat der User neu entschieden:

- Es bleibt nur Cinematic mit dem Licht des Spiels, wie in der Vorschau.
  Showcase und die Strahlen zum Himmel fallen weg.
- Cinematic ist ein Schalter der Karte: dieselbe Karte mit oder ohne
  Cinematic, Kacheln samt Pyramide. `--render` kann es auch.
- Jede Kamera, Richtung und jeder scale, den die Karte annimmt, geht auch
  mit Cinematic, im selben Pixelraster.
- Bloom bleibt.
- Erst die Kamera (#67, #68), dann #64, dann Cinematic.

Mit Strahlen aus der Kamera kostete schon die Basis der grossen Welt bei
scale 32 rund einen bis drei Tage statt 44 min, hochgerechnet aus
[Cinematic, Zeit je Bild](../messungen/2026-10-01-cinematic-zeit.md).

## Entscheidung

- **Cinematic zeichnet im Raster der Karte.** Dieselben Kandidaten,
  dieselbe Deckungsmaske, dieselben Draws, also dasselbe Pixelraster. Nur
  das Licht je sichtbarem Pixel ist ein anderes. Es gibt kein
  Strahlen-Backend und keine Strahlen aus der Kamera.
- **Was ein Pixel bekommt:**
  - das Licht des Spiels, Himmels- und Blocklicht getrennt über die Ecken
    der weichen Beleuchtung, nach der Kurve der Lightmap, das Himmelslicht
    in der Farbe des Himmels aus den Attributen des Bioms, das Blocklicht
    in `BlockLightTint`;
  - die Sonne nach dem Winkel zur Fläche, mit hartem Schatten;
  - das Leuchten leuchtender Blöcke, nur ihrer hellen Texel;
  - auf Wasser die Spiegelung des Himmels nach Fresnel und darunter die
    Farbe nach der Strecke bis zum Grund;
  - Nebel nach der Tiefe unter einer festen Höhe;
  - Bloom aus dem, was leuchtet, über einer Schwelle, vor der filmischen
    Kurve;
  - eine feste Belichtung, ein fester Weissabgleich und die filmische
    Kurve.
- **Die Sprites für Cinematic** tragen je Pixel die Tiefe entlang der
  Blickachse, die Normale und die Farbe ohne die Schattierung nach
  Richtung. Gebacken werden sie nur mit dem Schalter.
- **Ein Strahl zur Sonne je sichtbarem Texel,** unter scale 32 je Pixel.
  Auf achsparallelen Flächen beginnt er in der Mitte des Texels, auf
  1/16 Block gerundet; schräge Flächen bleiben je Pixel. Er geht durchs
  Blockgitter über die Bitmasken der Sections und durch den Chunk-Cache.
  Volle Würfel treffen ohne Flächentest, reines Wasser lässt die Sonne
  durch, die übrigen Zellen prüfen die Flächen ihres Modells mit
  Alpha-Test. Das ist exakt: kein Raster für Schatten, kein Bias, kein
  Rauschen.
- **Die Sonne steht fest zur Kamera:** von links, rund 48° hoch, wie in der
  Vorschau. Sie scheint, wo der Dimensionstyp Himmelslicht zeigt.
- **Nähte:** Was ein Pixel bekommt, hängt nur an der Welt und an seinem
  Platz im Pixelraster. Bloom rechnet mit einem Rand um jede Kachel.
- **Schalter `--cinematic`.** Den Namen entscheidet der User. Ein
  Kachelbaum ist mit oder ohne Cinematic: `map.json` hält es als `look`
  fest, und ein Lauf mit dem anderen bricht ab, bevor er einen Chunk liest.
- **Auf der CPU.** Die Grafikkarte zeichnet weiter nur die Karte.
- **Die Karte bleibt Byte für Byte gleich und wird nicht langsamer:**
  Cinematic hat einen eigenen Weg im Blit und eigene Sprites, nur mit dem
  Schalter.

Belegt am Prototyp, an Ausschnitten der Testwelt: Der schnelle Gang zur
Sonne gibt Pixel für Pixel dasselbe Bild wie der langsame. Ein Strahl
kostet dort heute 1,3 bis 2,7 µs je Thread; das Ziel für die Umsetzung ist
höchstens 0,5 µs. Kacheln in Cinematic wiegen verlustfrei das 0,83- bis
1,27-Fache der Karte. Je Texel statt je Pixel ändert 0,9 bis 2,7 % der
Pixel, einzelne im Laub und an Schattenkanten. Siehe
[Strahl zur Sonne und Grösse der Kacheln für Cinematic](../messungen/2026-10-02-strahl-zur-sonne.md).

## Was sie ablöst

| Entscheidung | Abgelöst | Neu |
|---|---|---|
| [0046](0046-drei-renderarten.md) | drei Renderarten, zwei Backends, ein Kern; Cinematic und Showcase als Einzelbilder aus Strahlen | eine Karte mit oder ohne Cinematic, im Raster; Showcase entfällt |

Regel 22 in `AGENTS.md` fragt danach nach der Karte mit oder ohne
Cinematic.

### Was 0046 gegen Cinematic im Raster sagte

0046 hatte Cinematic im Raster verworfen. Ihre Gründe tragen nicht mehr:

| Grund in 0046 | Warum er nicht mehr trägt |
|---|---|
| Das Raster kennt keine Szene; Schatten fragen, was in einer anderen Richtung liegt. | Der Strahl zur Sonne fragt die Welt über denselben Chunk-Cache, dieselben Bitmasken und dieselben Modelle, die das Raster schon hält. Das Raster liefert nur den sichtbaren Punkt. |
| Im Bildraum endet jede Wirkung an der Kachelkante und an allem, was die Kamera nicht sieht. | Der Strahl zur Sonne geht durch die Welt, nicht durch das Bild. Er trifft auch Schattenwerfer ausserhalb von Kachel und Band und solche, die die Kamera nicht sieht. Bloom rechnet einen Rand um jede Kachel mit. |
| Eine Schattenkarte für eine Welt aus hunderttausenden Kacheln wäre ein eigenes Raster neben dem ersten. | Es gibt keine Schattenkarte, sondern einen Strahl je Texel. |
| Jede Wirkung säße in den heissen Schleifen der Karte und gefährdete ihr Tempo und ihr Goldbild. | Cinematic hat einen eigenen Weg im Blit und eigene Sprites, nur mit dem Schalter. Die Karte bleibt Byte für Byte gleich und wird nicht langsamer; das prüfen ihr Goldbild und eine Messung in Phase 1. |

Dazu hat sich das Ziel geändert: 0046 wollte Einzelbilder in wenigen
Sekunden, und dafür reichten Strahlen aus der Kamera. Kacheln für eine
ganze Welt verlangen das Raster, denn mit Strahlen aus der Kamera kosteten
sie Tage.

## Verworfene Alternativen

- **Strahlen aus der Kamera (0046).** Sie allein kosten am Prototyp das
  3,5- bis 6,6-Fache des ganzen Rasters. Sie träfen dieselben Flächen wie
  das Raster, an 99,91 bis 99,99 % der Pixel Byte für Byte, siehe
  [Strahlen durch die Blockwelt](../messungen/2026-10-01-strahlen-prototyp.md).
- **Eine Schattenkarte statt eines Strahls je Texel.** Ein Gitter in der
  Welt, je Feld die Höhe des ersten Treffers von der Sonne her. Bei 1/16
  Block spart es bei scale 32 keine Strahlen. Bei 1/8 Block spart es die
  Hälfte bis drei Viertel, aber die Kanten springen in Stufen von 4 Pixeln,
  nicht im Raster der Texturen. Dazu kommen ein Bias gegen Selbstschatten
  und 2 bis 3 Tage Arbeit. Sie kommt nur, wenn der Strahl je Texel sein
  Ziel von 0,5 µs verfehlt.
- **Strahlen zum Himmel und Showcase:** vom User verworfen.
- **Bloom aus jedem hellen Pixel.** Dann blühte auch Schnee oder Sand in
  der Sonne, je nach Belichtung, und der Rand jeder Kachel bräuchte Strahlen
  zur Sonne. Bloom aus dem, was leuchtet, braucht am Rand nur das Raster.
- **Karte und Cinematic in einem Lauf,** beide Bäume auf einmal.
  Kandidaten, Deckung, Sprite-Wahl, Chunks und Licht teilten sich beide;
  das spart gerechnet 25 bis 30 min je Paar auf der grossen Welt. Es
  braucht aber zwei Ausgaben und zwei Pyramiden in einem Lauf. Erst, wenn
  beide Bäume regelmässig entstehen.
- **Die Grafikkarte:** Seit der Deckungsmaske zeichnet die CPU ohne Karte
  etwa so schnell wie mit, siehe [Grafikkarte](../benutzung/grafikkarte.md).
  Den Schatten müsste der Shader mit der Welt rechnen, die heute nur die
  CPU hat. Erst, wenn eine Messung es verlangt.
- **Die Sonne fest in der Welt.** Dann sähe eine Richtung immer gegen die
  Sonne, und ihre sichtbaren Seiten lägen im Schatten.
- **Ein `--sun` in der ersten Fassung.** Die Richtung der Sonne gehörte zum
  Baum wie der Radius der Biomfarben. Ohne Bedarf kommt sie nicht dazu
  (Regel 22).

## Folgen

- Das Goldbild der Karte bleibt. Cinematic bekommt eigene Goldbilder.
- Jede Kamera, Richtung und jeder scale geht mit Cinematic, ohne eigenen
  Code: Es ist dasselbe Raster.
- Koordinaten und Höhen sind in beiden Bäumen gleich; das Frontend schaltet
  zwischen ihnen um, ohne die Mitte zu verlieren.
- Die Liste der Kachelbäume aus #68 trägt `look` von Anfang an, und jeder
  neue Baum schreibt es ausdrücklich, auch die Karte als `"map"`. Fehlt
  es, ist es ein alter Baum.
- Cinematic zeigt die Verdeckung des Himmels nur so fein wie das Licht des
  Spiels: In Gassen und unter Bäumen bleibt es heller als mit Strahlen zum
  Himmel.
- Ein Cinematic-Baum kostet gerechnet das 2- bis 3,5-Fache der Zeit der
  Karte, auf der grossen Welt bei scale 32 rund 1,5 bis 2,5 h statt 44 min,
  wenn der Strahl zur Sonne sein Ziel trifft, und etwa so viel Platz wie
  die Karte.
- Neue Seite `docs/renderer/cinematic.md` mit der Umsetzung; `schalter.md`,
  `map-json.md`, `zoomstufen.md` und `kosten.md` nennen den Schalter.
