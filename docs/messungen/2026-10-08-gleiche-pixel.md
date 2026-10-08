---
title: Gleiche Pixel nicht kodieren
description: Was der Hash der Pixel vor dem Kodieren an einem Update mit einem Thread spart, was er einen Lauf in einen leeren Baum kostet, wie gross die Hashes werden, und dass das Bild in fünf Fällen und nach einem Update Byte für Byte gleich bleibt.
date: 2026-10-08
commits: [a11017a, dec91a3]
code:
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# Gleiche Pixel nicht kodieren

Ein Update an der Testwelt mit einem Thread kodiert 198 von 388 Kacheln
nicht mehr und braucht 13,9 statt 14,4 s, rund 3 % weniger. Mehr ist es
hier nicht, weil feste Kosten das Update an dieser Welt bestimmen. Ein
Lauf in einen leeren Baum kostet gleich viel wie vorher. Ein zweiter Lauf
über denselben Baum spart je Kachel rund 4 ms. Das Bild ist in jedem Fall
Byte für Byte dasselbe wie mit master.

## Aufbau

- **Stände:** master `a11017a` gegen `dec91a3`, je ein Release-Build,
  ohne Grafikkarte.
- **Bild:** fünf Ausschnitte der Testwelt um (-64, 416), `--size 4096
  --scale 16`, je in einen leeren Baum. Dazu `dec91a3` ein zweites Mal
  über seinen eigenen Baum.

  | Fall | Schalter |
  |---|---|
  | Karte, 4 Threads | `--threads 4 --native-levels 0` |
  | Karte, ein Thread | `--threads 1 --native-levels 0` |
  | native Stufen | `--threads 4 --native-levels 3` |
  | Cinematic | `--threads 4 --native-levels 0 --cinematic` |
  | andere Kamera | `--threads 4 --native-levels 0 --camera 4:3 --direction nw` |

- **Update:** wie in
  [2026-10-04, Updates, Kosten](2026-10-04-updates.md), Teil D:
  - eine Kopie der Testwelt, die Karte bei scale 8 aus `se` mit der Vorgabe
    der nativen Stufen;
  - je Stand ein voller Lauf auf allen Threads als Baum, danach eine
    Sicherung mit den Zeiten der Dateien;
  - dann aus 16 verteilten Regionen, deren acht Nachbarn da sind, je den
    Chunk in der Mitte aus der Tabelle genommen;
  - das Update mit `--threads 1`, wie das Plugin es startet.

## Ablauf

- **Reihenfolge:** Die Updates liefen abwechselnd, master zuerst, in Runde
  2 umgekehrt, je drei.
- **Vor jedem Update** bekam der Baum aus der Sicherung jede Datei zurück,
  die das letzte Update geändert, entfernt oder angelegt hatte, mit ihrer
  alten Zeit. Danach 15 s Pause.
- **Ruhe:** Die ganze Reihe lief unter der Sperre und ohne laufenden
  Minecraft-Client; die Last vor jedem Lauf lag unter 10 %.
- **Quelle,** am 08.10.:
  - das Messskript: Wanduhr, CPU-Zeit und Spitze des Arbeitsspeichers des
    Prozesses, SHA-256 jeder Kachel;
  - die Ausgabe der Läufe: Kacheln, Pyramide, `Pixel:`;
  - die Dateigrössen.

## Ergebnis

### Bild

| Fall | Kacheln | gleich wie master | zweiter Lauf: nicht kodiert | zweiter Lauf: neu geschrieben |
|---|---|---|---|---|
| Karte, 4 Threads | 401 | alle | 401 | keine |
| Karte, ein Thread | 401 | alle | 401 | keine |
| native Stufen | 548 | alle | 548 | keine |
| Cinematic | 401 | alle | 401 | keine |
| andere Kamera | 397 | alle | 397 | keine |

Nach den Updates waren die Bäume von master und `dec91a3` gleich, und
beide gleich einem vollen Lauf über die geänderte Welt: 24 353 Kacheln.

### Update, ein Thread

| | master | `dec91a3` |
|---|---|---|
| Wanduhr | 16,99, 14,47, 14,28 s | 14,01, 13,92, 13,79 s |
| CPU | 16,6, 14,3, 14,1 s | 13,8, 13,8, 13,5 s |
| Pyramide laut Ausgabe | 3,2, 2,7, 2,7 s | 2,4, 2,4, 2,4 s |
| Spitze | 0,176 GiB | 0,176 bis 0,177 GiB |
| nicht kodiert | – | 198 |

- **Im Gebiet:** In jedem Update lagen 140 Basiskacheln, 45 davon mit
  neuen Bytes. Die Pyramide setzte 248 Kacheln zusammen. Von diesen 388
  Kacheln kodierte `dec91a3` 198 nicht: 95 der Basis und 103 der
  Pyramide.
- **Die erste Runde von master** lief kalt, die Welt lag noch nicht im
  Cache. Ohne sie liegt master bei 14,3 und 14,5 s, `dec91a3` bei 13,8 bis
  14,0 s. Der Unterschied von rund 0,5 s, 3 %, liegt ausserhalb der
  Streuung jedes Stands. Er passt zu 198 Kacheln zu je rund 2,5 ms.
- **Hashes geschrieben:** 44 Blöcke je Update.

### Lauf in einen leeren Baum und zweiter Lauf

| | master | `dec91a3` |
|---|---|---|
| voller Lauf, Kopie der Testwelt, alle Threads | 92,6 s, Spitze 2,16 GiB | 92,3 s, Spitze 2,13 GiB |
| Karte, ein Thread, 401 Kacheln, leerer Baum | 6,39 s | 6,47 s |
| dito, zweiter Lauf über denselben Baum | | 4,81 s, nichts kodiert |

Je Lauf ein Wert; den Unterschied in den leeren Baum misst das nicht
genauer.

### Platz

- **Hashes:** 99 Blöcke mit zusammen 732 966 Byte für 24 353 Kacheln, rund
  30 Byte je Kachel.
- **Zum Vergleich:** `stand.bin` desselben Baums hat 7 454 745 Byte.

## Schluss

- **Bild und Abnahme:** In allen fünf Fällen, nach dem Update und gegen
  einen vollen Lauf ist das Bild Byte für Byte gleich.
- **Weniger als an #207 hochgerechnet,** dort rund 20 % eines Updates an
  einer kleinen Welt. An der Testwelt bestimmen feste Kosten das Update:
  - die Köpfe von 383 Regionen;
  - der Stand mit 7,5 MB, dreimal geschrieben;
  - Assets und Sprites.

  Das Kodieren war dort nur rund 1 s von 14 s.
- **Grösserer scale:** Bei scale 32 trägt jede Kachel weniger Welt, und das
  Gebiet hat 16-mal so viele Kacheln. Dort sollte der Anteil höher sein;
  gemessen ist das nicht.
- **Volle Läufe** über einen bestehenden Baum desselben Builds sparen je
  Kachel rund 4 ms, also Kodieren und Schreiben.
- **Entscheidung:**
  [0091](../entscheidungen/0091-gleiche-pixel-nicht-kodieren.md).
