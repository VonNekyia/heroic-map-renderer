---
name: alternativen-recherchieren
description: Die Arbeitsweise des Researchers. Sucht für ein Problem des Renderers oder des Frontends bessere Verfahren, Optimierungen und Alternativen in Papern, belegten Algorithmen und verwandten Projekten, misst sie nach und schlägt sie als Issue vor. Nutzen vor einem grösseren Umbau, wenn ein Issue oder Review nach dem besten Verfahren fragt, und aus eigenem Antrieb bei den grössten Posten an Zeit, Platz und Qualität.
---

# Alternativen recherchieren

Der Researcher treibt Neues voran. Er sucht, wie andere dasselbe Problem
schneller, kleiner oder schöner lösen, und belegt jeden Vorschlag mit
Zahlen. Er setzt nicht selbst um. Sein Ergebnis ist ein Issue; nimmt der
Maintainer es an, setzt der zuständige Programmierer es um und der Reviewer
prüft es.

## Quellen

Bevorzugt:

- wissenschaftliche Paper und belegte Algorithmen;
- starke Programmierer;
- angesehene Websites, spezialisierte Archive, persönliche Blogs;
- alles, was mit exakten Zahlen und Fakten belegt ist;
- Projekte, die genau dieses Problem im Detail angehen, etwa andere
  Renderer für Minecraft-Karten.

Kein Beleg sind Erinnerung, Zusammenfassungen ohne die Quelle dahinter und
Behauptungen ohne Zahl. Wie das Spiel selbst etwas rechnet, belegt der Skill
[`spielverhalten-belegen`](../spielverhalten-belegen/SKILL.md).

Eine Quelle steht mit Autor, Titel, Jahr und Link, bei Websites mit dem Tag
des Abrufs, dazu die Zahl, auf die es ankommt, samt ihren Bedingungen.

## Ablauf

1. **Thema wählen,** das mit dem grössten Hebel zuerst. Zeit und Platz
   zeigen die Messungen in [`docs/messungen/`](../../docs/messungen/), die
   Abstände zum Spiel die Abschnitte „Was bleibt eine Näherung“ in
   [`docs/`](../../docs/index.md). Dazu die offenen Issues und die Zeile
   „Noch nicht“ im README.
2. **Stand kennen.** Die Seiten des Themas lesen, die Entscheidungen mit
   `status: gilt` samt ihren verworfenen Alternativen und die letzte Messung
   als Vergleichsbasis. Verworfenes nur mit neuen Belegen wieder aufgreifen.
3. **Suchen,** von der genauen Frage aus: Welches Verfahren, welche
   Datenstruktur, welcher Encoder, welches Projekt löst genau dieses
   Problem? Auch Wege neben dem bisherigen betrachten, etwa Echtzeit statt
   fertiger Kacheln.
4. **Bewerten,** je Kandidat:
   - Was behauptet die Quelle, mit welcher Zahl, unter welchen Bedingungen?
   - Passt es hierher: offline gerenderte isometrische Rasterkacheln,
     Minecraft 26.x, CPU und Vulkan, im Browser Leaflet? Braucht das
     Projekt es wirklich ([`AGENTS.md`](../../AGENTS.md), Regel 22)?
   - Gewinn an Renderzeit, Speicherplatz oder Qualität der Karte. Kostet es
     sichtbar Qualität, steht das im Vorschlag.
   - Aufwand, Risiken, neue Abhängigkeiten.
   - Lizenz, wenn fremder Code übernommen werden soll. Ist jemand vielfach
     schneller und passt die Lizenz, wird seine Arbeit übernommen und
     angepasst.
5. **Nachmessen.** Eine fremde Zahl gilt erst, wenn sie hier gemessen ist:
   ein Prototyp in einem eigenen Worktree, gemessen nach dem Skill
   [`messung-protokollieren`](../messung-protokollieren/SKILL.md). Der
   Prototyp wird keine PR. Lässt sich nichts messen, sagt der Vorschlag,
   dass die Zahl nur aus der Quelle stammt.
6. **Vorschlagen** als Issue: Problem und Stand mit Zahl, die Kandidaten mit
   Quelle und Zahl, die eigene Messung, Empfehlung, Aufwand und Risiken.
   Der Entwurf geht zuerst an den Reviewer; veröffentlicht wird das Issue
   erst, wenn der Reviewer ihn durchgesehen hat. Es ist öffentlich, also
   ohne Interna
   ([`AGENTS.md`](../../AGENTS.md), Regel 20).
7. **Übergeben.** Die Quellen wandern mit der Umsetzung in die Doku: die
   gewählte Lösung auf die Seite des Themas, die verworfenen Kandidaten
   unter „Verworfene Alternativen“ der Entscheidung, Skill
   [`entscheidung-festhalten`](../entscheidung-festhalten/SKILL.md). Das
   Review prüft die Quellen mit.
