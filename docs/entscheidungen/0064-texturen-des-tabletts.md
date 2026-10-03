---
title: "0064: Texturen des Tabletts im Worker, Schmuck gemalt"
description: Warum der Skin Tablett seine Flächen aus Höhenkarten, Licht und den gemessenen Rampen erzeugt und den Schmuck malen lässt, warum ein Worker rechnet und malt, während zuerst Flächenfarben stehen, und warum die Adern des Marmors nicht in der Kachel liegen.
status: gilt
date: 2026-10-03
issues: [112]
code:
  - web/skins/tablett/stoffe.ts
  - web/skins/tablett/werkstatt.ts
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/index.ts
---

# 0064: Texturen des Tabletts im Worker, Schmuck gemalt

## Anlass

Nach [0063](0063-tablett-als-skin.md) zeichnete das Tablett nur in
Flächenfarben. Die Vorlage zeigt Maserung, Fasen, eine erhabene Lippe,
vertiefte Felder, Messingnägel und Marmor mit Adern (#112).

## Entscheidung

Entschieden vom Maintainer am 03.10. (#112, issuecomment-5970177877):

- **Erzeugt im Skin:** je Fläche in genau ihrer Pixelgrösse, aus
  Höhenkarte, Licht und den gemessenen Farbrampen der Vorlage: Maserung,
  Fasen, Lippe, vertiefte Felder, Messingnägel und Marmor. So bleibt alles
  bei jeder Fenstergrösse scharf.
- **Gemalt in Aseprite:** was nach Handwerk aussehen soll, Ranken an der
  Wand, Rauten, Lilien, Blätter, Blüten und Gegenstände, in zwei bis drei
  Grössen, verkleinert nur um ganze Faktoren.

Dazu, vom Frontend:

- **Im Worker:** Ein Worker im Skin rechnet die Texturen und malt beide
  Bilder. Bis seine Antwort da ist, stehen die Flächen in ihrer Farbe;
  dann tauscht der Skin die Bilder einmal ein, nie während die Karte sich
  bewegt.
- **Nah aus fern:** Mit Texturen ist das nahe Bild das ferne, beschnitten
  auf die Umrisse der nahen Teile. Jedes Stück wird nur einmal gerechnet
  und gemalt.
- **Adern über die ganze Leinwand,** als Risse mit Ästen, nicht in der
  Kachel des Marmors.
- **Vier Proben je Pixel** nur für schmale Rollen, Oberkante und Leisten:
  Sie mitteln das Licht, die Farbe bleibt eine der Rampe.
- **Keine Kartuschen im Fries:** Bei einem Rand von rund 12 px zerfielen
  sie in Striche. Die Felder bleiben, die Ranken darin werden gemalt.

Wie es gebaut ist: [Tablett](../tablett.md), „Texturen“ und „Zeichnen“.

## Verworfene Alternativen

- **Gemalte Kacheln für die Flächen.** Sie wären nur bei einer Grösse des
  Fensters scharf. Der Maintainer hat für erzeugte entschieden.
- **Rechnen im Hauptthread.** Die Arbeit des Workers dauert bei CPU 4×
  rund 0,37 bis 0,49 s, siehe
  [Texturen des Tabletts](../messungen/2026-10-03-texturen-tablett.md). Als
  ein Stück blockierte sie das Laden; Lighthouse lässt höchstens 300 ms
  Total Blocking Time zu ([`web/lighthouserc.cjs`](../../web/lighthouserc.cjs)).
- **Zeitscheiben im Hauptthread.** Sie hielten die Aufgaben kurz, aber die
  Arbeit bliebe auf dem Hauptthread neben den Kacheln, und `setTimeout`
  wartet nach wenigen Runden mindestens 4 ms. Der Worker rechnet daneben.
- **Das nahe Bild eigens malen,** wie ohne Texturen. Dann rechnete und
  malte jedes nahe Stück zweimal.
- **Adern in der Kachel.** Sie wiederholten sich alle 512 px, und gerade
  seltene Adern fallen dabei auf.
- **Vier Proben überall.** Nur schmale Rollen zerfallen in Punkte; überall
  kostete es ein Mehrfaches der Zeit.

## Folgen

- Kurz nach dem Laden wechselt das Tablett einmal von Flächenfarben zu
  Texturen, bei CPU 1× nach rund 0,2 s.
- Die Zeit im Worker misst CDP nicht gedrosselt; die Messung stellt seine
  Arbeit für CPU 4× auf dem Hauptthread nach.
- Der Skin hat eine Datei mehr, den Worker. Bilder für Texturen gibt es
  keine.
- Ohne Worker, oder wenn er scheitert, bleibt es bei den Flächenfarben,
  und die Konsole sagt es.
