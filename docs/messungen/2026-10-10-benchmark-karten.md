---
title: Benchmark gegen andere Karten
description: Zeit, RAM und Platz von squaremap, Pl3xMap, Dynmap und Heroic als CLI und als Plugin auf erzeugten Welten mit 3 000, 5 000 und 15 000 Blöcken Seitenlänge, dazu je Million Pixel; die Quelle des Diagramms im README.
date: 2026-10-10
commits: [d5b7f9e]
code:
  - renderer/src/cli.rs
---

# Benchmark gegen andere Karten

Auf der Welt mit 15 000 Blöcken Seitenlänge zeichnet Heroic von oben bei
scale 4 rund 3,6 Milliarden Pixel in 320 s mit 1,3 bis 1,5 GiB RAM.
squaremap und Pl3xMap zeichnen ein Sechzehntel davon, ein Pixel je Block,
in 245 und 113 s mit je rund 6,4 GiB. Dynmap `flat` zeichnet etwa so viele
Pixel wie Heroic, in 43 min mit 3,0 GiB. Je Million Pixel ist Heroic damit
rund 12-mal schneller als squaremap, 8-mal schneller als Dynmap `flat` und
6-mal schneller als Pl3xMap. Schräg auf 3 000 Blöcken braucht Dynmap
`surface` 61 min, Heroic in `2:1` bei scale 16 21 s; je Million Pixel ist
das ein Faktor von rund 80.

## Aufbau

- **Messrechner:** AMD Ryzen 9 5900X, 12 Kerne, 24 Threads, 32 GiB RAM,
  NVMe-SSD Samsung 970 EVO Plus, Windows 11. Der Ordner der Messung ist
  vom Echtzeitschutz ausgenommen.
- **Server:** Paper 26.2 Build 129 und, nur für Dynmap, Paper 1.21.11
  Build 132. Java Temurin 25.0.1 mit `-Xms8G -Xmx8G`, ohne Spieler, nur
  auf 127.0.0.1. Nether und End sind aus. Die Webserver aller Plugins sind
  aus: Gemessen wird nur das Rendern.
- **Welten:** mit Chunky erzeugt, ein Quadrat um 0, 0, je eine Welt für
  26.2 und für 1.21.11.

  | Welt | Radius | fertige Chunks | Chunks samt Rand |
  |---|---|---|---|
  | 3k | 1 500 | 35 721 | 44 521 |
  | 5k | 2 500 | 99 225 | 113 569 |
  | 15k | 7 500 | 881 721 | 923 521 |

  Gezählt sind die Chunks mit dem Status `minecraft:full` in den
  Regionsdateien. Am Rand liegen halb erzeugte Chunks.
- **Grundlast:** Vor jedem Lauf lag die Last des Rechners bei 4 bis 18 %,
  vor allem von Discord und dem Treiber der Grafikkarte. Gemessen wurde
  mit dieser Grundlast, ein Lauf startete nur unter 25 %.

### Werkzeuge

Ab Werk, mit zwei Ausnahmen: Jede Einstellung für Threads steht auf alle
24, und wo es ein verlustfreies Format gibt, wird es genommen.

| Werkzeug | Fassung | Karte | Pixel je Block | gesetzt |
|---|---|---|---|---|
| squaremap | 1.3.15 auf Paper 26.2 | von oben, Farbe je Block | 1 | `max-render-threads: 24` für vollen und Hintergrund-Render |
| Pl3xMap | 26.2-555 auf Paper 26.2 | von oben, Renderer `vintage_story` | 1 | `render-threads: 24`, `live-update.threads: 24` |
| Dynmap | 3.7-SNAPSHOT-1015 auf Paper 1.21.11, als `Dynmap-3.8-spigot.jar` | `flat` von oben, Texturen; `surface` schräg 30° | `flat` 4, `surface` Raute rund 22,6 | `parallelrendercnt: 24`, `tiles-rendered-at-once: 24`, `image-format: png` statt `jpg-q90` |
| Heroic CLI | v0.3.0 | `top-north` bei scale 4, Texturen mit Licht; `2:1` bei scale 16 | 4, schräg Raute 16 | `--gpu off`, Threads ab Werk 24 |
| Heroic Plugin | 0.1.0 mit dem Renderer v0.3.0 | wie die CLI | wie die CLI | ab Werk, ohne Grafikkarte |
| Heroic CLI kompakt | v0.3.0 | wie die CLI | 4 | zusätzlich `--compact` |

Dynmap 3.8 läuft nicht auf Paper 26.2. Es läuft darum auf 1.21.11, mit
denselben Grössen. `surface` steht auf 3k einem eigenen Paar gegenüber,
„3k iso“: Heroic in `2:1` bei scale 16.

## Ablauf

- **Je Lauf ein frischer Server:** Die Welt kommt aus der Vorlage, mit
  ihren Zeiten, der Ordner der Kacheln ist leer. Nach „Done“ läuft der
  Server 60 s im Leerlauf, dann kommt der Befehl. Die CLI liest die
  Vorlage direkt.
- **Zeit:** vom Befehl bis zum fertigen Kartenwerk samt Zoomstufen.
  - squaremap: `squaremap fullrender minecraft:overworld` bis „Finished
    rendering map“.
  - Pl3xMap zeichnet beim Start von selbst. Gemessen ist dieser erste
    Render, ab der ersten Sekunde nach „Done“, in der die CPU des Servers
    klar über dem Leerlauf liegt, bis zur letzten Kachel der Oberwelt.
    Pl3xMap wartet ab Werk rund 10 s nach dem Start, bevor es zeichnet;
    Done bis letzte Kachel: 14,7 s auf 3k, 22,4 s auf 5k, 121,1 s auf 15k.
  - Dynmap: `dynmap fullrender world:flat` oder `world:surface` bis zur
    letzten Zoomstufe. Die Zoomstufen rechnet es nach der Basis im Takt
    von 30 s. Bis „Full render … completed“ sind es auf 3k 117 s von 225 s,
    auf 5k 294 s von 430 s, auf 15k 2 423 s von 2 586 s.
  - Heroic Plugin: `/heroicmap render` bis „Voller Lauf bis“, Heroic CLI
    bis zum Ende des Prozesses, beide samt Pyramide.
- **RAM:** die Spitze des Working Set, jede Sekunde abgefragt. Bei den
  Plugins die JVM samt Kindprozessen über dem Mittel des Leerlaufs davor;
  bei Pl3xMap ist das der Leerlauf zwischen „Done“ und dem Beginn des
  Renders. Bei der CLI die Spitze des Prozesses.
- **Platz:** belegt, je Datei auf 4 KiB aufgerundet, alle Zoomstufen. MB
  heisst 10^6 Byte.
- **Pixel:** die nicht durchsichtigen Pixel der Basisstufe, mit Pillow
  gezählt. Sie machen 1 und 4 Pixel je Block vergleichbar.
- **Läufe:** auf 3k und 5k je Werkzeug drei, im Wechsel, auf 15k und für
  `surface` je einer, kompakt je einer.
- **Gestört und ersetzt:** Ein Lauf von squaremap auf 3k zählt nicht. Ein
  abgebrochener Render vom Vorabend lief beim Start weiter, mitten im
  Leerlauf. Ein vierter Lauf ersetzt ihn. Während `surface` lief eine
  Prüfung des Frontends 20 s lang auf einem Kern; der Lauf zählt.
- **Rohdaten:** jeder Lauf mit allen Werten in
  [2026-10-10-benchmark-karten.csv](2026-10-10-benchmark-karten.csv).

## Ergebnis

Median der ungestörten Läufe. Zeit, RAM und Platz wie oben, Mpx sind die
Megapixel der Basis.

<!-- diagramm -->
| Welt | Werkzeug | Zeit (s) | RAM (GiB) | Platz (MB) | Mpx |
|---|---|---|---|---|---|
| 3k | squaremap | 11,9 | 4,67 | 7,4 | 9,145 |
| 3k | Pl3xMap | 5,3 | 5,59 | 17,9 | 9,145 |
| 3k | Dynmap flat | 225,1 | 2,96 | 165,1 | 152,572 |
| 3k | Heroic Plugin | 14,3 | 1,14 | 200,9 | 146,313 |
| 3k | Heroic CLI | 14,3 | 1,10 | 201,4 | 146,313 |
| 3k | Heroic CLI kompakt | 15,9 | 1,17 | 137,5 | 146,313 |
| 3k iso | Dynmap surface | 3680,4 | 3,05 | 2678,9 | 1367,867 |
| 3k iso | Heroic Plugin | 20,5 | 1,71 | 941,9 | 643,699 |
| 3k iso | Heroic CLI | 20,7 | 1,85 | 942,4 | 643,699 |
| 5k | squaremap | 31,3 | 5,08 | 19,6 | 25,402 |
| 5k | Pl3xMap | 13,3 | 6,39 | 46,8 | 25,402 |
| 5k | Dynmap flat | 430,0 | 2,99 | 450,1 | 416,813 |
| 5k | Heroic Plugin | 37,4 | 1,15 | 534,7 | 406,426 |
| 5k | Heroic CLI | 36,3 | 1,14 | 535,9 | 406,426 |
| 5k | Heroic CLI kompakt | 42,2 | 1,17 | 368,0 | 406,426 |
| 15k | squaremap | 245,3 | 6,46 | 150,0 | 225,721 |
| 15k | Pl3xMap | 112,5 | 6,42 | 348,9 | 225,721 |
| 15k | Dynmap flat | 2586,4 | 3,02 | 3479,6 | 3642,364 |
| 15k | Heroic Plugin | 319,0 | 1,45 | 4214,1 | 3611,528 |
| 15k | Heroic CLI | 319,9 | 1,31 | 4222,6 | 3611,528 |
| 15k | Heroic CLI kompakt | 368,7 | 1,28 | 2991,0 | 3611,528 |

Einzelheiten:

| Welt | Werkzeug | Läufe | Zeit s, Spanne | CPU s | s je Mpx | MB je Mpx |
|---|---|---|---|---|---|---|
| 3k | squaremap | 3 | 11,9 bis 12,0 | 119 | 1,301 | 0,811 |
| 3k | Pl3xMap | 3 | 5,2 bis 5,5 | 95 | 0,580 | 1,954 |
| 3k | Dynmap flat | 3 | 224,6 bis 231,7 | 2 105 | 1,475 | 1,082 |
| 3k | Heroic Plugin | 3 | 13,9 bis 15,0 | 230 | 0,098 | 1,373 |
| 3k | Heroic CLI | 3 | 13,2 bis 14,3 | 243 | 0,098 | 1,376 |
| 3k | Heroic CLI kompakt | 1 | – | 280 | 0,109 | 0,940 |
| 3k iso | Dynmap surface | 1 | – | 62 450 | 2,691 | 1,958 |
| 3k iso | Heroic Plugin | 3 | 20,2 bis 21,5 | 355 | 0,032 | 1,463 |
| 3k iso | Heroic CLI | 3 | 20,2 bis 20,7 | 360 | 0,032 | 1,464 |
| 5k | squaremap | 3 | 30,9 bis 31,8 | 285 | 1,232 | 0,772 |
| 5k | Pl3xMap | 3 | 13,0 bis 13,6 | 249 | 0,524 | 1,841 |
| 5k | Dynmap flat | 3 | 423,2 bis 433,0 | 5 459 | 1,032 | 1,080 |
| 5k | Heroic Plugin | 3 | 37,2 bis 37,8 | 686 | 0,092 | 1,316 |
| 5k | Heroic CLI | 3 | 36,2 bis 36,7 | 699 | 0,089 | 1,318 |
| 5k | Heroic CLI kompakt | 1 | – | 823 | 0,104 | 0,906 |
| 15k | squaremap | 1 | – | 2 005 | 1,087 | 0,665 |
| 15k | Pl3xMap | 1 | – | 2 220 | 0,498 | 1,546 |
| 15k | Dynmap flat | 1 | – | 44 880 | 0,710 | 0,955 |
| 15k | Heroic Plugin | 1 | – | 6 452 | 0,088 | 1,167 |
| 15k | Heroic CLI | 1 | – | 6 542 | 0,089 | 1,169 |
| 15k | Heroic CLI kompakt | 1 | – | 7 654 | 0,102 | 0,828 |

- **Pixel:** squaremap und Pl3xMap zeichnen genau 256 Pixel je fertigem
  Chunk, Heroic 4 096. Dynmap `flat` zeichnet dazu einen Teil der halb
  erzeugten Chunks am Rand: auf 3k 37 248 Chunks, auf 5k 101 761, auf 15k
  889 249.
- **Durchsichtige Pixel bei Heroic:** Auf 5k und 15k bleiben in wenigen
  Blöcken einzelne Pixel durchsichtig.
  - Auf 5k sind es 75, das sind 0,00002 % der Pixel. Auf 15k fehlen rund
    770.
  - An Zeit, RAM und Platz ändert das nichts. Es ist ein Fehler im
    Renderer.
- **Streuung:** Die drei Läufe je Werkzeug liegen bei der Zeit höchstens
  10 % auseinander, beim RAM höchstens 11 %, beim Platz gar nicht.

## Schluss

- **Zeit je Million Pixel:** Heroic ist von oben auf jeder Welt das
  schnellste Werkzeug.

  | Werkzeug | s je Mpx |
  |---|---|
  | Heroic | 0,09 bis 0,11 |
  | Pl3xMap | 0,5 bis 0,6 |
  | Dynmap `flat` | 0,7 bis 1,5 |
  | squaremap | 1,1 bis 1,3 |

- **RAM:** Heroic braucht am wenigsten, 1,1 bis 1,5 GiB. squaremap,
  Pl3xMap und Dynmap liegen bei 3 bis 6,5 GiB über dem Leerlauf.
- **Platz je Million Pixel:** squaremap liegt vorn, es speichert eine Farbe
  je Block.

  | Werkzeug | MB je Mpx |
  |---|---|
  | squaremap | 0,67 bis 0,81 |
  | Heroic mit `--compact` | 0,83 bis 0,94 |
  | Dynmap `flat` | 0,96 bis 1,08 |
  | Heroic ohne `--compact` | 1,17 bis 1,38 |
  | Pl3xMap | 1,55 bis 1,95 |
- **Grenzen:**
  - 1k fehlt noch nach dieser Methode, im Pilot liefen andere Fenster.
  - Auf 15k lief je Werkzeug ein Lauf.
  - Mit abgeschaltetem Webserver meldet Dynmap beim Start einen Fehler
    beim Laden eines Kartentyps. Gezeichnet wird trotzdem vollständig.
