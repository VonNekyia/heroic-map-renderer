---
title: Cinematic, Phase 1
description: Was Cinematic aus #72 an Stand und Fichtenwald der Testwelt gegen die Karte kostet, an Zeit, Spitze und Grösse der Kacheln, und dass die Karte mit #72 Byte für Byte gleich und gleich schnell bleibt, ohne und mit Grafikkarte.
date: 2026-10-03
commits: [a15af05, d83a41f]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
---

# Cinematic, Phase 1

Cinematic aus #72 kostet an Stand und Fichtenwald das 1,11- bis 1,21-Fache
der Karte ohne Grafikkarte im ganzen Lauf. Die Basis braucht das 1,11- bis
1,24-Fache, die nativen Stufen das 1,20- bis 1,29-Fache. Die Kacheln sind
7 bis 9 % leichter, die Spitze des Speichers liegt bis 5 % höher. Die Karte
bleibt mit #72 Byte für Byte gleich. Ohne und mit Karte liegen ihre Zeiten
gegen master in der Streuung; nur die nativen Stufen am Stand ohne Karte
lagen in allen drei Runden 0,1 bis 0,3 s über master.

## Aufbau

- Stände, Release-Build:
  - **A:** `a15af05`, master. In `renderer/` gleich der Basis des Zweigs
    `f843a53`;
  - **B:** `d83a41f`, #72 nach der ersten Runde des Reviews, mit der
    Umgebungsfarbe, dem Licht je Ecke und der rohen Tönung. Danach ändert
    die PR in `renderer/` nichts mehr.
- Die Testwelt, 2:1 bei scale 32 aus `se`, `--native-levels 3` mit
  nativen Stufen bei 16, 8 und 4, Pyramide, 24 Threads; dieselben
  Ausschnitte wie in [2026-10-02, Richtungen](2026-10-02-richtungen.md):
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln.
- Serien:
  - **Gleich:** A und B je ein Lauf mit der Karte, `--gpu off`, alle
    Kacheln per SHA-256 verglichen;
  - **Karte:** A und B im Wechsel, ohne (`--gpu off`) und mit Karte
    (`--gpu auto`), drei Runden, die Reihenfolge in Runde 2 umgekehrt;
  - **Cinematic:** B ohne und mit `--cinematic` im Wechsel, beide
    `--gpu off`, denn Cinematic zeichnet immer die CPU; drei Runden, die
    Reihenfolge in Runde 2 umgekehrt.
- Nach jedem Lauf wird sein Baum gelöscht und dann 15 s gewartet, siehe
  [2026-10-02, Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md).
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf ging in eine
  frische Wurzel, die vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Last:
  - Vor jedem Lauf über 5 s im Mittel 1,0 bis 9,8 %; kein Lauf musste
    warten.
  - Vor und nach jeder Serie über 20 s: Gleich 8,3 und 4,4 %, Karte 2,8
    und 2,3 %, Cinematic 2,5 und 2,5 %.
  - In der ersten Minute von Gleich lief noch eine Auswertung einer anderen
    Sitzung mit rund 20 % Last. Die Zeiten dieser Serie zählen deshalb
    nicht, nur ihre Kacheln.

Der Befehl, aus der Wurzel des Repositorys, für den Stand mit Cinematic:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <wurzel> --center -64 416 --size 18432 --scale 32 --native-levels 3 --gpu off --cinematic
```

## Ablauf

- **Aus der Ausgabe des Laufs,** auf 0,1 s genau: die Dauer der Basis
  (`Kacheln:`), der nativen Stufen (die Zeile „in Bändern“) und der
  Pyramide. Bei 1,8 s im Fichtenwald sind 0,1 s schon 6 %.
- **Vom Messskript:** die Dauer des ganzen Laufs und die Spitze des
  Speichers, `PeakWorkingSetSize` des Prozesses.
- **Aus den Dateien des Baums:** Kacheln und Bytes, dezimal; die Basis ist
  die feinste Stufe.
- **In den Tabellen** steht der Median, dahinter die Spanne.

Gemessen am 03.10.2026.

## Ergebnis

### Gleich

| Ausschnitt | Kacheln A | Kacheln B | anders |
|---|---|---|---|
| Stand | 8564 | 8564 | 0 |
| Fichtenwald | 2144 | 2144 | 0 |

### Karte gegen master

| Karte | Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|---|
| ohne | Stand | A | 5,0 s (4,8–5,1) | 5,0 s (5,0–5,2) | 11,31 s (11,17–11,60) | 2,41 GiB (2,38–2,44) |
|  |  | B | 5,0 s (5,0–5,0) | 5,3 s (5,2–5,3) | 11,50 s (11,49–11,57) | 2,39 GiB (2,39–2,43) |
|  | Fichtenwald | A | 1,8 s (1,8–1,8) | 2,0 s (2,0–2,0) | 4,73 s (4,70–4,78) | 2,07 GiB (2,05–2,09) |
|  |  | B | 1,8 s (1,8–1,9) | 2,1 s (2,0–2,1) | 4,81 s (4,81–4,82) | 2,09 GiB (2,08–2,10) |
| mit | Stand | A | 4,2 s (4,0–4,2) | 4,8 s (4,8–4,9) | 10,47 s (10,45–10,51) | 3,23 GiB (3,19–3,23) |
|  |  | B | 4,2 s (4,1–4,2) | 4,9 s (4,8–5,0) | 10,48 s (10,41–10,61) | 3,17 GiB (3,16–3,27) |
|  | Fichtenwald | A | 1,6 s (1,6–1,7) | 2,0 s (2,0–2,0) | 4,68 s (4,62–4,69) | 2,67 GiB (2,67–2,69) |
|  |  | B | 1,7 s (1,7–1,7) | 1,9 s (1,9–2,0) | 4,64 s (4,61–4,73) | 2,69 GiB (2,68–2,72) |

- **Je Runde,** B durch A: Am Stand ohne Karte im ganzen Lauf 1,03, 0,99
  und 1,02, die nativen Stufen 5,2 gegen 5,0, 5,3 gegen 5,2 und 5,3 gegen
  5,0 s. Im Fichtenwald ohne Karte im ganzen Lauf 1,02, 1,02 und 1,01. Mit
  Karte 0,99 bis 1,01.
- **Kacheln und Bytes** in jedem Lauf wie in „Gleich“: am Stand 586,7 MB,
  davon 439,1 MB Basis, im Fichtenwald 202,3 MB, davon 155,9 MB Basis.

### Cinematic gegen die Karte

Beide B, ohne Karte.

| Ausschnitt | look | Basis | native Stufen | ganzer Lauf | Spitze | Bytes | Basis |
|---|---|---|---|---|---|---|---|
| Stand | Karte | 4,9 s (4,9–4,9) | 5,0 s (4,9–5,1) | 11,23 s (11,23–11,29) | 2,40 GiB (2,39–2,40) | 586,7 MB | 439,1 MB |
|  | Cinematic | 5,9 s (5,8–5,9) | 6,3 s (6,1–6,3) | 13,41 s (13,41–13,57) | 2,51 GiB (2,46–2,53) | 535,8 MB | 404,3 MB |
| Fichtenwald | Karte | 1,8 s (1,7–1,8) | 2,0 s (2,0–2,0) | 4,71 s (4,67–4,78) | 2,12 GiB (2,10–2,12) | 202,3 MB | 155,9 MB |
|  | Cinematic | 2,0 s (2,0–2,1) | 2,5 s (2,4–2,5) | 5,33 s (5,32–5,39) | 2,10 GiB (2,08–2,11) | 187,8 MB | 145,2 MB |

Je Runde, Cinematic durch die Karte:

| Ausschnitt | ganzer Lauf | Basis | native Stufen | Spitze | Bytes |
|---|---|---|---|---|---|
| Stand | 1,19 bis 1,21 | 1,18 bis 1,20 | 1,20 bis 1,29 | 1,03 bis 1,05 | 0,913 |
| Fichtenwald | 1,11 bis 1,14 | 1,11 bis 1,24 | 1,20 bis 1,25 | 0,99 | 0,928 |

Gegen die Karte mit Grafikkarte aus „Karte gegen master“ braucht Cinematic
im ganzen Lauf am Stand das 1,28-Fache, im Fichtenwald das 1,15-Fache.

## Schluss

- **Die Karte bleibt gleich.** Alle Kacheln sind Byte für Byte die von
  master, an beiden Ausschnitten.
- **Die Karte bleibt gleich schnell,** in der Streuung: Die Basis liegt im
  Median gleich, mit Karte auch der ganze Lauf. Ohne Karte liegen die
  nativen Stufen am Stand in allen drei Runden 0,1 bis 0,3 s über master,
  die Spannen berühren sich (5,0–5,2 gegen 5,2–5,3 s). Die einzige neue
  Arbeit der Karte, je Aufruf von `licht_fuer` und `ecken_at` die Frage, ob
  Cinematic zeichnet, läuft mit Karte ebenso und zeigt dort nichts.
- **Cinematic kostet in Phase 1 rund ein Fünftel mehr,** am Stand mehr als
  im Fichtenwald. Dazu kommen das Licht je Ecke in HDR, die Farbe des
  Himmels je Block und der Ton am Ende. Die Sonne aus #73 kommt dazu, siehe
  [0056](../entscheidungen/0056-exakter-strahl-zur-sonne.md).
- **Die Kacheln sind leichter,** 7 bis 9 %: Ohne Sonne und ohne
  Schattierung nach Richtung sind sie dunkler und gleichmässiger.
- **Nicht gemessen:** die grosse Welt, andere Kameras und Richtungen.
  Cinematic zeichnet dasselbe Raster, nur das Licht je Pixel ist ein
  anderes.
