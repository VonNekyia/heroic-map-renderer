---
title: Entpacken und Höhen mit zlib-rs und fdeflate
description: Chunkdaten entpacken und Höhen packen mit flate2 auf miniz_oxide wie heute, auf zlib-rs und mit fdeflate, in der CI; zlib-rs entpackt 28 bis 31 % schneller, fdeflate 53 bis 54 %, und zlib-rs packt die Höhen 1,6- bis 3,2-mal so schnell für 2 % mehr Bytes; nicht gebaut.
date: 2026-11-14
commits: [de436f3a]
code:
  - renderer/src/world/region.rs
  - renderer/src/render/heights.rs
  - renderer/Cargo.toml
---

# Entpacken und Höhen mit zlib-rs und fdeflate

Die Chunkdaten entpackt `flate2` heute mit `miniz_oxide`. Mit `zlib-rs` als
Backend entpackt dasselbe `flate2` 28 bis 31 % schneller, `fdeflate` 53 bis
54 % schneller; die Ausgabe ist gleich. Die Höhen packt `zlib-rs` 1,6- bis
3,2-mal so schnell, sechs Dateien werden dabei zusammen 2,1 % grösser.
Gebaut ist nichts davon.

## Aufbau

- **Frage:** `decompress` in
  [`region.rs`](../../renderer/src/world/region.rs) entpackt Chunkdaten des
  Verfahrens 2 (zlib) mit `flate2`, das auf `miniz_oxide` rechnet.
  `Heights::encode` in [`heights.rs`](../../renderer/src/render/heights.rs)
  packt die Höhen damit. Was bringen zwei andere Bibliotheken in Rust?
  - `zlib-rs` als Backend von `flate2`, ein Schalter in
    [`Cargo.toml`](../../renderer/Cargo.toml); der Code bleibt.
  - `fdeflate`, hier nur zum Entpacken, mit `decompress_to_vec`. Es steckt
    schon über `png` im Baum.
- **Vorher:** [0021](../entscheidungen/0021-bitmasken-statt-blockbesuche.md)
  nennt `zlib-rs` statt `miniz` unter den verworfenen Alternativen, mit
  „kein Unterschied“ und ohne Zahl. Diese Messung misst das Entpacken
  allein.
- **Messcode:** der Tag `versuch/libwebp-leiter`, `de436f3a`, Test
  `probe_entpacken`. Gebaut wird zweimal im selben Lauf: wie heute und mit
  `zlib-rs` hinter `flate2`. `fdeflate` läuft in beiden Programmen gleich
  und zeigt, wie weit sie vergleichbar sind. Gemessen an den Ständen
  `1a11b117` und `f99228f6` eines Probezweigs; nicht gemergt.
- **Chunks:** die 20 echten Chunks der Testwelt aus den Fixtures der Tests,
  siehe [Tests](../entwicklung/tests.md), „Fixtures“: zusammen 114 383 Byte
  gepackt und 937 002 Byte entpackt.
- **Höhen:** sechs Dateien aus Bäumen der Testwelt, im Tag unter
  `renderer/tests/probe-hoehen`: von den Orten Wald, Wasser und Wüste aus
  [Farb-Cache fest statt gesucht](2026-11-13-farbcache-fest-statt-suchen.md)
  je eine Datei `heights` mit 32 768 Byte Werten und eine Datei `ground`
  mit 524 288 Byte. Sie sind zu 69, 67 und 49 % gefüllt.
- **Arbeit:** ein Thread.
  - Entpacken wie `decompress`: `ZlibDecoder` mit `read_to_end`; 300 Runden
    über alle Chunks, im Wechsel mit `fdeflate`.
  - Höhen: `Heights::encode` und `Heights::decode`, 40 Runden je Datei.
- **Rechner:** die CI, `ubuntu-latest`, Release-Build; jeder Lauf auf einem
  anderen geteilten Rechner. Verglichen wird nur innerhalb eines Laufs.

## Ablauf

Am 14.11.2026, zwei Läufe des Workflows. Die Zahlen sind die Ausgabe des
Tests `probe_entpacken` im Log.

- **Je Lauf:** die zwei Programme dreimal im Wechsel; je Wert der Median.
  Die drei Durchgänge liegen beim Entpacken unter 1 % auseinander.
- **Gleiche Ausgabe:** Der Test vergleicht für jeden Chunk, was `fdeflate`
  entpackt, mit dem, was `flate2` entpackt. Was `zlib-rs` an Höhen packt,
  liest der Test als dieselben Werte zurück.
- **MB** heisst 10^6 Byte entpackter Daten.

## Ergebnis

### Chunks entpacken

| | Lauf 1, MB/s | gegen heute | Lauf 2, MB/s | gegen heute |
|---|---|---|---|---|
| `flate2` mit `miniz_oxide`, heute | 540 | | 718 | |
| `flate2` mit `zlib-rs` | 692 | +28 % | 941 | +31 % |
| `fdeflate`, im Programm von heute | 824 | +53 % | 1106 | +54 % |
| `fdeflate`, im Programm mit `zlib-rs` | 841 | | 1126 | |

`fdeflate` liegt in den zwei Programmen 2 % auseinander. So weit sind sie
vergleichbar.

### Höhen packen und entpacken

Zeit in ms je Datei, heute gegen `zlib-rs`.

| Datei | Bytes heute | Bytes mit `zlib-rs` | Packen, Lauf 1 | Packen, Lauf 2 | Entpacken, Lauf 1 | Entpacken, Lauf 2 |
|---|---|---|---|---|---|---|
| `ground`, Wald | 42 121 | 41 869, −0,6 % | 12,25 / 6,80 | 9,30 / 5,22 | 0,68 / 0,37 | 0,53 / 0,29 |
| `ground`, Wasser | 3872 | 5058, +30,6 % | 1,43 / 0,74 | 1,11 / 0,58 | 0,46 / 0,18 | 0,36 / 0,15 |
| `ground`, Wüste | 13 663 | 14 305, +4,7 % | 3,90 / 2,39 | 2,97 / 1,73 | 0,51 / 0,23 | 0,41 / 0,18 |
| `heights`, Wald | 9242 | 9108, −1,4 % | 1,49 / 0,47 | 1,14 / 0,37 | 0,085 / 0,059 | 0,066 / 0,047 |
| `heights`, Wasser | 455 | 519, +14,1 % | 0,17 / 0,07 | 0,13 / 0,06 | 0,032 / 0,012 | 0,022 / 0,009 |
| `heights`, Wüste | 2183 | 2153, −1,4 % | 0,45 / 0,17 | 0,35 / 0,14 | 0,045 / 0,021 | 0,033 / 0,016 |
| alle sechs | 71 536 | 73 012, +2,1 % | | | | |

## Schluss

- **`zlib-rs` hinter `flate2`** entpackt Chunks 28 bis 31 % schneller und
  packt die Höhen 1,6- bis 3,2-mal so schnell. Die Dateien der Höhen werden
  zusammen 2,1 % grösser: Gelände wird 0,6 bis 1,4 % kleiner, flaches
  Wasser 14 und 31 % grösser.
- **`fdeflate`** entpackt 53 bis 54 % schneller als heute. Es nähme nur das
  Verfahren 2 in `decompress`; zum Packen der Höhen ist es nicht gemessen.
- **Was daran hängt:** Im Profil des vollen Laufs von oben ist das
  Entpacken 2,6 % der CPU-Zeit, im Update von oben gut 5 %; das Packen der
  Höhen ist im Update 3,1 %. Siehe die CSV von
  [Renderzeit je Funktion](2026-11-13-renderzeit-je-funktion.md), Zeilen
  `miniz_oxide`. Hochgerechnet wären das beim Entpacken 0,6 bis 0,9 % des
  vollen Laufs und 1 bis 2 % des Updates, bei den Höhen noch einmal 1 bis
  2 % des Updates. Nicht gemessen; die Anteile stammen von Windows.
- **Nicht gebaut.** Mit `zlib-rs` änderten sich die Bytes der Höhen, nicht
  ihre Werte.
- **Grenzen:** 20 Chunks von zwei Orten; die Höhen sind Teile von Regionen;
  nur die Testwelt; geteilte Rechner der CI unter Linux. Am ganzen Render
  ist nichts davon gemessen.
