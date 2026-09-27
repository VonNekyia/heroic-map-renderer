---
title: "0019: --resume behält die Basiskacheln bis auf die letzten zwei Minuten"
description: Warum ein fortgesetzter Lauf vorhandene Basiskacheln ungelesen übernimmt, die jüngsten zwei Minuten aber entfernt und neu rendert, und native Stufen und Pyramide ganz neu baut.
status: gilt
date: 2026-09-26
issues: [10]
code:
  - renderer/src/cli.rs
---

# 0019: --resume behält die Basiskacheln bis auf die letzten zwei Minuten

## Anlass

Ein Vollrender der grossen Welt dauert lange; nach einem Abbruch soll er
nicht von vorn beginnen.

## Entscheidung

`--resume` übernimmt vorhandene Basiskacheln, wie sie sind, ausser denen
aus den letzten zwei Minuten vor der jüngsten (`frische`). Die entfernt er,
bevor er sie neu rendert: Bricht auch er ab, fehlen sie, und das nächste
Fortsetzen rendert sie. Die nativen Stufen rendert er ganz neu, die
Pyramide baut er ganz neu wie jeder Lauf. Siehe
[Pyramide und Fortsetzen](../benutzung/pyramide-und-resume.md),
„Fortsetzen: `--resume`“.

## Verworfene Alternativen

- **Jede Kachel öffnen und prüfen.** Ansehen lässt sich ein Stromausfall
  einer Kachel nicht sicher; der Dekoder liest zwei von drei zerrissenen
  ohne Fehler. Die frischen werden deshalb ungelesen neu gerendert.
- **Native Stufen übernehmen.** Dort kann `--pyramid` verkleinerte Kacheln
  abgelegt haben, womöglich bevor die Basis darunter fertig war.
- **Nur veraltete Kacheln der Pyramide neu bauen.** Einer Elternkachel sieht
  man nicht an, ob sie zu ihren Kindern passt, und ihre Zeit kann von einer
  anderen Uhr stammen oder von `--pyramid` gestempelt sein; jede Regel dafür
  hatte eine Lücke.

## Folgen

- Die zwei Minuten setzen voraus, dass das System jede Kachel so schnell
  auf die Platte bringt und die Uhr nicht springt; sonst rendert erst ein
  Lauf ohne den Schalter sicher alles neu.
- Nach einer Änderung der Welt oder der Assets behielte `--resume` jede
  alte Kachel, die der Lauf noch nicht erreicht hat. Der Schalter gehört nur
  an einen abgebrochenen Lauf.
- Die Pyramide neu kostete ohne native Stufen bei der Testwelt rund 2 von 8
  Minuten; nach einem Abbruch in der Basis kostet das Fortsetzen trotzdem
  nur die zwei Minuten mehr.
