---
title: "0115: Datapacks aus der Welt"
description: Warum der Renderer die Datapacks einer Welt selbst wie das Spiel aus level.dat und datapacks/ liest und je Pack nur Biome, Bannermuster und Dimensionen als Datenwurzel in den Cache legt, statt eines Schalters oder des ganzen Packs, und was er dabei nähert.
status: gilt
date: 2026-10-20
issues: [288]
code:
  - renderer/src/cli/datenpakete.rs
  - renderer/src/world/mod.rs
  - renderer/src/cli.rs
---

# 0115: Datapacks aus der Welt

## Anlass

Eine Welt mit einem Datapack, das eigene Biome bringt, sah auf der Karte aus
wie `plains`: Der Renderer kannte ihre Biome nicht (#288). Von Hand ging es
nur mit `--data <pack>/data`, für ein ausgepacktes Pack ohne Overlays.
Solche Packs kommen meist als Zip, mit Overlays für mehrere Versionen des
Spiels.

## Entscheidung

- **Kein Schalter.** Der Renderer liest die Packs aus der Weltwurzel, die
  `--world` ohnehin nennt, und wählt sie wie das Spiel: `Data.DataPacks` in
  `level.dat`, die Packs unter `datapacks/` als Ordner oder Zip, die
  Overlays nach dem Datenformat der Welt.
- **Nur was er liest:** Biome, Bannermuster und Dimensionen eines Packs legt
  er als eine Datenwurzel in den Cache, die geltenden Overlays schon
  darüber. Die Wurzeln stehen in der Reihe des Spiels hinter dem Jar und
  `--data`.
- **Der Stand bleibt:** Nur diese Wurzeln kommen in den Fingerabdruck der
  Daten. Eine Welt ohne Packs, oder nur mit Packs ohne diese Listen, hat
  dieselben Wurzeln wie vorher; `--update` geht weiter. Ein Test hält den
  Fingerabdruck als Konstante fest.
- **Ein Pack, das etwas trägt,** verlangt einmal einen vollen Lauf, wie
  andere Daten. Die Meldung behält den Teiltext „stammt von anderen Assets
  oder Daten“; auf ihn prüft das Plugin.
- **Kaputte Packs** übergeht der Renderer mit Grund, statt abzubrechen.
- **Der Zeichenstand bleibt:** Der Renderer zeichnet nicht anders, er
  bekommt nur mehr Daten. Es kommt ein Goldbild dazu, `metatile-datapack`.

Einzelheiten und Belege: [Assets und Biomdaten](../benutzung/assets.md),
„Datapacks der Welt“.

## Verworfene Alternativen

- **Ein Schalter für die Packs:** Jeder Betreiber müsste sie selbst nennen
  und die Reihe des Spiels nachbauen. Die Welt kennt beides schon, und das
  Plugin gibt immer die Weltwurzel.
- **Das ganze Pack als Wurzel:** Ein Ordner trägt oft Funktionen und
  Strukturen, die sich ändern, ohne dass ein Biom sich ändert. Jede solche
  Änderung hätte einen vollen Lauf verlangt. Ein Zip liesse sich so gar
  nicht lesen, und Overlays gälten nicht.
- **Jedes Pack eine Wurzel, auch ohne Listen:** Dann änderte schon
  `file/bukkit` auf Paper den Stand jeder Welt, und jede bräuchte einen
  vollen Lauf ohne eine neue Farbe.
- **Zips nur im Speicher lesen:** Der Fingerabdruck zählt Dateien mit ihrer
  Zeit. Ohne Ordner im Cache hätte er einen zweiten Weg gebraucht.

## Grenzen

Ausführlich in [Assets und Biomdaten](../benutzung/assets.md), „Grenzen“:

- **Vanilla zuunterst:** Die Daten des Spiels stehen immer vor den Packs,
  auch wenn `vanilla` in `Enabled` hinter einem Pack stünde.
- **`formats` allein** gilt in 26.2 und 26.3 nie, wie im Spiel. Die
  strengeren Prüfungen des Spiels für solche Einträge baut der Renderer
  nicht nach, und der Abschnitt `filter` bleibt ungelesen.
