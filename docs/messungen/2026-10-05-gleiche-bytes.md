---
title: Gleiche Bytes liegen lassen, voller Lauf über einen bestehenden Baum
description: Was es einen vollen Lauf über einen bestehenden Baum kostet, Kacheln mit gleichen Bytes zu vergleichen und liegen zu lassen, statt sie neu zu schreiben, master gegen #171 an der Testwelt bei scale 8. Dazu, welche Runden fremde Last störten.
date: 2026-10-05
commits: [9d1dfde, 501ae6f]
code:
  - renderer/src/cli.rs
---

# Gleiche Bytes liegen lassen, voller Lauf über einen bestehenden Baum

Ein voller Lauf über einen bestehenden Baum, in dem sich nichts geändert
hat, wird mit #171 nicht langsamer. In den zwei sauberen Runden brauchte die
Basis 1,8 und 2,4 s weniger als auf master, bei rund 80 s, also 2 bis 3 %.
Liegen blieben dabei alle 17 820 Basiskacheln mit Inhalt. Zwei weitere
Runden störte fremde Last; sie zählen nicht. Zu #166.

## Aufbau

- **Bedingungen** wie in [Updates, Kosten](2026-10-04-updates.md):
  - die Testwelt, nur gelesen;
  - 2:1 aus `se`, scale 8, `--gpu off`, alle 24 Threads;
  - die Vorgabe der nativen Stufen, ohne `--area`.
- **Stände,** je ein Release-Build auf derselben Basis:
  - **master:** `9d1dfde`;
  - **#171:** `501ae6f`.
- **Ablauf je Stand:** Zuerst legt ein voller Lauf in einen leeren Ordner den
  Baum an. Danach folgt je Runde ein voller Lauf über diesen Baum. Auf master
  schreibt dieser jede Kachel neu, auf #171 lässt er jede mit gleichen Bytes
  liegen.
- **Runden** im Wechsel, nach jedem Lauf 15 s Pause.
  - Den Baum löscht das Skript erst am Ende, denn jeder Lauf braucht den
    vorigen.
  - Deshalb gibt es hier kein Löschen unmittelbar vor einem Lauf, wie es
    [Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md)
    vermeidet.
- **Gemessen** wird die Basis, die Zeit aus der Zeile nach „Kacheln:“ in der
  Ausgabe jedes Laufs.

```bash
heroic-map-renderer --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --scale 8 --gpu off
```

## Ablauf

- Am 05.10., mit Sperrdatei:
  - die Reihe von 22:24:55 bis 22:38:50;
  - eine vierte Runde von 22:40:26 bis 22:49:06, mit eigenen neu angelegten
    Bäumen, als Ersatz für Runde 1.
- **Fremde Last:**
  - Mutationstests einer anderen Sitzung liefen von vor 22:24:55 bis
    22:31:16, mit einem bis zwei Kernen. Das stört die Läufe zum Anlegen
    und Runde 1.
  - Ein Release-Build einer anderen Sitzung lief von 22:40:25 bis etwa
    22:44:25 auf allen Kernen. Das stört die Läufe zum Anlegen der vierten
    Runde.
  - Während Runde 4 lag die Last weiter hoch; am Ende 25 %, Herkunft
    unbekannt.
- **Liegen gelassen:** Während Runde 2 trug eine Kachel im Baum von #171
  noch die Zeit vom Anlegen, im Baum von master die von Runde 1.

## Ergebnis

Basis in Sekunden:

| Runde | master | #171 | gilt |
|---|---|---|---|
| 1 | 72,2 | 72,2 | nein, fremde Last |
| 2 | 80,2 | 78,4 | ja |
| 3 | 82,1 | 79,7 | ja |
| 4 | 105,9 | 88,5 | nein, fremde Last |

- **In Runde 2 und 3** war #171 je etwas schneller, um 1,8 s und 2,4 s.
- **Von Runde zu Runde** wurde jeder Stand langsamer, von 72 auf 82 s, wohl
  weil die Maschine unter Dauerlast drosselt. Verglichen ist deshalb nur
  innerhalb einer Runde.
- **Die Ausgabe** jedes Laufs: 18 164 Kacheln im Vorlauf, 17 820 mit Inhalt,
  344 leer, 1022,6 MiB.

## Schluss

- Kacheln mit gleichen Bytes zu vergleichen und liegen zu lassen, kostet
  einen vollen Lauf über einen bestehenden Baum keine Zeit. Es spart hier
  2 bis 3 % der Basis.
- Der Gewinn ist das ETag, nicht die Zeit: Keine der 17 820 Kacheln bekam
  eine neue Zeit.
