---
title: Kartenschrift als WOFF2
description: Woher die Kartenschrift IM FELL English SC kommt, wie die WOFF2 im Repository aus der TTF entsteht, mit Befehl und Versionen, wie geprüft wird, dass nur komprimiert und nichts geändert ist, und wie man die TTF wieder bekommt.
code:
  - web/src/ebenen/schrift/IMFeENsc28P.woff2
  - web/src/ebenen/schrift/OFL.txt
  - web/src/schrift.ts
---

# Kartenschrift als WOFF2

Die Webkarte liefert IM FELL English SC als WOFF2 aus, nach
[0130](../entscheidungen/0130-kartenschrift-als-woff2.md). Die Datei entsteht
einmal von Hand aus der TTF; der Build wandelt nichts um. Diese Seite sagt,
woher die TTF kommt und wie man die WOFF2 neu baut und prüft.

## Herkunft

- **TTF:** `ofl/imfellenglishsc/IMFeENsc28P.ttf` aus dem Repository
  `google/fonts` auf GitHub, Git-Blob `c97d4a0748328052055f42ab27c0659981ce8be7`,
  183 932 Byte.
- **Im Repository** lag die TTF bis zum Wechsel auf WOFF2, eingeführt in
  `e8bdcf9` und seitdem unverändert. Zurück holt sie
  `git show e8bdcf9:web/src/ebenen/schrift/IMFeENsc28P.ttf > IMFeENsc28P.ttf`.
- **Lizenz:** SIL Open Font License 1.1, Text in
  [`OFL.txt`](../../web/src/ebenen/schrift/OFL.txt) neben der Schrift. Die
  Name-Tabelle der Schrift nennt `With Reserved Font Name IM FELL English SC`.

## Bauen

In einer eigenen venv, nicht im Projekt; fontTools 4.60.1 und Brotli 1.1.0:

```bash
python -m venv venv-schrift
venv-schrift/Scripts/python -m pip install fonttools==4.60.1 brotli==1.1.0
venv-schrift/Scripts/python -c "from fontTools.ttLib import woff2; woff2.compress('IMFeENsc28P.ttf', 'IMFeENsc28P.woff2')"
```

Ergebnis: 85 776 Byte, ohne WOFF-Metadaten.

## Prüfen, dass nur komprimiert ist

Nach der FAQ zur OFL, Q2.2.1, behält eine WOFF2 den Namen nur, wenn die Daten
der Schrift bis auf die Kompression gleich bleiben und WOFF-Metadaten fehlen
oder die des Originals unverändert tragen. Geprüft am 24.10.:

- **Zurückgewandelt** mit `woff2.decompress` hat die Schrift dieselben
  Tabellen wie die TTF.
- **Glyphen:** 359 von 359 kompilieren gleich. `glyf` und `loca`
  unterscheiden sich nur in der Form ihrer Bytes, das ist die Transformation
  von WOFF2.
- **`head`:** anders sind nur `flags` Bit 11 und `checkSumAdjustment`. Bit 11
  heisst nach OpenType, die Daten sind verlustfrei durch eine Kompression wie
  WOFF 2.0 gegangen; die Prüfsumme folgt daraus.
- **Metadaten:** keine.

Die Kartenschrift auf dem Schirm ist mit beiden Dateien bitgleich, siehe
[Kartenschrift als WOFF2](../messungen/2026-10-24-kartenschrift-als-woff2.md).
