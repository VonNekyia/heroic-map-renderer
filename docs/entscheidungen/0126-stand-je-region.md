---
title: "0126: Der Stand je Region"
description: Warum der Stand für Updates ab Fassung 2 je Region in einer eigenen Datei unter stand/ liegt, mit stand.bin als Kopf, warum ein Lauf nur die geänderten Regionen und den Kopf zuletzt schreibt, der angefangene Stand nur das, was anders ist, warum je Chunk 9 Bytes für die Grenze nach unten schon leer mitkommen und was Fassung 1 und ältere Renderer dann tun.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/render/stand.rs
  - renderer/src/cli.rs
---

# 0126: Der Stand je Region

## Anlass

Bis Fassung 1 stand der ganze Stand eines Baums in `stand.bin`, 19 Bytes je
Chunk. Jedes Update mit einer Änderung schrieb ihn ganz neu, mit
`sync_all`, und vor der ersten Kachel noch einmal als `stand-neu.bin`:

- **An der Testwelt** 7,5 MB je Schreiben, 0,08 s, siehe
  [2026-10-24, Updates je Phase](../messungen/2026-10-24-updates-je-phase.md).
- **Auf der grossen Welt** rund 47 MB je Baum und Schreiben. Bei einem
  Update alle zwei Minuten sind das gerechnet bis zu 68 GB am Tag je Baum,
  für eine Handvoll geänderter Chunks.
- **Die Grenze nach unten** (Punkt 4 A der Arbeitsliste) braucht je Chunk
  9 Bytes mehr. Das ändert das Format ohnehin.

Ein Update ohne Änderung schreibt den Stand seit #349 nicht mehr.

## Entscheidung

- **Dateien:** `stand.bin` ist der Kopf, 33 Bytes und 8 je Region. Die
  Einträge jeder Region stehen in `stand/<rx>.<rz>.bin`, 12 Bytes Kopf und
  1024 Einträge zu 28 Bytes.
- **Schreiben:** Ein Lauf schreibt nur die Regionen, deren Einträge anders
  sind als in `stand.bin` auf der Platte, jede getauscht wie in
  [0018](0018-dateien-tauschen-statt-ueberschreiben.md). Danach den Kopf,
  zuletzt fallen die Dateien der Regionen weg, die der Kopf nicht mehr
  nennt.
- **Abbruch dazwischen:** Der alte Kopf gilt, einige Regionen tragen schon
  den neuen Stand. Der Stand entsteht erst nach den Kacheln; eine neue
  Region nennt also nie mehr, als die Kacheln zeigen. Das nächste Update
  zeichnet höchstens mehr.
- **Angefangener Stand:** `stand-neu.bin` ist ebenso ein Kopf. In
  `stand-neu/` stehen nur die Regionen, die anders sind als in `stand.bin`;
  die anderen liest `--resume` aus `stand/`. Ein neuer Lauf räumt
  `stand-neu/` vorher.
- **Je Chunk 28 Bytes:** die 19 aus Fassung 1, dazu die Section des
  tiefsten ersten dichten Blocks (1 Byte) und ein Abdruck der Sections
  darunter (8 Bytes). Beide bleiben leer, `0x80` und 0, bis die Grenze nach
  unten sie füllt; das kommt in derselben Version des Renderers.
- **Im Speicher** hält der Stand je Chunk die 19 Bytes der Datei statt
  eines `Eintrag` mit 32.
- **Fassung 1** liest der Renderer weiter. Sie gilt in jeder Region als
  geändert: Der erste fertige Lauf schreibt alle Regionen in Fassung 2,
  auch ein Update ohne Änderung.

## Verworfene Alternativen

- **Eine Datei, je Region an ihrer Stelle überschrieben:** spart den Kopf
  je Region, aber nichts wird getauscht. Ein Abbruch mitten in einer
  Region hinterliesse halbe Einträge, die niemand erkennt.
- **Änderungen an `stand.bin` anhängen:** schreibt wenig, aber die Datei
  wächst, und jemand muss sie wieder verdichten. Lesen kostet mehr, je
  länger der letzte volle Lauf her ist.
- **Den angefangenen Stand ganz schreiben** wie bisher: Vor der ersten
  Kachel schriebe jedes Update wieder alle Regionen, auf der grossen Welt
  rund 70 MB.
- **Ohne die 9 leeren Bytes:** Die Grenze nach unten bräuchte dann eine
  Fassung 3, mit einem zweiten Umstieg im Renderer und im Plugin.

## Folgen

- **Auf der Platte** 28 statt 19 Bytes je Chunk: an der Testwelt gerechnet
  rund 11 statt 7,5 MB, auf der grossen Welt rund 70 statt 47 MB.
- **Ein Update** schreibt je geänderter Region 28 684 Bytes, dazu den Kopf.
- **Ein voller Lauf** schreibt jede Region, je eine Datei mit `sync_all`,
  parallel.
- **Mehr Dateien** im Baum, eine je Region. `--serve` liefert keine davon,
  Manifest und Download nehmen sie nicht mit.
- **Das Plugin** liest die ersten 13 Bytes von `stand-neu.bin`. Die Fassung
  dort ist jetzt 2; ein Plugin, das nur 1 kennt, setzt nicht mehr mit
  `--resume` fort.
- **Ein älterer Renderer** liest Fassung 2 nicht. Wer zurückgeht, braucht
  einen vollen Lauf, bevor `--update` wieder geht.
