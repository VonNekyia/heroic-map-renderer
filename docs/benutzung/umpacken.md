---
title: "Umpacken: --repack"
description: Wie --repack einen fertigen Baum von maximal oder maximal_compact auf eine andere Stufe packt, ohne Welt und Assets; was es ablehnt, wie es nach einem Abbruch fortsetzt, was neben einem Update geschieht und was gilt, wenn ein halb umgepackter Baum kopiert wird.
code:
  - renderer/src/cli/verdichten.rs
  - renderer/src/cli.rs
---

# Umpacken: `--repack`

`--repack <baum> --quality <stufe>` packt einen fertigen Baum auf eine der
fünf Stufen um, ohne Welt und ohne Assets (`packe_um` in
[`renderer/src/cli/verdichten.rs`](../../renderer/src/cli/verdichten.rs)).
Danach gleicht er Byte für Byte einem vollen Lauf mit dieser Stufe und
denselben Gebieten. Die Stufen stehen in [map.json](map-json.md), „Die
Stufe“. Entschieden in [0146](../entscheidungen/0146-umpacken.md).

```bash
cargo run --release --manifest-path renderer/Cargo.toml -- --repack ./tiles/2x1-se --quality good --threads 1 --low-priority
```

## Was geht

| Baum packt | Ziel | Weg |
|---|---|---|
| `maximal` oder `maximal_compact` | `maximal` oder `maximal_compact` | jede Kachel neu gepackt, Pixel und Zeit bleiben, wie [Kacheln](kacheln.md), „Nachverdichten“ |
| `maximal` oder `maximal_compact` | `good`, `small`, `minimal` | Basis und native Stufen mit zusammengelegten Farben, danach die Eltern neu |
| `good`, `small`, `minimal` | jede | Fehler, vor der ersten Datei; zurück führt nur ein voller Lauf mit `--quality` |

- **Je Kachel** gilt die höchste Stufe aus dem Ziel und den gemerkten
  Gebieten, siehe [Gebiete mit eigener Stufe](gebiete.md). `--quality-areas`
  gibt es an `--repack` nicht; eine neue Datei setzt das nächste Update um.
- **`--compact-tree <baum>`** ist `--repack <baum> --quality maximal_compact`.
- **Schicht und Rest des Reliefs** bleiben verlustfrei, kompakt ausser mit
  dem Ziel `maximal`.
- **Mit `--manifest`** schreibt es das Manifest neu, wie ein Lauf.

## Mit zusammengelegten Farben

1. `map.json` trägt das Ziel ein, bevor eine Kachel sich ändert. Jeder Lauf,
   der jetzt beginnt, packt schon so.
2. Daneben legt es `repack-offen.json` ab: Quelle, Ziel und Beginn.
3. Basis und native Stufen: jede Kachel dekodieren, Farben zusammenlegen wie
   ein Lauf, packen. Der Hash gilt den Pixeln vor der Vorstufe, mit dem
   Präfix des Ziels, siehe [Updates](updates.md), „Gleiche Pixel“. Die
   Kachel bekommt die Zeit des Umpackens.
4. Am Ende baut es jede Elternkachel über den nativen Stufen aus ihren
   Kindern neu, wie `--pyramid`. Die nativen Stufen lässt es dabei, wie sie
   sind.
5. Danach kommt `repack-offen.json` weg.

```
Zoom  5:     18 neu, 0 schon so, 0 übergangen, 0 nicht lesbar
Zoom  4:     8 neu, 0 schon so, 0 übergangen, 0 nicht lesbar
Pyramide:   18 Basiskacheln auf Zoom 5
…
Umgepackt: 26 Kacheln, 0.0 MB statt 0.0 MB, 0 schon so, 0 übergangen, 0 nicht lesbar, in 0.1 s
```

## Abbruch

- **Fortsetzen:** Solange `repack-offen.json` liegt, setzt ein neuer Aufruf
  mit demselben Ziel fort. Was schon umgepackt ist, sagt die Zeit der Datei
  gegen den Beginn, nicht der Hash: Jünger heisst umgepackt oder von einem
  Update mit dem Ziel geschrieben. So geht es auch in einem Baum ohne
  Hashes.
- **Ein anderes Ziel** bricht ab, solange die Datei liegt:

  ```
  Error: <baum>/repack-offen.json liegt da: ein abgebrochenes Umpacken nach good; --repack setzt es nur mit --quality good fort, eine andere Stufe nimmt ein voller Lauf
  ```

- **Ein Update dazwischen** packt mit dem Ziel aus `map.json`. Seine Kacheln
  sind jünger als der Beginn und bleiben; seine Eltern baut das Ende des
  Umpackens neu.
- **Ein voller Lauf** räumt `repack-offen.json` weg; danach trägt jede
  Kachel die Stufe des Laufs.
- **Kopieren:** Wer einen halb umgepackten Baum kopiert und den Dateien dabei
  neue Zeiten gibt, macht jede Kachel jünger als den Beginn. Das Fortsetzen
  hielte dann alles für umgepackt, auch Kacheln, die noch verlustfrei sind.
  Einen solchen Baum macht nur ein voller Lauf mit `--quality <ziel>` wieder
  richtig. Einen Baum also erst fertig umpacken und dann kopieren, oder beim
  Kopieren die Zeiten behalten.
- **Geprüft** in `umpacken_nach_abbruch` in `renderer/tests/cli.rs`: mitten
  in einem Block abgebrochen und fortgesetzt, ohne Hashes, mit einem Update
  dazwischen; je wie ein voller Lauf.

## Was es ablehnt

Vor der ersten Datei, mit einer Zeile `Error:`:

- einen Baum unter `maximal_compact`;
- einen Baum mit `gebiete-neu.json`: Ein abgebrochener Lauf kann andere
  Gebiete haben, das nächste Update oder `--resume` schliesst ihn ab;
- mit zusammengelegten Farben einen Baum mit `stand-neu.bin`;
- einen Baum mit `gebiete.json` ohne `stand.bin`, wie `--pyramid`, siehe
  [Gebiete mit eigener Stufe](gebiete.md), „`--pyramid`“.

Geprüft in `umpacken_lehnt_ab` und `gebiete_datei_bricht_ab`.

## Was es kostet

- **Zeit:** Es dekodiert und packt jede Kachel der Basis und der nativen
  Stufen und baut die Pyramide. An der Testwelt von oben brauchte die
  Pyramide eines vollen Laufs 7,5 bis 8,4 s, siehe
  [Stufen am Baum](../messungen/2026-11-17-stufen-am-baum.md). Gegen einen
  vollen Lauf fällt das Zeichnen weg.
- **Platz:** wie ein voller Lauf mit dem Ziel.
- **Download:** Jeder Spieler mit dem Mod lädt den Baum einmal neu.

Geprüft in `umpacken_gleicht_einem_vollen_lauf`: von `maximal_compact` auf
jede Stufe und von `maximal` auf `good` und `maximal_compact`, im iso und
von oben, je mit einer nativen Stufe, ohne und mit einem Gebiet; jedes Mal
Byte für Byte wie ein voller Lauf.
