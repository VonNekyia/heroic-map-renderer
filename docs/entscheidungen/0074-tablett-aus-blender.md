---
title: "0074: Tablett aus Blender"
description: Warum das Brett des Skins Tablett aus einer Blender-Szene gerendert wird, einmal je Kamera und Richtung, fern und nah aus einem Render, als feste Bilder im Repository, und warum der Skin sie ohne Glättung auf Pixel des Geräts legt, in der Gesamtansicht mit ganzem Faktor, wo eine erlaubte Stufe ihn hat; warum nicht je Karte gerendert, nicht bis 50 % gefüllt, keine vergrösserten Kacheln und keine mehreren Dichten; löst mit der Lieferung der Szene 0070 und den Bildteil von 0071 ab, in 0067 die Wahl unter den erlaubten Stufen.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/werkzeug/brett.py
  - web/skins/tablett/werkzeug/brett_blender.py
  - web/skins/tablett/werkzeug/ganzer_faktor.py
  - web/skins/tablett/brett.ts
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.css
---

# 0074: Tablett aus Blender

Löst mit der Lieferung der Szene [0070](0070-bilder-aus-der-vorlage.md) ab,
und in [0071](0071-tisch-und-gegenstaende-im-bezugsrahmen.md), wie Tisch und
Gegenstände ins Bild kommen. Bis die Bilder in `brett/` liegen, gelten
beide.

Löst in [0067](0067-gesamtansicht-zwischen-zwei-stufen.md) die Wahl unter
den erlaubten Stufen ab, nur mit gerenderten Bildern: Die Gesamtansicht
nimmt dort eine Stufe mit ganzem Faktor, wo es eine gibt.

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
    ±1 px bei krummem Faktor sind hingenommen.
  - Ein Test prüft, dass keine Mischfarben entstehen.
- **Laden** nach den Kacheln wie in
  [0073](0073-bilder-nach-den-kacheln.md).
- **Bis zur Lieferung** zeigt die Seite die Bilder aus der Vorlage. Den
  Platzhalter gibt es nur in den Tests, nie in `bilder/` oder `brett/`.
- **Mit der Lieferung** gelten die gerenderten Bilder in allen Kameras
  zugleich; gemischt je Kamera wird nicht.

Vom User am 04.10. nach dem Vorschlag des Frontends, über den Reviewer:

1. **Die Stufe der Gesamtansicht** bleibt nach
   [0067](0067-gesamtansicht-zwischen-zwei-stufen.md): 92,5 % und die Regel
   für Kacheln, nie unter der ganzen Stufe, auf die Leaflet einpasst.
2. **Ganzes n:** Deckt ein Pixel des Bilds auf einer dieser erlaubten
   Stufen, die 71 bis 100 % füllt, ganze n ≥ 1 Pixel des Geräts, gilt sie;
   von mehreren die nächste an 92,5 %.
3. **Sonst** gilt der krumme Faktor, nach dem nächsten Nachbarn.
4. **Unter Faktor 1** wird geglättet, ohne zweiten Render.
5. **Leinwände** in Pixeln des Geräts.
6. **Ausschnitt:** Jedes Bild deckt Fenster von 9:20 hochkant bis 21:9 quer
   bei 71 % Füllung. Das kostet je Lieferung 41 bis 66 MB.

Dazu vom Frontend:

- **Fern ohne die nahen Pixel:** Nah liegt ohnehin darüber. Das spart bei
  der Szene rund 40 % der Bytes.
- **Fehlt eine Kamera** in `brett.json`, bleibt das Tablett aus, und die
  Konsole sagt es. Die Bilder aus der Vorlage nimmt der Skin dann nicht.
- **Nur die Bilder der eigenen Kamera** lädt der Skin, zwei Dateien.
- **Zwischen zwei Pixeln des Geräts,** bei `devicePixelRatio` 1,25 oder 1,5,
  nimmt der Browser je Pixel das nächste der Leinwand
  (`image-rendering: pixelated`). Ohne das glättete er dort, auch den
  Marmor aus [0075](0075-marmor-als-pixelkunst.md).

Wie es gebaut ist: [Tablett aus Blender](../tablett-gerendert.md).

## Wie oft ein ganzes n kommt

[`werkzeug/ganzer_faktor.py`](../../web/skins/tablett/werkzeug/ganzer_faktor.py)
rechnet die Regel nach, in 8:5:

- vier Welten mit 25 600, 4096, 1024 und 128 Blöcken Kante;
- 13 Fenster von 360 × 740 bis 3840 × 2160;
- `devicePixelRatio` 1, 1,25, 1,5, 2 und 3.

Das sind 260 Fälle. So oft hat eine Stufe ein ganzes n:

| Variante | Fälle mit ganzem n |
|---|---|
| **gilt:** 71 bis 100 %, Kacheln nur verkleinert, nicht unter der Stufe, auf die Leaflet einpasst | 62, 24 % |
| 71 bis 100 %, Kacheln nur verkleinert, auch unter dieser Stufe | 87, 33 % |
| 71 bis 100 %, Kacheln auch vergrössert | 168, 65 % |
| 50 bis 100 %, Kacheln nur verkleinert, auch unter dieser Stufe | 151, 58 % |
| 50 bis 100 %, Kacheln auch vergrössert | 224, 86 % |

- Mit 50 % statt 71 % als Untergrenze gibt die geltende Regel dieselben
  62: Unter der Stufe, auf die Leaflet einpasst, liegt sie nie, und die
  erlaubten Stufen darüber füllen fast immer mindestens 71 %.
- **Krumm** bleibt es etwa am Bildschirm mit 1280 × 720 und
  `devicePixelRatio` 1: Mit n = 1 füllte der Rahmen nur 65 %, mit n = 2
  ragte er aus dem Fenster.
- **Unter Faktor 1** liegt die Gesamtansicht etwa auf Telefonen mit
  `devicePixelRatio` 1 bis 1,5.

Die Zahlen „33 %“ und „58 %“ im Vorschlag an den User rechneten ohne die
Bedingung, nie unter der Stufe zu liegen, auf die Leaflet einpasst. Mit ihr
sind es 24 %.

## Verworfene Alternativen

- **Je Karte rendern:** Blender liefe beim Bau jeder Karte, und jede Karte
  hätte eigene Bilder. Das Brett hängt nur an Kamera und Richtung: Die
  Karte ist in der Szene immer 1 BU breit, das Bild wird mit ihr skaliert.
- **Bis 50 % gefüllt:** Ein ganzes n käme in 58 % der Fälle, aber nur, wenn
  die Gesamtansicht auch unter der Stufe liegen darf, auf die Leaflet
  einpasst. Jedes Bild müsste dafür ein doppelt so grosses Fenster decken:
  alle 64 Bilder 81 bis 130 MB, siehe
  [Grösse des gerenderten Bretts](../messungen/2026-10-04-brett-groesse.md).
- **Kacheln bis √2 vergrössern:** Ein ganzes n käme in 65 % der Fälle,
  dafür würden die Kacheln der Gesamtansicht vergrössert. Das schliesst
  [0067](0067-gesamtansicht-zwischen-zwei-stufen.md) aus.
- **Mehrere Dichten:** je Kamera Bilder in mehreren Massstäben, damit
  überall ein ganzes n passt. Das vervielfachte die Bilder im Repository.

## Folgen

- **Grösse:** alle 64 Bilder 41 bis 66 MB, ein Blick in 8:5 1,2 bis 2,0 MB,
  hochgerechnet in
  [Grösse des gerenderten Bretts](../messungen/2026-10-04-brett-groesse.md).
- **Krumme Faktoren:** In rund drei von vier Fällen bleibt der Faktor in
  der Gesamtansicht krumm. Pixel des Bilds sind dort um 1 px ungleich
  breit; ab der nächsten Stufe sind sie meist mehrere Pixel gross.
- **Tiefer Zoom:** Ein Pixel des Bilds wird mit der Welt gross. Bei einer
  Welt von 10 000 Blöcken Kante und scale 32 deckt es auf der feinsten
  Stufe rund 500 × 500 Pixel.
- **Wer die Szene ändert,** rendert alle Kameras neu und legt alle Bilder
  in einen Commit.
- **Mit der Lieferung** kommen aus den Objekten der Szene noch der Text auf
  den Buchrücken und die Kästen, denen die UI ausweicht.
