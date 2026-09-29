---
title: Assets und Biomdaten
description: Welche Asset- und Datenwurzeln der Renderer braucht, wie man sie aus dem Client-JAR holt und in welcher Reihenfolge er sie stapelt, samt Biomen und Bannermustern.
code:
  - renderer/src/cli.rs
  - renderer/src/assets/pack.rs
  - renderer/src/assets/colors.rs
  - renderer/src/assets/blockentity.rs
  - renderer/src/assets/mod.rs
---

# Assets und Biomdaten

Der Renderer braucht einen vollständigen Asset-Baum aus dem Client-JAR der
unterstützten Version, heute 26.2, und für die Biomfarben die Daten aus
demselben JAR. Beides kommt über `--assets` und `--data`,
jeweils mehrfach; spätere Wurzeln gewinnen. Wie der Renderer eine
Wurzel liest, steht in [Packs und Wurzeln](../renderer/packs.md).

## Assets aus dem Client-JAR

Ein Overlay-Pack allein reicht nicht: das TerraNova-Pack bringt 39 von 1198
Blockstates mit und keine Colormaps. Die Basis kommt aus dem Client-JAR der
Version, die der Server fährt (hier 26.2):

```powershell
$v = "26.2"; $m = Get-Content "$env:APPDATA\.minecraft\versions\$v\$v.json" | ConvertFrom-Json; Invoke-WebRequest $m.downloads.client.url -OutFile "$env:TEMP\mc.zip"; Expand-Archive "$env:TEMP\mc.zip" "$env:TEMP\mc" -Force; Move-Item "$env:TEMP\mc\assets" vanilla-assets
```

Die SHA1-Prüfsumme steht im selben Manifest unter `downloads.client.sha1`.
Der ganze Baum gehört dazu: Truhen, Banner und die übrigen Blockentities
brauchen die Texturen unter `textures/entity`, siehe
[Blockentities](../renderer/blockentities.md). Fehlen Texturen, sagt es der
Lauf gleich nach der Sprite-Tabelle („… Texturen fehlen, dort steht die
Missing-Textur“), und die Liste am Ende nennt jede, etwa
`minecraft:entity/chest/normal`; auf der Karte stehen dort Schachbretter.

Gibt es `vanilla-assets` schon, legt der letzte Befehl oben den neuen Baum
darin als `vanilla-assets\assets` ab, und den liest der Renderer nicht.
Fehlen einem bestehenden Baum nur die Texturen der Blockentities, holt sie
nach dem Auspacken statt des letzten Befehls dieser:

```powershell
Move-Item "$env:TEMP\mc\assets\minecraft\textures\entity" vanilla-assets\minecraft\textures\entity
```

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

## Bannermuster

Die Muster des Spiels kennt der Renderer aus seiner Tabelle. Eine
Datenwurzel braucht es nur für Muster aus Datenpaketen: Erwartet wird
`<DIR>/<namespace>/banner_pattern/**/*.json` in denselben Wurzeln wie die
Biome, spätere überschreiben frühere und die des Spiels. Die Ausgabe nennt,
wie viele Muster jede Wurzel bringt, und am Ende jedes Muster und jeden
Farbstoff aus der Welt, den weder das Spiel noch eine Wurzel kennt, siehe
[Blockentities](../renderer/blockentities.md), „Banner“.

## Dimensionen

Die Dimensionstypen des Spiels kennt der Renderer ebenso aus einer Tabelle.
Eine Datenwurzel braucht es nur für eigene Dimensionen und Typen aus
Datenpaketen: `<DIR>/<namespace>/dimension/**/*.json` und
`<DIR>/<namespace>/dimension_type/**/*.json`, in denselben Wurzeln wie die
Biome. Welcher Typ zu welcher Dimension gehört:
[Dimensionstypen](../renderer/dimensionstypen.md).

## Was eine Datenwurzel tragen muss

Eine Datenwurzel darf nur Biome, nur Bannermuster oder nur Dimensionen
tragen. Ab bricht der Lauf nur bei einer, die nichts davon trägt, auch
keine Datei, die der Codec ablehnt: Dann stimmt vermutlich der Pfad nicht.
Die Meldung nennt die erwarteten Orte und jeden Ordner, der sich nicht
lesen liess (`Assets::load_data`).

## Im Repository

`vanilla-assets/`, `vanilla-data/` und `assets/` stehen in `.gitignore`,
siehe [Eingabedaten](../entwicklung/eingabedaten.md).
