---
title: "0041: Dimensionstypen aus einer Tabelle des Spiels, Datenwurzeln darüber"
description: Warum der Renderer die Dimensionstypen des Spiels aus einer erzeugten Tabelle kennt, Datenwurzeln darüberlegt und sonst den der Oberwelt nimmt.
status: gilt
date: 2026-09-29
issues: [45, 34]
code:
  - renderer/src/assets/dimension.rs
  - renderer/src/assets/dimensionstypen.txt
  - renderer/src/assets/Dimensionstypen.java
---

# 0041: Dimensionstypen aus einer Tabelle des Spiels, Datenwurzeln darüber

## Anlass

#45: Der Renderer schattierte jede Dimension wie die Oberwelt. Im Nether
nimmt das Spiel andere Faktoren, `cardinal_light` in seinem Dimensionstyp.
#34 braucht aus demselben Typ `has_skylight` und die Farben der Lightmap.
Die Typen des Spiels stehen im Client-JAR unter
`data/minecraft/dimension_type`, die Vorgaben der Attribute nur im Code.

## Entscheidung

- Die Typen des Spiels schreibt `Dimensionstypen.java` aus dem Spiel selbst
  in die Tabelle `dimensionstypen.txt`, samt den Vorgaben der vier
  Attribute aus `EnvironmentAttributes`. Sie ist ins Binär einkompiliert.
- Datenwurzeln unter `--data` bringen `dimension_type/` und `dimension/` dazu
  und überschreiben die Typen des Spiels, wie gestapelte Datenpakete.
- Jede Dimension findet ihren Typ zuerst in ihrer Definition aus einer
  Datenwurzel, wie im Spiel; die drei des Spiels sonst den gleichen
  Namens; jede andere den der Oberwelt, mit einer Meldung.
- Einen Leser für alles, was Schattierung und Lightmap brauchen, gibt es
  genau einmal: `DimensionType` in `dimension.rs`. Die Vorgaben stehen nur
  in der Tabelle.

Wie der Typ gefunden und gelesen wird: [Dimensionstypen](../renderer/dimensionstypen.md).

## Verworfene Alternativen

- **Die JSON-Dateien aus `vanilla-data` lesen.** Die Anleitung in
  [Assets und Biomdaten](../benutzung/assets.md) holt nur die Biome aus dem
  JAR, und ohne `--data` bliebe der Nether falsch schattiert. Die Vorgaben
  der Attribute stehen ohnehin nur im Code.
- **Feste Werte je Dimension im Code,** wie die Konstanten `SHADE_*` bisher.
  Eine Dimension aus einem Datenpaket bekäme nie ihren Typ, und die Werte
  stünden neben denen der Lightmap ein zweites Mal.
- **`DimensionTypes.bootstrap` mit einem mitschreibenden
  `BootstrapContext`,** wie `Blockentities.java` die Bannermuster holt. Der
  Bootstrap fragt `lookup` nach Blöcken, Zeitleisten und Uhren und bräuchte
  dafür eigene Attrappen. `VanillaRegistries.createLookup` ruft ihn schon
  mit allem, was er braucht.
- **Modifikatoren der Attribute rechnen.** Vanilla setzt keinen, und für
  die Lightmap braucht es nur Werte. Ein Typ mit Modifikator bekommt die
  Vorgabe, und die Ausgabe nennt ihn.

## Folgen

- Eine fünfte Tabelle, die mit jeder Spielversion neu erzeugt wird, siehe
  [Erzeugte Tabellen](../entwicklung/tabellen.md).
- Datenwurzeln tragen jetzt auch Dimensionen und ihre Typen. Eine Wurzel
  bricht den Lauf nur noch ab, wenn sie nichts trägt, was der Renderer
  liest, keine Biome, Bannermuster, Dimensionen oder Typen; eine nur mit
  einer Dimension genügt, siehe [Assets und Biomdaten](../benutzung/assets.md).
- Die Dimensionen, die die Welt selbst speichert, liest der Renderer
  nicht, siehe [Dimensionstypen](../renderer/dimensionstypen.md), „Was
  bleibt eine Näherung“.
- Ohne Weltwurzel, also mit nur einem Ordner `region`, gilt immer die
  Oberwelt.
