---
title: map.json
description: Die Felder von map.json, wann der Export die Datei schreibt und warum die Projektion nicht darin steht.
code:
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
  - web/src/main.ts
---

# `map.json`

`map.json` liegt an der Wurzel jedes Kachelbaums und sagt dem Frontend, was
es vorfindet: Kachelgrösse, scale, Zoomstufen, Pfadmuster, den belegten
Bereich, die Zahl nativer Stufen und die Kennung der Welt. Der Typ ist
`MapInfo` in
[`renderer/src/render/pyramid.rs`](../../renderer/src/render/pyramid.rs);
das Frontend liest die Datei in `web/src/main.ts`.

## Felder

```json
{
  "tileSize": 256,
  "scale": 32,
  "minZoom": 0,
  "maxZoom": 10,
  "tiles": "{z}/{x}/{y}.webp",
  "bounds": [-10240, 0, -6144, 4096],
  "nativeLevels": 0,
  "world": "cb13a94d6c88dae1-6872d5d8ff54db07"
}
```

| Feld | Inhalt | Mehr |
|---|---|---|
| `tileSize` | Kantenlänge einer Kachel in Pixeln | |
| `scale` | Pixelbreite eines Blocks auf der Basis | [Kamera](../renderer/kamera.md) |
| `minZoom`, `maxZoom` | gröbste und feinste Stufe; `maxZoom` ist die Basis | [Zoomstufen](zoomstufen.md) |
| `tiles` | Pfadmuster der Kacheln | [Kacheln exportieren](kacheln.md) |
| `bounds` | belegter Bereich auf der feinsten Stufe in Pixeln, `[links, oben, rechts, unten]` | |
| `nativeLevels` | Zahl der nativen Stufen | [Zoomstufen](zoomstufen.md), „Native Stufen“ |
| `world` | Kennung der Welt und Dimension, oder `null` | [Welten und Kennung](welten.md) |

Die Projektion selbst steht nicht drin: sie hängt allein an `scale`, und
die Formel gehört in den Renderer, nicht in eine Datei. Sonst gäbe es zwei
Quellen für dieselbe Wahrheit.

## Wann sie geschrieben wird

Jeder Export schreibt `map.json` vor seiner ersten Kachel und am Ende,
`--pyramid` bei jedem Aufruf. Die Datei geht dabei jedes Mal ganz auf die
Platte, bevor sie die alte ersetzt: Nach einem Stromausfall steht die alte
oder die neue da, und kein Lauf scheitert an einer halben.
