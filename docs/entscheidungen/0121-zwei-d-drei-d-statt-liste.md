---
title: "0121: 2D und 3D statt der Liste der Ansichten"
description: Warum die Webkarte bei einem Baum von oben und einem iso statt der Liste der Bäume einen Knopf 2D/3D zeigt, an derselben Stelle der Welt, warum weitere Bäume nur über ?tree= offen sind und warum die Ansichten mit 1 und 2 px je Block aus der Webkarte fallen; löst 0109 für das Frontend ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/main.ts
---

# 0121: 2D und 3D statt der Liste der Ansichten

## Anlass

Der User am 24.10. über die Webkarte auf der Produktion: Oben soll statt
der Liste der Ansichten ein Knopf stehen, der zwischen 2D und 3D
umschaltet, an derselben Stelle der Welt. Die Ansichten mit 1 und 2 px je
Block zeigt die Webkarte dort nicht; sie sind für den Mod da. Plan des
Frontends, freigegeben vom Reviewer am 24.10.

## Entscheidung

- **2D** ist der erste Baum aus `trees.json` mit der Kamera `top-north`,
  der nicht einfarbig ist; **3D** der erste mit einer Kamera W:H.
- **Gibt es beide,** steht oben rechts ein Knopf mit dem Namen der anderen
  Art. Er wechselt wie bisher die Liste: derselbe Block in der Mitte,
  dieselbe Grösse eines Blocks. Aus jedem anderen Baum führt er nach 2D.
- **Weitere Bäume** sind nur über `?tree=` offen.
- **Fehlt eine Art,** bleibt die Liste aller Bäume.
- **Keine Ansichten mit 1 und 2 px je Block** auf der Webkarte, samt `?px`.
  Dieselben Stufen zeigt die Webkarte beim Herauszoomen ohnehin. Der Mod
  lädt sie weiter nach
  [0109](0109-1-und-2-px-als-stufen.md); daran ändert sich nichts.

## Verworfene Alternativen

- **`web: false` je Baum, gesetzt vom Plugin:** ein neues Feld in
  `trees.json` zwischen Backend, Plugin und Frontend. Die Webkarte weiss
  auch ohne es, welcher Baum 2D und welcher 3D ist.
- **Die Liste behalten und kürzen:** Der User will einen Knopf.

## Folgen

- Löst in 0109 den Teil „das Frontend die gekappten Ansichten“ ab.
- Ein alter Link mit `?px` zeigt die volle Ansicht.
- Das Feld `downscale` in `map.json` liest das Frontend nicht mehr.
