---
title: "0087: Der Assistent auf der Konsole, im selben Binär"
description: Warum die EXE ein Assistent auf der Konsole im selben Binär wie die CLI ist, mit dem Ordnerdialog von Windows nur dort, warum jeder Schritt dasselbe Binär als Kindprozess aufruft, wann er auf Enter wartet und warum die gebaute Karte als web/ in beiden Paketen liegt.
status: gilt
date: 2026-10-06
issues: [152, 147, 151]
code:
  - renderer/src/cli/assistent.rs
  - renderer/src/cli.rs
  - renderer/Cargo.toml
  - .github/workflows/release.yml
---

# 0087: Der Assistent auf der Konsole, im selben Binär

## Anlass

Wer kein Plugin hat und keine Kommandozeile mag, soll unter Windows ohne
weitere Installation von der Welt bis zur offenen Karte kommen (#152). Die
Form hat der Maintainer am 05.10. festgelegt, die Umsetzung der Reviewer am
06.10. angenommen.

## Entscheidung

- **Ein Assistent auf der Konsole im selben Binär:** ohne Schalter an einer
  Konsole unter Windows gestartet, etwa per Doppelklick. Mit Schaltern bleibt
  alles CLI. Unter Linux kommt nichts ins Binär, ein Aufruf ohne Schalter
  bleibt dort, wie er war.
- **Der Ordnerdialog von Windows** für Welt und Ziel, über rfd (MIT), nur
  unter `[target.'cfg(windows)'.dependencies]` und ohne seine Vorgaben. Nach
  einem abgebrochenen Dialog fragt er den Pfad auf der Konsole.
- **Jeder Schritt ist ein Aufruf desselben Binärs** mit Schaltern, als
  Kindprozess: die Schätzung mit `--estimate`, der Lauf, der Server mit
  `--exit-with-stdin`. Die Kommandozeile zeigt er vorher. Der Entwurf sah
  Schätzung und Lauf im selben Prozess vor; als Kindprozess baut jeder
  Schritt seinen Pool von rayon und seine Grafikkarte neu, und ein Absturz
  des Laufs nimmt den Assistenten nicht mit.
- **Die Zustimmung zum Client-Jar** fragt er mit j/n, Vorgabe n, nur wenn das
  Jar noch nicht im Cache liegt, siehe
  [0086](0086-client-jar-von-mojang.md).
- **Jeder Ausgang wartet nach einem Doppelklick auf Enter,** auch ein Fehler
  und ein „n“. `GetConsoleProcessList` sagt, ob die Konsole nur dem Programm
  gehört. Strg+C beendet ohne Warten.
- **Die gebaute Karte liegt als `web/` in beiden Paketen,** gebaut im
  Release-Workflow mit `npm ci` und `npm run build`, samt `lizenzen.txt`.
  Fehlt sie neben der EXE, bietet der Assistent nur „ohne Webserver“.

Wie er sich verhält, steht in [Assistent der EXE](../benutzung/assistent.md).

## Verworfene Alternativen

- **Ein eigenes Fenster mit einer GUI-Bibliothek:** gepackt rund 2 MB mehr,
  46 neue Crates, Lizenzen ausserhalb der Liste des Maintainers und vor
  Windows 11 24H2 ein zweites Binär, damit keine Konsole hinter dem Fenster
  steht. Verworfen vom Maintainer.
- **Schätzung und Lauf im selben Prozess:** Der globale Pool von rayon lässt
  sich nur einmal je Prozess bauen, und weitere Zustände gelten für den
  ganzen Prozess, etwa die Ausgabe als JSON. Jeder zweite Lauf bräuchte
  dafür Sonderwege.
- **Die Karte ins Binär einbetten:** Das Binär würde grösser, auch im Jar des
  Plugins, das die Seite schon getrennt mitbringt.
- **`--defender-exclusion` im Assistenten:** Es braucht Adminrechte. Den
  Hinweis auf den Echtzeitschutz gibt der Lauf wie in der CLI.

## Folgen

- **Grösse:** rfd kostet unter Windows gepackt rund 9 kB, am Prototyp
  gemessen; unter Linux nichts. Die Pakete wachsen um die Karte, siehe
  [Weitergabe](../entwicklung/weitergabe.md).
- **Ohne Dialog in der CI:** Den Ablauf testen Antworten von Hand; Dialog,
  Browser und das Erkennen des Doppelklicks prüft niemand automatisch.
- **Neue Fragen,** etwa zu `--site-*`, kommen nur, wenn jemand sie braucht.
