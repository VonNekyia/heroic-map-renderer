---
title: Assistent der EXE
description: Wie die EXE unter Windows ohne Schalter, etwa per Doppelklick, durch Welt, Ziel, Client-Jar, Einstellungen, Schätzung, Lauf und Webserver fragt, welche Schalter sie dafür nimmt und wann sie auf Enter wartet.
code:
  - renderer/src/cli/assistent.rs
  - renderer/src/cli.rs
---

# Assistent der EXE

Unter Windows startet `heroic-map-renderer.exe` ohne Schalter an einer
Konsole, etwa per Doppelklick im Explorer, einen Assistenten. Er fragt durch
den Weg von der Welt bis zur offenen Karte und ruft für jeden Schritt
dasselbe Binär mit Schaltern auf; jede Kommandozeile zeigt er vorher. Mit
Schaltern bleibt alles, wie es in [Schalter](schalter.md) steht. Unter Linux
gibt es den Assistenten nicht. Warum so:
[0087](../entscheidungen/0087-assistent-auf-der-konsole.md).

## Ablauf

1. **Welt:** der Ordnerdialog von Windows. Der Ordner braucht `level.dat`,
   sonst fragt er neu. Bricht man den Dialog ab, fragt er den Pfad auf der
   Konsole.
2. **Ziel der Kacheln:** ebenso; die Wurzel für `--tiles`, siehe
   [Kacheln](kacheln.md).
3. **Client-Jar:** Liegt es für die Version der Welt noch nicht im Cache,
   zeigt er den Text aus [Assets](assets.md), „Von Mojang laden“, und fragt
   „Laden? (j/n)“, Vorgabe n. Ohne Zustimmung endet er. Liegt es schon im
   Cache, fragt er nicht noch einmal: Die Zustimmung kam beim Laden.
4. **Webserver:** „Danach mit Webserver im Browser ansehen? (j/n)“, Vorgabe
   j. Die Frage kommt nur, wenn die Karte neben der EXE liegt,
   `web/index.html`, wie im Paket, siehe [Installation](installation.md).
5. **Einstellungen,** die Vorgabe in Klammern, Enter nimmt sie:

   | Frage | Vorgabe | Schalter |
   |---|---|---|
   | Kamera | `2:1` | `--camera` |
   | Richtung | `se`, genordet `s` | `--direction` |
   | scale | 32, genordet 16 | `--scale` |
   | Cinematic | n | `--cinematic` |
   | Threads | alle | `--threads` |
   | Grafikkarte | `auto` | `--gpu` |

   Genordet heisst `top-north` und `north-45`.
6. **Schätzung:** dieselben Schalter mit `--estimate`, siehe
   [Kosten](kosten.md). Geht sie nicht, etwa weil der scale nicht zur Kamera
   passt, nennt der Renderer den Grund, und der Assistent fragt, ob er die
   Einstellungen neu fragen soll.
7. **Lauf** nach „Starten? (j/n)“, mit dem Fortschritt wie in der CLI.
8. **Karte:** `--serve` mit dem Ziel, `--web` mit der Karte neben der EXE,
   `--listen 127.0.0.1:0` und `--exit-with-stdin`, siehe
   [Server](server.md). Die Adresse aus der ersten Zeile des Servers öffnet
   er im Browser des Systems. Enter beendet den Server.

Zum Lauf gibt er stets `--download-client-jar` und `--cache-dir` mit dem
Cache des Systems; die Kommandozeile danach zeigt er als „Ohne Assistent:
…“, so lässt sie sich später ohne Assistenten wiederholen.

## Ende und Fehler

- **Nach einem Doppelklick** wartet jeder Ausgang auf Enter, auch ein Fehler
  und ein „n“: Sonst schlösse die Konsole mit der Meldung. Ob es ein
  Doppelklick war, sagt `GetConsoleProcessList` von Windows: Gehört die
  Konsole nur dem Programm, schliesst sie mit ihm.
- **Aus einer Shell gestartet,** etwa aus `cmd`, wartet er nicht.
- **Strg+C** beendet sofort, auch Lauf und Server.
- **Ein Fehler** endet mit Code 1.

## Getestet

In [`renderer/src/cli/assistent.rs`](../../renderer/src/cli/assistent.rs)
spielen Tests den Ablauf mit Antworten von Hand durch, ohne Dialog, Netz und
Browser: alle Vorgaben bis zur Karte, eigene Antworten mit dem Jar schon im
Cache, ein Pfad von Hand nach einem abgebrochenen Dialog, ohne Karte neben
der EXE, ohne Zustimmung, eine gescheiterte Schätzung und „n“ beim Starten.
Sie laufen unter Windows und Linux; der Ordnerdialog, der Browser und
`GetConsoleProcessList` laufen nur am Rechner, nicht in der CI.
