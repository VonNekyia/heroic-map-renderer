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
`blockstates`, `models` und die Ordner des Block-Atlas, in 26.2
`textures/block` und `textures/entity/conduit`; unter Windows gelten sie
also in jeder Schreibweise. Darunter zählt eine Datei nur, wenn ihr Name
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
(`SingleFile`, `getResource`). So öffnet der Renderer auch jede andere
Textur ausserhalb der Ordner des Atlas. Der Client zeigte für sie die
Missing-Textur, es sei denn, ein Pack erweitert `atlases/blocks.json`; diese
Dateien liest der Renderer nicht. Andere Ordner unter `textures` listet er
wie der Client nicht auf.

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

- **`atlases/blocks.json` liest der Renderer nicht.** Eine Textur
  ausserhalb der Ordner des Atlas zeigt er, wo der Client ohne erweiterten
  Atlas die Missing-Textur zeigte.
