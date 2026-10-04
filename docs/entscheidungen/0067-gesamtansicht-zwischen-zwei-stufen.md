---
title: "0067: Gesamtansicht des Tabletts zwischen zwei Stufen"
description: Warum die Gesamtansicht mit dem Skin Tablett auch zwischen zwei Zoomstufen liegen darf, damit der Rahmen rund 90 % des Fensters füllt wie in der Vorlage, warum nur dort, wo Leaflet die Kacheln dabei verkleinert, warum ein Texel dort 1 px breit ist und der Rand darauf einrastet statt die Stufe, und wo die Ecken des Texelgitters liegen, damit jedes Texel ein Pixel bekommt und keine Pixelmitte auf einer Kante liegt.
status: gilt
date: 2026-10-03
issues: [112]
code:
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.css
---

# 0067: Gesamtansicht des Tabletts zwischen zwei Stufen

Löst [0061](0061-tablett-im-frontend.md) im Punkt „Gebrochene Zoomstufen“
ab, nur für die Gesamtansicht mit dem Skin.

In Teilen abgelöst durch [0070](0070-bilder-aus-der-vorlage.md): Die
Bilder sind gemalt und werden geglättet gelegt, ein Texelgitter gibt es
nicht mehr; der Rand rastet nicht mehr ein. Der Rahmen füllt 92,5 % wie in
der Vorlage statt rund 90 %. Die Gesamtansicht zwischen zwei Stufen, nur wo
Leaflet die Kacheln verkleinert, bleibt.

## Anlass

In ganzen Stufen füllt das Tablett die Hälfte bis das Ganze des Fensters,
an der Testwelt in 8:5 bei 1280, 1491 und 1920 px Breite 72, 61 und 48 %.
Die Vorlage zeigt es bei rund 90 %. Mit festen Bildern
([0066](0066-texturen-des-tabletts.md)) muss ein Texel dort zudem auf
Pixel passen, sonst werden Holz und Kanten ungleich (#112).

## Entscheidung

Entschieden vom Maintainer am 03.10. (#112, issuecomment-5971641826, und in
der Review zu #120): Die Gesamtansicht nimmt mit Skin die Zwischenstufe,
bei der ein Texel ganze Pixel deckt und das Tablett rund 90 % des Fensters
füllt. Die Kacheln werden dort nur verkleinert.

Wie ein Texel auf Pixel passt, hat der Reviewer am 03.10. festgelegt, eine
Regel für alle Kameras: Diagonal ist ein Texel waagrecht 1 px breit, die
Schritte sind (±1, a/h) und (0, b/h); in 2:1 also (1, 0,5) statt (2, 1).
1:1, von oben und genordet sind damit ganz. Die Ecken des Texelgitters
liegen waagrecht auf Pixelmitten und senkrecht 1/64 px neben dem
Pixelraster. Nachgerechnet an Pixelmitten, an inneren Texeln:

| Lage der Ecken | 8:5 | 2:1 | 4:3 | 16:9 | Pixelmitten auf Kanten |
|---|---|---|---|---|---|
| auf Pixelecken | 0 bis 2 px je Texel | 0 bis 2 | 0 bis 2 | 0 bis 2 | viele |
| waagrecht auf Pixelmitten | 1 bis 2 | 1 | 1 bis 2 | 1 bis 2 | viele |
| dazu senkrecht 1/64 px daneben | 1 bis 2 | genau 1 | 1 bis 2 | 1 bis 2 | keine |

Liegt eine Pixelmitte auf einer Kante, entscheidet das Runden des
Browsers, und Browser dürfen dort verschieden zeichnen.

Gebaut so:

- **Gesamtstufe:** die Stufe, auf der der Rahmen 90 % füllt, wenn Leaflet
  dort die Kacheln verkleinert, also ab einem Bruch von 0,5 und nicht über
  der feinsten Stufe. Sonst die nächste von ganzer Stufe darunter und
  halber Stufe darüber, auf der er noch passt. So füllt er 71 bis 100 %
  statt 50 bis 100 %, meist 90 %.
- **Nie unter der ganzen Stufe,** auf die Leaflet die Grenzen einpasst,
  damit die erste Ansicht und der Knopf ⌂ auf ihr landen.
- **1 px je Texel über den Rand:** Die Breite w rastet so ein, dass ein
  Texel waagrecht 1 px breit ist; sie weicht dafür höchstens ein halbes
  Texel von 1,3 % der Kante ab. Atlanten gibt es bis Dichte 16; ist der
  Rand breiter, auf Schirmen um 4K, ist ein Texel 2 px breit.
- **Ecken an Wänden auf Pixelkanten:** An Wänden laufen die Kanten der
  Texel senkrecht. Auf Pixelmitten läge jede Pixelmitte ihrer Spalte auf
  einer Kante; auf Pixelkanten deckt jedes Texel genau ein Pixel. Genordet
  ebenso.
- **Das Gitter rückt, nicht die Fläche:** Je Fläche rückt das Texelgitter
  um höchstens ein halbes Pixel je Richtung in diese Lage. Die Umrisse
  bleiben, wo sie sind, so entsteht keine Naht; was am Rand frei bliebe,
  deckt ein Texel Anschnitt.

Die Regel im Einzelnen: [Tablett](../tablett.md), „Ganze Pixel“.
- **Geglättet:** Zwischen zwei Stufen glättet der Browser die verkleinerten
  Kacheln, statt Pixel auszulassen.
- **Nur die Gesamtansicht:** Jede Stufe darüber bleibt ganz.

Wie es gebaut ist: [Tablett](../tablett.md), „Zeichnen“.

## Verworfene Alternativen

- **Ganze Stufen allein,** wie nach 0061. Der Rahmen füllte bei manchen
  Fenstern nur die Hälfte.
- **Die Stufe einrasten statt den Rand,** so dass ein Texel bei festem w
  ganze Pixel deckt. Die Hälfte jeder Stufe fällt weg, weil Leaflet dort die
  Kacheln vergrösserte; an der Testwelt bei 1280 px füllte der Rahmen dann
  rechnerisch nur 62 %.
- **Ganze Schritte auch in 8:5,** mit 8 px je Texel. Das Band der
  Oberkante, in der Gesamtansicht bei 1920 × 1080 px rund 10 px breit,
  hätte nicht einmal anderthalb Texel.
- **Ganze Schritte, wo sie mit bis zu 2 px je Texel gehen,** sonst nur
  waagrecht, vom Frontend vorgeschlagen. Zwei Regeln je nach Kamera; in
  2:1 wären die Texel doppelt so breit wie in 8:5.
- **Ecken senkrecht 1/16 px neben dem Raster.** In 16:9 lägen noch
  Pixelmitten auf Kanten.
- **Die Fläche rücken statt des Gitters.** Benachbarte Flächen rückten
  verschieden weit, und zwischen ihnen bliebe eine Naht.
- **Auch vergrössern,** um immer 90 % zu füllen. Vergrösserte Kacheln
  verwischen oder werden ungleich pixelig.
- **Die Kacheln der Stufe darüber erzwingen,** über `minNativeZoom` der
  Kachelebene. Die gehört der Grundkarte; ein Skin griffe in ihre Logik,
  und Leaflet lüde bis zu viermal so viele Kacheln.
- **Alle Stufen um denselben Bruch verschoben.** Dann wäre keine Stufe mehr
  auf ganzen Pixeln der Kacheln.

## Folgen

- Von einer gebrochenen Gesamtansicht führt + über die nächste ganze Stufe
  hinweg: Leaflet rundet z + 1 auf. − kommt über sie zurück.
- Ohne Animation rundet Leaflet jede Stufe auf eine ganze. Der Skin setzt
  die Untergrenze deshalb selbst und die Gesamtansicht mit `zoomSnap` 0,
  sofort; siehe [Tablett](../tablett.md), „Zeichnen“.
- Der Rand ist je Fenster ein wenig breiter oder schmaler, bei Dichte 8 bis
  6 %, auf Telefonen bis 25 %.
