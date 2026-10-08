---
title: "0090: WebP mit method 0 und quality 75"
description: Warum libwebp die Kacheln mit method 0 und quality 75 statt auf Stufe 0 packt, was das an Platz spart und kostet, was es für Bäume bedeutet, die schon liegen, und welche Wege verworfen oder vertagt sind. Löst 0028 in der Einstellung von libwebp ab.
status: gilt
date: 2026-10-08
issues: [205]
code:
  - renderer/src/render/tiles.rs
  - renderer/tests/tiles.rs
---

# 0090: WebP mit method 0 und quality 75

Löst [0028](0028-libwebp-statt-image.md) in der Einstellung von libwebp
ab: statt Stufe 0, also method 0 und quality 0, packt libwebp mit method 0
und quality 75. Alles andere aus 0028 gilt weiter.

## Anlass

#205: Die Kacheln liessen sich mit anderen Einstellungen von libwebp
kleiner packen, im selben Format und Pixel für Pixel gleich. Der Maintainer
entschied am 08.10., den ersten von drei Wegen sofort als Vorgabe zu nehmen,
nach einer Messung auf dem Referenzrechner.

## Entscheidung

- **`encode_webp`** nimmt weiter `WebPConfigLosslessPreset(&config, 0)`,
  `exact = 1` und `thread_level = 0` und setzt danach `quality = 75`.
- **Warum quality:** libwebp 1.6.0 sucht Rückverweise bis quality 25 nur in
  den letzten 16 Zeilen, ab quality 51 in den letzten 256
  (`GetWindowSizeForHashChain` in `src/enc/backward_references_enc.c`).
  Eine Kachel hat 256 Zeilen, libwebp findet also jede Wiederholung einer
  Textur in ihr. Kacheln bestehen aus wenigen Texturen, die sich wiederholen.
- **Kein Schalter:** Jeder Lauf packt so.
- **Gemessen** in [2026-10-08, WebP mit quality 75](../messungen/2026-10-08-webp-quality-75.md):
  - Basis ×0,79, native Stufen ×0,85, Pyramide ×0,90, Cinematic ×0,82;
  - Pixel für Pixel gleich, Byte für Byte reproduzierbar;
  - rund 0,35 ms mehr je Kachel auf einem Thread, ein voller Lauf mit
    nativen Stufen auf 4 Threads 6 % länger.

  Für die grosse Welt sind das hochgerechnet rund 150 statt 188 GB, für 1
  bis 2 min mehr.
- **Der Test** `webp_findet_wiederholungen_in_der_ganzen_kachel` in
  [`renderer/tests/tiles.rs`](../../renderer/tests/tiles.rs) hält fest,
  dass libwebp Wiederholungen 128 Zeilen tiefer findet. Mit quality 0 oder
  50 fällt er.

## Verworfene und vertagte Alternativen

- **Stufe 0 behalten:** Der Platz ist bei der grossen Welt rund 40 GB wert,
  die Zeit 1 bis 2 min.
- **method 1 mit quality 75:** an #205 auf einem anderen Rechner ×0,73,
  aber 17 statt 2,5 ms je Kachel. Das Kodieren ist schon der grösste Posten
  eines Laufs.
- **Höhere Stufen:** schon in 0028 verworfen, dort mit Zahlen.
- **Ohne räumliche Vorhersage packen,** `--compact` und Nachverdichten (#205,
  Wege 2 und 3): kommen als eigener Schalter mit eigener Entscheidung. Sie
  brauchen einen Patch an libwebp.

## Folgen

- **Bäume, die schon liegen:**
  - Nach dem nächsten vollen Lauf hat jede Kachel neue Bytes und ein neues
    ETag. Ein Abgleich des Mods und ein Download laden den Baum einmal ganz
    neu, dann rund ein Fünftel kleiner.
  - `--update` verlangt nach einem neuen Build ohnehin einen vollen Lauf,
    siehe [Updates](../benutzung/updates.md), „Anderer Renderer, andere
    Assets“.
  - Bis dahin dürfen alte und neue Kacheln im selben Baum liegen; beide
    dekodieren zu denselben Pixeln.
- **Gleiche Bytes bleiben liegen** gilt weiter: Zwei Läufe mit diesem Build
  über dieselbe Welt geben dieselben Bytes.
- Ob libwebp unter Windows mit SSE4.1 und AVX2 dieselben Bytes gibt wie
  ohne, ist nicht geprüft; die Pixel sind in jedem Fall gleich. Das prüft
  #205 vor Weg 2.
