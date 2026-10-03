---
title: "0060: Grenze Schatten/Sonne am Renderer"
description: Warum am Renderer für Schatten/Sonne die Grenze 0,35 bis 0,75 gilt statt 0,35 bis 0,66 aus 0058, und warum der Look dafür gleich bleibt.
status: gilt
date: 2026-10-03
issues: [73, 101]
code:
  - renderer/tests/kennzahlen.rs
---

# 0060: Grenze Schatten/Sonne am Renderer

## Anlass

[0058](0058-look-von-cinematic.md) legt für die Kennzahl Schatten/Sonne 0,35
bis 0,66 fest, abgestimmt am Prototyp zu #89. Am Renderer mit Cinematic aus
#73 liegt sie im Mittel der 24 Ansichten der Testwelt bei 0,639 statt
0,593, in 9 Ansichten über 0,66, höchstens bei 0,743. In der Sonne liegen
Renderer und Prototyp beieinander. Schatten sind am Renderer heller, siehe
[Look am Renderer](../messungen/2026-10-03-look-am-renderer.md), „Schluss“:

- Der Renderer rechnet das Licht des Spiels mit weicher Beleuchtung je
  Ecke, seit #101; der Prototyp rechnete ein vereinfachtes Modell.
- Die Oberseite einer Schneeschicht beleuchtet der Renderer mit dem Licht
  der eigenen Zelle, eine Näherung, siehe #51.

## Entscheidung

Am Renderer gilt für Schatten/Sonne 0,35 bis 0,75. Die übrigen Grenzen
aus 0058 bleiben, ebenso jeder Wert des Looks. Entschieden vom Maintainer am
03.10.2026.

Damit halten 21 von 24 Ansichten alle Grenzen. Schatten/Sonne hält in allen
24. Ausserhalb bleibt Schnee von oben und schräg von Norden aus beiden
Richtungen, wegen dL in der Sonne, aus `s` dazu wegen C zur Karte. Auch
am Prototyp lagen Schnee von oben und schräg von Norden ausserhalb, siehe
0058.

## Verworfene Alternativen

- **Den Look ändern, bis 0,66 hält:** Die Schatten dunkler zu machen,
  hiesse vom Licht des Spiels abzuweichen, das der Renderer gerade erst
  genau rechnet.
- **Zuerst #51:** Die Fläche im Innern einer Zelle mit gemischtem Licht
  zu beleuchten, senkte Schatten/Sonne vor allem im Schnee. Die übrigen
  Schatten sind 1,1- bis 1,6-mal so hell wie am Prototyp, weil das Licht
  des Spiels so ist.

## Folgen

- Prüfmass für Cinematic am Renderer ist 0,35 bis 0,75. Wer den Look
  ändert, rechnet die Kennzahlen mit `kennzahlen_der_ansichten` in
  [`renderer/tests/kennzahlen.rs`](../../renderer/tests/kennzahlen.rs) neu
  und vergleicht mit dieser Grenze.
- Ändert #51 das Licht im Schnee, liegt Schatten/Sonne dort niedriger; die
  Grenze bleibt.
