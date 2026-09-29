---
title: Sprites und Deckung
description: Wie die Sprite-Tabelle aus Familien und Fassungen entsteht, wann ein Sprite als deckend gilt, wann ein Würfel wegfällt und wie die Deckungsmaske nur zeichnet, was am Ende zu sehen ist.
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
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
aus dem Blockentity und gleiche Kollisionsform (`family_key` in
`sprites.rs`). Das Bild zählt, weil eine Truhe in jeder Lage dasselbe
Blockmodell hat, die Kollisionsform, weil bei voller jede ebene Fläche im
Licht der Zelle davor liegt, siehe
[Weiche Beleuchtung](weiche-beleuchtung.md), „Die Regeln des Spiels“. Licht
trägt kein Sprite, es kommt beim Zeichnen. Banner mit Mustern und Krüge mit
Scherben bekommen je Familie und Daten eine eigene Familie
(`SpriteSet::add_entities`), siehe [Blockentities](blockentities.md), „Im
Renderpfad“.
Eine Familie hat ihre Alternativen, siehe
[Varianten aus der Position](varianten.md), und
Fassungen: eine je Maske verdeckter Flüssigkeitsflächen, dazu die Streifen
an Wasserstufen, siehe [Wasser und Licht](wasser-und-licht.md). Gefärbte
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
Schilder, Weizen, Rote Bete, Schienen, Feuer und das Lesepult legen je nach
scale ein paar Pixel knapp daneben, die kein Nachbar sicher deckt; sie
werden immer gezeichnet. Aus demselben Grund kommen Teile, die ein Modell
in einen Nachbarwürfel legt, auch in einen verdeckten Würfel: die Zerlegung
lässt ihnen eine Pixelbreite Spielraum.

Nachbarn verdecken nur bei Vielfachen von 4 als scale; bei anderen, die nur
die Bibliothek annimmt, liegen Blöcke auf halben Pixeln, und ihre Umrisse
schliessen nicht lückenlos an. Bei scale 6 blieben sonst Spalten von einem
Pixel.

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
