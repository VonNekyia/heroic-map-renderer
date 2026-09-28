---
title: "0035: Koordinaten aus Höhenkarten"
description: Warum das Frontend die Koordinaten unter Maus und Finger aus einer Höhenkarte je 4 × 4 Spalten rechnet, genommen aus der Heightmap, die das Spiel in jedem Chunk speichert, statt eine Höhe anzunehmen oder je Spalte oder je Pixel zu speichern.
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
„synchron mit denen von Minecraft“, und dann „möglichst viel Effizienz bei
menschenfreundlicher Koordinatenauflösung, die nicht unbedingt genau sein
muss“. Ein Bildpunkt zeigt entlang der Blickachse (1, 1, 1) einen von rund
1150 Würfeln; welchen, verrät das Bild nicht. Deshalb hielt
[0007](0007-karteneinheit-ist-ein-pixel-der-basis.md) fest, dass es keine
Koordinatenanzeige gibt. Das Format der Höhen steht in #29.

## Entscheidung

Der Renderer schreibt je Region eine Höhenkarte: je Zelle aus 4 × 4
Spalten den Median der Heightmap `WORLD_SURFACE`, die das Spiel in jedem
Chunk speichert, also des obersten Blocks, der nicht Luft ist, Wasser
eingeschlossen. Das Frontend geht den Strahl durch die Mitte des Pixels von
vorn nach hinten ab. Der erste Würfel, dessen Zelle bis zu ihm hinauf
gefüllt ist, ist der Treffer. Über Wasser nennt die Anzeige so die
Oberfläche, die man sieht. Siehe [Frontend](../frontend.md),
„Koordinaten“, und [map.json](../benutzung/map-json.md), „Höhen“.

## Verworfene Alternativen

Die Zahlen stehen in [2026-09-28, Höhen](../messungen/2026-09-28-hoehen.md).
Gemessen ist gegen das, was man sieht: den Strahl über `WORLD_SURFACE` je
Spalte.

- **Eine feste Höhe annehmen, etwa Y 64.** Das braucht keine Daten. Ein
  Block auf Höhe Y erscheint dann aber um Y − 64 Blöcke versetzt, in X und
  in Z; auf einem Hügel bei Y 100 sind das 36 Blöcke. Ein Punkt auf einem
  Berg nennt die Stelle auf Y 64 hinter ihm, die das Bild verdeckt; das
  stimmt nur, wo das Gelände dort auf Y 64 liegt. An Land lagen auf der
  grossen Welt nur 1 bis 11 % der Pixel höchstens 4 Blöcke daneben.
- **Je Spalte, in einem eigenen Durchgang durch die Welt,** wie die erste
  Fassung von #35: 249 MB auf der grossen Welt und knapp eine Minute je
  Export. Gegen das, was man sieht, ist sie an Land nicht genauer, 92,2
  gegen 92,6 % der Pixel auf 2 Blöcke. Über Wasser nennt sie den Grund.
- **Je 2 × 2 Spalten:** an Land genauer, 95 bis 97 % auf 2 Blöcke, aber
  43,6 statt 13,7 MB.
- **Je Chunk, 16 × 16 Spalten:** 1,3 MB, aber an Land nur 56 bis 66 % auf
  2 Blöcke.
- **`OCEAN_FLOOR`,** der Grund unter Wasser, wie das Spiel zielt. Es zielt
  durch Flüssigkeiten hindurch (`Fluid.NONE`, belegt in #29). Der
  Maintainer wollte die Oberfläche, die man sieht.
- **Eine zweite Lage je Spalte,** die erste Lücke unter dem obersten
  Block, etwa Luft unter Laub oder Wasser unter Eis. Mit ihr träfe der
  Strahl den gezeigten Würfel öfter, 97,7 statt 90,2 % der Pixel auf der
  Testwelt, siehe dieselbe Messung, „Überhänge“. Sie kostet aber mehr Daten
  und den eigenen Durchgang.
- **Ein Pick-Puffer je Basiskachel,** je Pixel der Würfel, den das Zeichnen
  dort hinterlässt. Er wäre exakt, kostet aber je Basiskachel eine Datei
  mehr: auf der grossen Welt 2,5 Millionen Dateien und geschätzt 5 bis
  15 GB. Aus der Deckungsmaske fällt er nicht ab. Sie hält je Pixel nur ein
  Bit, und das setzen nur deckende Pixel.
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

## Folgen

- Die Anzeige liegt meist wenige Blöcke daneben: an Land auf der grossen
  Welt bei 88 bis 93 % der Pixel höchstens 2 Blöcke, über Wasser bei 99,7
  bis 99,9 %. Siehe [Frontend](../frontend.md), „Was bleibt eine Näherung“.
- Über Wasser weicht die Anzeige vom Spiel ab: Es zielt auf den Grund.
- Die Höhen kosten keinen eigenen Durchgang; der Vorlauf liest die
  Heightmap mit. Auf der grossen Welt sind es 13,7 MB.
- Das Frontend rechnet die Projektion rückwärts, sie steht damit an zwei
  Stellen. Sein Test prüft deshalb seine Formel an
  `renderer/tests/fixtures/projektion.json`, die ein Test des Renderers
  aktuell hält, und damit jeden Bildpunkt eines kleinen Geländes gegen
  das, was das Zeichnen dort hinterlässt.
- Ein Baum ohne Höhen zeigt keine Koordinaten, bis ein Export oder
  `--heights` sie schreibt.
