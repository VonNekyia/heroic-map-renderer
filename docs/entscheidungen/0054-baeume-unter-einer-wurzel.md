---
title: "0054: Bäume unter einer Wurzel"
description: Warum --tiles mit #68 die Wurzel ist, jeder Baum in einem Ordner <kamera>-<richtung> liegt, trees.json aus der Platte entsteht, die Höhen für alle Bäume gemeinsam unter der Wurzel liegen und ein Baum der alten Ablage abbricht statt umgedeutet zu werden.
status: gilt
date: 2026-10-02
issues: [68]
code:
  - renderer/src/cli.rs
  - renderer/src/render/heights.rs
---

# 0054: Bäume unter einer Wurzel

## Anlass

Mit `--direction` aus #68 bekommt eine Welt mehrere Kachelbäume, je
Kamera und Richtung einen. Bisher war `--tiles` der Baum selbst, und jede
Kamera brauchte ein eigenes Verzeichnis, siehe
[0051](0051-kameras-und-richtungen.md). Das Frontend soll zwischen den
Bäumen einer Welt umschalten, ohne einen Ordnernamen herzuleiten.
Abgestimmt zwischen Backend und Frontend am 02.10., die fünf Punkte des
Reviewers eingeschlossen.

## Entscheidung

- **`--tiles` ist die Wurzel.** Jeder Baum liegt darunter im Ordner
  `<kamera>-<richtung>`, die Kamera gekürzt und mit `x` statt `:`, den
  Windows im Pfad nicht erlaubt: `2x1-se`, `8x5-se`, `top-se`,
  `top-north-s`, `north-45-n`.
- **`trees.json` neben den Bäumen** nennt je Baum `path`, `camera`,
  `direction` und `look`. Der Lauf liest sie aus der Platte, je Ordner mit
  `map.json` ein Eintrag; er führt sie nicht fort.
- **Geschrieben** direkt nach der ersten `map.json` eines Laufs und an
  seinem Ende, jedes Mal über eine eigene Datei, die die alte ersetzt,
  wie in [0018](0018-dateien-tauschen-statt-ueberschreiben.md).
- **Reihenfolge:** `2x1-se` zuerst, wenn es ihn gibt, sonst nach `path`.
  Das Frontend nimmt den ersten Eintrag als Vorgabe.
- **Die Höhen liegen unter der Wurzel,** `heights/{x}.{z}.bin`, für alle
  Bäume gemeinsam; `map.json` nennt sie als `../heights/{x}.{z}.bin`. Sie
  hängen nur an der Welt, nicht an Kamera oder Richtung.
- **Ein Baum der alten Ablage,** `map.json` direkt unter `--tiles`, bricht
  ab, bevor der Lauf die Welt liest. Die Meldung nennt den Ordner, in den
  er gehört. `--pyramid` und `--heights` nehmen weiter jeden Baum, auch
  einen der alten Ablage.

Siehe [`map.json`](../benutzung/map-json.md), „Liste der Bäume“.

## Verworfene Alternativen

- **Die Liste fortführen statt aus der Platte lesen.** Zwei Läufe
  nebeneinander überschrieben einander die Einträge, und ein gelöschter
  Baum bliebe stehen.
- **Nur am Ende schreiben.** Dann liesse sich ein neuer Baum während
  seines ersten Laufs nicht wählen, obwohl seine `map.json` schon mit der
  ersten Kachel dasteht.
- **Die Liste nur bei mehr als einem Baum.** Dann gäbe es zwei Wege für
  das Frontend; mit der Liste immer gibt es einen.
- **Der Name `maps.json`.** Neben `map.json` vertippt man sich leicht, und
  `trees.json` passt zum Schlüssel `trees`.
- **Einen alten Baum still als `2x1-se` nehmen oder ihn verschieben.**
  Ein Lauf darf nichts verschieben, das er nicht angelegt hat, und still
  umgedeutet läge ein neuer Baum neben dem alten. Die Meldung sagt, wohin
  er gehört.
- **Höhen je Baum, wie bisher.** Sie wären für jede Kamera und Richtung
  dieselben Bytes, viermal und mehr geschrieben.

## Folgen

- Das Frontend lädt zuerst `trees.json`; fehlt sie, gilt der Kachelpfad
  selbst als Baum, wie in der alten Ablage.
- Wer einen Baum der alten Ablage weiterrendern will, verschiebt ihn samt
  allem darin in den genannten Ordner. Der nächste Lauf schreibt die
  Höhen dann unter die Wurzel; die alten im Ordner des Baums liest niemand
  mehr.
- `--defender-exclusion` nimmt eine Wurzel mit `trees.json` wie einen
  Kachelordner.
