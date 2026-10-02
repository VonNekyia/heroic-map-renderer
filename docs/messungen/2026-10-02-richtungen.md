---
title: Richtungen
description: Was die Drehung der Welt aus #68 kostet, an Stand und Fichtenwald der Testwelt, ohne und mit Karte, in 2:1 aus se gegen nw und aus der Vorgabe gegen #86, und dass se und s Byte für Byte gleich bleiben.
date: 2026-10-02
commits: [0fe7a65, cf8ce5c]
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
---

# Richtungen

Aus der Vorgabe ändert die Drehung der Welt kein Byte: In 2:1 und in
`north-45` sind an Stand und Fichtenwald alle Kacheln gleich. Zeit und
Spitze liegen im Wechsel gegen #86 in der Streuung, ohne wie mit Karte:
Der `match` je Zugriff ist nicht zu sehen. Aus `nw` kostet 2:1 so viel wie
aus `se`: Die Basis liegt im Median bis 0,2 s darüber oder darunter, die
Spannen überlappen, die Spitze liegt in der Streuung. Die Kacheln aus `nw`
sind am Stand 3 % schwerer, im Fichtenwald 14 % leichter; die Kamera sieht
dort die Nord- und Westseiten derselben Spalten.

## Aufbau

- Stände, Release-Build:
  - **A:** `0fe7a65`, #86, die Ablage ohne Drehung, gleich master
    `703d4d2`;
  - **B:** `cf8ce5c`, #87 mit der Drehung. Die Runde des Reviews danach
    ändert in `renderer/src` ausser Unit-Tests nur ein `debug_assert`; der
    Release-Build rechnet wie B.
- Die Testwelt, `--native-levels 3`, Pyramide, 24 Threads, 2:1 bei scale 32
  mit nativen Stufen bei 16, 8 und 4, `north-45` bei scale 16 mit 8 und 4.
  - **Stand:** um (-64, 416) mit `--size 18432`, wie in
    [2026-10-01, Kameras](2026-10-01-kameras.md);
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, ebenso.
- **Dieselben Spalten aus `nw`:** `--center` nennt eine Spalte der Welt.
  Aus `nw` liegt das Fenster deshalb um dieselbe Mitte, halb gedreht, und
  deckt dieselben Spalten wie aus `se`, bis auf die Ränder. Beide haben
  gleich viele Basiskacheln. Die Kamera sieht aber die andere Seite: Nord-
  und Westwände statt Süd- und Ostwänden, andere Schatten, anderes Wasser
  im Blick.
- Serien:
  - **Gleich:** A und B aus der Vorgabe je ein Lauf, 2:1 bei 32 und
    `north-45` bei 16, `--gpu off`, alle Kacheln per SHA-256 verglichen;
  - **se gegen nw:** B in 2:1 bei 32 aus `se` und `nw` im Wechsel, ohne
    und mit Karte, drei Runden, die Reihenfolge je Runde umgekehrt;
  - **gegen #86:** A und B in 2:1 bei 32 aus `se` im Wechsel, ohne und mit
    Karte, drei Runden, die Reihenfolge je Runde umgekehrt.
- Nach jedem Lauf wird sein Baum gelöscht und dann 15 s gewartet, siehe
  [2026-10-02, Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md).
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf ging in eine
  frische Wurzel, die vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Last: Nebenher liefen keine Builds, Tests oder Messungen.
  - Vor dem Start über 20 s im Mittel 7,7 %. Davor lag sie eine halbe
    Stunde bei 12 bis 18 %, die Messung wartete so lange.
  - Gleich: direkt davor 5 %, danach 0 %.
  - se gegen nw: direkt davor 5 %, danach 9 %.
  - gegen #86: direkt davor 8 %, danach 25 %.

Der Befehl, aus der Wurzel des Repositorys, für den Fichtenwald aus `nw`
ohne Karte:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <wurzel> --center -2712 -3297 --size 8192 --scale 32 --native-levels 3 --gpu off --camera 2:1 --direction nw
```

## Ablauf

Die Dauer der Basis stammt aus der Ausgabe des Laufs, auf 0,1 s genau; bei
2,0 s im Fichtenwald sind das allein ±5 %. Die Dauer des ganzen Laufs misst
das Messskript. Die Spitze des Speichers ist `PeakWorkingSetSize` des
Prozesses. Kacheln und Bytes sind die Dateien des Baums, Bytes dezimal.
In den Tabellen steht der Median, dahinter die Spanne. Gemessen am
02.10.2026.

## Ergebnis

### Gleich

| Kamera | Ausschnitt | Kacheln A | Kacheln B | anders |
|---|---|---|---|---|
| 2:1 bei 32 | Stand | 8564 | 8564 | 0 |
|  | Fichtenwald | 2144 | 2144 | 0 |
| `north-45` bei 16 | Stand | 7326 | 7326 | 0 |
|  | Fichtenwald | 1745 | 1745 | 0 |

### se gegen nw

| Karte | Ausschnitt | Richtung | Basiskacheln | Basis | ganzer Lauf | Spitze | Kacheln | Bytes |
|---|---|---|---|---|---|---|---|---|
| ohne | Stand | se | 6400 | 5,9 s (5,3–6,2) | 12,8 s (12,1–13,8) | 2,37 GiB (2,27–2,40) | 8564 | 586,7 MB |
|  |  | nw | 6400 | 6,0 s (5,3–6,1) | 13,1 s (12,4–13,5) | 2,36 GiB (2,36–2,42) | 8564 | 602,9 MB |
|  | Fichtenwald | se | 1600 | 2,2 s (2,0–2,6) | 6,1 s (5,2–6,2) | 1,93 GiB (1,87–2,01) | 2144 | 202,3 MB |
|  |  | nw | 1600 | 2,0 s (2,0–2,2) | 5,3 s (5,2–5,6) | 2,04 GiB (2,00–2,05) | 2144 | 174,0 MB |
| mit | Stand | se | 6400 | 5,0 s (4,9–5,1) | 12,1 s (12,0–12,2) | 3,19 GiB (3,13–3,22) | 8564 | 586,7 MB |
|  |  | nw | 6400 | 4,9 s (4,9–5,2) | 11,9 s (11,9–12,2) | 3,23 GiB (3,13–3,31) | 8564 | 602,9 MB |
|  | Fichtenwald | se | 1600 | 2,0 s (2,0–2,0) | 5,4 s (5,3–5,5) | 2,54 GiB (2,53–2,56) | 2144 | 202,3 MB |
|  |  | nw | 1600 | 1,9 s (1,9–2,0) | 5,4 s (5,4–5,5) | 2,53 GiB (2,50–2,54) | 2144 | 174,0 MB |

Ohne Karte stieg die Basis am Stand über die Runden, aus beiden Richtungen
gleich: in Runde 1 je 5,3 s, in Runde 2 und 3 5,9 bis 6,2 s.

### gegen #86

| Karte | Ausschnitt | Stand | Basis | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| ohne | Stand | A | 6,0 s (6,0–6,7) | 13,6 s (13,3–14,3) | 2,38 GiB (2,33–2,39) |
|  |  | B | 5,9 s (5,7–6,0) | 13,5 s (13,1–13,5) | 2,37 GiB (2,32–2,37) |
|  | Fichtenwald | A | 2,2 s (2,1–2,4) | 5,7 s (5,7–5,9) | 1,99 GiB (1,98–2,00) |
|  |  | B | 2,3 s (2,2–2,4) | 5,7 s (5,7–5,8) | 1,99 GiB (1,97–1,99) |
| mit | Stand | A | 5,0 s (5,0–5,0) | 12,3 s (12,2–12,4) | 3,20 GiB (3,19–3,21) |
|  |  | B | 4,9 s (4,9–5,0) | 12,2 s (12,1–12,2) | 3,21 GiB (3,21–3,26) |
|  | Fichtenwald | A | 2,0 s (2,0–2,0) | 5,6 s (5,5–5,7) | 2,57 GiB (2,52–2,58) |
|  |  | B | 2,0 s (2,0–2,3) | 5,6 s (5,5–5,7) | 2,55 GiB (2,54–2,56) |

Kacheln und Bytes wie in „Gleich“, in jedem Lauf.

## Schluss

- **Aus der Vorgabe ändert sich nichts.** Die Kacheln sind Byte für Byte
  gleich, und Zeit und Spitze liegen im Wechsel gegen #86 in der
  Streuung: die Basis am Stand 6,0 gegen 5,9 s ohne und 5,0 gegen 4,9 s mit
  Karte, im Fichtenwald 2,2 gegen 2,3 s und 2,0 gegen 2,0 s. Eine Tabelle
  ohne Sprung für die Drehung braucht es nicht.
- **Aus `nw` kostet 2:1 so viel wie aus `se`.** Die Basis liegt am Stand
  bei 6,0 gegen 5,9 s ohne und 4,9 gegen 5,0 s mit Karte, im Fichtenwald
  bei 2,0 gegen 2,2 s und 1,9 gegen 2,0 s; die Spannen überlappen.
- **Die Grösse hängt an der Seite, die man sieht.** Aus `nw` zeigen
  dieselben Spalten ihre Nord- und Westseiten: am Stand 602,9 statt
  586,7 MB, im Fichtenwald 174,0 statt 202,3 MB.
- **Nicht gemessen:** `sw` und `ne`, die übrigen Kameras aus anderen
  Richtungen und die grosse Welt. Ihr Weg ist derselbe; nur `sw` und `ne`
  zeigen die Spalten um eine Vierteldrehung, dort deckt das Fenster andere
  Spalten.
