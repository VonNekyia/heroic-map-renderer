---
name: renderzeit-erneuern
description: Erneuert die Zeitaufschlüsselung des Renderns in docs/humankeys/renderzeit.md. Misst die Läufe unter dem Profiler, schreibt eine neue Messseite mit CSV und setzt die Tabellen je Phase und Funktion neu. Nutzen vor einem Release, das den Renderpfad oder die Leistung merklich ändert, und auf Wunsch des Maintainers.
---

# Renderzeit erneuern

[`docs/humankeys/renderzeit.md`](../../docs/humankeys/renderzeit.md) zeigt,
wohin die Zeit beim Rendern geht, je Phase und Funktion. Der Maintainer hat
die Seite angefordert; dieser Skill hält ihre Zahlen aktuell. Jede
Erneuerung ist eine Messung: unter der Sperre und mit Freigabe, nach dem
Skill [`messung-protokollieren`](../messung-protokollieren/SKILL.md).

## Was fest steht

Das sind die Kriterien der Seite. Ein Agent ändert sie nicht selbst
([`AGENTS.md`](../../AGENTS.md), Regel 28):

- **Die Läufe** und ihre Schalter, `LAEUFE` in
  [`laeufe.py`](laeufe.py): ein voller Lauf und zwei Updates von oben, ein
  voller Lauf und ein Update im iso mit Cinematic, über die Testwelt.
- **Die Phasen,** `PHASEN` in [`phasen.py`](phasen.py).
- **Die Schwelle** von 0,5 %, ab der eine Funktion einzeln steht, und die
  Spalten der Tabellen, `SCHWELLE` und `tabellen` in
  [`auswerten.py`](auswerten.py).

Pflege und ohne Rückfrage erlaubt ist, was dem Code folgt:

- die Zuordnung einer Funktion zu ihrer Phase, die Regeln in `phasen.py`;
- die Liste `MARKEN` dort, vor deren Funktionen die Messung
  `#[inline(never)]` setzt;
- die Spalte „Wo im Code“ der Tabelle „Die Phasen“ auf der Seite.

## Voraussetzungen

- **Windows** mit dem Sammler von Visual Studio (`VSDiagnostics.exe`, im
  Ordner `Team Tools\DiagnosticsHub\Collector`) und `xperf` aus dem Windows
  Performance Toolkit. Liegen sie woanders, nennen die Umgebungsvariablen
  `VSDIAGNOSTICS` den Ordner des Sammlers und `XPERF` die Datei. Beides sind
  Werkzeuge am Messrechner, keine Abhängigkeit des Renderers.
- **Die Testwelt** und die Assets, siehe
  [Eingabedaten](../../docs/entwicklung/eingabedaten.md).
- **Platz:** rund 20 GB im Arbeitsordner für die Kopie der Welt und die
  Bäume, die das Skript am Ende wieder löscht.

## Ablauf

1. **Freigabe holen und Dauer nennen:** rund 40 min, in denen niemand baut
   oder testet. Die Sperre `messung.lock` legt und löst `laeufe.py` selbst;
   liegt eine fremde, bricht es ab.
2. **Läufe messen,** aus dem Wurzelverzeichnis, am Stand, der gemessen
   werden soll:

   ```bash
   python skills/renderzeit-erneuern/laeufe.py --welt ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --arbeit <arbeitsordner> --profile <profilordner>
   ```

   - Das Skript baut den Stand in einem eigenen Worktree im Arbeitsordner,
     mit Zeilentabellen und mit `#[inline(never)]` vor den Funktionen aus
     `MARKEN`. Passt `MARKEN` nicht mehr zum Code, bricht es ab und nennt
     die Funktion: in `phasen.py` nachziehen und neu starten.
   - Es kopiert die Welt, misst die fünf Läufe unter dem Profiler und löscht
     Kopie und Bäume wieder. Für die Updates ersetzt
     [`bloecke_setzen.py`](bloecke_setzen.py) in 16 Chunks der Kopie den
     obersten festen Block.
   - `--zeige` nennt nur die Befehle der Läufe.
   - Nach dem Lösen der Sperre rechnet es aus jedem Profil
     `<lauf>.profil.txt` und `<lauf>.stack.txt`.
3. **Tabellen erzeugen:**

   ```bash
   python skills/renderzeit-erneuern/auswerten.py <profilordner> <JJJJ-MM-TT> <commit> <version>
   ```

   Das schreibt je Lauf eine CSV nach
   `docs/messungen/<datum>-renderzeit-je-funktion-<lauf>.csv`, setzt die
   Tabellen zwischen den Marken in `docs/humankeys/renderzeit.md` neu und
   gibt die Tabelle der Läufe für die Messseite aus.
4. **Zuordnung prüfen:** Steht unter „Rest“ eine Funktion über der
   Schwelle, bekommt sie in `phasen.py` ihre Regel; dann Schritt 3 noch
   einmal.
5. **Abweichung des Builds messen:** Der gemessene Build verbietet das
   Einbetten der Funktionen aus `MARKEN`. Wie viel das an der Dauer ändert,
   ist bisher nicht gemessen. Dafür den vollen Lauf von oben ohne Profiler
   je dreimal im Wechsel laufen lassen, einmal mit dem Binär aus dem
   Arbeitsordner und einmal mit einem Release-Build desselben Stands ohne
   Marken; die Dauer beider kommt in die Messseite. Solange das fehlt,
   steht auf beiden Seiten „nicht gemessen“.
6. **Messseite schreiben:** eine neue Datei
   `docs/messungen/<datum>-renderzeit-je-funktion.md` nach der Vorlage in
   `messung-protokollieren`, mit Bedingungen, der Tabelle der Läufe aus
   Schritt 3 und dem Verweis auf ihre CSV. Die alte Messseite und ihre CSV
   bleiben stehen.
7. **Dauerseite durchsehen:** Die Tabellen unter „Zahlen“ sind neu. In „Die
   Phasen“ die Spalte „Wo im Code“ nachziehen, wenn Funktionen umbenannt
   oder verschoben sind. „Was die Zahlen nicht zeigen“ anpassen, wenn
   Schritt 5 gemessen ist. Sonst bleibt die Seite, wie sie ist.
8. **Eintragen:** die Zeile der neuen Messseite in
   [`docs/index.md`](../../docs/index.md).
9. **Prüfen:** `bash .github/pruefe-doku.sh`; keine Hardware, kein Pfad des
   Rechners und kein Weltname in Seiten und CSV
   ([`AGENTS.md`](../../AGENTS.md), Regel 20). `auswerten.py` nennt Treiber
   darum nur als Gruppe, `fremdes_modul` in `phasen.py`.

## Was die Messung nicht kann

- **Nur Windows:** Unter Linux, wo ein Server meist läuft, packt die
  Bibliothek die Kacheln nur mit der ersten Stufe von SIMD; der Anteil des
  Kodierens ist dort höher. Ein Profil unter Linux bräuchte ein anderes
  Werkzeug.
- **Der Anfang fehlt:** Der Profiler hängt sich ein bis drei Sekunden nach
  dem Start an den Prozess.
- **Feine Phasen nur als CPU-Zeit:** Der Renderer misst selbst nur Vorlauf,
  Höhen, Basis und Pyramide. Lesen, Licht, Sammeln, Zeichnen und Kodieren
  trennt erst das Profil, als Anteil an der CPU-Zeit.
