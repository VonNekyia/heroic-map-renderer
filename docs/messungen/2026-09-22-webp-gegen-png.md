---
title: Erster Export, WebP gegen PNG
description: Grösse eines Bilds als PNG gegen die Kacheln daneben als verlustfreies WebP und die ersten hochgerechneten Grössen für die ganze Testwelt, gemessen mit Schritt 5.
date: 2026-09-22
commits: [41e2537]
code:
  - renderer/src/render/tiles.rs
---

# Erster Export, WebP gegen PNG

Ein Bild von 256 × 256 Pixeln wog als PNG 173 kB; die 72 Kacheln um
denselben Punkt wogen als verlustfreies WebP 108 bis 165 kB, im Mittel
137 kB, gut ein Fünftel weniger. Dieselbe Kachel in beiden Formaten ist
nicht gemessen. Der Grund für
[0004](../entscheidungen/0004-webp-verlustfrei.md). Die Zahlen sind
überholt: Seit libwebp packt, siehe
[2026-09-27, libwebp](2026-09-27-libwebp.md), wiegt eine Kachel ein Drittel.

## Aufbau

- Welt: die Testwelt, scale 16, der damalige Standard.
- Stand: Schritt 5 (`41e2537`), WebP mit dem Encoder aus `image`.
- Testausschnitt: `--center -64 416 --size 2048`, 72 Kacheln, so im README
  seit `41e2537`. Das PNG ist `--render --size 256` um denselben Punkt; es
  deckt sich mit keiner Kachel genau.

## Ablauf

Den Testausschnitt als Kacheln geschrieben und das Bild als PNG; die
Grössen der beiden stammen aus den Dateien, dezimal gezählt. Ein
Vollrender der Testwelt bei scale 16 lief bis 6746 Kacheln und 801 MiB,
dann wurde er abgebrochen und hochgerechnet. Der Vorlauf lief über die
ganze Welt bei scale 16 und 8. Die Werte je Kachel stammen aus der Ausgabe,
die MiB des Vollrenders aus `du`, beide binär.

## Ergebnis

| | Grösse |
|---|---|
| Bild aus `--render`, 256 × 256 Pixel, PNG | 173 kB |
| die 72 Kacheln um denselben Punkt, WebP verlustfrei | 108 bis 165 kB, im Mittel 137 kB (134 KiB) |
| der abgebrochene Vollrender, je Kachel | 122 KiB |

| `--scale` | Vorlauf, gemessen | Kacheln, gemessen | je Kachel | ganze Welt, hochgerechnet |
|---|---|---|---|---|
| 16 | 10,7 s | 73 920 | 134 KiB | ~9 GiB |
| 8 | 6,4 s | 18 951 | 137 KiB | ~2,5 GiB |

Die 20 bis 40 Prozent Ersparnis gegenüber PNG standen schon in `41e2537`
im README. Gegen die kleinste Kachel sind es 38 %, gegen das Mittel 21 %.

Der Vorlauf über die ganze Welt mit 316 223 Chunks brauchte 10,7 s,
gegenüber 80,8 s für denselben Scan einkernig in Schritt 1.

## Schluss

Verlustfreies WebP war auf diesem Inhalt im Mittel gut ein Fünftel kleiner
als PNG, gemessen an einem Bild gegen die Kacheln daneben, nicht an
derselben Kachel. Die Grösse einer Kachel hängt kaum am scale: eine Kachel
ist immer 256 × 256 Pixel, und ihr Inhalt ist bei scale 8 so dicht wie
bei 16. Der scale wirkt über die Kachelzahl.
