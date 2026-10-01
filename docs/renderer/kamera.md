---
title: Die Kamera
description: Die feste isometrische Projektion, der scale, die Zeichenreihenfolge ohne Tiefenpuffer, wie überhängende Modelle im Raum in Teile je Würfel zerfallen und warum Weltkoordinaten in f64 projiziert werden.
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/tests/heights.rs
  - renderer/tests/fixtures/projektion.json
---

# Die Kamera

Die Kamera ist fest und orthographisch, alle Faktoren stehen in
[`renderer/src/render/projection.rs`](../../renderer/src/render/projection.rs).
Sichtbar sind immer dieselben drei Seiten: oben, Süden und Osten. Daraus
folgt eine Zeichenreihenfolge nach Höhe und Tiefe, die jeden Tiefenpuffer
über die Kachel überflüssig macht. `scale` ist die Pixelbreite eines
Würfels, Vorgabe 32.

## Projektion

```text
screen_x = (x - z) * scale/2
screen_y = (x + z) * scale/4 - y * scale/2
```

Damit belegt ein voller Würfel genau `scale` mal `scale` Pixel. Sichtbar
sind immer dieselben drei Seiten: oben, Süden (links im Bild) und Osten
(rechts). Die Blickachse ist (1, 1, 1): Punkte, die sich um ein Vielfaches
davon unterscheiden, landen auf demselben Pixel.

`Projection::project_block` bildet die Ecke (x, y, z) eines Blocks ab, die
mit den kleinsten Koordinaten. Für die Koordinatenanzeige rechnet das
Frontend dieselbe Formel rückwärts, siehe
[Frontend](../frontend.md), „Koordinaten“. Damit beide gleich rechnen,
stehen je scale einige Blöcke samt Bildpunkt in
[`renderer/tests/fixtures/projektion.json`](../../renderer/tests/fixtures/projektion.json),
auch negative und welche bei 2²⁴. Ein Test des Renderers schlägt an, wenn
die Datei veraltet ist, und schreibt sie mit
`UPDATE_GOLDEN=1 cargo test --test heights` neu. Das Frontend prüft sein
Modell an ihr.

## scale

`scale` ist die Breite des ganzen Würfels; eine Seitenfläche ist halb so
breit. Bei scale 16 hat sie acht Pixel für sechzehn Texel, bei scale 32
sechzehn: erst dann ist die Textur vollständig zu sehen. Deshalb ist 32 der
Standard, siehe [0013](../entscheidungen/0013-scale-32-als-standard.md). Der
Preis: viermal so viele Kacheln, für die Testwelt rund 300 000 statt 74 000
bei scale 16. Wer die Hälfte der Texturzeilen verschmerzen kann, gibt
`--scale 16` an.

`--scale` nimmt nur Vielfache von 4: Die Projektion setzt Blöcke in
Schritten von scale/4 Pixeln, und nur dann liegt jeder Block auf ganzen
Pixeln. Sonst läge jede zweite Blockreihe auf einem halben Pixel, und
benachbarte Reihen überdeckten sich.

## Zeichenreihenfolge

Der Metatile-Renderer sortiert erst nach Höhe `y`, innerhalb einer Höhe
nach Tiefe `v = x + z`. Beides ist nötig:

- Verdeckt B den Block A, dann liegt B nie tiefer. Sonst wäre der
  senkrechte Abstand im Bild mindestens eine Blockhöhe, und die Umrisse
  berührten sich höchstens.
- Auf gleicher Höhe verdecken Blöcke einander sehr wohl: der Südnachbar
  `(x, y, z+1)` verdeckt die Südfläche von `(x, y, z)`, der Ostnachbar
  `(x+1, y, z)` die Ostfläche. Dort heisst "verdeckt" genau `v_B > v_A`,
  denn `depth = x + y + z = v + y`.

Zusammen ergibt das eine gültige Reihenfolge, und ein globaler Tiefenpuffer
wird unnötig, siehe
[0001](../entscheidungen/0001-zeichenreihenfolge-statt-tiefenpuffer.md). Die
zweite Regel wegzulassen sieht nicht nach einem Sortierfehler aus, sondern
nach Textur; links läuft `u` aussen, rechts `v`:

![Zeichenreihenfolge](../bilder/zeichenreihenfolge.png)

Das Muster links sind die Süd- und Ostflächen jedes Blattblocks, die durch
den Block davor schlagen.

## Sortiert wird nach Würfeln

Sortiert wird nach Blockwürfeln und nicht nach Blöcken. Der Unterschied
zählt für Modelle, die ihren Würfel verlassen: Feuer ist höher als ein
Block. Solche Sprites zerfallen beim Bauen der Sprite-Tabelle in einen Teil
je Würfel, und jeder Teil wird zu dem Zeitpunkt gezeichnet, der zu seinem
eigenen Würfel gehört. Sonst käme ein zwei Blöcke hohes Modell zu früh, und
ein Block dahinter mit höherem Ursprung übermalte seine obere Hälfte.

Zugeordnet wird im Raum, je Fragment (`Raster::teile` in
`renderer/src/render/rasterizer.rs`):
- **Fragment:** Der Rasterizer gibt jeder Ecke ihre Lage im Raum mit. Ein
  Fragment, der Beitrag einer Fläche zu einem Pixel, gehört dem Würfel, in
  dem sein Punkt liegt.
- **Gemischt je Würfel:** Welche Pixel eine Fläche deckt, das Mittel der
  Textur, die AO-Karte und die Füllregel bleiben die des ganzen Modells.
  Nur die Mischung der Fragmente geschieht je Würfel. Übereinander gelegt
  sind die Teile ohne Nachbarn Pixel für Pixel das ganze Modell; eine Naht
  gibt es nicht.
- **Fläche in einer Würfelebene:** Sie gehört dem Würfel dahinter, von der
  Kamera aus gesehen. Die Oberseite eines Blocks bei y = 1 gehört dem
  eigenen.
- **Grenze je Dreieck:** Der Würfel eines Fragments bleibt zwischen dem der
  kleinsten und dem der grössten Ecke seines Dreiecks. Die Gewichte runden,
  und ein Fragment an einer eigenen Kante fiele sonst knapp in den
  Nachbarwürfel. Eine Toleranz an den Ecken braucht es nicht, denn seit
  [0045](../entscheidungen/0045-varianten-genau-drehen.md) liegt eine Ecke
  auf einer Würfelebene genau darauf.
- **Spielraum:** Passt das Bild des ganzen Modells bis auf eine Pixelbreite
  in den eigenen Umriss (`fits_cell`), bleibt es ganz. So bleiben
  Wandfackeln, Korallenfächer und Getreide bei kleinem scale ein Teil. Erst
  was weiter hinausragt, zerfällt.

Bis #65 wurde über den Bildschirm zugeordnet: Ein Pixel gehörte dem
vordersten Würfel der Hülle, dessen Umriss ihn enthält. Die Umrisse
kacheln die Ebene aber nicht. Das Sechseck eines Würfels hat die Fläche
3s²/4, seine Stellen auf dem Bildschirm liegen alle s²/4 auseinander, also
liegt jeder Pixel in drei Sechsecken. Lag das sichtbare Fragment weiter
hinten, kam sein Teil zu spät und übermalte einen Block davor: Feuer etwa
die Südseite des Blocks darüber. Warum es so ist:
[0046](../entscheidungen/0046-teile-je-wuerfel-im-raum.md).

Die Kandidaten kommen sortiert nach `(y, v, u)` aus den Bitmasken, siehe
[Der Weg einer Kachel](renderpfad.md), „Bitmasken“.

### Ein Teil im Würfel eines anderen Blocks

Ragt ein Modell in einen Würfel, in dem ein anderer Block steht, fehlt im
selben Würfel die Tiefe je Pixel. Es entscheidet der Rang im Schlüssel der
Kandidaten (`candidates` in `renderer/src/render/metatile.rs`):
- **Würfelform:** Liegt jede Fläche des Blocks, die die Kamera sieht, auf
  einer der drei vorderen Seiten seines Würfels (`auf_den_vorderseiten`),
  liegt das Teil hinter jeder von ihnen. Es kommt vor dem Block.
  - Ein deckender Block wie Stein deckt es dann ganz, und die
    Deckungsmaske lässt es fallen.
  - Durch die Löcher von Laub und Glas scheint es durch.
  - Ein Grasblock mit Overlay hat Würfelform, Schleim mit seinem inneren
    Würfel und Ackerboden mit 15/16 haben keine.
  - Eine Familie hat Würfelform nur, wenn jede ihrer Alternativen sie hat.
- **Sonst:** Das Teil kommt nach dem Block, siehe „Was bleibt eine
  Näherung“.

Die Referenz ohne Culling (`render_area_without_culling`) zeichnet in
derselben Reihenfolge.

Das Licht eines Teils kommt von seinem eigenen Block, nicht vom Würfel, in
dem es liegt. 2:1 ändert sich durch die Zuordnung im Raum also nur über die
Reihenfolge.

## Stufen, die von der Kamera wegzeigen

Eine Geländestufe, die nach Norden oder Westen zeigt, ist in der Projektion
unsichtbar: Die Oberseite eine Stufe höher liegt auf der Blickachse genau
auf dem Boden dahinter. Was hinter der Stufe steht, verdeckt sie bis auf
die Ränder, die über ihre hintere Ecke ragen. Über einer scheinbar ebenen
Wiese stehen deshalb einzelne Pixel, von einem roten Pilz etwa zwei. Das
ist kein Fehler. Nachzustellen in der Testwelt am Pilz bei
(−155, 72, −4359):

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render stufe.png --center -227 -4431 --size 128 --scale 32
```

## Weltkoordinaten in f64

Weltkoordinaten werden in `f64` projiziert. Minecraft erlaubt knapp 30
Millionen Blöcke in jede Richtung; ab 2²⁴ kann `f32` benachbarte
ganzzahlige Blöcke nicht mehr auseinanderhalten, und zwei Nachbarn landen
auf demselben Pixel.

## Was bleibt eine Näherung

- **Ein Teil im Würfel eines Blocks ohne Würfelform** kommt nach dem
  Block, auch wo das Spiel es dahinter zeigt. Das trifft etwa die oberen bis
  zu 6,7/16 des Feuers unter einer Platte, einer Stufe oder Schleim.
  Innerhalb eines Würfels fehlt die Tiefe je Pixel, und keine feste
  Reihenfolge stimmt immer: Der Fuss von Getreide liegt über der Oberseite
  des Ackerbodens, also vor ihm.
