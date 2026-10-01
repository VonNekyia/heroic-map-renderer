---
title: "0049: Umriss nur ohne Zeiger"
description: Warum die Karte den Umriss des Blocks unter dem Zeiger nur beim Tippen mit Finger oder Stift zeichnet und mit der Maus nicht.
status: gilt
date: 2026-10-01
issues: [71]
code:
  - web/src/main.ts
  - web/tests/smoke.spec.ts
---

# 0049: Umriss nur ohne Zeiger

Löst aus [0035](0035-koordinaten-aus-hoehenkarten.md) den Satz ab, dass die
Maus den Umriss zeigt; alles andere dort gilt.

## Anlass

Der Maintainer wollte den Umriss nur noch auf dem Handy: Am Rechner zeigt
der Mauszeiger, wohin man zielt. Issue #71.

## Entscheidung

- Den Umriss zeichnet nur ein Zeiger, der keine Maus ist: `pointerType`
  `touch` oder `pen`. Massgeblich ist das letzte `pointerdown` oder
  `pointermove` auf der Karte.
- Die Koordinaten unten links zeigt die Karte für jeden Zeiger.
- Ein Gerät mit Touch und Maus zeigt den Umriss beim Tippen, und er
  verschwindet, sobald sich die Maus bewegt.

## Verworfene Alternativen

- **Nach dem Gerät statt nach dem Zeiger**, etwa mit
  `matchMedia('(pointer: coarse)')`. Ein Laptop mit Touchscreen hätte dann
  für beides dieselbe Antwort.

## Folgen

- Mit der Maus gibt es keine Linie mehr auf der Karte, nur die Anzeige.
