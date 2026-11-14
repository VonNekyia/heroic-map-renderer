---
title: "0142: Kompakt ohne Suche des Farb-Caches, ohne RLE und ohne Analyse"
description: Warum libwebp beim kompakten Packen den Farb-Cache fest auf seine grösste Grösse setzt, RLE nicht mehr als zweite Zerlegung probiert und die Analyse der Entropie auslässt, wo ihr Ergebnis feststeht; warum dafür der Kodierstand steigt, was es spart und kostet und was verworfen ist; ergänzt 0092.
status: gilt
date: 2026-11-14
issues: [417]
code:
  - renderer/vendor/libwebp-sys/vendor/src/enc/vp8l_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.c
  - renderer/vendor/libwebp-sys/vendor/src/enc/backward_references_enc.h
  - renderer/src/cli/pixel.rs
---

# 0142: Kompakt ohne Suche des Farb-Caches, ohne RLE und ohne Analyse

Ergänzt [0092](0092-kompakt-ohne-vorhersage.md) um drei Stellen im Patch an
libwebp. Der Satz dort, Bilder mit Palette blieben, wie sie waren, gilt für
die ersten zwei nicht mehr.

## Anlass

Kodieren ist 24,6 % der CPU-Zeit eines vollen Laufs von oben und 29,7 % im
iso mit Cinematic, siehe
[2026-11-13, Renderzeit je Funktion](../messungen/2026-11-13-renderzeit-je-funktion.md).
Davon rechnet libwebp je kompakter Kachel drei Dinge, die an Kacheln fast
nichts bringen: Es sucht die Grösse des Farb-Caches
(`CalculateBestCacheSize`), es zerlegt mit LZ77 und dazu mit RLE und
bewertet beide, und es schätzt die Transformation (`AnalyzeEntropy`), deren
Ergebnis der Patch aus 0092 danach überschreibt.

## Entscheidung

Mit `WEBP_HINT_GRAPH`, also nur beim kompakten Packen:

- **Farb-Cache fest:** `GetBackwardReferences` lässt die Suche aus. Der
  Cache bleibt bei der grössten Grösse, die libwebp dem Bild erlaubt, ohne
  Palette 10 Bit; wo es weniger oder keinen erlaubt, bleibt es dabei. Den
  Hinweis trägt das Feld `cache_fixed` der Zerlegung bis dorthin.
- **Ohne RLE:** Die erste Zerlegung ist nur LZ77, nicht LZ77 und RLE.
- **Ohne Analyse:** Ein Bild ohne Palette ruft `AnalyzeEntropy` nicht und
  nimmt `kSubGreen` gleich. Ausser bei method 6 mit quality 100, die alle
  Transformationen probiert und den Wert liest.
- **Der Kodierstand steigt** von 1 auf 2: Dieselben Pixel ergeben andere
  Bytes, siehe [0098](0098-der-zeichenstand-statt-des-builds.md).
- Ohne den Hinweis packt libwebp Byte für Byte wie das Original; schnell
  bleibt unberührt.

## Verworfene Alternativen

- **Eine kleinere feste Grösse:** Schon fest 8 Bit kostet 2,6 % Bytes von
  oben und 7,2 % im iso, 6 Bit 4,5 und 12,0 %, siehe
  [2026-11-13, Farb-Cache fest statt gesucht](../messungen/2026-11-13-farbcache-fest-statt-suchen.md).
- **Begrenzt suchen:** nie besser als dieselbe Grösse fest, gleiche Bytes,
  weniger gespart, ebenda.
- **Mehr Weglassen, nicht Teil dieser Entscheidung:** quality 51, weniger
  Felder für Histogramme, ohne TraceBackwards, method 0. Sie sind gemessen
  in [2026-11-14, libwebp für Kacheln zuschneiden](../messungen/2026-11-14-libwebp-zuschneiden.md)
  und nicht freigegeben.

## Folgen

- **Zeit des kompakten Packens:** −31,1 % von oben und −34,9 % im iso, an
  409 Kacheln der Testwelt, jede Stufe auf der davor, ebenda. Hochgerechnet
  rund 8 % der CPU-Zeit eines vollen Laufs von oben und rund 10 % im iso.
- **Platz:** +0,03 %. Fast leere Kacheln wachsen bis 9 %, je Kachel 50 bis
  110 Byte; die schlechteste von 1216 auf 1326 Byte.
- **Bilder mit Palette** bekommen den festen Cache und keine RLE; die Analyse
  bleibt bei ihnen.
- **Einmal neu kodieren:** Wegen des neuen Kodierstands kodiert jeder Lauf
  jede Kachel einmal neu, die er ablegt, ein Update also die seines Gebiets.
  Eine kompakte Kachel mit anderen Bytes bekommt dabei ein neues ETag, in der
  Messung zum Farb-Cache 42 von 409. Eine mit gleichen Bytes bleibt liegen,
  auch jede Kachel eines schnellen Baums. Kein voller Lauf nötig: Alte und
  neue Kacheln dekodieren zu denselben Pixeln.
- **Der Patch wächst** um zwei Dateien und geht bei jedem neuen
  `libwebp-sys` mit, siehe [libwebp mit Patch](../entwicklung/libwebp-mit-patch.md).
- **Ein Cache, wo libwebp keinen erlaubt,** verdürbe das Bild. Der Patch
  setzt darum nie mehr als den erlaubten Höchstwert;
  `kompakt_bleibt_verlustfrei` packt Bilder ohne Palette, mit Palette und
  mit höchstens 16 Farben und vergleicht die Pixel.
