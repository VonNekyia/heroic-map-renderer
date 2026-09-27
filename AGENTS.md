# Anweisungen für Agenten

Was hier steht, gilt für jeden Agenten und jeden Menschen, der in diesem
Repository Code, Doku, Commits, Issues oder PR-Texte schreibt. Dieser Teil
regelt die Dokumentation: wo was steht, wie man sie nutzt und wie man sie
pflegt.

## Wo was steht

- **`README.md` ist das Aushängeschild:** was das Projekt ist, ein oder zwei
  Bilder, ein Schnellstart, der Stand in wenigen Zeilen und der Link zur
  Doku. Keine Herleitungen, keine Messtabellen, keine Entscheidungen.
- **`docs/` ist die Entwicklerdoku**, Markdown, gebaut mit MkDocs
  (`mkdocs.yml` im Wurzelverzeichnis). Dort steht alles, was man zum
  Verstehen und Weiterentwickeln braucht: Benutzung im Detail, wie der
  Renderer das Spiel nachbaut, Entscheidungen, Messungen und
  Testergebnisse.
- **Code-Kommentare** sagen knapp, was der Code tut und welche Invariante
  gilt. Das Warum steht in `docs/`, der Kommentar verweist darauf, siehe
  „Code-Kommentare und Verweise“.
- **Übergang:** Bis #19 umgesetzt ist, steht die Entwicklerdoku noch im
  README und in Kommentaren. Wer #19 umsetzt, legt die Gliederung unten an,
  zieht den Inhalt um und stellt die Kommentare um. Danach gilt diese Datei
  ohne Ausnahme.

## Gliederung von `docs/`

| Pfad | Inhalt |
|---|---|
| `docs/index.md` | Überblick und Wegweiser |
| `docs/benutzung/` | alle Schalter; Kacheln, Zoomstufen und native Stufen; Pyramide und `--resume`; `map.json`; Grafikkarte; Echtzeitschutz |
| `docs/renderer/` | wie der Renderer das Spiel nachbaut: Kamera und Projektion, Blockstates und Modelle, Sprites und Deckung, Wasser und Licht, weiche Beleuchtung, Biomfarben, keine Nähte |
| `docs/frontend.md` | das Frontend |
| `docs/entwicklung/` | bauen, Tests, CI, Mutationen, Eingabedaten; die erzeugten Tabellen `blocks.txt`, `leuchten.txt` und `schatten.txt` samt Befehl zum Neuerzeugen |
| `docs/entscheidungen/` | eine Datei je Entscheidung, nummeriert, etwa `0001-nur-26x.md` |
| `docs/messungen/` | Messprotokolle und Testergebnisse, eine Datei je Messreihe, mit Datum im Namen |
| `docs/bilder/` | Bilder, nur aus der Testwelt |

Jede neue Seite kommt in `nav` in `mkdocs.yml`. Eine neue Seite innerhalb
dieser Gliederung braucht keine Freigabe. Eine neue oberste Ebene wird im
Issue oder PR angesprochen.

## Doku nutzen

- **Erst lesen, dann ändern.** Vor einer Änderung die Seiten in `docs/`
  lesen, die den Code betreffen, und die Entscheidungen, auf die seine
  Kommentare verweisen. Sie erklären, warum der Code so ist und welches
  Verhalten des Spiels er nachbaut.
- **Entscheidungen gelten.** Wer von einer abweichen will, sagt es im Issue
  oder PR und schreibt eine neue Entscheidung, die die alte ablöst. Die
  alte bleibt stehen, mit dem Status „abgelöst durch …“. Nie still
  überschreiben.
- **Messungen sind die Vergleichsbasis.** Neue Messungen laufen unter
  denselben Bedingungen wie die, mit denen sie verglichen werden: gleicher
  Ausschnitt, scale, Threads, Grafikkarte an oder aus, native Stufen, abwechselnd
  gemessen, jeder Lauf frisch.
- **Verhalten des Spiels** wird nicht aus Erinnerung beschrieben, sondern
  per javap am JAR der unterstützten Version belegt und mit Klasse und
  Methode zitiert.
- **Widerspricht die Doku dem Code**, gilt keins von beiden stillschweigend:
  klären, was stimmt, und im selben PR richtigstellen.

## Doku pflegen

- **Im selben PR.** Jede Änderung an Verhalten, Schaltern, Ausgabe,
  Dateiformaten, Leistung oder Grösse ändert die Doku mit. Ein PR ohne
  passende Doku ist nicht fertig; das Review prüft es.
- **Das README** nur ändern, wenn sich Schnellstart, Stand oder das Bild des
  Projekts ändern.
- **Eine neue Entscheidung** bekommt eine Datei in `docs/entscheidungen/`
  nach dieser Vorlage:

  ```markdown
  # 0007: Weiche Beleuchtung erst für volle Würfel

  - Datum: 2026-09-27
  - Status: gilt (oder: abgelöst durch 0012)
  - Issue und PR: #15, #18

  ## Anlass
  ## Entscheidung
  ## Verworfene Alternativen
  ## Folgen
  ```

- **Eine neue Messung** bekommt eine Datei in `docs/messungen/`, etwa
  `2026-09-27-weiche-beleuchtung.md`, mit:
  - Datum und den Commits der verglichenen Stände;
  - was gemessen wurde: die Welt allgemein benannt, Ausschnitt,
    Kachelzahl, scale, Threads, Grafikkarte, native Stufen;
  - wie: abwechselnd, frische Läufe, Zahl der Läufe, bestes oder Mittel;
  - die Tabelle, die Streuung und den Schluss daraus.

  Alte Messungen werden nicht überschrieben. Die Seiten zur Leistung zeigen
  den aktuellen Stand und verweisen auf die Messung, aus der er stammt.
- **Näherungen und Abweichungen vom Spiel** stehen auf der Seite des Themas
  unter „Was bleibt eine Näherung“, jeweils mit dem Grund.
- **Die erzeugten Tabellen** werden für eine neue Spielversion neu erzeugt.
  Der Befehl steht in `docs/entwicklung/`, die Seite nennt die Version.
- **Sprache:** Deutsch wie das übrige Projekt, kurze Sätze. Bezeichner,
  Pfade und Befehle stehen in Backticks.

## Code-Kommentare und Verweise

- Ein Kommentar sagt in wenigen Sätzen, was der Code tut und welche
  Invariante gilt.
- **Entscheidungen, verworfene Alternativen, Messungen, Herleitungen und
  Belege aus dem Spiel gehören nicht in Kommentare**, sondern nach `docs/`.
  Dort ist Platz, sie ausführlich zu beschreiben, ohne den Code
  zuzumüllen.
- Der Kommentar verweist mit Datei und Überschrift, im Stil der bisherigen
  Verweise auf das README:

  ```rust
  /// Das Himmelslicht, in dem das Spiel den Block zeichnet.
  /// Siehe docs/renderer/wasser-und-licht.md, „Licht von der Seite“.
  ```

- Ein Satz Begründung im Code bleibt erlaubt, wenn der Code ohne ihn falsch
  aussähe, etwa bei einer Reihenfolge, die nicht vertauscht werden darf.
  Alles darüber hinaus steht in der Doku.
- Überschriften in `docs/` bleiben stabil. Wer eine umbenennt oder eine
  Seite verschiebt, zieht alle Verweise nach: `git grep "docs/<pfad>"`.
- Tests dürfen auf die Doku verweisen, wenn sie ein Verhalten des Spiels
  festschreiben. Den erwarteten Wert und seine Herkunft nennt der Test
  selbst in einem Satz.

## Keine Interna

Das Repository ist öffentlich. In `docs/`, im README, im Code, in Commits,
Issues und PR-Texten stehen:

- keine Welt- oder Servernamen, keine Pfade vom eigenen Rechner (Laufwerke,
  Benutzernamen), keine Seeds, keine Hardware und keine Koordinaten der
  grossen Welt;
- in Messungen die Welt nur allgemein: „die grosse Welt“, „die Testwelt“;
- in Bildern nur die Testwelt.

## Bauen und prüfen

Die Doku baut ein Docker-Container, ohne Python-Installation. Wer #19
umsetzt, setzt hier und im README den festen Tag des Images ein:

```bash
docker run --rm -it -p 8000:8000 -v "$PWD":/docs squidfunk/mkdocs-material:<tag> serve -a 0.0.0.0:8000
docker run --rm -v "$PWD":/docs squidfunk/mkdocs-material:<tag> build --strict
```

Der erste Befehl zeigt die Doku unter `http://localhost:8000`, der zweite
baut sie nach `site/`. Vor jedem PR mit Doku-Änderung muss `build --strict`
ohne Warnung laufen; mit #19 prüft die CI dasselbe. Links zwischen Seiten
und auf Bilder sind relativ.

## Review

Das Review prüft die Doku mit:

- Stimmt sie mit dem Code überein?
- Ist jede neue Entscheidung festgehalten, jede Messung reproduzierbar
  beschrieben?
- Verweisen Kommentare auf die Doku, statt Entscheidungen zu erzählen?
- Stehen keine Interna darin?
