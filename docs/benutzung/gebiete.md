---
title: Gebiete mit eigener Stufe
description: Wie --quality-areas Gebiete besser packen lässt als den Baum, etwa Städte; die Datei, welche Kacheln ein Gebiet hebt, was der Baum sich merkt, Update, Abbruch, --pyramid und was es kostet.
code:
  - renderer/src/cli/gebiete.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# Gebiete mit eigener Stufe

Mit `--quality-areas <datei>` packen Gebiete besser als der Baum, etwa
Städte in `maximal_compact` über einem Baum in `small`. Je Kachel gilt die
höchste Stufe aus der des Baums und denen der Gebiete, die sie zeigt. Die
Pixel bleiben, wie der Renderer sie zeichnet; nur die Packung ändert sich,
siehe [map.json](map-json.md), „Die Stufe“. Entschieden in
[0145](../entscheidungen/0145-gebiete-mit-eigener-stufe.md).

## Die Datei

So schreibt sie das Plugin, siehe [Plugin](../plugin.md), „Was das Plugin
vom Renderer nutzt“:

```json
{"v":1,"areas":[{"id":"meinplugin:regionen/42","quality":"maximal_compact",
  "polygons":[{"outer":[[0,0],[200,0],[200,120],[0,120]],
               "holes":[[[50,50],[80,50],[80,80]]]}]}]}
```

| Feld | Inhalt |
|---|---|
| `v` | 1 |
| `areas[].quality` | eine der fünf Stufen von `--quality` |
| `areas[].polygons[].outer` | die Ecken eines Vielecks in Blöcken, x und z, ohne y |
| `areas[].polygons[].holes` | Löcher darin, ebenso; darf fehlen |
| `areas[].id` | nur für Meldungen; verglichen wird die Geometrie |

- **Gerade/ungerade** über Rand und Löcher zusammen. Ein Vieleck, das sich
  selbst schneidet, ist kein Fehler. Weniger als drei Ecken zeigen nichts.
- **Eine Regel für jeden Fehler:** Fehlt die Datei, ist sie kaputt, grösser
  als 64 MiB, hat sie eine andere Fassung als 1 oder nennt sie eine Stufe,
  die es nicht gibt, bricht der Lauf ab, bevor er etwas schreibt:

  ```
  Error: Gebiete:    <datei> ist keine Datei mit Gebieten
  ```

- **Eine leere Liste** heisst keine Gebiete. Nach dem Lauf vergisst der
  Baum die gemerkten.
- `--quality-areas` gibt es nur mit `--tiles`, nicht mit `--pyramid`,
  `--compact-tree`, `--repack` oder `--serve`.
- **`--estimate`** schätzt mit der Stufe des Baums, ohne Gebiete; mit
  `--quality-areas` schätzt es zu wenig Platz, siehe
  [Kosten](kosten.md), „Schätzen: `--estimate`“.

## Welche Kacheln ein Gebiet hebt

1. **Spalten:** Eine Blockspalte gehört zum Gebiet, wenn ihre Mitte, x + ½
   und z + ½, im Vieleck liegt. Gerechnet wird je Blockzeile mit den
   Schnittpunkten der Kanten (`chunks_im` in
   [`renderer/src/cli/gebiete.rs`](../../renderer/src/cli/gebiete.rs)).
   Eine Mitte genau auf einem linken Schnittpunkt gehört dazu, eine auf
   einem rechten nicht.
   - **Nur in der Hülle:** Es zählt nur Zeilen und Spalten im Rechteck um
     die Regionen der Welt, geschnitten mit dem Rechteck des Baums aus
     `--area` (`huelle`); `--pyramid` nimmt die Regionen des Stands. Ein
     Vieleck bis an die Weltgrenze kostet so nur die Fläche der Welt
     (`riesig_wie_knapp_um_die_welt`).
2. **Kasten je Chunk:** Jeder Chunk mit mindestens einer solchen Spalte
   gibt einen Kasten über seine 16 × 16 Spalten. In der Höhe reicht er bis
   über seinen höchsten Block und hinunter bis zur Section des tiefsten
   ersten dichten Blocks in ihm und seinen acht Nachbarn, beides aus dem
   Stand, siehe [Updates](updates.md), „Der Stand“ und „Die Grenze nach
   unten“ (`hoehe`). Steht ein Chunk dort als unbekannt oder fehlt der
   Stand, reicht der Kasten über die ganze Höhe der Dimension.
3. **Kacheln:** Der Kasten geht durch die Projektion, samt der Reserve für
   überstehende Sprites, aber ohne die Ränder für Licht, Schatten und Bloom
   eines Updates (`kacheln_des_chunks` in
   [`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)).
   Jede Basiskachel, die er berührt, zeigt das Gebiet.

- **Zu viel gehoben** wird höchstens ein Chunk Rand um ein Gebiet. Nach der
  Fläche sind das bei einer Stadt von 200 × 200 Blöcken höchstens rund 17 %
  mehr gehobene Kacheln, bei 1000 × 1000 höchstens rund 3 %. Das kostet nur
  Bytes, kein Bild ist falsch.
- **Die Stufe einer Basiskachel** ist die höchste aus der des Baums und
  denen der Gebiete, die sie zeigt. Ein Gebiet unter der Stufe des Baums
  ändert nichts.
- **Eine Elternkachel** und eine Kachel einer nativen Stufe haben die höchste
  Stufe der Basiskacheln unter sich. Die groben Stufen bis Zoom 0 packen so
  mit der höchsten Stufe aller Gebiete darunter.
- **Schicht und Rest des Reliefs** bleiben, wie sie ohne Gebiete wären; sie
  sind in jeder Stufe verlustfrei.

Geprüft in `gebiet_packt_mit_seiner_stufe` in `renderer/tests/cli.rs`: In
einem Baum `minimal` mit einer Stadt in `maximal_compact`, einem Loch über
einem fernen Chunk und zwei Vororten in `good` und `small` gleicht jede
Kachel der Basis und der nativen Stufen der eines Baums ganz in
`maximal_compact` oder ganz in `minimal`. Im Loch packt sie wie der Baum,
im iso und von oben. Schicht und Rest prüft `schicht_bleibt_in_gebieten_wie_ohne`.

## Was der Baum sich merkt

| Datei neben `stand.bin` | wann | Inhalt |
|---|---|---|
| `gebiete-neu.json` | vor der ersten Kachel eines vollen Laufs oder Updates mit Gebieten, neben `stand-neu.bin` | die Gebiete dieses Laufs |
| `gebiete.json` | am Ende, mit dem Stand; ohne Gebiete fällt sie weg | die Gebiete des letzten fertigen Laufs |

- **Inhalt:** `{"v":1,"areas":[…]}` je Gebiet nur mit `quality` und
  `polygons`, sortiert, ohne `id`. Der Ordner eines Baums liegt oft hinter
  einem fremden Webserver, und die `id` nennt Plugin, Ebene und Region.
- **Nicht** in `map.json`, nicht im Manifest, nicht in `--serve`: Der
  Server liefert nur Pfade aus seiner Liste aus, siehe
  [Server](server.md).
- **Ohne den Schalter** nimmt ein Lauf die Gebiete von `gebiete-neu.json`,
  sonst die von `gebiete.json`. Gibt es keine, rechnet er nichts dafür,
  liest nichts dafür und schreibt keine der beiden Dateien
  (`ohne_gebiete_nichts_dafuer`).
- **Ein Ausschnitt** packt seine Kacheln nach den Gebieten, mit den Höhen
  aus `stand.bin`, und schreibt keine der beiden Dateien. Seine Kacheln
  liegen offen; das nächste Update packt sie wie ein voller Lauf.

## Update

- **Ändern sich die Gebiete** gegen `gebiete.json`, zeichnet ein Update
  dazu jede Basiskachel, deren Stufe sich ändert. Ändert sich nur die
  Datei, zeichnet es nur diese:

  ```
  Update:     0 Chunks geändert, dazu 12 Basiskacheln mit neuer Stufe aus den Gebieten, 6 Kacheln von 256x256 px bei scale 16 im Gebiet, in 0.0 s
  ```

- **Die Stufen** rechnet es nach dem neuen Stand, wie ein voller Lauf, und
  über alle Gebiete: Eine Elternkachel bis Zoom 0 hängt auch an Gebieten
  fern der Änderung. Gleiche Pixel mit neuer Stufe treffen den Hash nicht,
  denn er trägt die Packung im Präfix, siehe [Updates](updates.md), „Gleiche
  Pixel“.
- **Geprüft** in `update_mit_gebieten_gleicht_einem_vollen_lauf`: Gebiet
  verschoben, weg, neu, Chunks darin geändert und ohne den Schalter; jedes
  Mal gleicht der Baum Byte für Byte einem vollen Lauf mit denselben
  Gebieten, im iso und von oben.

## Abbruch und `--resume`

- **Fortsetzen:** Nennt `--quality-areas` andere Gebiete als
  `gebiete-neu.json`, setzt der Lauf nicht fort und zeichnet wie ohne
  `--resume`, wie bei einer anderen Stufe:

  ```
  Gebiete:    --quality-areas nennt andere Gebiete als der abgebrochene Lauf; dieser setzt ihn nicht fort und zeichnet wie ohne --resume
  ```

  Ohne den Schalter gelten die Gebiete des abgebrochenen Laufs.
- **Ein abgebrochenes Update** hat die Kacheln mit neuer Stufe in seinem
  Gebiet und damit im offenen Gebiet, siehe [Updates](updates.md), „Das
  offene Gebiet“. Das nächste Update ohne den Schalter nimmt die Gebiete
  des abgebrochenen und vergleicht gegen `gebiete.json`.
- **Geprüft** in `gebiete_nach_abbruch`.

## `--pyramid`

`--pyramid` kennt die Stufen aus `gebiete.json` und dem Stand und baut damit
dieselben Bytes wie der Lauf (`feine_stufen_im_speicher_mit_gebiet`).

- **Ein Chunk, den der Server während des Laufs schrieb,** steht bis zum
  nächsten Update als unbekannt im Stand. Um ihn hebt `--pyramid` über die
  ganze Höhe, mehr als der Lauf. Das nächste Update baut die Eltern über
  seinen Kacheln wieder wie ein voller Lauf.
- **Ohne `stand.bin`** kennt es die Chunks der Welt nicht und lehnt den
  Baum ab, ebenso `--repack`:

  ```
  Error: Gebiete:    <baum>/gebiete.json liegt da, <baum>/stand.bin nicht: --pyramid und --repack kennen ohne Stand die Chunks der Welt nicht; ein voller Lauf baut den Baum neu
  ```

  Ein Baum mit `gebiete.json` hat immer einen Stand, beide schreibt das
  Ende desselben Laufs; fehlen kann er nur, wenn jemand ihn löscht. Ohne
  Stand braucht auch ein Update einen vollen Lauf, siehe
  [Updates](updates.md), „Abbruch und `--resume`“.

## `--compact-tree`

`--compact-tree` und `--repack` packen je Kachel mit der höchsten Stufe aus
ihrem Ziel und den gemerkten Gebieten, aus `gebiete.json` und dem Stand wie
`--pyramid`; eine Kachel eines Gebiets in `maximal` bleibt so schnell
gepackt. Liegt `gebiete-neu.json`, lehnen beide den Baum ab: Ein
abgebrochener Lauf kann andere Gebiete haben. Siehe
[Umpacken](umpacken.md) und
[0146](../entscheidungen/0146-umpacken.md); bis dahin lehnte
`--compact-tree` jeden Baum mit Gebieten ab.

## Was es kostet

- **Ohne Gebiete:** nichts.
- **Mit Gebieten** rechnet jeder Lauf die Tabelle einmal, ein Update mit
  neuer Datei zweimal, jedes Update über alle Gebiete. Die Zeit wächst mit
  den Chunks in Gebieten, je Gebiet gezählt, der Speicher mit den Kacheln,
  die Gebiete heben.
- **Gemessen** in `tabelle_fuer_500_vielecke`: 500 Städte mit je 50 Ecken
  und 200 Blöcken Durchmesser und ein Land bis an die Weltgrenze, über
  einer Welt von 21 × 17 Regionen, bei scale 16. Das sind 439 568 Chunks in
  Gebieten, 365 568 davon verschieden, und 125 011 Einträge in der Tabelle
  über alle Zoomstufen. Zwei Tabellen samt Vergleich und der Zählung des
  Tests dauern in der CI unter Ubuntu im Release-Build 1,33 s, also
  höchstens rund 1,5 ms je 1000 Chunks und Tabelle; im Testprofil 2,90 s,
  unter Windows 2,96 s (Lauf 44199409597, Stand 18.11.2026). Für 500
  Städte allein, 74 000 Chunks, sind das rund 0,11 s je Tabelle; ein
  Update von oben mit einem Thread braucht an der Testwelt rund 5,6 s, siehe
  [Stufen am Baum](../messungen/2026-11-17-stufen-am-baum.md).
- **Im Speicher** hält die Tabelle je Zoomstufe nur die gehobenen Kacheln,
  je Eintrag 12 B in einer `HashMap`, mit ihrer Verwaltung und Füllung 15
  bis 30 B. Bei scale 16 sind das je 1000 Chunks in Gebieten rund 340
  Einträge, 5 bis 10 KiB. Dazu kommen, solange ein Gebiet seine Kacheln
  sucht, seine Chunks.
- **Liegt der grösste Teil der Karte in Gebieten,** gehört die Stufe an den
  Baum, mit `--quality`: Dann rechnet kein Lauf die Tabelle.
