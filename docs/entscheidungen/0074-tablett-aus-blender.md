---
title: "0074: Tablett aus Blender"
description: Warum das Brett des Skins Tablett aus einer Blender-Szene gerendert wird, einmal je Kamera und Richtung, fern und nah aus einem Render, als feste Bilder im Repository, und warum der Skin sie ohne Glättung auf Pixel des Geräts legt; warum nicht je Karte gerendert; löst mit der Lieferung der Szene 0070 und den Bildteil von 0071 ab.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/werkzeug/brett.py
  - web/skins/tablett/werkzeug/brett_blender.py
  - web/skins/tablett/brett.ts
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.css
---

# 0074: Tablett aus Blender

Löst mit der Lieferung der Szene [0070](0070-bilder-aus-der-vorlage.md) ab,
und in [0071](0071-tisch-und-gegenstaende-im-bezugsrahmen.md), wie Tisch und
Gegenstände ins Bild kommen. Bis die Bilder in `brett/` liegen, gelten
beide.

## Anlass

Die Bilder aus der Vorlage bleiben eine Näherung, siehe
[Tablett](../tablett.md), „Was bleibt eine Näherung“:
- Die Kamera der Vorlage ist nicht 8:5.
- Andere Kameras nehmen dieselben Bilder; Lilien und Gegenstände stehen im
  Licht von 8:5.
- Ab etwa der Gesamtansicht + 2 werden die Bilder weich.

Der Maintainer will das Tablett genau wie die Vorlage, in jeder Kamera. Ein
Artist baut die Szene dafür in Blender nach (#112).

## Entscheidung

Vom Maintainer: feste Bilder im Repository. Vom Reviewer festgelegt:

- **Rendern** einmal je Kamera und Richtung, alle Kameras des Renderers mit
  allen Richtungen. Fern unter den Kacheln und nah darüber kommen aus
  demselben Render.
- **Bilder** ohne Stempel, neu geschrieben ohne Metadaten und ohne `pHYs`,
  WebP verlustfrei.
- **Lage:** Der Skin legt sie aus der Projektion über die Ecken der Karte.
- **Scharf:**
  - Die Leinwände haben Pixel des Geräts, beide, auch für die Bilder aus
    der Vorlage.
  - Ab einem Faktor von 1 nach dem nächsten Nachbarn; ungleiche Breiten von
    ±1 px bei gebrochenem Faktor sind hingenommen.
  - Ein Test prüft, dass keine Mischfarben entstehen.
- **Laden** nach den Kacheln wie in
  [0073](0073-bilder-nach-den-kacheln.md).
- **Bis zur Lieferung** zeigt die Seite die Bilder aus der Vorlage. Den
  Platzhalter gibt es nur in den Tests, nie in `bilder/` oder `brett/`.
- **Mit der Lieferung** gelten die gerenderten Bilder in allen Kameras
  zugleich; gemischt je Kamera wird nicht.

Dazu vom Frontend:

- **Fern ohne die nahen Pixel:** Nah liegt ohnehin darüber. Das spart bei
  der Szene rund 40 % der Bytes.
- **Fehlt eine Kamera** in `brett.json`, bleibt das Tablett aus, und die
  Konsole sagt es. Die Bilder aus der Vorlage nimmt der Skin dann nicht.
- **Nur die Bilder der eigenen Kamera** lädt der Skin, zwei Dateien.

Wie es gebaut ist: [Tablett aus Blender](../tablett-gerendert.md).

## Verworfene Alternativen

- **Je Karte rendern:** Blender liefe beim Bau jeder Karte, und jede Karte
  hätte eigene Bilder. Das Brett hängt nur an Kamera und Richtung: Die
  Karte ist in der Szene immer 1 BU breit, das Bild wird mit ihr skaliert.

## Folgen

- **Grösse:** alle 64 Bilder 41 bis 66 MB, ein Blick in 8:5 1,2 bis 2,0 MB,
  hochgerechnet in
  [Grösse des gerenderten Bretts](../messungen/2026-10-04-brett-groesse.md).
- **Tiefer Zoom:** Ein Pixel des Bilds wird mit der Welt gross. Bei einer
  Welt von 10 000 Blöcken Kante und scale 32 deckt es auf der feinsten
  Stufe rund 500 × 500 Pixel.
- **Wer die Szene ändert,** rendert alle Kameras neu und legt alle Bilder
  in einen Commit.
- **Mit der Lieferung** kommen aus den Objekten der Szene noch der Text auf
  den Buchrücken und die Kästen, denen die UI ausweicht.
