---
title: "0143: Kacheln je Baum near-lossless oder verlustbehaftet"
description: Warum ein neuer Baum mit dem Preset standard von oben near-lossless 40 und sonst verlustbehaftet mit quality 90 packt, Gebiete besserer Qualität near-lossless 60, warum Eltern dann immer aus den dekodierten Kindern entstehen und Alpha Bit für Bit bleibt, was verlustfrei bleibt, wie ein Baum seine Qualität behält und wechselt und was verworfen ist; löst 0004 ab.
status: gilt
date: 2026-11-14
issues: [421]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
  - renderer/src/cli/verdichten.rs
  - renderer/src/render/pyramid.rs
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
---

# 0143: Kacheln je Baum near-lossless oder verlustbehaftet

Löst [0004](0004-webp-verlustfrei.md) ab: Kacheln müssen nicht mehr
überall Pixel für Pixel stimmen. Verlustfrei bleibt wählbar.

## Anlass

Der User hat am 14.11.2026 sieben Packungen an einem Ausschnitt der grossen
Welt nebeneinander angesehen und gewählt: Städte near-lossless 60, sonst
verlustbehaftet mit quality 90, von oben aber sonst near-lossless 40. Das
soll die Vorgabe sein, nicht ein Schalter, den man erst setzen muss. Die
Grössen stehen in
[2026-11-14, Packungen an einem Ausschnitt der grossen Welt](../messungen/2026-11-14-packungen-am-ausschnitt-der-grossen-welt.md),
die Abweichung der Pixel in
[2026-11-14, Near-lossless und verlustbehaftetes WebP](../messungen/2026-11-14-near-lossless-und-verlustbehaftet.md).

## Entscheidung

- **`--quality standard|lossless`.** Ein neuer Baum ohne den Schalter packt
  `standard`, `map.json` trägt dann `"quality": "standard"`. Fehlt das Feld,
  packt der Baum verlustfrei wie bisher (`Qualitaet` und `packen` in
  [`renderer/src/cli.rs`](../../renderer/src/cli.rs)).
- **Je Baum nach der Kamera:**

  | Baum | Gebiete besserer Qualität | sonst |
  |---|---|---|
  | von oben, `top` und `top-north` | near-lossless 60 | near-lossless 40 |
  | jede andere Kamera, mit und ohne Cinematic | near-lossless 60 | verlustbehaftet, quality 90, method 4 |

  Die Gebiete übergibt das Plugin mit `--quality-areas`, das folgt in einem
  zweiten Schritt; ohne sie gilt überall „sonst“.
  Von oben gibt es nichts Verlustbehaftetes, also auch keinen Baum zum
  Download des Mods, der verlustbehaftet packt.
- **Eltern aus den dekodierten Kindern.** In einem Baum, der nicht
  verlustfrei packt, entsteht jede gröbere Kachel aus ihren Kindern, wie
  sie auf der Platte stehen, im vollen Lauf wie im Update, auch wo die
  Pixel im Speicher lägen (`ImSpeicher` baut dort keine Stufe). So gleicht
  ein Update weiter Byte für Byte einem vollen Lauf. Die Stufen packen wie
  ihre Kacheln: Der Verlust schaukelt sich nicht auf, siehe
  [2026-11-14, Verlust je Stufe der Pyramide](../messungen/2026-11-14-verlust-je-stufe-der-pyramide.md).
- **Alpha bleibt Bit für Bit**, in jeder Packung. Near-lossless rundet in
  libwebp auch Alpha; der Patch setzt es mit `WEBP_HINT_GRAPH` nach dem
  Runden zurück, siehe [libwebp mit Patch](../entwicklung/libwebp-mit-patch.md).
  Verlustbehaftet packt Alpha verlustfrei (`alpha_quality` 100).
- **Verlustfrei bleiben** Schicht und Rest des Reliefs, sie packen kompakt;
  ebenso Höhen, Boden, Köpfe, Banner und alles, was Daten sind.
- **Die Qualität bleibt beim Baum.** Ohne `--quality` gewinnt immer, was
  `map.json` sagt. Einen anderen Wert nimmt nur ein voller Lauf, er zeichnet
  jede Kachel neu. Ein Update oder ein Ausschnitt mit anderem Wert bricht vor
  der ersten Kachel ab:

  ```
  Qualität:   ./tiles/2x1-se/map.json packt lossless, dieser Lauf will standard. Ein Update oder Ausschnitt stellt nicht um, ein voller Lauf ohne --update mit --quality standard schon.
  ```

- **`--compact`** gilt nur für Bäume mit `lossless`, `--compact-tree`
  ebenso. Near-lossless packt auf dem Weg von kompakt.
- **Kein Zeichenstand** steigt: Die Packung zählt nicht dazu
  ([0098](0098-der-zeichenstand-statt-des-builds.md)). Der Hash der Pixel
  nennt die Packung, siehe [Updates](../benutzung/updates.md), „Gleiche
  Pixel“.

## Verworfene Alternativen

- **Verlustfrei bleiben, wie 0004:** Am Ausschnitt der grossen Welt kostet
  kompakt verlustfrei ausserhalb der Städte das 1,4-Fache (von oben) bis
  2,3-Fache (iso) der Bytes von `standard`.
  Was 0004 gegen verlustbehaftet sagte, Matsch aus Pixelkunst, hat der User
  an den echten Kacheln anders gesehen.
- **Eltern aus dem Speicher:** spart das Dekodieren der Kinder, gäbe aber
  die Gleichheit von Update und vollem Lauf auf. Sie ist die Probe, an der
  die Updates hängen.
- **Near-lossless 60 oder verlustfrei über einer Basis in quality 90:** 3
  bis 8 dB besser, aber das 3,4- bis 6,3-Fache der Bytes über der Basis.
- **Verlustbehaftet mit quality 95, verlustbehaftet von oben, Wasser als
  eigene Klasse:** nicht gewählt.
- **`use_sharp_yuv`:** 9 bis 13 % mehr Bytes und 41 bis 51 % mehr Zeit für
  höchstens 0,7 dB.

## Folgen

- **Platz und Download:** am Ausschnitt der grossen Welt ausserhalb der
  Städte 43 % (iso) und 71 % (von oben) der Bytes von kompakt verlustfrei.
- **Zeit:** Verlustbehaftet packt schneller als kompakt, near-lossless 40
  langsamer; dazu dekodiert jede Elternkachel ihre Kinder, je Kachel +5 bis
  +14 %. Gemessen in der Messung zur Pyramide.
- **Abweichung:** near-lossless höchstens 6 (60) und 11 (40) Stufen je
  Kanal über alle Stufen; verlustbehaftet am Ausschnitt einzelne Pixel bis
  187 Stufen.
- **255 bleibt 255.** Near-lossless klemmt an 255, ein Kanal bei 255 und
  einer bei 0 bleiben, wie sie sind. Ein Kanal bei 252 bis 254 kann an einem
  Pixel, der nicht glatt ist, zu 255 werden.
- **Der Mod** liest beides. Seine Zusage, das Relief auf ±1 wie der Renderer
  zu rechnen, gilt nur für verlustfreie Bäume; auf near-lossless etwa ±5.
- **Ein bestehender Baum** bleibt verlustfrei, bis ein voller Lauf mit
  `--quality standard` ihn umstellt. Danach ist jede Kachel neu, und jeder
  Spieler mit dem Mod lädt den Baum zum Download einmal ganz neu.
- **Tests,** die verlustfreie Pixel brauchen, laufen mit
  `--quality lossless`; die übrigen prüfen `standard` mit.
