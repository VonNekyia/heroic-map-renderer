---
title: "0023: Zeichnen auf der Grafikkarte, Byte für Byte wie die CPU"
description: Warum die Karte nur den Blit übernimmt, ganzzahlig mischt und jederzeit auf die CPU zurückfällt, und warum der Lauf die Karte vor dem Vorlauf öffnet.
status: gilt
date: 2026-09-26
issues: [11]
code:
  - renderer/src/render/gpu.rs
  - renderer/src/render/gpu.wgsl
  - renderer/src/render/metatile.rs
  - renderer/src/cli.rs
---

# 0023: Zeichnen auf der Grafikkarte, Byte für Byte wie die CPU

## Anlass

Schritt 10: Die Kacheln sollen auf der Grafikkarte gezeichnet werden, ohne
dass sich das Bild ändert und ohne dass ein Rechner ohne Karte schlechter
fährt.

## Entscheidung

Der Renderlauf stellt je Kachel dieselbe Zeichenliste auf wie für die CPU
(`draw_list`); ein Compute-Shader über wgpu setzt sie zusammen, je Zelle
von 16×16 Pixeln die Liste der Sprites in Zeichenreihenfolge, je Pixel ein
Thread. `over` rechnet ganzzahlig, auf 1/255² erweitert und einmal am
Schluss gerundet, im Shader dieselbe Rechnung wie auf der CPU. Versagt die
Karte mit einem Fehler oder einer Panik aus wgpu oder antwortet sie 60 s
nicht, zeichnet die CPU den Rest. Der Lauf öffnet die Karte vor dem
Vorlauf. Siehe [Grafikkarte](../benutzung/grafikkarte.md).

## Verworfene Alternativen

- **Gleitkomma im Shader.** Nicht über Hersteller gleich: FMA, `round` halb
  zu gerade in WGSL, Division ohne garantierte Rundung.
- **Ein Tiefenpuffer oder der feste Blender der Karte.** Sie runden je
  Hersteller anders.
- **Ein Atlas auf der Karte, den sich alle Threads teilen.** Er müsste
  seltener hochladen, bräuchte aber eine Sperre. Die Sprites eines
  Durchgangs gehen deshalb mit ihm hinauf, jedes einmal: die Kacheln einer
  Gegend brauchen ein paar hundert, die Tabelle hat zehntausende.
- **Die Karte erst nach dem Vorlauf öffnen.** Dann bräche `--gpu on` ohne
  Karte erst nach Minuten ab; später öffnen spart nichts, auch ein
  `--resume` braucht die Karte.

## Folgen

- Chunks dekodieren, Kandidaten sammeln, Sprite-Wahl und WebP bleiben auf
  der CPU; `--render` und `--pyramid` zeichnen immer dort.
- Jeder Thread hält sechzehn Zeichenlisten, an der Spitze mehr Speicher.
- Onboard-Grafik und Karten anderer Hersteller sind nicht geprüft.
- Seit [0026](0026-deckungsmaske.md) ist die CPU ohne Karte etwa so schnell
  wie mit.
