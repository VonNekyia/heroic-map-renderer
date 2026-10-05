---
title: Masken aus den Klassen
description: Was es bringt, Flächen und Quellen für die Ausbreitung aus den Klassenmasken zu lesen statt in der Schleife über die Blöcke zu sammeln (#53), im Wechsel gegen master, mit einem und 24 Threads, dazu Bytegleichheit.
date: 2026-09-29
commits: [8308264, 6496410]
code:
  - renderer/src/render/metatile.rs
---

# Masken aus den Klassen

Liest `Masks::of` die Flächen und Quellen für die Ausbreitung aus den
Masken ihrer Klassen, statt sie in der Schleife über die 4096 Blöcke zu
sammeln, braucht es 46 bis 48 % weniger Zeit. Seinen Anteil an der Zeit der
Basis senkt das am Stand der Testwelt von 6,0 auf 3,3 %, am Fichtenwald von
12,4 auf 7,1 %. Der ganze Lauf wird dadurch im Median 1,7 bis 5,5 %
kürzer, am Stand liegt das in der Streuung. Jede Kachel bleibt Byte für
Byte gleich.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **master:** `8308264`, mit dem Licht aus #34;
  - **#53:** `6496410`.
- In beide Stände war nur für diese Messung eine Uhr eingebaut, nicht
  eingecheckt. Sie summiert über alle Threads die Zeit in `Masks::of` und
  zählt die Aufrufe; der Lauf gibt beides am Ende aus.
- Die Testwelt, scale 32, ohne Karte (`--gpu off`), ohne native Stufen:
  - **ein Thread:** `RAYON_NUM_THREADS=1`, 676 Basiskacheln um (-64, 416)
    mit `--size 6656`, Streifen von 8 Spalten;
  - **Stand:** 24 Threads, 5184 Basiskacheln um (-64, 416) mit
    `--size 18432`, Streifen von 4 Spalten;
  - **Fichtenwald:** 24 Threads, 1089 Basiskacheln um (-2712, -3297) mit
    `--size 8192`, Streifen von 2 Spalten.
- Ein Vollrender der grossen Welt läuft mit Streifen von 8 Spalten. Schmale
  Streifen dekodieren einen Chunk öfter; ein Gewinn je Dekodieren wirkt an
  diesen Ausschnitten deshalb stärker als dort.
- Ohne Karte, weil die Basis mit Karte zwischen zwei Zuständen streut,
  siehe
  [2026-09-29, Pyramide von der Platte und im Speicher](2026-09-29-pyramide-platte-und-speicher.md),
  „Zwei Zustände der Basis“.
- Jeder Lauf in ein frisches Verzeichnis, das vom Echtzeitschutz
  ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Nebenher
  lief nichts von uns. Die Last lag kurz vor der Serie bei 5 %, beim Start
  der Serie bei 20 % und danach bei 10 %; woher die 20 % kamen, ist nicht
  bekannt. Den Nachweis trägt deshalb die Uhr um `Masks::of`, der ganze
  Lauf ist nur ein Hinweis.

Der Befehl, aus der Wurzel des Repositorys, am Stand:

```bash
renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 0 --gpu off
```

## Ablauf

Abwechselnd, master zuerst in ungeraden Runden, fünf Runden je Fall. Die
Zeit in `Masks::of` und die Zahl der Aufrufe stammen von der Uhr, die
Dauer der Basis aus der Ausgabe des Laufs, auf eine Zehntelsekunde. Die
Dauer des ganzen Laufs misst die Uhr des Messskripts. Alle Läufe vom 29.09.
In den Tabellen steht der Median, dahinter die Spanne.

Byte für Byte: beide Stände an beiden Ausschnitten mit drei nativen Stufen,
ohne und mit Karte. Alle 8564 und 2144 Kacheln sind gleich.

## Ergebnis

**Zeit in `Masks::of`**, über alle Threads summiert:

| | master | #53 | |
|---|---|---|---|
| ein Thread | 0,380 s (0,374–0,396) | 0,205 s (0,201–0,211) | −46 % |
| Stand | 8,97 s (8,09–9,85) | 4,74 s (4,48–5,01) | −47 % |
| Fichtenwald | 6,25 s (6,05–6,47) | 3,25 s (3,13–3,37) | −48 % |

Je Aufruf braucht `Masks::of` mit #53 auf einem Thread 3,7 statt 6,8 µs,
am Stand auf 24 Threads 8,9 statt 17,0 µs. Aufgerufen wird `Masks::of` je Section eines
dekodierten Chunks: auf einem Thread 55 608-mal, am Stand rund 530 000-mal,
am Fichtenwald 251 208-mal.

**Anteil an der Zeit der Basis**, die Zeit in `Masks::of` durch Threads mal
Dauer der Basis:

| | master | #53 |
|---|---|---|
| ein Thread | 5,4 % | 2,9 % |
| Stand | 6,0 % | 3,3 % |
| Fichtenwald | 12,4 % | 7,1 % |

**Der ganze Lauf:**

| | master | #53 | |
|---|---|---|---|
| ein Thread | 7,83 s (7,70–8,14) | 7,70 s (7,39–7,85) | −1,7 % |
| Stand | 7,62 s (6,74–8,54) | 7,43 s (7,01–7,72) | −2,5 % |
| Fichtenwald | 3,08 s (2,97–3,36) | 2,91 s (2,74–3,06) | −5,5 % |

Die Basis braucht auf einem Thread mit master 7,1 s und mit #53 7,0 s, am
Stand 6,2 und 6,0 s, am Fichtenwald 2,1 und 1,9 s. Am Stand liegt der
Unterschied des ganzen Laufs in der Streuung.

## Schluss

- Die Uhr um `Masks::of` streut kaum und zeigt den Gewinn klar: knapp die
  Hälfte der Zeit. Das passt zum Profil in #53, dort 5,3 → 2,7 % am Stand
  und 10,1 → 5,7 % am Fichtenwald.
- Vor #49 lag der Anteil laut dem Profil bei 2,0 und 4,9 %. Der Rest kommt
  vermutlich aus dem Lesen der gesetzten Bits und aus den drei Ebenen, die
  #49 dazugebracht hat; getrennt gemessen ist das nicht.
- Am ganzen Lauf sind es 1,7 bis 5,5 %. Auf der grossen Welt mit Streifen
  von 8 Spalten dekodiert ein Lauf seltener; gemessen ist das dort nicht.
