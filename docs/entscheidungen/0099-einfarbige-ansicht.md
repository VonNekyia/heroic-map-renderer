---
title: "0099: Die einfarbige Ansicht mit --flat"
description: Warum --flat top-north bei scale 1 mit einer Farbe je Block zeichnet, über den bestehenden Pfad statt eines eigenen für Spalten, warum der Baum look flat trägt, Wasser wie mit Texturen bleibt und das Relief der Karte des Spiels folgt, was es nach Regel 26 kostet und welche Wege verworfen sind.
status: gilt
date: 2026-10-10
issues: [230]
code:
  - renderer/src/cli.rs
  - renderer/src/cli/schaetzung.rs
  - renderer/src/render/projection.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/rasterizer.rs
---

# 0099: Die einfarbige Ansicht mit --flat

Löst für diese Ansicht den Teil von
[0013](0013-scale-32-als-standard.md) und
[0051](0051-kameras-und-richtungen.md) ab, der keinen scale unter 4
erlaubt.

## Anlass

Für grosse Welten fehlte eine leichte Ansicht. Die Karte aus Rasterkacheln
zeichnet ab 4 Pixeln je Block, und das kostet Zeit und Platz (Regel 22).
Eine Farbe je Block bei einem Pixel braucht ein Sechzehntel der Pixel von
scale 4. #230 hat niedrige Priorität, nach #228 und den Ebenen, so hat es
der Maintainer am 09.10. entschieden. Der Reviewer gab den Plan am 10.10.
frei.

## Entscheidung

- **Schalter `--flat`:** `top-north` bei scale 1. Er geht nicht mit
  `--scale`, `--camera`, `--direction`, `--cinematic` oder
  `--native-levels`. Für alle Ansichten mit Texturen bleibt `--scale` ab 4
  und `NATIVE_MIN_SCALE` 4. `Projection::mit_kamera` erlaubt scale 1 nur
  für `top-north`, jede andere Kamera bleibt bei mindestens 2.
- **Der Pfad bleibt derselbe:** Kandidaten, Deckung, Sprites und Blit wie
  in der Karte mit Texturen. Bei scale 1 ist die Oberseite eines Würfels ein
  Pixel, und der Rasterizer mittelt die Textur darüber schon in linearem
  Licht, gewichtet mit Alpha. Das ist die Farbe je Zustand. Bei scale 1
  tastet er so dicht ab, wie der Frame Texel hat, bis 64 je Kante; andere
  Bäume trifft das nicht. Ein eigener Weg für Spalten kommt nur, wenn die
  Messung es verlangt.
- **Ein eigener Baum:** `top-north-s-flat`, `look` `"flat"` in `map.json`
  und `trees.json`. Der look folgt aus Kamera und scale; ein eigenes Feld
  braucht es nicht.
- **Biomfarbe und Wasser** wie in der Karte mit Texturen, keine zweite
  Regel für die Tiefe des Wassers.
- **Relief** nach der Regel der Karte des Spiels: drei Helligkeiten nach dem
  Unterschied zum Nachbarn im Norden. Bei ganzen Blöcken und der Karte im
  Massstab 1:1 zählt nur sein Vorzeichen, das Schachbrett aus x + z ändert
  nichts. Wasser bleibt eben. Die Einzelheiten stehen in
  [Die einfarbige Ansicht](../renderer/einfarbig.md), „Relief“, belegt am
  Client von 26.2 unter „Die Karte des Spiels“.
- **Nur die CPU,** wie bei Cinematic: Das Relief braucht je Pixel die Höhe
  aus der Deckung, und bei einem Pixel je Block bringt die Grafikkarte
  wenig.
- **Nur die Richtung `s`:** Norden liegt oben, der Nachbar im Norden ist
  die Zeile darüber.

## Kosten nach Regel 26

- **Platz:** ein Sechzehntel der Pixel von `top-north` bei scale 4, flache
  Farben packen gut. Gemessen wird in einem eigenen Schritt.
- **Initial:** Lesen, Licht und Kandidaten je Block bleiben. Erwartet ist
  ein Lauf 2- bis 4-mal so schnell wie `top-north` bei scale 4, nicht
  16-mal; das zeigt erst die Messung.
- **Live gerendert:** Ein flacher Baum neben dem mit Texturen heisst auf dem
  Server ein zweites `--update` über die geänderten Chunks. Es liest
  dieselben Chunks und zeichnet bei scale 1 ein Sechzehntel der Pixel.
- **RAM:** nur mit `--flat`. Eine Kachel umfasst 16 × 16 Chunks und ihren
  Rand; `oben` hält 8 Byte je Pixel, rund 0,5 MB je Kachel und rund 8 MB je
  Stück von 1024 × 1024 Pixeln, dazu 4 Byte je Draw.
- **Die Karte mit Texturen** wird weder langsamer noch anders: Sie nimmt
  den Weg mit Relief nie.

## Verworfene Alternativen

- **Ein eigener Renderer für Spalten:** je Spalte den obersten Block
  suchen, Wasser mischen, Farbe nachschlagen. Er wäre schneller, aber er
  verdoppelte Modelle, Tönung, Licht und Deckung. Zuerst misst der
  bestehende Pfad.
- **Die Farben der Karte des Spiels (`MapColor`):** eine feste Palette je
  Block. Sie passte nicht zu Packs und eigenen Blöcken, und das Mittel der
  Textur gibt es schon.
- **Wasser nach seiner Tiefe** wie die Karte des Spiels: eine zweite Regel
  neben dem Licht, und die Ansichten zeigten Wasser verschieden.
- **Ein eigenes Feld in `map.json`:** `look` trennt schon Karte und
  Cinematic, und das Frontend zeigt einen unbekannten look als Zusatz im
  Namen.
