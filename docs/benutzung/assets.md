---
title: Assets und Biomdaten
description: Welche Asset- und Datenwurzeln der Renderer braucht, wie er sie mit Zustimmung selbst aus dem Client-JAR von Mojang nimmt oder man sie von Hand holt, und in welcher Reihenfolge er sie stapelt, samt Biomen und Bannermustern.
code:
  - renderer/src/cli.rs
  - renderer/src/cli/client.rs
  - renderer/src/cli/zip.rs
  - renderer/src/assets/pack.rs
  - renderer/src/assets/colors.rs
  - renderer/src/assets/blockentity.rs
  - renderer/src/assets/mod.rs
---

# Assets und Biomdaten

Der Renderer braucht einen vollständigen Asset-Baum aus dem Client-JAR der
unterstützten Versionen, heute 26.2 oder 26.3 ([0059](../entscheidungen/0059-welten-aus-26-2-und-26-3.md)), und für die Biomfarben die Daten aus
demselben JAR. Beides nimmt der Renderer mit `--download-client-jar` selbst
aus dem Jar, siehe „Von Mojang laden“, oder es kommt von Hand über
`--assets` und `--data`, jeweils mehrfach; spätere Wurzeln gewinnen. Wie der Renderer eine
Wurzel liest, steht in [Packs und Wurzeln](../renderer/packs.md).

## Von Mojang laden

Mit `--download-client-jar` nimmt der Renderer Assets und Daten selbst aus
dem Client-Jar, ohne `--assets` und `--data` von Hand. Ohne den Schalter
lädt er nichts. Warum so: [0086](../entscheidungen/0086-client-jar-von-mojang.md).

```bash
heroic-map-renderer --world ./world --tiles ./tiles --download-client-jar
```

- **Zustimmung:** Der Schalter ist sie. Ohne ihn und ohne `--assets` bricht
  ein Lauf mit `--tiles`, `--render` oder `--block` ab und nennt den Text,
  für eine Welt aus 26.3 etwa:

  > Der Renderer lädt das Client-Jar von Minecraft 26.3 (41,5 MB) von
  > Mojangs Servern und nutzt daraus Texturen, Modelle und Biome. Das Jar
  > gehört Mojang und darf nicht weitergegeben werden. Mit der Zustimmung
  > bestätigst du, dass du Minecraft: Java Edition besitzt, und nimmst die
  > Minecraft-EULA an: https://www.minecraft.net/eula

  `eula=true` eines Servers zählt nicht: Es gilt dem Server-Programm, nicht
  dem gekauften Spiel.
- **Version:** die neueste, die der Renderer kennt und deren DataVersion
  höchstens die aus `level.dat` ist, also 26.2 bis DataVersion 5022 und 26.3
  ab 5023; ohne `level.dat` 26.3. `--client-version 26.2` oder `26.3` wählt
  selbst.
- **Laden:** über HTTP von `piston-data.mojang.com`. Die Adresse folgt aus
  dem SHA-1, den das Binär je Version kennt; geprüft werden SHA-1 und
  Grösse, eine Umleitung bricht ab, nach 10 min ebenso. Wo nur HTTPS
  hinausgeht, scheitert der Download; dann bleibt der Weg von Hand, die
  Meldung nennt ihn.
- **Cache:** einmal ausgepackt nach `client-<version>-<sha1>` unter
  `--cache-dir`, ohne den Schalter unter Windows in
  `%LOCALAPPDATA%\heroic-map-renderer`, sonst in
  `$XDG_CACHE_HOME/heroic-map-renderer` oder `~/.cache/heroic-map-renderer`.
  Nur `assets/` und aus `data/` Biome, Bannermuster und Dimensionen, für
  26.2 11 081 Dateien. Liegt der Ordner da, geht der Lauf nicht ins Netz.
  Jede Datei trägt die Bauzeit des Jars, so gibt ein neu ausgepackter Cache
  denselben Fingerabdruck, siehe [Updates](updates.md), „Anderer Renderer,
  andere Assets“. Ausgepackt wird erst in einen Ordner des Prozesses, dann
  umbenannt; zwei Läufe zugleich stören sich nicht. Hält unter Windows ein
  Echtzeitschutz frische Dateien noch offen, versucht der Lauf das
  Umbenennen bis zu 10 s lang erneut.
- **Nie weitergeben:** Der Cache darf nicht unter `--tiles` liegen, sonst
  bricht der Lauf ab, bevor er etwas lädt; verglichen werden die Ordner,
  wie die Platte sie sieht, mit `..` und unter Windows in jeder Schreibung.
  Ebenso wenig gehört er unter die Seite von `--serve --web`: Auch von dort
  lieferte der Server ihn aus, und das prüft der Lauf nicht. In ein Paket
  für andere gehört er auch nicht.
- **Stapeln:** Die Basis aus dem Jar steht vor allen `--assets` und
  `--data`. Overlay-Packs und Datenpakete kommen dazu wie unten:

  ```bash
  heroic-map-renderer --world ./world --tiles ./tiles --download-client-jar --assets ./assets --data ./weitere-daten
  ```

## Assets aus dem Client-JAR

Von Hand, ohne `--download-client-jar`:

Ein Overlay-Pack allein reicht nicht: das Pack der grossen Welt bringt 39 von 1198
Blockstates mit und keine Colormaps. Die Basis kommt aus dem Client-JAR der
Version, die der Server fährt (hier 26.3). Für eine Welt aus 26.2 also die
aus 26.2: Mit denen aus 26.3 schattiert der Renderer etwa die Stängel der
Blumenbeete nach Norden, wie 26.3.

```powershell
$v = "26.3"; $m = Get-Content "$env:APPDATA\.minecraft\versions\$v\$v.json" | ConvertFrom-Json; Invoke-WebRequest $m.downloads.client.url -OutFile "$env:TEMP\mc.zip"; Expand-Archive "$env:TEMP\mc.zip" "$env:TEMP\mc" -Force; Move-Item "$env:TEMP\mc\assets" vanilla-assets
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
falsch. Cinematic liest aus ihnen auch die Farben des Himmels für das
Wasser und die Temperatur für seine Wärme. Ein Biom ohne Definition wird `plains`; ohne
`--data` hat auch `plains` keine, und Cinematic nimmt die Farben des
Dimensionstyps und die Temperatur 0,8, siehe
[Cinematic](../renderer/cinematic.md), „Farbe des Himmels“ und „Wärme“.
Datenpakete mit eigenen Biomen kommen als weitere Wurzeln dazu, spätere
überschreiben frühere, auch Vanilla-Biome, die ein Paket umdefiniert:

```bash
--data ./vanilla-data --data ./weitere-daten
```

Erwartet wird `<DIR>/<namespace>/worldgen/biome/**/*.json`; Unterordner
gehören zur ID: `beispiel:hoehle/pilzwald` liegt unter
`beispiel/worldgen/biome/hoehle/pilzwald.json`. Kommt in der Welt ein Biom
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
