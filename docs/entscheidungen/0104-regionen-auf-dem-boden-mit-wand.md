---
title: "0104: Regionen auf dem Boden, mit Wand und Nebel"
description: Warum Regionen, Kreise und Linien in schrägen Ansichten auf dem Boden ohne Laub liegen und unter Kronen ganz zu sehen bleiben, warum am Rand einer Fläche eine Wand nach oben durchsichtiger wird, wie hoch und wie deckend, und warum der Nebel die Füllung ist; ergänzt 0096.
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
  sichtbar wie bisher. Beides Wunsch des Users: Die Linie soll man durch
  Blöcke hindurch sehen und nie verlieren.
- **Nadeln, Banner und Kartenschrift** bleiben auf `heights`, wie die
  Koordinaten: Sie stehen auf dem, was das Bild zeigt.
- **Die Wand:** an jedem Rand einer Fläche, der gezeichnet wird, 6 Blöcke
  hoch, in der Farbe des Rands ohne Alpha, am Boden 0,6 deckend, nach oben
  linear bis 0. Der User wählte am Bild die Richtung „nach oben
  durchsichtiger“ gegen „nach oben deckender“ und die Höhe 6. Hinter
  Gelände fällt sie weg.
- **Der Nebel** ist die Füllung, wie das Plugin sie schickt. Die Doku
  empfiehlt etwa 15 % Deckkraft.
- **Kein neues Feld:** Wand und Nebel folgen aus Rand und Füllung. Ohne
  Rand, mit `width: 0`, gibt es auch keine Wand.

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

## Folgen

- Eine Ebene mit Flächen lädt im iso `ground` zusätzlich, 32 KB je Region;
  Nadeln und Schrift laden es nicht. Eine Karte ohne Flächen lädt und
  zeichnet nichts mehr.
- Je Ebene und Farbe eines Rands 12 Pfade mehr; ihre Punkte wachsen mit dem
  Umfang der Flächen, nicht mit der Fläche.
- Ältere Bäume ohne `ground` zeichnen wie bisher, auf `heights`, bis ein
  `--heights` die Dateien nachträgt.
