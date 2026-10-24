---
title: Pyramide nur über neuen Kindern
description: Was ein Update wie im Plugin mit 16 Änderungen an der Oberfläche an der Testwelt braucht, bevor und nachdem die Pyramide eines Updates nur noch über Kindern ohne Treffer im Hash baut (0124); Dauer, Pyramide und Spitze des Arbeitsspeichers, je drei Läufe im Wechsel.
date: 2026-10-24
commits: [d0074bf, a06d540]
code:
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
---

# Pyramide nur über neuen Kindern

Mit [0124](../entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md)
braucht ein Update mit 16 Änderungen an der Oberfläche 13,1 bis 13,4 s
statt 15,0 bis 17,3 s. Die Pyramide baut 560 statt 970 Kacheln, in 0,6 bis
0,7 s statt 2,6 bis 3,0 s. Die Basis und der Arbeitsspeicher bleiben
gleich.

## Aufbau

- **Welt, Baum und Updates** wie in
  [2026-10-24, Updates je Phase](2026-10-24-updates-je-phase.md): eine
  Kopie der Testwelt, 2:1 aus `se`, scale 32, ohne native Stufen,
  `--gpu auto`, die Updates mit `--threads 1 --low-priority` wie das
  Plugin. Vorher ein voller Lauf mit A.
- **Stände,** Release-Builds:
  - **A:** master mit der Änderung aus 0125 (d0074bf); die betrifft nur
    `--serve`.
  - **B:** #352 (a06d540).
- **Änderungen:** 16 über die Welt verteilte, fertig erzeugte Chunks. In
  jedem wurde der oberste feste Block einer Spalte nahe der Mitte ersetzt,
  mit neuem Stempel, direkt in der Regionsdatei der Kopie, und vor jedem
  Lauf abwechselnd zurückgesetzt und wieder ersetzt.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel.
- **Läufe:** sechs auf demselben Baum, in der Folge A, B, B, A, A, B, je
  5 s Pause dazwischen.
- **Die Zahlen** aus den Zeilen `Update:`, `Kacheln:` und `Pyramide:` der
  Ausgabe. Die Dauer vom Start bis zum Ende des Prozesses, die Spitze des
  Arbeitsspeichers aus `GetProcessMemoryInfo` des beendeten Prozesses.

## Ergebnis

Jeder Lauf: 1968 Kacheln im Gebiet, davon 13 geschrieben, 1849 gleich
geblieben und 64 leer.

| | A | B |
|---|---|---|
| Dauer | 17,34 / 15,22 / 15,02 s | 13,36 / 13,12 / 13,15 s |
| Pyramide, Kacheln | 970, 60,2 MiB | 560, 35,0 MiB |
| Pyramide, Zeit | 3,0 / 2,8 / 2,6 s | 0,7 / 0,6 / 0,6 s |
| davon schon während des Renderns | 488 | 488 |
| über Kacheln mit gleichen Pixeln nicht neu gebaut | – | 1851 |
| Spitze Working Set | 0,338–0,341 GiB | 0,341–0,356 GiB |
| Spitze privat | 0,494–0,498 GiB | 0,493–0,501 GiB |

- **Der erste Lauf von A** war der erste nach dem vollen Lauf und langsamer.
  Ohne ihn bleiben A 15,0 bis 15,2 s.

## Schluss

- **Ein Update** wird im Mittel um 1,9 s schneller, ohne den ersten Lauf
  von A gerechnet, rund 13 %, fast ganz in der Pyramide.
- **Was bleibt:** 488 Eltern baut auch B im Speicher, während die Basis
  entsteht, siehe [Zoomstufen](../benutzung/zoomstufen.md), „Feine Stufen
  im Speicher“; die Regel aus 0124 gilt dort nicht.
- **Der Arbeitsspeicher** liegt innerhalb der Streuung.
