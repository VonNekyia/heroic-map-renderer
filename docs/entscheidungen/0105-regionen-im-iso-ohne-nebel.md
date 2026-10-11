---
title: "0105: Regionen im iso ohne Nebel"
description: Warum eine Fläche in schrägen Ansichten keinen Nebel und keine Füllung mehr zeigt, nur Rand und Wand, warum der Code des Nebels gelöscht statt abgeschaltet ist und was von oben bleibt; löst 0104 im Punkt Nebel ab.
status: gilt
date: 2026-10-11
issues: [273]
code:
  - web/src/formen.ts
  - web/src/ebenen.ts
---

# 0105: Regionen im iso ohne Nebel

## Anlass

[0104](0104-regionen-auf-dem-boden-mit-wand.md) legte in schrägen Ansichten
einen Nebel über Flächen: ein Wolkenmuster in ihrer Farbe, zuletzt als Band
von 16 Blöcken innen am Rand. Am 11.10. wünschte sich der User, den
Nebel-Effekt ganz zu entfernen.

## Entscheidung

- **Im iso kein Nebel und keine Füllung:** Eine Fläche zeigt sich dort nur
  durch ihren Rand und die Wand, beide entlang des Wegs geglättet, wie in
  0104. Die Füllung bleibt als unsichtbarer Pfad, Ziel für Name und Tafel.
- **Von oben** füllt eine Fläche weiter in der Farbe von `fill`.
- **Der Code ist gelöscht,** nicht abgeschaltet: das Muster, der Beschnitt
  auf den Umriss, die Striche des Bands und das Ausblenden beim Zoom.
  Wählt der User später wieder einen Nebel, steht der alte Weg in 0104 und
  in der Geschichte des Repositorys.
- Wahl des Users vom 11.10.

## Verworfene Alternativen

- **Den Nebel nur abschalten,** etwa über eine Konstante: Code, den nichts
  mehr ausführt, ist zu pflegen und zu testen, ohne dass er etwas zeigt.
- **Im iso wieder einfarbig füllen:** der Stand vor 0104; der User wollte
  die Fläche im iso nicht verdeckt.

## Folgen

- Je Fläche im iso keine Pfade, kein `pattern` und kein `clipPath` mehr für
  den Nebel; je Rand nur noch Linie und Wand.
- Das Feld `fill` bestimmt im iso nur noch die Farbe der Wand, wenn der Rand
  keine eigene hat; von oben die Füllung.
