---
title: Grösse des Binärs und statische CRT
description: Woraus das Release-Binär unter Windows besteht, was LTO, strip, ein kleinerer opt-level und die statische CRT an Grösse bringen, roh und gepackt wie im Jar, und was die statische CRT und opt-level "s" für den GPU-Stapel an Laufzeit kosten, gemessen an der Testwelt mit und ohne Grafikkarte.
date: 2026-10-05
commits: [3c99cbb]
code:
  - renderer/Cargo.toml
  - .cargo/config.toml
---

# Grösse des Binärs und statische CRT

Das Release-Binär unter Windows hat 10,07 MB, gepackt 3,84 MB. Mit
statischer CRT und `opt-level = "s"` für den GPU-Stapel und clap sind es
9,15 MB, gepackt 3,57 MB, und es braucht keine VC++-Laufzeit mehr. An
Laufzeit kostet beides nichts Messbares: An der Testwelt liegen alle Stände
mit und ohne Grafikkarte in der Streuung. Zu #146.

## Aufbau

- **Stand:** master `3c99cbb`, Release-Profil `opt-level = 3`,
  `codegen-units = 1`, Ziel `x86_64-pc-windows-msvc`.
- **Je Variante ein kalter Build** in ein eigenes Zielverzeichnis. Die
  Variante kam über `cargo --config` dazu, die Quellen blieben gleich.
- **Gepackt** heisst: Deflate mit Stufe 6 in einer Zip-Datei, wie im Jar
  des Plugins. Gerechnet mit `zipfile` aus Python, `compresslevel=6`.
- **Aufteilung:** aus der Map-Datei von `link.exe`
  (`cargo rustc --release --bin heroic-map-renderer -- -C link-arg=/MAP:…`).
  Jedes Symbol bekommt als Grösse den Abstand zum nächsten in seinem
  Abschnitt, summiert je Bibliothek. Das ist eine Näherung; zugeordnet sind
  9,82 von 10,07 MB.
- **Importe:** `llvm-readobj --coff-imports` aus `llvm-tools`.
- **Laufzeit:** die Testwelt, Ausschnitt wie in
  [Zwei Zustände der Basis](2026-10-02-zwei-zustaende-der-basis.md): um
  (-64, 416) mit `--size 18432`, scale 32, ohne native Stufen, 5184
  Basiskacheln. 24 Threads, `--gpu off` und `--gpu on`.

  ```bash
  <binär> --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --tiles <ordner> --center -64 416 --size 18432 --scale 32 --native-levels 0 --gpu on
  ```

## Ablauf

- **Grössen:** am 05.10. zwischen 20:57 und 21:20, aus den Dateigrössen.
- **Laufzeit:** am 05.10. von 21:31:35 bis 21:37:02, mit Sperrdatei.
  - Die Last vor dem Start lag bei 6 %.
  - Drei Runden, von Runde zu Runde in anderer Folge.
  - Jeder Lauf in einen frischen Ordner, der vom Echtzeitschutz ausgenommen
    war; gleich danach gelöscht, dann 15 s Pause.
  - Die Rate der Basis steht in der Ausgabe jedes Laufs, Zeile nach
    „Kacheln:“.

## Woraus das Binär besteht

| Bibliothek | Grösse | Anteil |
|---|---|---|
| der eigene Code, Bibliothek und Binär, samt eingebauter Tabellen | 3,05 MB | 31 % |
| `naga` | 2,00 MB | 20 % |
| `wgpu-core` | 1,48 MB | 15 % |
| `wgpu-hal` | 0,97 MB | 10 % |
| `libwebp-sys`, der C-Quelltext von libwebp | 0,49 MB | 5 % |
| `clap_builder` | 0,35 MB | 4 % |
| `wgpu` | 0,23 MB | 2 % |
| `ash` | 0,21 MB | 2 % |
| `std` | 0,20 MB | 2 % |
| `libmimalloc-sys` | 0,15 MB | 2 % |
| übrige | 0,69 MB | 7 % |

Der GPU-Stapel, also `naga`, `wgpu-core`, `wgpu-hal`, `wgpu`, `ash`,
`gpu-allocator` und `wgpu-types`, macht rund 5,0 MB aus, die Hälfte.

## Grösse je Variante

| Variante | roh | gepackt |
|---|---|---|
| heute | 10 066 432 Byte | 3 835 447 Byte |
| `lto = "fat"` | 9 935 360 Byte | 3 837 654 Byte |
| `lto = "thin"` | 10 348 544 Byte | 3 960 413 Byte |
| `opt-level = "s"` für alles | 8 252 928 Byte | 3 116 499 Byte |
| `opt-level = "z"` für alles | 7 587 328 Byte | 2 831 795 Byte |
| `strip = "symbols"` | 10 066 432 Byte | 3 835 484 Byte |
| statische CRT | 10 301 440 Byte | 3 952 991 Byte |
| statische CRT, `lto = "fat"`, `strip` | 10 170 880 Byte | 3 956 040 Byte |
| **statische CRT, GPU-Stapel und clap mit `opt-level = "s"`** | **9 154 048 Byte** | **3 570 002 Byte** |
| statische CRT, GPU-Stapel und clap mit `opt-level = "z"` | 8 667 648 Byte | 3 378 372 Byte |

„GPU-Stapel und clap“ heisst hier `naga`, `wgpu-core`, `wgpu-hal`, `wgpu`,
`ash`, `gpu-allocator` und `clap_builder`.

- **LTO** spart gepackt nichts. Mit `codegen-units = 1` ist schon fast
  alles in einer Einheit.
- **`strip`** ändert unter Windows nichts: Die Symbole liegen in der PDB,
  nicht im Binär.
- **Die statische CRT** kostet 0,12 MB gepackt. Danach importiert das
  Binär weder `VCRUNTIME140.dll` noch `VCRUNTIME140_1.dll` noch eine der
  `api-ms-win-crt-*`-DLLs, auch nicht über libwebp und mimalloc.
- **`opt-level = "s"` nur für den GPU-Stapel und clap** spart gepackt
  0,38 MB gegen die statische CRT allein. Für den eigenen Code bliebe
  `opt-level = 3`, denn dort liegt die Arbeit je Pixel.

## Laufzeit

Basis in Kacheln/s, Runde 1, 2 und 3:

| | heute | statische CRT | statische CRT, GPU-Stapel `"s"` |
|---|---|---|---|
| `--gpu off` | 988, 1110, 1126 | 996, 1064, 1114 | |
| `--gpu on` | 1188, 1320, 1312 | 1132, 1301, 1323 | 1148, 1322, 1302 |

- **Runde 1** ist bei allen Ständen langsamer, die Caches sind noch kalt.
- **Ohne Grafikkarte** lag die statische CRT in Runde 1 um 0,8 % höher, in
  Runde 2 um 4,1 % und in Runde 3 um 1,1 % tiefer, im Mittel 1,6 % tiefer.
  Derselbe Stand streut von Runde zu Runde um bis zu 14 %; ein Unterschied
  ist darin nicht zu sehen.
- **Mit Grafikkarte** liegen die drei Stände in Runde 2 und 3 innerhalb von
  1,6 % beieinander, ohne Richtung.
- **Ein Lauf** dauerte 5,9 bis 8,0 s, die Basis 3,9 bis 5,2 s.

## Schluss

- Statische CRT und `opt-level = "s"` für den GPU-Stapel und clap kommen
  ins Release-Profil, siehe [Weitergabe](../entwicklung/weitergabe.md).
- `opt-level = "z"` spart weitere 0,19 MB gepackt. Gemessen ist es nicht;
  es kommt nur, wenn das Linux-Binär sonst nicht passt.
- Das Linux-Binär misst die CI, siehe [Weitergabe](../entwicklung/weitergabe.md).
