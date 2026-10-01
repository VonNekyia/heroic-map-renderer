---
title: "0052: Genordete Kameras"
description: Warum top-north und north-45 als zweiter Azimut mit u = x und v = z rechnen, h und a gleich scale, bei jedem scale, mit direction s und azimuth north in map.json, und warum genordet nur zwei Nachbarn verdecken.
status: gilt
date: 2026-10-01
issues: [67]
code:
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/sprites.rs
  - renderer/src/cli.rs
  - renderer/src/render/pyramid.rs
---

# 0052: Genordete Kameras

## Anlass

Stufe 2 aus [0051](0051-kameras-und-richtungen.md), #67. Vom User am 01.10.
entschieden: `top-north` gehört zu den Kameras der ersten Entscheidung,
`north-45` kam dazu. Beide blicken mit Norden oben, die Kamera steht im
Süden. Bis hier rechnete die Projektion nur diagonal, mit u = x − z und
v = x + z.

## Entscheidung

- **Zweiter Azimut:** Genordet ist u = x und v = z, dazu h = a = scale. Die
  Formel bleibt `screen_x = u · h`, `screen_y = v · a − y · b`, die
  Blickachse (0, a, b).

  | Kamera | h | a | b | Achse |
  |---|---|---|---|---|
  | `top-north` | scale | scale | 0 | (0, 1, 0) |
  | `north-45` | scale | scale | scale | (0, 1, 1) |

- **Jeder scale:** Alle Ecken liegen auf ganzen Pixeln, auch bei einem
  ungeraden scale. Native Stufen gehen, solange der scale gerade ist und
  die Hälfte mindestens 4.
- **`map.json`:** `azimuth` ist `north`, `direction` ist `s`, die Kamera
  steht im Süden. Diagonal bleibt es `diagonal` und `se`. Fehlt
  `direction`, gilt die Richtung der Kamera; eine andere bricht ab, bis
  Stufe 3 die übrigen bringt.
- **Renderpfad:**
  - Das Band der Kandidaten ist ein Rechteck in x und z.
  - Gezeichnet wird nach `(y, z, x)`.
  - Der Umriss eines Würfels ist das Rechteck von (0, −b) bis (h, a).
  - Verdeckt ist ein Würfel, wenn der Nachbar nach +z seinen ganzen Umriss
    deckt und der nach +y seinen Boden. Der nach +x liegt neben dem
    Umriss und zählt nicht. Bei `top-north` deckt der Block darüber allein,
    wie bei `top`.
- **Unverändert:** Die Teile je Würfel im Raum aus
  [0050](0050-teile-je-wuerfel-im-raum.md), der Spielraum (von oben nur im
  eigenen Würfel) und das Licht unbekannter Blöcke aus dem Raster in 2:1
  beim scale der Basis hängen nicht am Azimut.

Siehe [Die Kamera](../renderer/kamera.md), „Genordet“.

## Verworfene Alternativen

- **Genordet als Drehung der Welt.** Norden oben ist kein Vielfaches von
  90° gegen die Diagonale; die Welt liesse sich nicht Block für Block
  drehen. Der Azimut ändert die Achsen im Bild, die Drehung aus Stufe 3
  die Daten. Beides bleibt getrennt.
- **h = scale/2 wie diagonal.** Dann wäre ein Block genordet nur halb so
  breit wie hoch, und bei scale 16 fiele jede zweite Texelspalte weg. Mit
  h = a = scale ist bei scale 16 jedes Texel genau ein Pixel.
- **`north-45` mit b = scale/2,** gleich hohe Wände wie diagonal. Dann wäre
  die Südwand halb so hoch wie die Oberseite tief. Der User wollte
  Oberseite und Südwand je scale × scale.
- **Kantenpixel in `projektion.json` auch genordet.** Jede Blockkante liegt
  dort auf ganzen Pixeln, nie auf einer Pixelmitte; es gibt keine.
- **`direction` `n` für Norden oben.** `direction` sagt, wo die Kamera
  steht, nicht wohin sie blickt (User, 01.10.).

## Folgen

- Eine Oberseite belegt genordet scale × scale Pixel, bei scale 16 so viel
  wie in 2:1 bei scale 32, bei scale 32 das Vierfache.
- Bei `north-45` sind Nordwände verdeckt, Ost- und Westwände stehen auf der
  Kante und fallen weg. Häuser zeigen ihre Südwand.
- `top-north` zeigt wie `top` nur Oberseiten, um 45° gedreht.
- Eine Stufe nach Norden ist bei `north-45` unsichtbar, wie in 2:1 eine nach
  Norden oder Westen, siehe [Die Kamera](../renderer/kamera.md), „Stufen,
  die von der Kamera wegzeigen“.
- Bis das Frontend genordet rechnet, stimmen seine Koordinaten in diesen
  Bäumen nicht.
- Was die beiden bei scale 16 gegen 2:1 bei scale 32 kosten, steht in
  [2026-10-02, Genordete Kameras](../messungen/2026-10-02-genordete-kameras.md).
