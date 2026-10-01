---
title: Doppelte Arbeit an Streifengrenzen
description: Wie oft die Basis einen Chunk dekodiert und sein Licht rechnet, gezählt am Stand der Testwelt mit einem Thread, der Hälfte und allen, und übertragen auf die Reihenfolge eines Vollrenders mit Streifen zu 8, 16 und 32 Spalten.
date: 2026-09-29
commits: [799c2f2, c9d2f7e]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
---

# Doppelte Arbeit an Streifengrenzen

In der Reihenfolge eines Vollrenders dekodiert die Basis bei scale 32 jeden
Chunk im Mittel 1,73-mal und rechnet sein Licht 1,31-mal; vor #49 waren es
1,56 Dekodierungen. Doppelt gearbeitet wird nur an den Grenzen der
Streifen, innerhalb eines Streifens verdrängt der Cache nichts, was er noch
braucht. Ausschnitte mit allen Threads überzeichnen das Doppelte: Das
Stück eines Threads umfasst dort höchstens gut einen Streifen, viele
Threads beginnen mitten in einem, und bei wenigen Kacheln je Thread werden
die Streifen schmaler, am Stand 4 Spalten, am Fichtenwald 2. Am Stand sind
es 2,86 Dekodierungen und 1,68 Lichtrechnungen je Chunk. Breitere
Streifen sparten gerechnet rund 4 % CPU-Zeit und kosten Speicher; ein Cache
für alle Threads hilft in dieser Reihenfolge nicht.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **vor #49:** `799c2f2`;
  - **#49:** `c9d2f7e`, der Code unter `renderer/` gleich dem Merge
    `099c456`.
- Die Testwelt, scale 32, der Stand um (-64, 416) mit `--size 18432`:
  72 × 72 = 5184 Basiskacheln, ohne native Stufen, ohne Karte.
- Ein Thread, die Hälfte der Threads und alle.
- Ein Protokoll, nur für diese Serie in beide Stände gebaut und nicht
  eingecheckt, mit Zeit und Thread:
  - jedes Dekodieren eines Chunks im Renderpfad (`ChunkCache::load`);
  - jede Lichtrechnung (`ChunkCache::licht_slot`);
  - je Kachel ihre Lage und die Chunks, die sie benutzt hat;
  - die Slots jedes Caches vor dem Aufräumen (`next_tile`).

Der Befehl, aus der Wurzel des Repositorys, für einen Thread:

```bash
RAYON_NUM_THREADS=1 renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 0 --gpu off
```

## Ablauf

Je Stand und Zahl der Threads ein Lauf am 29.09., jeder in ein frisches
Verzeichnis. Alle Zahlen kommen aus dem Protokoll:

- **Je Chunk:** Dekodierungen und Lichtrechnungen, geteilt durch die
  verschiedenen Chunks.
- **Woher die Wiederholungen kommen**, je Dekodieren eines Chunks, den
  schon ein Thread dekodiert hat:
  - derselbe Thread, ein anderer Streifen: Streifenwechsel;
  - ein anderer Thread, ein anderer Streifen: Rand zum Nachbarstreifen;
  - ein anderer Thread, derselbe Streifen: kalter Start mitten im
    Streifen;
  - derselbe Thread, derselbe Streifen: verdrängt.
- **Reihenfolge eines Vollrenders:** Er rendert alle Basiskacheln in einem
  Aufruf von `rendere`. Bei so vielen Kacheln je Thread schneidet
  `breite_der_streifen` 8 Spalten, und das zusammenhängende Stück jedes
  Threads aus `verteile` umfasst mehrere ganze Streifen, die er
  nacheinander geht. So geht auch der Lauf mit einem Thread vor. Dort wird
  im Streifen nie verdrängt, also steht jedes Dekodieren und jede
  Lichtrechnung für einen Streifen, der den Chunk braucht. Fasst man die
  Streifen so zu 16 und 32 Spalten zusammen, wie `tile.x.div_euclid` sie
  schneidet, ergeben sich die Zahlen für breitere Streifen. Gezählt sind
  nur Chunks, die keinen der zwei äussersten Streifen des Ausschnitts
  berühren: 5648 bei #49, 5781 vor #49, 4521 mit Licht. Die Grenzen der
  Stücke und das Stehlen gegen Ende lässt das Modell weg; beides kommt je
  Thread nur einige Male vor.
- **Chunks je Streifenzeile:** die Chunks, die die Kacheln einer Zeile
  eines Streifens zusammen benutzen.

## Ergebnis

**Am Ausschnitt**, je Chunk:

| Threads | Spalten je Streifen | Dekodieren vor #49 | Dekodieren #49 | Licht #49 |
|---|---|---|---|---|
| einer | 8 | 1,55 | 1,70 | 1,33 |
| die Hälfte | 8 | 1,96 | 2,09 | 1,36 |
| alle | 4 | 2,59 | 2,86 | 1,68 |

Die Spalten setzt `breite_der_streifen` aus den Kacheln je Thread; der
Fichtenwald aus [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md)
bekommt mit allen Threads 2.

Woher die Wiederholungen kommen, #49, in % aller Dekodierungen:

| Threads | Rand zum Nachbarstreifen | kalter Start | Streifenwechsel | verdrängt |
|---|---|---|---|---|
| einer | – | – | 41 % | 0 |
| die Hälfte | 32 % | 17 % | 3 % | 0 |
| alle | 47 % | 14 % | 3 % | 0 |

Vor #49 sind es mit einem Thread 36 % Streifenwechsel, mit der Hälfte und
allen 27 und 42 % Rand und 19 und 16 % kalte Starts. Beim Licht sind es mit
allen Threads 38 % Rand, 2 % kalte Starts und 1 % Streifenwechsel, mit
einem Thread 25 % Streifenwechsel.

**In der Reihenfolge eines Vollrenders**, je Chunk:

| Spalten je Streifen | Dekodieren vor #49 | Dekodieren #49 | Licht #49 | Chunks je Streifenzeile, Mittel und grösste |
|---|---|---|---|---|
| 8, heute | 1,56 | 1,73 | 1,31 | 289, 319 |
| 16 | 1,24 | 1,31 | 1,13 | 416, 531 |
| 32 | 1,08 | 1,10 | 1,04 | 575, 955 |

**Was 16 Spalten brächten**, gerechnet, nicht gemessen:

- Mit #49 wird 24 % seltener dekodiert und rund 14 % seltener Licht
  gerechnet.
- Laden samt Masken (`ChunkCache::load`) und die Ausbreitung
  (`Ausbreitung::chunk`) brauchen in einem Profil vom selben Tag 13 und
  7 % der CPU-Zeit des Prozesses: ein Thread, 676 Basiskacheln am Stand,
  samt Pyramide. Zusammen spart das rund 4 %.
- Der Cache eines Threads hält in der Spitze rund das 1,6-Fache der
  grössten Streifenzeile, mit einem Thread 516 Chunks bei 319. Bei 531
  wären es rund 860, gut 340 mehr.
- Ein Chunk im Cache kostet samt Masken und Licht höchstens rund 0,3 MB:
  die Spitze des Arbeitsspeichers mit allen Threads am Stand, 2,27 bis
  2,37 GiB in [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md),
  geteilt durch die 7858 Chunks, die hier alle Caches zusammen in der
  Spitze hielten. Das sind bis rund 100 MB mehr je Thread.

**Ohne jede doppelte Arbeit**, 1,0 je Chunk, sparte dasselbe Profil
gerechnet höchstens 7 %: 42 % des Ladens und 24 % der Ausbreitung.

## Schluss

- In der Reihenfolge eines Vollrenders kommt das Doppelte nur von den
  Streifengrenzen: 1,73 Dekodierungen und 1,31 Lichtrechnungen je Chunk
  mit #49, 1,56 Dekodierungen vor #49.
- Ausschnitte mit allen Threads überzeichnen das. Viele Threads beginnen
  dort mitten in einem Streifen, und Ausschnitte mit wenigen Kacheln je
  Thread bekommen schmalere Streifen, der Stand 4 Spalten, der
  Fichtenwald 2. Mehrkosten, die an solchen
  Ausschnitten gemessen und auf einen Vollrender hochgerechnet sind, liegen
  deshalb eher zu hoch.
- Streifen zu 16 Spalten und ein Cache für alle Threads sind verworfen,
  siehe [Der Weg einer Kachel](../renderer/renderpfad.md), „Streifen und
  Cache je Thread“.
