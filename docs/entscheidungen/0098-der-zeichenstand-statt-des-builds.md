---
title: "0098: Der Zeichenstand statt des Builds"
description: Warum Stand und --resume nicht mehr am Build des Renderers hängen, sondern an einem Zeichenstand im Code und an den eingebauten Tabellen, die Hashes der Pixel an einem eigenen Kodierstand, wie ein Test über die Goldbilder und das Review den Zeichenstand absichern, wie Stände alter Builds übernommen werden und welche Wege verworfen sind.
status: gilt
date: 2026-10-10
issues: [232]
code:
  - renderer/src/render/stand.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
  - .github/workflows/release.yml
  - .github/voller-lauf.sh
---

# 0098: Der Zeichenstand statt des Builds

Löst in [0062](0062-updates-nach-stempel-und-fingerabdruck.md) den
Fingerabdruck des Renderers ab, „Anderer Renderer, andere Assets“, und in
[0091](0091-gleiche-pixel-nicht-kodieren.md) den Renderer als Bedingung der
Hashes.

Zum Teil abgelöst durch [0101](0101-zeichenstand-je-look.md): ein
Zeichenstand je Look statt eines für alle, mit `GOLDBILDER` je Look.

## Anlass

Der Stand eines Baums, die Hashes der Pixel und der angefangene Stand für
`--resume` trugen einen Fingerabdruck der ausführbaren Datei. Jeder neue
Build zählte als anders, auch einer, der dasselbe Bild zeichnet. Nach jedem
Release brauchte jeder Baum einen vollen Lauf, bevor `--update` wieder
ging. Auf einem Server heisst das: die Karte einmal ganz neu, mit den
Threads, die das Plugin dem Renderer gibt, neben den Spielern. Das Plugin
sagt dann „neuer Renderer: erst /heroicmap render“ (#232).

## Entscheidung

- **Der Zeichenstand:** `ZEICHENSTAND` in
  [`renderer/src/render/stand.rs`](../../renderer/src/render/stand.rs),
  eine Zahl ab 1. Er steigt um eins mit jeder Änderung, nach der der
  Renderer eine Kachel oder eine Höhe anders schreiben kann. Ändert sich,
  was der Stand je Chunk festhält (`Chunk::abdruck`, `oben`), steigt
  ebenso `ZEICHENSTAND`. `FASSUNG` des Stands steigt nur mit einem neuen
  Aufbau der Datei, abgesprochen mit dem Plugin: Das Plugin liest den Kopf
  von `stand-neu.bin`, und `Stand::aus_bytes` lehnt jede andere Fassung
  ab.
- **Der Fingerabdruck des Renderers** ist FNV-1a über den Zeichenstand und
  die eingebauten Tabellen unter `renderer/src/assets/`, je Tabelle ihr Name
  und ihre Zeilen ohne `\r`. Er gilt für `stand.bin` und `stand-neu.bin`.
  - Neue Tabellen aus dem Spiel ändern ihn ohne Zutun. Eine neue Datei
    gehört in `TABELLEN`, sonst fällt `jede_tabelle_im_fingerabdruck`.
  - Ein neuer Build, der gleich zeichnet, hat denselben.
  - Linux und Windows haben denselben. Dass sie dieselben Pixel zeichnen,
    stützt nur die CI beider Systeme mit den Goldbildern: `ln`, `exp`,
    `sin`, `cos` und `powf` kommen aus der Mathematik des Systems.
- **Die Meldung** von `--update` behält „stammt von einem anderen Build des
  Renderers“, dahinter „, der anders zeichnet“: Auf den Wortlaut stützt sich
  das Plugin, siehe [Plugin](../plugin.md).
- **Der Test über die Goldbilder:** `GOLDBILDER` neben dem Zeichenstand ist
  FNV-1a über alle Goldbilder, je Bild Name, Breite, Höhe und Pixel, nach
  Namen. `zeichenstand_folgt_den_goldbildern` fällt, sobald sich eines
  ändert, und sagt, was zu tun ist: den Zeichenstand heben und `GOLDBILDER`
  neu setzen. Ändert sich ein Goldbild nur, weil sich die Szene des Tests
  ändert oder eines dazukommt, bleibt der Zeichenstand.
- **Was kein Goldbild zeigt:** Eine Änderung am Zeichnen, die kein Goldbild
  zeigt, hebt den Zeichenstand trotzdem; das prüft das Review. Die Frage
  steht in der Prüfliste von
  [`doku-pflegen`](../../skills/doku-pflegen/SKILL.md).
- **Cinematic:** Der Look geht nicht in den Fingerabdruck ein. Seine Werte
  und sein Verfahren schützt schon `lookHash` in `map.json`, siehe
  [map.json](../benutzung/map-json.md), „Look“. Das Goldbild
  `metatile-cinematic` zeigt eine Änderung am Bild.
- **Die Hashes der Pixel** unter `pixel/` hängen am Kodierstand,
  `KODIERSTAND` in
  [`renderer/src/cli/pixel.rs`](../../renderer/src/cli/pixel.rs), nicht am
  Zeichenstand.
  - Die Kacheln sind verlustfrei, dieselben Pixel ergeben nach dem
    Dekodieren immer dasselbe. Was gezeichnet wird, zählt für den Hash
    nicht.
  - Wer am Kodieren dreht, hebt den Kodierstand: an libwebp oder seinem
    Patch, an den Vorgaben in `encode_webp` oder an `Packen`. Sonst
    erreichte die Änderung keine Kachel, die schon dieselben Pixel zeigt,
    auch kein voller Lauf und kein `--compact-tree`.
  - Ein gehobener Zeichenstand kodiert so keine Kachel neu, deren Pixel
    gleich bleiben.
- **Stände alter Builds:** `ALTE_BUILDS` in `stand.rs` hält die
  Fingerabdrücke der ausführbaren Dateien von v0.4.0 und v0.5.0 für Linux
  und Windows. Die von v0.5.0 sind aus den Archiven des Release gerechnet;
  der für Windows gleicht dem in zwei `stand.bin`, die v0.5.0 schrieb. Von
  v0.4.0 gibt es kein Release mehr; seine Abdrücke stammen aus den Paketen
  seines Release-Laufs. Unter `renderer/` änderte sich zwischen v0.4.0 und
  dieser Entscheidung nur die Version.
  - Ein Stand mit einem davon gilt als `ZEICHENSTAND_1`, eingefroren: der
    Fingerabdruck von Zeichenstand 1 mit den Tabellen von v0.5.0
    (`als_zeichenstand`). Er gilt also nur, solange Zeichenstand und
    Tabellen die von v0.5.0 sind; danach verlangt er einen vollen Lauf wie
    jeder andere.
  - Ein Block unter `pixel/` mit einem davon gilt als Kodierstand 1.
  - Der nächste Lauf schreibt den neuen Wert.
- **Releases:** Die Notizen eines Release sagen, ob ein voller Lauf kommt:
  ob sich seit dem letzten Tag `ZEICHENSTAND` oder eine Tabelle geändert
  hat, siehe [0082](0082-versionen-und-releases.md), „Folgen“. Sie sagen
  auch, dass ein Baum eines Release vor v0.4.0 einmal ganz gerendert wird.

## Kosten nach Regel 26

- **Live gerendert:** Nach einem Release, das nicht anders zeichnet, geht
  `--update` gleich weiter. Der volle Lauf, den bisher jedes Release
  verlangte, fällt weg.
- **Initial:** unverändert.
- **RAM und Platte:** unverändert. Statt der ausführbaren Datei, bei v0.5.0
  10,5 MB unter Linux und 11,9 MB unter Windows, liest der Fingerabdruck
  einmal je Lauf die Tabellen, rund 0,47 MB, die schon im Binär liegen.

## Verworfene Alternativen

- **Der Fingerabdruck der ausführbaren Datei,** wie bisher: verlangt nach
  jedem Release einen vollen Lauf, auch wenn das Release nur Doku, Server
  oder Plugin änderte.
- **Die Version aus `Cargo.toml`:** steigt mit jedem Release, ob es anders
  zeichnet oder nicht, und nicht zwischen zwei Releases.
- **Ein Hash über die Quellen des Zeichnens:** steigt mit jedem Kommentar
  und jeder Umformung. Abhängigkeiten fehlen darin.
- **Die Goldbilder selbst im Fingerabdruck,** ohne Zeichenstand: Ein
  Goldbild ändert sich auch, wenn sich die Szene des Tests ändert oder
  eines dazukommt. Jedes Mal zeichnete jeder Baum alles neu.
- **Die Hashes der Pixel am Zeichenstand:** Das Kodieren hinge an einer
  Zahl, die es nicht beschreibt. libwebp fehlt im Zeichenstand, und wer
  nur das Zeichnen ändert, kodierte jede Kachel neu.
- **Stände alter Builds als den heutigen Fingerabdruck:** Hebt ein späteres
  Release eine Tabelle und lässt den Zeichenstand bei 1, nähme es einen
  Stand von v0.5.0 an und mischte alte und neue Tabellen.
