---
title: "0092: Kompakt packen ohne Vorhersage"
description: Warum --compact libwebp mit method 1 und ohne räumliche Vorhersage packen lässt, warum dafür libwebp-sys als Kopie mit Patch im Repository liegt, warum ein Baum sich die Packung merkt, was es spart und kostet und welche Wege verworfen sind.
status: gilt
date: 2026-10-08
issues: [205, 207]
code:
  - renderer/src/render/tiles.rs
  - renderer/src/cli.rs
  - renderer/src/cli/pixel.rs
  - renderer/src/cli/schaetzung.rs
  - renderer/src/render/pyramid.rs
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/Cargo.toml
---

# 0092: Kompakt packen ohne Vorhersage

## Anlass

#205: Kacheln bestehen aus wenigen Texturen, die sich Pixel für Pixel
wiederholen. libwebp schätzt je Bild, ob die räumliche Vorhersage lohnt,
und sieht dabei nur Histogramme, keine Rückverweise. Mit Vorhersage hängt
jeder Rest an den Nachbarpixeln, und dieselbe Textur an anderer Stelle
gibt andere Reste. Ohne sie findet LZ77 die Wiederholungen. Der Maintainer
entschied am 08.10., das als Schalter zu bauen (Weg 2), dazu später das
Nachverdichten eines fertigen Baums (Weg 3).

## Entscheidung

- **`--compact`** packt mit method 1, quality 75 und `image_hint =
  WEBP_HINT_GRAPH` (`Packen::Kompakt` in
  [`renderer/src/render/tiles.rs`](../../renderer/src/render/tiles.rs)).
- **Der Patch:** Über seine API lässt libwebp die Vorhersage nicht
  abschalten. Ein Patch an `EncoderAnalyze` lässt es bei `WEBP_HINT_GRAPH`
  und Bildern ohne Palette nur „subtract green“ nehmen. Bilder mit Palette
  bleiben, wie sie waren. Dafür liegt `libwebp-sys` als Kopie im
  Repository, siehe [libwebp mit Patch](../entwicklung/libwebp-mit-patch.md).
- **Ein Übergang:** Nimmt libwebp eine solche Einstellung selbst auf, fällt
  die Kopie weg. Den Patch schlägt der Maintainer libwebp vor, sobald er
  hier abgenommen ist.
- **Lizenzen der Kopie:** `COPYING` und `PATENTS` von libwebp bleiben in
  ihr. Dazu kommt `LICENSE-MIT` mit dem Text von MIT und den Autoren von
  `libwebp-sys`, denn die Kiste nennt MIT, bringt den Text aber nicht mit.
  Der User entschied am 08.10. für eine Übergangslösung ohne Eintrag in
  `NOTICE`.
- **Der Baum merkt es sich** in `map.json` als `"compact": true`, wie die
  nativen Stufen und den Radius der Mischung. Jeder Lauf auf dem Baum packt
  so, auch ohne den Schalter: voll, `--update`, `--resume`, `--pyramid`.
  Sonst bekäme jede Kachel neue Bytes und ein neues ETag, und der Mod lüde
  alles neu.
  - Ein bestehender schneller Baum bleibt mit `--compact` schnell und sagt
    es.
  - Der Probelauf von `--estimate` packt wie der Baum.
- **Der Hash der Pixel** aus
  [0091](0091-gleiche-pixel-nicht-kodieren.md) nimmt die Packung mit
  hinein. Ein Eintrag aus der schnellen Packung gilt nie für eine kompakte.
- **Gemessen** in [2026-10-08, Kompakt packen](../messungen/2026-10-08-kompakt.md):
  - Basis ×0,71 gegen schnell, ×0,57 gegen Stufe 0;
  - rund 8 bis 10 ms mehr je Kachel;
  - Pixel für Pixel gleich, Byte für Byte reproduzierbar, mit und ohne
    SIMD dieselben Bytes.

  Für die grosse Welt sind das hochgerechnet rund 107 statt 150 GB, für
  rund 20 min mehr.

## Verworfene Alternativen

- **Kompakt als Vorgabe:** Jeder Lauf kostete die Hälfte mehr. Wer schnell
  rendern will, etwa für einen ersten Blick, soll das weiter können.
- **method 1 mit quality 75 ohne Patch:** an #205 auf einem anderen Rechner
  nur ×0,73 gegen Stufe 0.
- **Höhere method mit Patch:** method 3 gab dort ×0,52 für rund 32 ms je
  Kachel, kaum weniger für das Dreifache.
- **Die Vorhersage über eine eigene Schätzung abschalten,** etwa beide
  Wege kodieren und den kleineren nehmen: kostet beide Kodierungen.
- **Der Schalter ohne Gedächtnis im Baum:** Ein Lauf ohne den Schalter
  schriebe jede Kachel mit anderen Bytes neu.

## Folgen

- Die Kopie muss bei jedem Update von `libwebp-sys` mitgehen, mit den
  Schritten in [libwebp mit Patch](../entwicklung/libwebp-mit-patch.md).
- Ein kompakter Baum braucht rund 8 bis 10 ms mehr je neu kodierter
  Kachel, auch in jedem Update. Kacheln mit gleichen Pixeln kodiert ein
  Lauf ohnehin nicht, siehe 0091.
- Einen bestehenden Baum macht `--compact-tree` kompakt, siehe
  [0093](0093-nachverdichten.md).
