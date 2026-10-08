---
title: Arbeit je Block über die nativen Stufen
description: Was der Prototyp zu #207 B1 spart und an Speicher kostet, wenn die nativen Stufen eines Bands Licht, Farben und Masken voneinander nehmen, wo ihre Tabellen dasselbe geben. Ausschnitte der Testwelt mit drei nativen Stufen, ein Thread und alle, dazu wie oft die Eingaben zwischen den Stufen abweichen.
date: 2026-10-09
commits: [54ded9d, 11b49b3, a49c0a0]
code:
  - renderer/src/render/metatile.rs
---

# Arbeit je Block über die nativen Stufen

Der Prototyp zu #207 B1 spart mit drei nativen Stufen 5 bis 7 % der
Wanduhr. Die Spitze des Speichers steigt dafür mit allen Threads von 2,5
auf 4,5 GiB. Licht und Farben je Block allein bringen auf einem Thread
nichts Messbares und auf allen −4 % für +0,8 GiB. Jedes Bild bleibt Byte
für Byte gleich.

## Aufbau

- **Stände,** Release-Build:
  - `master`, `54ded9d`;
  - `a`, `11b49b3`: Licht und Farben je Block aus `sprite_at` im Vorrat,
    für die Stufen eines Bands;
  - `b`, `a49c0a0`: `a`, dazu die Masken eines Chunks über die Stufen
    geteilt.
- **Wann ein Stand nimmt statt rechnet,** ohne die Prüfung wäre das Bild
  falsch, siehe „Diagnose“:
  - Licht und Farben nur, wenn die Tabelle der Stufe für den Block dasselbe
    gibt: Eigenschaften der AO-Karte, deckende Nachbarn vor den Seiten,
    Flüssigkeit, Tönung, Doppelkiste (`AusDerTabelle`).
  - Masken nur, wenn die Bits aus `flags` für jeden Paletteneintrag
    gleich sind (`Loaded::kennung`).
- **Welt:** die Testwelt, um `--center -64 416`, scale 32, 2:1 aus `se`,
  ohne Grafikkarte, drei native Stufen.
- **Läufe:**
  - ein Thread: `--size 8192 --native-levels 3 --threads 1`;
  - alle 24 Threads: `--size 20480 --native-levels 3`.

## Ablauf

- **Zeit und Speicher:** je Ausschnitt drei Runden, in der Folge `master`,
  `a`, `b`, dann `a`, `b`, `master`, dann `b`, `master`, `a`. Jeder Lauf
  geht in einen leeren Baum. Nach dem Lauf wird der Baum gelöscht, dann
  folgen 15 s Pause, und die Last liegt vor jedem Lauf unter 10 %.
- **Bild:** Ausschnitte mit `--size 4096`, je Stand in einen leeren Baum,
  SHA-256 jeder Kachel gegen `master`:
  - drei native Stufen mit 4 Threads und mit einem;
  - ohne native Stufen;
  - Cinematic mit drei Stufen;
  - Kamera 4:3 aus `nw` mit drei Stufen.
- **Update:** eine Welt aus der Region `r.-1.0` der Testwelt, scale 32 mit
  drei nativen Stufen.
  - Je Stand ein voller Lauf, dann drei Chunks gelöscht und ein `--update`
    mit einem Thread.
  - Danach die Bäume gegen `master` und gegen einen vollen Lauf.
- **Diagnose:** ein eigener Build von `b` mit Zählern, nicht im
  Repository, über `--size 8192` mit drei Stufen.
- **Ruhe:** die ganze Reihe unter der Sperre, ohne laufenden
  Minecraft-Client.
- **Quelle,** am 09.10. nachts: das Messskript misst Wanduhr, CPU-Zeit und
  die Spitze des Arbeitsspeichers (`PeakWorkingSetSize`) des Prozesses;
  dazu die Zähler aus der Ausgabe der Diagnose.

## Ergebnis

### Zeit und Speicher

Median aus drei Läufen, dahinter die Spanne.

| Lauf | Stand | Wanduhr | CPU | Spitze |
|---|---|---|---|---|
| ein Thread | `master` | 25,33 s (25,31–26,52) | 25,2 s | 0,20 GiB |
| ein Thread | `a` | 25,17 s (24,79–25,68) | 24,9 s | 0,25 GiB |
| ein Thread | `b` | 24,08 s (23,88–24,87) | 23,9 s | 0,29 GiB |
| alle Threads | `master` | 15,09 s (14,43–15,22) | 253,0 s | 2,49 GiB |
| alle Threads | `a` | 14,42 s (14,31–14,55) | 244,7 s | 3,32 GiB |
| alle Threads | `b` | 14,09 s (14,05–14,33) | 234,7 s | 4,45 GiB |

- **`a`:** Auf einem Thread liegt es in der Streuung. Mit allen Threads
  sind es −4,4 % Wanduhr und −3,3 % CPU. Die Spitze steigt um 25 und 33 %.
- **`b`:**
  - −4,9 % Wanduhr auf einem Thread, die Spannen getrennt;
  - −6,6 % Wanduhr und −7,2 % CPU mit allen Threads;
  - die Spitze +47 % auf einem Thread, +1,96 GiB oder +79 % mit allen.

### Bild

In allen fünf Fällen haben `a` und `b` jede Kachel Byte für Byte wie
`master`, 776, 776, 373, 776 und 546 Kacheln. Nach dem Update gleichen
beide Bäume `master` und einem vollen Lauf, 2207 Kacheln.

### Diagnose

Über alle drei nativen Stufen, 2:1 aus `se`. „Neu gerechnet“ und „ohne
Masken im Vorrat“ heisst: Keine Stufe davor im Band hatte den Block oder
Chunk.

| | neu gerechnet | genommen | andere Eingabe | davon anderer Wert |
|---|---|---|---|---|
| Licht und Farben je Block | 912 207 | 1 607 281 | 66 237 | 60 430 |

| | ohne Masken im Vorrat | geteilt | andere Bits | davon andere Masken |
|---|---|---|---|---|
| Masken je Chunk | 10 171 | 13 327 | 6 247 | 6 247 |

- **Andere Eingabe, anderer Wert:**
  - vor allem Seegras und hohes Seegras mit anderen Plätzen der AO-Karte;
  - Laub mit anderen deckenden Nachbarn;
  - Laubstreu und Wildblumen mit oder ohne AO-Karte und Tönung.
- **Andere Bits:** vor allem Laub, das bei grobem scale deckt, dazu Lava,
  Schienen, Truhen und Spawner.
- **Andere Kameras:** Cinematic zählt ähnlich. Bei 4:3 aus `nw` weichen
  nur 542 Blöcke und 162 Chunks ab.

## Schluss

- **Weit unter der Schätzung:** #207 rechnete mit 15 bis 20 % des ganzen
  Laufs. Gemessen sind 5 bis 7 %.
  - Die Masken entstehen weiter für jede Stufe neu, wo sich die Bits
    unterscheiden, und für jeden Chunk, der aus dem Vorrat fiel.
  - Die Nachschläge in der Tabelle je Block kosten selbst etwas.
- **Speicher:** Die Masken bleiben für jeden Chunk zweier Bänder im
  Vorrat liegen, neben Chunk und Licht. Mit allen Threads sind das rund
  2 GiB mehr, rund 80 %.
- **Die Prüfungen sind nötig:** Ohne sie bekämen rund 60 000 Blöcke und
  6 000 Chunks Werte einer anderen Stufe.
  - Im Bild zeigt sich das selten, denn bei den nativen Stufen von fein
    nach grob deckt eine grobe Stufe eher mehr als weniger.
  - `vorrat_gilt_nur_bei_gleicher_tabelle` in
    [`renderer/tests/metatile.rs`](../../renderer/tests/metatile.rs)
    wechselt deshalb in beiden Richtungen.
- **Entscheidung:** offen, der Maintainer entscheidet am gemessenen
  Speicher, siehe #207.
