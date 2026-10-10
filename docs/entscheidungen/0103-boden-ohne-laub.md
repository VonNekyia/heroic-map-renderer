---
title: "0103: Der Boden ohne Laub neben den Höhen"
description: Warum der Renderer neben den Höhen für die Koordinaten einen Boden ohne Laub aus der Heightmap MOTION_BLOCKING_NO_LEAVES schreibt, je Block nach der Wahl des Users, gepackt schon im Vorlauf, warum dazu und nicht statt, wann map.json ihn nennt, warum der Zeichenstand bleibt und was es an Platz, Arbeitsspeicher und im Browser kostet; ergänzt 0036.
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
- **Quelle:** die Heightmap `MOTION_BLOCKING_NO_LEAVES` des Chunks, je
  Spalte ihr Wert. Fehlt sie einem Chunk oder passt ihre Länge nicht, gilt
  für ihn die Oberfläche, `WORLD_SURFACE`.
- **Je Block:** die Wahl des Users am 11.10., zwischen 4×4 wie die Höhen,
  2×2 und je Block. So folgt eine Form jedem Block des Geländes. Die Höhen
  der Koordinaten bleiben je 4×4.
- **Format** wie die Höhen, Werte i16 in zlib, −32768 ohne Wert, `minY` und
  `maxY` gelten mit; aber 512 × 512 Werte je Region. `map.json` nennt die
  Zelle als `groundCell`, heute 1, neben `heightsCell`.
- **Gepackt im Vorlauf:** Der Vorlauf packt Höhen und Boden einer Region,
  sobald er sie gelesen hat, und hält bis zum Schreiben nur die Bytes. Nur
  ein Ausschnitt entpackt sie beim Schreiben wieder, je Region, um sie mit
  der alten Datei zu mischen. Ungepackt hielte er je Block 512 KiB je
  Region, auf der grossen Welt 1,23 GiB.
- **`map.json`** nennt ihn als `ground`, erst wenn ein Lauf über die ganze
  Welt oder `--heights` ihn für jede Region geschrieben hat. Ein Ausschnitt
  oder ein Update schreibt ihn sonst nur für seine Regionen und behält das
  Feld, wie es war. Fehlt das Feld, nehmen die Formen die Höhen.
- **Zeichenstand bleibt:** Keine Kachel und keine Höhe ändert sich, es
  kommt nur eine Datei dazu. Ein bestehender Baum bekommt sie mit einem
  `--heights`, ohne Kacheln.

Vom Reviewer am 10.10. mit dem Plan aus #273 freigegeben, je Block nach der
Wahl des Users am 11.10. Die Felder stehen in
[map.json](../benutzung/map-json.md), „Höhen“.

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
- **Je 4×4 wie die Höhen:** am kleinsten, ein Format mit den Höhen. Eine
  Form folgte dem Gelände aber nur in Stufen von 4 Blöcken.
- **Je 2×2:** vor der Wahl auf der grossen Welt auf 43,6 MB geschätzt, ein
  Viertel der Werte je Block. Der User nahm je Block.
- **Je Block ungepackt im Vorlauf halten:** wie bei den Höhen bisher, aber
  mit 1,23 GiB auf der grossen Welt.
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

Gemessen auf der Testwelt in
[2026-10-11, Boden je Block](../messungen/2026-10-11-boden-je-block.md),
auf die grosse Welt hochgerechnet:

- **Platz:** auf der Testwelt 11,19 MB, achtmal so viel wie je 4×4; auf der
  grossen Welt rund 85 MB, neben gut 13 MB Höhen. Vor der Wahl waren 220
  bis 250 MB geschätzt. Neben den Kacheln zählt beides nicht.
- **Arbeitsspeicher:** Der Vorlauf hält Höhen und Boden je Region gepackt,
  auf der grossen Welt rund 100 MB statt 157 MiB je 4×4 ungepackt. Dazu
  hält jeder Thread die Region, die er liest, ungepackt, je Block 512 KiB,
  und den Zustand von zlib. Die Spitze von `--heights` auf der Testwelt:
  mit einem Thread im Mittel 6,5 MiB unter der von vorher, mit allen
  Kernen 24,5 MiB darüber.
- **Live-Rendern und erster Render:** je Chunk ein Long-Array mehr, wie die
  Höhen, und je Region einmal 512 KiB packen. Die Zeit ist nicht gemessen;
  eine Messung gehört in die Nacht.
- **Browser:** je Region eine Datei von rund 29 kB, entpackt 512 KiB, 16-mal
  so viele Werte wie die Höhen. Wie viele Regionen das Frontend lädt und
  wie es die Sicht rechnet, entscheidet es in seiner PR zu #273; seine
  Grenze von 1024 Regionen für die Höhen hielte je Block 512 MiB.

## Folgen

- Jeder Lauf, der Höhen schreibt, schreibt den Boden mit, auch `--heights`
  und `--resume`; `--prune` entfernt beide.
- Der Server liefert `ground/` aus wie `heights/`.
- Das Frontend legt Regionen, Kreise und Linien auf den Boden; Nadeln,
  Banner und Schrift bleiben auf den Höhen. Das entscheidet es in seiner
  PR zu #273.
- Das Plugin stösst für bestehende Bäume einmal `--heights` an.
