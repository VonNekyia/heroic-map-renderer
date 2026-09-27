---
title: Biomübergänge
description: Was die Mischung der Biomfarben und die Tönung beim Zeichnen kosten, auf einem Thread und auf 24, mit und ohne Karte, dazu die Grösse der Kacheln und die Sprite-Tabellen beider Welten.
date: 2026-09-27
commits: [c08d6fa, 08594cf]
code:
  - renderer/src/render/tint.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.wgsl
---

# Biomübergänge

Mit #21 braucht eine Kachel der Testwelt auf einem Thread 7,41 statt
6,94 ms, 7 % mehr. Auf 24 Threads schafft die Basis der grossen Welt 6 bis
7 % weniger Kacheln/s, mit und ohne Karte, und die Kacheln wiegen 1 bis
1,5 % mehr. Die Sprite-Tabelle schrumpft auf gut ein Viertel: auf der
grossen Welt 7489 statt 26 341 Sprites, in rund 3,3 statt 5,9 s und mit
halb so viel Speicher. Hochgerechnet braucht die Testwelt bei scale 32
damit rund 7 statt 6,5 Minuten; die grosse Welt bleibt bei 65 bis 75 min.

## Aufbau

- Welten: die Testwelt, der Ausschnitt um (-64, 416) aus
  [Was ein Lauf kostet](../benutzung/kosten.md), scale 32 ohne native
  Stufen: 676 Basiskacheln (`--size 6656`) auf einem Thread, 10 816
  (`--size 26624`) auf 24; die grosse Welt, scale 32, der Ausschnitt mit
  65 536 Basiskacheln aus
  [2026-09-27, Weiche Beleuchtung](2026-09-27-weiche-beleuchtung.md).
- Stände: master `c08d6fa`, der Renderer wie `1e13363`; #21 `08594cf`.
- Release-Builds, 24 Threads, wo nichts anderes steht; der Kachelordner war
  vom Echtzeitschutz ausgenommen.

## Ablauf

Abwechselnd master und #21, jeder Lauf frisch, am Abend des 27.09.

- **Ein Thread:** fünf Läufe je Stand; ms je Kachel ist 1000 durch die Rate
  der Basis, davon der Median samt Spanne.
- **24 Threads:** auf der Testwelt drei Läufe je Stand ohne Karte und drei
  mit, auf der grossen Welt je zwei.
- **Sprite-Tabelle:** der Export der ganzen Welt bis zur Zeile mit den
  Sprites, dann Abbruch. Die Zeit liegt zwischen der Zeile des Vorlaufs und
  der mit den Sprites, der Speicher ist die Spitze bis dahin. Je Welt und
  Stand drei Läufe.
- **Quelle:** die Ausgabe der Läufe. Die Rate der Basis nennt jeder Lauf
  selbst, der ganze Lauf ist die Wanduhr des Prozesses, die Grössen sind die
  Dateien auf der Platte, dezimal gezählt, der Speicher ist die Spitze des
  Arbeitssatzes in GiB.

## Ergebnis

**Ein Thread**, Testwelt, 676 Basiskacheln:

| | master | #21 |
|---|---|---|
| ms je Kachel, Median (Spanne) | 6,94 (6,85–6,99) | 7,41 (7,35–7,52) |
| ganzer Lauf | 8,3 bis 8,8 s | 8,6 bis 8,8 s |
| je Kachel | 82,2 kB | 83,4 kB |

**24 Threads, Testwelt**, 10 816 Basiskacheln:

| | master | #21 |
|---|---|---|
| ohne Karte, Kacheln/s | 1407, 1200, 1158 | 1214, 927, 1009 |
| mit Karte | 1703, 1493, 1672 | 1655, 1570, 1214 |
| ganzer Lauf ohne Karte | 14,1 / 15,3 / 15,7 s | 15,1 / 17,9 / 16,9 s |
| mit Karte | 12,8 / 13,5 / 12,8 s | 12,7 / 13,3 / 15,0 s |
| je Kachel | 67,4 kB | 68,3 kB |
| Spitze ohne Karte / mit | 1,33–1,39 / 1,83–1,94 GiB | 1,43–1,49 / 1,89–1,95 GiB |

**24 Threads, grosse Welt**, 65 536 Basiskacheln:

| | master | #21 |
|---|---|---|
| ohne Karte, Kacheln/s | 1490, 1535 | 1391, 1442 |
| mit Karte | 1852, 1842 | 1762, 1709 |
| ganzer Lauf ohne Karte | 80,4 / 71,4 s | 75,4 / 74,5 s |
| mit Karte | 63,6 / 64,2 s | 64,8 / 66,5 s |
| Basis | 3,35 GB, 51,1 kB je Kachel | 3,38 GB, 51,6 kB je Kachel |
| Spitze ohne Karte / mit | 1,45–1,47 / 2,10–2,13 GiB | 1,44–1,47 / 2,16–2,18 GiB |

**Sprite-Tabelle der ganzen Welt**, scale 32:

| | master | #21 |
|---|---|---|
| grosse Welt: Sprites, davon Fassungen | 26 341, 21 425 | 7489, 2589 |
| Zeit | 5,5 / 5,9 / 6,4 s | 3,1 / 3,4 / 3,5 s |
| Spitze bis dahin | 0,61 GiB | 0,28–0,32 GiB |
| Testwelt: Sprites, davon Fassungen | 15 096, 12 599 | 4893, 2413 |
| Zeit | 3,0 / 3,1 / 3,7 s | 1,9 / 2,1 / 2,2 s |
| Spitze bis dahin | 0,16–0,17 GiB | 0,13 GiB |

Der Vorlauf bleibt: auf der grossen Welt 58 bis 67 s bei beiden Ständen, auf
der Testwelt 6,6 bis 7,8 s. Die Fassungen je Biom fallen weg; was bleibt,
sind die Fassungen der Flüssigkeiten und die Varianten.

Auf 24 Threads streut die Testwelt stark, ohne Karte 927 bis 1407
Kacheln/s; die Werte eines Stands überlappen sich mit denen des anderen. Die
grosse Welt streut weniger, dort liegt #21 in jeder Runde unter master.

## Die Beispielausgabe

Die Ausgabe in [Kacheln exportieren](../benutzung/kacheln.md), „Ein
Ausschnitt“, 256 Kacheln mit Karte, je drei Läufe am selben Stück: master
756 bis 809 Kacheln/s, #21 660 bis 798; der Lauf im Beispiel gab 752.

## Die Mischung allein

Wie viel davon die Mischung über 25 Blöcke kostet und wie viel die Tönung
beim Zeichnen, sollte ein Vergleich von #21 mit `--biome-blend 0` und mit
der Vorgabe auf einem Thread zeigen, in zwei Serien nach der ersten. Er
ging im Rauschen unter: Die erste Serie lag je Stand auf 2 % genau, die
beiden danach streuten um bis zu 30 %, #21 mit der Vorgabe von 7,35 bis
10,53 ms je Kachel, master von 6,90 bis 7,69; nebenher liefen andere
Programme. Die Mediane mit Radius 0 und 2 lagen gleich auf, 7,69 und 8,89
gegen 7,69 und 8,77 ms. Ein Ergebnis gibt das nicht; die Zahlen oben
stammen aus der ersten Serie.

## Hochgerechnet

- **Testwelt**, scale 32 mit allen nativen Stufen und Pyramide: aus
  [2026-09-27, Weiche Beleuchtung](2026-09-27-weiche-beleuchtung.md) rund
  26 GB in 6,5 min. Mit 7 % mehr Zeit je Basiskachel und 1,5 % mehr je
  Kachel rund 26 GB in rund 7 min. Nicht neu gemessen, nur mit den
  Faktoren von oben.
- **Grosse Welt:** rund 185 GB wie bisher, 1 % mehr; die Dauer bleibt bei
  65 bis 75 min. Der ganze Lauf am Ausschnitt braucht mit Karte 3 % länger
  als auf master, ohne Karte gleich lang; die Basis verliert 6 bis 7 %, die
  Sprite-Tabelle gewinnt 2 bis 3 s.

## Schluss

Die Übergänge kosten auf einem Thread 0,47 ms je Kachel, 7 %, und auf 24
Threads 6 bis 7 % der Rate der Basis; die Kacheln wachsen um 1 bis 1,5 %,
weil sich ein Verlauf schlechter packt als eine Fläche in einer Farbe.
Dafür schrumpft die Sprite-Tabelle der grossen Welt von 26 341 auf 7489
Sprites und braucht halb so viel Speicher. Entscheidung:
[0033](../entscheidungen/0033-toenung-beim-zeichnen.md).
