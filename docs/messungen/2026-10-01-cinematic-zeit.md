---
title: Cinematic, Zeit je Bild
description: Was ein Bild in Cinematic am Prototyp kostet, mit dem Licht des Spiels und mit Strahlen zum Himmel, und was ein Strahl je Pixel, der Start an der Höhe und eine Decke für die Strahlen zur Sonne bringen; drei Ausschnitte der Testwelt.
date: 2026-10-01
commits: [89b667b]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/licht.rs
---

# Cinematic, Zeit je Bild

Am Prototyp braucht ein Bild in Cinematic mit dem Licht des Spiels bei
1600 × 1600 Pixeln heute 9,5 bis 18,1 s; mit Strahlen zum Himmel statt dem
Licht des Spiels das 2,4- bis 3,7-Fache. Mit einem Strahl je Pixel statt
vier und dem Start an der Höhe sind es 1,5 bis 3,7 s. Der Start an der Höhe
ändert dabei kein Pixel. Glättet man nur die Kanten mit vier Strahlen,
sind es geschätzt 2,5 bis 4,9 s.

## Aufbau

- **Welt:** die Testwelt, dieselben drei Ausschnitte wie in
  [Strahlen durch die Blockwelt](2026-10-01-strahlen-prototyp.md):
  1600 × 1600 Pixel bei scale 32, `--render` mit `--center` auf
  (−414, 516), (556, 876) und (−135, 345). Dorf mit viel Wasser, Hügel,
  Stand mit Wald.
- **Stand:** master `89b667b` mit dem Prototyp darauf, nur für diese
  Messung gebaut und nicht eingecheckt.
- **Cinematic im Prototyp:**
  - je Strahl die Schichten bis zur ersten deckenden Fläche, mit allen
    Flächen der Modelle
  - je Fläche ein Strahl zur Sonne, bis 128 Blöcke weit
  - Himmels- und Blocklicht des Spiels, zwischen den Mitten der acht Zellen
    um die Stelle vor der Fläche gemischt
  - Himmel und Nebel in den Farben aus den Attributen der Biome, Wasser
    nach Tiefe, Tonemapping
- **Arten:**

  | Art | Strahlen je Pixel | Start der Strahlen |
  |---|---|---|
  | heute | 4 | y = 320 |
  | Himmel mit Strahlen | 4, je Fläche dazu 8 zum Himmel statt des Himmelslichts des Spiels | y = 320 |
  | 1 Strahl | 1 | y = 320 |
  | 1 Strahl, Start an der Höhe | 1 | über der höchsten belegten Zelle, die ins Bild fällt |
  | ohne Strahlen zur Sonne | wie davor, ohne Strahl zur Sonne; nur zum Zerlegen der Zeit | Höhe |
  | nur Primärstrahlen | 1, ohne Licht, nur die Flächen zur Kamera; nur zum Zerlegen | Höhe |
  | Decke für die Sonne | wie „1 Strahl, Start an der Höhe“; Strahlen zur Sonne enden über der höchsten belegten Zelle im Gebiet, aus dem ein Block ins Bild schatten kann | Höhe |
  | Raster | die Karte wie heute | — |

- Die Starthöhe sucht der Prototyp vor dem Bild in 0,09 bis 0,11 s, je
  Chunk im Band des Ausschnitts die oberste belegte Section, deren Würfel
  ins Bild fällt, zwei Zellen Rand für überstehende Modelle. Sie zählt
  nicht zur Zeit je Bild; im Renderer käme sie aus dem Vorlauf.
- Release-Build, alle Threads, ohne Grafikkarte.

## Ablauf

- Die Zahlen stammen aus der Ausgabe der Läufe vom 01.10., Zeilen
  `Strahlen:` und `Raster:`, ausgewertet mit einem Skript.
- Ein Durchgang je Lauf mit leerem Cache, also die Zeit je Bild samt Laden
  der Chunks. Vorlauf und Sprites zählen nicht mit, beim Raster ist es
  `render_area`.
- **Hauptreihe:** je Ausschnitt alle Arten bis auf die Decke für die
  Sonne, drei Runden, die Reihenfolge der Ausschnitte wechselnd. Last vor
  der Reihe 5 %, danach 0 %. Andere Läufe ruhten.
- **Zusatzreihe:** „1 Strahl, Start an der Höhe“ und „Decke für die Sonne“
  im Wechsel, drei Runden. Last vorher 8 %, danach 1 %.
- **Gleichheit:** die Bilder mit und ohne Start an der Höhe und mit und
  ohne Decke für die Sonne, Pixel für Pixel verglichen.
- **Kanten:** In der Art „heute“ zählt ein Pixel als Kante, wenn seine vier
  Strahlen nicht dieselbe vorderste Fläche treffen, nach Block und
  Normale.

## Ergebnis

Zeit je Bild, Median der drei Runden, in Klammern die Spannweite:

| Art | Dorf | Hügel | Stand |
|---|---|---|---|
| heute | 18,1 s (15,9–18,6) | 9,47 s (9,22–9,49) | 10,9 s (10,9–11,0) |
| Himmel mit Strahlen | 67,2 s (64,8–67,3) | 22,6 s (22,5–23,8) | 28,5 s (28,4–28,7) |
| 1 Strahl | 4,82 s (4,58–4,88) | 2,65 s (2,60–2,85) | 3,08 s (3,01–3,27) |
| 1 Strahl, Start an der Höhe | 3,71 s (3,63–3,72) | 1,53 s (1,50–1,55) | 2,07 s (2,01–2,09) |
| ohne Strahlen zur Sonne | 2,34 s (2,23–2,40) | 1,03 s (1,02–1,04) | 1,56 s (1,52–1,64) |
| nur Primärstrahlen | 1,71 s (1,66–1,72) | 0,81 s (0,80–0,84) | 1,25 s (1,19–1,25) |
| Raster | 0,26 s | 0,23 s | 0,30 s |

- **Himmel mit Strahlen** kostet das 3,7-, 2,4- und 2,6-Fache von heute.
- **1 Strahl** braucht 27 und zweimal 28 % der Zeit von heute.
- **Start an der Höhe:** y = 98 im Dorf und am Hügel, 130 am Stand, statt
  319. Er spart gegen 1 Strahl 23, 42 und 33 % und ändert kein Pixel.
- **Kanten:** 10,7 % der Pixel im Dorf, 20,5 % am Hügel, 34,9 % am Stand;
  im Wald machen die Löcher im Laub die meisten.
- **Zerlegt,** aus den Medianen von „1 Strahl, Start an der Höhe“:

  | Anteil | Dorf | Hügel | Stand |
  |---|---|---|---|
  | Primärstrahlen | 46 % | 53 % | 60 % |
  | Strahlen zur Sonne | 37 % | 33 % | 25 % |
  | Licht und Farbe | 17 % | 14 % | 15 % |

**Decke für die Sonne,** Zusatzreihe, Median der drei Runden:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| 1 Strahl, Start an der Höhe | 3,40 s | 1,36 s | 1,78 s |
| mit Decke für die Sonne | 3,32 s | 1,20 s | 1,70 s |
| je Runde | +2,9, −2,3, −4,8 % | −8,6, −11,5, −9,3 % | −1,0, −4,4, −4,5 % |

- Die Decke liegt bei y = 98, 98 und 130. Kein Pixel ändert sich.
- „1 Strahl, Start an der Höhe“ streut zwischen den Reihen: im Dorf 3,71 s
  in der Haupt-, 3,40 s in der Zusatzreihe.

**Geschätzt, nicht gemessen:**
- **Kanten glätten,** 4 Strahlen nur an Kanten, sonst 1: die Zeit von
  „1 Strahl, Start an der Höhe“ mal (1 + 3 × Anteil der Kanten). Das gibt
  4,9, 2,5 und 4,2 s, also 27, 26 und 39 % von heute.
- **Sicht aus dem Raster:** das Raster statt der Primärstrahlen, mit dem
  1,5-Fachen seiner Zeit für Tiefe und Normale je Pixel, sonst wie
  „1 Strahl, Start an der Höhe“: 2,4, 1,05 und 1,3 s, ohne Glätten.

## Schluss

- Strahlen zum Himmel kosten das 2,4- bis 3,7-Fache des Lichts des Spiels.
- Der grösste Hebel ist ein Strahl je Pixel: ein Viertel der Zeit. Glätten
  nur an Kanten kostet je nach Laub ein Drittel bis das Doppelte dazu;
  zusammen bleiben 26 bis 39 % der Zeit von heute.
- Der Start an der Höhe spart ein Viertel bis zwei Fünftel und ändert kein
  Pixel.
- Die Decke für die Sonne spart 0 bis 9 %: Die Strahlen zur Sonne kosten
  nah am Boden, durch Laub und Häuser, nicht im leeren Raum darüber.
- Die Sicht aus dem Raster halbierte die Zeit noch einmal. Dafür bräuchte
  das Raster Tiefe und Normale je Pixel, dazu etwas für Böden unter Wasser.
