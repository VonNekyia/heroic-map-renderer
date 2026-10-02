---
title: "0051: Kameras und Richtungen"
description: Warum der Renderer jede Raute W:H von 2:1 bis 1:1 und die Draufsicht top zeichnet, mit gleich hohen Wänden, der Regel „ganze Pixel“ statt Vielfachen von 4 und einer Kamera je Baum, und warum die Richtung schon jetzt in map.json steht.
status: gilt
date: 2026-10-01
issues: [66]
code:
  - renderer/src/render/projection.rs
  - renderer/src/cli.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/pyramid.rs
---

# 0051: Kameras und Richtungen

Stufe 2, die genordeten Kameras `top-north` und `north-45`, hält
[0052](0052-genordete-kameras.md) fest. Die Ablage je Kamera und Richtung
unter einer Wurzel mit der Liste der Bäume hält
[0054](0054-baeume-unter-einer-wurzel.md) fest.

## Anlass

Bis #66 zeichnete der Renderer nur 2:1: eine feste Achse (1, 1, 1),
Schritte von scale/4 und scale/2, `--scale` nur in Vielfachen von 4. Vom
User am 01.10. entschieden: jedes W:H, das auf ganzen Pixeln liegt, dazu
die Draufsichten `top` (diagonal) und `top-north` (genordet) und die
genordete Schrägsicht `north-45`, jede Kamera aus einer von vier
Richtungen, eine Richtung je Lauf. 2:1 bleibt Vorgabe. Das kommt in drei
Stufen:

| Stufe | Inhalt |
|---|---|
| 1, #66 | allgemeine Projektion, `--camera W:H` und `top` |
| 2, #67 | der genordete Azimut: `top-north` und `north-45` |
| 3, #68 | `--direction` für alle Kameras |

Diese Entscheidung hält Stufe 1 fest.

## Entscheidung

- **Projektion:** `screen_x = u · h`, `screen_y = v · a − y · b` mit
  u = x − z, v = x + z, h = scale/2 und der Blickachse (b, 2a, b). Schräg
  ist b = scale/2: Die Wände bleiben bei jeder Raute so hoch, nur die
  Oberseite wächst mit a = scale · H/(2W). Von oben ist a = scale/2 und
  b = 0.
- **`--camera`:** `W:H` von 2:1 bis 1:1, gekürzt, oder `top`. Vorgabe 2:1,
  Byte für Byte wie vorher.
- **Ganze Pixel statt Vielfachen von 4:** Ein Paar aus scale und Kamera
  gilt, wenn a ganz und der scale gerade ist. Für 2:1 ist das dieselbe
  Regel. Sie gilt für `--scale`, für die nativen Stufen und für das
  Verdecken. Ein ungültiges Paar bricht ab und nennt die nächsten gültigen
  Kameras beim selben scale und die nächsten scales für diese Kamera.
- **Ein Baum, eine Kamera:** `map.json` hält `camera`, `direction` und
  `projection` fest, ein Lauf mit einer anderen Kamera bricht ab, bevor er
  einen Chunk liest. Ein Baum ohne `camera` ist 2:1.
- **Die Richtung steht schon jetzt da:** `direction` ist in dieser Stufe
  immer `se`. So ändert sich das Format für das Frontend nur einmal.
- **Licht unbekannter Blöcke** entscheidet bei jeder Kamera das Raster in
  2:1 beim scale der Basis. So hängt das Licht einer Welt nicht an der
  Kamera.

Siehe [Die Kamera](../renderer/kamera.md) und
[`map.json`](../benutzung/map-json.md), „Kamera und Projektion“.

### Strahl im Frontend

- **Schräge Kameras (`y` > 0 in `projection`):** Das Frontend geht den
  Strahl durch die Mitte des Pixels als Gang durch das Würfelgitter entlang
  der Blickachse (b, 2a, b), von `maxY` bis `minY`, für jede schräge
  Kamera, auch 2:1. Treffer ist wie bisher der erste Würfel, dessen Zelle
  bis zu ihm hinauf gefüllt ist.
- **Draufsicht (`y` = 0):** Der Strahl ist senkrecht und trifft je Schicht
  genau einen Würfel, den der Spalte unter dem Pixel. Welcher es ist, sagt
  die Höhe aus `heights`.
- **Kantenpixel:** Liegt die Mitte eines Pixels genau auf einer Blockkante,
  gilt die Füllregel des Renderers. Die Kantenpixel in
  `renderer/tests/fixtures/projektion.json` legen sie fest: auf Oberseiten
  für `top`, 1:1 und 5:3, an Wänden für 1:1 und 5:3.
- **Zahlen nur aus `map.json`:** Das Frontend rechnet aus `projection`
  (`u`, `v`, `y`) und führt keine Tabelle der Kameras. Fehlt `projection`,
  gilt 2:1 aus `scale`. Kennt es `azimuth` oder `direction` nicht, zeigt es
  keine Koordinaten und warnt in der Konsole.
- **Kosten:** Je Pixel höchstens etwa so viele Schritte wie bei 2:1, rund
  1150 über die ganze Bauhöhe. Steilere Kameras brauchen weniger, die
  Draufsicht einen je Schicht.

## Was sie ablöst

| Entscheidung | Abgelöst | Neu |
|---|---|---|
| [0002](0002-deckend-entscheidet-das-bild.md) | Nachbarn verdecken nur bei Vielfachen von 4 | nur auf ganzen Pixeln |
| [0008](0008-sprite-kanten-nicht-glaetten.md) | Treppen im 2:1-Raster | Treppen im Raster der Kamera |
| [0013](0013-scale-32-als-standard.md) | `--scale` nur in Vielfachen von 4 | scale und Kamera auf ganzen Pixeln |
| [0016](0016-native-stufen-nur-auf-wunsch.md) | native Stufen, solange der scale durch vier teilbar ist | solange die Stufe auf ganzen Pixeln liegt |
| [0035](0035-koordinaten-aus-hoehenkarten.md) | der Strahl entlang (1, 1, 1) | ein Gang durch das Würfelgitter entlang der Achse aus `projection`, von oben senkrecht |
| [0038](0038-cutout-wie-im-spiel.md) | der Satz zur festen Blickachse | die Blickachse der Kamera des Laufs |

Die Reihenfolge nach Höhe und Tiefe aus
[0001](0001-zeichenreihenfolge-statt-tiefenpuffer.md) gilt für jede Achse
mit b ≥ 0 und a > 0, siehe [Die Kamera](../renderer/kamera.md),
„Zeichenreihenfolge“.

## Verworfene Alternativen

- **Echt orthografische Kameras.** Keine schräge Kamera ist es, auch 2:1
  nicht: Sie projizieren entlang ihrer Achse und stauchen das Bild danach
  senkrecht. Echt orthografisch hiesse b = √(2(h² − a²)), bei 2:1 etwa
  scale · √6/4. Das ist kaum je ganz, die Blöcke lägen nicht auf ganzen
  Pixeln, und die Wände wären je Kamera anders hoch.
- **Flacher als 2:1:** verdeckt mehr Gelände und kostet mehr Pixel je
  Spalte.
- **Steiler als 1:1:** Die Oberseite erschiene höher als von oben.
- **Ungültige Paare still runden,** etwa a = 9,6 auf 10. Dann zeichnete der
  Renderer eine andere Kamera als verlangt, ohne es zu sagen.
- **Ein Ordner je Kamera unter einem `--tiles`, schon jetzt.** Die
  Gliederung je Kamera und die Liste der Bäume kommen mit #68; bis dahin
  ist jede Kamera ein eigener Baum.
- **`direction` erst mit #68.** Dann änderte sich `map.json` für das
  Frontend zweimal.
- **Eine Formel je Kamera im Frontend:** Jede Zahl stünde dann zweimal,
  im Renderer und im Frontend.
- **Die drei Würfel je Schicht des heutigen Strahls verallgemeinern**
  ([Frontend](../frontend.md), „Koordinaten“): Sie tragen nur, wenn b/(2a)
  ganz ist, also bei 2:1.
- **Licht unbekannter Blöcke im Raster der Kamera des Laufs.** Von oben
  deckt schon eine flache Seerose ihren ganzen Umriss, und ihr Würfel
  bliebe dunkel.

## Folgen

- 2:1 bleibt Byte für Byte gleich, das Goldbild und die Tests mit festen
  Pixeln ändern sich nicht. Neue Goldbilder zeigen 4:3 und `top`.
- Mit a wachsen die Pixel je Spalte: 4:3 hat das 1,5-fache von 2:1, 1:1
  und `top` das Doppelte. Die Bytes der Basis je Spalte folgen dem bei den
  schrägen Kameras eng, `top` bleibt darunter, siehe
  [2026-10-01, Kameras](../messungen/2026-10-01-kameras.md).
- Manche Kameras haben weniger native Stufen: 16:9 bei scale 32 keine,
  4:3 zwei.
- Von oben verschwinden senkrechte Flächen, etwa Gras und Blumen.
- Bis das Frontend die Kamera liest, stimmen seine Koordinaten nur in
  Bäumen mit 2:1.
