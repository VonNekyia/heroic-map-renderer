---
title: Updates je Phase
description: Was ein Update wie im Plugin, ein Thread mit niedriger Priorität, an der Testwelt in 2:1 bei scale 32 je Phase kostet, ohne Änderung und mit 16 Änderungen an der Oberfläche, wie viele Kacheln im Gebiet neue Pixel bekommen und wie viele eine Grenze nach unten wegliesse.
date: 2026-10-24
commits: [df2d1cf]
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
  - renderer/src/render/stand.rs
---

# Updates je Phase

Ein Update mit 16 Änderungen an der Oberfläche braucht 15,1 bis 15,9 s, zu
61 % für die Basis und zu 19 % für die Pyramide. Von 1960 Kacheln im Gebiet
bekommen nur 10 neue Pixel. Eine Grenze nach unten liesse 816 davon weg,
41,6 %, und keine der weggelassenen hätte neue Pixel. Ohne Änderung braucht
ein Update 0,71 s, zwei Drittel davon für die Stempel der Regionen.

## Aufbau

- **Welt:** eine Kopie der Testwelt, 316 223 Chunks in 383 Regionsdateien,
  26.2. Das Original wurde nur gelesen.
- **Baum:** 2:1 aus `se`, scale 32, ohne native Stufen, `--gpu auto`. Der
  volle Lauf vorher mit allen Threads: 274 273 Basiskacheln, 20 GB, 264 s.
- **Updates** wie das Plugin:

  ```bash
  heroic-map-renderer --world <kopie> --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <wurzel> --scale 32 --gpu auto --threads 1 --low-priority --update
  ```

- **Stand:** master df2d1cf, Release-Build, mit Zeitstempeln an den Grenzen
  der Phasen. Diese standen nur in einem Versuch, sie ändern kein Pixel.
- **U0:** ohne Änderung.
- **U16:** 16 über die Welt verteilte, fertig erzeugte Chunks mit acht
  Nachbarn. In jedem wurde der oberste feste Block einer Spalte nahe der
  Mitte durch einen anderen ersetzt, mit neuem Stempel, direkt in der
  Regionsdatei der Kopie. Danach wurde er abwechselnd zurückgesetzt und
  wieder ersetzt. So hat jeder Lauf eine echte Änderung.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel.
- **Läufe:** je drei, 5 s Pause dazwischen.
- **Die Zeit je Phase** aus den Zeitstempeln des Versuchs.
- **Die Spitze des Arbeitsspeichers** aus `GetProcessMemoryInfo` des
  beendeten Prozesses.
- **Neue Pixel:** die Basiskacheln, die das Update geschrieben hat. Kacheln
  mit gleichen Pixeln schreibt es nicht, siehe
  [Updates](../benutzung/updates.md), „Gleiche Pixel“.
- **Grenze nach unten:**
  - Je geändertem Chunk ist E das tiefste y über alle 256 Spalten, an dem
    von oben der erste dichte Block steht. Dicht ist ein Block, der in
    jedem Zustand das Licht ganz aufhält (`licht.txt`, Stufe f).
  - K ist die Section von E vor der Änderung, `E_neu` ist E danach.
  - Für beide Grenzen, `16·K − 16` und `min(16·K, E_neu) − 16`, rechnete
    der Versuch das Gebiet mit dieser Unterkante statt der der Dimension
    und gab die Kacheln aus, die dann fehlen.
- **Ein erster Satz** traf 6 Chunks, die nicht fertig erzeugt sind; das
  Update zählte nur 10 Änderungen. Er wurde mit 16 fertigen Chunks
  wiederholt, die Zahlen unten sind von dort.

## Ergebnis

**U0,** ohne Änderung: 0,71 s, der erste Lauf kalt 1,17 s.

| Phase | s |
|---|---|
| Stempel der Regionen lesen | 0,455 |
| Stand schreiben | 0,081–0,084 |
| Grafikkarte öffnen | 0,079–0,080, kalt 0,216 |
| Assets | 0,023, kalt 0,034 |
| Prüfen, Stand lesen, Vergleich | zusammen 0,040 |

Spitze Working Set 0,14 GiB, privat 0,27 GiB.

**U16,** 16 Änderungen: 15,12 / 15,13 / 15,86 s.

| Phase | s | Anteil |
|---|---|---|
| Basis, 1960 Kacheln | 9,19–9,38 | 61 % |
| Pyramide | 2,84–3,09 | 19 % |
| Vorlauf | 1,35–1,63 | 9 % |
| Sprites | 0,66–0,70 | 4 % |
| Stempel der Regionen lesen | 0,47 | 3 % |
| Liste der Basis | 0,21–0,24 | 1,5 % |
| Grafikkarte, Vergleich, Gebiet, Hashes, `map.json`, Stand schreiben | je unter 0,1 | |

- **Spitze:** Working Set 0,34 GiB, privat 0,50 GiB.
- **Gebiet:** 1960 Basiskacheln, 122 je geändertem Chunk.
- **Neue Pixel:** 10 Basiskacheln, 0,5 % des Gebiets.
- **Grenze nach unten:**

| Grenze | Kacheln weg | davon mit neuen Pixeln |
|---|---|---|
| `16·K − 16` | 816, 41,6 % | 0 |
| `min(16·K, E_neu) − 16` | 816, 41,6 % | 0 |

- **K und E:** K lag zwischen den Sections 2 und 6, E zwischen y 37 und 110.
  Ein Block oben öffnet keinen Schacht: `E_neu` war nie tiefer als 16·K,
  darum geben beide Grenzen dieselben Kacheln.

## Schluss

- **Die Basis zeichnet fast nur Gleiches:** 1950 von 1960 Kacheln im Gebiet
  zeigen danach dieselben Pixel. Die Grenze nach unten nähme 42 % davon
  weg, ohne eine Kachel mit neuen Pixeln zu verlieren.
- **Die Pyramide** baut die Eltern aller 1960 Kacheln neu, gebraucht hätten
  es die Eltern der 10.
- **Feste Kosten** sind an der Testwelt klein: Stempel 0,46 s, Liste der
  Basis 0,22 s, Stand schreiben 0,08 s. Sie wachsen mit der Zahl der
  Regionen und Kacheln.
- **Nicht geprüft:** Änderungen, die einen Schacht öffnen oder unter K
  liegen; die Grenze mit `E_neu` ist für sie gedacht.
