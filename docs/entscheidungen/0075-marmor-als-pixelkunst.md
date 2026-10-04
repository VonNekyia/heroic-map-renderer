---
title: "0075: Marmor als Pixelkunst"
description: Warum der Marmor jenseits der Vorlage bis zur gerenderten Szene Pixelkunst aus Blöcken von 2 × 2 Pixeln in 20 Farben ist und ohne Glättung liegt, solange ein Block ein Pixel deckt; löst in 0070 das Glätten des Marmors und in 0071 den Marmor aus Flicken ab.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/bilder/marmor.webp
  - web/skins/tablett/bilder.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/werkzeug/ausschnitte.py
---

# 0075: Marmor als Pixelkunst

Löst in [0070](0070-bilder-aus-der-vorlage.md) ab, dass der Marmor
geglättet liegt, und in [0071](0071-tisch-und-gegenstaende-im-bezugsrahmen.md),
dass er das Quadrat aus Flicken der Platte selbst ist.

## Anlass

Das Tablett soll Pixelkunst werden, scharf statt verwischt (#112). Die
Szene dafür baut ein Artist in Blender. Bis sie kommt, bekommt der Marmor
jenseits der Vorlage einen Pixelfilter: Er ist dort die grösste Fläche und
verwischte geglättet beim Hineinzoomen.

## Entscheidung

- **Bild:** `marmor.webp`, 768 × 768 px, verlustfrei, nahtlos, aus
  einfarbigen Blöcken von 2 × 2 Pixeln (`MARMOR_PIXEL` in
  [`bilder.ts`](../../web/skins/tablett/bilder.ts)) in 20 Farben.
- **Verfahren,** aus dem bisherigen Marmor aus Flicken:
  - Ader ist ein Pixel, das heller als sein Umfeld und warm ist, oder
    deutlich golden.
  - Je Block gilt das Mittel seiner vier Pixel. Läuft eine Ader durch den
    Block, gilt ihr goldenstes Pixel, das mit dem grössten Abstand von Rot
    zu Blau; das hellste wäre blass beige.
  - Jeder Block nimmt die nächste von 20 Farben in OKLab, ohne Dithering:
    Grund eine von 14 Tönen des Grunds, Adern eine von 6 Tönen der Adern.
    Die Töne des Grunds sind Mittel von Gruppen ähnlicher Blöcke, die der
    Adern je Stufe der Helligkeit aus ihren sattesten Pixeln.
  - Die Blöcke liegen im Raster ab der Ecke, 768 ist durch 2 teilbar: So
    bleibt der Marmor nahtlos.
- **Zeichnen:** ohne Glättung, solange ein Block mindestens ein Pixel der
  Leinwand deckt, kleiner geglättet (`lege` in
  [`zeichnen.ts`](../../web/skins/tablett/zeichnen.ts)). Alle anderen Bilder
  liegen weiter geglättet.
- **Werkzeug:** [`ausschnitte.py`](../../web/skins/tablett/werkzeug/ausschnitte.py)
  schreibt den Marmor nicht mehr. Seinen Marmor aus Flicken braucht es nur
  noch für die Löcher im Tisch.
- **Übergang:** So bleibt es, bis die gerenderte Szene den Tisch bringt.

## Verworfene Alternativen

- **Geglättet wie bisher:** verwischt beim Hineinzoomen.
- **Immer ohne Glättung:** Auf Telefonen ist ein Block in der Gesamtansicht
  kleiner als ein Pixel. Dann fielen Blöcke aus, und der Marmor flimmerte.
- **Je Block nur das Mittel, dann die nächste Farbe der ganzen Palette:**
  Dünne Adern verblassen im Mittel zu Braun und reissen ab. Mit ihrem
  goldensten Pixel bleiben sie golden und ganz. Der Maintainer wählte diese
  Variante.

## Folgen

- Der Marmor wiegt 76 KB statt 122 KB.
- Am Rand der Vorlage läuft der gemalte, geglättete Tisch in Pixelkunst
  aus: zwei Stile nebeneinander, bis die Szene kommt.
- Bei krummen Faktoren sind Blöcke um 1 px ungleich breit.
- Bei `devicePixelRatio` über 1 zieht der Browser die Leinwand geglättet
  auf Gerätepixel hoch; dort kommen die Blöcke mit weichen Kanten an, bis
  die Leinwände in Gerätepixeln malen. Das tun sie seit
  [0074](0074-tablett-aus-blender.md).
- Wer die Bilder neu schneidet, ändert den Marmor nicht.
