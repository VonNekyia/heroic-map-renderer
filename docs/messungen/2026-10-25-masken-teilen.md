---
title: Masken teilen leere und volle Ebenen
description: Was R3 im ersten Render der Testwelt an Spitze des Arbeitsspeichers spart und ob der volle Lauf langsamer wird, in vier Reihen gegen master, mit allen Threads und in Reihe 1 auch mit einem Thread über den Ausschnitt aus der Messung zum Speicher im ersten Render.
date: 2026-10-25
commits: [0daca42, 4f6e89c, d079f43]
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/metatile/strahl.rs
---

# Masken teilen leere und volle Ebenen

Teilen die Masken ihre leeren und vollen Ebenen, liegt die Spitze im ersten
Render der Testwelt mit allen Threads 0,41 GiB tiefer, 2,99 statt 3,41 GiB,
−12 %. Die Zeit des vollen Laufs bleibt in der Streuung: In der ungestörten
Reihe 4 sind es 245,2 gegen 242,0 s, +1,3 %, bei einer Streuung von A von
4,7 s.

## Aufbau

- **Welt:** eine Kopie der Testwelt, 26.2, wie in
  [2026-10-24, Speicher im ersten Render](2026-10-24-speicher-im-ersten-render.md).
- **Baum:** 2:1 aus `se`, scale 32, ohne native Stufen, `--gpu auto
  --low-priority`, jeder Lauf in einen frischen Baum, also ein erster
  Render: 274 273 Basiskacheln und 93 721 der Pyramide.
- **Stände**, Release-Builds mit eigenem Cargo-Ziel:
  - A: master `0daca42`.
  - B: `4f6e89c`, die Masken geteilt (`Masks::ebene`, `Masks::packe`);
    `d079f43` dazu die Ebenen einmal je Section statt je Spalte geholt.
- **Läufe:** der volle Lauf mit allen Threads; in Reihe 1 dazu der
  Ausschnitt mit `--threads 1`, `--center 0 0 --size 49152`, wie in der
  Messung zum Speicher.

## Ablauf

- **Ruhe:** unter der Sperre `messung.lock`, ohne laufendes Spiel, ohne
  Server des Spiegels.
- **Zeit** von aussen, die **Spitze** des privaten Speichers und des
  Working Sets aus `GetProcessMemoryInfo` des beendeten Prozesses.
- **Reihe 1,** 24.10. vormittags: A, B (`4f6e89c`), B, A, dann der
  Ausschnitt mit A und B.
- **Reihe 2,** 24.10. nachmittags, verworfen: A war aus Versehen `4f6e89c`,
  nicht master, und die Maschine war belastet, die Läufe dauerten 400 bis
  510 s.
- **Reihe 3,** 24.10. 22:00, gestört: A, B (`d079f43`), B, A, A, B. Der
  letzte Lauf dauerte 382 s statt rund 250 s; die Ursache blieb offen, und
  die Zeilen zur Last vor dem Start gingen verloren.
- **Reihe 4,** 25.10. 01:31 bis 01:59: wie Reihe 3. Vor jedem Lauf die
  Lastprüfung: die Auslastung der CPU dreimal im Abstand von 2 s und die
  Prozesse mit der meisten CPU-Zeit, im Protokoll, bevor der Lauf begann;
  über 10 % hätte er gewartet. Sie lag vor dem Fenster bei 3 bis 10 %, vor
  den Läufen bei 0 bis 5 %.

## Ergebnis

**Reihe 4,** alle Threads, ungestört:

| Stand | Zeit | Mittel | Spitze privat | Spitze Working Set |
|---|---|---|---|---|
| A | 239,4 / 242,4 / 244,1 s | 242,0 s | 3,40 / 3,46 / 3,35 GiB | 2,64 / 2,67 / 2,59 GiB |
| B | 243,5 / 244,8 / 247,3 s | 245,2 s | 2,98 / 3,01 / 2,98 GiB | 2,25 / 2,27 / 2,23 GiB |

- **Zeit:** B +3,2 s, +1,3 %. Die Streuung von A ist 4,7 s, die von B 3,8 s.
- **Spitze:** privat −0,41 GiB, im Mittel 2,991 gegen 3,405 GiB; Working Set
  −0,38 GiB.

**Reihe 3,** gestört, die ersten fünf Läufe:

| Stand | Zeit | Spitze privat |
|---|---|---|
| A | 244,1 / 252,6 / 261,4 s | 3,38 / 3,39 / 3,39 GiB |
| B | 250,6 / 253,2 s, der dritte 382,4 s | 3,00 / 2,96 / 2,98 GiB |

**Reihe 1,** B noch ohne das Holen je Section:

| Stand | voll, Zeit | voll, Spitze privat | Ausschnitt, Zeit | Ausschnitt, Spitze privat |
|---|---|---|---|---|
| A | 267,0 / 282,6 s | 3,40 / 3,40 GiB | 288,8 s | 0,492 GiB |
| B | 282,0 / 293,3 s | 2,99 / 2,97 GiB | 295,7 s | 0,476 GiB |

## Schluss

- **Der Speicher** fällt in jeder Reihe um 0,41 bis 0,42 GiB an der Spitze
  mit allen Threads, rund 12 %. Das ist mehr als die 0,32 GiB leerer und
  voller Ebenen an der Spitze in der Messung zum Speicher; woher der Rest
  kommt, ist nicht gemessen.
- **Mit einem Thread** sind es 16 MB, in einem Lauf je Stand.
- **Die Zeit:** In Reihe 1 lag B ohne das Holen je Section 4 bis 6 % hinten,
  verdächtig, aber in je zwei Läufen. Mit dem Holen je Section liegen Reihe
  3 und 4 innerhalb der Streuung: −0,3 % in den ungestörten Läufen von
  Reihe 3, +1,3 % in Reihe 4.
