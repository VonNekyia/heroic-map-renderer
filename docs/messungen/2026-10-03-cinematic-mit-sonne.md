---
title: Cinematic mit Sonne
description: Was Cinematic aus #73, mit Sonne, Schatten, Wasser, Leuchten, Wärme und Bloom, an Stand und Fichtenwald gegen die Karte kostet, dass die Karte Byte für Byte gleich bleibt, und was ein Strahl zur Sonne am Renderer vor und nach dem schnellen Gang kostet, mit dem Vergleich zu Gang 12 aus 0056.
date: 2026-10-03
commits: [2f3ef45, cacdad5, dfd6ed5]
code:
  - renderer/src/render/metatile/strahl.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/kino.rs
  - renderer/src/render/sonne.rs
  - renderer/src/render/tiles.rs
---

# Cinematic mit Sonne

Cinematic aus #73 kostet an Stand und Fichtenwald das 2,15- bis
3,63-Fache der Karte im ganzen Lauf: die Basis das 2,69- bis 4,72-Fache,
die nativen Stufen das 2,11- bis 3,18-Fache. Die Spitze des Speichers liegt
27 bis 35 % höher, die Kacheln wiegen 2 % weniger bis 5 % mehr. Die Karte
bleibt Byte für Byte gleich wie auf master, ihre Zeit liegt um ±3 % dabei.
Ein Strahl zur Sonne kostet allein wiederholt 306 bis 345 ns, vor dem
schnellen Gang 519 bis 609 ns. Das ist das 1,22- bis 1,36-Fache von Gang 12
am Prototyp aus 0056. Je Bild kostet die Sonne aber 8 bis 13 % weniger als
dort, denn der Renderer schiesst 27 bis 36 % weniger Strahlen.

## Aufbau

- **Stände,** Release-Build:
  - **B:** master `2f3ef45`, mit Cinematic aus #72, ohne Sonne;
  - **C:** `dfd6ed5`, #73 nach der ersten Runde des Reviews von #104;
  - **alt:** `cacdad5`, #73 vor dem schnellen Gang, nur für Teil C.
- **Die Testwelt,** 2:1 bei scale 32 aus `se`, `--native-levels 3`,
  Pyramide, 24 Threads, `--gpu off`; dieselben Ausschnitte wie in
  [2026-10-03, Cinematic, Phase 1](2026-10-03-cinematic-phase-1.md):
  - **Stand:** um (-64, 416) mit `--size 18432`, 6400 Basiskacheln;
  - **Fichtenwald:** um (-2712, -3297) mit `--size 8192`, 1600
    Basiskacheln.
- **Serien:**
  - **Gleich:** die Karte mit B und C je ein Lauf, alle Kacheln per
    SHA-256 verglichen.
  - **Reihe:** Karte B, Karte C und Cinematic C im Wechsel, drei Runden,
    die Reihenfolge in Runde 2 umgekehrt. Cinematic aus #72 ist in Phase 1
    gemessen und fehlt hier.
- **Teil C, je Strahl:** wie Teil C in
  [2026-10-02, Gang zur Sonne in Stufen](2026-10-02-gang-zur-sonne-in-stufen.md),
  mit einem Test, der nur für die Messung entstand und nicht eingecheckt
  ist. Dazu zählt der Renderer in dieser Fassung die Strahlen eines
  Threads und nimmt sie auf.
  - Je Ausschnitt ein Bild aus 1600 × 1600 Pixeln, 2:1 bei scale 32 aus
    `se`, mit `LOOK`, um die Mitten wie am Prototyp: Dorf (-414, 516),
    Hügel (556, 876), Stand (-135, 345).
  - Ein Thread. Je Prozess zwei Durchgänge zum Warmlaufen, dann acht im
    Wechsel ohne Sonne (`sonne` 0) und mit, dann die Strahlen eines
    Durchgangs mit Sonne fünf Runden allein wiederholt, mit warmem Cache.
  - Je Ausschnitt sechs Prozesse, alt und C im Wechsel ABBAAB.
- **Nach jedem Lauf** wird sein Baum gelöscht und dann 15 s gewartet, siehe
  [2026-10-02, Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md).
  Jeder Lauf ging in eine frische Wurzel, die vom Echtzeitschutz
  ausgenommen war.
- **Last:** vor jedem Lauf und Prozess über 5 s im Mittel 0,6 bis 6,0 %,
  keiner musste warten. Vor und nach jeder Serie über 20 s: Gleich 3,4 und
  3,5 %, Reihe 1,8 und 2,1 %, Teil C 2,7 und 4,9 %. Während der Messung
  lief kein Build und kein Test, die Sperre lag.
- **Werte aus dem Prototyp** zu #89, auf dem Stand `89b667b`, an dem 0058
  abgestimmt ist. Sie stehen in `LOOK` und kommen nicht aus 0058:
  - **Leuchten:** weich ab 0,25 bis 0,75 im hellsten Kanal, linear,
    gerechnet als `(m − 0,25) / 0,5`: `leuchten_ab`, `leuchten_voll`.
  - **Wasser:** die Textur mit 0,6 ihrer Deckkraft (`wasser_textur`); in
    der Spiegelung vom Nebel zum Himmel als `(y + 0,1) / 0,7`
    (`wasser_horizont`, `wasser_horizont_breite`); die Dichte je Kanal
    `−ln(max(Anteil, 0,02)) + 0,35` (`wasser_anteil_min`,
    `wasser_dichte_grund`).
  - **Weite des Strahls:** 128 Blöcke (`sonne_weite`), siehe
    [2026-10-02, Gang zur Sonne in Stufen](2026-10-02-gang-zur-sonne-in-stufen.md).

## Ablauf

Am 03.10. von 07:17 bis 07:42 Uhr, mit einem Skript, das Sperre, Last,
Läufe und Auswertung übernahm: Gleich bis 07:21, die Reihe bis 07:34, dann
Teil C.

- **Aus der Ausgabe der Läufe,** auf 0,1 s genau: die Basis, die nativen
  Stufen und die Pyramide; im Fichtenwald sind 0,1 s bei der Karte 6 %.
- **Vom Messskript:** die Dauer des ganzen Laufs, die Spitze des
  Speichers aus `PeakWorkingSetSize`, die Kacheln und ihre Bytes.
- **Teil C** aus den Zeilen `Durchgang:` und `Wiederholt:` des Tests. Je
  Prozess der Median der fünf Runden; je Stand der Median der drei
  Prozesse. „Im Wechsel“ ist der Median mit Sonne weniger dem ohne,
  geteilt durch die Strahlen des Bilds.

## Ergebnis

### Gleich

Am Stand 8564 Kacheln, im Fichtenwald 2144, mit B und C Byte für Byte
gleich.

### Reihe

Median der drei Runden, Spannweite in Klammern:

| Ausschnitt | Art | Basis | native Stufen | ganzer Lauf | Spitze | Kacheln | davon Basis |
|---|---|---|---|---|---|---|---|
| Stand | Karte B | 4,5 s (4,5–4,5) | 4,8 s (4,8–4,9) | 10,57 s (10,49–10,71) | 2,40 GiB (2,39–2,43) | 586,7 MB | 439,1 MB |
| | Karte C | 4,6 s (4,5–4,6) | 4,9 s (4,9–5,0) | 10,75 s (10,75–10,79) | 2,39 GiB (2,38–2,40) | 586,7 MB | 439,1 MB |
| | Cinematic C | 21,7 s (20,9–21,9) | 15,6 s (15,6–15,7) | 38,97 s (38,17–39,16) | 3,23 GiB (3,21–3,26) | 574,2 MB | 428,1 MB |
| Fichtenwald | Karte B | 1,6 s (1,6–1,7) | 1,9 s (1,9–2,0) | 4,42 s (4,36–4,46) | 2,19 GiB (2,18–2,19) | 202,3 MB | 155,9 MB |
| | Karte C | 1,6 s (1,6–1,6) | 1,9 s (1,9–2,0) | 4,32 s (4,26–4,37) | 2,14 GiB (2,12–2,15) | 202,3 MB | 155,9 MB |
| | Cinematic C | 4,3 s (4,3–4,4) | 4,0 s (3,9–4,1) | 9,29 s (9,16–9,42) | 2,72 GiB (2,71–2,73) | 212,8 MB | 162,8 MB |

Die Pyramide brauchte überall 0,0 bis 0,1 s.

Verhältnisse je Runde, im ganzen Lauf:

| Ausschnitt | Karte C gegen Karte B | Cinematic C gegen Karte C |
|---|---|---|
| Stand | 1,029; 1,004; 1,017 | 3,63; 3,55; 3,63 |
| Fichtenwald | 0,989; 0,977; 0,969 | 2,10; 2,21; 2,15 |

Cinematic C gegen Karte C, Median der Runden: am Stand die Basis 4,72, die
nativen Stufen 3,18; im Fichtenwald die Basis 2,69, die nativen Stufen
2,11. Die Bytes: am Stand 0,979, im Fichtenwald 1,052.

### Je Strahl

Ns je Strahl, ein Thread, Median der drei Prozesse; allein wiederholt mit
der Spannweite aller Runden, im Wechsel mit den Werten der Prozesse:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| alt, allein wiederholt | 609 (603–618) | 553 (549–562) | 519 (512–534) |
| C, allein wiederholt | 345 (344–350) | 315 (312–322) | 306 (299–310) |
| C gegen alt | 0,57 | 0,57 | 0,59 |
| alt, im Wechsel | 641 (641; 641; 649) | 591 (591; 590; 591) | 561 (561; 553; 563) |
| C, im Wechsel | 371 (368; 371; 374) | 333 (333; 336; 332) | 328 (328; 333; 320) |
| Strahlen je Bild, C | 2 452 447 | 1 640 078 | 1 651 230 |

Ein Bild ohne Sonne brauchte mit C im Dorf 0,225 s, am Hügel 0,117 s, am
Stand 0,164 s; mit Sonne 1,135, 0,664 und 0,705 s. Mit alt zählte das Bild
im Dorf 9 und am Stand 4487 Strahlen weniger, am Hügel gleich viele: Die
Befunde aus dem Review von #104 ändern die Bilder an wenigen Pixeln.

### Gegen Gang 12

Gang 12 kommt aus einer anderen Reihe, Teil C vom 02.10. am Prototyp, siehe
[2026-10-02, Gang zur Sonne in Stufen](2026-10-02-gang-zur-sonne-in-stufen.md).
Das Verfahren ist dasselbe, Tag und Rechner im Zustand nicht. Die Zeit je
Bild ist gerechnet, ns je Strahl mal Strahlen je Bild, nicht gemessen:

| | Dorf | Hügel | Stand |
|---|---|---|---|
| Gang 12, ns je Strahl | 254 | 244 | 251 |
| C, ns je Strahl | 345 | 315 | 306 |
| C gegen Gang 12 je Strahl | 1,36 | 1,29 | 1,22 |
| Gang 12, Strahlen je Bild | 3 836 791 | 2 291 045 | 2 265 925 |
| C, Strahlen je Bild | 2 452 447 | 1 640 078 | 1 651 230 |
| Gang 12, Sonne je Bild | 0,97 s | 0,56 s | 0,57 s |
| C, Sonne je Bild | 0,85 s | 0,52 s | 0,50 s |
| C gegen Gang 12 je Bild | 0,87 | 0,92 | 0,89 |

### Proben während der Arbeit

Ohne Sperre und nicht als Messung gezählt, nur zur Suche, am Hügel:

- **Das Profil vor dem schnellen Gang** (`cacdad5`): SipHash im
  Chunk-Cache rund 28 % der Zeit der Strahlen, `springe` mit sechs
  Divisionen 27 %. Freie Strahlen liefen Chunk für Chunk bis zur Weite.
- **Behoben** mit dem Hasher `Streuer`, dem Slot je Chunk, dem Sprung mit
  1/d und dem Horizont, siehe [Cinematic](../renderer/cinematic.md), „Der
  schnelle Gang“. Der Horizont über alle Chunks, die der Strahl erreicht,
  ist zugleich ein Fix: Am Prototyp nahm er nur die Chunks bis zwei weiter
  zur Sonne hin, und ein Turm weiter draussen warf keinen Schatten.
- **Die Sonnenblume:** Der erste Fix der oberen Hälfte (`7d73d31`) stand
  als Vergleich in der Schleife des Gangs und kostete je Strahl rund 12 %.
  In `dfd6ed5` steht die Ausnahme in den Bits; danach lag der Gang rund 2
  bis 3 % über dem Stand vor dem Review, durch den Test der Hülle in f64.

## Schluss

- **Die Karte bleibt** Byte für Byte gleich. Ihre Zeit liegt am Stand in
  allen drei Runden 0,4 bis 2,9 % über master, im Fichtenwald 1,1 bis
  3,1 % darunter. Eine Richtung zeigt das nicht; in Phase 1 lag master am
  Stand ebenso leicht vorn.
- **Cinematic mit Sonne** kostet das 2,15- bis 3,63-Fache der Karte, in
  Phase 1 ohne Sonne waren es 1,11 bis 1,21. Das Mehr kommt aus Sonne,
  Wasser, Leuchten, Wärme und Bloom; welcher Teil davon die Strahlen sind,
  zerlegt diese Reihe nicht.
- **Je Strahl** liegt der Gang mit 306 bis 345 ns über Gang 12 mit 244
  bis 254 ns, das 1,22- bis 1,36-Fache. 0056 nennt das einen Befund des
  Reviews. Je Bild bleibt er 8 bis 13 % darunter, denn er rechnet einen
  Strahl je Folge von Pixeln auf einem Texel.
- **Gegen den Gang vor dem schnellen Gang** braucht ein Strahl 57 bis 59 %
  der Zeit.
