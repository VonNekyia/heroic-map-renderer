---
title: "0061: Tablett und Tisch zeichnet das Frontend"
description: Warum das Frontend Rahmen, Tisch und Gegenstände um die Karte selbst aus ebenen Flächen zeichnet, in zwei Ebenen um die Kacheln, mit der Welt wachsend, mit Licht über Höhenkarten und Schmuck als Sprites, und warum weder der Renderer noch Bilder je Perspektive noch gebrochene Zoomstufen.
status: gilt
date: 2026-10-03
issues: [112]
code:
  - web/src/tablett.ts
  - web/src/main.ts
---

# 0061: Tablett und Tisch zeichnet das Frontend

## Anlass

Am Rand der Welt zeigt die Karte ihren Schnitt, dunklen Stein bis `minY`.
Der Maintainer will die Welt stattdessen in einem Tablett aus Holz auf einem
Tisch zeigen, in jeder Kamera und Richtung, auch von oben. Gelände über dem
Wasserspiegel soll dabei frei bleiben. Issue #112.

## Entscheidung

Das Frontend zeichnet Tablett, Tisch und Gegenstände selbst, entschieden vom
Maintainer am 03.10.:

- **Aus ebenen Rechtecken**, mit derselben Projektion wie die Karte. In einer
  Parallelprojektion geht jedes Rechteck affin aufs Bild. Ein Satz Flächen
  passt deshalb zu jeder Kamera und Richtung.
- **In zwei Ebenen um die Kacheln:** unter ihnen alles, über ihnen nur, was
  Gelände nie verdecken kann. So deckt das Tablett den Schnitt, und Gelände
  über dem Wasserspiegel bleibt frei, ohne dass das Frontend die Welt kennen
  muss. Die Regel: [Frontend](../frontend.md), „Vor und hinter der Welt“.
- **Texturen** werden wie die Blöcke der Karte nach dem nächsten Nachbarn
  abgetastet.
- **Beim Hineinzoomen** blenden Rahmen und Tisch aus.

Dazu entschied der Maintainer am 03.10. (#112, issuecomment-5968793257):

- **Grösse:** Das Tablett wächst mit der Welt. Rand, Wand und Tiefe der
  Platte sind ein fester Anteil von ihr, so wie im Vorbild.
- **Tiefe über Karten:** Jede Textur bekommt eine Höhenkarte. Aus ihr kommen
  die Normalen, und mit ihnen ein Licht von oben, fest im Blick. Richtig
  gestellt nach der Vermessung der Vorlage: leicht von links, 77° über der
  Tischebene (issuecomment-5969026988), nicht von rechts. Das Frontend schattiert jede Fläche einmal je Kamera beim Laden.
  Mehr Geometrie gibt es nur, wo die Silhouette sie braucht: das Profil des
  Rahmens, je Stufe eine Fläche.
- **Schatten:** Rahmen und Gegenstände werfen weiche Schatten auf die Platte,
  die Oberkante einen schmalen auf die Karte.

Aus den Zahlen des Researchers, festgehalten in der Review zu #114:

- **Schmuck und Gegenstände** sind aufrechte Sprites in Pixeln des
  Bildschirms, je Zoomstufe eigens gepixelt. Als Textur wären Lilien und
  Blätter erst auf Stufen erkennbar, auf denen das Tablett schon ausblendet.
- **Ausblenden:** ganz bis `fitZoom` samt Rahmen, eine Stufe darüber halb,
  danach nicht mehr.

## Verworfen

- **Der Renderer zeichnet das Tablett in die Kacheln.** Es bräuchte Zeit des
  Backends, die erst nach dem Vollrender und #100 frei wird. Jede Änderung
  am Tablett hiesse einen neuen Lauf, und ausblenden liesse es sich nicht.
- **Ein Bild je Perspektive, von Hand gepixelt.** Bei Kameras von 2:1 bis
  1:1, von oben und genordet, je vier Richtungen wären das Dutzende Sätze.
  Jede neue Kamera bräuchte einen weiteren.
- **Eine Ebene über den Kacheln** mit allem. Sie deckte Gelände am fernen
  Rand, das über die Oberkante ragt.
- **Gebrochene Zoomstufen,** damit die Karte samt Rahmen das Fenster füllt
  wie im Vorbild. Die Kacheln verlören die ganzen Pixel. In ganzen Stufen
  füllt sie die Hälfte bis das Ganze; die Vorderkante des Tischs folgt dem.
