---
title: Stufen am Baum
description: Volle Läufe und Updates mit maximal_compact, good und small auf der Testwelt, von oben mit Relief und im iso mit Cinematic (#421); good braucht von oben 64,9 % der Bytes und im iso 52,0 %, small von oben 52,0 %; ein voller Lauf dauert 0,6 bis 3,7 % länger, ein Update mit einem Thread gleich lang.
date: 2026-11-17
commits: [0aab8539]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# Stufen am Baum

Gegen `maximal_compact` braucht ein Baum mit `good` von oben mit Relief
64,9 % der Bytes und im iso mit Cinematic 52,0 %. Mit `small` sind es von
oben 52,0 %. Die Basis allein kommt von oben auf 60,5 und 45,9 %, im iso
auf 51,9 %; das Relief ist in jeder Stufe gleich gross. Ein voller Lauf
dauert an der Wanduhr mit `good` 0,6 % länger von oben und 3,4 % im iso,
mit `small` von oben 3,7 %. Ein Update mit einem Thread braucht in jeder
Stufe gleich lang.

## Aufbau

- **Stand:** master `0aab8539`, v0.21.0, Release-Build ohne Profiler.
- **Welt:** eine Kopie der Testwelt mit 316 223 Chunks, davon 249 103 fertig
  erzeugt und gezeichnet. Das Original wurde nur gelesen.
- **Stufen:** `maximal_compact` als Vergleich, `good` und `small`, siehe
  [map.json](../benutzung/map-json.md), „Die Stufe“. Im iso ohne `small`,
  damit das Fenster kurz bleibt. `minimal` und `maximal` sind nicht
  gemessen.
- **Läufe** wie in [Renderzeit je Funktion, 15.11.](2026-11-15-renderzeit-je-funktion.md),
  mit `--quality <stufe>` statt `--compact`:

  | Lauf | Schalter | Threads |
  |---|---|---|
  | von oben, voll | `--camera top-north --scale 4 --relief slope --gpu off --quality <stufe>` | alle |
  | Update von oben, dreimal | dieselben, dazu `--threads 1 --low-priority --update` | einer |
  | iso, voll | `--camera 4:3 --direction se --cinematic --scale 24 --quality <stufe>` | alle |
  | Update iso, dreimal | dieselben, dazu `--threads 1 --low-priority --update` | einer |

- **Volle Läufe** je in einen leeren Baum. Von oben zwei Durchgänge,
  `maximal_compact`, `good`, `small` und dann umgekehrt; im iso einer,
  `maximal_compact` vor `good`.
- **Updates** in den Bäumen des ersten Durchgangs. Vor jeder Runde ist in 16
  über die Welt verteilten, fertig erzeugten Chunks der oberste feste Block
  einer Spalte ersetzt oder zurückgesetzt, mit neuem Stempel, in der Kopie.
  Alle Bäume einer Runde sehen dieselbe Änderung; die Reihenfolge der
  Stufen dreht sich je Runde.
- **Echtzeitschutz:** nicht eigens geprüft, siehe
  [Echtzeitschutz](../benutzung/echtzeitschutz.md).

## Ablauf

- **Ruhe:** am 17.11.2026 von 21:44 bis 22:52 unter `messung.lock`, samt
  Build und Kopie der Welt.
  - Es lief kein Spiel.
  - Ein Lauf durfte erst bei höchstens 15 % Last starten. Vor den Läufen
    lag sie bei 0 bis 15 %. Über 10 % lag sie vor dem ersten vollen Lauf
    mit `good` von oben (15 %) und vor dem dritten Update im iso mit
    `maximal_compact` (13 %).
  - Zwischen zwei Läufen 15 s Pause. Kein Baum wurde während der Reihe
    gelöscht.
- **Quelle,** am 17.11.2026:
  - Wanduhr und CPU-Zeit, Kernel und Nutzer des Prozesses: das Messskript;
  - Kacheln und Dauer der Phasen: die Ausgabe der Läufe;
  - Bytes: die Grössen der Dateien nach dem letzten Lauf, dezimal.

## Ergebnis

### Bytes

Von oben, die Bäume des zweiten Durchgangs, also nur der volle Lauf. Die
Bäume des ersten Durchgangs nach drei Updates weichen davon um höchstens
2403 B ab.

| Teil | `maximal_compact` | `good` | `small` |
|---|---|---|---|
| Basis, Zoom 8, 16 127 Kacheln | 641 363 466 B | 388 025 220 B, 60,50 % | 294 657 950 B, 45,94 % |
| Pyramide, Zoom 0 bis 7, 5910 Kacheln | 265 412 524 B | 162 301 788 B, 61,15 % | 124 614 820 B, 46,95 % |
| Relief mit Rest, 34 759 Dateien | 84 895 232 B | gleich | gleich |
| Stand, Höhen, Pixel, `map.json` | 22 901 955 B | gleich bis auf 30 B | gleich bis auf 29 B |
| zusammen | 1,015 GB | 0,658 GB, 64,87 % | 0,527 GB, 51,95 % |

Im iso, die Bäume nach den drei Updates:

| Teil | `maximal_compact` | `good` |
|---|---|---|
| Basis, Zoom 10, 226 277 Kacheln | 9 571 128 412 B | 4 970 528 534 B, 51,93 % |
| Pyramide, Zoom 0 bis 9, 77 684 Kacheln | 4 721 065 800 B | 2 439 279 508 B, 51,67 % |
| Stand, Höhen, Pixel, `map.json` | 31 369 891 B | gleich bis auf 30 B |
| zusammen | 14,324 GB | 7,441 GB, 51,95 % |

### Voller Lauf

Von oben, alle Threads:

| Stufe | Wanduhr 1 / 2 | Mittel | gegen `maximal_compact` | CPU-Zeit 1 / 2 | Mittel | gegen `maximal_compact` |
|---|---|---|---|---|---|---|
| `maximal_compact` | 115,5 / 117,1 s | 116,3 s | – | 2260,5 / 2301,5 s | 2281,0 s | – |
| `good` | 116,3 / 117,7 s | 117,0 s | +0,6 % | 2345,9 / 2358,3 s | 2352,1 s | +3,1 % |
| `small` | 120,1 / 121,1 s | 120,6 s | +3,7 % | 2344,8 / 2344,8 s | 2344,8 s | +2,8 % |

- **Drift:** Der zweite Durchgang ist bei jeder Stufe 0,8 bis 1,4 %
  langsamer als der erste. Die Reihenfolge war umgekehrt, im Mittel hebt
  sich das auf.
- **Basis** laut Ausgabe: `maximal_compact` 82,7 und 86,5 s, `good` 85,9 und
  86,9 s, `small` 87,9 und 90,5 s. Die Pyramide brauchte in jeder Stufe
  7,5 bis 8,4 s.

Im iso mit Cinematic, alle Threads, ein Lauf je Stufe:

| Stufe | Wanduhr | gegen `maximal_compact` | CPU-Zeit | gegen `maximal_compact` | Basis laut Ausgabe |
|---|---|---|---|---|---|
| `maximal_compact` | 966,9 s | – | 20 769,0 s | – | 941,5 s |
| `good` | 999,6 s | +3,4 % | 21 937,6 s | +5,6 % | 974,1 s |

`good` lief nach `maximal_compact`. Mit einer Drift wie von oben steckt
rund ein Punkt der 3,4 % darin.

### Update mit einem Thread

Wanduhr je Runde; die CPU-Zeit liegt je Update 0 bis 1,7 s darunter.

| Stufe | von oben 1 / 2 / 3 | Mittel | iso 1 / 2 / 3 | Mittel |
|---|---|---|---|---|
| `maximal_compact` | 5,4 / 5,2 / 6,1 s | 5,57 s | 72,3 / 80,0 / 78,6 s | 76,97 s |
| `good` | 5,4 / 5,3 / 6,1 s | 5,60 s | 72,7 / 71,8 / 86,4 s | 76,97 s |
| `small` | 5,3 / 5,2 / 5,5 s | 5,33 s | – | – |

- **Reihenfolge** von oben: Runde 1 `maximal_compact`, `good`, `small`;
  Runde 2 `good`, `small`, `maximal_compact`; Runde 3 `small`,
  `maximal_compact`, `good`. Im iso: `maximal_compact`, `good`; dann
  umgekehrt; dann wieder `maximal_compact` zuerst.
- **Kacheln von oben:** Jedes Update schrieb 16 der 72 gezeichneten
  Kacheln neu, 56 blieben gleich. Die Pyramide baute 52 Eltern neu, mit
  `small` 54, je eine mehr auf Zoom 4 und 3; warum, ist nicht untersucht.
- **Kacheln im iso:** Jedes Update schrieb 17 der 2315 gezeichneten Kacheln
  neu, 2254 blieben gleich, 44 waren leer. Die Pyramide baute 636 Eltern
  neu, mit `good` 638.

## Schluss

- **Die Bytes:** `good` spart gegen `maximal_compact` von oben 35 % des
  Baums und im iso 48 %, `small` von oben 48 %. Von oben spart es weniger,
  weil das Relief rund 8 % des Baums mit `maximal_compact` ausmacht und in
  jeder Stufe verlustfrei bleibt; an der Basis allein sind es 39,5 und
  54,1 %.
- **Gegen die Stichprobe:** An den 563 Kacheln aus
  [Farben zusammenlegen](2026-11-15-farben-zusammenlegen.md), „Ohne Rückfall
  mit 6 bis 16“, kam die feinste Stufe mit 2 von oben auf 56,1 % und im
  iso mit Cinematic auf 47,2 %, mit 4 von oben auf 41,8 %. Der Baum liegt
  an der Basis 4,1 bis 4,7 Punkte darüber.
- **Die Zeit:** Ein voller Lauf dauert an der Wanduhr mit `good` 0,6 % länger
  von oben und 3,4 % im iso, mit `small` 3,7 % von oben; an CPU-Zeit
  sind es 3,1, 5,6 und 2,8 %. Ob von oben `good` oder `small` mehr kostet,
  zeigen die zwei Durchgänge nicht: an der Wanduhr `small`, an CPU-Zeit
  `good`. Ein Update mit einem Thread, wie im Plugin,
  braucht in jeder Stufe gleich lang: Die Unterschiede liegen in der
  Streuung von rund 20 % zwischen den Runden.
- **Einordnung:** `maximal_compact` braucht hier für den vollen Lauf
  von oben 116,3 s und im iso 966,9 s, für das Update im iso im Mittel
  77,0 s. [Renderzeit je Funktion, 15.11.](2026-11-15-renderzeit-je-funktion.md)
  mass mit `--compact` unter dem Profiler 115,1, 980,4 und 74,2 s.
- **Grenzen:**
  - Im iso ein voller Lauf je Stufe, ohne `small`.
  - `minimal` und `maximal` sind nicht gemessen, die grosse Welt nicht.
  - Alles lief unter Windows.
