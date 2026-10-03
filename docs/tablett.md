---
title: Tablett
description: Der Skin Tablett legt die Welt in ein Holztablett auf einem Tisch, nur für quadratische Karten, einmal für fitZoom vorgerendert in zwei Bilder um die Kacheln. Mit Massen nach der Vorlage, Profil, Licht und Schatten und der Regel, was vor und was hinter der Welt liegt.
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/tablett.css
  - web/skins/tablett/package.json
  - web/skins/tablett/tests/tablett.spec.ts
  - web/skins/tablett/tests/karte.spec.ts
  - web/skins/tablett/tests/auslagern.spec.ts
---

# Tablett

Der Skin in [`web/skins/tablett/`](../web/skins/tablett/) legt die Welt in
ein Tablett aus Holz auf einem Tisch (#112). Er zeichnet beides aus ebenen
Rechtecken, mit derselben Projektion wie die Karte, einmal für `fitZoom`.
Wie Skins eingebunden werden: [Frontend](frontend.md), „Skins“. Warum so:
[0061](entscheidungen/0061-tablett-im-frontend.md) und
[0063](entscheidungen/0063-tablett-als-skin.md). Stand: Geometrie mit
Profil, Licht und Schatten, in Flächenfarben. Texturen mit Höhenkarten und
die Gegenstände als Sprites kommen in eigenen PRs.

## Einschalten

- **Build:** `SKIN=./skins/tablett npm run build`.
- **Daten:** `seaLevel` und `area` aus `map.json`, geschrieben vom Backend
  (#112, #115). `area` ist `[x0, z0, x1, z1]` in Blöcken der Welt, `x1` und
  `z1` sind die Kanten hinter dem letzten Block. Ohne `minY` gilt −64.
- **Nur ein Quadrat:** Ist `area` kein Quadrat, bleibt das Tablett aus, und
  die Konsole sagt `Tablett: area ist kein Quadrat, das Tablett bleibt aus.`
  Ebenso bei unbrauchbaren Werten. Fehlen `seaLevel` oder `area`, bleibt es
  ohne Meldung aus.
- **Schnittstelle:** geschrieben für `VERSION` 1; bei einer anderen bleibt
  es aus und sagt es in der Konsole.

## Zeichnen

- **Einmal für `fitZoom`:** beim Laden und bei jeder neuen Grösse des
  Fensters, in Pixeln des Bildschirms, um die Mitte des Rahmens wie
  `fitBounds`. Je Seite reichen die Bilder ein Viertel des Fensters über
  das Fenster hinaus; weiter lässt sich auf `fitZoom` nicht ziehen
  (`maxBounds`). Die linke obere Ecke liegt auf ganzen Pixeln, so trifft
  jedes Pixel der Leinwand eines des Bildschirms.
- **Zwei Bilder:** `tablett-fern` unter den Kacheln (z-index 150) mit allem
  ausser dem Saum, `tablett-nah` darüber (250) nur mit den nahen Teilen,
  siehe „Vor und hinter der Welt“. Beide sind Leinwände als Bild-Ebenen der
  Karte (`L.svgOverlay`, das jedes Element nimmt). Ein Bild aus der
  Leinwand ginge nur über `data:` oder `blob:`, und das verbietet die
  Content-Security-Policy.
- **Beim Ziehen und Zoomen** zeichnet der Skin nichts. Die Bilder gleiten und
  wachsen mit der Karte, pixelig (`image-rendering: pixelated`).
- **Ausblenden:** ganz auf `fitZoom`, bis `fitZoom` + 1 linear auf 0, am Ende
  des Zooms verborgen. Die Deckkraft folgt auch dem Zoom mit zwei Fingern.
- **Kleinste Stufe:** `fitZoom`, neu bei jeder Grösse des Fensters.
- **Einpassen:** Der Skin meldet den Rahmen samt Pfeilern (`grenzen`) als
  ganze Karte. Darauf passen die erste Ansicht und der Knopf ⌂ ein.
- **Klicks** gehen durch beide Bilder hindurch (`pointer-events: none`).
- **Gemessen:** einmaliges Zeichnen und Bildzeit beim Ziehen in
  [Skin Tablett](messungen/2026-10-03-skin-tablett.md).

## Masse

Alle Masse sind Anteile der Welt und damit auf `fitZoom` Anteile der
Kartenbreite W, der Breite der Welt am Wasserspiegel im Bild. Die Kante ist
das Mittel aus Breite und Tiefe von `area`. Die Zahlen hat der Researcher
an der Vorlage gemessen (#112, issuecomment-5969026988). Umgerechnet für
2:1 und 8:5, mit b = h:

- **waagrecht** im Bild: Band / W = Breite / Kante;
- **senkrecht** im Bild: px / W = Höhe / (2 · Kante).

| Mass | Wert | Vorlage |
|---|---|---|
| Rand w | 1,3 % der Kante | Oberkante 1,6 % von W waagrecht; die Schrägen machen das Band in 2:1 1,25-mal, in 8:5 1,2-mal so breit wie w |
| Tiefe D bis zur Platte | 6,4·w, 8,3 % der Kante | Wand mit Sockel 4,0 % von W senkrecht, dazu der Abfall der Schrägen |
| Pfeiler an den Ecken | 2,7·w im Quadrat, 3,5 % der Kante | Pfosten 3,5 % von W breit |
| Holzkante des Tischs | 5 % der Kante | |

Das Profil des Rahmens im Schnitt, von der Kante der Welt nach aussen, Höhen
ab dem Wasserspiegel. Jede Stufe ist eine eigene Fläche:

| Stufe | nach aussen | Höhe | Vorlage, senkrecht |
|---|---|---|---|
| Oberkante, flach | 0 bis 0,45·w | 0 | |
| drei Schrägen | bis w | bis −0,25·w | |
| obere Leiste | steht 0,1·w vor | bis −0,6·w | Fries mit beiden Leisten 2,2 % von W |
| Fries | w | bis −3,28·w | |
| untere Leiste | steht 0,1·w vor | bis −3,63·w | |
| Fuge | 0,1·w zurück, fast schwarz | bis −4,4·w | 0,5 % von W |
| Sockel | steht 0,15·w vor | bis −D = −6,4·w | 1,3 % von W |

Nichts von Rahmen und Pfeilern liegt über dem Wasserspiegel. Die erhabene
Lippe der Vorlage kommt mit der Höhenkarte der Textur.

Der Tisch:
- **Hinten und an den Seiten** reicht er weit über das Fenster.
- **Vorn** liegt seine Kante an der Ansicht der ganzen Karte:
  - diagonal die vordere Ecke 4 % der Fensterhöhe unter der Mitte des
    unteren Rands;
  - genordet die Kante 10 % der Fensterhöhe über ihm.

  So laufen Holzkante und Zarge wie in der Vorlage durch die unteren Ecken.
  In ganzen Stufen füllt die Karte samt Rahmen die Hälfte bis das Ganze des
  Fensters; die Kante folgt dem. Nie liegt sie näher als 0,05 Kanten am
  Rahmen.
- **Von oben** gibt es keine Zarge zu sehen; dort reicht er überall weit.

Die Platzhalter der Gegenstände stehen an den Plätzen der Vorlage, in 2:1
von Südost: links oben Bücher, Pergament und Leuchter, rechts oben die Kerze,
links unten die Sphäre, rechts unten der Kompass auf einem Buch. Was vor der
Welt über den Wasserspiegel ragt, steht neben ihrem Bild.

## Licht und Schatten

- **Licht:** von oben, leicht von links im Bild, fest im Blick, so dass es
  aus jeder Richtung gleich aussieht. So hat es der Researcher an der
  Vorlage vermessen (#112, issuecomment-5969026988). Je Fläche gerechnet
  beim Bauen, als Farbe · (0,22 + 0,8 · max(0, n·l)):
  - l = 0,975 · oben − 0,223 · rechts, 77° über der Tischebene; oben ist die
    Normale der Platte, rechts die Richtung nach rechts im Bild;
  - 0,22 Umgebungslicht, 0,8 diffus nach der Normalen;
  - die Oberkante zeigt so ihre volle Farbe, die linke nahe Wand 0,35 davon,
    die rechte 0,22: links rund 1,6-mal so hell wie rechts, wie in der
    Vorlage.
  - Glanzlichter nach Blinn-Phong auf Messing und Gold kommen mit den
    Texturen.
- **Schatten auf die Platte:**
  - Rahmen und Gegenstände werfen ihn, jede Ecke entlang des Lichts auf die
    Ebene der Platte geworfen. Die konvexen Hüllen werden als ein Pfad
    gefüllt, so dunkeln Überlappungen nicht doppelt.
  - Deckkraft 0,55, weich mit 0,9·w, über den Schatten des Canvas.
  - Vor den Kacheln fällt er nur auf das, was dort schon gemalt ist, die
    nahen Stücke der Platte (`source-atop`). So glättet seine Kante wie
    ihre, und an der Grenze zur fernen Platte bleibt keine Linie.
- **Saum auf der Karte:**
  - Die Oberkante wirft einen schmalen Schatten auf die Karte, an den Seiten,
    über die das Licht auf sie fällt, also an den beiden linken im Bild.
  - 0,5·w breit, bis 0,4 Deckkraft, nach innen auslaufend.
  - Er liegt über den Kacheln und dunkelt dort auch Gelände leicht ab: die
    einzige Ausnahme von „Vor und hinter der Welt“.

## Vor und hinter der Welt

Eine Fläche ist nah und liegt im Bild über den Kacheln, wenn Gelände sie nie
verdecken kann:

- **Warum das reicht:** Ein Bildpunkt zeigt Gelände vor einem Punkt des
  Tabletts nur, wenn es entlang der Blickachse weiter vorn und höher liegt.
  - Diagonal ist die Achse (b, 2a, b): Das Gelände liegt bei grösserem x
    und z.
  - Genordet ist sie (0, a, b): grösseres z bei gleichem x.
- **Nah ist** im Blick jede Fläche, die ganz bei x ≥ x1 oder z ≥ z1 liegt,
  genordet auch ganz bei x ≤ x0. Das sind die Seiten des Rahmens zur
  Kamera, die vorderen Stücke von Platte und Zarge und die vorderen
  Gegenstände.
- **Kein Pixel** von Gelände über dem Wasserspiegel liegt deshalb unter
  einer nahen Fläche, in keiner Kamera und Richtung: Was nah ist, liegt
  nicht über dem Wasserspiegel, oder es steht neben dem Bild der Welt.
- **Der Schnitt** der Welt zur Kamera liegt ganz unter Rahmen, Platte und
  Zarge.
- **Die fernen Teile** liegen unter den Kacheln. Was dort über sie
  hinausragt, deckt sie richtig.
- **Gemalt** wird in einer festen Reihenfolge, ein späteres Teil deckt ein
  früheres:
  1. Zarge und Platte, ihr Schatten, der Boden des Tabletts;
  2. die Gegenstände hinter dem Rahmen;
  3. der Rahmen: ferne Ecke, ferne Seiten, seitliche Ecken, nahe Seiten,
     nahe Ecke; je Seite von aussen unten nach innen oben;
  4. die Gegenstände davor;
  5. der Saum.

  Gegenstände untereinander sind nach Tiefe sortiert.
- **Ohne Nähte:** Benachbarte Stücke derselben Ebene überlappen. Was nah
  ist, liegt auch im fernen Bild. So zeigt keine Kante den Hintergrund.
- **Ausnahmen:** der Saum, siehe „Licht und Schatten“; mit den Sprites
  dazu die Lilie an der nahen Ecke, die wie in der Vorlage ins Bild der
  Karte ragen darf ([0063](entscheidungen/0063-tablett-als-skin.md)).

## Tests

- [`tests/tablett.spec.ts`](../web/skins/tablett/tests/tablett.spec.ts)
  prüft die Geometrie an den Einträgen des Renderers, an Gelände bis fast an
  die Bauhöhe an jedem Rand zweier Welten, am Schnitt bis `minY`, daran,
  dass der Tisch das Fenster füllt, und an der Richtung des Lichts.
- [`tests/karte.spec.ts`](../web/skins/tablett/tests/karte.spec.ts) prüft
  im Browser die beiden Ebenen, das Ausblenden bis `fitZoom` + 1, `fitZoom`
  als kleinste Stufe, dass beim Ziehen und Zoomen nichts gezeichnet wird und
  dass ein `area`, das kein Quadrat ist, kein Tablett zeichnet.
- [`tests/auslagern.spec.ts`](../web/skins/tablett/tests/auslagern.spec.ts)
  baut die Karte mit einer Kopie des Skins aus einem Ordner ausserhalb des
  Repositorys.
