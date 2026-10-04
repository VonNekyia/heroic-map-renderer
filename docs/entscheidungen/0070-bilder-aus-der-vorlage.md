---
title: "0070: Bilder aus der Vorlage"
description: Warum Rahmen, Tisch, Lilien und Gegenstände des Skins Tablett Ausschnitte der Vorlage sind, entzerrt oder freigestellt, und geglättet gezeichnet werden; warum jede Seite des Rahmens ein Streifen über ihre ganze Länge ist und der Tisch ein Bild der ganzen Platte statt Kacheln; löst in 0066 das Erzeugen der Bilder und in 0067 das Texelgitter ab.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/werkzeug/ausschnitte.py
  - web/skins/tablett/bilder
  - web/skins/tablett/bilder.ts
  - web/skins/tablett/tablett.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/index.ts
---

# 0070: Bilder aus der Vorlage

Löst in [0066](0066-texturen-des-tabletts.md) das Erzeugen der Bilder aus
Höhenkarten, Licht und Rampen ab, den Atlas je Dichte, das Legen nach dem
nächsten Nachbarn und das Malen des Schmucks in Aseprite. Löst in
[0067](0067-gesamtansicht-zwischen-zwei-stufen.md) das Texelgitter ab, also
1 px je Texel und das Einrasten des Rands; die Gesamtansicht zwischen zwei
Stufen bleibt.

In Teilen abgelöst durch
[0071](0071-tisch-und-gegenstaende-im-bezugsrahmen.md): Tisch und Füsse der
Gegenstände kommen nicht mehr über die Homographie der Vorlage auf die
Platte, sondern über die Umkehrung der Projektion im Bezugsrahmen; der Tisch
läuft am Rand der Vorlage nicht dunkel aus, sondern setzt sich gespiegelt
fort.

In Teilen abgelöst durch [0075](0075-marmor-als-pixelkunst.md): Der Marmor
liegt nicht mehr geglättet.

## Anlass

Der Maintainer will das Tablett genau so, wie die Vorlage es zeigt, und gibt
dafür die Vorlage frei: Ausschnitte von Büchern, Kerzen, Leuchter, Sphäre,
Kompass, Blumen, Ranken, Schmuck des Rahmens, Marmor und Holz des Tischs
dürfen ins Repository (#112, issuecomment-5974570397). Was das Skript aus
[0066](0066-texturen-des-tabletts.md) rechnete, blieb weit davon weg: der
Marmor braun statt grünschwarz mit goldenen Adern, der Rahmen ohne Lilien,
Relief und Efeu, die Gegenstände Kisten.

## Entscheidung

Vom Reviewer am 04.10. nachts festgelegt: Die Vorlage ist gemalt, keine
Pixelkunst. Was aus ihr kommt, wird geglättet gezeichnet; Gegenstände und
Schmuck sind freigestellte Ausschnitte, ein Bild für alle Kameras und
Richtungen, nie gespiegelt, denn ihr Licht ist eingebacken; Flächen werden
aus der Vorlage zurückprojiziert. Dazu vom Frontend:

- **Rahmen:** je Seite ein Streifen über ihre ganze Länge, das Band der
  Oberkante und an den nahen Seiten die Wand mit Relief, Efeu, Blüten,
  Rauten und Sockel. Das Skript entzerrt ihn per Homographie aus den
  gemessenen Kanten der Vorlage. Gekachelt wird nichts: Das Tablett wächst
  mit der Welt, jede Seite ist immer 1 / `RAND` = 62,5·w lang. Ferne Wände
  zeigt keine Kamera.
- **Pfeiler:** ihre Seiten zur Kamera aus dem vorderen Pfeiler der Vorlage,
  die Lilien darauf freigestellt, jede Ecke mit der Lilie, die sie in der
  Vorlage hat.
- **Ecken:** innen rund wie in der Vorlage. Je Ecke ein Eckstück auf dem
  Wasserspiegel, zurückprojiziert wie die Streifen; es deckt die Ecke der
  Karte und liegt deshalb mit allen vier Lilien vor den Kacheln.
- **Tisch:** ein Bild der ganzen Platte, so weit die Vorlage den Tisch
  zeigt, entzerrt auf seine Ebene: Marmor mit Adern und Glanz, der Rand aus
  geschnitztem Holz mit Gold, das Pergament, der Kerzenschein, die
  Schatten und die dunklen Ecken. Unter dem Tablett und unter den
  Gegenständen füllt das Skript Marmor auf. Zum Rand der Vorlage läuft das
  Bild aus; dahinter ist Grund.
- **Gegenstände:** freigestellt wie die Lilien, aufrecht auf ihrem Fuss auf
  der Platte.
- **Grösse und Lage** wie in der Vorlage, als Anteil der Breite der Karte:
  das Band 1,6 % der Kante, die Wand mit Sockel 5,4 Bänder, die Pfeiler 2,2.
  Die Gesamtansicht füllt mit dem Rahmen 92,5 % des Fensters statt 90 %, ihre
  Mitte liegt 5,8 % der Breite der Karte unter der Mitte der Karte.
- **Geglättet** mit `imageSmoothingQuality = 'high'`; ein Bild reicht 0,75 px
  über seine Fläche, so scheint an den Kanten nichts durch.
- **Geschnitten** einmal von Hand von `werkzeug/ausschnitte.py`, mit der
  Vorlage als Argument. Die Vorlage selbst liegt nicht im Repository, nur
  die Ausschnitte als WebP.
- **Tiefer Zoom:** Die Bilder haben die Auflösung der Vorlage. Ab etwa der
  Gesamtansicht + 2 werden sie weich; das nimmt der Reviewer hin.

Wie es gebaut ist: [Tablett](../tablett.md), „Bilder aus der Vorlage“.

## Abweichung von der Vorgabe

Der Reviewer sah Marmor und Holzrand als Stücke, die über die Fläche und
entlang der Kanten gekachelt werden. Das Frontend nimmt stattdessen das
Bild der ganzen Platte:

- Die Vorlage zeigt den ganzen Tisch, den man je sieht. Über ihren Rand
  hinaus zeigt sie nichts; dorthin läuft der Tisch dunkel aus wie ihre
  Ecken.
- Ein gekacheltes Stück wiederholte sein Licht, den Kerzenschein und die
  dunklen Ecken, und Stösse zeigten sich als Nähte. Licht zur Laufzeit
  träfe die Vorlage nur ungefähr.
- So liegt jede Ader, jeder Schatten und jede Spiegelung, wo sie in der
  Vorlage liegt.

## Verworfene Alternativen

- **Bilder aus einem Skript,** wie in [0066](0066-texturen-des-tabletts.md):
  Holz aus Höhenkarten, Marmor aus Rauschen, Gegenstände aus Formen. Sie
  sehen nie aus wie die gemalte Vorlage.
- **Marmor und Holzrand gekachelt:** siehe „Abweichung von der Vorgabe“.
- **Die ganze Vorlage als Kulisse,** mit einem Loch für die Karte: Die
  Vorlage hat eine leichte Perspektive, die Karte nicht. Ihre Ecken lägen
  bis 25 px neben denen der Karte.
- **Bilder je Kamera:** Die Vorlage zeigt nur 8:5 von Südost. Andere
  Kameras nehmen dieselben Bilder; die Flächen folgen ihrer Geometrie, die
  Lilien und Gegenstände stehen aufrecht.

## Folgen

- Die Vorlage hat die Seitenverhältnisse 0,59 und eine leichte Perspektive,
  die Karte in 8:5 0,625 und keine. Ihre Ecken weichen deshalb bis 2,5 %
  der Breite der Karte von denen der Vorlage ab; die rechte Ecke der
  Vorlage liegt 39 px tiefer als die linke.
- Gelände an den Ecken der Welt liegt unter den Eckstücken. An der fernen
  Ecke decken Eckstück und Lilie auch Gelände, das vor ihnen höher ragt.
- In anderen Kameras liegen Rahmen und Tisch auf ihrer Geometrie; was in
  der Vorlage steht, Lilien und Gegenstände, steht dort im Licht von 8:5.
- Wer die Ausschnitte ändert, schneidet neu und legt die Bilder mit in den
  Commit. Weder Build noch CI rufen das Skript; `tests/bilder.spec.ts`
  prüft, dass jedes Bild da ist und zu seiner Fläche passt.
