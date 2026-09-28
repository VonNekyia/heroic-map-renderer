---
title: Blockentities
description: Was das Lesen von Mustern und Scherben aus `block_entities` kostet, im Wechsel gegen master gemessen, einfädig mit --scan über die Testwelt, mit allen Threads im Vorlauf von --heights über die Testwelt und die grosse Welt und als Export über den Ausschnitt aus „Was ein Lauf kostet“.
date: 2026-09-28
commits: [0293e76, 3048c59]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/sprites.rs
---

# Blockentities

Dass der Decoder Muster und Scherben aus `block_entities` mitliest, kostet
nichts Messbares. Einfädig dekodiert `--scan` die Testwelt mit #30 wie
master in 45,7 bis 46,1 s. Der Vorlauf mit allen Threads braucht auf der
Testwelt 4,1 bis 5,0 s, auf der grossen Welt 61 bis 72 s, bei beiden
Ständen. Ein Export über den Ausschnitt aus
[Was ein Lauf kostet](../benutzung/kosten.md) rastert 34 Sprites mehr und
rendert gleich schnell. Die Testwelt hat 96 Banner mit Mustern und 2073
Krüge mit Scherben, samt Block 20 verschiedene.

## Aufbau

- Stände, Release-Build:
  - **master:** `0293e76`;
  - **#30:** `3048c59`. Gebaut war der Stand vor dem Commit; danach kam
    nur die Zählung „verschiedene samt Block“ in `--scan` dazu, siehe
    „Ergebnis“.
- Welten, beide 26.2:
  - die Testwelt, 383 Regionen, 316 223 Chunks, davon 249 103 fertig
    erzeugt;
  - die grosse Welt, 2509 Regionen, 2 520 778 Chunks, davon 2 449 850
    fertig erzeugt.
- **`--scan`** dekodiert jeden Chunk, auch die nicht fertig erzeugten, in
  einem Thread und ohne Assets. Banner und Krüge zählt er nur in den
  fertigen, wie der Renderer sie liest.
- **`--heights`** schreibt in ein Verzeichnis, in dem nur eine `map.json`
  liegt, wie in [2026-09-28, Höhen](2026-09-28-hoehen.md): der Vorlauf
  eines Exports ohne Sprite-Tabelle und Kacheln, mit allen 24 Threads. Dabei
  sammelt #30 jedes Blockentity mit Daten samt seinem Block.
- **Export:** der Ausschnitt um (-64, 416) mit `--size 8192`, 1600
  Basiskacheln, scale 32, alle drei nativen Stufen und Pyramide, 24 Threads
  ohne Karte, wie in [2026-09-28, Cutout](2026-09-28-cutout.md). Die
  Texturen der Entities kamen bei beiden Ständen als eigene Wurzel zwischen
  `vanilla-assets` und `assets` dazu, siehe
  [Assets und Biomdaten](../benutzung/assets.md).
- Die Testwelt lag als Kopie in einem Ordner, der vom Echtzeitschutz
  ausgenommen war, ebenso jede Ausgabe, siehe
  [Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md). Die
  grosse Welt lag an ihrem Ort, wie bei der Messung der Höhen.
- Die anderen Sitzungen warteten. Nebenher lief ein Programm, das gut 6 %
  der Rechenzeit nahm. Die Last lag vor `--scan` bei 7 %, vor dem Vorlauf
  bei 5 %, vor dem Export bei 17 %.

Die Befehle, aus der Wurzel des Repositorys, für die Testwelt:

```bash
renderer/target/release/terranova-render --world ./world --scan
renderer/target/release/terranova-render --world ./world --heights <ordner>
renderer/target/release/terranova-render --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 8192 --scale 32 --native-levels 3 --gpu off
```

## Ablauf

Am 28.09. abends, je Reihe abwechselnd master und #30 zuerst:

- **`--scan`:** je Stand ein Lauf zum Aufwärmen, dann drei Runden, von
  20:52 bis 21:00;
- **`--heights`:** je Welt vier Runden, von 21:00 bis 21:09;
- **Export:** drei Runden, von 21:09 bis 21:10;
- **master gegen `3a782ef`:** drei Runden auf der grossen Welt, von 21:11
  bis 21:18, siehe „Ergebnis“.

Welche Dateien des Exports sich ändern, zeigt ein weiterer Lauf je Stand,
Datei für Datei verglichen. Die verschiedenen samt Block zählte `--scan`
mit dem Release-Build von `3048c59` um 21:36.

Die Zahlen stammen aus drei Quellen:

- aus der Ausgabe jedes Laufs, den Zeilen `Scan:`, `Vorlauf:` und
  `Kacheln:` und der Zählung der Banner, Krüge und Sprites;
- aus einem Messskript: die Dauer des ganzen Prozesses und bei `--heights`
  die Spitze des Arbeitsspeichers, `PeakWorkingSetSize` aus
  `GetProcessMemoryInfo`, alle 0,2 s abgefragt;
- aus der Summe der Dateien im Verzeichnis nach jedem Export.

Eine erste Reihe eine halbe Stunde vorher ist verworfen. Neben ihr lief
ein Durchlauf über einen Kachelbaum auf demselben Laufwerk, und einfädig
brauchte `--scan` 121 bis 129 s statt 46 s.

## Ergebnis

### `--scan`, ein Thread

| Stand | Aufwärmen | Runde 1 | Runde 2 | Runde 3 |
|---|---|---|---|---|
| master | 109,9 s | 46,1 s | 45,8 s | 45,7 s |
| #30 | 46,1 s | 46,0 s | 45,7 s | 45,8 s |

- Die Läufe zum Aufwärmen zählen nicht; der erste der Reihe brauchte
  109,9 s.
- Beide Stände dekodieren alle 316 223 Chunks ohne Fehler.
- #30 fand 96 Banner mit Mustern und 2073 Krüge mit Scherben, samt Block
  20 verschiedene. So viele Familien baut ein Export der ganzen Testwelt
  für die Daten dazu.

### Vorlauf von `--heights`, alle Threads

Die Spanne über die vier Runden:

| Welt | Stand | ganzer Lauf | Vorlauf | Speicher an der Spitze |
|---|---|---|---|---|
| Testwelt | master | 4,6 bis 5,3 s | 4,1 bis 5,0 s | 0,11 GiB |
| Testwelt | #30 | 4,7 bis 5,0 s | 4,4 bis 4,7 s | 0,11 GiB |
| grosse Welt | master | 62,4 bis 73,6 s | 60,9 bis 72,2 s | 0,37 bis 0,39 GiB |
| grosse Welt | #30 | 64,6 bis 72,2 s | 63,1 bis 70,8 s | 0,37 bis 0,39 GiB |

Auf der grossen Welt liegen beide Stände über den 48,5 bis 51,6 s aus
[2026-09-28, Höhen](2026-09-28-hoehen.md), auf der Testwelt nicht. Das
liegt nicht an master: Der Build von damals, `3a782ef`, brauchte direkt
danach im Wechsel mit master drei Runden lang 63,2 bis 67,9 s, master
64,4 bis 68,0 s. `3a782ef` liest dabei auch die 70 928 nicht fertig
erzeugten Chunks mit, die seit #32 übersprungen werden.

### Export des Ausschnitts

| Stand | Sprites | davon Fassungen | Basis, Kacheln/s | ganzer Lauf | alle Dateien |
|---|---|---|---|---|---|
| master | 1151 | 620 | 904, 1104, 1081 | 4,5 bis 5,8 s | 137 977 508 Byte |
| #30 | 1185 | 638 | 1031 bis 1087 | 4,6 bis 4,7 s | 137 977 528 Byte |

- 2148 Dateien je Lauf, die Basis 103,3 MB, 63 kB je Kachel; die Grössen
  waren in allen Runden Byte für Byte gleich.
- Der erste Lauf von master war der erste der Reihe und der langsamste.
- 6 der 2148 Dateien ändern sich: eine Basiskachel, in der sich 164 Pixel
  an einer Stelle unter Wasser ändern, und fünf Kacheln der nativen Stufen
  und der Pyramide. Die 34 Sprites mehr gehören zu Blockstates des
  Ausschnitts, die bisher kein Bild hatten.

## Schluss

Muster und Scherben mitzulesen verlängert weder den Scan noch den Vorlauf
messbar; die Spannen überlappen auf beiden Welten, und der Speicher an der
Spitze bleibt gleich. Ein Export rastert dazu die Bilder der Blockentities
in seinem Ausschnitt und für ihre Daten eigene Familien, auf der ganzen
Testwelt 20; auf dem Ausschnitt rendert er gleich schnell.
