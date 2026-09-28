---
title: Packs und Wurzeln
description: Wie der Renderer eine Asset- oder Datenwurzel auflistet wie PathPackResources in 26.2, mit Links, Junctions, Schreibweisen und Lesefehlern.
code:
  - renderer/src/assets/pack.rs
  - renderer/src/assets/texture.rs
  - renderer/src/assets/mod.rs
---

# Packs und Wurzeln

Jede Wurzel aus `--assets` und `--data` listet der Renderer einmal auf, so
wie der Client ein Pack (`PathPackResources`), und schlägt danach nur noch
in dieser Liste nach (`Pack` in
[`renderer/src/assets/pack.rs`](../../renderer/src/assets/pack.rs)). Welche
Dateien darin zählen, entscheiden die Regeln des Clients, auch unter
Windows, wo die Platte keine Schreibweise unterscheidet.

## Was aufgelistet wird

Der Wurzel folgt der Renderer, auch über einen Link, und ebenso jedem
Namensraum darin. Die Anfänge der Listen nennt der Client selbst:
`blockstates`, `models` und die Ordner der Atlanten, die der Renderer
braucht. In 26.2 sind das `textures/block` und `textures/entity/conduit`
aus `atlases/blocks.json`, dazu für die Blockentities `entity/chest`,
`entity/banner`, `entity/shulker` und `entity/decorated_pot` aus
`chests.json`, `banner_patterns.json`, `shulker_boxes.json` und
`decorated_pot.json`; jeder Atlas listet sie mit demselben
`DirectoryLister`. Unter Windows gelten die Anfänge also in jeder
Schreibweise. Darunter zählt eine Datei nur, wenn ihr Name
auf der Platte ein `Identifier` ist: unter Windows fände `block/stone` sonst
auch `Stone.json`, das der Client übergeht, und eine `.mcmeta` gehört nur
in genau dieser Schreibweise zur PNG.

## Links

Einen Link unter den Anfängen übergeht der Client, wie `Files.find` ohne
`FOLLOW_LINKS`. Für einen Link hält Java unter Windows nur einen
Analysepunkt mit dem Tag `IO_REPARSE_TAG_SYMLINK` (`WindowsFileAttributes`),
eine Datei mit anderem Tag für keine Datei (`isOther`). Eine Junction ist
für Java 25, auf dem 26.2 läuft, deshalb ein Ordner, und dem folgt er; ab
Java 26 nicht mehr. Ganz aus lässt er ein Pack mit einem Link nur im Ordner
`resourcepacks` (`DirectoryValidator`), die Wurzeln hier nennt der Nutzer.
Kein Pfad führt aus seinem Pack heraus.

## Direkt geöffnete Dateien

Eine Colormap öffnet der Client direkt, ohne Liste, und folgt dabei jedem
Link; ebenso die beiden einzelnen Texturen des Block-Atlas,
`entity/bell/bell_body` und `entity/enchantment/enchanting_table_book`
(`SingleFile`, `getResource`), und die Texturen von Blockentities, die in
keinem Atlas stehen, etwa die der Köpfe. So öffnet der Renderer jede
Textur ausserhalb der Ordner oben. Nennt ein Blockmodell eine solche, zeigte
der Client die Missing-Textur, es sei denn, ein Pack erweitert
`atlases/blocks.json`; diese Dateien liest der Renderer nicht. Andere
Ordner unter `textures` listet er wie der Client nicht auf.

## Lesefehler

Lässt sich der Anfang einer Liste nicht lesen, listet der Client dort
nichts. Fehlt er, fehlt sein Ziel oder ist es kein Ordner, etwa bei einer
Junction ohne Ziel, geschieht das still (`NoSuchFileException`, und
`NotDirectoryException` beim Öffnen des Ordners). Jeden anderen Fehler, etwa bei
einer Junction auf sich selbst oder unter Linux, wenn im Pfad davor eine
Datei steht, schreibt er ins Log, und der Renderer nennt ihn in der
Ausgabe. Ein Fehler tiefer im Baum lässt im Client das Laden der Packs
scheitern und bricht hier den Lauf ab.

## Was bleibt eine Näherung

- **Die Atlanten liest der Renderer nicht,** ihre Ordner stehen für 26.2
  im Code (`ASSETS` in
  [`renderer/src/assets/pack.rs`](../../renderer/src/assets/pack.rs)).
  Eine Textur eines Blockmodells ausserhalb der Ordner des Block-Atlas zeigt
  er, wo der Client ohne erweiterten Atlas die Missing-Textur zeigte.
