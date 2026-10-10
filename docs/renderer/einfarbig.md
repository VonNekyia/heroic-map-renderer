---
title: Die einfarbige Ansicht
description: Wie --flat die Welt von oben mit einem Pixel je Block zeichnet - top-north bei scale 1, je Blockzustand das Mittel seiner Oberseite, Biomfarbe und Wasser wie in der Karte mit Texturen, Relief nach dem Nachbarn im Norden wie die Karte des Spiels, ein eigener Baum mit look flat, und was eine Näherung bleibt.
code:
  - renderer/src/render/metatile.rs
  - renderer/src/render/projection.rs
  - renderer/src/render/rasterizer.rs
  - renderer/src/cli.rs
---

# Die einfarbige Ansicht

`--flat` zeichnet die Welt von oben, genordet, mit einem Pixel je Block:
`top-north` bei scale 1. Jeder Block bekommt eine Farbe statt seiner
Textur, Hänge zeigt ein Relief wie auf der Karte des Spiels. Die Ansicht ist
eine leichte Ergänzung für grosse Welten, kein Ersatz für die Karte mit
Texturen. Entschieden in
[0099](../entscheidungen/0099-einfarbige-ansicht.md).

## Schalter und Baum

- **`--flat`** geht mit `--render` oder `--tiles`. Kamera und scale setzt
  der Schalter selbst; er geht nicht mit `--scale`, `--camera`,
  `--direction`, `--cinematic` oder `--native-levels`
  (`flat_ist_ein_eigener_baum` in
  [`renderer/tests/cli.rs`](../../renderer/tests/cli.rs)).
- **scale 1 gibt es nur so.** `--scale` nimmt weiter erst ab 4, und
  `Projection::mit_kamera` hebt jede andere Kamera auf mindestens 2; nur
  `top-north` geht bis 1 (`Projection::flach` in
  [`renderer/src/render/projection.rs`](../../renderer/src/render/projection.rs)).
- **Ein eigener Baum:** `top-north-s-flat`, in `map.json` und `trees.json`
  mit `look` `"flat"`, siehe [map.json](../benutzung/map-json.md), „Look“.
  Der scale gehört zum Baum wie bei jeder Ansicht.
- **Die CPU zeichnet,** auch mit `--gpu on`; das Log sagt
  `GPU:        aus, die einfarbige Ansicht zeichnet die CPU`.
- **Keine nativen Stufen:** Die hören bei scale 4 auf. Die Pyramide nimmt
  je 2 × 2 den nächsten Pixel wie jeder Baum aus `top-north`, siehe
  [Zoomstufen](../benutzung/zoomstufen.md), „Verkleinern“.

## Farbe je Zustand

Der Pfad ist derselbe wie für die Karte mit Texturen, siehe
[Der Weg einer Kachel](renderpfad.md). Von oben stehen senkrechte Flächen
auf der Kante und fallen weg, siehe [Die Kamera](kamera.md), „Von oben“.
Bei scale 1 deckt die Oberseite eines Würfels genau einen Pixel. Der
Rasterizer mittelt die Textur über den Pixel mit 16 × 16 Proben
(`texture_samples` in
[`renderer/src/render/rasterizer.rs`](../../renderer/src/render/rasterizer.rs)):
jedes Texel einer Textur von 16 × 16 einmal, in linearem Licht, gewichtet
mit Alpha, nach dem Alphatest. Das gibt je Blockzustand das Mittel seiner
Oberseite (`flach_ist_das_mittel_der_oberseite` in
[`renderer/tests/metatile.rs`](../../renderer/tests/metatile.rs)).

## Biomfarbe und Wasser

Beides wie in der Karte mit Texturen:

- **Biomfarbe** je Block beim Zeichnen, siehe
  [Biomfarben](biomfarben.md), „Tönung beim Zeichnen“
  (`flach_toent_nach_dem_biom`).
- **Wasser** mit seiner Oberfläche über dem Grund, die Tiefe im Licht,
  siehe [Wasser und Licht](wasser-und-licht.md). Die Karte des Spiels
  schattiert Wasser nach seiner Tiefe; diese zweite Regel gibt es hier
  nicht.

## Relief

Nach dem Zeichnen setzt `relief` in
[`renderer/src/render/metatile.rs`](../../renderer/src/render/metatile.rs)
jeden Pixel in eine von drei Helligkeiten der Karte des Spiels.

- **Die Höhe** eines Pixels ist das y des Blocks, der dort von oben als
  erster etwas zeichnet: der erste Draw an dem Pixel von vorn nach hinten
  (`merke_oben`). Durchsichtiges zählt, Glas über Stein hat die Höhe des
  Glases.
- **Der Nachbar im Norden** ist der Pixel eine Zeile darüber. Jedes Stück
  rendert dafür eine Zeile mehr nach Norden und schneidet sie danach ab
  (`render_flach`). So hat auch die erste Zeile einer Kachel ihren
  Nachbarn. Fehlt dort ein Block, gilt der Nachbar als gleich hoch.
- **Die Regel:** d = Unterschied · 4/5 + (±0,5) · 0,4, mit +0,5, wenn
  x + z ungerade ist. Über 0,6 hell (255), unter −0,6 dunkel (180),
  sonst eben (220), je in 255steln auf Rot, Grün und Blau. Ein Unterschied
  von einem Block zeigt so ein Schachbrett aus hell und eben, ab zwei
  Blöcken ist der Hang ganz hell oder ganz dunkel
  (`flach_relief_nach_norden`).
- **Wasser** bleibt eben (`flach_wasser_ueber_grund_ohne_relief`).
- **Beleg folgt:** Die Regel soll der Karte des Spiels folgen, `MapItem`
  und `MapColor.Brightness` im Client von 26.2. Der Beleg per `javap` steht
  noch aus.

## Was bleibt eine Näherung

- **Texturen über 16 Texel** je Kante mittelt der Rasterizer mit 16 × 16
  Proben wie bei jedem scale, nicht mit jedem Texel.
- **Pflanzen aus Kreuzen** fehlen von oben, wie in `top-north` bei jedem
  scale. Die Karte des Spiels gibt ihnen eine Farbe.
- **Nur die Richtung `s`:** Das Relief nimmt den Nachbarn im Norden als
  Zeile darüber, und Norden liegt oben.
