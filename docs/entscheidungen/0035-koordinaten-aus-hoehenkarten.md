---
title: "0035: Koordinaten aus Höhenkarten"
description: Warum das Frontend die Koordinaten unter Maus und Finger über den Strahl durch das Pixel gegen die Höhen des Renderers rechnet, statt über einen Puffer je Pixel, eine zweite Lage oder Höhen als Bild, und was die Anzeige zeigt.
status: gilt
date: 2026-09-28
issues: [29]
code:
  - web/src/pick.ts
  - web/src/main.ts
---

# 0035: Koordinaten aus Höhenkarten

Den Umriss zeigt seit [0049](0049-umriss-nur-ohne-zeiger.md) nur noch ein Tippen, nicht mehr die Maus.

## Anlass

Der Maintainer wollte die Koordinaten des Blocks unter Maus und Finger,
„synchron mit denen von Minecraft“, und dann „möglichst viel Effizienz bei
menschenfreundlicher Koordinatenauflösung, die nicht unbedingt genau sein
muss“. Ein Bildpunkt zeigt entlang der Blickachse (1, 1, 1) einen von rund
1150 Würfeln; welchen, verrät das Bild nicht. Deshalb hielt
[0007](0007-karteneinheit-ist-ein-pixel-der-basis.md) fest, dass es keine
Koordinatenanzeige gibt.

## Entscheidung

Das Frontend geht den Strahl durch die Mitte des Pixels von vorn nach
hinten ab. Der erste Würfel, dessen Zelle bis zu ihm hinauf gefüllt ist,
ist der Treffer. Die Anzeige nennt X, Y und Z dieses Würfels und zeichnet
seinen Umriss wie den Auswahlrahmen im Spiel, beim Darüberfahren mit der
Maus und beim Tippen. Sie stimmt auf wenige Blöcke. Siehe
[Frontend](../frontend.md), „Koordinaten“.

Woher die Höhen kommen, wie grob sie sind und warum über Wasser die
Oberfläche zählt, entscheidet
[0036](0036-hoehen-aus-der-heightmap.md).

## Verworfene Alternativen

- **Ein Pick-Puffer je Basiskachel,** je Pixel der Würfel, den das Zeichnen
  dort hinterlässt. Er wäre exakt, kostet aber je Basiskachel eine Datei
  mehr: auf der grossen Welt 2,5 Millionen Dateien und geschätzt 5 bis
  15 GB. Aus der Deckungsmaske fällt er nicht ab. Sie hält je Pixel nur ein
  Bit, und das setzen nur deckende Pixel.
- **Eine zweite Lage je Spalte,** die erste Lücke unter dem obersten
  Block, etwa Luft unter Laub oder Wasser unter Eis. Mit ihr träfe der
  Strahl den gezeigten Würfel öfter, siehe
  [2026-09-28, Höhen](../messungen/2026-09-28-hoehen.md), „Überhänge“. Sie
  kostet aber mehr Daten und mehr Arbeit im Vorlauf, der die Lücke aus den
  Blöcken suchen müsste, die er ohnehin dekodiert; einen eigenen Durchgang
  bräuchte sie nicht. Der Maintainer wollte den effizienten Weg.
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

- Die Anzeige liegt oft ein paar Blöcke daneben, siehe
  [Frontend](../frontend.md), „Was bleibt eine Näherung“.
- Das Frontend rechnet die Projektion rückwärts, sie steht damit an zwei
  Stellen. Sein Test prüft deshalb seine Formel an
  `renderer/tests/fixtures/projektion.json`, die ein Test des Renderers
  aktuell hält, und damit jeden Bildpunkt eines kleinen Geländes gegen
  das, was das Zeichnen dort hinterlässt.
- Ein Baum ohne Höhen zeigt keine Koordinaten, bis ein Export oder
  `--heights` sie schreibt.
