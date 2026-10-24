---
title: Grenze nach unten
description: Was die Grenze nach unten aus 0127 an der Testwelt bringt und kostet, gegen den Stand je Region aus 0126 allein; ein Update wie im Plugin mit 16 Änderungen an der Oberfläche, je drei Läufe im Wechsel, und der volle Lauf samt Vorlauf.
date: 2026-10-24
commits: [0a5ddb5, ad83f1b]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
---

# Grenze nach unten

Mit [0127](../entscheidungen/0127-grenze-nach-unten.md) braucht ein Update
mit 16 Änderungen an der Oberfläche 10,2 bis 11,4 s statt 13,4 bis
17,0 s. Sein Gebiet hat 1144 statt 1968 Kacheln, 42 % weniger, und es
schreibt dieselben 13 Kacheln. Der Vorlauf des vollen Laufs braucht 3,1 s
mehr, der Lauf selbst und der Arbeitsspeicher bleiben gleich.

## Aufbau

- **Welt, Baum und Updates** wie in
  [2026-10-24, Updates je Phase](2026-10-24-updates-je-phase.md): eine
  Kopie der Testwelt, 2:1 aus `se`, scale 32, ohne native Stufen,
  `--gpu auto`. Der volle Lauf mit allen Threads und `--low-priority`, die
  Updates wie das Plugin mit `--threads 1 --low-priority`.
- **Stände,** Release-Builds, jeder mit eigenem Baum:
  - **B:** der Stand je Region aus [0126](../entscheidungen/0126-stand-je-region.md), #356 (0a5ddb5).
  - **A:** dazu die Grenze nach unten, #360 (ad83f1b).
- **Änderungen:** dieselben 16 fertig erzeugten Chunks, über die Welt
  verteilt. In jedem wurde der oberste feste Block einer Spalte nahe der
  Mitte ersetzt, mit neuem Stempel, direkt in der Regionsdatei der Kopie;
  dann zurückgesetzt und wieder ersetzt. E lag vor jeder Änderung zwischen
  y 37 und 110, die Grenzen bei 16, 32, 48 und 80.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel.
- **Erst** je ein voller Lauf, B, dann A.
- **Dann** drei Änderungen; nach jeder ein Update auf beiden Bäumen, die
  Reihenfolge im Wechsel (B A, A B, B A), je 5 s Pause.
- **Die Zahlen** aus den Zeilen `Vorlauf:`, `Update:`, `Kacheln:` und
  `Pyramide:` der Ausgabe. Die Dauer vom Start bis zum Ende des Prozesses,
  die Spitze des Arbeitsspeichers aus `GetProcessMemoryInfo` des beendeten
  Prozesses.

## Ergebnis

**Update mit 16 Änderungen:**

| | B | A |
|---|---|---|
| Dauer | 17,01 / 13,47 / 13,37 s | 11,38 / 10,48 / 10,24 s |
| Kacheln im Gebiet | 1968 | 1144 |
| geschrieben / gleich geblieben / leer | 13 / 1849 / 64 | 13 / 1037 / 58 |
| Vorlauf | 5967 Chunks, 1,4–1,7 s | 4980 Chunks, 1,2 s |
| Pyramide | 560 Kacheln, 0,6–0,7 s | 346 Kacheln, 0,6–0,7 s |
| Spitze Working Set | 0,332–0,333 GiB | 0,339–0,340 GiB |
| Spitze privat | 0,484–0,487 GiB | 0,488–0,489 GiB |

Das erste Update je Baum war langsamer, das von B am meisten: Es lief als
erstes nach den vollen Läufen.

**Voller Lauf,** je einer:

| | B | A |
|---|---|---|
| Dauer | 265,5 s | 268,5 s |
| Vorlauf, 249 103 Chunks | 10,0 s | 13,1 s |
| Spitze Working Set | 2,633 GiB | 2,610 GiB |
| Spitze privat | 3,401 GiB | 3,381 GiB |

Beide schrieben 274 273 Basiskacheln und 93 721 Kacheln der Pyramide.

## Schluss

- **Ein Update** wird ohne das erste je Baum im Mittel um 3,1 s schneller,
  13,4 → 10,4 s, rund 23 %. Das Gebiet verliert 824 Kacheln, so viele, wie
  [Updates je Phase](2026-10-24-updates-je-phase.md) gerechnet hatte (816
  von 1960).
- **Dieselben 13 Kacheln** schreiben beide Stände: Was die Grenze weglässt,
  hätte dieselben Pixel gezeigt.
- **Der Vorlauf** kostet für E und H 3,1 s mehr bei 24 Threads, gerechnet
  rund 0,3 ms Rechenzeit je Chunk. Am ganzen vollen Lauf ist das 1 %,
  innerhalb der Streuung eines einzelnen Laufs. Billiger ginge es, wenn
  `abdruck` H im selben Durchgang mitrechnet; das lohnt erst, wenn der
  Vorlauf stört.
- **Der Arbeitsspeicher** liegt innerhalb der Streuung, im vollen Lauf wie
  im Update.
- **Nicht gemessen:** die Stempel der Regionen nur bei anderer mtime und
  Grösse zu lesen. Das kostet an der Testwelt unter Windows 0,46 s je
  Update, siehe [Updates je Phase](2026-10-24-updates-je-phase.md); unter
  Linux erst messen.
