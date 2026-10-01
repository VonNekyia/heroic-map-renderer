---
title: Teile je Würfel im Raum
description: Was die Zuordnung der Teile im Raum aus #65 an Kacheln ändert und kostet, am Stand und an einer Feuerszene der Testwelt, im Wechsel gegen master, mit und ohne Karte, dazu Sprites und die Spitze des Speichers.
date: 2026-10-01
commits: [fbadb5d, 6471c86]
code:
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/metatile.rs
---

# Teile je Würfel im Raum

Die Zuordnung im Raum aus #65 kostet keine messbare Zeit. Am Stand wie an
einer Feuerszene der Testwelt liegen Basis, native Stufen, ganzer Lauf und
die Spitze des Speichers mit und ohne Karte innerhalb der Streuung von
master. Die Zahl der Sprites bleibt gleich. Es ändern sich nur Kacheln, in
denen ein zerfallenes Modell liegt: am Stand 1 der 8500 gerenderten
Kacheln, an der Feuerszene 3 von 2125. Ein zerfallenes Modell liegt am
Stand in 27 gerenderten Kacheln, an der Feuerszene in 14.

## Aufbau

- Stände, Release-Build, je aus eigenem Worktree:
  - **A:** `fbadb5d`, master mit #63;
  - **B:** `6471c86`, mit #65;
  - **BZ:** B mit einem **Zähler**, nur für diese Messung und nicht
    eingecheckt: Je Kachel merkt er sich in `render_area_with` und
    `draw_list`, ob ein Sprite gezeichnet wurde, das ein Teil in einem
    fremden Würfel hat, also zerfallen ist, und schreibt dann eine Zeile mit
    scale und Kachel.
- Die Testwelt, scale 32, drei native Stufen, Pyramide, 24 Threads:
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln, nativ
    1600, 400 und 100, wie in
    [2026-10-01, Native Stufen in Bändern](2026-10-01-native-stufen-in-baendern.md);
  - **Feuerszene:** um (-168, -5960) mit `--size 8192`, 1600
    Basiskacheln, nativ 400, 100 und 25. Dort brennt ein Fichtenwald: Im
    Chunk mit den meisten Feuern der Testwelt über y 50 stehen 21, teils
    unter Laub und Stämmen. Feuer ragt bis 22,7/16 hoch und zerfällt.
- Serien:
  - **Gleich:** A und BZ je ein Lauf, `--gpu off`, alle Kacheln
    verglichen;
  - **Ohne Karte:** A und B im Wechsel, `--gpu off`, drei Runden;
  - **Mit Karte:** A und B im Wechsel, `--gpu on`, drei Runden.
- 24 Threads heisst `RAYON_NUM_THREADS=24`. Jeder Lauf in ein frisches
  Verzeichnis, das vom Echtzeitschutz ausgenommen war, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).
- Last: Nebenher liefen keine Builds, Tests oder Messungen. Die Last vor
  und nach jeder Serie: gleich 21 und 25 %, ohne Karte 25 und 8 %, mit
  Karte 7 und 0 %. Sie kam von Programmen, die nicht zur Messung gehören;
  deshalb liefen A und B im Wechsel.

Der Befehl, aus der Wurzel des Repositorys, für die Feuerszene ohne Karte:

```bash
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -168 -5960 --size 8192 --scale 32 --native-levels 3 --gpu off
```

## Ablauf

Die Dauer der Basis und der nativen Stufen stammt aus der Ausgabe, die der
nativen Stufen aus der Zeile, die die Zeit für alle zusammen nennt, die des
ganzen Laufs von der Uhr des Messskripts. Der ganze Lauf enthält den Bau
der Sprite-Tabelle, die Zahl der Sprites steht in der Ausgabe nach dem
Vorlauf, für die Basis. Die Spitze des Arbeitsspeichers stammt aus
`GetProcessMemoryInfo` (`PeakWorkingSetSize`) des beendeten Prozesses.
Verglichen wurden die Kacheln über SHA-256 ihrer Dateien. Alle Läufe vom
01.10. In den Tabellen steht der Median, dahinter die Spanne.

## Ergebnis

### Was sich ändert

Gerendert sind die Basis, Zoom 10, und die drei nativen Stufen, Zoom 9 bis
7; die Stufen darüber sind verkleinert.

- **Gerendert:** „Erklärt“ zählt die geänderten Kacheln, in denen BZ ein
  zerfallenes Modell meldet.
- **Verkleinert:** „Erklärt“ zählt die geänderten Kacheln, von deren vier
  Quellkacheln eine geändert ist.

| Ausschnitt | Zoom | Kacheln | anders | erklärt | mit zerfallenem Modell |
|---|---|---|---|---|---|
| Stand | 10 | 6400 | 1 | 1 | 24 |
| | 9 | 1600 | 0 | 0 | 1 |
| | 8 | 400 | 0 | 0 | 1 |
| | 7 | 100 | 0 | 0 | 1 |
| | 6 bis 0, verkleinert | 64 | 0 | 0 | – |
| Feuerszene | 10 | 1600 | 3 | 3 | 14 |
| | 9 | 400 | 0 | 0 | 0 |
| | 8 | 100 | 0 | 0 | 0 |
| | 7 | 25 | 0 | 0 | 0 |
| | 6 bis 0, verkleinert | 19 | 0 | 0 | – |

- Keine Kachel ändert sich ohne Erklärung. Keine Kachel kommt hinzu oder
  fällt weg.
- In den übrigen Kacheln mit zerfallenem Modell, 26 am Stand und 11 an der
  Feuerszene, ändert die neue Zuordnung kein Pixel.
- Die Würfel, in die ein Teil ragt: am Stand bei A wie bei B der darüber
  und der darunter. An der Feuerszene bei A nur der darüber, bei B auch der
  darunter. Das ist das Feuer auf dem Boden: Zwei seiner vier Flächen,
  gedreht um z mit `rescale`, reichen 0,33/16 unter den Boden. Das Teil
  liegt jetzt im Würfel des Bodens; hat er Würfelform, kommt es vor ihm
  und wird gedeckt.

### Sprites

Die Tabelle bleibt gleich gross: bei scale 32 am Stand 2224 Sprites, davon
1262 Fassungen, an der Feuerszene 1106, davon 555, bei A wie bei B.

### 24 Threads ohne Karte

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | A | 6,0 s (5,9–7,2) | 5,8 s (5,8–6,1) | 13,5 s (13,1–14,5) | 2,36 GiB (2,32–2,37) |
| | B | 6,1 s (5,9–8,2) | 5,9 s (5,8–7,1) | 14,7 s (13,2–15,3) | 2,37 GiB (2,29–2,40) |
| Feuerszene | A | 1,8 s (1,7–1,8) | 1,9 s (1,9–1,9) | 4,4 s (4,4–4,6) | 1,55 GiB (1,52–1,58) |
| | B | 1,9 s (1,7–3,4) | 1,9 s (1,8–1,9) | 4,8 s (4,3–6,1) | 1,53 GiB (1,51–1,57) |

### 24 Threads mit Karte

| Ausschnitt | Stand | Basis | native Stufen | ganzer Lauf | Spitze |
|---|---|---|---|---|---|
| Stand | A | 4,8 s (4,8–6,6) | 5,4 s (5,3–5,4) | 11,8 s (11,6–13,6) | 3,15 GiB (3,15–3,20) |
| | B | 4,9 s (4,8–5,1) | 5,4 s (5,3–5,5) | 12,0 s (11,9–12,8) | 3,13 GiB (3,12–3,19) |
| Feuerszene | A | 1,6 s (1,5–1,9) | 1,8 s (1,7–1,8) | 4,2 s (4,2–4,8) | 2,15 GiB (2,04–2,16) |
| | B | 1,6 s (1,5–2,3) | 1,8 s (1,7–1,9) | 4,5 s (4,2–5,1) | 2,14 GiB (2,05–2,14) |

## Schluss

- Bei der Zeit überlappen in jeder Serie die Spannen von A und B. Ein
  Unterschied ist nicht zu sehen, weder durch die drei Raster je Modell
  beim Bau der Tabelle noch durch das Nachschlagen der Familie je fremdem
  Kandidaten.
- Am Stand ohne Karte liegt der Median des ganzen Laufs bei B 1,2 s höher,
  Basis und native Stufen aber gleichauf. Den Unterschied machen einzelne
  Läufe bei bis zu 25 % Last, bei A einer, bei B zwei: Basis 7,2 s bei A,
  Basis 8,2 s und native Stufen 7,1 s bei B. Mit Karte, bei 0 bis 7 %
  Last, liegen die Mediane bei 11,8 und 12,0 s.
- Die Spitze des Speichers bleibt in jeder Serie innerhalb der Streuung.
