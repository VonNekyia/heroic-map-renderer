---
title: Look von Cinematic
description: Kennzahlen des Looks aus 0058 am Prototyp zu #89 über 24 Ansichten der Testwelt gegen die Karte, dazu die verworfenen Kurven, Dunst, feste Weissabgleiche und die Stufen des Pflanzenschattens an Wiesen.
date: 2026-10-02
commits: [89b667b, 666cd46]
code:
  - renderer/src/render/tint.rs
  - renderer/src/render/metatile.rs
---

# Look von Cinematic

Mit dem Look aus [0058](../entscheidungen/0058-look-von-cinematic.md) ist
in 24 Ansichten der Testwelt kein Pixel übersteuert.
- Flächen in der Sonne liegen in 21 Ansichten innerhalb von ±3 L* der
  Karte.
- Schatten erreichen 0,51 bis 0,67 der Helligkeit in der Sonne.
- 20 von 24 Ansichten halten alle Grenzen; ausserhalb liegen drei im
  Schnee und eine im Dorf.

Die verworfenen Varianten liegen weiter weg:
- Eine Filmkurve mit Fuss übersättigt mittlere Töne oder dunkelt
  Schatten.
- Eine Schulter über den ganzen Bereich macht Schnee grau.
- Dunst hebt Schatten und nimmt Farbe.
- Ein fester warmer Weissabgleich färbt Schnee.
- Harte Schatten der Bodenpflanzen machen Wiesen dunkler und unruhiger.

## Aufbau

- **Welt:** die Testwelt, vier Szenen:

  | Szene | Mitte (x, y, z) | Inhalt |
  |---|---|---|
  | Dorf | (−350, 64, 580) | Dorf an der Küste, Sand, Fackeln, Dachüberstände; Meer, Ebene, Strand |
  | Hügel | (620, 64, 940) | Savanne mit Bäumen und Wasser |
  | Stand | (−71, 64, 409) | Wald und Strand |
  | Schnee | (472, 88, −408) | verschneiter Hang mit grossem Überhang, gefrorenes Meer |

- **Ansichten:** je Szene sechs.
  - 2:1 bei scale 32 aus `se` und `nw`;
  - 2:1 bei 16 aus `se`;
  - `top-north` bei 16 aus `s`;
  - `north-45` bei 16 aus `s` und `n`.
  - Jede Ansicht hat 1600 × 1600 Pixel, die Mitte der Szene liegt in der
    Bildmitte.
- **Cinematic:** der Prototyp zu #89 auf master `89b667b`, nur für diese
  Messung gebaut und nicht eingecheckt.
  - Er schiesst einen Strahl je Pixel durch die Pixelmitte und zur Sonne
    den exakten Strahl aus
    [0056](../entscheidungen/0056-exakter-strahl-zur-sonne.md).
  - Er schreibt je Pixel die Anteile des Lichts mit der Stärke 1: Sonne,
    Himmel, Blocklicht, Leuchten.
  - Stärken, Weissabgleich, Belichtung, Kurve und Bloom rechnet eine
    Nachbearbeitung je Pixel daraus, mit den Werten aus 0058.
  - Bodenpflanzen dämpfen den Strahl zur Sonne auf 0,5, wo nichts anderes
    steht.
- **Karte:** master `666cd46`, dieselbe Ansicht mit `--render`.
  - Aus `nw` und `n` liegt `--center` einen Block tiefer in x und z. Dann
    deckt sich die Karte Pixel für Pixel mit dem Prototyp; ein Versatz ist
    keiner gemessen.
- **Biom je Pixel:**
  - aus dem Block des ersten Treffers;
  - aus den Zellen von 4 × 4 × 4 Blöcken der Regionsdateien, ohne den Zoom
    des Spiels;
  - gemischt über 5 × 5 Blöcke auf seiner Höhe;
  - die Temperatur aus `worldgen/biome/*.json` der Spieldaten.

## Kennzahlen

Alle in CIELAB mit D65, aus den sRGB-Bildern von Cinematic und Karte.

| Kennzahl | Was | Grenze |
|---|---|---|
| übersteuert | Anteil der Pixel mit einem Kanal ab 254, ohne Leuchtendes und Bloom | höchstens 0,1 % |
| Schatten/Sonne | L* im Schatten durch L* in der Sonne an derselben Textur und Seite, nur deckende vorderste Flächen ohne Kante, nach Pixeln gewichtet | 0,35 bis 0,66 |
| dL in der Sonne | L* von Cinematic minus L* der Karte, Pixel in der Sonne | ±3 |
| dL gesamt | dasselbe über alle Pixel | nur Angabe |
| C zur Karte | Median von C* Cinematic durch C* Karte, Pixel in der Sonne | 0,9 bis 1,25 |
| Farbton | gewichtetes Mittel der Abweichung des Farbwinkels, Pixel in der Sonne | ±5° |
| C hell | Median des C*-Verhältnisses an Pixeln, die auf der Karte heller als L* 70 sind | mindestens 0,85 |
| Unruhe | mittlere Streuung von L* in Fenstern von 8 × 8 Pixeln, die zu 80 % Wiese sind | nur Angabe |

Wärme ist das gemischte Gewicht des Weissabgleichs nach Biom: 0 für 1, 1
für 1,5.

## Ergebnis

### Der Look, je Ansicht

Belichtung 0,250, aus den Oberseiten in der Sonne von Dorf, Hügel und
Stand in 2:1 bei 32 aus `se`.

| Szene | Ansicht | Wärme | übersteuert | Schatten/Sonne | dL in der Sonne | dL gesamt | C zur Karte | Farbton | C hell | ausserhalb |
|---|---|---|---|---|---|---|---|---|---|---|
| Dorf | 2:1, 32, se | 0,11 | 0,00 % | 0,61 | −1,3 | −2,4 | 1,01 | −1,5° | 1,18 | – |
| Dorf | 2:1, 32, nw | 0,12 | 0,00 % | 0,60 | −0,9 | −2,0 | 1,02 | −1,3° | 1,16 | – |
| Dorf | 2:1, 16, se | 0,26 | 0,00 % | 0,55 | −0,2 | −2,2 | 1,08 | −1,5° | 1,19 | – |
| Dorf | top-north, s | 0,14 | 0,00 % | 0,62 | −1,8 | −3,3 | 1,01 | −1,5° | 1,18 | – |
| Dorf | north-45, s | 0,14 | 0,00 % | 0,64 | −2,3 | −3,3 | 1,00 | −1,0° | 1,17 | – |
| Dorf | north-45, n | 0,15 | 0,00 % | 0,67 | −2,4 | −3,2 | 0,99 | −0,9° | 1,16 | Schatten/Sonne |
| Hügel | 2:1, 32, se | 0,99 | 0,00 % | 0,58 | −0,9 | −4,2 | 1,07 | −1,0° | 1,25 | – |
| Hügel | 2:1, 32, nw | 0,98 | 0,00 % | 0,58 | −0,3 | −3,1 | 1,10 | −1,0° | 1,23 | – |
| Hügel | 2:1, 16, se | 0,88 | 0,00 % | 0,58 | −0,4 | −3,9 | 1,10 | −1,0° | 1,20 | – |
| Hügel | top-north, s | 0,92 | 0,00 % | 0,63 | −0,5 | −6,0 | 1,07 | −0,3° | 1,32 | – |
| Hügel | north-45, s | 0,91 | 0,00 % | 0,61 | −1,6 | −6,0 | 1,06 | +0,2° | 1,32 | – |
| Hügel | north-45, n | 0,92 | 0,00 % | 0,64 | −1,1 | −4,1 | 1,07 | +0,2° | 1,32 | – |
| Stand | 2:1, 32, se | 0,35 | 0,00 % | 0,56 | +2,9 | −1,6 | 1,10 | −0,8° | 1,14 | – |
| Stand | 2:1, 32, nw | 0,32 | 0,00 % | 0,54 | +2,9 | −2,0 | 1,11 | −0,9° | 1,14 | – |
| Stand | 2:1, 16, se | 0,30 | 0,00 % | 0,55 | +2,3 | −1,5 | 1,15 | −0,8° | 1,18 | – |
| Stand | top-north, s | 0,34 | 0,00 % | 0,56 | +0,2 | −6,4 | 1,00 | −1,1° | 1,13 | – |
| Stand | north-45, s | 0,35 | 0,00 % | 0,63 | −1,4 | −6,4 | 0,98 | +0,5° | 1,13 | – |
| Stand | north-45, n | 0,34 | 0,00 % | 0,63 | −1,0 | −5,1 | 0,98 | +0,3° | 1,15 | – |
| Schnee | 2:1, 32, se | 0,00 | 0,00 % | 0,51 | −2,3 | −2,4 | 1,13 | −1,1° | 0,99 | – |
| Schnee | 2:1, 32, nw | 0,00 | 0,00 % | 0,55 | −2,8 | −5,6 | 1,09 | +0,2° | 0,98 | – |
| Schnee | 2:1, 16, se | 0,00 | 0,00 % | 0,55 | −1,9 | −1,2 | 1,15 | +0,3° | 0,97 | – |
| Schnee | top-north, s | 0,00 | 0,00 % | 0,56 | −6,7 | −9,3 | 0,97 | +0,6° | 0,97 | dL in der Sonne |
| Schnee | north-45, s | 0,00 | 0,00 % | 0,66 | −6,7 | −8,9 | 0,73 | −0,5° | 2,69 | Schatten/Sonne, dL in der Sonne, C zur Karte |
| Schnee | north-45, n | 0,00 | 0,00 % | 0,62 | −5,1 | −6,9 | 1,01 | +2,9° | 0,97 | dL in der Sonne |

### Weissabgleich

Derselbe Look, nur der Weissabgleich fest oder nach Biom. Ausserhalb ist
gezählt, wo eine Ansicht eine der Grenzen verletzt; bei 0 und 0,3 nicht
gezählt.

| Weissabgleich | Ansichten ausserhalb | Schnee in der Sonne b* | Schnee im Schatten b* | Dorf in der Sonne b* |
|---|---|---|---|---|
| 0 | – | −10,2 | −25,5 | +19,1 |
| 0,3 | – | −8,2 | −22,6 | +21,3 |
| 1 | 4 von 24 | +0,7 | −14,9 | +26,9 |
| 1,25 | 6 von 24 | +5,4 | −11,9 | +29,1 |
| 1,5 | 7 von 24 | +10,4 | −8,6 | +31,3 |
| nach Biom | 4 von 24 | +0,7 | −14,9 | +28,0 |

### Kurven und Dunst

- **Ansichten:** 2:1 bei 32 aus `se`, Schnee auch `top-north`.
- **Belichtung je Zeile:**
  - beim Look aus den Oberseiten in der Sonne;
  - bei den Kurven aufs Mittel der Karte über alle Pixel, wie sie gebaut
    sind.
- **Sand:** Oberseiten aus Sand in der Sonne im Dorf.

| Variante | Belichtung | Sand in der Sonne L* / C* | Schatten/Sonne Hügel / Stand / Dorf | Schnee dL in der Sonne 2:1 / von oben | C zur Karte, Spanne |
|---|---|---|---|---|---|
| Look aus 0058 | 0,250 | 76,5 / 26,9 | 0,58 / 0,56 / 0,61 | −2,3 / −6,7 | 1,01–1,13 |
| Filmkurve mit Fuss, je Kanal | 0,265 | 83,3 / 19,9 | 0,47 / 0,43 / 0,53 | −2,3 / −7,7 | 1,36–1,53 |
| dieselbe, nur auf die Helligkeit | 0,273 | 84,3 / 29,2 | 0,47 / 0,42 / 0,53 | −1,8 / −7,3 | 1,17–1,23 |
| Schulter über den ganzen Bereich | 0,639 | 67,7 / 18,0 | 0,65 / 0,64 / 0,68 | −14,3 / −21,3 | 0,83–0,99 |
| Look mit Dunst, Strecke 800 | 0,222 | 72,9 / 24,2 | 0,70 / 0,69 / 0,71 | −5,9 / −10,9 | 0,81–0,95 |
| Karte | | 78,2 / 22,0 | | | |

### Pflanzen an Wiesen

- **Pixel:** nur Gras, Farn, Moos und Bodenpflanzen.
- **Look:** aus 0058, die Belichtung bleibt 0,250.
- **Licht:** In allen Zeilen bekommen Flächen ohne `shade` das Licht einer
  Oberseite.

| Szene | Ansicht | Pflanzenschatten | dL Wiese | Schatten/Sonne Wiese | Unruhe | Unruhe Karte |
|---|---|---|---|---|---|---|
| Hügel | 2:1, 32, se | keiner | −5,3 | 0,50 | 6,5 | 5,2 |
| Hügel | 2:1, 32, se | weich 0,5 | −5,8 | 0,58 | 6,5 | 5,2 |
| Hügel | 2:1, 32, se | weich 0,3 | −6,0 | 0,56 | 6,6 | 5,2 |
| Hügel | 2:1, 32, se | hart | −8,6 | 0,50 | 7,4 | 5,2 |
| Hügel | top-north, s | keiner | −6,0 | 0,51 | 5,3 | 4,8 |
| Hügel | top-north, s | weich 0,5 | −7,6 | 0,64 | 5,4 | 4,8 |
| Hügel | top-north, s | weich 0,3 | −8,4 | 0,60 | 5,6 | 4,8 |
| Hügel | top-north, s | hart | −9,7 | 0,55 | 6,1 | 4,8 |
| Dorf | 2:1, 32, se | keiner | −4,0 | 0,55 | 5,9 | 4,6 |
| Dorf | 2:1, 32, se | weich 0,5 | −4,2 | 0,60 | 5,9 | 4,6 |
| Dorf | 2:1, 32, se | weich 0,3 | −4,3 | 0,58 | 5,9 | 4,6 |
| Dorf | 2:1, 32, se | hart | −5,2 | 0,55 | 6,2 | 4,6 |
| Stand | 2:1, 32, se | keiner | −7,9 | 0,53 | 5,3 | 4,5 |
| Stand | 2:1, 32, se | weich 0,5 | −7,9 | 0,54 | 5,3 | 4,5 |
| Stand | 2:1, 32, se | weich 0,3 | −8,0 | 0,54 | 5,3 | 4,5 |
| Stand | 2:1, 32, se | hart | −8,1 | 0,53 | 5,3 | 4,5 |

## Schluss

- **Der Look:** Er hält die Grenzen in 20 von 24 Ansichten.
  - **Schnee:** Von oben und schräg von Norden liegt er in der Sonne bis
    6,7 L* unter der fast weissen Karte. Er übersteuert nicht und behält
    seine Textur.
  - **Dorf aus `n`:** Dort reichen Schatten bis 0,67 der Sonne.
  - **Mittel:** Es liegt 1,2 bis 9,3 L* unter der Karte, weil die Karte
    keine Schlagschatten kennt. Am tiefsten liegt es im Wald und in der
    Savanne von oben und im Schnee.
- **Weissabgleich nach Biom:** Er hält die Grenzen wie der neutrale (je 4
  ausserhalb).
  - Fest 1,25 und 1,5 färben auch den Schnee (b* +5,4 und +10,4) und
    lassen 6 und 7 Ansichten ausserhalb.
  - Unter 1 wird das Bild blau statt warm: Der Himmel ist blauer als die
    warme Sonne.
- **Filmkurve mit Fuss:**
  - **Je Kanal:** Sie bleicht Sand (C* 19,9 statt 22,0 auf der Karte) und
    übersättigt mittlere Töne (C 1,36 bis 1,53 der Karte).
  - **Nur auf die Helligkeit:** Die Farbe bleibt, aber Schatten werden
    dunkler (0,42 bis 0,53 statt 0,56 bis 0,61) und Sand heller (L* 84,3
    statt 78,2).
- **Schulter über den ganzen Bereich:** Mit der Belichtung aufs Mittel der
  Karte drückt sie helle Texturen stärker als mittlere. Schnee liegt in der
  Sonne 14 bis 21 L* unter der Karte.
- **Dunst:** Er hebt Schatten auf 0,69 bis 0,71 und nimmt Farbe (C 0,81
  bis 0,95).
- **Harte Pflanzenschatten:** Sie machen Wiesen bis 3,7 L* dunkler als
  keine und unruhiger (7,4 statt 6,5, Karte 5,2).
  - Weich mit 0,5 kostet bis 1,6 L*, die Unruhe steigt um höchstens 0,1.
  - Unter Bäumen ändert sich kaum etwas: Dort liegt der Boden im Schatten
    der Bäume.
