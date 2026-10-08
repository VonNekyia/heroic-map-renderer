---
title: "0091: Gleiche Pixel nicht kodieren"
description: Warum jeder Lauf je Kachel einen Hash ihrer Pixel festhält und eine Kachel mit denselben Pixeln nicht kodiert, warum der Hash nur bei gleicher Grösse und Zeit der Datei und gleichem Build gilt, warum er in Blöcken unter pixel/ liegt und nicht in stand.bin, und welche Wege verworfen sind.
status: gilt
date: 2026-10-08
issues: [207, 166]
code:
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# 0091: Gleiche Pixel nicht kodieren

## Anlass

#207, Teil A1: Ein Update zeichnet das ganze Gebiet der geänderten Chunks
neu, aber nur wenige Kacheln darin bekommen neue Pixel. Die übrigen kodierte
es trotzdem und merkte erst danach, dass die Bytes gleich sind (#166). Das
Plugin startet alle 2 min ein Update. Der Reviewer gab A1 am 08.10. frei,
der Maintainer bestätigte die Reihenfolge.

## Entscheidung

- **Vor dem Kodieren** bildet `schreibe_oder_lass` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs) einen Hash der
  RGBA-Bytes. Steht für die Kachel derselbe Hash fest und gilt er, kodiert
  und schreibt es nichts. Die Kachel bleibt liegen, mit ihrer Zeit und
  ihrem ETag, als hätte sie dieselben Bytes.
- **Auf jeder Stufe:** Basis, native Stufen und die Eltern, im Speicher
  wie von der Platte, laufen alle durch `schreibe_oder_lass`. Ein eigener
  Weg für Eltern über unveränderte Kinder entfällt.
- **Der Hash:** SHA-256 aus `ring`, das schon für die Token von `/download/`
  im Baum steckt (Regel 22). Davon die ersten 16 Bytes. Verglichen wird nur
  mit dem alten Hash derselben Kachel; dass zwei verschiedene Bilder
  zufällig gleich herauskommen, ist bei 128 Bit ausgeschlossen.
- **Wann er gilt:** nur für die Datei, die ein Lauf hinterliess, mit
  derselben Grösse und derselben Zeit der letzten Änderung auf die
  Nanosekunde, und nur für einen Renderer mit demselben Fingerabdruck wie
  in [Updates](../benutzung/updates.md), „Anderer Renderer, andere Assets“.
  - Schrieb jemand anders die Datei, ein abgebrochener Lauf, `--pyramid`
    oder ein Werkzeug, hat sie eine andere Zeit. Dann kodiert der Lauf wie
    ohne Hash.
  - Andere Bytes bei gleichen Pixeln, etwa nach
    [0090](0090-webp-mit-quality-75.md), kommen nur mit einem neuen Build.
    Dann gilt kein Hash.
- **Jeder Lauf** liest und schreibt die Hashes der Kacheln, die er ablegt:
  ein voller Lauf, ein Update, ein Ausschnitt und `--resume`.
  - Geschrieben wird am Ende, nach der Pyramide.
  - Bricht ein Lauf ab, fehlen nur seine Hashes; der nächste kodiert diese
    Kacheln einmal mehr.
  - Gegen die Ordnung des Stands, der nur von vollen Läufen und Updates
    stammt, braucht es so nichts: Ein Hash gilt nie für eine Datei, die er
    nicht beschreibt.
- **Ablage:** unter `pixel/` im Baum, je Stufe ein Ordner und je Block aus
  32 × 32 Kacheln eine Datei. Das Format steht in
  [Updates](../benutzung/updates.md), „Gleiche Pixel“.
  - Ein Lauf liest einen Block erst, wenn er eine Kachel darin ablegt.
  - Er schreibt nur Blöcke, in denen sich etwas geändert hat.
  - Ein Update liest und schreibt so nur Blöcke um sein Gebiet.

## Verworfene Alternativen

- **Die Hashes in `stand.bin`,** wie an #207 vorgeschlagen:
  - Ein Lauf mit Kacheln schreibt den Stand dreimal ganz: vor der ersten
    Kachel `stand.bin` mit dem unbekannten Gebiet und `stand-neu.bin`, am
    Ende wieder `stand.bin`. An der grossen Welt sind das schon heute rund
    47 MB je Datei.
  - Ein Eintrag je Kachel aller Stufen legte bei 3,3 Mio. Kacheln rund
    100 MB dazu. Jedes Update schriebe dann einige 100 MB, auch für wenige
    Kacheln.
  - Ein Ausschnitt schreibt keinen Stand, seine Hashes gingen verloren.
- **Gültigkeit nur über das Protokoll** wie beim Stand, also vor der
  ersten Kachel alles Gezeichnete aus den Hashes nehmen und erst am Ende
  eintragen: mehr Regeln für dasselbe. Grösse und Zeit der Datei prüft
  jeder Lauf ohnehin, das ETag hängt an denselben beiden.
- **Eltern nur über unveränderte Kinder überspringen,** ohne eigenen Hash:
  Eine Elternkachel, deren Kinder ein abgebrochener Lauf schrieb, bliebe
  dann veraltet stehen, und bei `--resume` stimmte die Regel nicht mehr.
  Der Hash der Eltern kostet im Speicher nur das Zusammensetzen. Von der
  Platte muss der Lauf die Kinder ohnehin lesen.
- **Die alte Kachel dekodieren und Pixel vergleichen:** ohne Ablage, aber
  rund 1 ms Lesen und Dekodieren je Kachel statt eines Hashs.
- **Ein schnellerer Hash ohne Kryptografie:** Dafür bräuchte es eine neue
  Crate, oder einen eigenen, über Builds stabil. SHA-256 ist da und reicht.
- **Ein kleineres Gebiet je Update** (#207, A2): erst nach dieser
  Entscheidung prüfen.

## Folgen

- **Ein Update** kodiert nur noch Kacheln mit neuen Pixeln. Ein voller
  Lauf über einen bestehenden Baum desselben Builds kodiert ebenso nur, was
  sich änderte.
- **Platz:** je Kachel 30 Byte unter `pixel/`. Speicher: je Kachel, die ein
  Lauf ablegt, ein Eintrag, solange er läuft.
- **Werkzeuge,** die die Zeiten der Kacheln ändern, etwa ein Kopieren ohne
  Zeiten, lassen den nächsten Lauf alles einmal kodieren.
- **Eine Kachel, die ein Werkzeug** mit anderen Bytes, derselben Grösse und
  derselben Zeit auf die Nanosekunde zurücklegt, bleibt stehen, bis sich
  ihre Pixel ändern.
- Gemessen in
  [2026-10-08, Gleiche Pixel nicht kodieren](../messungen/2026-10-08-gleiche-pixel.md).
