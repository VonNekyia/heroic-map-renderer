---
title: Relief ohne Pflanzen
description: Was die Höhen des Reliefs aus MOTION_BLOCKING an Kacheln, Schicht und Rest ändern, gegen WORLD_SURFACE, an zwei Ausschnitten der Testwelt; wie oft die Chunks die Heightmap tragen und beide Heightmaps abweichen; was daraus für das Neuzeichnen und den Download der Spieler folgt.
date: 2026-11-01
commits: [420e53b, 81119ed]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/metatile/relief.rs
---

# Relief ohne Pflanzen

Mit den Höhen aus `MOTION_BLOCKING` ändern sich an zwei Ausschnitten der
Testwelt 77–98 % der Kacheln, nach Bytes 89–99 %. Der Baum wird dabei kleiner:
Kacheln −3,0 und −3,8 %, Schicht −7,7 und −8,4 %, Rest −1,5 und −0,5 %. Alle
fertigen Chunks in den Stichproben tragen die Heightmap. Spieler, die die Karte
schon haben, laden danach fast alles neu.

## Aufbau

- **Welt:** die Testwelt, 26.2, direkt gelesen.
- **Stände:** A v0.19.0 (420e53b), Höhen aus `WORLD_SURFACE`; B der Zweig
  (81119ed), Höhen aus `MOTION_BLOCKING`. Beide Release-Build.
- **Kamera:** `top-north`, Richtung `s`, scale 4, `--relief slope`, `--size
  8192`, ohne native Stufen.
- **Ausschnitte,** je rund 1000 Kacheln samt Pyramide:
  - `hang`, `--center -2816 -3328`, die Region mit den meisten Daten;
  - `weit`, `--center 9984 7936`, eine zweite grosse Region.
- **Last:** mit Idle-Priorität, `--low-priority`, 2 Threads, ohne Sperre, neben
  anderer Arbeit. Es geht um Bytes und Pixel, nicht um Zeit. Die Zeit ist nicht
  gemessen; das Feld kostet nur ein Lesen mehr, entschieden vom Reviewer am
  01.11.

## Ablauf

- **Grösse und Anteil:** je Ausschnitt und Stand ein Lauf in einen leeren Ordner,
  dann die Dateien aller Stufen Byte für Byte verglichen. Skript eines
  Durchgangs, 01.11. Ein Lauf ist bitgleich wiederholbar, darum genügt einer.
- **Heightmaps auf der Platte:** die Regionsdateien direkt gelesen, am 01.11.:
  jede 8. Region der Testwelt (48 Regionen) und jede 80. der grossen Welt (32),
  je Chunk Status und beide Felder, und je Spalte der Abstand.

## Ergebnis

### Dateien

| | Dateien | ändern sich | Bytes A | Bytes B | Grösse | Bytes neu zu laden |
|---|---|---|---|---|---|---|
| `hang`, Kacheln | 1109 | 97,7 % | 68,2 MB | 65,6 MB | −3,8 % | 65,0 MB, 99,1 % |
| `hang`, Schicht | 1109 | 98,0 % | 6,0 MB | 5,5 MB | −8,4 % | 5,5 MB, 99,3 % |
| `hang`, Rest | 888 → 875 | 88,5 % | 5,9 MB | 5,9 MB | −0,5 % | 5,6 MB, 94,4 % |
| `weit`, Kacheln | 973 | 77,3 % | 54,1 MB | 52,5 MB | −3,0 % | 46,7 MB, 89,0 % |
| `weit`, Schicht | 973 | 77,5 % | 4,2 MB | 3,9 MB | −7,7 % | 3,5 MB, 89,9 % |
| `weit`, Rest | 656 → 645 | 52,3 % | 3,5 MB | 3,4 MB | −1,5 % | 2,3 MB, 66,7 % |

Nach Bytes ist der Anteil höher als nach Dateien: Die Kacheln, die gleich
bleiben, sind die leeren und einfachen, etwa über Wasser.

### Heightmaps auf der Platte

| | fertige Chunks | mit beiden, Länge 37 | Spalten tiefer als `WORLD_SURFACE` | davon um genau 1 |
|---|---|---|---|---|
| Testwelt | 33 264 | alle | 21,7 % | 21,3 % |
| grosse Welt | 32 192 | alle | 14,1 % | 13,2 % |

Ohne das Feld sind in beiden nur unfertige Chunks, die der Renderer nicht
zeichnet: `structure_starts`, `biomes` und ein Teil von `carvers`. Kein Chunk der
Stichproben nimmt also den Rückfall. Höher als `WORLD_SURFACE` liegt keine
Spalte.

### Neuzeichnen

Gerechnet, nicht gemessen:

- **Nur Bäume mit `--relief`.** Der Zeichenstand von `map`, `cinematic` und
  `flat` bleibt; Cinematic zeichnet nichts neu.
- **Der Baum `top-north-s` der grossen Welt:** 13,5 min für den letzten vollen
  Lauf ohne Schicht mal 1,17 für die Schicht
  ([2026-10-25](2026-10-25-update-mit-relief-schicht.md)) ergibt rund 16 min,
  angesagt 20–25 min. Das Feld allein kostet davon nichts Messbares.
- **Gemessen auf einer Kopie der grossen Welt** mit Relief und Schicht: 27 min
  18 s, auf anderer Hardware.

### Download der Spieler

Hochgerechnet aus den Anteilen oben. Die grosse Welt ist nicht verglichen; dort
liegen weniger Spalten tiefer, der Anteil kann also kleiner sein.

- **Der volle Lauf lässt liegen, was gleich bleibt** (siehe
  [Kacheln](../benutzung/kacheln.md), „Gleiche Bytes bleiben liegen“), mit seinem
  ETag. Geladen wird nur, was sich ändert. Das sind aber nach den Zahlen oben
  89–99 % der Bytes der Kacheln.
- **Wer die Karte schon hat,** gleicht sie ab. Der Deckel eines Abgleichs sind
  50 % der Kacheln des Baums, einer je Tag und Spieler, einzeln über das ETag,
  siehe die Doku des Plugins, `docs/download.md`, „Token“. Bei rund 90 % neuen
  Bytes braucht ein Spieler also zwei Abgleiche, verteilt auf zwei Tage. Dazwischen
  zeigt seine Karte teils das alte Relief, um eine Stufe anders; sonst nichts.
- **Wer nicht warten will,** lädt voll. Das geht als ein Strom und zählt gegen die
  fünf vollen Downloads je Woche.
- **Schicht und Rest** lädt nur, wer im Mod „ohne“ oder `shadows` wählt. Ein
  Abgleich lässt sie ganz zu.

## Schluss

- Die Höhen bleiben ohne messbaren Aufwand, und der Baum wird etwas kleiner.
- Der Preis liegt beim einmaligen Neuzeichnen und vor allem beim Nachladen:
  Für Spieler ist es fast ein neuer Download, verteilt auf zwei Tage.
