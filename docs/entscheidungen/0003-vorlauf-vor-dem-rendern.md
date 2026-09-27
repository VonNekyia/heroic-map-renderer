---
title: "0003: Ein Vorlauf vor dem Rendern"
description: Warum der Renderer die Welt zuerst einmal ganz liest, bevor er parallel rendert, und warum die Kachelmenge aus den belegten Sections kommt.
status: gilt
date: 2026-09-22
issues: [5]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/render/sprites.rs
  - renderer/src/cli.rs
---

# 0003: Ein Vorlauf vor dem Rendern

## Anlass

Schritt 5 rendert die Welt parallel in Kacheln. Jeder Thread braucht dafür
die Sprites aller Blockstates, die in seinen Kacheln vorkommen.

## Entscheidung

Ein Vorlauf liest jeden Chunk einmal und beantwortet zwei Fragen auf einmal:
welche Blockstates vorkommen, und welche Kacheln überhaupt etwas zeigen
(`survey`). Danach steht die Sprite-Tabelle unveränderlich, und alle Threads
teilen sie ohne Sperre. Die Kachelmenge kommt aus den nicht-leeren Sections
je Chunk, nicht aus der Welthöhe. Siehe
[Der Weg einer Kachel](../renderer/renderpfad.md), „Vorlauf“.

## Verworfene Alternativen

- **Die Tabelle während des Renderns füllen.** Dann stünde eine Sperre im
  Renderpfad jedes Workers.
- **Die Kachelmenge aus der Welthöhe, y −64 bis 319.** Jede Blockspalte wäre
  auf dem Bild über 3000 px hoch, und der Vollrender malte überwiegend leere
  Kacheln. Mit den belegten Sections zeigten im Testausschnitt 72 von 72
  Kacheln etwas.

## Folgen

- Die Welt wird mehrmals gelesen: vom Vorlauf, von der Basis und von jeder
  nativen Stufe. Der Vorlauf kostete in Schritt 5 für die ganze Testwelt
  10,7 s, heute 5 bis 11 s.
- `survey` trägt einen `ponytail:`-Hinweis: Eine Blockstate-Liste neben der
  Welt spart den ersten Lauf, sobald er weh tut.
