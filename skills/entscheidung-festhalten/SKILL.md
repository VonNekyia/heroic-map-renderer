---
name: entscheidung-festhalten
description: Hält eine Entscheidung als eigene Datei in docs/entscheidungen/ fest. Nutzen, wenn eine PR eine Richtung festlegt, eine Alternative verwirft, eine Näherung bewusst in Kauf nimmt oder eine frühere Entscheidung ablöst.
---

# Entscheidung festhalten

## Ablauf

1. **Gibt es schon eine?** `git grep -il "<stichwort>" docs/entscheidungen/`.
   Ändert die neue eine alte, wird die alte abgelöst, nicht umgeschrieben
   (Schritt 5).
2. **Nummer:** die nächste freie, vierstellig. `ls docs/entscheidungen/`
   zeigt die letzte.
3. **Datei** `docs/entscheidungen/NNNN-kurzer-titel.md` nach der Vorlage
   unten.
4. **Inhalt:**
   - Anlass: welches Problem, welches Issue.
   - Entscheidung: was gilt, in einem Absatz.
   - Verworfene Alternativen: jede mit Grund, gemessen, wo es geht.
   - Folgen: was dadurch teurer, grösser, ungenauer oder unmöglich wird.
5. **Ablösen:** In der alten Datei nur `status: abgelöst durch NNNN` setzen
   und unter dem Titel einen Satz mit Verweis auf die neue ergänzen. Der
   Rest bleibt, wie er war.
6. **Verweisen:** Die Seiten in `docs/`, die das Thema beschreiben, und die
   Kommentare am Code verweisen auf die Entscheidung. Der Kommentar erzählt
   sie nicht nach.
7. **Eintragen:** eine Zeile in `docs/index.md`, ein Eintrag in `nav` in
   `mkdocs.yml`.

## Vorlage

```markdown
---
title: "0007: Weiche Beleuchtung zuerst nur für volle Würfel"
description: Warum die weiche Beleuchtung zuerst nur Modelle aus vollen Seiten abdunkelt.
status: gilt
date: 2026-09-27
issues: [15, 18]
code:
  - renderer/src/render/metatile.rs
---

# 0007: Weiche Beleuchtung zuerst nur für volle Würfel

## Anlass

## Entscheidung

## Verworfene Alternativen

## Folgen
```
