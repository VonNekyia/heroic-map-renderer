---
title: Strahl zur Sonne und Grösse der Kacheln für Cinematic
description: Was ein Strahl zur Sonne am Prototyp kostet, mit dem alten Gang, über die Bitmasken, mit schnellem Test der Zelle und mit Nachschlag je Section, wie schwer Kacheln in Cinematic gegen die der Karte sind und was ein Strahl je Texel statt je Pixel am Bild ändert; Ausschnitte der Testwelt.
date: 2026-10-02
commits: [89b667b]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/tiles.rs
---

# Strahl zur Sonne und Grösse der Kacheln für Cinematic

Am Prototyp kostet ein Strahl zur Sonne mit dem schnellsten Gang 1,3 bis
2,7 µs je Thread, mit dem alten in derselben Reihe 3,3 bis 7,5 µs. Jedes
Bild bleibt dabei Pixel für Pixel gleich. Die Zeit steckt in der Arbeit je
Zelle, nicht im Laden der Chunks. Kacheln in Cinematic wiegen verlustfrei
das 0,83- bis 1,27-Fache der Karte. Beginnt der Strahl in der Mitte des
Texels statt am Punkt des Pixels, ändern sich 0,9 bis 2,7 % der Pixel.
Grundlage für
[0053: Cinematic als Schalter der Karte](../entscheidungen/0053-cinematic-als-schalter-der-karte.md).

## Aufbau

- **Welt:** die Testwelt, dieselben drei Ausschnitte wie in
  [Cinematic, Zeit je Bild](2026-10-01-cinematic-zeit.md): 1600 × 1600
  Pixel bei scale 32, `--render` mit `--center` auf (−414, 516), (556, 876)
  und (−135, 345). Dorf mit viel Wasser, Hügel, Stand mit Wald.
- **Stand:** master `89b667b` mit dem Prototyp darauf, nur für diese
  Messung gebaut und nicht eingecheckt.
- **Cinematic im Prototyp** wie in
  [Cinematic, Zeit je Bild](2026-10-01-cinematic-zeit.md), „1 Strahl,
  Start an der Höhe“ mit Decke für die Sonne: ein Strahl je Pixel, je
  Fläche, die zur Sonne zeigt, ein Strahl zur Sonne bis 128 Blöcke weit.
- **Arten des Strahls zur Sonne:**

  | Art | Was |
  |---|---|
  | alt | der Gang des Prototyps: je Zelle Familie und Überhänge nachschlagen, je Block alle Dreiecke seines Modells mit Alpha-Test |
  | Bitmasken | Gang über die Bitmasken der Sections: leere Zellen ohne Nachschlag, volle deckende Würfel (`SOLID` und `DICHT`) ohne Flächentest, über der obersten belegten Zelle um einen Chunk (drei Zellen Rand) zum Rand des Chunks; sonst wie alt |
  | Test der Zelle | dazu: reines Wasser ohne Test, Flächen aus Wasser ohne Test, erst die Hülle des Modells, der erste deckende Treffer genügt |
  | Nachschlag je Section | dazu: Überhänge und Lage der Section einmal je Section statt je Zelle, die Nachbarn eines Chunks nur beim ersten Besuch |
  | ohne | kein Strahl zur Sonne; nur zum Zerlegen der Zeit |

- Release-Build, alle 24 Threads, ohne Grafikkarte.

## Ablauf

- Die Zahlen stammen aus der Ausgabe der Läufe vom 01. und 02.10., Zeilen
  `Strahlen:` und `Sonne:`, ausgewertet mit einem Skript.
- **Gleichheit:** Nach jeder Runde vergleicht ein Skript die Bilder jeder
  schnellen Art Pixel für Pixel mit dem alten Gang.
- **Reihen:** je Ausschnitt die Arten im Wechsel, drei Runden, die
  Reihenfolge der Ausschnitte wechselnd. Last vor und nach jeder Reihe
  gemessen, andere Läufe ruhten.

  | Reihe | Arten | Durchgänge | Last vorher, nachher |
  |---|---|---|---|
  | 1, kalt | alt, Bitmasken, ohne | einer, Cache leer | 4 %, 3 % |
  | 2, warm | alt, Bitmasken, ohne | zwei, gezählt der zweite | 0 %, 4 % |
  | 3, warm | alt, Bitmasken, Test der Zelle, ohne | zwei | 0 %, 3 % |
  | 4, warm | alt, Bitmasken und Test der Zelle, beide mit Nachschlag je Section, ohne | zwei | 1 %, 14 % |

  Eine erste Reihe 4 lief unter 18 bis 20 % Last von Programmen
  ausserhalb unserer Sitzungen. Sie zählt nicht; gezählt ist ihre
  Wiederholung. Dort lief der Stand als letzter Ausschnitt der dritten
  Runde bis 6 % langsamer als in den Runden davor; der Median nimmt die
  mittlere Runde.

- **Zeit je Strahl:** Zeit je Bild mit Strahlen weniger der ohne, mal 24
  Threads, geteilt durch die Strahlen zur Sonne.
- **Grösse:** Je Ausschnitt die Bilder der Karte (`--render`, Raster) und
  von Cinematic mit harten Schatten (`SONNE_RADIUS=0`), in 6 × 6 Kacheln
  zu 256 × 256 Pixeln geschnitten, links oben beginnend, und als WebP
  gepackt mit libwebp 1.6.0 über Pillow 12.2.0: verlustfrei wie der
  Renderer, Stufe 0 (method 0, quality 0), `exact`. Nur zum Vergleich auch
  verlustbehaftet mit quality 90. Cinematic einmal mit dem Nebel der
  Vorschau (`NEBEL_D=1200`), einmal ohne Nebel.
- **Je Texel gegen je Pixel:** Nahaufnahmen 640 × 640 Pixel an Häusern des
  Dorfs, Mitte (−433, 516), und im Wald am Stand, (−135, 345), bei scale
  24, 32 und 48; schnellster Gang, harte Schatten, Nebel der Vorschau.
  Einmal beginnt der Strahl zur Sonne am Punkt des Pixels, einmal in der
  Mitte des Texels der getroffenen Fläche: auf achsparallelen Flächen auf
  1/16 Block gerundet, schräge Flächen bleiben je Pixel. Ein Skript zählt
  die Pixel, die sich unterscheiden.

## Ergebnis

**Gleichheit:** Jedes Bild jeder schnellen Art ist Pixel für Pixel gleich
dem des alten Gangs, in allen Runden aller Reihen.

**Strahlen zur Sonne je Bild:** 3,84 Mio. im Dorf, 1,50 je Pixel, denn
unter Wasser bekommt auch der Grund einen; 2,29 und 2,27 Mio. am Hügel und
am Stand, 0,89 je Pixel.

**Zeit je Bild** in s, Median der drei Runden, Spannweite in Klammern;
Reihe 1 im einzigen Durchgang mit leerem Cache, die anderen im zweiten:

| Reihe | Art | Dorf | Hügel | Stand |
|---|---|---|---|---|
| 1, kalt | alt | 3,31 (2,98–3,31) | 1,30 (1,26–1,39) | 1,83 (1,77–1,86) |
| 1, kalt | Bitmasken | 3,14 (2,94–3,19) | 1,20 (1,16–1,24) | 1,73 (1,72–1,79) |
| 1, kalt | ohne | 2,20 (2,10–2,22) | 0,99 (0,98–1,06) | 1,43 (1,40–1,45) |
| 2, warm | alt | 3,25 (2,89–3,25) | 1,28 (1,25–1,29) | 1,73 (1,69–1,75) |
| 2, warm | Bitmasken | 3,13 (2,92–3,20) | 1,15 (1,14–1,18) | 1,68 (1,65–1,70) |
| 2, warm | ohne | 2,17 (2,14–2,20) | 0,94 (0,93–0,94) | 1,35 (1,35–1,40) |
| 3, warm | alt | 3,27 (2,88–3,32) | 1,27 (1,27–1,40) | 1,76 (1,74–1,83) |
| 3, warm | Bitmasken | 3,06 (2,92–3,13) | 1,17 (1,10–1,19) | 1,66 (1,66–1,75) |
| 3, warm | Test der Zelle | 2,61 (2,49–2,61) | 1,15 (1,15–1,16) | 1,59 (1,58–1,61) |
| 3, warm | ohne | 2,14 (2,04–2,15) | 0,95 (0,93–0,96) | 1,43 (1,39–1,48) |
| 4, warm | alt | 3,43 (3,11–3,46) | 1,32 (1,28–1,35) | 1,76 (1,74–1,86) |
| 4, warm | Bitmasken, Nachschlag je Section | 3,21 (2,91–3,26) | 1,17 (1,14–1,19) | 1,66 (1,64–1,72) |
| 4, warm | Test der Zelle, Nachschlag je Section | 2,67 (2,54–2,68) | 1,16 (1,12–1,18) | 1,55 (1,51–1,65) |
| 4, warm | ohne | 2,24 (2,08–2,27) | 1,00 (0,95–1,02) | 1,42 (1,41–1,43) |

**Je Strahl und Thread:**

| Reihe | Art | Dorf | Hügel | Stand |
|---|---|---|---|---|
| 1, kalt | alt | 6,9 µs | 3,3 µs | 4,2 µs |
| 1, kalt | Bitmasken | 5,9 µs | 2,2 µs | 3,2 µs |
| 2, warm | alt | 6,7 µs | 3,6 µs | 4,0 µs |
| 2, warm | Bitmasken | 6,0 µs | 2,2 µs | 3,5 µs |
| 3, warm | alt | 7,1 µs | 3,3 µs | 3,6 µs |
| 3, warm | Bitmasken | 5,8 µs | 2,2 µs | 2,5 µs |
| 3, warm | Test der Zelle | 2,9 µs | 2,0 µs | 1,7 µs |
| 4, warm | alt | 7,5 µs | 3,3 µs | 3,6 µs |
| 4, warm | Bitmasken, Nachschlag je Section | 6,1 µs | 1,7 µs | 2,5 µs |
| 4, warm | Test der Zelle, Nachschlag je Section | 2,7 µs | 1,6 µs | 1,3 µs |

Die Arten liegen nur innerhalb einer Reihe nebeneinander. Gegen den alten
Gang derselben Reihe kostet ein Strahl mit Test der Zelle in Reihe 3 42,
62 und 47 %, mit Nachschlag je Section in Reihe 4 36, 49 und 38 %.

**Was ein Strahl tut,** Reihe 3, Test der Zelle: 18 bis 23 Zellen, davon
0,4 bis 0,8 mit Flächentest und 4 bis 7 Dreiecken, im Dorf 5,8 Zellen
reines Wasser ohne Test, am Hügel 0,2, am Stand 1,8.

**Kalt gegen warm,** Reihe 2: Im ersten Durchgang, mit leerem Cache, sind
die Bilder nur 4 bis 14 % langsamer als im zweiten, mit und ohne Strahlen
zur Sonne. Was ein Strahl kostet, ist also nicht das Laden der Chunks zur
Sonne hin.

**Grösse,** kB je Kachel:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Karte | 59,5 | 78,6 | 119,6 |
| Cinematic, Nebel der Vorschau | 57,2 (0,96) | 99,5 (1,27) | 118,4 (0,99) |
| Cinematic ohne Nebel | 50,3 (0,85) | 80,4 (1,02) | 99,7 (0,83) |
| nur zum Vergleich: Karte, verlustbehaftet q90 | 19,2 | 27,3 | 29,4 |
| nur zum Vergleich: Cinematic mit Nebel, verlustbehaftet q90 | 21,5 | 32,7 | 32,5 |

In Klammern das Vielfache der Karte.

**Je Texel gegen je Pixel,** Anteil der Pixel, die sich unterscheiden:

| | scale 24 | scale 32 | scale 48 |
|---|---|---|---|
| Dorf, Häuser | 1,4 % | 0,9 % | 1,2 % |
| Stand, Wald | 2,7 % | 1,7 % | 2,5 % |

Es sind einzelne Pixel im Laub, wo die Sonne durch Lücken der Blätter
fällt, und Pixel an Schattenkanten.

## Schluss

- Der schnelle Gang ändert kein Pixel. In Reihe 4 kostet ein Strahl mit
  ihm 36 bis 49 % des alten Gangs.
- Am meisten bringt der Test der Zelle, vor allem über Wasser. In Reihe 4
  sinkt ein Strahl mit ihm im Dorf von 6,1 auf 2,7 µs, bei 5,8 Zellen
  reinem Wasser je Strahl; am Stand von 2,5 auf 1,3 µs, am Hügel von 1,7
  auf 1,6 µs.
- Der Nachschlag je Section bringt weniger: gegen den alten Gang derselben
  Reihe 36 bis 49 % statt 42 bis 62 %.
- Was bleibt, ist Arbeit je Zelle, rund 75 bis 120 ns bei 18 bis 23 Zellen
  je Strahl. Für das Ziel aus 0053, höchstens 0,5 µs je Strahl, dürften es
  nur 22 bis 28 ns sein. Das Laden der Chunks ist es nicht: Kalt sind die
  Bilder nur 4 bis 14 % langsamer als warm.
- Kacheln in Cinematic machen den Platz nicht knapp: verlustfrei das 0,83-
  bis 1,27-Fache der Karte. Der Nebel der Vorschau macht sie 14 bis 24 %
  schwerer als ohne Nebel.
- Je Texel statt je Pixel ändert 0,9 bis 2,7 % der Pixel. Zu sehen ist das
  erst vergrössert.
