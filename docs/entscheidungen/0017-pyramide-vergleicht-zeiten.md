---
title: "0017: --pyramid vergleicht Zeiten, nicht Inhalte"
description: Warum --pyramid über Änderungszeiten entscheidet, was neu gebaut wird, und seine eigenen Kacheln zwei Sekunden vor seinem Beginn stempelt.
status: gilt
date: 2026-09-25
issues: [9, 10]
code:
  - renderer/src/cli.rs
---

# 0017: --pyramid vergleicht Zeiten, nicht Inhalte

## Anlass

Beim ersten Vollrender der grossen Welt sah man herausgezoomt nichts, bis
alles fertig war: `map.json` schreibt jeder Export vor seiner ersten Kachel,
die gröberen Stufen entstehen aber erst am Ende.

## Entscheidung

`--pyramid DIR` baut Zoomstufen und `map.json` aus den Basiskacheln auf der
Platte, ohne Welt und Assets, auch während ein Render läuft. Verglichen wird
auf jeder Stufe, jede Kachel mit ihren Kindern, aus einer Liste je Stufe mit
den Änderungszeiten. Was der Aufruf schreibt, trägt seinen Beginn minus zwei
Sekunden: Ein Kind, das der Render währenddessen fertigstellt, ist danach
jünger als seine Elternkachel. Was nach dem Beginn und vor der Liste seiner
Stufe entstand, hat der Render geschrieben; auf nativen Stufen und bei
`map.json` bleibt es stehen. Siehe
[Pyramide und Fortsetzen](../benutzung/pyramide-und-resume.md).

## Verworfene Alternativen

- **Inhalte vergleichen.** Eine Pyramide über 2,5 Millionen Basiskacheln
  liest 300 GB, das will niemand stündlich.
- **Fremd ab dem Stempel statt ab dem Beginn.** Das machte Kacheln aus den
  zwei Sekunden vor dem Aufruf unantastbar; deshalb zwei Zeiten, der Beginn
  zum Vergleichen und der Stempel zum Schreiben.
- **Fremde Kacheln auf allen Stufen stehen lassen.** Neben einem
  Ausschnitt-Export blieb dann eine grobe Kachel dauerhaft veraltet; eine
  verkleinerte Kachel hängt nur an ihren Kindern.

## Folgen

- Nicht bemerkt werden ein einzelnes Kind, das von aussen verschwindet,
  solange Geschwister bleiben, eine Kachel, die mit alter Zeit aus einer
  Sicherung zurückkommt, und eine, die ein Stromausfall zerrissen hat.
- Eine Zeit in der Zukunft kommt von einer Uhr, die vorging; eine
  verkleinerte Kachel mit so einer Zeit baut der Aufruf einmal neu (#10).
- Zwei Sekunden, weil keine gängige Uhr eines Dateisystems gröber zählt.
