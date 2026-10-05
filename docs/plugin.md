---
title: Plugin
description: Das Paper-Plugin lebt im eigenen Repo. Was es vom Renderer nutzt, die Schalter, den Ordner eines Baums, den Kopf von stand-neu.bin, die Ausgabe und den Code, und wer bei einer Änderung Bescheid bekommt.
code:
  - renderer/src/cli.rs
  - renderer/src/render/stand.rs
---

# Plugin

Das Paper-Plugin startet den Renderer auf dem Server als Kindprozess, nach
Zeitplan und per Befehl. Es lebt im eigenen Repo
[`heroic-map-renderer-plugin`](https://github.com/VonNekyia/heroic-map-renderer-plugin),
mit eigener Doku und der Lizenz Apache-2.0. Plan und Aufträge stehen hier,
in #142 und #153. Diese Seite nennt nur, was das Plugin vom Renderer nutzt.
Wer daran etwas ändert, spricht es vorher mit dem Plugin-Programmierer ab.

## Was das Plugin vom Renderer nutzt

- **Schalter** aus `Args` in
  [`renderer/src/cli.rs`](../renderer/src/cli.rs): `--world`, `--assets`,
  `--data`, `--tiles`, `--camera`, `--direction`, `--scale`, `--cinematic`,
  `--gpu`, `--update` und `--resume`, siehe
  [Schalter und Beispiele](benutzung/schalter.md).
- **Den Ordner eines Baums,** `baum_name` in `cli.rs`. Das Plugin rechnet ihn
  nach, um `stand.bin` und `stand-neu.bin` zu finden, siehe
  [map.json](benutzung/map-json.md), „Liste der Bäume“.
- **Den Kopf von `stand-neu.bin`:** Magie, Fassung und Art aus
  `Stand::als_bytes` in
  [`renderer/src/render/stand.rs`](../renderer/src/render/stand.rs). Aus der
  Art entscheidet das Plugin, ob es mit `--resume` fortsetzt, siehe
  [Updates](benutzung/updates.md), „Der Stand“.
- **`RAYON_NUM_THREADS`:** Das Plugin gibt dem Renderer so einen Thread, bis
  #148 einen Schalter bringt.
- **Ausgabe und Code:** Zeilen auf stdout und stderr landen im Log des
  Servers. Die Zeile des Fortschritts, `n/N Kacheln` aus `rendere` in
  `cli.rs`, zeigt das Plugin nur im Status; ändert sich ihre Form, landet
  sie wieder im Log. Code 0 heisst fertig.
