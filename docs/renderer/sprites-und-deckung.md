---
title: Sprites und Deckung
description: Wie die Sprite-Tabelle aus Familien und Fassungen entsteht, wann ein Sprite als deckend gilt, wann ein Würfel wegfällt, welche Flächen zu gleichen Nachbarn entfallen und wie die Deckungsmaske nur zeichnet, was am Ende zu sehen ist.
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
  - renderer/src/assets/baker.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/nachbarn.txt
---

# Sprites und Deckung

Jede vorkommende Blockstate wird vor dem Rendern einmal als Sprite
gerastert; die Tabelle ist danach unveränderlich und wird von allen Threads
geteilt (`SpriteSet` in
[`renderer/src/render/sprites.rs`](../../renderer/src/render/sprites.rs)).
Ob ein Sprite deckt, entscheidet sein fertiges Bild, Pixel für Pixel. Beim
Zeichnen fallen verdeckte Würfel weg, und die Deckungsmaske in
`renderer/src/render/metatile.rs` lässt jeden Pixel aus, den am Ende etwas
Deckendes übermalt.

## Die Sprite-Tabelle

Die Tabelle entsteht nach dem Vorlauf, der alle vorkommenden Blockstates
einsammelt, siehe [Der Weg einer Kachel](renderpfad.md), „Vorlauf“.
Blockstates mit gleichem Bild teilen sich eine Familie: gleicher Name,
gleiche Modelle, gleiche Flüssigkeit, gleicher Ort der Saat, gleiches Bild
aus dem Blockentity, gleiche Kollisionsform und gleiche Regel zu den
Nachbarn (`family_key` in `sprites.rs`). Das Bild zählt, weil eine Truhe in
jeder Lage dasselbe Blockmodell hat, die Kollisionsform, weil bei voller
jede ebene Fläche im Licht der Zelle davor liegt, siehe
[Weiche Beleuchtung](weiche-beleuchtung.md), „Die Regeln des Spiels“. Die
Regel zählt, weil ein Pack einem Gitter in jeder Verbindung dasselbe Modell
geben kann, siehe unten, „Flächen zu gleichen Nachbarn“. Licht
trägt kein Sprite, es kommt beim Zeichnen. Banner mit Mustern und Krüge mit
Scherben bekommen je Familie und Daten eine eigene Familie
(`SpriteSet::add_entities`), siehe [Blockentities](blockentities.md), „Im
Renderpfad“.
Eine Familie hat ihre Alternativen, siehe
[Varianten aus der Position](varianten.md), und
Fassungen: eine je Maske verdeckter Flüssigkeitsflächen, dazu die Streifen
an Wasserstufen, siehe [Wasser und Licht](wasser-und-licht.md), und eine je
Maske gleicher Nachbarn, siehe unten. Gefärbte
Flächen tragen statt der Farbe eine Tönungskarte, siehe
[Biomfarben](biomfarben.md), „Tönung beim Zeichnen“. Pixelgleiche Sprites
teilen sich einen Eintrag.

Alle Alternativen und Fassungen sind vorab gerastert; der Renderpfad
kopiert Pixel und rechnet je Block nur die Saat der Alternative, die Farben
seines Bioms, das Licht und die weiche Beleuchtung.

## Teile je Würfel

Ein Modell, das seinen Würfel verlässt, zerfällt beim Bauen der Tabelle in
einen Teil je Würfel, siehe [Die Kamera](kamera.md), „Sortiert wird nach
Würfeln“. Solange kein Modell seinen Würfel verlässt, kostet das im
Renderpfad nichts.

## Wann ein Sprite deckt

Ob ein Sprite deckt, entscheidet sein fertiges Bild und nicht sein Modell,
Pixel für Pixel gegen einen gerasterten vollen Würfel, damit Glas von selbst
herausfällt, ohne gepflegte Blockliste. Mit einer Pixelbreite Toleranz am
Rand galten flache Modelle mit schmalem Rand, Druckplatten und Kuchen, als
bodendeckend: Der Block darunter fiel weg, und ihr Rand zeigte den
Hintergrund. Bei scale 4 blieb vom geschrumpften Boden gar kein Pixel übrig.
Siehe [0002](../entscheidungen/0002-deckend-entscheidet-das-bild.md).

## Verdeckte Würfel

Ein Würfel wird übersprungen, wenn seine drei kamerazugewandten Nachbarn ihn
ganz decken: deren Umrisse setzen genau den eigenen zusammen, mehr nicht.
Der Ost- und der Südnachbar müssen dafür ihren ganzen Umriss deckend füllen,
dem Nachbarn darüber genügt sein Boden: Lava endet bei 8/9 und deckt
trotzdem den Block darunter.

Genauso streng ist die Frage, ob ein verdeckter Block überhaupt wegfallen
darf: nur, wenn sein Sprite Pixel für Pixel in diesem Umriss bleibt.
Schilder, Weizen, Rote Bete, Schienen, Feuer und das Lesepult bleiben je
nach scale im Spielraum einer Pixelbreite ganz (siehe [Die Kamera](kamera.md),
„Sortiert wird nach Würfeln“). Dann legen sie ein paar Pixel knapp daneben,
die kein Nachbar sicher deckt, und werden immer gezeichnet. Zerfällt ein
Modell, liegt jeder Teil im Umriss seines Würfels. Teile, die es in einen
Nachbarwürfel legt, kommen trotzdem auch in einen verdeckten Würfel; was
dort verdeckt ist, lässt die Deckungsmaske fallen.

Nachbarn verdecken nur bei Vielfachen von 4 als scale; bei anderen, die nur
die Bibliothek annimmt, liegen Blöcke auf halben Pixeln, und ihre Umrisse
schliessen nicht lückenlos an. Bei scale 6 blieben sonst Spalten von einem
Pixel.

## Flächen zu gleichen Nachbarn

Das Spiel zeichnet eine Fläche mit `cullface` nur, wenn
`Block.shouldRenderFace` sie erlaubt, zur `cullface`, wie die Variante sie
dreht (`UnbakedCuboidGeometry`, `Direction.rotate`). Flächen ohne
`cullface` zeichnet es immer. `shouldRenderFace` entscheidet in dieser
Reihenfolge, belegt per javap am Client 26.2:

1. Deckt die Seite des Nachbarn voll (`getFaceOcclusionShape` ist
   `Shapes.block()`), entfällt die Fläche. Zeigt sie zur Kamera, übermalt
   der Nachbar sie im Renderer ohnehin.
2. Sonst entfällt sie, wenn `skipRendering(nachbar, richtung)` des eigenen
   Blocks wahr ist. Das baut der Renderer nach.
3. Sonst vergleicht das Spiel die Formen beider Seiten. Ohne `canOcclude`,
   also bei allem Durchscheinenden, hat der Block keine Form, und die
   Fläche bleibt.

`skipRendering` überschreiben in 26.2 sechs Klassen mit 72 Blöcken. Ihre
Regeln stehen in [`nachbarn.txt`](../../renderer/src/assets/nachbarn.txt),
aus dem Spiel gelesen, siehe [Erzeugte Tabellen](../entwicklung/tabellen.md):

| Regel | Klasse | Blöcke | Fläche entfällt zu einem Nachbarn |
|---|---|---|---|
| `gleich` | `HalfTransparentBlock`, `PowderSnowBlock` | 32: Eis, brüchiges Eis, blaues Eis, Glas, getöntes Glas, Buntglas, Slime, Honig, Kupferrost (`copper_grate`), Pulverschnee | desselben Blocks, in jeder Richtung |
| `senkrecht` | `MangroveRootsBlock` | Mangrovenwurzeln | desselben Blocks, nur oben und unten |
| `verbunden` | `IronBarsBlock` | 26: Scheiben und Gitter | desselben Blocks oben und unten; waagrecht nur, wenn beide zueinander verbunden sind. Eisengitter und Kupfergitter teilen den Tag `bars` und lassen waagrecht auch zueinander weg |

- **Laub** (`LeavesBlock`, 11 Blöcke) lässt nur ohne `cutoutLeaves` etwas
  weg. Das schaltet die Grafik „Schnell“ aus; mit der Vorgabe des Spiels
  entfällt nichts, und Laub fehlt in der Tabelle.
- **Wasser und Lava** (`LiquidBlock`) lassen ihre Flächen zu derselben
  Flüssigkeit weg. Das tun schon die Masken der Flüssigkeiten, siehe
  [Wasser und Licht](wasser-und-licht.md).
- **Verschiedene Blöcke** bleiben übereinander: Eis neben Glas, Gläser
  verschiedener Farbe, `ice` neben `frosted_ice`, gewachstes neben
  ungewachstem Kupferrost, Scheibe neben Gitter.

Im Renderer, siehe
[0044](../entscheidungen/0044-flaechen-zu-gleichen-nachbarn.md):

- **Baker:** Jedes Viereck trägt seine `cullface`, mit der Variante gedreht
  (`Quad::cullface`, `rotate_face` in `baker.rs`).
- **Seiten:** Eine Familie mit Regel kennt die Seiten, zu denen eine
  Fläche, die die Kamera sieht, ihre `cullface` hat und die Regel wirken
  kann (`Nachbarregel::wirkt` in `blockstate.rs`). Bei Eis sind das oben,
  Süden und Osten. Bei einer Scheibe ist es nur das Ende eines Arms nach
  Osten oder Süden; Pfosten und Kanten haben keine `cullface`. Bei
  Mangrovenwurzeln sind es oben und unten: Die untere Schicht zeigt ihre
  Oberseite mit `cullface` unten zur Kamera. Bei Pulverschnee sind es alle
  sechs: Seine inneren Schichten zeigen mit `cullface` nach unten, Norden
  und Westen zur Kamera.
- **Fassungen:** Je Alternative gibt es eine Fassung je Maske über diese
  Seiten, ohne die Flächen, deren `cullface` zu einer Seite der Maske zeigt.
  Führt der Block eine Flüssigkeit, gibt es sie je Maske der Flüssigkeit
  noch einmal (`SpriteSet::insert_nachbarn`). Bei Eis sind das 8, bei einer
  gefluteten Scheibe mit einem Arm nach Osten 16, bei gefluteten
  Mangrovenwurzeln 32, bei Pulverschnee 64. Die Fassungen gehören der Familie, nicht wie bei
  Wasser dem Sprite.
- **Beim Zeichnen** fragt `sprite_at` die Nachbarn zu diesen Seiten
  (`Nachbarregel::verdeckt`) und nimmt die Fassung. Bleibt nichts, fällt der
  Block weg, etwa mitten in einer Eismasse.

Deckende Blöcke mit Regel ändern so kein Pixel, blaues Eis etwa: Was sie zu
einem gleichen Nachbarn weglassen, übermalt der Nachbar.

Zeit kostet das nicht messbar: Am Stand und an einer Eisszene der Testwelt
liegen A und B mit und ohne Karte innerhalb der Streuung. Die Tabelle hat
bei scale 32 dort 3 bis 4 % mehr Sprites. Gemessen in
[2026-10-01, Flächen zu gleichen Nachbarn](../messungen/2026-10-01-flaechen-zu-gleichen-nachbarn.md).

Was bleibt eine Näherung:

- **Innere Flächen vor einem vollen Nachbarn.** Den ersten Fall baut der
  Renderer nicht nach. Er greift bei Flächen, die zur Kamera zeigen, deren
  `cullface` aber nach unten, Norden oder Westen weist. Solche Flächen
  haben in 26.2 die Mangrovenwurzeln, der Spawner, der Prüfungs-Spawner und
  die Choruspflanze, dazu der Pulverschnee, bei dem es nicht zu sehen ist,
  denn er deckt. Steht dort ein voller Block, lässt das Spiel sie weg, der
  Renderer zeichnet sie.

## Hineinragende Nachbarmodelle

Die Suche nach hineinragenden Nachbarmodellen kostet nichts, solange kein
Modell seinen Würfel verlässt. Sonst geht sie von den Blöcken aus, deren
Modell hinausragt, die Masken kennen sie: je solchem Block ein Nachschlagen
je Würfel, in den ein Modell ragen kann, statt eines je leerem Würfel.

## Deckungsmaske

Die Kandidaten laufen auf der CPU von vorn nach hinten über eine Maske mit
einem Bit je Leinwandpixel: "hier liegt schon ein deckender Pixel"
(`Deckung` in `renderer/src/render/metatile.rs`). Ein Block, dessen Umriss
bedeckt ist, bekommt keine Sprite-Wahl; ein Sprite, von dem nichts mehr
durchscheint, fällt weg; die übrigen merken sich ihre sichtbaren Pixel, und
nur die zeichnet der Blit, in der alten Reihenfolge. Das löst die
Nachbartabelle ab, die nur drei Nachbarn in derselben Kachel sah, siehe
[0026](../entscheidungen/0026-deckungsmaske.md).

Die Karte bekommt dieselben Draws, die die Maske behält, und zeichnet jeden
ganz; was davon verdeckt ist, übermalt ein späterer Draw mit Alpha 255. Die
Liste ist so dreimal kürzer, und CPU und Karte teilen sich einen Durchgang.
Was die Maske bringt:
[2026-09-27, Die grossen Posten, zweite Runde](../messungen/2026-09-27-grosse-posten-zweite-runde.md).
