---
title: "0103: Der Boden ohne Laub neben den Höhen"
description: Warum der Renderer neben den Höhen für die Koordinaten einen Boden ohne Laub aus der Heightmap MOTION_BLOCKING_NO_LEAVES schreibt, im selben Format je 4×4, warum dazu und nicht statt, wann map.json ihn nennt, warum der Zeichenstand bleibt und was es kostet; ergänzt 0036.
status: gilt
date: 2026-10-10
issues: [273]
code:
  - renderer/src/render/heights.rs
  - renderer/src/world/chunk.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# 0103: Der Boden ohne Laub neben den Höhen

## Anlass

Regionen, Kreise und Linien der Ebenen liegen in den schrägen Ansichten auf
den Höhen aus [0036](0036-hoehen-aus-der-heightmap.md). Unter Laub sind das
die Kronen; eine Region im Wald schwebt so über den Bäumen, und ihr Rand ist
überall verdeckt (#273). Die Formen brauchen den Boden. Die Koordinaten
sollen weiter den Block nennen, den man sieht.

## Entscheidung

- **Dazu, nicht statt:** Die Höhen bleiben, wie 0036 sie festlegt. Daneben
  schreibt der Renderer den Boden ohne Laub, `ground/{x}.{z}.bin` neben
  `heights/`. Die Koordinaten nehmen weiter die Höhen, die Formen den Boden.
- **Quelle:** die Heightmap `MOTION_BLOCKING_NO_LEAVES` des Chunks, je 4×4
  Spalten der obere Median wie bei den Höhen. Fehlt sie einem Chunk oder
  passt ihre Länge nicht, gilt für ihn die Oberfläche, `WORLD_SURFACE`.
- **Format** wie die Höhen: 128 × 128 Werte i16 in zlib, −32768 ohne Wert,
  `heightsCell`, `minY` und `maxY` gelten mit.
- **`map.json`** nennt ihn als `ground`, erst wenn ein Lauf über die ganze
  Welt oder `--heights` ihn für jede Region geschrieben hat. Ein Ausschnitt
  oder ein Update schreibt ihn sonst nur für seine Regionen und behält das
  Feld, wie es war. Fehlt das Feld, nehmen die Formen die Höhen.
- **Zeichenstand bleibt:** Keine Kachel und keine Höhe ändert sich, es
  kommt nur eine Datei dazu. Ein bestehender Baum bekommt sie mit einem
  `--heights`, ohne Kacheln.

Vom Reviewer am 10.10. mit dem Plan aus #273 freigegeben. Die Felder stehen
in [map.json](../benutzung/map-json.md), „Höhen“.

## Belege

Per `javap`, am Client von 26.2 und am Server von 26.3:

- `ChunkStatus.FINAL_HEIGHTMAPS` ist `OCEAN_FLOOR`, `WORLD_SURFACE`,
  `MOTION_BLOCKING` und `MOTION_BLOCKING_NO_LEAVES`. Sie gilt in 26.2 ab dem
  Status `carvers`, in 26.3 ab `terrain`, also für jeden fertigen Chunk.
  `SerializableChunkData` schreibt die Heightmaps aus `heightmapsAfter()`
  des Status.
- `Heightmap$Types.MOTION_BLOCKING_NO_LEAVES` hat die Nutzung `CLIENT`; sie
  bleibt also nach der Generierung erhalten.
- Wer zählt: in 26.2 ein Block, der Bewegung aufhält (`blocksMotion`) oder
  Flüssigkeit hält, und kein `LeavesBlock` ist. In 26.3 einer im Tag
  `blocks_motion_in_heightmap_no_leaves` oder mit Flüssigkeit. Über Wasser
  ist der Boden also die Oberfläche wie bei den Höhen, und Gras und Blumen
  zählen nicht.
- In der Testwelt hat jeder fertige Chunk sie: 5 124 von 5 124 in sechs
  Regionen und dem Fixture, am 10.10. gezählt.

## Verworfene Alternativen

- **Statt der Höhen:** Die Gründe aus 0036 gelten weiter. Unter Laub nennt
  die Anzeige die Krone, die man sieht.
- **Nur die Differenz zur Oberfläche als u8:** ausserhalb von Wald fast nur
  Nullen, also kleiner. Es kostet aber ein zweites Format im Frontend, und
  die Ersparnis zählt neben den Kacheln nicht.
- **Eine zweite Ebene in der Datei der Höhen:** änderte das Format, das
  ältere Frontends lesen.
- **Aus den Blöcken rechnen, wenn die Heightmap fehlt:** baute
  `blocksMotion` und das Tag aus 26.3 nach, für einen Fall, den es in
  fertigen Chunks nicht gibt.
- **Den Zeichenstand heben:** zwänge jeden Baum zu einem vollen Lauf, nur
  für eine neue Datei.

## Kosten nach Regel 26

Geschätzt, nicht gemessen:

- **Live-Rendern und erster Render:** je Chunk ein Long-Array mehr und 16
  Mediane, wie die Höhen; kaum messbar.
- **Arbeitsspeicher:** je Region 32 KiB mehr im Vorlauf.
- **Platz:** etwa so viel wie die Höhen, auf der grossen Welt rund 14 MB;
  neben den Kacheln zählt das nicht.

## Folgen

- Jeder Lauf, der Höhen schreibt, schreibt den Boden mit, auch `--heights`
  und `--resume`; `--prune` entfernt beide.
- Der Server liefert `ground/` aus wie `heights/`.
- Das Frontend legt Regionen, Kreise und Linien auf den Boden; Nadeln,
  Banner und Schrift bleiben auf den Höhen. Das entscheidet es in seiner
  PR zu #273.
- Das Plugin stösst für bestehende Bäume einmal `--heights` an.
