---
title: Native Stufen in Bändern
description: Was die Bänder aus #59 an Ausschnitten der Testwelt bringen, auf 24 Threads mit und ohne Karte und auf einem, dazu Bänder aus einer Kachel, die Spitze des Speichers und wie oft ein Chunk dekodiert und sein Licht gerechnet wird.
date: 2026-10-01
commits: [89b667b, 2f85e31]
code:
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
---

# Native Stufen in Bändern

Mit drei nativen Stufen brauchen die nativen Stufen in Bändern 30 bis
38 % weniger Zeit als je Stufe in einem eigenen Durchgang, auf 24 Threads
mit und ohne Karte wie auf einem Thread; der ganze Lauf wird 11 bis 19 %
kürzer. Jede Kachel ist Byte für Byte dieselbe. Am Stand mit 24 Threads
dekodieren die nativen Stufen einen Chunk 2,3- statt 7,8-mal und rechnen
sein Licht 1,3- statt 4,6-mal, auf einem Thread jeden Chunk einmal. Die
Spitze des Arbeitsspeichers steigt ohne Karte um bis zu 0,1 GiB, mit Karte
um 0,2 bis 0,35 GiB. Im privaten Speicher sind es mit Karte rund 0,7 GiB,
ohne Karte 0,12 GiB.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **A:** `89b667b`, vor #59, jede native Stufe in einem eigenen
    Durchgang;
  - **B:** `2f85e31`, mit #59, Bänder aus bis zu vier Kacheln;
  - **C:** B mit `BAND = 1`, nur für diese Messung;
  - **D:** B, das den Zeichner der Karte zu Beginn jedes Bands verwirft,
    nur für diese Messung;
  - **F:** B, das die gröbste Stufe auf der CPU zeichnet, nur für diese
    Messung;
  - A und B mit einem **Zähler**, nur für diese Messung und nicht
    eingecheckt, nach dem Muster von
    [2026-09-29, Licht ausbreiten](2026-09-29-licht-ausbreiten.md): Er
    zählt jedes Dekodieren eines Chunks in `ChunkCache::load`, jede
    Lichtrechnung in `ChunkCache::licht_slot` und je die verschiedenen
    Chunks, ausgegeben vor den nativen Stufen und am Ende des Laufs.
- Die Testwelt, scale 32, drei native Stufen, Pyramide:
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln, nativ
    1600, 400 und 100;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln, nativ 400, 100 und 25;
  - **Stand, klein:** um (-64, 416) mit `--size 6656`, 1024 Basiskacheln,
    nativ 256, 64 und 16, für einen Thread;
  - **Beispiel:** um (-64, 416) mit `--size 2048`, 256 Basiskacheln, nativ
    64, 16 und 4, die Beispielausgabe in
    [Kacheln exportieren](../benutzung/kacheln.md), „Ein Ausschnitt“.
- Serien:
  - **Gleich:** Stand und Fichtenwald, 24 Threads, `--gpu off`, A und B je
    ein Lauf, alle Kacheln Byte für Byte;
  - **Ohne Karte:** Stand und Fichtenwald, 24 Threads, `--gpu off`, A, B
    und C in den Folgen ABC, CBA, BCA, drei Runden;
  - **Mit Karte:** Stand und Fichtenwald, 24 Threads, `--gpu on`, A und B
    im Wechsel, drei Runden;
  - **Ein Thread:** Stand, klein, und Fichtenwald,
    `RAYON_NUM_THREADS=1`, `--gpu off`, A und B im Wechsel, drei Runden;
  - **Beispiel:** 24 Threads, `--gpu on`, A und B im Wechsel, fünf Runden;
  - **Zeichner:** Stand, 24 Threads, `--gpu on`, A, B und D in den Folgen
    ABD, DBA, BDA;
  - **Zähler:** Stand mit 24 Threads, Stand, klein, und Fichtenwald auf
    einem Thread, `--gpu off`, je ein Lauf;
  - **Speicher über die Zeit:** Stand, 24 Threads, A und B mit und ohne
    Karte, je ein Lauf, der Arbeitsspeicher alle 20 ms;
  - **Privater Speicher:** Stand, 24 Threads, A und B im Wechsel, mit Karte
    drei Runden, ohne zwei. Das Skript lief versehentlich zweimal
    hintereinander; beide Durchgänge zählen. Dazu A, B und F mit Karte in
    den Folgen ABF, FBA.
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf in ein frisches
  Verzeichnis, das vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Bis auf
  die letzte Serie lief nebenher nichts, die Last lag vor und nach jeder
  Serie bei 0 bis 5 %. Während der Serien „Speicher über die Zeit“ und
  „Privater Speicher“ baute und renderte nebenher ein anderer Prozess; sie
  messen keine Zeit.

Der Befehl, aus der Wurzel des Repositorys, für den Stand ohne Karte:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 3 --gpu off
```

## Ablauf

Die Dauer der nativen Stufen stammt aus der Ausgabe: bei A die Summe der
drei Zeilen „nativ bei scale“, bei B, C und D die Zeile darunter, die die
Zeit für alle zusammen nennt. Beide schliessen das Bauen der
Sprite-Tabellen ein. Die Dauer der Basis stammt ebenfalls aus der Ausgabe,
die des ganzen Laufs von der Uhr des Messskripts, die Spitze des
Arbeitsspeichers aus `GetProcessMemoryInfo` (`PeakWorkingSetSize`) des
beendeten Prozesses, über die Zeit aus `WorkingSetSize`, die Spitze des
privaten Speichers aus `PeakPagefileUsage`, der Spitze von
`PrivateUsage`. Alle Läufe vom
01.10. In den Tabellen steht der Median, dahinter die Spanne.

## Ergebnis

### Gleich

Am Stand 8564 Kacheln, am Fichtenwald 2144, bei A und B Byte für Byte
gleich. Der Rückfall für Blöcke, die 26.2 nicht kennt, nimmt seit #59 das
Raster der Basis; in diesen Ausschnitten ändert das keine Kachel.

### 24 Threads ohne Karte

| Ausschnitt | Stand | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|
| Stand | A | 7,4 s (7,1–7,9) | 14,1 s (13,3–14,8) | 2,33 GiB (2,29–2,36) |
| | B, Bänder aus 4 | 5,2 s (5,0–5,4), −30 % | 12,2 s (11,7–12,6), −13 % | 2,40 GiB (2,40–2,42) |
| | C, Bänder aus 1 | 6,1 s (6,0–6,3), −18 % | 14,4 s (12,4–14,8) | 1,83 GiB (1,81–1,84) |
| Fichtenwald | A | 3,4 s (3,3–4,6) | 6,1 s (6,0–7,4) | 2,00 GiB (1,95–2,03) |
| | B, Bänder aus 2 | 2,1 s (2,1–2,2), −38 % | 4,9 s (4,8–5,1), −19 % | 2,05 GiB (2,02–2,08) |
| | C, Bänder aus 1 | 2,3 s (2,2–2,7), −32 % | 5,7 s (5,1–7,6) | 2,04 GiB (2,02–2,06) |

Der Fichtenwald hat 25 Kacheln der gröbsten Stufe. Auf 24 Threads macht B
daraus Bänder aus zwei, damit 13 Threads eines haben. Die Basis ist bei
allen Ständen derselbe Code und streut am Stand von 4,9 bis 7,5 s; der
ganze Lauf streut mit ihr.

### 24 Threads mit Karte

| Ausschnitt | Stand | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|
| Stand | A | 7,4 s (6,6–7,4) | 13,2 s (12,0–13,4) | 2,82 GiB (2,80–2,82) |
| | B | 4,8 s (4,5–4,9), −35 % | 11,7 s (10,5–12,7), −11 % | 3,15 GiB (3,13–3,26) |
| Fichtenwald | A | 3,4 s (3,1–5,0) | 6,2 s (5,8–7,8) | 2,26 GiB (2,25–2,30) |
| | B | 2,1 s (2,0–2,2), −38 % | 5,0 s (4,7–5,3), −19 % | 2,60 GiB (2,54–2,63) |

Serie Zeichner, Stand: A 7,0 s (6,5–7,2) und 2,86 GiB (2,79–2,87), B 4,7 s
(4,6–4,8) und 3,17 GiB (3,14–3,19), D 4,8 s (4,6–4,9) und 3,14 GiB
(3,10–3,21).

### Ein Thread

| Ausschnitt | Stand | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|
| Stand, klein | A | 8,4 s (8,3–8,5) | 16,4 s (16,2–16,7) | 0,16 GiB |
| | B | 5,6 s (5,5–5,8), −33 % | 13,6 s (13,5–13,8), −17 % | 0,17 GiB |
| Fichtenwald | A | 16,3 s (16,3–16,4) | 30,2 s (30,1–30,4) | 0,21 GiB |
| | B | 10,7 s (10,6–10,8), −34 % | 24,6 s (24,4–24,6), −18 % | 0,25 GiB |

### Beispiel

| Stand | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|
| A | 0,7 s (0,7–0,8) | 1,5 s (1,5–1,7) | 1,36 GiB (1,34–1,37) |
| B, Bänder aus 1 | 0,6 s (0,6–0,7) | 1,4 s (1,4–1,6) | 1,20 GiB (1,19–1,20) |

Vier Kacheln der gröbsten Stufe auf 24 Threads: B macht daraus vier Bänder
aus einer Kachel. Die Läufe sind kurz, die Rate sagt wenig; langsamer wird
der kleine Ausschnitt nicht.

### Zähler

„Je Chunk“ heisst: Aufrufe über alle Threads geteilt durch die
verschiedenen Chunks des ganzen Laufs. Die nativen Stufen sind der ganze
Lauf weniger der Basis.

| Ausschnitt | Stand | Dekodierungen, Basis | Dekodierungen, native Stufen | Licht, Basis | Licht, native Stufen |
|---|---|---|---|---|---|
| Stand, 24 Threads, 9077 Chunks, 6740 mit Licht | A | 25 285, 2,79 | 71 126, 7,84 | 11 155, 1,66 | 31 066, 4,61 |
| | B | 25 490, 2,81 | 20 871, 2,30 | 11 168, 1,66 | 9 046, 1,34 |
| Stand, klein, ein Thread, 2155 Chunks, 1175 mit Licht | A | 3 094, 1,44 | 8 896, 4,13 | 1 447, 1,23 | 4 148, 3,53 |
| | B | 3 094, 1,44 | 2 153, 1,00 | 1 447, 1,23 | 1 175, 1,00 |
| Fichtenwald, ein Thread, 2712 Chunks, 1785 mit Licht | A | 4 127, 1,52 | 12 121, 4,47 | 2 216, 1,24 | 6 431, 3,60 |
| | B | 4 127, 1,52 | 2 787, 1,03 | 2 216, 1,24 | 1 780, 1,00 |

A trifft am Stand die Zählung vom 29.09., 10,6 Dekodierungen und 6,3
Lichtrechnungen je Chunk über den ganzen Lauf; B kommt auf 5,1 und 3,0.

### Speicher über die Zeit

| Stand | Basis | native Stufen | danach |
|---|---|---|---|
| A ohne Karte | 1,13 GiB | 2,37 GiB | 1,13 GiB |
| B ohne Karte | 1,11 GiB | 2,37 GiB | 1,45 GiB |
| A mit Karte | 1,61 GiB | 2,83 GiB | 2,84 GiB |
| B mit Karte | 1,60 GiB | 3,04 GiB | 1,19 GiB |

Das Höchste je Phase. Bei beiden Ständen liegt die Spitze in den nativen
Stufen: Bei scale 4 braucht eine Kachel 368 Chunks, und die Caches sind
entsprechend voll.

### Privater Speicher

| Stand, 24 Threads | Läufe | Working Set | privat |
|---|---|---|---|
| A mit Karte | 8 | 2,88 GiB (2,79–2,99) | 3,57 GiB (3,49–3,72) |
| B mit Karte | 8 | 3,15 GiB (3,08–3,22) | 4,28 GiB (4,23–4,35) |
| F mit Karte | 2 | 3,04 GiB (3,03–3,05) | 4,16 GiB (4,15–4,17) |
| A ohne Karte | 4 | 2,33 GiB (2,30–2,34) | 2,72 GiB (2,70–2,74) |
| B ohne Karte | 4 | 2,42 GiB (2,36–2,44) | 2,84 GiB (2,80–2,86) |

Mit Karte liegt B im Working Set 0,27 GiB über A, im privaten Speicher
0,71 GiB; ohne Karte 0,09 und 0,12 GiB. Zeichnet B die gröbste Stufe auf
der CPU (F), bleiben davon 0,59 GiB.

## Schluss

- **Zeit:** Am Stand und am Fichtenwald werden die nativen Stufen in
  allen Serien 30 bis 38 % kürzer, der ganze Lauf 11 bis 19 %. Der Prototyp aus #59, der nie etwas aus dem
  Vorrat warf, kam auf einem Thread auf 43 bis 51 %.
- **Bänder aus vier statt einer:** am Stand 5,2 statt 6,1 s, am Fichtenwald
  2,1 statt 2,3 s. Bänder aus einer Kachel brauchen am Stand 0,57 GiB
  weniger als aus vier und 0,5 GiB weniger als A, am Fichtenwald gleich
  viel.
- **Dekodierungen:** Am Stand mit 24 Threads bekommt jeder Thread rund ein
  Band, 100 Kacheln der gröbsten Stufe auf 24 Threads. Von Band zu Band
  teilt dort kaum ein Thread etwas, und ein Chunk wird auf den nativen
  Stufen 2,3-mal dekodiert. Auf einem Thread folgen die Bänder aufeinander
  wie im Vollrender, und jeder Chunk wird einmal dekodiert und sein Licht
  einmal gerechnet; an so kleinen Ausschnitten teilen sich auch die
  Spalten ihre Chunks. Im Vollrender sind die Spalten lang, gerechnet sind
  es dort 1,5 je Chunk, so oft, wie Spalten ihn schneiden.
- **Speicher:** Ohne Karte steigt die Spitze um bis zu 0,1 GiB, mit Karte
  um 0,2 bis 0,35 GiB, an beiden Ausschnitten gleich. Das Mehr mit Karte
  steckt auch im privaten Speicher, dort sind es 0,71 GiB; eingeblendeter
  Speicher allein ist es also nicht. Ein Zeichner je Band statt je Thread
  ändert daran nichts (D), die gröbste Stufe auf der CPU wenig (F): Die
  vielen kleinen Durchgänge der gröbsten Stufe sind es nicht. Woher es
  kommt, zeigt diese Messung nicht. Im Vollrender hält jeder Thread gerechnet bis
  zu anderthalb Bänder, rund 1300 Chunks samt Licht zu rund 65 KB, also
  bis rund 85 MB, und einen Teil davon ohnehin im Cache.
- **Vollrender, gerechnet:** Mit 33 bis 38 % weniger, wie mit Karte und
  auf einem Thread, kämen die nativen Stufen des Vollrenders der grossen
  Welt von 50 auf 31 bis 34 min, der ganze Lauf mit drei Stufen von 95 auf
  76 bis 79 min. Gemessen ist das erst mit dem nächsten Vollrender.
