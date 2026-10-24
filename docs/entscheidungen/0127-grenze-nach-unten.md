---
title: "0127: Die Grenze nach unten"
description: Warum ein Update auf der Karte ohne Cinematic und Relief einen geänderten Chunk erst ab min(16·K_alt, E_neu) − 16 zeichnet, wenn seine Sections unter K gleich geblieben sind; was E, K und H sind, warum dicht heisst, dass ein Block das Licht ganz aufhält, und warum Cinematic, Relief und Chunks ohne Tiefe die volle Höhe behalten.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/world/chunk.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
---

# 0127: Die Grenze nach unten

Ergänzt [0062](0062-updates-nach-stempel-und-fingerabdruck.md) im
Gebiet einer Änderung: Nach unten gilt nicht mehr immer die volle Höhe.
Die dort verworfene Grenze „16 unter die Änderung“ bleibt verworfen.

## Anlass

Ein Update zeichnet um jeden geänderten Chunk von der Unterkante der
Dimension an. An der Testwelt bekamen von 1960 Kacheln im Gebiet 10 neue
Pixel. Eine Grenze unter dem tiefsten ersten dichten Block liesse 816 davon
weg, 41,6 %, keine mit neuen Pixeln, siehe
[2026-10-24, Updates je Phase](../messungen/2026-10-24-updates-je-phase.md).

## Entscheidung

- **E:** das tiefste y, an dem in einer Spalte des Chunks von oben der
  erste dichte Block steht. Dicht ist ein Block, der das Licht ganz
  aufhält, Dämpfung 15 in `licht.txt` (`lichtweg`). Ein Block, den die
  Tabelle nicht kennt, ist nicht dicht. Hat eine Spalte keinen, gibt es
  kein E.
- **K und H:** K ist die Section von E, H der Abdruck der Sections unter K
  wie in `Chunk::abdruck`, mit den Blockdaten darin und allen Laubfarben.
  Beide stehen je Chunk im Stand, in den 9 Bytes, die
  [0126](0126-stand-je-region.md) dafür frei hält.
- **Die Grenze:** Kennt der alte Stand K und H, hat der Chunk jetzt ein E,
  und haben seine Sections unter dem alten K wieder H, beginnt sein Gebiet
  bei `min(16·K_alt, E_neu) − 16`. Sonst bei der Unterkante.
- **Nur auf der Karte:** ohne Cinematic und ohne Relief. Von oben ändert
  die Grenze ohnehin nichts.
- **Unbekannt:** Ein Chunk, den der Server während des Laufs schrieb,
  verliert seine Tiefe. Ein Stand aus Fassung 1 kennt keine. Beide
  bekommen sie wieder, wenn ein Lauf den Chunk liest.

## Warum es reicht

- **Unter K ist nichts anders.** Ein Pixel ändert sich nur, wo es einen
  geänderten Block zeigt oder durch dessen Stelle sieht. Das liegt alles ab
  16·K, im Gebiet.
- **Licht:** Das einer Änderung reicht 15 Blöcke, die weiche Beleuchtung
  einen weiter. Himmelslicht fällt in einer Spalte bis zu ihrem ersten
  dichten Block, vorher höchstens bis 16·K_alt, danach bis E_neu, und
  reicht von dort 15 weiter. Ein geöffneter Schacht zieht E_neu nach unten,
  das Gebiet also mit.
- **Biome und Blockdaten** wirken auf derselben Höhe und auf Nachbarn;
  unter K stehen sie in H.
- **Laubfarben** gelten für den ganzen Chunk und stehen darum ganz in H.

## Verworfene Alternativen

- **16 unter die Änderung:** Ein Block hoch über dem Boden dunkelt den
  Boden darunter, wie [0062](0062-updates-nach-stempel-und-fingerabdruck.md)
  sagt. Die Grenze hier liegt unter dem ersten dichten Block jeder Spalte,
  nicht unter der Änderung.
- **Je Spalte eine Grenze:** genauer, aber 256 Werte je Chunk im Stand,
  und das Gebiet wird ohnehin auf Kacheln aufgerundet.
- **Dicht wie im Bild:** Ob ein Block undurchsichtig gezeichnet wird, zählt
  hier nicht. Was ein Pixel zeigt, deckt schon die Stelle der Änderung ab;
  die Grenze braucht nur das Licht.
- **Auch mit Cinematic:** Der Schatten eines Turms fällt auf jeden Block im
  Licht dahinter, auch tief unten in einem Tal. Ebenso der Schatten aus
  `--relief shadows`.
- **Seitlich enger:** liesse weitere der Kacheln ohne neue Pixel weg,
  braucht aber einen dritten Abdruck über die Eingaben des Lichts. Erst
  messen, wie viele es wären.

## Folgen

- **Kosten nach Regel 26:** Je gelesenem Chunk E und H, im Vorlauf eines
  vollen Laufs für jeden Chunk. Im Stand 28 statt 19 Bytes je Chunk, auch im
  Speicher. Der Vorlauf hält je Chunk die Tiefe, bis der Stand steht, 24
  Bytes mehr, auf der grossen Welt gerechnet rund 60 MB.
- **Gemessen** in
  [2026-10-24, Grenze nach unten](../messungen/2026-10-24-grenze-nach-unten.md):
  Ein Update mit 16 Änderungen braucht an der Testwelt 10,4 statt 13,4 s,
  sein Gebiet hat 1144 statt 1968 Kacheln. Der Vorlauf des vollen Laufs
  braucht 3,1 s mehr, 1 % des Laufs; der Arbeitsspeicher bleibt gleich.
- **Wirkung:** nur für Chunks, deren Tiefe der Stand kennt, also nach einem
  vollen Lauf oder sobald ein Update den Chunk liest.
- **Geprüft:**
  - `erster_dichter_und_abdruck_darunter` und `abdruck_bleibt_wie_er_war`
    in `renderer/src/world/chunk.rs`;
  - `grenze_nach_unten_nur_mit_gleichem_abdruck` und `vergleich_mit_tiefe`
    in `renderer/src/render/stand.rs`;
  - `grenze_nach_unten_nur_auf_der_karte` und
    `gebiet_reicht_16_bloecke_um_die_aenderung` in
    `renderer/src/render/tiles.rs`;
  - `grenze_nach_unten_gleicht_einem_vollen_lauf` in
    `renderer/tests/cli.rs`.
