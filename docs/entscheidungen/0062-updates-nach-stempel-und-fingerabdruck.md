---
title: "0062: Updates nach Stempel und Fingerabdruck je Chunk"
description: Warum ein Update Chunks nach dem Zeitstempel im Regionskopf vorfiltert und nach einem Fingerabdruck je Chunk entscheidet, warum der Stand je Baum liegt, die Höhe nach unten voll bleibt und ein anderer Build oder andere Assets einen vollen Lauf verlangen.
status: gilt
date: 2026-10-03
issues: [100]
code:
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
  - renderer/src/world/chunk.rs
  - renderer/src/world/region.rs
  - renderer/src/cli.rs
---

# 0062: Updates nach Stempel und Fingerabdruck je Chunk

Im Fingerabdruck des Renderers abgelöst durch
[0098](0098-der-zeichenstand-statt-des-builds.md): Er hängt am
Zeichenstand und den eingebauten Tabellen statt an der ausführbaren Datei.

## Anlass

Jeder Lauf zeichnete die ganze Welt neu: die grosse Welt in rund 76 bis
95 min, Cinematic in rund 3 bis 5 h, siehe [Was ein Lauf kostet](../benutzung/kosten.md).
Die Karte soll sich regelmässig aktualisieren (#100). Ausschnitte in einen
bestehenden Baum gab es schon. Es fehlte, Änderungen zu finden und viele
Stücke in einem Lauf zu zeichnen.

An #100 ist belegt: Vanilla und Paper schreiben jeden Chunk neu, den ein
Spieler geladen hat, auch ohne Änderung. Der Zeitstempel im Kopf heisst
also „geschrieben“, nicht „geändert“.

## Entscheidung

- **Je Baum ein Stand,** `stand.bin` neben `map.json`, geschrieben am Ende
  jedes vollen Laufs über die ganze Welt und jedes Updates. Er nennt je
  Chunk den Stempel vom Beginn des Laufs und was der Renderer aus ihm
  zeichnete.
- **Zwei Stufen:** Der Stempel aus dem Kopf, Zeit und Eintrag der Tabelle,
  filtert vor. Nur Chunks mit neuem Stempel liest das Update und vergleicht
  ihren Fingerabdruck: FNV-1a über Blöcke und Biome jeder Section als Läufe
  gleicher Werte, dazu die Daten der Blockentities, unabhängig von Palette
  und Feldnamen.
- **Ein Fingerabdruck je Chunk,** nicht je Section, dazu der höchste Block,
  der nicht Luft ist.
- **Das Gebiet einer Änderung:** 16 Blöcke rundum; in der Höhe von der
  Unterkante der Dimension bis 16 über dem höheren von altem und neuem
  höchstem Block, mindestens bis 16 über dem höchsten der acht Nachbarn;
  mit Cinematic dazu die Strahlen zur Sonne und der Rand des Bloom.
  Aufgerundet auf Kacheln der gröbsten nativen Stufe.
- **Während des Laufs geschrieben:** Am Ende liest der Lauf die Köpfe noch
  einmal. Ein Chunk mit anderem Stempel ist im Stand unbekannt und gilt
  beim nächsten Mal als geändert, über die volle Höhe.
- **Kacheln ohne neuen Stand:** Jeder Lauf, der Kacheln zeichnet, macht vor
  der ersten Kachel im alten Stand unbekannt, was er liest, samt den acht
  Nachbarn; ein voller Lauf alles. Der Stand nennt so nie einen Inhalt, den
  eine Kachel vielleicht nicht zeigt, auch nach einem Abbruch oder einem
  Ausschnitt.
- **Anderer Renderer, andere Assets:** Der Stand trägt einen Fingerabdruck
  der ausführbaren Datei und einen der Dateien unter `--assets` und
  `--data` (Pfad, Grösse, Zeit). Passt einer nicht, bricht `--update` vor
  der ersten Kachel ab und verlangt einen vollen Lauf. `--resume` übernimmt
  den angefangenen Stand nur mit denselben Fingerabdrücken.

Die Einzelheiten stehen in [Updates](../benutzung/updates.md).

## Verworfene Alternativen

- **Nur der Stempel.** Wo ein Spieler war, sind je Spieler mindestens
  (2 · 10 + 1)² = 441 Chunks neu geschrieben, siehe #100. Ein Update
  zeichnete dort alles neu.
- **Ein Fingerabdruck je Section.** Mit 24 Sections zu 8 Byte wären es rund
  470 MB je Baum für die grosse Welt statt rund 47 MB. Gewonnen wäre nur die
  Oberkante: Ein Chunk mit hohem Bau über einer Änderung weiter unten
  zeichnete nur bis zur Änderung statt bis zum Bau. Nach unten bleibt es
  ohnehin die volle Höhe.
- **Der Fingerabdruck der rohen Bytes.** 26.3 benennt die Felder der
  Palette um. Jeder Chunk, den ein Server beim Wechsel auf 26.3 umschreibt,
  wäre eine Änderung.
- **Nach unten nur bis 16 unter die Änderung.** Himmelslicht fällt in einer
  Spalte beliebig tief: Ein Block hoch über dem Boden dunkelt den Boden
  darunter.
- **Vergleich auf später statt auf gleich.** Dann zählte die Uhr des
  Servers gegen die des Renderers, und eine Sicherung mit älteren Stempeln
  würde übersehen.
- **Nach einem Abbruch den alten Stand stehen lassen.** Wechselt ein Chunk
  danach zurück, etwa ein Ofen, der wieder ausgeht, gleicht er dem Stand,
  und kein Update zeichnet die Kachel mit dem brennenden Ofen mehr.
- **Nur die geänderten Chunks unbekannt machen.** Was der Server während
  des Laufs schreibt, zeichnet der Lauf vielleicht schon; vorher weiss das
  niemand.
- **Den Stand vor einem vollen Lauf entfernen.** Dann wüsste ein Lauf mit
  `--resume` nicht mehr, in welche Kacheln ein Chunk früher reichte.
- **Ein Stand je Welt, für alle Bäume.** Jeder Baum hat seine eigene Zeit,
  ein Update eines Baums machte den Stand eines anderen falsch.
- **Bei anderem Renderer nur warnen,** wie `--resume` es dokumentiert.
  Dann mischte das Update Kacheln zweier Stände des Renderers still. Eine
  Konstante, die bei jeder Änderung am Bild wächst, würde vergessen; der
  Fingerabdruck der Datei braucht keine Pflege.

## Folgen

- Ein Update gleicht Byte für Byte einem vollen Lauf über dieselbe Welt.
  Die Tests prüfen es mit Licht unter einem Dach über einer Chunkgrenze,
  einem Schatten im Himmelslicht 55 Blöcke tiefer und Türmen, die
  entstehen und verschwinden.
- Jeder neue Build verlangt einen vollen Lauf, bevor `--update` wieder
  geht, auch wenn er dasselbe Bild zeichnet.
- Ein voller Lauf liest jeden Chunk ohnehin; den Fingerabdruck rechnet er
  im Vorlauf mit.
- Kacheln, in die ein Chunk nicht mehr reicht, der noch da ist, zeichnet
  ein Lauf mit Stand neu und entfernt sie, wenn sie leer sind; ohne Stand
  bleiben sie bis `--prune` stehen wie vorher.
- Ein Werkzeug, das einen Chunk ohne neue Zeit an derselben Stelle
  überschreibt, bemerkt ein Update nicht.
