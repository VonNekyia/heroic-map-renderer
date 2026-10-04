---
title: "0066: Texturen des Tabletts als feste Bilder"
description: Warum Holz, Messing und Marmor des Skins Tablett feste PNG-Bilder sind, die ein Skript einmal mit eingebackenem Licht erzeugt, und der Skin sie zur Laufzeit nur je Fläche mit setTransform und drawImage nach dem nächsten Nachbarn legt; verworfen sind das Erzeugen zur Laufzeit im Worker, eine Schleife je Pixel und Bilder je Kamera und Richtung.
status: gilt
date: 2026-10-03
issues: [112]
code:
  - web/skins/tablett/bilder
  - web/skins/tablett/zeichnen.ts
  - web/skins/tablett/index.ts
---

# 0066: Texturen des Tabletts als feste Bilder

In Teilen abgelöst durch [0070](0070-bilder-aus-der-vorlage.md): Die
Bilder sind Ausschnitte der Vorlage statt aus Höhenkarten, Licht und Rampen
erzeugt, je Fläche eines statt eines Atlas je Dichte, auch Lilien und
Gegenstände, und sie werden geglättet gelegt statt nach dem nächsten
Nachbarn. Es bleiben feste Bilder, die der Skin je Fläche mit
`setTransform` und `drawImage` legt.

## Anlass

Nach [0063](0063-tablett-als-skin.md) zeichnete das Tablett nur in
Flächenfarben. Die Vorlage zeigt Maserung, Fasen, eine erhabene Lippe,
vertiefte Felder, Messingnägel und Marmor mit Adern (#112). Ein erster Bau
in #120 erzeugte all das zur Laufzeit aus Höhenkarten, in einem Worker.

## Entscheidung

Entschieden vom Maintainer am 03.10. (#112, issuecomment-5971641826): Das
Tablett ist ein statisches Bild, also kommen seine Texturen aus festen
Bildern.

- **Feste Bilder** im Ordner des Skins, als PNG: Oberkante, Wände mit
  Sockel, Pfeiler und ihre Deckel, die Holzkante des Tischs und der Marmor
  als Kachel. Licht und Tiefe sind eingebacken; das Licht kommt fest von
  oben links.
- **Erzeugt einmal von einem Skript** im Ordner des Skins, nicht im Build.
  Es rechnet aus Höhenkarten, Licht und den gemessenen Rampen der Vorlage,
  wie es der erste Bau zur Laufzeit tat. Seine Ausgabe liegt im Repository.
- **Gemalt in Aseprite:** was nach Handwerk aussehen soll, Ranken an der
  Wand, Rauten, Lilien, Blätter, Blüten und Gegenstände, als Sprites in
  eigenen PRs.
- **Zur Laufzeit** legt der Skin die fertigen Bilder nur auf die Flächen:
  je Fläche eine affine Abbildung mit `setTransform`, dann `drawImage`, mit
  `imageSmoothingEnabled = false`, einmal beim Laden in die beiden
  Leinwände. Kein Worker, keine Höhenkarten, kein Schleier, keine Adern und
  keine Schleife je Pixel in JS. So hat es der Reviewer am 03.10. bestätigt.
- **Scharf** bleibt es über die Gesamtansicht aus
  [0067](0067-gesamtansicht-zwischen-zwei-stufen.md): Dort ist ein Texel
  waagrecht 1 px breit, und jedes deckt mindestens ein Pixel.

Dazu, vom Frontend:

- **Ein Atlas je Dichte,** 2 bis 16 Texel je w. Der Skin nimmt den, mit
  dem ein Texel in der Gesamtansicht 1 px breit ist, und rastet w darauf
  ein. Den Plan des Atlas teilen Skin und Skript in einer Datei.
- **Je Fläche ein Bild:** Das Profil des Rahmens liegt im Bild von
  Oberkante und Wand statt in einer Fläche je Stufe. Schmale Stufen ergäben
  sonst Bruchteile von Texeln, und deren Raster liefen nicht durch.
- **Bilder, die sich wiederholen:** Band, Wand und Holzkante wiederholen
  sich alle 20·w, die Stösse des Frieses kommen als eigenes Bild auf ganze
  Texel, durchsichtig, wo sie die Wand nicht ändern. So wird kein Bild auf
  die Länge einer Seite gestreckt.
- **Licht aus einer Kamera je Blick:** diagonal 8:5, genordet 45°.
- **Pixelkunst:** PNG mit Palette, nur Farben der Rampen; Adern als Pixel,
  höchstens 2 px breit.
- **Licht über die ganze Platte** bleibt ein Verlauf zur Laufzeit, wie
  Schatten und Saum. Die Kachel wiederholt sich, dieses Licht nicht.

Wie es seit [0070](0070-bilder-aus-der-vorlage.md) gebaut ist:
[Tablett](../tablett.md), „Bilder aus der Vorlage“ und „Zeichnen“.

## Verworfene Alternativen

- **Erzeugen zur Laufzeit,** in einem Worker, wie im ersten Bau in #120:
  je Fläche in genau ihrer Pixelgrösse, bei jeder Fenstergrösse scharf.
  Zu viel Aufwand für ein statisches Bild: ein Worker, Höhenkarten, Rampen
  und Marmor im Bündel, ein Tausch nach der Bewegung, bei CPU 4× rund 0,4 s
  Arbeit beim Laden, gemessen in #120.
- **Legen in einer Schleife je Pixel,** in JS: Jedes Pixel einer Fläche
  sucht das Texel unter seiner Mitte. Dasselbe tut der Browser mit
  `drawImage`, ohne eigenen Code je Pixel.
- **Bilder schon im Blick,** je Kamera und Richtung vorgerechnet, so dass
  jedes Pixel genau ein Texel ist. Das wären Bilder je Kamera, Richtung und
  Dichte.
- **Ein Bild je Fläche in jeder Fenstergrösse.** Es gäbe unendlich viele;
  die Dichten mit eingerastetem w decken jede Grösse.
- **Ein Bild auf die Länge seiner Seite strecken.** Mit eingerastetem w ist
  eine Seite nicht genau 76,9·w lang; gestreckt fehlte oder doppelte sich
  etwa jede sechzehnte Spalte.
- **Glätten beim Legen,** um Bilder einer Dichte auf jede Grösse zu
  bringen. Das Holz verwischte, und die Kanten der Leisten verlören ihre
  Pixel.
- **Eine Fläche je Stufe des Profils** mit eigenem Bild. Bänder von 0,1·w
  sind bei Dichte 8 kein ganzes Texel.
- **Das Licht über die Platte in die Kachel.** Die Kachel wiederholt sich;
  ein Bild der ganzen Platte wäre für jede Fenstergrösse ein anderes.

## Folgen

- Die Bilder liegen im Repository, alle Atlanten und der Marmor; laden muss
  der Browser nur den Atlas seiner Dichte und den Marmor. Die Grössen nennt
  das Skript.
- Wer Stoffe, Höhenkarten oder den Plan des Atlas ändert, erzeugt die
  Bilder neu. Weder Build noch CI rufen das Skript; ein Test prüft, dass
  jeder Atlas zum Plan passt.
- Das Tablett erscheint, sobald die Bilder geladen sind, in einem Zug,
  ohne Flächenfarben vorher.
- Was dabei eine Näherung bleibt, etwa die Oberseiten in 8:5: [Tablett](../tablett.md),
  „Was bleibt eine Näherung“.
