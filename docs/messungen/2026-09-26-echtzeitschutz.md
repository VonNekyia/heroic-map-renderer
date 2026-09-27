---
title: Echtzeitschutz unter Windows
description: Was die Prüfung jeder geschriebenen Kachel durch Microsoft Defender einen Export kostet, mit und ohne Ausnahme für den Kachelordner.
date: 2026-09-26
commits: [17d9bbd]
code:
  - renderer/src/cli.rs
---

# Echtzeitschutz unter Windows

Mit einer Ausnahme für den Kachelordner braucht ein Export ein Drittel
weniger Zeit: 23,8 statt 35,7 s für 16 384 Basiskacheln, die Basis schafft
1116 statt 705 Kacheln/s.

## Aufbau

- Welt: ein Ausschnitt der grossen Welt auf Land, 16 384 Basiskacheln bei
  scale 32, ohne native Stufen, 24 Threads.
- Stand: ein Bau von #10 vom Vormittag des 26.09., vor `f96ff46`; die
  Zahlen kamen mit `17d9bbd` ins README. Welcher Commit genau gebaut war,
  ist nicht festgehalten.
- Die Ausnahme für den Kachelordner setzte der User als Administrator.
- Daneben schrieb ein Leistungszähler die Rechenzeit des Prozesses von
  Defender mit.

## Ablauf

Erst ein Aufwärmlauf und drei Läufe mit der Ausnahme, dann, nachdem der
User sie wieder entfernt hatte, ein Aufwärmlauf und drei ohne; jeder Lauf
frisch in einen leeren Ordner. Nicht abwechselnd, denn die Ausnahme setzt
nur ein Administrator. Eine erste Reihe ohne Ausnahme lief vor einer
Änderung an den Einstellungen des Rechners und zählt nicht.

Quelle: das Messskript und seine Ausgabe vom 26.09., 11:36 bis 11:43. Es
startete jeden Lauf, stoppte die Zeit, las Raten und Dauer aus der Ausgabe
und die Rechenzeit von Defender einmal je Sekunde aus dem Leistungszähler.

## Ergebnis

Jeweils der schnellste von drei Läufen:

| | ohne Ausnahme | mit Ausnahme für den Kachelordner |
|---|---|---|
| ganzer Lauf | 35,7 s | 23,8 s |
| Basis | 705 Kacheln/s | 1116 Kacheln/s |
| Pyramide | 8,6 s | 5,8 s |
| Rechenzeit des Echtzeitschutzes | 219 s, im Mittel sechs Kerne | 7 s |

Die drei Läufe ohne Ausnahme lagen zwischen 35,7 und 39,3 s, die mit ihr
zwischen 23,8 und 26,0 s.

## Schluss

Der Echtzeitschutz kostet ein Drittel der Laufzeit und viel Rechenzeit
daneben. Daraus wurden der Hinweis beim ersten Export und
`--defender-exclusion`, siehe
[0022](../entscheidungen/0022-defender-ausnahme-nur-mit-zustimmung.md). Ein
Dev Drive ist nicht gemessen.
