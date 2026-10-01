---
title: Die Kamera
description: Die Kameras von --camera, die Projektion mit h, a und b, die Regel „ganze Pixel“ für scale und Kamera, die Zeichenreihenfolge ohne Tiefenpuffer, die Draufsicht, Blockkanten auf Pixelmitten, wie überhängende Modelle im Raum in Teile je Würfel zerfallen und warum Weltkoordinaten in f64 projiziert werden.
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/cli.rs
  - renderer/tests/heights.rs
  - renderer/tests/fixtures/projektion.json
---

# Die Kamera

Die Kamera ist eine Parallelprojektion ohne Perspektive. `--camera` wählt
sie je Lauf: schräg mit dem Rautenverhältnis `W:H` von 2:1 bis 1:1, Vorgabe
2:1, oder von oben (`top`). Sichtbar sind schräg immer dieselben drei
Seiten, oben, Süden und Osten, von oben nur die Oberseite. Daraus folgt
eine Zeichenreihenfolge nach Höhe und Tiefe, die jeden Tiefenpuffer über
die Kachel überflüssig macht. `scale` ist die Pixelbreite eines Würfels,
Vorgabe 32. Alle Faktoren stehen in
[`renderer/src/render/projection.rs`](../../renderer/src/render/projection.rs).

## Projektion

```text
screen_x = u · h
screen_y = v · a − y · b

u = x − z,  v = x + z,  h = scale/2
```

| Kamera | a | b | Blickachse (b, 2a, b), gekürzt |
|---|---|---|---|
| `W:H` | scale · H/(2W) | scale/2 | (W, 2H, W) |
| `top` | scale/2 | 0 | (0, 1, 0) |

- **`h`:** Pixel je Schritt in u. Ein Würfel ist immer `scale` breit.
- **`a`:** Pixel je Schritt in v. Die Oberseite eines Blocks ist eine Raute
  von scale × 2a.
- **`b`:** Pixel je Block Höhe. Schräg bleibt b = scale/2, die Wände sind
  bei jeder Raute gleich hoch. Von oben ist b = 0.
- **2:1:** a = scale/4, b = scale/2, Achse (1, 1, 1). Ein voller Würfel
  belegt genau `scale` mal `scale` Pixel.
- **Blickachse:** Punkte, die sich um ein Vielfaches von ihr
  unterscheiden, landen auf demselben Pixel (`Projection::achse`). Sie
  heisst (b, 2a, b) statt (1, k, 1), damit bei b = 0 nichts unendlich wird.
  Die Tiefe eines Punkts ist sein Produkt mit der Achse
  (`Projection::depth`).

Bei scale 32:

| Kamera | a | Achse | Winkel der Achse | native Stufen | Pixel je Spalte gegen 2:1 |
|---|---|---|---|---|---|
| 2:1 | 8 | (1, 1, 1) | 35,3° | 16, 8, 4 | 1 |
| 16:9 | 9 | (8, 9, 8) | 38,5° | keine | 1,13 |
| 8:5 | 10 | (4, 5, 4) | 41,5° | 16 | 1,25 |
| 4:3 | 12 | (2, 3, 2) | 46,7° | 16, 8 | 1,5 |
| 1:1 | 16 | (1, 2, 1) | 54,7° | 16, 8, 4 | 2 |
| `top` | 16 | (0, 1, 0) | 90° | 16, 8, 4 | 2 |

Der Winkel der Achse über dem Horizont ist atan(√2 · a/b).

`Projection::project_block` bildet die Ecke (x, y, z) eines Blocks ab, die
mit den kleinsten Koordinaten. Für die Koordinatenanzeige rechnet das
Frontend dieselbe Formel rückwärts, aus den Zahlen in `map.json`, siehe
[`map.json`](../benutzung/map-json.md), „Kamera und Projektion“. Damit
beide gleich rechnen, stehen je Kamera und scale einige Blöcke samt
Bildpunkt in
[`renderer/tests/fixtures/projektion.json`](../../renderer/tests/fixtures/projektion.json),
auch negative und welche bei 2²⁴, dazu Pixel genau auf Blockkanten. Ein
Test des Renderers schlägt an, wenn die Datei veraltet ist, und schreibt sie
mit `UPDATE_GOLDEN=1 cargo test --test heights` neu. Das Frontend prüft sein
Modell an ihr.

### Gestaucht, nicht isometrisch

Jede Kamera ist eine Parallelprojektion entlang ihrer Blickachse, danach
senkrecht gestaucht oder gestreckt. Das gilt auch für 2:1:
- Beide Zeilen der Projektion stehen senkrecht auf der Achse (b, 2a, b):
  (h, 0, −h) für `screen_x`, (a, −b, a) für `screen_y`.
- Ihre Längen sind verschieden: h · √2 gegen √(2a² + b²).
- 2:1 ist so die echte Isometrie entlang (1, 1, 1), senkrecht um √3/2
  gestaucht. Nach der Raute wirkt sie asin(H/W) = 30° hoch, die Achse steht
  35,3° über dem Horizont.
- 1:1 ist um √(3/2) gestreckt. Unverzerrt ist nur `top`.

## Kameras

`--camera` nimmt `W:H` oder `top` (`Kamera::parse`):
- **Gekürzt:** `16:10` wird `8:5`, bevor der Renderer es schreibt. So
  landet dieselbe Kamera nie in zwei Kachelbäumen.
- **Von 2:1 bis 1:1:** Flacher verdeckt das Gelände mehr und kostet mehr;
  steiler erschiene die Oberseite höher als von oben. Die Meldung sagt das
  so, etwa „3:1 ist flacher als 2:1“ oder „1:2 ist steiler als 1:1“.
- **Nur auf ganzen Pixeln,** siehe „Ganze Pixel“. Sonst bricht der Lauf ab,
  bevor er die Welt liest.
- **Ein Baum, eine Kamera:** Die Kamera gehört zum Kachelbaum wie der
  scale, siehe [Zoomstufen](../benutzung/zoomstufen.md), „Ein Baum, eine
  Kamera“.

Was sich je Kamera im Bild ändert:
- **Schräg** bleiben die Wände gleich hoch, und die Oberseiten wachsen mit
  a.
- **Kanten gegen Luft** laufen wie die Kanten der Raute H:W. Eine
  gleichmässige Treppe ergibt das nur bei 2:1, mit zwei Pixeln je Zeile,
  und bei 1:1 und `top`, mit einem. Bei 4:3 läuft die Treppe 3:4, und
  Texelzeilen werden ungleich hoch.
- **Von oben** verschwindet jede senkrechte Fläche, siehe „Von oben“.
- **Kosten:** Je Spalte der Welt kostet eine schräge Kamera in der Basis
  etwa so viel mehr wie ihre Pixel, an Bytes 8:5 das 1,22- bis 1,28-fache,
  4:3 das 1,39- bis 1,46-fache, 1:1 das 1,88- bis 2,02-fache; `top` liegt
  mit 1,12 bis 1,61 darunter. Auf gleicher Bildfläche ist jede andere
  Kamera schneller als 2:1 und braucht weniger Speicher, siehe
  [2026-10-01, Kameras](../messungen/2026-10-01-kameras.md).

![Dasselbe Dorf in 2:1, 4:3, 1:1 und von oben](../bilder/kameras.webp)

Dasselbe Dorf der Testwelt in 2:1 und 4:3 (oben), 1:1 und von oben
(unten), scale 16, um den Block (−352, 64, 578), Stand `ad17fd5`.

## scale

`scale` ist die Breite des ganzen Würfels; eine Seitenfläche ist halb so
breit. Bei scale 16 hat sie acht Pixel für sechzehn Texel, bei scale 32
sechzehn: erst dann ist die Textur vollständig zu sehen. Deshalb ist 32 der
Standard, siehe [0013](../entscheidungen/0013-scale-32-als-standard.md). Der
Preis: viermal so viele Kacheln, für die Testwelt rund 300 000 statt 74 000
bei scale 16. Wer die Hälfte der Texturzeilen verschmerzen kann, gibt
`--scale 16` an.

## Ganze Pixel

`--scale` und `--camera` gehen nur zusammen, wenn jede Blockecke auf ganzen
Pixeln liegt (`Projection::ganze_pixel`): a ist ganz, und scale ist gerade.
Dann sind auch h und b ganz. Sonst läge jede zweite Blockreihe auf einem
halben Pixel, und benachbarte Reihen überdeckten sich.

- **2:1** heisst das: ein Vielfaches von 4, die Regel von früher.
- **Bei scale 32** gehen 16:a mit a von 8 bis 16, gekürzt 2:1, 16:9, 8:5,
  16:11, 4:3, 16:13, 8:7, 16:15 und 1:1.
- **`top`** braucht einen geraden scale.
- **Native Stufen** gehen, solange der scale der Stufe die Regel erfüllt,
  bis scale 4, siehe [Zoomstufen](../benutzung/zoomstufen.md), „Native
  Stufen“.
- **Verdecken** prüft dieselbe Regel, siehe
  [Sprites und Deckung](sprites-und-deckung.md), „Verdeckte Würfel“.

Geht ein Paar nicht, nennt die Meldung die nächsten Kameras beim selben
scale und die nächsten scales für diese Kamera (`projektion` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs)):

```text
5:3 geht bei scale 32 nicht (a = 9,6). Nächste gültige: 16:9 (a = 9) oder 8:5 (a = 10). 5:3 geht bei scale 30 oder 40.
top geht bei scale 31 nicht: der scale muss gerade sein. top geht bei scale 30 oder 32.
```

## Zeichenreihenfolge

Der Metatile-Renderer sortiert erst nach Höhe `y`, innerhalb einer Höhe
nach Tiefe `v = x + z`. Beides ist nötig, und beides gilt für jede Achse
mit b ≥ 0 und a > 0:

- **Verdeckt B den Block A, dann liegt B nie tiefer.** Auf einem Pixel
  liegt der vordere Punkt um ein Vielfaches der Achse vor dem hinteren, und
  die Achse steigt mit 2a > 0. Ein Würfel ganz unter dem anderen kann also
  nicht vorn liegen.
- **Auf gleicher Höhe verdecken Blöcke einander sehr wohl:** der Südnachbar
  `(x, y, z+1)` verdeckt die Südfläche von `(x, y, z)`, der Ostnachbar
  `(x+1, y, z)` die Ostfläche. Dort heisst "verdeckt" genau `v_B > v_A`,
  denn auf einer Höhe wächst die Tiefe mit b · v. Von oben überlappen sich
  Blöcke einer Höhe gar nicht.

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
  Textur, die AO-Werte der Fragmente und die Füllregel bleiben die des
  ganzen Modells.
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
- **Jede Kamera:** Der Umriss eines Würfels folgt aus h, a und b
  (`in_outline` in `renderer/src/render/sprites.rs`); die Zuordnung im
  Raum braucht nichts sonst. Von oben passt etwa ein Modell, das zwei
  Blöcke hoch senkrecht aufragt, in den eigenen Umriss und bleibt ganz.

Bis #65 wurde über den Bildschirm zugeordnet: Ein Pixel gehörte dem
vordersten Würfel der Hülle, dessen Umriss ihn enthält. Die Umrisse
kacheln die Ebene aber nicht. Das Sechseck eines Würfels hat die Fläche
3s²/4, seine Stellen auf dem Bildschirm liegen alle s²/4 auseinander, also
liegt jeder Pixel in drei Sechsecken. Lag das sichtbare Fragment weiter
hinten, kam sein Teil zu spät und übermalte einen Block davor: Feuer etwa
die Südseite des Blocks darüber. Warum es so ist:
[0050](../entscheidungen/0050-teile-je-wuerfel-im-raum.md).

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

Gemessen an der Testwelt: Am Stand änderte sich 1 der 8500 gerenderten
Kacheln, an einer Feuerszene 3 von 2125, jede mit einem zerfallenen
Modell. Basis, native Stufen und Speicher blieben in der Streuung. Die
Sprite-Tabelle bei scale 32 kostet einmal je Lauf rund 0,025 s mehr, siehe
[2026-10-01, Teile je Würfel im Raum](../messungen/2026-10-01-teile-je-wuerfel-im-raum.md).

## Von oben

Bei `top` ist b = 0 und die Achse (0, 1, 0). Was daraus folgt:
- **Senkrechte Flächen stehen auf der Kante** und fallen weg
  (`zur_kamera` und `EDGE_ON` in `renderer/src/render/rasterizer.rs`).
  Gras, Blumen, Getreide und Seegras sind Kreuze aus senkrechten Flächen
  und verschwinden. Häuser werden zu Rechtecken. Relief zeigen nur noch die
  weiche Beleuchtung und das Himmelslicht.
- **Keine Haarlinie:** Jede Fläche eines Vanilla-Blocks, die die Kamera
  von oben sieht, zeigt deutlich nach oben. Über alle 32 366 Zustände aus
  `blocks.txt` sieht sie 96 750 Flächen, die steilste mit n_y = 0,0079:
  die Fahne eines Banners, im Modell des Spiels um 0,45° geneigt. Danach
  kommen die Statuen des Kupfergolems mit 0,045. Zwischen dem Rauschen des
  Bakers, 1e-7 der Länge, und echter Neigung liegt also keine Fläche; die
  Grenze `EDGE_ON` liegt bei 1e-4. Das prüft
  `von_oben_keine_haarlinie_an_allen_vanilla_bloecken` in `sprites.rs`,
  ignoriert, weil er die Vanilla-Assets braucht.
- **Verdeckt** ist ein Würfel von oben allein durch den Block darüber,
  siehe [Sprites und Deckung](sprites-und-deckung.md), „Verdeckte Würfel“.
- **Jede Höhe liegt im Band:** Das Fenster von v ist für jede Höhe
  dasselbe, siehe [Der Weg einer Kachel](renderpfad.md), „Kandidaten“.
- **Der Boden eines Würfels,** die Oberseite des Blocks darunter, liegt
  schräg b tiefer im Bild, von oben an derselben Stelle.

## Blockkanten auf Pixelmitten

Der Rasterizer tastet jeden Pixel in seiner Mitte ab, bei +0,5. Senkrechte
Kanten liegen bei u · h, also auf ganzen Pixeln und nie auf einer Mitte.
Eine Kante der Raute steigt um a je h, also H:W, von oben 1:1:
- **Sie trifft Pixelmitten genau dann,** wenn W und H, gekürzt, beide
  ungerade sind: Eine Mitte liegt auf ihr, wenn (2j + 1)/(2i + 1) = H/W.
- **Das sind** 1:1, `top`, dazu etwa 5:3 und 7:5, bei jedem scale, an dem
  sie gelten. Bei scale 32 also nur 1:1 und `top`, 2:1 nie.
- **Welcher der beiden Blöcke** einen solchen Pixel bekommt, entscheidet
  die Füllregel, siehe [Rastern ohne Nähte](naehte.md), „Füllregel“. Sie
  braucht genaue Ecken; seit
  [0045](../entscheidungen/0045-varianten-genau-drehen.md) dreht der Baker
  Varianten genau.
- **In deckendem Gelände bleibt kein Pixel offen,** auch nicht auf solchen
  Kanten. Das prüft `kein_loch_in_deckendem_gelaende` in
  `renderer/tests/metatile.rs` an Stufen aus zufällig gedrehten Blöcken,
  für 2:1 und jede Kamera der Invarianten. In der Testwelt um (−64, 416)
  ist bei 4096 × 4096 Pixeln kein Pixel offen, in 2:1, 4:3, 1:1 und `top`
  bei scale 16 und in 5:3 bei scale 30.
- **Das Frontend** bekommt solche Pixel in `projektion.json` vorgerechnet,
  siehe [`map.json`](../benutzung/map-json.md), „Kamera und Projektion“.

## Stufen, die von der Kamera wegzeigen

Eine Geländestufe, die nach Norden oder Westen zeigt, ist in 2:1
unsichtbar. Die Oberseite eine Stufe höher liegt im Bild b Pixel über ihrem
Platz in ebenem Boden. In 2:1 ist b = 2a, genau eine Reihe Rauten: Sie
liegt dort, wo in ebenem Boden die Oberseite dahinter läge, und das Bild
sieht aus, als wäre der Boden eben. Was hinter der Stufe steht,
verdeckt sie bis auf die Ränder, die über ihre hintere Ecke ragen. Über
einer scheinbar ebenen Wiese stehen deshalb einzelne Pixel, von einem roten
Pilz etwa zwei. Das ist kein Fehler. Nachzustellen in der Testwelt am Pilz
bei (−155, 72, −4359):

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render stufe.png --center -227 -4431 --size 128 --scale 32
```

Unsichtbar ist die Stufe genau dann, wenn b = 2a oder b = 0 ist: in 2:1
und von oben. Bei jeder anderen Kamera ist 0 < b < 2a. Die höhere
Oberseite überdeckt dann einen Streifen der tieferen, und die Stufe zeigt
sich.

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
