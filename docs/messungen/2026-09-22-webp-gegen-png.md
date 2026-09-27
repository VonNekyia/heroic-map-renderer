---
title: Erster Export, WebP gegen PNG
description: Grösse einer Kachel als PNG und als verlustfreies WebP und die ersten hochgerechneten Grössen für die ganze Testwelt, gemessen mit Schritt 5.
date: 2026-09-22
commits: [41e2537]
code:
  - renderer/src/render/tiles.rs
---

# Erster Export, WebP gegen PNG

Dieselbe Kachel wog als PNG 173 kB, als verlustfreies WebP 108 kB, also 38 %
weniger. Der Grund für [0004](../entscheidungen/0004-webp-verlustfrei.md).
Die Zahlen sind überholt: Seit libwebp packt, siehe
[2026-09-27, libwebp](2026-09-27-libwebp.md), wiegt eine Kachel ein Drittel.

## Aufbau

- Welt: die Testwelt, scale 16, der damalige Standard.
- Stand: Schritt 5 (`41e2537`), WebP mit dem Encoder aus `image`.
- Welche Kachel verglichen wurde, ist nicht festgehalten, ebenso der
  Testausschnitt für die Grösse je Kachel.

## Ablauf

Eine Kachel als PNG und als WebP geschrieben. Ein Vollrender der Testwelt
bei scale 16 lief bis 6746 Kacheln und 801 MB, dann wurde er abgebrochen und
hochgerechnet. Der Vorlauf lief über die ganze Welt bei scale 16 und 8.

## Ergebnis

| | Grösse |
|---|---|
| eine Kachel als PNG | 173 kB |
| dieselbe als WebP, verlustfrei | 108 kB |
| Testausschnitt, je Kachel | 121 bis 134 kB |

| `--scale` | Vorlauf, gemessen | Kacheln, gemessen | je Kachel | ganze Welt, hochgerechnet |
|---|---|---|---|---|
| 16 | 10,7 s | 73 920 | 134 kB | ~9 GB |
| 8 | 6,4 s | 18 951 | 137 kB | ~2,5 GB |

Das README nannte später dazu 20 bis 40 Prozent Ersparnis gegenüber PNG;
woher die Spanne stammt, ist nicht festgehalten.

Der Vorlauf über die ganze Welt mit 316 223 Chunks brauchte 10,7 s,
gegenüber 80,8 s für denselben Scan einkernig in Schritt 1.

## Schluss

Verlustfreies WebP spart gegenüber PNG gut ein Drittel. Die Grösse einer
Kachel hängt kaum am scale: eine Kachel ist immer 256 × 256 Pixel, und ihr
Inhalt ist bei scale 8 so dicht wie bei 16. Der scale wirkt über die
Kachelzahl.
