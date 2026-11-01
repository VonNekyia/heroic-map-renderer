---
title: "0134: Die Höhen des Reliefs ohne Pflanzen"
description: Warum die Höhen des Reliefs aus der Heightmap MOTION_BLOCKING kommen statt aus WORLD_SURFACE, damit Bodenpflanzen kein Kreuz aus hell und dunkel mehr werfen; warum die gespeicherte Heightmap statt einer eigenen Liste, was weiter zählt, der Rückfall je Chunk, ZEICHENSTAND_RELIEF 3, was das Neuzeichnen und der Download kosten und was verworfen ist; ergänzt 0122.
status: gilt
date: 2026-11-01
issues: [387]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/stand.rs
---

# 0134: Die Höhen des Reliefs ohne Pflanzen

Löst in [0122](0122-relief-von-oben.md) nur ab, woher die Höhen kommen. Die
Regel im Einzelnen und der Beleg aus dem Spiel stehen in
[Relief von oben](../renderer/relief.md), „Höhen“.

## Anlass

Der User am 27.10.: Um jede Bodenpflanze liegt im Relief ein Kreuz, etwa bei
totem Gestrüpp und Gras in der Wüste. Der Nachbar im Norden und im Westen ist
heller, der im Osten und im Süden dunkler. Das sieht aus wie ein Schatten der
Pflanze und macht flache Flächen unruhig (#387).

`WORLD_SURFACE` zählt jede Pflanze als vollen Block. Damit bekommen ihre vier
Nachbarn ein Gefälle von einem halben Block, also den Faktor 1 ± 0,06, und
hohes Gras das Doppelte.

## Entscheidung

- **Die Höhen des Reliefs kommen aus `MOTION_BLOCKING`,** für Gefälle und
  Schatten, in Kacheln und Schicht. Das Feld steht in jedem fertigen Chunk.
- **Gezeichnet wird weiter ab `WORLD_SURFACE`.** Die Pflanze bleibt auf der
  Karte und hebt nur das Relief nicht mehr.
- **Die Höhen in `map.json` bleiben,** `heights` aus `WORLD_SURFACE` und
  `ground` aus `MOTION_BLOCKING_NO_LEAVES`. Sie dienen den Koordinaten und den
  Formen auf dem Gelände, nicht dem Relief.
- **Es gilt die Regel des Spiels,** nicht eine eigene Liste. Darum bleiben auch
  kleine Blöcke, die sie zählt, etwa Druckplatten, Hängeschilder, Laternen und
  `sculk_vein`, dazu Bambus und Kakteen. Um sie bleibt das Kreuz.
- **Fehlt das Feld** oder passt seine Länge nicht, gilt für den ganzen Chunk
  `WORLD_SURFACE`. In den gezählten Stichproben kommt das bei keinem fertigen
  Chunk vor.
- **`ZEICHENSTAND_RELIEF` steigt auf 3:** Bäume mit Relief brauchen einen
  vollen Lauf. Bäume ohne Relief behalten ihren Abdruck.
- **Der Mod** rechnet die Minimap nach derselben Regel. Er liest dasselbe Feld,
  sein Client bekommt es mit dem Chunk vom Server.

## Verworfene Alternativen

- **Eine eigene Liste der Pflanzen:** Die Regel des Spiels hat sich schon
  zwischen 26.2 und 26.3 geändert, siehe „Höhen“. Eine Liste müsste jeder
  Version folgen, jedem Paket und dem Mod. Das gespeicherte Feld kostet
  nichts, und beide Seiten lesen dasselbe.
- **`MOTION_BLOCKING_NO_LEAVES`:** Es lässt Laub weg. Dann wäre ein Wald von
  oben wieder eine flache Fläche, und genau dafür gibt es das Relief.
- **`OCEAN_FLOOR`:** Es lässt auch die Flüssigkeit weg. Das Relief zeigte den
  Grund durch das Wasser, und die Oberfläche wäre nicht mehr eben.
- **Bei fehlendem Feld aus den Blöcken rechnen, je Spalte:** Das braucht
  genau die Liste von oben, und es kommt nicht vor.
- **Die Pflanzen aus dem Gefälle herausrechnen,** etwa die Nachbarn glätten:
  Das träfe auch echte Stufen von einem Block, die das Relief zeigen soll.

## Folgen

Kosten nach Regel 26, die Zahlen in
[2026-11-01, Relief ohne Pflanzen](../messungen/2026-11-01-relief-ohne-pflanzen.md):

- **Live-Rendern:** kein Mehraufwand, den sich zu messen lohnt: ein Feld mehr
  beim Lesen des Chunks, sonst nur andere Zahlen in derselben Rechnung.
- **Arbeitsspeicher:** 296 Bytes mehr je geladenem Chunk, auch in Bäumen ohne
  Relief. Ein eigener Weg dafür lohnt nicht.
- **Neuzeichnen und Download:** Fast jede Kachel mit einer Pflanze in der Nähe
  ändert sich. Der volle Lauf und was Spieler danach nachladen, steht in der
  Messung.
