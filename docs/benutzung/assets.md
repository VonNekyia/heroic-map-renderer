---
title: Assets und Biomdaten
description: Welche Asset- und Datenwurzeln der Renderer braucht, wie man sie aus dem Client-JAR holt und in welcher Reihenfolge er sie stapelt.
code:
  - renderer/src/cli.rs
  - renderer/src/assets/pack.rs
  - renderer/src/assets/colors.rs
---

# Assets und Biomdaten

Der Renderer braucht einen vollständigen Asset-Baum aus dem Client-JAR der
unterstützten Version, heute 26.2, und für die Biomfarben die
Biomdefinitionen aus demselben JAR. Beides kommt über `--assets` und
`--data`, jeweils mehrfach; spätere Wurzeln gewinnen. Wie der Renderer eine
Wurzel liest, steht in [Packs und Wurzeln](../renderer/packs.md).

## Assets aus dem Client-JAR

Ein Overlay-Pack allein reicht nicht: das TerraNova-Pack bringt 39 von 1198
Blockstates mit und keine Colormaps. Die Basis kommt aus dem Client-JAR der
Version, die der Server fährt (hier 26.2):

```powershell
$v = "26.2"; $m = Get-Content "$env:APPDATA\.minecraft\versions\$v\$v.json" | ConvertFrom-Json; Invoke-WebRequest $m.downloads.client.url -OutFile "$env:TEMP\mc.zip"; Expand-Archive "$env:TEMP\mc.zip" "$env:TEMP\mc" -Force; Move-Item "$env:TEMP\mc\assets" vanilla-assets
```

Die SHA1-Prüfsumme steht im selben Manifest unter `downloads.client.sha1`.

Danach wird der Baum gestapelt übergeben, spätere Wurzeln gewinnen:

```bash
--assets ./vanilla-assets --assets ./assets
```

## Biomdefinitionen

Für die Färbung von Gras, Laub und Wasser braucht der Renderer ausserdem
die Biomdefinitionen. Sie liegen im selben JAR unter `data/`, nicht unter
`assets/`:

```powershell
New-Item -ItemType Directory -Force vanilla-data\minecraft\worldgen | Out-Null; Move-Item "$env:TEMP\mc\data\minecraft\worldgen\biome" vanilla-data\minecraft\worldgen\biome
```

```bash
--data ./vanilla-data
```

66 Dateien, 352 kB. Ohne `--data` bekommt jeder Block die Farben von
`plains`; Ozeane und Wälder sehen dann überall gleich aus, aber nicht
falsch. Datenpakete mit eigenen Biomen kommen als weitere Wurzeln dazu,
spätere überschreiben frühere, auch Vanilla-Biome, die ein Paket
umdefiniert:

```bash
--data ./vanilla-data --data ./weitere-daten
```

Erwartet wird `<DIR>/<namespace>/worldgen/biome/**/*.json`; Unterordner
gehören zur ID: `terranova:hoehle/pilzwald` liegt unter
`terranova/worldgen/biome/hoehle/pilzwald.json`. Kommt in der Welt ein Biom
vor, für das keine Definition geladen ist, sagt der Renderer es beim Start
und färbt es wie `plains`.

Welche Felder eines Bioms Pflicht sind und wie der Renderer sie liest,
steht in [Biomfarben](../renderer/biomfarben.md), „Biome lesen“.

## Im Repository

`vanilla-assets/`, `vanilla-data/` und `assets/` stehen in `.gitignore`,
siehe [Eingabedaten](../entwicklung/eingabedaten.md).
