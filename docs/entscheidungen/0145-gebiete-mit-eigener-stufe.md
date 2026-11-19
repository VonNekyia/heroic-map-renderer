---
title: "0145: Gebiete mit eigener Stufe"
description: Warum ein Gebiet Kacheln je Chunk hebt, mit den Höhen aus dem Stand, warum je Kachel die höchste Stufe gilt und das Update die Tabelle über alle Gebiete rechnet; dass --compact-tree Bäume mit Gebieten ablehnte, löst 0146 ab.
status: gilt
date: 2026-11-16
issues: [421]
code:
  - renderer/src/cli/gebiete.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# 0145: Gebiete mit eigener Stufe

## Anlass

Ein Betreiber soll Gebiete besser packen können als den Baum, etwa Städte,
die ein anderes Plugin als Vielecke meldet (#421). Das Plugin schreibt sie
in eine Datei; der Renderer liest keine Schutzzonen selbst. Stufen und
Packung stehen in [0143](0143-stufen-farben-zusammenlegen.md).

## Entscheidung

- **Schalter und Datei:** `--quality-areas <datei>`, wie in
  [Gebiete mit eigener Stufe](../benutzung/gebiete.md), „Die Datei“. Ohne
  den Schalter gelten die Gebiete, die der Baum sich gemerkt hat. Fehlt die
  Datei, ist sie kaputt oder grösser als 64 MiB, bricht der Lauf ab: Das
  Plugin tauscht die Datei, eine halbe gibt es von ihm nicht.
- **Spalte im Gebiet,** wenn ihre Mitte im Vieleck liegt, gerade/ungerade,
  und nur in der Hülle der Chunks, die der Lauf kennt: im Rechteck um die
  Regionen der Welt und des Baums, für `--pyramid` um die des Stands.
- **Kasten je Chunk** mit mindestens einer Spalte im Gebiet, ohne die Ränder
  eines Updates. In der Höhe von der Section des tiefsten ersten dichten
  Blocks im Chunk und seinen Nachbarn bis über seinen höchsten Block, beides
  aus dem Stand des Laufs.
- **Je Kachel die höchste Stufe** aus Baum und Gebieten; eine Elternkachel
  hat die höchste ihrer Kinder.
- **Ändert sich ein Chunk neben einem Gebiet,** ändert sich auch der Kasten
  des Gebiets, wenn das K des Chunks sinkt. Das Gebiet eines Updates fasst
  ihn: Es nimmt um jeden geänderten Chunk dieselben acht Nachbarn wie der
  Kasten, von der Grenze nach unten bis über den höchsten Block. Was sich am
  Kasten eines Nachbarn ändert, liegt also im Gebiet des Updates.
- **Ein Update rechnet die Tabelle über alle Gebiete,** nach seinem neuen
  Stand. Ändern sich die Gebiete, zeichnet es dazu die Kacheln mit neuer
  Stufe.
- **Der Baum merkt sich die Gebiete** ohne `id` in `gebiete.json`, mit dem
  Stand; der laufende Lauf hält seine in `gebiete-neu.json` fest. Für
  `--resume` gilt dieselbe Regel wie bei der Stufe.
- **`--compact-tree` lehnt einen Baum mit Gebieten ab.** Abgelöst durch
  [0146](0146-umpacken.md): Es packt jetzt je Kachel mit den Stufen der
  Gebiete.

## Verworfene Alternativen

- **Ohne Hülle:** Die Spalten liefen über die ganze Fläche des Vielecks.
  Ein Gebiet bis an die Weltgrenze, etwa „die ganze Welt“ eines fremden
  Plugins, hätte rund 14 Billionen Chunks und hinge jeden Lauf auf, auch
  jedes Update. Ohne Stand bekäme jeder davon Kacheln über die ganze Höhe.
- **Ein Kasten je Spalte:** Er hebt höchstens einen Chunk Rand weniger, auf
  der ganzen Karte höchstens rund 0,5 % der Kacheln. Dafür bräuchte er
  Läufe je Zeile statt eines Kastens je Chunk, wie ihn das Gebiet eines
  Updates schon hat. Zu viel gehoben kostet nur Bytes.
- **Die Höhen aus `heights/`:** Sie sind je 4 × 4 Blöcke der Median der
  obersten Blöcke, dazu geglättet, für die Koordinaten im Frontend. Für
  einen Turm in einer Stadt läge der Kasten darum zu tief, und seine Spitze
  packte mit der Stufe des Baums. Der Stand kennt je Chunk den höchsten
  Block und die Grenze nach unten, im vollen Lauf, im Update und für
  `--pyramid`.
- **Die Stufe des Gebiets statt der höchsten:** Bei zwei Gebieten
  übereinander hinge die Packung an ihrer Reihenfolge in der Datei.
- **Im Update nur die Gebiete nahe der Änderung:** Eine Elternkachel bis
  Zoom 0 hat die höchste Stufe aller Kinder, auch ferner. Ein Update, das
  ferne Gebiete auslässt, packte grobe Kacheln anders als ein voller Lauf.
  Die Tabelle über alle Gebiete kostet in der CI im Release-Build
  höchstens rund 1,5 ms je 1000 Chunks in Gebieten, siehe
  [Gebiete mit eigener Stufe](../benutzung/gebiete.md), „Was es kostet“.
- **Die Tabelle je Zoomstufe im Baum ablegen:** Ein Update müsste sie
  fortschreiben, und sie bräuchte einen eigenen Weg bei Abbruch und
  `--resume`. Aus Gebieten und Stand lässt sie sich jedes Mal rechnen.
- **Eine Warnung statt des Abbruchs bei einer zu grossen Datei:** Dann
  gälte stillschweigend etwas anderes, als das Plugin meint. Eine Regel für
  alle Fehler ist einfacher zu prüfen.
- **`--compact-tree` lässt gehobene Kacheln aus:** Das braucht die Tabelle
  im Nachverdichten; `--repack` packt Bäume mit Gebieten ohnehin.

## Folgen

- Jeder Lauf mit Gebieten rechnet die Tabelle; ohne Gebiete rechnet,
  liest und schreibt er nichts dafür.
- Die groben Stufen packen mit der höchsten Stufe aller Gebiete darunter
  und werden dadurch grösser.
- `--pyramid` hebt mehr als der Lauf um Chunks, die der Server während des
  Laufs schrieb, und lehnt einen Baum mit Gebieten ohne Stand ab, siehe
  [Gebiete mit eigener Stufe](../benutzung/gebiete.md), „`--pyramid`“.
- Ein Ausschnitt merkt sich keine Gebiete; seine Kacheln packt das nächste
  Update wie ein voller Lauf.
- **Ein Renderer ohne Gebiete** kennt `gebiete.json` nicht. Sein Update
  packt die Kacheln, die es zeichnet, mit der Stufe des Baums, und lässt
  `gebiete.json` liegen. Ein Renderer mit Gebieten sieht danach dieselben
  Gebiete wie vorher und zeichnet diese Kacheln nicht neu; sie bleiben mit
  der Stufe des Baums gepackt, bis ein voller Lauf sie neu packt. Stufen und
  Gebiete kommen darum im selben Release: Einen Renderer mit Stufen ohne
  Gebiete gibt es nicht.
