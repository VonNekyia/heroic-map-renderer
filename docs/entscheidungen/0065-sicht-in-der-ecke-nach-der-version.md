---
title: "0065: Die Sicht in der Ecke nach der Version der Welt"
description: Warum der Renderer eine Welt aus 26.2 mit der Regel von 26.2 für die Sicht in der Ecke zeichnet, aus einer eigenen Tabelle aus dem Spiel von 26.2, gewählt nach der Datenversion in level.dat, und warum die Version zum Baum gehört.
status: gilt
date: 2026-10-03
issues: [111]
code:
  - renderer/src/assets/sicht262.txt
  - renderer/src/assets/Sicht262.java
  - renderer/src/assets/blockstate.rs
  - renderer/src/world/mod.rs
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# 0065: Die Sicht in der Ecke nach der Version der Welt

Löst [0059](0059-welten-aus-26-2-und-26-3.md) in einem Punkt ab: Die
Tabellen kommen weiter aus 26.3, die Sicht in der Ecke nach der Version der
Welt.

## Anlass

Seit 0059 zeichnet der Renderer jede Welt mit der weichen Beleuchtung von
26.3. Die Spieler einer Welt aus 26.2 sehen sie aber im Client von 26.2,
und Welten bleiben oft lange auf einer Version (#111). Die beiden Versionen
prüfen die Ecke einer Fläche verschieden
(`BlockModelLighter.prepareQuadAmbientOcclusion`, per javap an beiden
Client-JARs):

- 26.2: `isViewBlocking` und `getLightDampening` > 0;
- 26.3: nicht `isLightPermeable`, also `solidRender` und
  `getLightDampening` > 0.

Anders sind 23 Blöcke: Eis, Brucheis, Schleimblock, Leuchtfeuer, Spawner,
die geflutete Barriere und die 17 Shulkerkisten. Die Mitte einer Fläche im
Innern fragt in beiden `isSolidRender`.

## Entscheidung

- **Eine eigene Tabelle aus 26.2:** `sicht262.txt`, von `Sicht262.java` am
  Server-JAR von 26.2 erzeugt, gibt Bit 8, `SICHT_262`. Bit 2 in
  `schatten.txt` bleibt die Regel von 26.3. Von Hand wird nichts
  eingetragen.
- **Die Wahl je Lauf:** `Data.DataVersion` aus `level.dat`; vor 5023, der
  Datenversion von 26.3, gilt die Regel von 26.2, sonst und ohne Angabe die
  von 26.3. Es zählt der Client, der die Welt zeigt, nicht der Chunk: Eine
  Welt im Übergang sehen alle schon mit 26.3.
- **Für die Ecke eine eigene Ebene:** `VIEW` nach der Version, `OPAQUE`
  für die Mitte, immer Bit 2.
- **Die Version gehört zum Baum:** `ambientOcclusion` in `map.json`. Ein
  Lauf mit der anderen Version bricht ab, wie bei `--biome-blend`. Ein Baum
  aus einem älteren Stand bekommt die der Welt.

Die Einzelheiten stehen in
[Weiche Beleuchtung](../renderer/weiche-beleuchtung.md), „Welten aus 26.2“.

## Verworfene Alternativen

- **Bit 8 in `schatten.txt`.** `isViewBlocking` hat in 26.3 eine andere
  Signatur und fragt dort die Form; die Regel von 26.2 lässt sich nur am
  Spiel von 26.2 lesen. In eine Datei mit den Bits aus 26.3 käme sie nur
  über einen zweiten Lauf und ein Zusammenführen.
- **Ein zweiter Satz aller Tabellen.** Verwirft schon 0059; die übrigen
  Bits sind für die Blöcke von 26.2 gleich.
- **Die Version je Chunk.** Ein Chunk aus 26.2 in einer Welt, die schon
  26.3 läuft, erscheint ihren Spielern mit 26.3.
- **Die höchste Datenversion der Chunks aus dem Vorlauf.** Das hiesse, die
  Version erst nach dem Vorlauf zu kennen; `level.dat` schreibt das Spiel
  beim Start mit der Version des Servers.
- **Nach einem Wechsel der Version ohne Abbruch weiterrendern.** Dann
  mischte ein Baum Kacheln zweier Regeln, auch mit `--resume`.
- **Ohne eigene Ebene, die Mitte aus der Ecke.** In einer Welt aus 26.2
  nähme die Mitte vor Eis das Licht der eigenen Zelle; das Spiel nimmt das
  der Zelle davor.

## Folgen

- Eine Welt aus 26.2 erscheint wie im Client von 26.2. Die Testwelt ist
  26.2: `eis.webp`, `map-wide.png` und die Kennzahlen des Looks ändern sich
  an Flächen neben den 23 Blöcken.
- Wechselt eine Welt auf 26.3, braucht ihr Baum eine neue Wurzel.
- Je Section eine Ebene mehr im Cache, 512 Byte.
- Bleibt 26.2 mit der nächsten Version unterstützt, bleibt
  `sicht262.txt`; sie wird nicht neu erzeugt.
