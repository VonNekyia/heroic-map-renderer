---
title: "0104: Regionen auf dem Boden, mit Wand und Nebel"
description: Warum Regionen, Kreise und Linien in schrägen Ansichten auf dem Boden ohne Laub liegen und unter Kronen ganz zu sehen bleiben, warum am Rand einer Fläche eine Wand nach oben durchsichtiger wird, wie hoch und wie deckend, warum Rand und Linie entlang des Wegs geglättet sind, und warum der Nebel ein gekacheltes Wolkenmuster in der Farbe der Füllung ist, als Band von 16 Blöcken innen am Rand, auf dem Schirm fest gross und gespiegelt gekachelt; ergänzt 0096; den Nebel löst 0105 ab.
status: gilt
date: 2026-10-10
issues: [273]
code:
  - web/src/formen.ts
  - web/src/gelaende.ts
  - web/src/ebenen.ts
---

# 0104: Regionen auf dem Boden, mit Wand und Nebel

## Anlass

In schrägen Ansichten lag eine Region auf der Oberfläche aus `heights`,
samt Baumkronen. Ihr Rand sprang über jede Krone, die Fläche wirkte
zerrissen, und im Wald war der Rand fast überall gestrichelt, weil Kronen
vor ihm lagen. Der User wünschte sich (#273): Bäume ignorieren, eine Wand
vom Rand nach oben mit einem Verlauf der Deckkraft, innen ein leichter
Nebel in der Farbe der Region; von oben bleibt alles, wie es ist.

## Entscheidung

Die Regeln stehen in [Ebenen](../benutzung/ebenen.md), „Zeichnen“. Im Kern,
nur für schräge Ansichten:

- **Auf dem Boden:** Flächen, Ränder, Kreise und Linien liegen auf den
  Höhen ohne Laub aus `ground`, siehe
  [0103](0103-boden-ohne-laub.md) und [map.json](../benutzung/map-json.md),
  „Höhen“. Fehlt einer Region die Datei, liegen sie dort auf `heights`.
  Linien auch, Entscheid des Reviewers: Eine Route durch den Wald sprang
  sonst ebenso.
- **Unter Kronen ganz zu sehen:** Auch die Prüfung, was verdeckt ist,
  nimmt den Boden. Kronen verdecken also nichts; ein Rand oder eine Linie
  unter ihnen ist über ihnen gezeichnet, voll und nicht gestrichelt.
  Hinter einem Hang des Bodens bleibt sie dünn, gestrichelt und blass
  sichtbar wie bisher. Hinter einem Haus oder einer Mauer bleibt sie voll:
  Ein verdecktes Stück kürzer als 16 Blöcke gilt als Haus. Beides Wunsch des Users: Die Linie soll man durch
  Blöcke hindurch sehen und nie verlieren.
- **Geglättet entlang des Wegs:** Ränder, Linien und Kreise springen nicht
  über Häuser und Mauern; je Punkt erst der Median über ±8 Blöcke des Wegs,
  dann das Mittel über ±4, im Frontend, nicht in `ground`. Die Wand hängt
  an derselben geglätteten Linie. Wunsch des Users vom
  11.10., das Verfahren wählte das Frontend.
- **Nadeln, Banner und Kartenschrift** bleiben auf `heights`, wie die
  Koordinaten: Sie stehen auf dem, was das Bild zeigt.
- **Die Wand:** an jedem Rand einer Fläche, der gezeichnet wird, also mit
  `fill`, `name` oder `panel`, 6 Blöcke
  hoch, in der Farbe des Rands ohne Alpha, am Boden 0,6 deckend, nach oben
  linear bis 0. Der User wählte am Bild die Richtung „nach oben
  durchsichtiger“ gegen „nach oben deckender“ und die Höhe 6. Hinter
  Gelände fällt sie weg.
- **Der Nebel,** abgelöst durch
  [0105](0105-regionen-im-iso-ohne-nebel.md): Am 11.10. wünschte der User,
  ihn ganz zu entfernen; im iso zeigen Rand und Wand die Fläche. Bis dahin
  galt: ein ruhiges Wolkenmuster in der Farbe der Füllung,
  zur Hälfte mit Weiss gemischt, gekachelt als `pattern` aus SVG, ohne Bild
  und ohne Filter. Der User wählte es an sechs Vorschauen aus der
  Testwelt, Variante 3, „Struktur“. Am 11.10. nach dem Testnetz änderte er
  drei Dinge: Der Nebel liegt nur in einem Band von 16 Blöcken innen am
  Rand; das Muster behält auf dem Schirm seine Grösse, auch beim Zoomen;
  es wiederholt sich weniger, mit gespiegelten Kacheln. Das Band besteht
  aus sechs Strichen, nach innen gleichmässig gestuft; mit dreien waren die
  Stufen am Bild hart, mit acht kaum weicher als mit sechs. Der User wählte
  sechs. Die Doku empfiehlt etwa 15 %.
- **Kein neues Feld:** Wand und Nebel folgen aus Rand und Füllung. Ohne
  Rand, mit `width: 0`, gibt es auch keine Wand.

Gewählt hat der User an Vorschauen aus der Testwelt; wie es aussieht, zeigt
[`regionen-boden.webp`](../bilder/regionen-boden.webp).

## Verworfene Alternativen

- **Weiter auf `heights`:** Das ist der Fehler aus #273.
- **Verdeckt weiter gegen `heights` prüfen, nur zeichnen auf dem Boden:**
  Dann wäre im Wald fast jeder Rand gestrichelt; der User will ihn ganz
  sehen.
- **Ein Rand unter Kronen ausblenden:** Gegen den Wunsch des Users, die
  Linie nie zu verlieren.
- **Nach oben deckender:** Am Bild verworfen; die Wand bekommt eine harte
  Oberkante und deckt mehr Karte zu.
- **Die Wand mit einem Verlauf je Strecke:** Ein Verlauf in SVG hängt an
  den Koordinaten des Schirms, die Leaflet bei jedem Zoom neu setzt; die
  Unterkante der Wand steigt und fällt mit dem Boden. Statt dessen Bänder
  mit fester Deckkraft, 12 Stück, je Farbe und Band ein Pfad je Ebene.
- **Felder für Wand und Nebel in der Ebene:** Regel 22; Rand und Füllung
  reichen.
- **Nebel als glatte Füllung:** der Stand vor der Wahl; der User wollte
  ihn realistischer.
- **Nebel mit weichen Rändern** (2), am Rand dichter, zur Mitte dünner,
  mit breiten Strichen, auf die Fläche beschnitten: am Bild verworfen.
- **Nebel in Schichten** (4), drei dünne Lagen bis 4,5 Blöcke hoch, und
  beide Mischungen, Ränder mit Schichten (2 + 4) und Struktur mit Schichten
  (3 + 4): am Bild verworfen.
- **Ein Muster aus einem Bild:** Die Content-Security-Policy der Karte
  erlaubt kein `data:`; das Muster ist darum aus SVG.
- **Ein Filter wie `feTurbulence`:** kostet über grossen Flächen bei jedem
  Zeichnen.
- **Nebel über die ganze Fläche:** der Stand vor dem 11.10.; der User
  wollte ihn nur am Rand.
- **Das Band als eigene Fläche,** die Fläche nach innen um 16 Blöcke
  verkleinert und abgezogen: genauer bei spitzen Ecken, braucht aber ein
  Versatzverfahren für Polygone mit Löchern. Striche auf dem Rand, auf die
  Fläche beschnitten, geben dasselbe Band mit dem, was schon da ist.
- **Das Muster während des Zooms gegenskalieren:** Leaflet skaliert per
  CSS-Übergang; ein `patternTransform` liefe nicht im Takt mit. Das Band
  blendet statt dessen für den Zoom aus.
- **Drei oder acht Striche im Band:** Drei zeigten harte Stufen. Acht
  hätten etwa ein Drittel mehr Fläche zum Zeichnen als sechs, gerechnet, nicht
  gemessen, und sehen kaum anders aus.
- **Ein unteres Quantil statt des Medians:** zieht die Linie an jedem Hang
  nach unten; der Median lässt eine gleichmässige Steigung, wie sie ist.
- **Glätten in `ground`:** Teleport und Deckung brauchen den Boden genau.
- **Ein Filter über das ganze Raster der Höhen:** rechnet je Zelle über
  17 × 17 Spalten und glättet auch, was keine Form berührt; entlang des Wegs
  sind es je Punkt rund 33 Werte.
- **Das Band an der Füllung beschneiden:** Ihr Rand folgt dem Boden
  ungeglättet und passt nicht mehr zur Linie. Das Band wird statt dessen am
  Umriss aus den geglätteten Rändern beschnitten und läuft nur auf
  sichtbaren Strecken. Ein Rest: Vor einem Kamm, der das Innere verdeckt,
  kann es bis zu 16 Blöcke über den Kamm reichen.
- **Gedrehte Kacheln:** Um 90° gedreht stünden die flachen Wolken
  senkrecht. Gespiegelt wird nahtlos, gedreht bräuchte eine eigene Kachel.
- **Eine Wand auch um einen Kreis nur mit Rand:** Am Bild der Ebenen stand
  um einen gestrichelten Umkreis eine volle weisse Wand; ein Umkreis ist
  eine Marke, keine Fläche. Entscheid des Frontends.

## Folgen

- Eine Ebene mit Flächen lädt im iso `ground` zusätzlich, 32 KB je Region;
  Nadeln und Schrift laden es nicht. Eine Karte ohne Flächen lädt und
  zeichnet nichts mehr.
- Je Ebene und Farbe eines Rands 12 Pfade mehr; ihre Punkte wachsen mit dem
  Umfang der Flächen, nicht mit der Fläche.
- Ältere Bäume ohne `ground` zeichnen wie bisher, auf `heights`, bis ein
  `--heights` die Dateien nachträgt.
