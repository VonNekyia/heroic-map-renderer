---
title: Speicher im ersten Render
description: Wo im ersten Render der Testwelt die Spitze des Arbeitsspeichers liegt, mit allen Threads und mit einem Thread wie im Plugin, wie viele Masken dann leben und wie viele ihrer Ebenen leer oder voll sind, wie viele Einträge die Pixel-Hashes halten und wie gross die Kachelmengen sind.
date: 2026-10-24
commits: [bde5637]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# Speicher im ersten Render

Im ersten Render der Testwelt liegt die Spitze des Arbeitsspeichers im
Rendern der Basis: mit allen Threads bei 3,33 GiB privat, mit einem Thread
über einen Ausschnitt bei 0,49 GiB. Zwischen dem Ende des Vorlaufs und der
ersten Kachel sind es 0,61 bis 0,66 und 0,26 bis 0,30 GiB. Mit allen
Threads leben an der Spitze rund 80 000 Masken, 0,65 GiB; 57 % ihrer
Ebenen sind ganz leer oder ganz voll, zusammen 0,32 GiB.

## Aufbau

- **Welt:** eine Kopie der Testwelt, 26.2. Das Original wurde nur gelesen.
- **Baum:** 2:1 aus `se`, scale 32, ohne native Stufen, `--gpu auto
  --low-priority`, jeweils leer vorher, also ein erster Render.
- **Läufe:**
  - **voller Lauf** mit allen Threads, wie in den letzten Messungen;
  - **Ausschnitt** mit `--threads 1`, wie das Plugin rendert: ein Quadrat
    von 49 152 px um den Ursprung der Welt, `--center 0 0 --size 49152`,
    also 192 × 192 Basiskacheln. Ein voller Lauf mit einem Thread dauerte
    gerechnet Stunden. Der Ausschnitt bleibt für spätere Vergleiche derselbe.
- **Stand:** master bde5637, Release-Build. Darauf eine Versuchsbuild, nur
  lokal, mit einem eigenen Cargo-Ziel. Sie zählt nur und ändert kein
  Pixel:
  - die Marken der Phasen;
  - alle 0,5 s die lebenden Masken (`Masks` in `metatile.rs`, je 8 760
    Bytes) und ihre 15 Ebenen aus `bits`, je 512 Bytes, nach leer, voll und
    gemischt;
  - die Einträge der Pixel-Hashes im Speicher;
  - die Grösse der Kachelmengen nach dem Vorlauf.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel. Gegen
  09:20 lief einige Sekunden ein `tsc` einer anderen Sitzung, während des
  Ausschnitts; das kann dessen Dauer um Sekunden verschieben, den Speicher
  dieses Prozesses nicht.
- **Speicher:** alle 0,5 s Working Set und privater Speicher aus
  `GetProcessMemoryInfo` des laufenden Prozesses, die Spitze dazu am Ende.
- **Phasen:** die Marken der Versuchsbuild stehen am Ende jeder Phase. Die
  Spitze je Phase ist das Höchste zwischen zwei Marken.

## Ergebnis

**Voller Lauf, alle Threads:** 277,9 s; Vorlauf 249 103 Chunks in 13,3 s;
274 273 Basiskacheln und 93 721 der Pyramide.

| Phase | bis s | privat GiB | Working Set GiB |
|---|---|---|---|
| Vorlauf | 14,7 | 0,608 | 0,264 |
| Sprites | 17,3 | 0,658 | 0,314 |
| Basis | 272,9 | 3,326 | 2,559 |
| Pyramide | 274,9 | 2,983 | 2,354 |
| Hashes, `map.json`, Stand | 277,3 | 1,587 | 1,002 |

- **Spitze:** 3,343 GiB privat, 2,583 GiB Working Set, bei 130,5 s im
  Rendern der Basis.
- **Masken an der Spitze:** 80 059, also 0,65 GiB (701 MB). Von ihren
  1 200 885 Ebenen sind 521 499 leer (43 %), 158 362 voll (13 %) und
  521 024 gemischt (43 %). Die leeren und vollen haben zusammen 348 MB,
  0,32 GiB.
- **Pixel-Hashes:** an der Spitze 161 764 Einträge, am Ende 367 994, eine
  je Kachel.
- **Kachelmengen:** 280 630 Kacheln aus dem Vorlauf; im ersten Render
  steht keine Basis da.

**Ausschnitt, ein Thread:** 308,2 s; Vorlauf 36 407 Chunks in 12,2 s;
34 445 Basiskacheln, 262 leer, und 11 587 der Pyramide.

| Phase | bis s | privat GiB | Working Set GiB |
|---|---|---|---|
| Vorlauf | 12,4 | 0,255 | 0,117 |
| Sprites | 14,0 | 0,302 | 0,157 |
| Basis | 305,3 | 0,487 | 0,336 |
| Pyramide | 307,7 | 0,472 | 0,312 |

- **Spitze:** 0,489 GiB privat, 0,337 GiB Working Set, bei 41,4 s im
  Rendern der Basis.
- **Masken:** an der Spitze 2 247, also 20 MB, höchstens 4 449, 39 MB; leer
  oder voll sind 61 % ihrer Ebenen.
- **Pixel-Hashes:** höchstens 46 032 Einträge.

## Schluss

- **Die Spitze** liegt in beiden Läufen im Rendern der Basis, nicht
  zwischen Vorlauf und erster Kachel. Auf der grossen Welt mit einem Thread
  wachsen aber die Teile je Welt: Stand, Inhalte des Vorlaufs,
  Kachelmengen, Pixel-Hashes. Dort ist es nicht gemessen.
- **Masken teilen** lohnt sich mit vielen Threads: Wären leere und volle
  Ebenen geteilt statt je 512 Bytes, fielen an der Spitze mit allen Threads
  gemessen 0,32 GiB weg, rund 10 % der Spitze. Mit einem Thread sind es
  rund 10 MB.
- **Die Pixel-Hashes** halten am Ende eine Zeile je Kachel, an der
  Testwelt 367 994. Mit rund 48 Bytes je Eintrag in der `HashMap` sind das
  rund 18 MB, auf einer neunmal grösseren Welt gerechnet rund 160 MB.
- **Die Kachelmengen** sind an der Testwelt klein, 280 630 Kacheln aus dem
  Vorlauf; im ersten Render ist die Basis leer.
