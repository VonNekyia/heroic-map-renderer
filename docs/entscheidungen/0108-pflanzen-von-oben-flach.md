---
title: "0108: Pflanzen liegen von oben flach"
description: Warum der Renderer Bodenpflanzen in top und top-north als ihre Textur flach über den Block legt, statt sie mit ihren senkrechten Flächen verschwinden zu lassen; welche Varianten der User an der Vorschau verworfen hat, warum Cinematic bleibt und was das Neu-Rendern kostet.
status: gilt
date: 2026-10-18
issues: [295]
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/stand.rs
---

# 0108: Pflanzen liegen von oben flach

Löst den Teil von [0051](0051-kameras-und-richtungen.md) und
[0052](0052-genordete-kameras.md) ab, nach dem Gras und Blumen von oben
verschwinden.

## Anlass

Von oben stehen die senkrechten Flächen eines Kreuzes auf der Kante und
fallen weg. Wiesen, Felder und Seegras waren in `top` und `top-north`
leer, auch in den Stufen mit 1 und 2 px je Block, die der Mod lädt
(#293). Der User will die Pflanzen von oben angedeutet, so dass es gut
aussieht (#295); dieselbe Regel soll die Minimap des Mods bekommen
(mod#133).

## Entscheidung

Von oben legt der Renderer jede Bodenpflanze, deren Modell keine Fläche
zur Kamera hat, als Textur ihrer ersten senkrechten Fläche flach über den
ganzen Block: Norden oben, Westen links, 1/16 über dem Boden des Blocks,
getönt wie die Fläche. Die übrigen Flächen fallen weg, ausser denen einer
Flüssigkeit. Die Regel im Einzelnen steht in
[Die Kamera](../renderer/kamera.md), „Pflanzen von oben“.

- **Gilt** für die Karte und die einfarbige Ansicht, in `top` und
  `top-north` bei jedem scale.
- **Nicht für Cinematic.** Dort bleiben die Kreuze, wie sie sind.
- Der User hat die Variante am 18.10. an einer Vorschau aus der Testwelt
  gewählt: Feld am Dorf, Seegras, Wiese mit Blumen, bei 4, 2 und 1 px je
  Block.

## Verworfene Alternativen

Alle drei an derselben Vorschau, gegen das Bild ohne Pflanzen:

- **Flach, halb so gross in der Mitte des Blocks:** Die Felder zeigen
  ihre Reihen nur bei 4 px. Bei 2 und 1 px trifft die Pyramide, die je
  2 × 2 einen Pixel nimmt, die Mitte kaum.
- **Wenige Punkte in der mittleren Farbe der Textur:** Bei 4 und 2 px gut
  zu sehen, bei 1 px fehlt alles, weil die Punkte neben dem Pixel liegen,
  den die Pyramide nimmt. Blumen bekommen eine gemischte Farbe aus Stiel
  und Blüte.
- **Die Textur der ersten Fläche wählen, wie sie im Modell liegt:** Die
  Lage hinge an der Reihenfolge der Ecken im Modell. Das Rechteck der UV
  mit Norden oben kann jede andere Ansicht genau so nachbauen.
- **Cinematic mit flachen Pflanzen:** Der Zeichenstand gilt je Look, nicht
  je Kamera ([0101](0101-zeichenstand-je-look.md)). Jeder Baum mit
  Cinematic müsste neu rendern, auch die schrägen, die sich nicht ändern;
  8:5 mit Cinematic brauchte auf der grossen Welt 5 h 26 min, siehe
  [2026-10-04, Vollrender mit Cinematic](../messungen/2026-10-04-vollrender-cinematic.md).
  Wer Cinematic von oben mit flachen Pflanzen will, entscheidet das
  eigens.

## Folgen

- **Zeichenstand:** `ZEICHENSTAND_MAP` steigt von 3 auf 4,
  `ZEICHENSTAND_FLAT` von 4 auf 5, `ZEICHENSTAND_CINEMATIC` bleibt 3.
- **Neu rendern:** Jeder Baum mit `look` `map` braucht einmal einen vollen
  Lauf, auch die schrägen, deren Bild gleich bleibt; ebenso jeder Baum mit
  `look` `flat`. Ein Update bricht bis dahin ab, siehe
  [Updates](../benutzung/updates.md), „Der Stand“.
- **Kosten nach Regel 26:** Eine Pflanze zeichnet von oben eine Fläche
  statt keiner. Gemessen ist das nicht; mehr als ein Rauschen in Zeit und
  Grösse ist nicht zu erwarten, denn eine Pflanze deckt höchstens ihren
  Block.
- **Gras** ist getönt wie der Grasblock und verschwindet fast in ihm, wie
  im Spiel von oben.
