---
title: "0109: 1 und 2 px je Block als Stufen des Baums mit 4 px"
description: Warum die Karten mit 1 und 2 px je Block für Mod und Webkarte die Stufen z − 1 und z − 2 des Baums aus top-north mit 4 px sind und keine eigenen Bäume, wie ein Spieler mehrere Massstäbe hält und was eigene Bäume gekostet hätten.
status: gilt
date: 2026-10-18
issues: [293]
code:
  - renderer/src/render/pyramid.rs
  - renderer/tests/metatile.rs
---

# 0109: 1 und 2 px je Block als Stufen des Baums mit 4 px

Zum Teil abgelöst durch [0121](0121-zwei-d-drei-d-statt-liste.md): Die Webkarte hat keine
gekappten Ansichten mehr; der Mod lädt die Stufen wie hier.

## Anlass

Der User will Karten mit 1 und 2 px je Block, mit Texturen, von oben,
zusätzlich zu 4 px: im Mod zum Download, auf der Webkarte als eigene
Ansichten (#293). Auf der grossen Welt sollen sie rund 16- und 4-mal
kleiner sein als die 4,8 GB mit 4 px. Offen war, ob ein Spieler mehrere
Massstäbe je Baum halten kann; das Plugin braucht die Antwort.

## Entscheidung

Die Karten mit 1 und 2 px je Block sind die Stufen z − 2 und z − 1 des
Baums aus `top-north` mit scale 4, keine eigenen Bäume. Der Reviewer hat
das am 18.10. entschieden.

- **Schon da:** Die Pyramide von `top-north` nimmt seit
  [0094](0094-von-oben-der-naechste-pixel.md) je 2 × 2 einen Pixel; die
  Stufen bleiben scharf. Das Token trägt die feinste freigegebene Stufe,
  und `/download/<baum>.zip` liefert bis dorthin, von grob nach fein.
- **Mehrere Massstäbe je Baum:** ja, von selbst. Wer 4 px hat, hat 2 und 1
  dazu. Das Plugin führt dafür nichts je Spieler, nur einen Baum je Kamera.
- **Wer was baut:** das Plugin die Wahl der Stufe und den Deckel je Stufe
  aus dem Manifest; der Mod die Wahl 1, 2 oder 4 px und das Löschen
  feinerer Stufen; das Frontend die gekappten Ansichten; der Renderer nur
  Goldbilder und Doku.

Einzelheiten in [Zoomstufen](../benutzung/zoomstufen.md), „1 und 2 px je
Block“.

## Verworfene Alternativen

- **Eigene Bäume mit scale 2 und 1:**
  - `--scale` nimmt erst ab 4, und scale 1 von `top-north` ist die
    einfarbige Ansicht (`Projection::flach`); beides müsste sich ändern.
  - Je Baum ein voller Lauf über alle Chunks der Welt, dazu ein eigener
    Ordner und ein Feld `scale` in `trees.json` für die Namen im
    Umschalter. Das Lesen der Chunks kostet bei scale 2 so viel wie bei 4.
  - Nativ gerendert mittelt scale 2 die Texel. Das sieht weicher und
    anders aus als die Stufe z − 1, die beim Herauszoomen ohnehin zu sehen
    ist.
  - Ein Gewinn gegen die Stufen fand sich nicht.

## Folgen

- **Kosten nach Regel 26:** keine Renderzeit und kein Platz; die
  Goldbilder sind wenige KB.
- **Grösse:** Bis z − 1 sind es rund ein Viertel des Baums, bis z − 2 rund
  ein Sechzehntel, wie im Issue erwartet.
- **Andere Kameras** bekommen keine Karten mit 1 und 2 px. Ihre Pyramide
  mittelt, und die Stufen sind dort weicher.
