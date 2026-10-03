---
title: Sprites und Deckung
description: Wie die Sprite-Tabelle aus Familien und Fassungen entsteht, wann ein Sprite als deckend gilt, wann ein Würfel wegfällt, welche Flächen zu gleichen Nachbarn und vor einem vollen Nachbarn entfallen und wie die Deckungsmaske nur zeichnet, was am Ende zu sehen ist.
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
  - renderer/src/assets/baker.rs
  - renderer/src/assets/blockstate.rs
  - renderer/src/assets/nachbarn.txt
  - renderer/src/assets/seiten.txt
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
aus dem Blockentity, gleiche Kollisionsform, gleiche Regel zu den
Nachbarn und gleiche volle Seiten (`family_key` in `sprites.rs`). Das Bild
zählt, weil eine Truhe in jeder Lage dasselbe Blockmodell hat, die
Kollisionsform, weil bei voller jede ebene Fläche im Licht der Zelle davor
liegt, siehe [Weiche Beleuchtung](weiche-beleuchtung.md), „Die Regeln des
Spiels“. Die Regel zählt, weil ein Pack einem Gitter in jeder Verbindung
dasselbe Modell geben kann, siehe unten, „Flächen zu gleichen Nachbarn“.
Die vollen Seiten zählen, weil ein Pack zwei Zuständen, die verschieden
decken, dasselbe Modell geben kann, siehe unten, „Flächen vor einem vollen
Nachbarn“. Licht
trägt kein Sprite, es kommt beim Zeichnen. Banner mit Mustern und Krüge mit
Scherben bekommen je Familie und Daten eine eigene Familie
(`SpriteSet::add_entities`), siehe [Blockentities](blockentities.md), „Im
Renderpfad“.
Eine Familie hat ihre Alternativen, siehe
[Varianten aus der Position](varianten.md), und
Fassungen: eine je Maske verdeckter Flüssigkeitsflächen, dazu die Streifen
an Wasserstufen, siehe [Wasser und Licht](wasser-und-licht.md), und eine je
Maske verdeckender Nachbarn, siehe unten. Gefärbte
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

Ein Würfel wird übersprungen, wenn die Nachbarn, deren Umriss den eigenen
überlappt, ihn ganz decken: deren Umrisse setzen genau den eigenen
zusammen, mehr nicht. Diagonal schräg sind das drei, der Ost-, der Süd-
und der Nachbar darüber. Ost- und Südnachbar müssen dafür ihren ganzen
Umriss deckend füllen, dem Nachbarn darüber genügt sein Boden: Lava endet
bei 8/9 und deckt trotzdem den Block darunter. Welche Nachbarn je Kamera
zählen, legt `Projection::verdeckende_seiten` in
[`renderer/src/render/projection.rs`](../../renderer/src/render/projection.rs)
an einer Stelle fest; `verdeckende_seiten_ueberlappen_den_umriss` prüft es
am Umriss.

Genauso streng ist die Frage, ob ein verdeckter Block überhaupt wegfallen
darf: nur, wenn sein Sprite Pixel für Pixel in diesem Umriss bleibt.
Schilder, Weizen, Rote Bete, Schienen, Feuer und das Lesepult bleiben je
nach scale im Spielraum einer Pixelbreite ganz (siehe [Die Kamera](kamera.md),
„Sortiert wird nach Würfeln“). Dann legen sie ein paar Pixel knapp daneben,
die kein Nachbar sicher deckt, und werden immer gezeichnet. Zerfällt ein
Modell, liegt jeder Teil im Umriss seines Würfels. Teile, die es in einen
Nachbarwürfel legt, kommen trotzdem auch in einen verdeckten Würfel; was
dort verdeckt ist, lässt die Deckungsmaske fallen.

Nachbarn verdecken nur, wenn jede Blockecke auf ganzen Pixeln liegt, siehe
[Die Kamera](kamera.md), „Ganze Pixel“; in 2:1 heisst das: bei Vielfachen
von 4 als scale. Bei anderen, die nur die Bibliothek annimmt, liegen Blöcke
auf halben Pixeln, und ihre Umrisse schliessen nicht lückenlos an. Bei
scale 6 blieben sonst Spalten von einem Pixel.

Von oben verdeckt der Block darüber allein, mit seinem Boden: Der Umriss
eines Würfels ist dort seine Oberseite, und die Nachbarn nach +x und +z
liegen daneben, nicht davor (`expose` in `renderer/src/render/metatile.rs`).
Bei `north-45` verdecken der Südnachbar mit seinem ganzen Umriss und der
Block darüber mit seinem Boden; der Ostnachbar liegt daneben. `top-north`
verdeckt wie von oben, siehe [Die Kamera](kamera.md), „Genordet“. Den Rand
eines Nachbarchunks, der nicht zählt, liest `expose` gar nicht.

## Flächen zu gleichen Nachbarn

Das Spiel zeichnet eine Fläche mit `cullface` nur, wenn
`Block.shouldRenderFace` sie erlaubt, zur `cullface`, wie die Variante sie
dreht (`UnbakedCuboidGeometry`, `Direction.rotate`). Flächen ohne
`cullface` zeichnet es immer. `shouldRenderFace` entscheidet in dieser
Reihenfolge, belegt per javap am Client 26.2:

1. Deckt die Seite des Nachbarn voll (`getFaceOcclusionShape` ist
   `Shapes.block()`), entfällt die Fläche. Das baut der Renderer nach,
   siehe „Flächen vor einem vollen Nachbarn“.
2. Sonst entfällt sie, wenn `skipRendering(nachbar, richtung)` des eigenen
   Blocks wahr ist. Das baut der Renderer nach.
3. Sonst vergleicht das Spiel die Formen beider Seiten. Ohne `canOcclude`,
   also bei allem Durchscheinenden, hat der Block keine Form, und die
   Fläche bleibt. Mit `canOcclude` liegt in 26.2 jede Fläche mit
   `cullface`, die eine Kamera sieht, auf ihrer Wand und zeigt hinaus. Was
   der Nachbar dort deckt, übermalt er. Der dritte Fall ändert so kein
   Pixel und fehlt im Renderer.

`skipRendering` überschreiben in 26.2 sechs Klassen mit 72 Blöcken, in
26.3 mit 75: Dazu kommt das Laub der Pappeln. Ihre
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
  (`Quad::cullface`, `rotate_face` in `baker.rs`). Sie bleibt eine Seite
  der Welt, aus jeder Richtung der Kamera; den Nachbarn dort sucht der
  Cache im Blick, siehe [Richtungen](richtungen.md).
- **Seiten:** Eine Familie mit Regel kennt die Seiten, zu denen eine
  Fläche, die die Kamera sieht, ihre `cullface` hat und die Regel wirken
  kann (`Nachbarregel::wirkt` in `blockstate.rs`). Bei Eis sind das aus
  der Vorgabe oben, Süden und Osten, aus jeder Richtung die Seiten der
  Welt, die im Blick oben, im Süden und im Osten liegen. Bei einer Scheibe
  ist es nur das Ende eines Arms nach Osten oder Süden im Blick; Pfosten
  und Kanten haben keine `cullface`. Bei
  Mangrovenwurzeln sind es oben und unten: Die untere Schicht zeigt ihre
  Oberseite mit `cullface` unten zur Kamera. Bei Pulverschnee sind es alle
  sechs: Seine inneren Schichten zeigen mit `cullface` nach unten und im
  Blick nach Norden und Westen zur Kamera. Dazu kommen die Seiten aus
  „Flächen vor einem vollen Nachbarn“.
- **Fassungen:** Je Alternative gibt es eine Fassung je Maske über diese
  Seiten, ohne die Flächen, deren `cullface` zu einer Seite der Maske zeigt.
  Führt der Block eine Flüssigkeit, gibt es sie je Maske der Flüssigkeit
  noch einmal (`SpriteSet::insert_nachbarn`). Bei Eis sind das 8, bei einer
  gefluteten Scheibe mit einem Arm nach Osten 16, bei Pulverschnee 64, bei
  gefluteten Mangrovenwurzeln mit den Seiten vor einem vollen Nachbarn
  128. Die Fassungen gehören der Familie, nicht wie bei Wasser dem
  Sprite.
- **Beim Zeichnen** fragt `sprite_at` die Nachbarn zu diesen Seiten
  (`Nachbarregel::verdeckt`) und nimmt die Fassung. Bleibt nichts, fällt der
  Block weg, etwa mitten in einer Eismasse.

Deckende Blöcke mit Regel ändern so kein Pixel, blaues Eis etwa: Was sie zu
einem gleichen Nachbarn weglassen, übermalt der Nachbar.

Zeit kostet das nicht messbar: Am Stand und an einer Eisszene der Testwelt
liegen A und B mit und ohne Karte innerhalb der Streuung. Die Tabelle hat
bei scale 32 dort 3 bis 4 % mehr Sprites. Gemessen in
[2026-10-01, Flächen zu gleichen Nachbarn](../messungen/2026-10-01-flaechen-zu-gleichen-nachbarn.md).

## Flächen vor einem vollen Nachbarn

Deckt ein Nachbar zur `cullface` einer Fläche voll, lässt das Spiel sie
weg, im ersten Fall von `Block.shouldRenderFace`. Der Renderer baut das
nach, siehe
[0057](../entscheidungen/0057-flaechen-vor-einem-vollen-nachbarn.md).
Belegt per javap am Client 26.2 und mit einer Probe gegen 26.2.

- **Voll** heisst: `getFaceOcclusionShape` des Nachbarn ist genau
  `Shapes.block()`, als Objekt verglichen. Ein voller Würfel
  (`solidRender`) liefert es an allen sechs Seiten. Sonst schneidet
  `VoxelShape.calculateFace` die Form an der Seite zu einer `SliceShape`
  und gibt `Shapes.block()` nur, wenn diese in jeder Achse genau die
  Koordinaten 0 und 1 hat (`isCubeLikeAlong`). Die beiden anderen Achsen
  nimmt die `SliceShape` von der ganzen Form: Jede Teilung dort zählt,
  auch eine über der Seite. Ist die Form entlang der Achse schon
  würfelartig, gibt `calculateFace` die Form selbst zurück, und voll ist
  sie nur als Würfel.
  - Eine untere Platte deckt so nach unten voll, Schnee mit acht Schichten
    an allen Seiten, Ackerboden nur unten.
  - Ein Endportalrahmen deckt unten voll, mit Auge nicht: Das Auge teilt
    die Form in x und z.
  - Eine Treppe deckt nie voll: Auch ihre volle Rückseite trägt die
    Teilung bei der Hälfte.
  - Blöcke ohne `canOcclude` wie Glas, Laub und Eis decken nirgends.
- **Die Tabelle** [`seiten.txt`](../../renderer/src/assets/seiten.txt)
  trägt je Zustand die Seiten, an denen er voll deckt, aus dem Spiel
  gelesen, siehe [Erzeugte Tabellen](../entwicklung/tabellen.md). Die
  Familie trägt sie als `Family::voll` aus `blockstate::volle_seiten`. Sie
  gehört zum Schlüssel der Familie: Zwei Zustände mit gleichem Bild, die
  verschieden decken, bleiben zwei Familien.
- **Die Seiten einer Familie:** Sie fragt zu einer Seite, wenn eine
  Fläche, die die Kamera sieht, ihre `cullface` dort hat und ein voller
  Nachbar sie nicht ohnehin übermalt. Das tut er nur, wenn sie auf der
  Wand zu ihm liegt oder dahinter in seinem Würfel und zu ihm zeigt
  (`auf_der_wand` in `sprites.rs`). So bekommen volle Würfel, Platten und
  Treppen dafür keine Fassungen; der Sockel des Hebels liegt 0,02 hinter
  der Wand und zählt mit.
- **Beim Zeichnen** fragt `sprite_at` zu jeder Seite den Nachbarn: Deckt er
  zur gegenüberliegenden Seite voll, oder lässt die Regel aus „Flächen zu
  gleichen Nachbarn“ die Fläche weg, nimmt es die Fassung ohne die Flächen
  mit `cullface` dort. Die Fassungen sind dieselben wie dort.

Aus allen Blockmodellen des Clients 26.2 haben diese Blöcke Flächen, die
eine Kamera sieht und ein voller Nachbar nicht übermalt:

| Block | Flächen | Seiten dafür aus `se` |
|---|---|---|
| Mangrovenwurzeln | die inneren Schichten, nach innen | unten, Norden, Westen |
| Pulverschnee | dasselbe; er deckt, zu sehen ist es nicht | unten, Norden, Westen |
| Spawner, Prüfungsspawner | die Innenseiten von Boden und Wänden, durch das Gitter | unten, Süden, Westen |
| Trichter | der Boden der Schale und die Innenseiten des Rands, mit `cullface` oben | oben |

- **Der Spawner** hat in `cube_all_inner_faces` und
  `cube_bottom_top_inner_faces` ein zweites Element mit `from` x 15,998 und
  `to` x 0,002. Das Spiel nimmt beide unsortiert (`FaceInfo.Extent`), die
  Richtung einer Fläche aus ihren Ecken (`FaceBakery.calculateFacing`).
  Seine Flächen zeigen deshalb nach innen. Die Wände in z tragen die
  `cullface` der Wand gegenüber, die übrigen die ihrer eigenen. Die
  Nordwand innen, die die Kamera aus `se` sieht, entfällt so vor einem
  vollen Block im Süden; einer im Norden ändert nichts. Der Baker rechnet
  genauso.
- **Der Trichter** lässt vor einem vollen Block darüber das Innere seiner
  Schale weg. Zu sehen ist das nicht: Jeder Strahl aus der Schale geht
  durch die Öffnung in den Block darüber. Die Seite kostet nur Fassungen
  und einen Nachschlag je Block.
- **Die Choruspflanze** hat solche Flächen nur im Modell `chorus_plant` für
  den Gegenstand; ihr Blockstate nimmt es nicht.
- **Kerzenkuchen** haben sie nur nach unten, das sieht keine Kamera. Der
  **Hebel** hat seinen Sockel hinter der Wand, an Wand und Decke gedreht;
  der volle Nachbar übermalt ihn.

Was bleibt eine Näherung:

- **Blöcke, die `blocks.txt` nicht kennt,** decken nirgends. Im Spiel kann ein
  solcher Block voll decken.
- **Der dritte Fall** ändert nur für die Modelle aus 26.2 kein Pixel. Ein
  Pack mit einer Fläche mit `cullface` neben ihrer Wand an einem Block mit
  `canOcclude` kann abweichen.
- **Das grosse Tropfblatt, gekippt:** Die Blattkanten in
  `big_dripleaf_partial_tilt` und `big_dripleaf_full_tilt` liegen auf der
  Wand im Westen und im Osten. Gekippt um x ragen sie bis z 17,53/16 und
  18,83/16 über den Block hinaus, ein voller Nachbar übermalt nur seinen
  Würfel. Was darüber hinausragt, zeichnet der Renderer, das Spiel nicht.
  Zu sehen ist es kaum: Die Textur ist dort durchsichtig, und das Blatt
  kippt nur kurz.

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
