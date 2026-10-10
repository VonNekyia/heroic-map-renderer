---
title: "0098: Der Zeichenstand statt des Builds"
description: Warum Stand, Hashes der Pixel und --resume nicht mehr am Build des Renderers hängen, sondern an einem Zeichenstand im Code und an den eingebauten Tabellen, wie ein Test über die Goldbilder ihn absichert, wie die Builds von v0.5.0 einmal übernommen werden und welche Wege verworfen sind.
status: gilt
date: 2026-10-10
issues: [232]
code:
  - renderer/src/render/stand.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli.rs
---

# 0098: Der Zeichenstand statt des Builds

Löst in [0062](0062-updates-nach-stempel-und-fingerabdruck.md) den
Fingerabdruck des Renderers ab, „Anderer Renderer, andere Assets“.

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
  Renderer eine Kachel anders zeichnen kann.
- **Der Fingerabdruck des Renderers** ist FNV-1a über den Zeichenstand und
  die eingebauten Tabellen unter `renderer/src/assets/`, je Tabelle ihr Name
  und ihre Zeilen ohne `\r`. Eine neue Tabelle aus dem Spiel ändert ihn ohne
  Zutun. Ein neuer Build, der gleich zeichnet, hat denselben. Er gilt für
  `stand.bin`, `stand-neu.bin` und die Blöcke unter `pixel/`.
- **Der Test über die Goldbilder:** `GOLDBILDER` neben dem Zeichenstand ist
  FNV-1a über alle Goldbilder, je Bild Name, Breite, Höhe und Pixel, nach
  Namen. `zeichenstand_folgt_den_goldbildern` fällt, sobald sich eines
  ändert, und sagt, was zu tun ist: den Zeichenstand heben und `GOLDBILDER`
  neu setzen. Ändert sich ein Goldbild nur, weil sich die Szene des Tests
  ändert oder eines dazukommt, bleibt der Zeichenstand.
- **Was kein Goldbild zeigt:** Eine Änderung am Zeichnen, die kein Goldbild
  zeigt, hebt den Zeichenstand trotzdem; das prüft das Review.
- **Cinematic:** Der Look steht im Code und geht nicht eigens in den
  Fingerabdruck ein. Eine Änderung daran zeigt das Goldbild
  `metatile-cinematic`.
- **Übergang von v0.5.0:** Die Fingerabdrücke der ausführbaren Dateien von
  v0.5.0 für Linux und Windows stehen in `stand.rs`. Ein Stand oder Block
  mit einem davon gilt als Zeichenstand 1, solange der Build Zeichenstand 1
  hat. Der nächste Lauf schreibt den neuen Fingerabdruck. Zwischen v0.5.0
  und dieser Entscheidung änderte sich am Zeichnen nichts.

## Kosten nach Regel 26

- **Live gerendert:** Nach einem Release, das nicht anders zeichnet, geht
  `--update` gleich weiter. Der volle Lauf, den bisher jedes Release
  verlangte, fällt weg.
- **Initial:** unverändert.
- **RAM und Platte:** unverändert. Der Fingerabdruck liest statt der
  ausführbaren Datei die Tabellen im Speicher, einmal je Lauf.

## Verworfene Alternativen

- **Der Fingerabdruck der ausführbaren Datei,** wie bisher: verlangt nach
  jedem Release einen vollen Lauf, auch wenn das Release nur Doku, Server
  oder Plugin änderte.
- **Die Version aus `Cargo.toml`:** steigt mit jedem Release, ob es anders
  zeichnet oder nicht, und nicht zwischen zwei Releases.
- **Ein Hash über die Quellen des Zeichnens:** steigt mit jedem Kommentar
  und jeder Umformung, und Abhängigkeiten wie libwebp fehlen darin.
- **Die Goldbilder selbst im Fingerabdruck,** ohne Zeichenstand: Ein
  Goldbild ändert sich auch, wenn sich die Szene des Tests ändert oder
  eines dazukommt. Jedes Mal zeichnete jeder Baum alles neu.
