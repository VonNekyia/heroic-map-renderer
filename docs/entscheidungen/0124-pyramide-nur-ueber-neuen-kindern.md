---
title: "0124: Pyramide eines Updates nur über Kindern ohne Treffer"
description: Warum ein Update eine Elternkachel nur baut, wenn eines ihrer Kinder keinen Treffer im Hash der Pixel hatte, wie ein Treffer belegt, dass der letzte fertige Lauf die Eltern aus genau dieser Datei baute, warum entfernte, liegen gelassene und mit gleichen Bytes geschriebene Kinder immer Eltern bauen, leere ohne Datei nie, und warum ein Umstellen und ein voller Lauf alles bauen; löst 0091 in einem Punkt ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
---

# 0124: Pyramide eines Updates nur über Kindern ohne Treffer

Löst in [0091](0091-gleiche-pixel-nicht-kodieren.md) die verworfene
Alternative „Eltern nur über unveränderte Kinder überspringen“ ab: nicht
über unveränderten Kindern, sondern über Kindern mit Treffer.

## Anlass

Ein Update baut die Eltern aller Kacheln seines Gebiets neu. An der
Testwelt mit 16 Änderungen an der Oberfläche bekommen von 1960 Kacheln im
Gebiet 10 neue Pixel, und die Pyramide kostet 2,8 bis 3,1 von 15 s, siehe
[2026-10-24, Updates je Phase](../messungen/2026-10-24-updates-je-phase.md).
Gebraucht wären nur die Eltern der 10.

## Entscheidung

- **Die Regel:** Ein Update baut eine Elternkachel von der Platte nur, wenn
  eines ihrer Kinder auf der Stufe darunter keinen Treffer hatte, oder wenn
  sie fehlt (Waisen). Das geht Stufe für Stufe hinauf: Eine neu gebaute
  Elternkachel mit Treffer zählt für ihre Eltern wie ein unverändertes Kind.
- **Treffer** heisst `Pixel::gleich` in diesem Lauf, also derselbe Hash, und
  die Datei hat noch Grösse und Zeit des Eintrags (`Pixel::getroffen` in
  `renderer/src/cli/pixel.rs`).
- **Warum ein Treffer genügt:** Die Hashes schreibt ein Lauf erst nach der
  Pyramide. Ein Treffer heisst also: Der letzte fertige Lauf hat aus genau
  dieser Datei die Eltern gebaut.
- **Immer Eltern bauen:**
  - ein Kind mit neuen Pixeln;
  - ein Kind, das mit gleichen Bytes, aber ohne gültigen Eintrag liegen
    blieb, etwa nach einem abgebrochenen Lauf, der es schrieb;
  - ein Kind, das `--resume` liegen lässt;
  - ein Kind, das dieses Update entfernt, das vorher eine Datei hatte.
- **Nie Eltern bauen:** ein Kind, das leer ist und vorher keine Datei
  hatte. Für seine Eltern ändert sich nichts. Das gilt ohne native Stufen,
  mit ihnen zählen leere wie bisher.
- **Nur in einem Update** und nicht, wenn es die Pyramide umstellt
  (`pyramide_nur_ueber_neuen` in `renderer/src/cli.rs`). Beim Umstellen
  entstehen die Pixel der Eltern anders, auch über Treffern. Ein voller
  Lauf und ein Ausschnitt bauen wie bisher jede Elternkachel.
- **Die Eltern im Speicher** ([`ImSpeicher`](../benutzung/zoomstufen.md))
  entstehen wie bisher. Sie lesen nichts von der Platte und kosten wenig;
  die Regel spart die Pyramide von der Platte.

## Verworfene Alternativen

- **Eltern über unveränderten Kindern überspringen,** wie in 0091 erwogen:
  „Unverändert“ hiess dort nur gleiche Pixel. Ein abgebrochener Lauf, der
  ein Kind schrieb und die Eltern nicht mehr baute, hinterliess dann eine
  veraltete Elternkachel. Ein Treffer schliesst das aus, denn sein Eintrag
  gilt nach dem Schreiben nicht mehr.
- **Die Regel auch im Speicher:** Eine übersprungene Elternkachel fehlte
  dort als Viertel für ihre Eltern und müsste dann von der Platte kommen.
  Mehr Code für wenig.
- **Die Regel auch im vollen Lauf:** Ändert ein Build das Verkleinern, ohne
  die Basis zu ändern, blieben alte Eltern stehen. Ein voller Lauf baut
  darum alles.

## Folgen

- **Kosten nach Regel 26:** Im Live-Betrieb fällt die Pyramide von der
  Platte auf die Vorfahren der Kacheln mit neuen Pixeln; in der Messung
  oben rund 18 % je Update mit Änderungen. Ein Update ohne Änderung, der
  erste Render, Arbeitsspeicher und Platz bleiben gleich.
- **Die Ausgabe** nennt es: `Pyramide:   … über N Kacheln mit gleichen
  Pixeln nicht neu gebaut`.
- **Geprüft** in `renderer/tests/cli.rs`:
  - `update_baut_eltern_nur_ueber_neuen_kindern`;
  - `update_nach_abbruch_gleicht_einem_vollen_lauf`, mit Abbruch in der
    Basis und vor der Pyramide, mit und ohne `--resume`, über den
    Testhaken `HEROIC_ABBRUCH`, siehe [Tests](../entwicklung/tests.md),
    „Testhaken“;
  - `update_baut_die_eltern_entfernter_kinder`;
  - `update_gleicht_einem_vollen_lauf` wie bisher.
