---
title: "0148: Entpacken mit fdeflate"
description: Warum der Renderer Chunks mit zlib per fdeflate statt flate2 entpackt, mit einer Obergrenze von 64 MiB, warum nicht zlib-rs, und was gleich bleibt.
status: gilt
date: 2026-11-21
issues: []
code:
  - renderer/src/world/region.rs
  - renderer/Cargo.toml
---

# 0148: Entpacken mit fdeflate

## Anlass

Im Profil von `--flat` vom 18.11. kostete das Entpacken der Chunks mit
einem Kern 11,4 % der Zeit. Die Messung vom 14.11.,
[Entpacken und Höhen mit zlib-rs und fdeflate](../messungen/2026-11-14-entpacken-und-hoehen.md),
hatte `fdeflate` als schnellste Wahl gezeigt. Der User gab die Änderung am
21.11. frei.

## Entscheidung

- **Das Verfahren 2, zlib,** entpackt `fdeflate::decompress_to_vec_bounded`
  statt `flate2` mit `miniz_oxide`. gzip und LZ4 bleiben, wie sie sind.
- **Höchstens 64 MiB** entpackt; ein Chunk des Spiels hat wenige MB. Mehr
  ist ein Fehler. Die Grenze kostet keine messbare Zeit.
- **Gleiche Ausgabe:** Byte für Byte dieselben Daten wie `flate2`; kaputte
  Daten sind ein Fehler wie vorher.
- **Abhängigkeit (Regel 22):** `fdeflate` steckt schon über `png` im Baum
  und im Binär. Neu ist nur die Zeile in `Cargo.toml`, die es direkt nennt.
  `THIRD-PARTY-NOTICES` bleibt Byte für Byte gleich.

## Verworfene Alternativen

- **`flate2` mit `zlib-rs`:** wäre ein neues Crate im Baum und brächte nach
  der Messung vom 14.11. rund die Hälfte des Gewinns.
- **Ohne Obergrenze,** wie `flate2` bisher: Kaputte Daten könnten den
  Speicher füllen.
- **Auch gzip mit `fdeflate`:** Es entpackt zlib, nicht gzip.

## Folgen

- Das Entpacken braucht in der CI an 2048 Chunks der Testwelt 34 % weniger
  Zeit unter Linux und 39 % unter Windows, siehe
  [Welten und Kennung](../benutzung/welten.md), „Kompressionsverfahren“.
- Kein Zeichenstand und kein Kodierstand steigt, keine Kachel ändert sich.
