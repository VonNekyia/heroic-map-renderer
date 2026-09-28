---
title: "0035: Koordinaten aus Höhenkarten"
description: Warum das Frontend die Koordinaten unter Maus und Finger aus einer Höhenkarte je Region und dem Strahl durch das Pixel rechnet, statt eine Höhe anzunehmen oder je Kachel einen Pick-Puffer zu laden.
status: gilt
date: 2026-09-28
issues: [29]
code:
  - web/src/pick.ts
  - web/src/main.ts
---

# 0035: Koordinaten aus Höhenkarten

## Anlass

Der Maintainer wollte die Koordinaten des Blocks unter Maus und Finger,
„synchron mit denen von Minecraft“. Ein Bildpunkt zeigt entlang der
Blickachse (1, 1, 1) einen von rund 1150 Würfeln; welchen, verrät das Bild
nicht. Deshalb hielt
[0007](0007-karteneinheit-ist-ein-pixel-der-basis.md) fest, dass es keine
Koordinatenanzeige gibt. Das Format der Höhen steht in #29.

## Entscheidung

Der Renderer schreibt je Region eine Höhenkarte: je Spalte das Y des
obersten Blocks, auf den das Spiel zielt, ohne Flüssigkeiten. Das Frontend
geht den Strahl durch die Mitte des Pixels von vorn nach hinten ab. Der
erste Würfel, dessen Spalte bis zu ihm hinauf gefüllt ist, ist der
Treffer. Siehe [Frontend](../frontend.md), „Koordinaten“, und
[map.json](../benutzung/map-json.md).

## Verworfene Alternativen

- **Eine feste Höhe annehmen, etwa Y 64.** Das braucht keine Daten vom
  Renderer. Ein Block auf Höhe Y erscheint dann aber um Y − 64 Blöcke
  versetzt, in X und in Z; auf einem Hügel bei Y 100 sind das 36 Blöcke.
- **Ein Pick-Puffer je Basiskachel,** je Pixel der Würfel, den das Zeichnen
  dort hinterlässt. Er wäre auch unter Überhängen exakt, kostet aber je
  Basiskachel eine Datei mehr: auf der grossen Welt 2,5 Millionen Dateien
  und geschätzt 5 bis 15 GB, gegen einige hundert MB Höhenkarten. Aus der
  Deckungsmaske fällt er nicht ab. Sie hält je Pixel nur ein Bit, und das
  setzen nur deckende Pixel.
- **Die Höhen als Bild,** PNG oder WebP, das der Browser selbst dekodiert.
  Die Werte kämen dann über `getImageData` aus einem Canvas, und dort
  verrauschen Browser die Pixel gegen Fingerprinting: Brave in der
  Voreinstellung
  ([Brave, „Fingerprinting defenses 2.0“, 2020](https://brave.com/privacy-updates/4-fingerprinting-defenses-2.0/),
  abgerufen am 28.09.2026), Safari ab 17.0 im privaten Modus
  ([Wilander u. a., „Private Browsing 2.0“, WebKit, 2024](https://webkit.org/blog/15697/private-browsing-2-0/),
  abgerufen am 28.09.2026). Eine Höhe, die um eins rauscht, zeigt einen
  falschen Block. Die Höhenkarten sind deshalb rohe Zahlen in zlib,
  entpackt mit `DecompressionStream`.
- **Eine zweite Ebene mit Flüssigkeiten,** also was man sieht statt
  worauf das Spiel zielt. Das Spiel zielt durch Wasser hindurch
  (`Fluid.NONE`, belegt in #29). Die Ebene verdoppelte die Daten für eine
  Anzeige, die vom Spiel abwiche.

## Folgen

- Unter Überhängen liegt der Treffer 1 bis 4 Blöcke zu weit vorn, siehe
  [Frontend](../frontend.md), „Was bleibt eine Näherung“.
- Das Frontend rechnet die Projektion rückwärts, sie steht damit an zwei
  Stellen. Sein Test prüft deshalb seine Formel an
  `renderer/tests/fixtures/projektion.json`, die ein Test des Renderers
  aktuell hält, und damit jeden Bildpunkt eines kleinen Geländes gegen
  das, was das Zeichnen dort hinterlässt.
- Ein Baum ohne Höhen zeigt keine Koordinaten, bis ein Export sie schreibt.
