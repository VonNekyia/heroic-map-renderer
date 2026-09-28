# AGENTS.md

Regeln für alle, die in diesem Repository arbeiten, Agenten wie Menschen.
Das Wissen des Projekts ist AI first aufgebaut: zuerst für Agenten
geschrieben, für Menschen auf GitHub lesbar.

## Wo das Wissen liegt

| Ort | Inhalt | Wann lesen |
|---|---|---|
| `AGENTS.md` | Regeln | immer, diese Datei |
| `docs/` | Wissen: wie der Renderer das Spiel nachbaut, Entscheidungen, Messungen | vor jeder Änderung die betroffenen Seiten |
| `skills/` | Workflows, Schritt für Schritt | vor dem passenden Arbeitsschritt |

`README.md` ist das Aushängeschild für Menschen: was das Projekt ist,
Bilder, Schnellstart, Stand. Ausführlich steht alles in `docs/`.

## Rollen

| Rolle | Aufgabe |
|---|---|
| Maintainer | entscheidet, was umgesetzt wird, und merged |
| Backend-Programmierer | der Renderer in Rust, `renderer/` |
| Frontend-Programmierer | die Karte im Browser, `web/` |
| Reviewer | prüft jede PR |
| Researcher | sucht bessere Verfahren und Alternativen |

Die Schnittstelle zwischen Backend und Frontend sind
[`map.json`](docs/benutzung/map-json.md) samt den Höhen, die es nennt, und
die Projektion, die beide rechnen und an
`renderer/tests/fixtures/projektion.json` prüfen. Wer daran etwas ändert,
spricht es vorher mit der anderen Seite ab.

## Skills

| Skill | Wann |
|---|---|
| [`doku-pflegen`](skills/doku-pflegen/SKILL.md) | jede Änderung an Verhalten, Schaltern, Ausgabe, Dateiformaten, Leistung oder Grösse; jedes Review |
| [`entscheidung-festhalten`](skills/entscheidung-festhalten/SKILL.md) | eine Richtung wird festgelegt, eine Alternative verworfen oder eine Entscheidung abgelöst |
| [`messung-protokollieren`](skills/messung-protokollieren/SKILL.md) | Laufzeit, Grösse oder Speicher werden gemessen oder verglichen |
| [`spielverhalten-belegen`](skills/spielverhalten-belegen/SKILL.md) | Code oder Doku sagt, wie das Spiel etwas macht |
| [`alternativen-recherchieren`](skills/alternativen-recherchieren/SKILL.md) | ein besseres Verfahren, eine Optimierung oder eine Alternative wird gesucht; vor jedem grösseren Umbau |
| [`tabellen-neu-erzeugen`](skills/tabellen-neu-erzeugen/SKILL.md) | eine neue Spielversion, oder `blocks.txt`, `leuchten.txt` oder `schatten.txt` passt nicht mehr zum Spiel |
| [`goldbild-erneuern`](skills/goldbild-erneuern/SKILL.md) | das Goldbild fällt nach einer gewollten Änderung am Bild |
| [`doku-bilder-rendern`](skills/doku-bilder-rendern/SKILL.md) | ein Bild in `docs/bilder/` oder im README ist veraltet |

Vor dem Arbeitsschritt die `SKILL.md` ganz lesen und ihr folgen.

## Regeln

### Doku nutzen

1. Vor einer Änderung die Seiten lesen, die den Code beschreiben:
   `git grep -l "<pfad>" docs/` findet sie über `code:` in der Frontmatter.
   Dazu die Seiten, auf die Kommentare im Code verweisen.
2. Entscheidungen in `docs/entscheidungen/` mit `status: gilt` gelten. Wer
   abweichen will, schreibt eine neue, die sie ablöst.
3. Messungen in `docs/messungen/` sind die Vergleichsbasis. Neue Messungen
   laufen unter denselben Bedingungen.
4. Verhalten des Spiels wird belegt, nicht aus Erinnerung beschrieben:
   Skill `spielverhalten-belegen`.
5. Widersprechen sich Doku und Code, gilt keins von beiden. Klären und im
   selben PR richtigstellen.

### Doku pflegen

6. Die Doku ändert sich im selben PR wie der Code. Ein PR ohne passende Doku
   ist nicht fertig.
7. Eine Tatsache steht an genau einer Stelle. Andere Stellen verweisen
   darauf, statt sie zu kopieren.
8. Regeln stehen hier, Wissen in `docs/`, Workflows in `skills/`.
9. Alte Entscheidungen und Messungen bleiben stehen. Eine abgelöste
   Entscheidung bekommt `status: abgelöst durch NNNN`.

### Für Agenten schreiben

10. Eine Seite, ein Thema. Lieber drei kurze Seiten als eine lange.
11. Jede Seite beginnt mit Frontmatter (`title`, `description`, `code`) und
    einem Absatz, der allein reicht, um zu entscheiden, ob man weiterliest.
12. Fakten vor Prosa: Listen, Tabellen, Zahlen mit Einheit und Stand.
    Bezeichner, Pfade und Befehle in Backticks.
13. Code wird mit Pfad und Symbol genannt, nicht mit Zeilennummer.
14. Jede Seite steht mit einer Zeile in `docs/index.md`. Jeder Skill steht
    in der Tabelle oben.
15. Überschriften bleiben stabil. Wer eine umbenennt oder eine Seite
    verschiebt, zieht alle Verweise nach: `git grep "docs/<pfad>"`. Die CI
    prüft Verweise, Links und Pfade unter `code:` mit
    `bash .github/pruefe-doku.sh`.
16. Links sind relativ, zwischen Seiten wie auf Skills und Code, etwa
    `../../renderer/src/render/metatile.rs` aus `docs/renderer/`. So gehen
    sie auf GitHub und im Checkout.

### Code-Kommentare

17. Ein Kommentar sagt knapp, was der Code tut und welche Invariante gilt.
18. Entscheidungen, verworfene Alternativen, Messungen, Herleitungen und
    Belege aus dem Spiel stehen in `docs/`. Der Kommentar verweist darauf:

    ```rust
    /// Das Himmelslicht, in dem das Spiel den Block zeichnet.
    /// Siehe docs/renderer/wasser-und-licht.md, „Licht von der Seite“.
    ```

19. Ein Satz Begründung bleibt, wo der Code ohne ihn falsch aussähe, etwa
    bei einer Reihenfolge, die nicht vertauscht werden darf.

### Öffentlich

20. Das Repository ist öffentlich. Nirgends stehen Welt- oder Servernamen,
    Pfade vom eigenen Rechner, Seeds, Hardware oder Koordinaten der grossen
    Welt. Messungen nennen die Welt allgemein („die grosse Welt“, „die
    Testwelt“). Bilder zeigen nur die Testwelt oder Szenen, die ein
    Test baut.
21. Deutsch, kurze Sätze. Frontmatter-Schlüssel englisch, wie in Agent
    Skills.

### Entwurf

22. Bei jeder neuen Abhängigkeit und jedem neuen Feature gilt die Frage:
    Braucht ein Offline-Renderer für isometrische Minecraft-Rastertiles das
    wirklich? Wenn nein, kommt es nicht dazu.

## Gliederung von `docs/`

| Pfad | Inhalt |
|---|---|
| `docs/index.md` | Wegweiser: jede Seite mit einer Zeile |
| `docs/benutzung/` | Schalter, Assets, Welten, Kacheln und Zoomstufen, Pyramide und `--resume`, `map.json`, Kosten, Grafikkarte, Echtzeitschutz |
| `docs/renderer/` | wie der Renderer das Spiel nachbaut: Kamera, der Weg einer Kachel, Sprites und Deckung, Nähte, Wasser und Licht, weiche Beleuchtung, Biomfarben, Varianten, Blockstates, Packs, Modelle |
| `docs/frontend.md` | das Frontend |
| `docs/entwicklung/` | Aufbau des Codes, Tests, CI, Eingabedaten, erzeugte Tabellen |
| `docs/entscheidungen/` | `NNNN-titel.md`, eine Datei je Entscheidung |
| `docs/messungen/` | `JJJJ-MM-TT-titel.md`, eine Datei je Messreihe |
| `docs/bilder/` | Bilder, nur aus der Testwelt oder aus Szenen der Tests; Quellen in `docs/bilder/quellen/` |
