---
title: "0147: Gefälle an der höheren Spalte"
description: Warum das Relief von oben eine Stufe nur der höheren Spalte gibt, doppelt, statt die mittlere Differenz der Nachbarn zu nehmen; was das an schmalen Dingen, Hängen und Terrassen ändert, was verworfen ist und was es kostet; ergänzt 0122.
status: gilt
date: 2026-11-19
issues: []
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/stand.rs
---

# 0147: Gefälle an der höheren Spalte

## Anlass

Der User am 18.11. über das Relief mit `slope`, an Bildern einer Palme mit
Wedeln von einem Block Breite: „gibts ein problem mit dem slope shading? soll
die palme oben heller sein und unten dunkler das sieht diffus aus“, dazu
„auch bäume haben das problem“, „und kisten“, „und kaktus“.

Die mittlere Differenz, h Ost − h West + h Süd − h Nord, verteilt eine
Stufe auf zwei Spalten, die obere und die untere. An einem Ding von einem
Block Breite heben sich die Seiten an ihm selbst auf; hell im Nordwesten
und dunkel im Südosten wird nur der Boden daneben. Um jedes schmale Ding
lag so ein Kreuz auf dem Boden.

Am 19.11. wählte der User nach Vorschaubildern einer Szene mit Palme,
Waldrand, Kisten, Kakteen, Hang und Dach, je heute, B und C nebeneinander:
„B does look the best“.

## Entscheidung

- **Nur die höhere Spalte bekommt die Stufe.** Je Nachbar zählt, um wie viel
  die Spalte höher ist: nach Westen und Norden zum Licht hin hell, nach
  Osten und Süden dunkel, doppelt:
  n = 2 · (max(0, h − h West) + max(0, h − h Nord) − max(0, h − h Ost) −
  max(0, h − h Süd)). Wie es rechnet, steht in
  [Relief von oben](../renderer/relief.md), „Gefälle“.
- **n bleibt in −4 bis 4,** der Code der Schicht und der Faktor daraus
  bleiben. Der Mod rechnet wie bisher, siehe
  [0132](0132-relief-als-schicht.md).
- **`ZEICHENSTAND_RELIEF` steigt von 3 auf 4.** Nur Bäume mit Relief
  zeichnen neu.
- **`--flat` bleibt,** wie es ist: Sein Relief nimmt nur den Nachbarn im
  Norden, wie die Karte des Spiels, und färbt keinen Boden.

## Was sich ändert

- **Ein Ding von einem Block Breite,** Stamm, Wedel, Kiste, Kaktus,
  Laterne: Es selbst und der Boden um es bleiben, wie sie sind.
- **Ein breiteres Ding,** Krone, Dach, Haus: Seine Kante im Nordwesten wird
  hell, die im Südosten dunkel; der Boden daneben bleibt.
- **Ein Hang mit einem Block je Spalte:** gleich wie vorher, Pixel für
  Pixel.
- **Flachere Hänge und Terrassen:** Die ganze Stufe liegt auf einer Spalte
  statt die Hälfte auf zweien. Die Kanten werden schärfer.
- **Mit `shadows`** fällt der Schatten wie vorher; nur das Gefälle darunter
  ändert sich.

## Verworfene Alternativen

- **Die mittlere Differenz, wie bisher:** ein Schein auf dem Boden und ein
  Kreuz um jedes schmale Ding.
- **C, die Differenz nur zu den Nachbarn im Westen und Norden:** Das hohe
  Ding wird hell, der Boden im Südosten daneben dunkel, wie ein kurzer
  Schatten. Der User wählte B.
- **B ohne den Faktor 2:** Jeder Hang wäre halb so plastisch wie vorher.

## Folgen

- Jeder Baum mit Relief zeichnet beim nächsten Lauf einmal ganz neu, und
  jeder Spieler mit dem Mod lädt Kacheln und Schicht dieser Bäume einmal
  neu.
- Platz: siehe [Relief von oben](../renderer/relief.md), „Kosten“.
