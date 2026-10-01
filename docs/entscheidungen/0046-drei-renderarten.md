---
title: "0046: Drei Renderarten, zwei Backends, ein Kern"
description: Warum die Karte beim Raster bleibt, Cinematic und Showcase ein gemeinsames Strahlen-Backend bekommen, Cinematic nur das Licht des Spiels nimmt und beide Backends Welt, Modelle, Biomfarben und Licht aus einem Kern nehmen.
status: gilt
date: 2026-10-01
issues: [72, 73, 74]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/projection.rs
  - renderer/src/render/licht.rs
  - renderer/src/render/tint.rs
---

# 0046: Drei Renderarten, zwei Backends, ein Kern

## Anlass

Neben der Karte sollen Einzelbilder der Welt entstehen, in zwei Stufen:
**Cinematic** mit Sonne und Schatten, Himmel, leuchtenden Blöcken, Nebel
und Wasser, schnell genug für viele Bilder, und **Showcase** mit
Pfadverfolgung, Spiegelung und Brechung an Wasser, Glas und Eis und
Perspektive, für wenige Bilder. Die Karte darf dabei nichts verlieren.
Entschieden hat der User am 01.10., nach einer Vorschau aller drei Arten
aus einem Prototyp.

## Entscheidung

- **Karte:** Das Raster-Backend bleibt, wie es ist. Es bleibt Byte für Byte
  gleich und wird nicht langsamer.
- **Cinematic und Showcase:** ein neues Strahlen-Backend. Beide sind Stufen
  desselben Codes und unterscheiden sich in Reglern: Strahlen je Pixel,
  Licht, welche Materialien spiegeln und brechen, Kamera.
- **Cinematic nimmt nur das Licht des Spiels:** Himmels- und Blocklicht aus
  `licht.rs`, zwischen den Zellen weich gemischt. Dazu die Sonne mit einem
  Schattenstrahl, Himmel und Nebel in den Farben aus den Attributen der
  Biome (`minecraft:visual/sky_color`, `fog_color`, `water_fog_color`),
  leuchtende Blöcke, Wasser nach seiner Tiefe und Tonemapping. Keine
  Strahlen zum Himmel, kein Bounce: So bleibt Cinematic schnell und
  schlank.
- **Showcase** verfolgt Pfade: Sonne, Himmel und leuchtende Blöcke über die
  Pfade, Spiegelung und Brechung an Wasser, Glas und Eis, Perspektive.
- **Ein Kern für beide Backends:**
  - Welt mit Chunk-Cache, Familien und Varianten
  - Assets mit den gebackenen Flächen (`BakedModel`)
  - Biomfarben und das Licht des Spiels (`licht.rs`)
  - die Regeln der Flüssigkeit
  - die allgemeine Kamera
  - Was es davon gibt, bleibt, wo es ist; das Strahlen-Backend greift
    darauf zu. Herausgelöst wird nur, was beide brauchen und heute im
    Raster steckt. Umgezogen wird erst, wenn ein zweiter Nutzer es
    wirklich braucht.
- **Neu kommen dazu:**
  - das Material je Fläche aus einer Quelle, als Funktionen, die das
    Raster mit aufruft
  - eine Szene mit Strahlenabfrage durch die Blockwelt: Gang durchs
    Blockgitter, dann die Flächen der Modelle mit Alpha-Test
  - Strahlen aus der Kamera
- **Erst die CPU** mit `rayon`. Die Grafikkarte nur, wenn eine Messung es
  verlangt.
- **Keine neue Abhängigkeit,** kein Entrauscher aus einer Bibliothek, kein
  EXR, keine PBR-Packs in der ersten Fassung.
- **Im Frontend vorerst nur die Karte.** Einzelne Bilder kommen später dazu.

Belegt im Prototyp: Über Strahlen liefert derselbe Kern ohne Licht dasselbe
Bild wie die Karte, an drei Ausschnitten der Testwelt zu 99,91 bis
99,99 % Byte für Byte. Der Rest ist erklärt: Rundung der Tönungskarte,
Kanten auf Pixelmitten, Sonderfälle der Flüssigkeit, siehe
[Strahlen durch die Blockwelt](../messungen/2026-10-01-strahlen-prototyp.md).
Was ein Bild in Cinematic kostet, steht in
[Cinematic, Zeit je Bild](../messungen/2026-10-01-cinematic-zeit.md).

## Verworfene Alternativen

- **Cinematic im Raster:** Schatten, Himmelslicht und Bounce als Zusätze im
  Raster, etwa über Schattenkarten oder AO im Bildraum.
  - Das Raster kennt keine Szene: Es zeichnet vorab gerasterte Sprites in
    Malerreihenfolge. Schatten und AO fragen aber, was in einer anderen
    Richtung liegt.
  - Im Bildraum gerechnet, endet jede Wirkung an der Kachelkante und an
    allem, was die Kamera nicht sieht. Eine Schattenkarte für eine Welt aus
    hunderttausenden Kacheln wäre ein eigenes Raster neben dem ersten.
  - Jede Wirkung säße in den heissen Schleifen der Karte und gefährdete
    ihr Tempo und ihr Goldbild.
- **Himmelslicht in Cinematic mit Strahlen:** Je Pixel Strahlen zum Himmel
  geben weichere Verdeckung und Kontaktschatten an Ecken, unter Bäumen und
  Dächern. Sie kosten aber ein Vielfaches der Zeit und rauschen bei wenigen
  Strahlen, siehe die Messung oben. Der User hat sich nach der Vorschau für
  das Licht des Spiels entschieden. Showcase rechnet den Himmel ohnehin
  über die Pfade.
- **Zwei Backends für Cinematic und Showcase:** doppelte Pflege für dieselbe
  Frage, was ein Strahl trifft.
- **Zuerst die Grafikkarte:** neue Abhängigkeiten und ein zweiter Codepfad,
  bevor eine Messung zeigt, dass die CPU nicht reicht.
- **Eine fremde Engine, in die exportiert wird:** eine neue Abhängigkeit,
  ein Austauschformat, und Modelle, Biomfarben und Licht des Spiels gingen
  auf dem Weg verloren.

## Folgen

- Das Goldbild der Karte bleibt. Was aus dem Raster herausgelöst wird, wird
  bytegleich und mit einer Messung des Tempos geprüft.
- Cinematic zeigt die Verdeckung des Himmels nur so fein wie das Licht des
  Spiels: In Gassen, an Ecken und unter Bäumen bleibt es heller als mit
  Strahlen. Wer das will, nimmt Showcase.
- Regel 22 in `AGENTS.md` fragt nach allen drei Renderarten statt nach
  Rastertiles, und was nur die Bilder brauchen, macht die Karte weder
  langsamer noch anders.
- Neue Seiten in `docs/renderer/` für Kern, Material, Szene und die beiden
  Stufen kommen mit ihrer Umsetzung.
- Das Frontend zeigt vorerst nur die Karte; die Bilder bekommen ein eigenes
  Issue, wenn sie dran sind.
