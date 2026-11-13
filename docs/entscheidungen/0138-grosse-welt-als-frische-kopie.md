---
title: "0138: Die grosse Welt als frische Kopie"
description: Warum Messungen auf der grossen Welt an einer frischen Kopie der Welt eines laufenden Servers laufen, die nach der Reihe wieder wegfällt, und Basis und Kandidat auf derselben Kopie messen; warum keine eingefrorene Kopie bleibt, was aus den alten Reihen wird und was eine Reihe jetzt kostet.
status: gilt
date: 2026-11-13
issues: []
code:
  - skills/messung-protokollieren/SKILL.md
---

# 0138: Die grosse Welt als frische Kopie

## Anlass

Bis November 2026 liefen die Messungen auf der grossen Welt an zwei alten
Kopien in 26.2. Sie stammen vom 25.09. und von Anfang Oktober 2026 und lagen
seither unverändert, zusammen 61 GB. Sie wuchsen nicht mit der Welt und
blieben auf der alten Spielversion. Am 13.11.2026 hat der User entschieden,
dass beide wegfallen, um Platz zu schaffen.

## Entscheidung

- **Die grosse Welt ist für Messungen eine frische Kopie** der Welt eines
  laufenden Servers. Sie wird für eine Reihe gezogen und nach der Reihe
  gelöscht. Eine eingefrorene Kopie gibt es nicht.
- **Gemessen wird nur an der Kopie,** nie an der laufenden Welt.
- **Kopiert wird erst, wenn der Server nicht schreibt:** nach `save-off` und
  `save-all flush`, bis `save-on`, oder bei gestopptem Server. Die Kopie
  zieht, wer den Server betreibt.
- **Basis und Kandidat laufen auf derselben Kopie,** im selben Fenster. Die
  Basis wird auf jeder Kopie neu gemessen, auch wenn eine frühere Reihe sie
  schon hatte.
- **Die Seite der Messung nennt die Kopie:** Spielversion, Zahl der Regionen
  und Tag der Kopie. Sie heisst weiter „die grosse Welt“, ohne Namen, Pfade,
  Seeds und Koordinaten ([`AGENTS.md`](../../AGENTS.md), Regel 20).
- **Alte Reihen bleiben stehen** (Regel 9). Mit Läufen auf einer frischen
  Kopie sind sie nicht vergleichbar. Der erste volle Lauf auf einer frischen
  Kopie wird die neue Bezugsgrösse für
  [Was ein Lauf kostet](../benutzung/kosten.md) und das README.
- **Die Testwelt bleibt,** wie sie ist, siehe
  [Eingabedaten](../entwicklung/eingabedaten.md).

## Verworfene Alternativen

- **Die alten Kopien behalten:** 61 GB auf Dauer, Stand 26.2, ohne das, was
  seither in der Welt entstanden ist. Vom User am 13.11.2026 verworfen.
- **Eine Kopie vom Server auf Dauer einfrieren:** rund 32 GB auf Dauer, und
  sie veraltet wie die alten. Platz ist knapp (Regel 26); die doppelte
  Laufzeit je Reihe wiegt weniger.
- **An der laufenden Welt messen:** Der Server schreibt währenddessen Chunks.
  Zwei Läufe sähen nicht dieselbe Welt, und der Lauf belastet den Server.

## Folgen

- **Eine Reihe auf der grossen Welt kostet zwei volle Läufe statt einem:**
  die Basis und den Kandidaten auf derselben Kopie. Wie lange das dauert,
  steht in der Ansage vor der Messung, siehe Skill
  [`messung-protokollieren`](../../skills/messung-protokollieren/SKILL.md),
  Schritt 2.
- **Für die Dauer der Reihe** liegen rund 32 GB für die Kopie auf der Platte
  (Stand 13.11.2026), dazu die Kacheln der Läufe.
- **Zahlen zweier Kopien** sind nur grob vergleichbar, denn die Welt wächst
  zwischen ihnen.
- **Die Zahlen der grossen Welt in der Doku** stammen bis zum ersten vollen
  Lauf auf einer frischen Kopie von den alten Kopien in 26.2.
