---
title: "0018: Dateien tauschen statt überschreiben"
description: Warum jede Kachel und map.json erst als eigene Datei entstehen und dann umbenannt werden, und warum map.json vorher ganz auf die Platte geht.
status: gilt
date: 2026-09-25
issues: [9, 10]
code:
  - renderer/src/cli.rs
---

# 0018: Dateien tauschen statt überschreiben

## Anlass

Seit `--pyramid` liest ein zweiter Prozess den Baum, während der Export
schreibt, und das Frontend liefert Kacheln eines laufenden Renders aus.

## Entscheidung

Jede Kachel und `map.json` entstehen erst als eigene Datei daneben,
`<name>.<pid>.tmp`, und werden dann umbenannt (`tausche`, #9). Wer die alte
gerade liest, liest sie zu Ende. Bricht der Lauf mittendrin ab, steht die
alte noch da, daneben höchstens die halbe eigene, und die sucht kein Leser.
`map.json` geht vor dem Tausch ganz auf die Platte (#10), ohne sie bricht
jeder Lauf ab.

## Verworfene Alternativen

- **Überschreiben.** Ein Leser sähe eine halbe Datei, und nach einem
  Abbruch stünde eine halbe da.

## Folgen

- Nach einem Abbruch steht die alte Datei noch da; nach einem Stromausfall
  steht die alte oder die neue `map.json` da, und kein Lauf scheitert an
  einer halben.
- Kacheln gehen vor dem Tausch nicht eigens auf die Platte, das kostete je
  Kachel ein Warten auf die Platte. Ein Stromausfall kann die jüngsten
  zerreissen; `--resume` rendert deshalb die der letzten zwei Minuten neu,
  siehe [0019](0019-resume-behaelt-die-basiskacheln.md).
