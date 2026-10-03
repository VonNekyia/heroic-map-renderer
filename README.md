<p align="center">
  <img src="docs/bilder/banner.webp" alt="Heroic Map Renderer: isometrische Minecraft-Karten, pixelgenau wie im Spiel, in Rust, mit GPU">
</p>

<p align="center">
  <a href="https://github.com/VonNekyia/heroic-map-renderer/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/VonNekyia/heroic-map-renderer/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="Minecraft Java 26.x" src="https://img.shields.io/badge/Minecraft_Java-26.x-5d8a3a">
  <img alt="Renderer in Rust" src="https://img.shields.io/badge/Renderer-Rust-b7410e">
  <img alt="Grafikkarte über Vulkan oder DX12" src="https://img.shields.io/badge/GPU-Vulkan_%C2%B7_DX12-4a6fa5">
  <img alt="Frontend mit Leaflet" src="https://img.shields.io/badge/Frontend-Leaflet-199900">
</p>

**Heroic Map Renderer** zeichnet Minecraft-Java-Welten als isometrische
Karte. Jeder Block kommt mit seinem Modell, seiner Textur, seiner Biomfarbe
und seinem Licht aufs Bild, so wie der Client ihn zeichnet. Heraus kommen
WebP-Kacheln, die ein schlankes Leaflet-Frontend zeigt. Der Browser rendert
keine Geometrie, nur fertige Bilder.

```
Minecraft-Welt + Resourcepack  ->  Rust-Renderer  ->  WebP-Kacheln  ->  Leaflet
```

<p align="center">
  <img src="docs/bilder/dorf.webp" alt="Ein Dorf am Wasser mit Feldern, Stegen und Marktständen, scale 32">
</p>

## Auf einen Blick

| | |
|---|---|
| **1 700 bis 1 800 Kacheln/s** | Basis der grossen Welt auf 24 Threads mit Grafikkarte, gemessen an einem Ausschnitt von 65 536 Kacheln, siehe [Biomübergänge](docs/messungen/2026-09-27-biomuebergaenge.md) |
| **66 Minuten** | für eine Welt mit 2,5 Millionen Basiskacheln, ganz gemessen mit #21, mit Live-Ansicht nebenher, siehe [Vollrender mit #21](docs/messungen/2026-09-27-vollrender-mit-21.md) |
| **Byte für Byte** | dasselbe Bild auf der CPU und auf der Grafikkarte, von der CI geprüft, siehe [Grafikkarte](docs/benutzung/grafikkarte.md) |
| **verlustfrei** | WebP über libwebp, Pixel für Pixel; die grosse Welt gemessen 184 GB an Dateien, auf der Platte rund 194 GB |
| **7 Minuten** | für die ganze Testwelt bei scale 32 mit allen Stufen, hochgerechnet, rund 26 GB |

## Was drin ist

- **Wie im Spiel:** Blockstates, Modelle mit Drehung, Gewicht und `uvlock`,
  Varianten aus der Position, Biomfarben samt Übergängen, Wasser und Lava
  mit ihren Fallstufen, jeder Block im Licht des Spiels, volle Würfel weich
  beleuchtet. Das Verhalten ist am Code des Spiels belegt, siehe den
  [Wegweiser](docs/index.md).
- **Schnell:** Bitmasken statt Blockbesuche, ein Cache je Thread, Zeichnen
  auf der Grafikkarte über Vulkan oder DX12. Ohne Karte zeichnet die CPU
  dasselbe Bild.
- **Kameras:** 2:1 als Vorgabe, mit `--camera` jede Raute bis 1:1 oder
  die Draufsicht, dazu genordet von oben oder schräg, jede aus vier
  Richtungen mit `--direction`, siehe
  [Die Kamera](docs/renderer/kamera.md).
- **Für grosse Welten:** Zoomstufen darüber, native Stufen auf Wunsch,
  `--resume` nach einem Abbruch, Zusehen während eines Renders.
- **Schlankes Frontend:** Leaflet mit Vite und TypeScript. Es lädt nur
  Kacheln und `map.json`.

## Galerie

<table>
  <tr>
    <td width="50%"><img src="docs/bilder/ufer.webp" alt="Strand mit Schiffswrack, scale 32"></td>
    <td width="50%"><img src="docs/bilder/eis.webp" alt="Eisberge im gefrorenen Meer, scale 8"></td>
  </tr>
  <tr>
    <td>Strand mit Schiffswrack, scale 32</td>
    <td>Eisberge im gefrorenen Meer, scale 8</td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/bilder/savanne.webp" alt="Savanne mit Dorf und Fluss, scale 8"></td>
    <td width="50%"><img src="docs/bilder/welt.webp" alt="Die Insel um den Spawn, scale 4"></td>
  </tr>
  <tr>
    <td>Savanne mit Dorf und Fluss, scale 8</td>
    <td>Die Insel um den Spawn, scale 4</td>
  </tr>
</table>

Alle Bilder zeigen die Testwelt, Stand `05e6b16`. Ausschnitte und Befehle:
Skill [`doku-bilder-rendern`](skills/doku-bilder-rendern/SKILL.md).

## Schnellstart

Gebraucht werden Rust mit einem C-Compiler, unter Windows der von Visual
Studio, und für das Frontend Node.js; die CI nimmt Version 22.

1. **Assets besorgen:** `vanilla-assets/` und `vanilla-data/` aus dem
   Client-JAR von Minecraft 26.2, wie in
   [Assets und Biomdaten](docs/benutzung/assets.md) beschrieben. Die Welt
   liegt unter `./world`, ab Minecraft 26.1.
2. **Bauen:**

   ```bash
   cargo build --release --manifest-path renderer/Cargo.toml
   ```

3. **Die Welt als Kacheln rendern**, direkt dorthin, wo das Frontend sie
   findet:

   ```bash
   cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles web/public/tiles
   ```

4. **Ansehen:**

   ```bash
   cd web && npm install && npm run dev
   ```

   Ohne eigene Kacheln zeigt `http://localhost:5173/?tiles=/tiles-demo`
   einen kleinen Kachelbaum aus dem Repository.

Alle Schalter: [Schalter und Beispiele](docs/benutzung/schalter.md).

## Stand

- Liest Welten ab 26.1 und Resourcepacks wie der Client von 26.2.
- Truhen, Banner, Köpfe, Krüge und die übrigen Blockentities aus den
  Modellen des Spiels, mit Bannermustern und Scherben.
- Cinematic mit `--cinematic`: dieselbe Karte im Licht des Spiels in HDR,
  mit Sonne und Schatten, Wasser, Leuchten, Wärme nach Biom und Bloom,
  siehe [Cinematic](docs/renderer/cinematic.md).
- Noch nicht: Text auf Schildern und Gegenstände in Blöcken, weiche
  Beleuchtung für Flächen im Innern eines Blocks, etwa auf Schneedecken.

## Doku

Die Doku liegt in [`docs/`](docs/index.md), jede Seite im
[Wegweiser](docs/index.md): Benutzung, wie der Renderer das Spiel nachbaut,
Entscheidungen und Messungen. Regeln für alle, die hier arbeiten, stehen in
[`AGENTS.md`](AGENTS.md), die Workflows in [`skills/`](skills/). Tests und
CI: [Tests](docs/entwicklung/tests.md), [CI](docs/entwicklung/ci.md).

## Lizenz

Nutzen ja, auch für einen Minecraft-Server mit Einnahmen; verkaufen und
übernehmen nein. Die Bedingungen stehen in [`LICENSE`](LICENSE), der
Grund in [0034](docs/entscheidungen/0034-eigene-lizenz.md).

Kein offizielles Minecraft-Produkt. Nicht von Mojang oder Microsoft
genehmigt und nicht mit ihnen verbunden.
