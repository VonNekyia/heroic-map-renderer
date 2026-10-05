---
title: Biomübergänge
description: Was die Mischung der Biomfarben und die Tönung beim Zeichnen kosten, auf einem Thread und auf 24, mit und ohne Karte, dazu die Grösse der Kacheln, der Speicher und die Sprite-Tabellen beider Welten.
date: 2026-09-27
commits: [c08d6fa, 08594cf]
code:
  - renderer/src/render/tint.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/gpu.wgsl
---

# Biomübergänge

Mit #21 braucht eine Kachel der Testwelt auf einem Thread ohne Karte 7,41
statt 6,94 ms, 7 % mehr. Auf 24 Threads schafft die Basis der grossen Welt
6 bis 7 % weniger Kacheln/s, mit und ohne Karte, und die Kacheln wiegen 1
bis 1,5 % mehr. Die Sprite-Tabelle schrumpft auf gut ein Viertel: auf der
grossen Welt 7489 statt 26 341 Sprites, in rund 3,3 statt 5,9 s, und bis
zu ihr braucht der Lauf halb so viel Speicher; die Spitze des ganzen Laufs
steigt um 0,05 bis 0,1 GiB. Hochgerechnet braucht die Testwelt bei scale 32
damit rund 7 statt 6,5 Minuten, die grosse Welt 66 bis 76 statt 65 bis
75 min.

## Aufbau

- Welten: die Testwelt, der Ausschnitt um (-64, 416) aus
  [Was ein Lauf kostet](../benutzung/kosten.md), scale 32 ohne native
  Stufen: 676 Basiskacheln (`--size 6656`) auf einem Thread, 10 816
  (`--size 26624`) auf 24; die grosse Welt, scale 32, der Ausschnitt mit
  65 536 Basiskacheln aus
  [2026-09-27, Weiche Beleuchtung](2026-09-27-weiche-beleuchtung.md).
- Stände: master `c08d6fa`, der Renderer wie `1e13363`; #21 `08594cf`.
- Release-Builds. Die Serie auf einem Thread ohne Karte, sonst 24 Threads
  ohne und mit Karte, wie in der Tabelle; der Kachelordner war vom
  Echtzeitschutz ausgenommen.

Die Befehle, aus der Wurzel des Repositorys, je Lauf ein leerer Ordner:

```bash
# Testwelt, ein Thread
RAYON_NUM_THREADS=1 renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --center -64 416 --scale 32 --size 6656 --native-levels 0 --gpu off --tiles <ordner>
# Testwelt, 24 Threads, einmal mit --gpu off, einmal mit --gpu on
RAYON_NUM_THREADS=24 renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --center -64 416 --scale 32 --size 26624 --native-levels 0 --gpu off --tiles <ordner>
# grosse Welt, derselbe feste Ausschnitt wie in der Vormessung
RAYON_NUM_THREADS=24 renderer/target/release/heroic-map-renderer --world <grosse Welt> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --data <ihre Biomdaten> --center <Ausschnitt> --size 65536 --gpu off --tiles <ordner>
# Sprite-Tabelle: ohne --center und --size, abgebrochen nach der Zeile mit den Sprites
RAYON_NUM_THREADS=24 renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --gpu off --tiles <ordner>
```

## Ablauf

Abwechselnd master und #21, jeder Lauf frisch, am Abend des 27.09., in
einem Zug von rund einer halben Stunde: zuerst die Serie auf einem Thread,
gleich danach die auf 24 Threads, die grosse Welt und die Sprite-Tabellen.
Die beiden Serien in „Die Mischung allein“ liefen danach.

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

**Ein Thread, ohne Karte**, Testwelt, 676 Basiskacheln:

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

**Speicher:** Bis zur fertigen Sprite-Tabelle halbiert sich die Spitze auf
der grossen Welt, auf der Testwelt sinkt sie um ein Fünftel. Die Spitze des
ganzen Laufs steigt dagegen: auf der Testwelt ohne Karte um 0,1 GiB, auf
der grossen Welt mit Karte um 0,05 GiB. Auf der Testwelt mit Karte
überlappen sich die Spannen, auf der grossen Welt ohne Karte bleibt sie.
Vermutlich kommt das von den Biomen je Block, die der Cache jedes Threads
behält (`ChunkCache::biome_of`); gemessen ist das nicht.

## Gegen die Vormessung

master lief auf einem Thread mit 6,94 ms je Kachel, 13 % langsamer als
derselbe Renderer am Morgen in
[2026-09-27, Weiche Beleuchtung](2026-09-27-weiche-beleuchtung.md), 6,13 ms:
Zwischen `fc62f31` und `c08d6fa` änderten sich im Renderer nur Kommentare
und zwei Hilfetexte. Die Beispielausgabe fiel ebenso, von 1010 bis 1052 auf
756 bis 809 Kacheln/s. Die Ursache ist nicht gemessen, nebenher liefen
andere Programme. Der Vergleich der beiden Stände gilt, weil abwechselnd
gemessen wurde; die absoluten Werte gehören nur zu dieser Messung.

## Die Beispielausgabe

Die Ausgabe in [Kacheln exportieren](../benutzung/kacheln.md), „Ein
Ausschnitt“, 256 Kacheln mit Karte, je drei Läufe am selben Stück: master
756 bis 809 Kacheln/s, #21 660 bis 798; der Lauf im Beispiel gab 752.

## Die Mischung allein

Wie viel davon die Mischung über 25 Blöcke kostet und wie viel die Tönung
beim Zeichnen, sollte ein Vergleich von #21 mit `--biome-blend 0` und mit
der Vorgabe auf einem Thread zeigen, in zwei Serien nach der ersten. Er
ging im Rauschen unter. In der ersten Serie lag der langsamste Lauf je Stand
höchstens 2,3 % über dem schnellsten, bei master 2,0 %. In den beiden
danach lag er bei #21 mit der Vorgabe 43 % darüber, 10,53 gegen 7,35 ms je
Kachel, bei master reichten sie von 6,90 bis 7,69 ms; nebenher liefen
andere Programme. Die Mediane mit Radius 0 und 2 lagen gleich auf, 7,69 und
8,89 gegen 7,69 und 8,77 ms. Ein Ergebnis gibt das nicht; die Zahlen oben
stammen aus der ersten Serie.

## Hochgerechnet

- **Testwelt**, scale 32 mit allen nativen Stufen und Pyramide: aus
  [2026-09-27, Weiche Beleuchtung](2026-09-27-weiche-beleuchtung.md) rund
  26 GB in 6,5 min. Mit 7 % mehr Zeit je Basiskachel und 1,5 % mehr je
  Kachel: 6,5 min · 1,07 = 7,0 min, 26 GB · 1,015 = 26,4 GB, also rund
  26 GB in rund 7 min. Nicht neu gemessen, nur mit den Faktoren von oben;
  für scale 16 und 8 sind sie nicht gemessen.
- **Grosse Welt, Dauer:** Die Vormessung rechnete 60 bis 70 min aus
  [2026-09-27, Wasser im Licht](2026-09-27-wasser-im-licht.md) mal 1,06,
  also 64 bis 74 min, gerundet 65 bis 75. Ein Vollrender läuft mit Karte;
  dort braucht der ganze Lauf am Ausschnitt mit #21 im Mittel 65,7 statt
  63,9 s, 3 % länger: 64 bis 74 min · 1,03 gibt 66 bis 76 min. Ohne Karte
  hängt der Vergleich an einem Ausreisser: Im Lauf von master mit 80,4 s
  blieben neben der Basis 36,4 s, in den übrigen sieben Läufen 27,6 bis
  29,1 s. Die besten Läufe ohne Karte liegen bei 71,4 gegen 74,5 s, 4 %
  mehr.
- **Grosse Welt, Grösse:** rund 185 GB wie bisher, 1 % mehr, 51,6 statt
  51,1 kB je Kachel.

## Nachtrag, erste Runde des Reviews

Die Korrekturen der ersten Runde sind nicht nachgemessen: die obere Hälfte
von hohem Gras und grossem Farn tönt am Block darunter, Blütenteppich und
Wildblumen färben ihre Stiele, das Leuchten gehört zum Schlüssel einer
Familie, der Radius steht in `map.json`. Sie ändern an der Rechnung je Block
nichts; ein paar Blöcke mehr bekommen eine Tönungskarte.

## Schluss

Die Übergänge kosten auf einem Thread 0,47 ms je Kachel, 7 %, und auf 24
Threads 6 bis 7 % der Rate der Basis; die Kacheln wachsen um 1 bis 1,5 %,
weil sich ein Verlauf schlechter packt als eine Fläche in einer Farbe.
Dafür schrumpft die Sprite-Tabelle der grossen Welt von 26 341 auf 7489
Sprites, und bis zu ihr braucht der Lauf halb so viel Speicher; die Spitze
des ganzen Laufs steigt um 0,05 bis 0,1 GiB. Entscheidung:
[0033](../entscheidungen/0033-toenung-beim-zeichnen.md).
