---
title: Flächen zu gleichen Nachbarn
description: Was die Regel aus #58 an Kacheln ändert und kostet, am Stand und an einer Eisszene der Testwelt, im Wechsel gegen master, mit und ohne Karte, dazu Sprites, Fassungen und die Spitze des Speichers.
date: 2026-10-01
commits: [bb5b5d8, 81b84e8]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# Flächen zu gleichen Nachbarn

Die Regel aus #58 kostet keine messbare Zeit. Am Stand wie an einer
Eisszene der Testwelt liegen Basis, native Stufen und ganzer Lauf mit und
ohne Karte innerhalb der Streuung von master. Die Spitze des Speichers
liegt dreimal in der Streuung; an der Eisszene mit Karte liegt sie bei B
0,08 GiB tiefer, und die Spannen trennen sich. Die Tabelle wächst bei scale 32 um 90 Sprites am Stand und
um 45 an der Eisszene, 4,2 und 3,4 %. Es ändern sich nur Kacheln, in denen
`sprite_at` eine Fassung der Regel wählt: am Stand 486 der 8500 gerenderten
Kacheln, an der Eisszene 1112 von 2125.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **A:** `bb5b5d8`, master mit #60;
  - **B:** `81b84e8`, mit #58;
  - **BZ:** B mit einem **Zähler**, nur für diese Messung und nicht
    eingecheckt: Je Kachel merkt er sich in `render_area_with` und
    `draw_list`, ob `sprite_at` für einen Block mit Regel einen Nachbarn zu
    einer seiner Seiten fand und deshalb eine Fassung statt des Grundbilds
    nahm, und schreibt dann eine Zeile mit scale und Kachel.
- Die Testwelt, scale 32, drei native Stufen, Pyramide, 24 Threads:
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln, nativ
    1600, 400 und 100, wie in
    [2026-10-01, Native Stufen in Bändern](2026-10-01-native-stufen-in-baendern.md);
  - **Eisszene:** um (-229, -232) mit `--size 8192`, wo das Bild `eis.webp`
    des README liegt, gefrorenes Meer mit Eisbergen, 1600 Basiskacheln,
    nativ 400, 100 und 25.
- Serien:
  - **Gleich:** A und BZ je ein Lauf, `--gpu off`, alle Kacheln
    verglichen;
  - **Ohne Karte:** A und B im Wechsel, `--gpu off`, drei Runden;
  - **Mit Karte:** A und B im Wechsel, `--gpu on`, drei Runden.
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf in ein frisches
  Verzeichnis, das vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Nebenher
  lief nichts; die Last lag vor und nach jeder Serie bei 0 bis 3 %.

Der Befehl, aus der Wurzel des Repositorys, für die Eisszene ohne Karte:

```bash
renderer/target/release/heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -229 -232 --size 8192 --scale 32 --native-levels 3 --gpu off
```

## Ablauf

Die Dauer der Basis und der nativen Stufen stammt aus der Ausgabe, die der
nativen Stufen aus der Zeile, die die Zeit für alle zusammen nennt, die des
ganzen Laufs von der Uhr des Messskripts. Die Zahl der Sprites und
Fassungen steht in der Ausgabe nach dem Vorlauf, für die Basis. Die Spitze
des Arbeitsspeichers stammt aus `GetProcessMemoryInfo`
(`PeakWorkingSetSize`) des beendeten Prozesses. Verglichen wurden die
Kacheln über SHA-256 ihrer Dateien. Alle Läufe vom 01.10. In den Tabellen
steht der Median, dahinter die Spanne.

## Ergebnis

### Was sich ändert

Gerendert sind die Basis, Zoom 10, und die drei nativen Stufen, Zoom 9 bis
7; die Stufen darüber sind verkleinert. „Mit Regel“ zählt die geänderten
Kacheln, in denen BZ eine Fassung der Regel meldet.

| Ausschnitt | Zoom | Kacheln | anders | davon mit Regel |
|---|---|---|---|---|
| Stand | 10 | 6400 | 344 | 344 |
| | 9 | 1600 | 102 | 102 |
| | 8 | 400 | 30 | 30 |
| | 7 | 100 | 10 | 10 |
| | 6 bis 0, verkleinert | 64 | 17 | – |
| Eisszene | 10 | 1600 | 795 | 795 |
| | 9 | 400 | 234 | 234 |
| | 8 | 100 | 64 | 64 |
| | 7 | 25 | 19 | 19 |
| | 6 bis 0, verkleinert | 33 | 26 | – |

- Keine gerenderte Kachel ändert sich ohne eine Fassung der Regel. Keine
  Kachel kommt hinzu oder fällt weg.
- Eine Fassung der Regel meldet BZ am Stand in 503 Kacheln, an der
  Eisszene in 1148. In den übrigen 17 und 36 ändert das Weggelassene kein
  Pixel.

### Sprites

| Ausschnitt | Stand | Sprites bei scale 32 | davon Fassungen |
|---|---|---|---|
| Stand | A | 2134 | 1172 |
| | B | 2224, +4,2 % | 1262 |
| Eisszene | A | 1331 | 745 |
| | B | 1376, +3,4 % | 790 |

### 24 Threads ohne Karte

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | A | 5,0 s (4,5–5,3) | 5,0 s (4,4–5,1) | 11,3 s (10,2–11,6) | 2,42 GiB (2,40–2,43) |
| | B | 5,2 s (5,2–5,2) | 4,8 s (4,8–5,0) | 11,3 s (11,3–11,6) | 2,40 GiB (2,39–2,40) |
| Eisszene | A | 1,6 s (1,5–1,9) | 2,0 s (1,9–2,6) | 4,7 s (4,5–5,0) | 1,64 GiB (1,60–1,65) |
| | B | 1,6 s (1,5–2,2) | 2,0 s (1,8–2,0) | 4,6 s (4,4–5,1) | 1,61 GiB (1,58–1,68) |

### 24 Threads mit Karte

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | A | 4,7 s (4,1–6,6) | 4,7 s (4,5–4,9) | 11,1 s (10,2–12,6) | 3,15 GiB (3,15–3,20) |
| | B | 4,8 s (4,3–5,3) | 4,8 s (4,7–5,0) | 11,2 s (10,5–11,7) | 3,17 GiB (3,15–3,20) |
| Eisszene | A | 1,4 s (1,3–1,5) | 1,8 s (1,7–1,9) | 4,2 s (4,1–4,7) | 2,26 GiB (2,26–2,29) |
| | B | 1,3 s (1,3–1,9) | 1,8 s (1,8–1,8) | 4,2 s (4,0–4,7) | 2,18 GiB (2,08–2,18) |

## Schluss

- Bei der Zeit überlappen in jeder Serie die Spannen von A und B. Ein
  Unterschied ist nicht zu sehen, weder durch die Nachschläge zu den
  Nachbarn noch durch die Sprites mehr.
- An der Eisszene fallen Blöcke mitten im Eis ganz weg. Schneller wird sie
  dadurch nicht messbar.
- Die Spitze des Speichers bleibt am Stand und an der Eisszene ohne Karte
  innerhalb der Streuung. An der Eisszene mit Karte liegt B bei 2,18 GiB
  (2,08–2,18), A bei 2,26 GiB (2,26–2,29); die Spannen trennen sich. Woran
  das liegt, zeigt die Messung nicht.
