---
title: "0120: Banner von vorn in jeder Kamera"
description: Warum der Renderer die Banner aus --banners für jeden Baum von vorn zeichnet wie den Satz oben, auch im Iso, so dass Tuch und Name gerade stehen; warum die Kacheln und kein Zeichenstand sich ändern, kein Baum einen vollen Lauf braucht und BANNERSTAND nicht steigt; löst 0100 im Punkt „Blick“ und 0102 im schrägen Satz ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/cli/banner.rs
---

# 0120: Banner von vorn in jeder Kamera

Löst [0100](0100-der-renderer-zeichnet-die-banner.md) im Punkt „Blick“ ab
und [0102](0102-name-im-bogen.md) im schrägen Satz.

## Anlass

Der User am 23.10.: Die Banner auf der Iso-Karte sollen nach vorn zeigen,
der Name auch; schräg sind sie schlecht lesbar. Bis dahin zeichnete
`--banners` den Satz eines Iso-Baums in seiner Kamera und Richtung. Das
Tuch stand so schräg wie die Blöcke daneben, in `2:1` mit der Unterkante um
26,6° fallend, in `4:3` um 36,9°, und der Name drehte sich mit.

## Entscheidung

- **Jeder Satz zeichnet wie `oben`:** aus `north-45`, Richtung `s`, in jeder
  Kamera und Richtung des Baums. Das Sprite ist Pixel für Pixel das aus
  `oben`.
- **Die Sätze bleiben:** je Baum, der nicht von oben schaut, einer unter
  seinem `path`. Webkarte und Mod finden ihn wie bisher.
- **`satz.json`:** `foot` wie im Satz `oben`, `angle` in jedem Satz 0. Das
  Format bleibt. Der Name steht so gerade, und sein Bogen dreht nicht mehr.
- **Was gleich bleibt:**
  - der Look: Die Sätze zeichnet `--banners` weiter im Look der Karte, auch
    für Bäume mit Cinematic;
  - die Grösse, höchstens 32 × 64, und der Fuss, der Punkt P(x, y+1, z);
  - die Banner in der Welt: Als Blöcke gehören sie zu den Kacheln und
    stehen weiter in der Kamera des Baums.

## Folgen

- **Keine Kachel ändert sich,** denn die Sprites liegen unter
  `layers/…/banner/` über der Karte. Kein Zeichenstand eines Looks steigt,
  und kein Baum braucht einen vollen Lauf.
- **`BANNERSTAND` bleibt.** Der Stempel je Entwurf und Satz enthält Kamera
  und Richtung. Ein Satz aus der Zeit davor hat darum einen anderen Stempel,
  und der nächste Aufruf von `--banners` zeichnet ihn neu, beim Plugin
  spätestens beim Start des Servers. Das belegt der Test
  `schraeger_satz_wird_neu_gezeichnet`.
- **Kosten:** einmal je Satz und Entwurf zwei Sprites neu, rund 2,9 ms je
  Sprite, siehe [2026-10-11, Sprites der Banner](../messungen/2026-10-11-banner-sprites.md).
  Danach wie vorher.
- **Webkarte und Mod** ändern nichts dafür. Die Webkarte dreht den Namen um
  `angle`; mit 0 steht er waagrecht, ohne Änderung im Frontend. Der Mod
  nimmt `oben`.

## Verworfene Alternativen

- **Die Sätze der Iso-Bäume streichen und überall `oben` nehmen:** Das wäre
  noch einfacher, aber die Webkarte müsste vorher den Satz wechseln, sonst
  fehlten ihr die Sprites. Erst, wenn die doppelten Dateien stören.
- **`BANNERSTAND` heben:** Das zeichnete auch `oben` neu, ohne dass sich
  dort ein Pixel ändert. Der Stempel reicht.
