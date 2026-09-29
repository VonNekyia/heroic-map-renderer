---
title: Licht ausbreiten
description: Was das Licht aus der Ausbreitung samt Licht an den Ecken kostet, im Wechsel gemessen gegen master an zwei Ausschnitten der Testwelt, mit 24 Threads und einem, mit und ohne Karte, dazu Dekodierungen, Speicher und das Vorsieben der Quellen.
date: 2026-09-29
commits: [799c2f2, 6f8a54c]
code:
  - renderer/src/render/licht.rs
  - renderer/src/render/metatile.rs
---

# Licht ausbreiten

Mit dem Licht aus der Ausbreitung (#34) schafft die Basis auf 24 Threads
23 bis 40 % weniger Kacheln je Sekunde. Ein ganzer Lauf mit drei nativen
Stufen und Pyramide braucht 51 bis 64 % länger. Gerechnet waren für die
Basis 27 % mehr Zeit, gemessen sind es 30 bis 65 %. Den grössten Teil
tragen die nativen Stufen: Jede breitet das Licht derselben Chunks noch
einmal aus und braucht damit 1,6- bis 2-mal so lange. Auf einem Thread
kostet eine Kachel 22 % mehr. Die Speicherspitze steigt um 0,5 bis
0,65 GiB, die Kacheln wachsen um 3 bis 8 %.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **master:** `799c2f2`;
  - **Licht:** `6f8a54c`, der Kopf von #49.
- Die Testwelt, scale 32:
  - **Stand:** der Ausschnitt um (-64, 416) mit `--size 18432`, 6400
    Basiskacheln;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln, alter Fichtenwald auf Podsol.
- Vier Serien:
  - **24 Threads:** beide Ausschnitte, drei native Stufen und Pyramide,
    ohne Karte (`--gpu off`) und mit (`--gpu on`), drei Runden;
  - **ein Thread:** `RAYON_NUM_THREADS=1`, 676 Basiskacheln um (-64, 416)
    mit `--size 6656`, ohne native Stufen, ohne Karte, fünf Runden;
  - **Zähler:** beide Ausschnitte wie in der ersten Serie ohne Karte, eine
    Runde, mit einem Zähler, der nur für diese Serie in beide Stände gebaut
    und nicht eingecheckt wurde. Er zählt jedes Dekodieren eines Chunks im
    Renderpfad (`ChunkCache::load`) und die verschiedenen Chunks;
  - **Vorsieben:** der Stand „Licht“ gegen denselben mit Vorsieben der
    Quellen, beide Ausschnitte wie in der ersten Serie ohne Karte, drei
    Runden.
- Jeder Lauf in ein frisches Verzeichnis, das vom Echtzeitschutz
  ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Nebenher
  lief nichts, die Last lag vor und nach jeder Serie bei 1 bis 7 %.

Der Befehl, aus der Wurzel des Repositorys, für den Stand ohne Karte:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 3 --gpu off
```

## Ablauf

Abwechselnd, master zuerst in ungeraden Runden, jeder Lauf frisch. Die
Kacheln je Sekunde und die Dauer der Basis stammen aus der Ausgabe der
Läufe, die Dauer des ganzen Laufs von der Uhr des Messskripts, die Spitze
des Arbeitsspeichers aus `GetProcessMemoryInfo` (`PeakWorkingSetSize`) des
beendeten Prozesses, die Grössen aus den Dateien auf der Platte, dezimal.
Alle Läufe vom 29.09. In den Tabellen steht der Median, dahinter die
Spanne.

## Ergebnis

**24 Threads**, Basis in Kacheln je Sekunde:

| | master | Licht | |
|---|---|---|---|
| Stand, ohne Karte | 1576 (1274–1656) | 1104 (1104–1157) | −30 % |
| Stand, mit Karte | 1739 (1551–1830) | 1340 (937–1365) | −23 % |
| Fichtenwald, ohne Karte | 1195 (1064–1232) | 719 (481–759) | −40 % |
| Fichtenwald, mit Karte | 1257 (1124–1302) | 790 (784–797) | −37 % |

**24 Threads**, der ganze Lauf mit drei nativen Stufen und Pyramide:

| | master | Licht | |
|---|---|---|---|
| Stand, ohne Karte | 9,98 s (9,88–10,94) | 15,53 s (15,19–15,65) | +56 % |
| Stand, mit Karte | 9,63 s (9,57–9,91) | 14,50 s (14,43–16,29) | +51 % |
| Fichtenwald, ohne Karte | 4,65 s (4,40–4,69) | 7,14 s (7,10–8,33) | +54 % |
| Fichtenwald, mit Karte | 4,22 s (4,22–4,36) | 6,93 s (6,84–7,09) | +64 % |

Ein Lauf je Stand, Stand ohne Karte, nach Stufen aus der Ausgabe:

| | master | Licht |
|---|---|---|
| Basis, scale 32, 6400 Kacheln | 3,8 s | 5,9 s |
| Zoom 9, scale 16, 1600 Kacheln | 2,1 s | 3,4 s |
| Zoom 8, scale 8, 400 Kacheln | 1,3 s | 2,6 s |
| Zoom 7, scale 4, 100 Kacheln | 1,1 s | 2,1 s |

**Ein Thread**, 676 Basiskacheln, fünf Runden:

| | master | Licht | |
|---|---|---|---|
| Lauf | 6,15 s (5,98–6,16) | 7,39 s (7,32–7,40) | +20 % |
| Basis | 5,5 s | 6,7 s | |
| je Kachel | 8,14 ms (7,84–8,14) | 9,91 ms | +22 % |
| Spitze | 86 MB | 104 bis 109 MB | |

Die Ausgabe nennt die Dauer der Basis auf eine Zehntelsekunde, die Zeit je
Kachel also auf 0,15 ms genau.

**Speicher und Grösse**, 24 Threads:

| | master | Licht |
|---|---|---|
| Spitze, Stand, ohne Karte | 1,74–1,80 GiB | 2,27–2,37 GiB |
| Spitze, Stand, mit Karte | 2,05–2,22 GiB | 2,84–2,87 GiB |
| Spitze, Fichtenwald, ohne Karte | 1,49–1,56 GiB | 1,94–2,07 GiB |
| Spitze, Fichtenwald, mit Karte | 1,85–1,87 GiB | 2,26–2,37 GiB |
| Stand, alle Dateien | 568,8 MB | 584,2 MB, +2,7 % |
| Fichtenwald, alle Dateien | 187,0 MB | 202,3 MB, +8,2 % |
| Stand, je Basiskachel | 65 kB | 67 kB |
| Fichtenwald, je Basiskachel | 89 kB | 95 kB |

Die AO-Karten der Testwelt fallen dabei kaum ins Gewicht: Ihre Sprite-Tabelle
hat bei scale 32 2134 Sprites. Das Mehr an der Spitze passt zum Licht, das
jeder der 24 Threads je Chunk in seinem Cache hält (`ChunkLicht`): 4 KB je
Section, in der nicht jede Zelle dasselbe Licht hat.

**Dekodierungen** im Renderpfad, über alle Stufen, eine Runde:

| | master | Licht |
|---|---|---|
| Stand | 87 556 für 8936 Chunks, 9,80 je Chunk | 96 616 für 9077 Chunks, 10,64 je Chunk |
| Fichtenwald | 40 650 für 2648 Chunks, 15,35 je Chunk | 44 078 für 2712 Chunks, 16,25 je Chunk |

Das Licht eines Chunks braucht seine acht Nachbarn. Am Rand eines Streifens
kommt damit ein zweiter Ring dazu, siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Streifen und Cache je
Thread“. Er kostet 6 bis 9 % mehr Dekodierungen und erklärt den Rest nicht.

**Vorsieben der Quellen:** Eine Quelle, deren Stufe nicht bis an den Chunk in
der Mitte reicht, nimmt die Ausbreitung gar nicht erst auf. Das ändert
keine Kachel, alle 2153 des Fichtenwalds bleiben Byte für Byte gleich:

| | ohne | mit |
|---|---|---|
| Stand, Basis, Kacheln/s | 1168 (1154–1261) | 1067 (991–1190) |
| Fichtenwald, Basis, Kacheln/s | 773 (712–783) | 776 (769–796) |

Das liegt in der Streuung; das Vorsieben ist deshalb nicht im Code.

## Schluss

Das Licht kostet mehr als gerechnet, und das liegt am Rechnen, nicht am
Lesen:
- Die Ausbreitung rechnet für jeden Chunk ein Fenster aus 44 × 44 Spalten,
  mehr als das Siebenfache seiner eigenen.
- Jede native Stufe rechnet das Licht derselben Chunks noch einmal, und
  auf den groben Stufen, wo das Zeichnen wenig kostet, überwiegt es.
- Auf 24 Threads kostet es mehr als auf einem: ohne Karte 43 % mehr Zeit
  je Basiskachel statt 22 %. Ob das am Licht liegt, das jeder Thread für
  die Chunks am Rand seines Streifens und ihren zweiten Ring selbst
  rechnet, oder an Speicher und Cache, ist nicht getrennt gemessen.

Hochgerechnet auf einen Vollrender der grossen Welt, gemessen ist das
nicht:
- **Vorgabe, ohne native Stufen, mit Karte:** Zuletzt brauchte er 66 min,
  39 min Basis und 26 min Pyramide, siehe
  [2026-09-27, Vollrender mit #21](2026-09-27-vollrender-mit-21.md). Die
  Pyramide rechnet kein Licht. Mit den Raten der Basis mit Karte, 23 und
  37 % weniger Kacheln je Sekunde, braucht die Basis 51 bis 62 min und der
  Lauf rund 78 bis 89 min, 12 bis 23 min mehr.
- **Mit drei nativen Stufen:** Dort gelten die 51 bis 64 % des ganzen
  Laufs. Entscheidung:
[0040](../entscheidungen/0040-licht-selbst-ausbreiten.md).
