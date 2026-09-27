---
title: Vollrender der grossen Welt mit #11
description: Der erste ganz gemessene Vollrender der grossen Welt, mit dem Stand von #11, Grafikkarte und dem alten WebP-Encoder - Dauer, Kacheln und Grösse.
date: 2026-09-26
commits: [6f0e365]
code:
  - renderer/src/cli.rs
  - renderer/src/render/tiles.rs
---

# Vollrender der grossen Welt mit #11

Der ganze Lauf brauchte 68 Minuten und schrieb 3,33 Millionen Kacheln,
354 GB: 266 GB Basis, 88 GB Pyramide. Die Grösse kam vom einfachen Encoder
aus `image`; daraus wurde #13, siehe
[0028](../entscheidungen/0028-libwebp-statt-image.md). Spätere Stände sind
nur hochgerechnet, siehe [Was ein Lauf kostet](../benutzung/kosten.md),
„Die grosse Welt“.

## Aufbau

- Welt: die grosse Welt, 26.2, scale 32, ohne native Stufen.
- Stand: das Binär von #11 (`6f0e365`), `--gpu auto` mit einer
  eigenständigen Karte über Vulkan, WebP noch mit dem Encoder aus `image`.
- Nebenher lief eine Live-Ansicht: alle fünf Minuten `--pyramid` über
  denselben Baum.
- Die Ausnahme vom Echtzeitschutz kam erst mitten im Lauf dazu.

## Ablauf

Ein Lauf über die ganze Welt, in einem Stück, gemessen von aussen.

## Ergebnis

| | Wert |
|---|---|
| ganzer Lauf | 68 min |
| Vorlauf | 64 s |
| Basis | 2 496 892 Kacheln, davon 7 569 leer, in 44 min |
| Pyramide | 835 252 Kacheln in 23 min, Zoom 0 bis 11 |
| Grösse | 354 GB: Basis 266 GB, Pyramide 88 GB |

- Solange ein Aufruf von `--pyramid` nebenher lief, halbierte sich das
  Tempo etwa, und die Aufrufe wurden mit der Basis länger: 7, 10 und 13 min.
- Ohne Nebenlauf stieg die Rate mit der Ausnahme vom Echtzeitschutz von 891
  auf 1593 Kacheln/s.
- Ohne Live-Ansicht und mit der Ausnahme von Anfang an wären es geschätzt
  etwa 50 Minuten gewesen; gemessen ist das nicht.

## Schluss

Das ist der einzige ganz gemessene Vollrender der grossen Welt. Grösse und
Dauer der Stände danach, libwebp, Wasser im Licht und weiche Beleuchtung,
sind aus Ausschnitten hochgerechnet.
