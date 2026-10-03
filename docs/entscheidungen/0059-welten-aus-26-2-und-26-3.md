---
title: "0059: Welten und Packs aus 26.2 und 26.3, Tabellen aus 26.3"
description: Warum der Renderer Welten und Resourcepacks aus 26.2 und aus 26.3 liest, seine Tabellen aber nur aus 26.3 erzeugt, und warum die Testwelt auf 26.2 bleibt.
status: gilt
date: 2026-10-03
issues: [98]
code:
  - renderer/src/world/chunk.rs
  - renderer/src/assets/model.rs
  - renderer/src/assets/blocks.txt
---

# 0059: Welten und Packs aus 26.2 und 26.3, Tabellen aus 26.3

Die Sicht in der Ecke wählt der Renderer seit
[0065](0065-sicht-in-der-ecke-nach-der-version.md) nach der Version der
Welt; sonst gilt diese Entscheidung weiter.

## Anlass

26.3 ist erschienen. Ein Server, der darauf wechselt, schreibt Chunks im
neuen Format. #98 hat am Client- und am Server-JAR von 26.2 und 26.3
belegt, was sich ändert:
- die Namen der Palette im Chunk;
- die Scherben eines Krugs;
- `shade` in den Modellen;
- die weiche Beleuchtung bei 23 Blöcken;
- 90 neue Blöcke.

[0015](0015-nur-welten-ab-26-1.md) sagt nur, ab welcher Version Welten
gelten.

## Entscheidung

Der Renderer liest Welten und Resourcepacks aus 26.2 und aus 26.3. Seine
Tabellen erzeugen die Generatoren nur aus 26.3. Die Testwelt bleibt auf
26.2 und prüft so den Weg von 26.2. Chunks und Modelle von 26.3 bauen die
Tests selbst. 0015 gilt weiter: ab 26.1, nur 26.x.

## Verworfene Alternativen

- **Nur 26.3 lesen.**
  - Eine Welt, die ein Server von 26.2 auf 26.3 hebt, hat beide Formate:
    Der Server schreibt einen Chunk erst neu, wenn er ihn speichert.
  - Packs für 26.2 schreiben weiter `shade`, und 26.3 liest es nicht mehr.
- **Tabellen je Version.** Die Tabellen von 26.3 kennen alle Blöcke von
  26.2. Sonst bleiben sie gleich oder sind nur neu nummeriert, siehe #98.
  Ein zweiter Satz hiesse doppelte Generatoren für dasselbe.
- **Die Testwelt auf 26.3 heben.** Dann prüfte keine echte Welt mehr den
  Weg von 26.2, den Welten im Übergang weiter brauchen.

## Folgen

- **Gezeichnet wie mit 26.3:** Eine Welt aus 26.2 erscheint mit den
  Regeln von 26.3. Eis, Brucheis, Schleimblöcke, Shulkerkisten,
  Leuchtfeuer, Spawner und Barriere lassen in der weichen Beleuchtung
  jetzt Licht durch die Ecke, siehe [Weiche Beleuchtung](../renderer/weiche-beleuchtung.md).
- **Belege:** Ein Beleg nennt die Version, an der er geholt ist, die
  meisten 26.2. Was #98 in 26.3 gleich fand, gilt weiter. Wo 26.3 anders
  ist, nennen die Seiten beide, etwa bei der Palette, bei `sherds`, bei
  `shade` und bei der weichen Beleuchtung.
- **Die nächste Version** braucht wieder neue Tabellen nach Skill
  [`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md);
  ob 26.2 dann bleibt, entscheidet eine neue Entscheidung.
