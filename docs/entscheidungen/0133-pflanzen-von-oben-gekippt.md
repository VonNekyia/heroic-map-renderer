---
title: "0133: Pflanzen von oben gekippt"
description: Warum der Renderer Bodenpflanzen von oben um 45° nach oben im Bild kippt und in ihren Block einpasst, statt ihre erste Fläche flach zu legen; dass die unterste Wurzel sagt, was eine Pflanze ist, dass nur eine liegende Fläche ein Modell stehen lässt, dass Redstone und Stiele nach ihrem Zustand färben, warum die Zeichenstände aller drei Looks steigen und was anders ist als im Mod, dazu der Nachtrag, dass Leeres ausserhalb des Blocks vor dem Kippen wegfällt; löst 0108 ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/render/sprites.rs
  - renderer/src/render/projection.rs
  - renderer/src/assets/mod.rs
  - renderer/src/assets/colors.rs
  - renderer/src/render/stand.rs
---

# 0133: Pflanzen von oben gekippt

Löst [0108](0108-pflanzen-von-oben-flach.md) ab. Der Mod rechnet nach
derselben Regel, entschieden dort in 0049.

## Anlass

- **Der User am 24.10.:** Aus Bildern des Mods mit mehreren Arten, Pflanzen
  von oben zu zeigen, wählte er „45° nach Norden, eingepasst“.
- **Pakete mit eigenen Pflanzen:** Mit einem Paket, das Pflanzenmodelle
  ersetzt, verlor eine Pflanze ihre Art, oder ihre leicht gekippten Flächen
  liessen das Modell stehen, und sie fehlte von oben.
- **Redstone und Stiele** blieben weiss, weil der Renderer ihre Farbe nicht
  kannte.

## Entscheidung

- **Kippen statt flach legen:** Von oben kippt eine Bodenpflanze starr um
  45°, die Spitze nach oben im Bild, und wird entlang der Kipprichtung in
  ihren Block eingepasst. Unter Wasser bleibt sie unter der Oberfläche. Die
  Regel im Einzelnen steht in [Die Kamera](../renderer/kamera.md),
  „Pflanzen von oben“.
- **Die unterste Wurzel sagt, was eine Pflanze ist (A):** nach ihrer
  Blockstate-Datei und den `parent`-Ketten in ihr; gezeichnet wird weiter
  das Modell des obersten Packs.
- **Nur eine liegende Fläche lässt ein Modell stehen (B):** n_y ≥ cos 45° ·
  |n|. Flächen, die nur leicht gekippt sind, kippen mit.
- **Dazu als Bodenpflanze:** die Stiele von Kürbis und Melone. Beim Bambus
  kippen nur die Blätter.
- **Farben nach dem Zustand:** Redstone nach `power`, Stiele nach `age`,
  angewachsene Stiele fest, siehe
  [Biomfarben](../renderer/biomfarben.md), „Nach dem Zustand“.
- **Zeichenstände:** `map` und `flat` steigen wegen des Kippens, `cinematic`
  wegen der Farben und weil die Stiele dort jetzt den Strahl zur Sonne
  dämpfen. In Cinematic kippt nichts. Jeder Baum braucht einen vollen Lauf.
  Auf main kommt er mit dem für die Schicht des Reliefs zusammen,
  [0132](0132-relief-als-schicht.md).

## Nachtrag 25.10.: Leeres ausserhalb des Blocks

Ein Paket mit Feldfrüchten von 32 Einheiten Höhe, oben durchsichtig, kam
gestaucht auf einen halben Block heraus: Das Einpassen mass auch den leeren
Teil über dem Block. Darum schneidet der Renderer vor dem Kippen ab, was
über y = 1 oder unter y = 0 liegt und dort ganz durchsichtig ist. Die Regel
steht in [Die Kamera](../renderer/kamera.md), „Pflanzen von oben“.

- **Nur ausserhalb des Blocks:** Ein Schnitt auf das ganze deckende
  Rechteck änderte die Blumen des Spiels mit ihren durchsichtigen Rändern.
  Regel des Reviewers, im Mod und im Renderer gleich, abgestimmt am 25.10.
- **Kein neuer Zeichenstand:** Mit den Assets des Spiels schneidet die
  Regel nichts, geprüft an allen Zuständen von 26.3
  (`vanilla_ohne_leeren_rand`). Die Packs auf main ändern daran nichts:
  Keines bringt Modelle oder Texturen von Pflanzen, und der Renderer liest
  dort nur die Assets des Spiels. Ein Baum mit einem Paket wie oben
  zeichnet nach einem vollen Lauf richtig; ein Update zieht nur nach, was
  es anfasst.

## Anders als im Mod

- **Die Richtung:** Der Renderer kippt nach oben im Bild. In `top-north`
  aus `s`, der Karte von main, ist das Norden wie im Mod; aus einer anderen
  Richtung kippt die Pflanze mit dem Bild.
- **Quer zur Kipprichtung** ragt eine Pflanze, die breiter ist als ihr
  Block, über ihn hinaus, wie andere Modelle, die überstehen. Der Mod
  zeichnet je Spalte und schneidet dort ab. Pflanzen des Spiels sind nicht
  breiter als ihr Block.

## Verworfene Alternativen

Alle an den Bildern des Mods, gegen das Kippen um 45° nach Norden:

- **Flach wie 0108:** gab Feldern und Wiesen keine Tiefe.
- **Eine Richtung je Block, gewürfelt:** unruhig.
- **60° statt 45°:** die Pflanzen fast liegend, der Grund kaum zu sehen.
- **Das ganze Modell um 90°:** die Pflanzen lagen wie umgefallen.
- **Ohne Einpassen:** der Rand des Blocks schnitt die Spitzen ab.
- **Ohne Klemmen unter Wasser:** Seegras stand über seinem Wasser.
- **Den Zeichenstand halten und für Pakete einen eigenen Abdruck führen:**
  unnötig, denn das Kippen ändert auch die Bilder ohne Paket, und die
  Zeichenstände steigen ohnehin.
