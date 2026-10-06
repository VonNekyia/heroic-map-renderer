---
title: "0089: Ungetöntes Laub mit eigener Farbe"
description: Warum eigene Laubfarben auch auf den sieben Blattsorten wirken, die das Spiel nicht tönt, über eine graue und eine helle Kopie der Textur nach festen Tabellen; warum die Blüten der blühenden Azalee ungetönt als eigene Fläche darüber liegen und welche Wege verworfen sind. Löst 0081 im Punkt „nur tönbares Laub“ ab.
status: gilt
date: 2026-10-06
issues: [197, 179, 156]
code:
  - renderer/src/assets/laubkopie.rs
  - renderer/src/assets/laubtabellen.py
  - renderer/src/assets/grau.txt
  - renderer/src/assets/hell.txt
  - renderer/src/assets/blueten.txt
  - renderer/src/assets/colors.rs
  - renderer/src/assets/texture.rs
  - renderer/src/render/sprites.rs
  - renderer/src/render/tiles.rs
---

# 0089: Ungetöntes Laub mit eigener Farbe

Löst [0081](0081-eigene-laubfarben.md) im Punkt „nur tönbares Laub“ ab:
Die eigene Farbe wirkte nur auf Laub, das das Spiel tönt, und auf Fichte
und Birke.

## Anlass

Sieben Blattsorten tönt das Spiel nicht: Azalee, blühende Azalee, Kirsche,
Blasse Eiche und die drei Pappeln aus 26.3, rot, orange und gelb. Ihre
Texturen tragen die Farbe schon. Ein Plugin, das ihnen eine eigene Farbe
gibt, sah auf der Karte nichts davon. Der Reviewer entschied am 06.10. an
#197, wie sie zeichnen.

## Entscheidung

- **Eine graue Kopie** der Textur für eine eigene Farbe ohne Bit 24, eine
  helle mit Bit 24. Beide tauschen je Texel Farben nach einer festen
  Tabelle, wie die helle Kopie aus
  [0088](0088-helles-laub-aus-dem-spiel.md), und werden danach mit der
  eigenen Farbe getönt, auf jeder Fläche mit der Blatttextur, auch ohne
  `tintindex` im Modell. Beide ohne `dark_cutout`, wie die helle Kopie in
  0088.
- **Die Tabellen** stehen in `grau.txt` und, für hell, in `hell.txt`.
  `laubtabellen.py` erzeugt sie aus den Texturen des Client-JARs: grau
  `round(L / Lmax · 188)` mit `L = 0,299 r + 0,587 g + 0,114 b` über die
  Farben der Sorte, hell dieselbe Regel wie in 0088 auf dem Grau. Aus den
  Texturen von 26.3 ergibt das Farbe für Farbe die Tabellen, die der
  Reviewer an #197 vorgab. Der Renderer nimmt die Tabellen, nicht die
  Formel, wie in 0088.
- **Eine Farbe, die nicht in der Tabelle steht,** etwa aus einem
  Resourcepack, bleibt und wird getönt.
- **Die Blüten der blühenden Azalee,** die Farben `9e5088`, `ba62ce` und
  `d07be3`, die die Textur der Azalee nicht hat, bleiben ungetönt. Sie
  stehen in `blueten.txt`. Die Kopie nimmt sie heraus, ihre Texel werden
  dort durchsichtig, und eine zweite Fläche mit denselben Ecken trägt nur
  die Blüten, ohne Tönung, über der getönten.
- **Je Familie** ungetönten Laubs mit eigener Farbe eine eigene, mit
  Bit 24 eine weitere, wie für Fichte und Birke in 0081 und für hell in
  0088. Der Renderpfad nimmt sie nur an Stellen mit eigener Farbe; jede
  andere Stelle zeichnet Byte für Byte wie zuvor.

Wie es wirkt, steht in [Eigene Laubfarben](../benutzung/laubfarben.md),
„Wirkung“.

## Verworfene Alternativen

- **Die Textur ohne Kopie tönen:** Rosa oder Rot mal einer Farbe ergibt
  trübe Töne, die die eigene Farbe nicht treffen.
- **Die Tabellen zur Laufzeit aus den Texturen der Assets rechnen:** wie in
  0088 verworfen.
- **Die Blüten mittönen:** Die blühende Azalee verlöre ihre Blüten und sähe
  aus wie die Azalee.
- **Weitere Sorten vorab:** Laub aus Mods oder späteren Versionen kommt
  erst dazu, wenn es eine Tabelle gibt.

## Folgen

- Je Familie ungetönten Laubs mit eigener Farbe rastert jede Stufe eine
  Familie mehr, mit Bit 24 noch eine; die blühende Azalee zeichnet dort je
  Fläche zwei.
- Eine neue Sorte braucht einen Eintrag in `UNGETOENT` in
  `laubtabellen.py`, eine Zeile in `grau.txt` und `hell.txt` und ihren
  Namen in `ungetoentes_laub` in
  [`renderer/src/assets/colors.rs`](../../renderer/src/assets/colors.rs).
- Eine neue Version mit anderen Blatttexturen braucht neu erzeugte
  Tabellen, Skill
  [`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md).
