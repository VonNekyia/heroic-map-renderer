---
title: Weiche Sonnenschatten am Prototyp
description: Was weiche Sonnenschatten über eine Scheibe von 0,53° und 3° in Cinematic zeigen und kosten, an einem Ausschnitt der Testwelt am Prototyp. Zeit gegen den harten Schatten mit Strahlen überall, nur im Halbschatten und mit einem Detektor aus dem harten Schatten; wie breit der Halbschatten wird; Grösse der Kacheln.
date: 2026-10-04
commits: [f7b7120]
code:
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/sonne.rs
---

# Weiche Sonnenschatten am Prototyp

Weiche Sonnenschatten kosten an einem Ausschnitt der Testwelt das 2,2- bis
5,2-Fache des harten Schattens, auch wenn nur Pixel am Rand mehr Strahlen
bekommen. Selbst ein perfekter Detektor käme nicht unter das 1,5-Fache bei
einer Scheibe von 0,53° und das 2,6-Fache bei 3°. Zu sehen ist 0,53° kaum,
mit Halbschatten von 1 bis 4 Pixeln, 3° deutlich, mit 9 bis 24 Pixeln. Die
Kacheln wachsen um 0,3 und 1,0 %. Grundlage von
[0077](../entscheidungen/0077-weiche-sonnenschatten-verworfen.md).

## Aufbau

- **Welt:** die Testwelt, ein Ausschnitt beim Dorf mit Häusern, Eichen und
  einem Hang mit Stufen:
  `--render --cinematic --camera 4:3 --direction se --scale 24 --center -408 597 --size 2400`.
  Ein Einzelbild, ohne Kacheln, Pyramide und native Stufe.
- **Stand:** master `f7b7120` mit einem Prototyp darauf, nur für diese
  Messung gebaut und nicht eingecheckt. Release-Build, alle Threads. Ohne
  Scheibe gibt er Byte für Byte das Bild des master.
- **Der Prototyp:**
  - Je Fragment mit Sonne geht der Strahl zur Mitte der Sonne wie im
    master, mit den Bits „frei zur Sonne“.
  - Dazu 16 Strahlen über die Scheibe. Ihre Richtungen liegen fest als
    Vogel-Spirale, gleich verteilt über die Fläche der Scheibe und für jedes
    Pixel dieselben: kein Zufall, kein Rauschen.
  - Der Wert ist das Mittel der 16, je Strahl wie im master: 0 hinter einer
    deckenden Stelle, Bodenpflanzen dämpfen.
  - Die Strahlen zur Seite gehen ohne die Bits durch den Gang. Die Werte
    der Dreiecke rechnen sie je Test, statt sie vorab zu haben. Der Horizont
    reicht für jede Richtung der Scheibe.
- **Scheiben,** als Durchmesser: 0,53°, die Sonne am Himmel, und 3°, so
  gewählt, dass man es an Bäumen deutlich sieht.
- **Wo die 16 Strahlen gehen,** aus einer Maske je Pixel:
  - **Überall:** jedes Fragment mit Sonne.
  - **Orakel:** genau die Pixel, an denen das Bild mit Strahlen überall vom
    harten abweicht. Das ist die untere Grenze für jeden Detektor.
  - **Detektor:** aus dem harten Schatten. Rand ist ein Pixel mit Sonne,
    dessen harter Wert sich von einem Nachbarn unterscheidet. Um jeden Rand
    liegt ein Quadrat mit dem Radius ⌈t · tan(Scheibe / 2) · 32⌉ + 1
    Pixel, `t` der Abstand zum Werfer, 32 Pixel rund ein Block am Boden.
    Pixel unter Wasser kommen aus dem Orakel dazu, siehe „Ergebnis“.
  - **Gestaffelt:** derselbe Detektor, aber 4 Strahlen je angefangenem
    Pixel Halbschatten, 4 bis 16.

## Ablauf

- Zwei Reihen unter der Sperre bei einer Last unter 10 %, die Stände
  abwechselnd, je drei Läufe. Die Zeit ist die, die der Renderer für das
  Bild nennt.
  - Reihe 1: hart, 0,53° überall, 3° überall.
  - Reihe 2: hart, dann je Scheibe Orakel, Detektor gestaffelt und
    Detektor mit 16 Strahlen.
- Die Arbeit je Strahl stammt aus Zählern im Prototyp: Zellen im Gang und
  geprüfte Blöcke.
- Für die Grösse der Kacheln sind die Bilder in 81 Kacheln zu 256 Pixeln
  geschnitten und mit libwebp kodiert wie im Renderer: verlustfrei, Stufe 0,
  `exact`.
- Die Breite des Halbschattens ist der Abstand zum Werfer an den harten
  Schattenkanten je Ausschnitt mal tan(Scheibe) mal 32 Pixel je Block.

## Ergebnis

Die Zeit des Bildes:

| Stand | Reihe | Läufe | Mittel | gegen hart |
|---|---|---|---|---|
| hart | 1 | 2,8 / 2,6 / 2,6 s | 2,67 s | ×1 |
| 0,53°, überall | 1 | 31,7 / 30,9 / 30,8 s | 31,1 s | ×11,7 |
| 3°, überall | 1 | 31,5 / 31,2 / 31,3 s | 31,3 s | ×11,8 |
| hart | 2 | 2,9 / 2,8 / 2,8 s | 2,83 s | ×1 |
| 0,53°, Orakel | 2 | 4,2 / 4,2 / 4,2 s | 4,20 s | ×1,48 |
| 0,53°, Detektor gestaffelt | 2 | 6,0 / 6,0 / 6,3 s | 6,10 s | ×2,15 |
| 0,53°, Detektor mit 16 | 2 | 11,9 / 12,5 / 12,2 s | 12,2 s | ×4,31 |
| 3°, Orakel | 2 | 7,7 / 7,2 / 7,4 s | 7,43 s | ×2,62 |
| 3°, Detektor gestaffelt | 2 | 10,8 / 10,3 / 10,6 s | 10,6 s | ×3,73 |
| 3°, Detektor mit 16 | 2 | 14,6 / 15,1 / 14,7 s | 14,8 s | ×5,22 |

Wie viele Pixel betroffen sind, bezogen auf die 5,15 Mio. Pixel mit Sonne:

| | 0,53° | 3° |
|---|---|---|
| Bild weicht vom harten ab (Orakel) | 2,7 % | 9,6 % |
| Rand im harten Schatten | 20 % | 20 % |
| vom Detektor markiert | 35 % | 41 % |
| davon aus dem Orakel unter Wasser | 0,9 % | 2,6 % |
| Strahlen je Pixel, gestaffelt | 1,6 | 3,6 |

- **Ränder überall:** Schattenkanten liegen auf Ebene der Texel, an Löchern
  im Laub, an Gras und Blumen und an Stufen im Gelände.
- **Schmale Halbschatten wegfiltern,** unter 0,25 Pixel, spart Strahlen.
  Dann fehlt aber bei 0,53° die Hälfte der Weichheit, bei 3° ein Viertel.
  Bei 0,53° liegt sie fast ganz auf den Texeln direkt an der Kante.
- **Unter Wasser:** Der harte Schatten des vordersten Fragments, der
  Wasserfläche, zeigt den Schatten auf dem Grund nicht. Ein Detektor muss
  die Fragmente darunter mit ansehen.
- **Ein Strahl im Halbschatten** geht durch 1,6- bis 1,8-mal so viele
  Zellen wie ein Strahl zur Seite im Mittel und prüft 3,2- bis 4,3-mal so
  viele Blöcke.
- **Das Bild:** Orakel und Detektor mit 16 Strahlen geben das Bild mit
  Strahlen überall bis auf ±2 von 255. Gestaffelt weicht es an 1,0 % (0,53°)
  und 1,9 % (3°) der Pixel um mindestens 3 ab, höchstens um 43 und 51.
- **16 oder 32 Richtungen:** Bei 3° zeigt der Hang dreifach vergrössert mit
  16 keine Stufen gegen 32. 16 reichen.

Der Halbschatten an drei Stellen:

| Ausschnitt | Abstand zum Werfer, p50 / p90 | 0,53° | 3° |
|---|---|---|---|
| Eiche | 1,0 / 5,5 Blöcke | 0,3 / 1,6 px | 1,7 / 9,2 px |
| Haus | 5,5 / 8,5 Blöcke | 1,6 / 2,5 px | 9,2 / 14,3 px |
| Hang | 10,0 / 14,5 Blöcke | 3,0 / 4,3 px | 16,8 / 24,3 px |

Die Kacheln: hart 7,51 MB in 81 Kacheln, mit 0,53° 0,3 % mehr, mit 3°
1,0 % mehr.

## Schluss

- In der Szene kostet 0,53° das 2,2- bis 4,3-Fache, 3° das 3,7- bis
  5,2-Fache, gestaffelt oder mit 16 Strahlen. Ein perfekter Detektor läge
  bei ×1,5 und ×2,6.
- Auf den [Vollrender 4:3](2026-10-04-vollrender-4x3.md) von 2 h 43 min
  übertragen wären das rund 5 h 50 min bis 11 h 40 min für 0,53° und
  10 h 10 min bis 14 h 10 min für 3°. Das ist eher zu hoch:
  - Die Szene ist Land mit Wald. Über Meer kommt kaum etwas dazu.
  - Kodieren und Pyramide wachsen nicht mit.
  - Die Strahlen zur Seite rechneten ihre Dreiecke je Test.
- Die Kacheln bleiben praktisch gleich gross, auf 160,2 GB kämen 0,5 und
  1,6 GB dazu.
