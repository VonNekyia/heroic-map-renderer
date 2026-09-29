---
title: "0042: Die feinen Stufen der Pyramide entstehen im Export aus dem Speicher"
description: Warum ein Export die Stufen über der Basis bis zur Breite eines Streifens aus den Bildern im Speicher baut, über die Threads hinweg, und alles andere wie bisher von der Platte.
status: gilt
date: 2026-09-29
issues: [39, 38]
code:
  - renderer/src/cli.rs
  - renderer/src/render/pyramid.rs
---

# 0042: Die feinen Stufen der Pyramide entstehen im Export aus dem Speicher

## Anlass

#39: Die Pyramide entstand erst nach der Basis, und jede Elternkachel las
und dekodierte ihre vier Kinder von der Platte. Die Stufen direkt über der
Basis werden aber schon im Streifen eines Threads fertig, und bei
Streifen von 8 Spalten sind die drei untersten rund 98 % der Pyramide.
Ein `--pyramid` neben dem Render baute sie ein zweites Mal.

## Entscheidung

- Jede gerenderte Kachel der Basis, mit nativen Stufen der gröbsten, gibt
  ihr Viertel ab. Wer das letzte Viertel einer Elternkachel abgibt, baut
  sie, schreibt sie und gibt ihr Viertel weiter, bis zu der Stufe, deren
  Kachel so breit ist wie ein Streifen.
- Welche Eltern so entstehen, steht vor dem Rendern fest, aus den Mengen
  des Laufs und den Listen der Stufen: jede, deren Kinder alle aus diesem
  Lauf kommen oder sicher fehlen. Alles andere baut der Durchgang am Ende
  von der Platte, wie bisher.
- Eine Elternkachel entsteht erst, wenn alle ihre Kinder fertig sind; es
  gibt keine vorläufigen Kacheln.

Wie das im Einzelnen geht: [Zoomstufen](../benutzung/zoomstufen.md),
„Feine Stufen im Speicher“.

## Verworfene Alternativen

- **Nur Eltern, deren Kinder alle derselbe Thread rendert,** und die an
  den Schnitten zwischen den Stücken der Threads und beim Stehlen von der
  Platte. Welche das sind, entschiede erst der Lauf, je nachdem, wer wann
  stiehlt, und der Durchgang am Ende müsste sie einsammeln. Über die
  Threads hinweg hängt es nur an den Mengen, und ein Test sieht jedes Mal
  dieselbe Aufteilung. Dafür wartet an jedem Schnitt eine Zeile, höchstens
  rund 1,3 MiB.
- **Alle Stufen im Speicher.** Über der Breite eines Streifens warten
  Eltern auf Streifen anderer Threads, bei scale 32 bis zu rund 13 000
  Kacheln mit je drei Vierteln, rund 2,4 GiB. Die groben Stufen sind 1,6 %
  der Pyramide und kosten von der Platte Sekunden.
- **Vorläufige Kacheln,** damit eine Ansicht während des Renders früher
  etwas zeigt. `--resume` müsste sie erkennen; einer Kachel sieht man
  nicht an, ob sie fertig ist.
- **Die Pyramide nur schneller von der Platte lesen.** Das ist #38; das
  Dekodieren bleibt, und `--pyramid` nebenher baut weiter alles zweimal.

## Folgen

- `--pyramid` bleibt, wie es ist, und findet die feinen Stufen neben einem
  Export frisch vor, siehe
  [0017](0017-pyramide-vergleicht-zeiten.md). `--resume` baut die
  Pyramide weiter ganz neu, siehe
  [0019](0019-resume-behaelt-die-basiskacheln.md).
- Ein Export listet die Stufen über der Basis ohnehin, für die Waisen; die
  Listen der feinen Stufen behält er jetzt bis zum Ende der Basis.
- Gemessen am Stand der Testwelt ohne native Stufen, in
  [2026-09-29, Pyramide von der Platte und im Speicher](../messungen/2026-09-29-pyramide-platte-und-speicher.md):
  - Nach der Basis bleiben 0,2 statt 1,4 s Pyramide, 1638 der 1857
    Elternkacheln entstehen während der Basis.
  - Die Basis braucht dafür 0,6 bis 0,7 s länger.
  - Der Export ist 4 bis 9 % kürzer, die Kacheln bleiben Byte für Byte
    gleich.
  - Die Speicherspitze steigt um 0,05 GiB.
  - Mit drei nativen Stufen ändert sich nichts Messbares.
