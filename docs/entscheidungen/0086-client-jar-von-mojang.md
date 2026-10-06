---
title: "0086: Das Client-Jar von Mojang, nur mit Zustimmung"
description: Warum der Renderer Assets und Daten auf Wunsch selbst aus dem Client-Jar von Mojang nimmt, nur nach ausdrücklicher Zustimmung mit --download-client-jar, mit SHA-1 und Grösse je Version im Binär, über HTTP geladen, mit eigenem ZIP-Leser einmal in einen Cache ausgepackt, jede Datei mit der Bauzeit des Jars; welche Wege verworfen sind.
status: gilt
date: 2026-10-06
issues: [147, 152, 153]
code:
  - renderer/src/cli/client.rs
  - renderer/src/cli/zip.rs
  - renderer/src/cli.rs
  - renderer/Cargo.toml
---

# 0086: Das Client-Jar von Mojang, nur mit Zustimmung

## Anlass

Bisher holt der Betreiber Assets und Daten von Hand aus dem Client-Jar
([Assets und Biomdaten](../benutzung/assets.md)). Plugin (#153) und EXE
(#152) brauchen das ohne Handarbeit. Mitliefern dürfen sie das Jar nicht:
Es gehört Mojang. Issue #147; der Weg ist am 05.10. im Review entschieden,
die Zustimmung am 06.10. vom Maintainer.

## Entscheidung

- **Nur mit Zustimmung:** Der Renderer lädt das Client-Jar nur mit
  `--download-client-jar`. Der Schalter ist die Zustimmung, Vorgabe aus.
  Ohne ihn und ohne `--assets` bricht ein Lauf ab und nennt den Text des
  Maintainers: Version und Grösse, das Jar gehört Mojang und darf nicht
  weitergegeben werden, der Nutzer besitzt Minecraft: Java Edition und
  nimmt die EULA an. Die EXE fragt dasselbe mit j/n, das Plugin hat einen
  Eintrag in `config.yml`, Vorgabe `false`. `eula=true` des Servers zählt
  nicht: Es gilt dem Server-Programm, das es ohne Kauf gibt.
- **SHA-1 und Grösse je Version im Binär,** in `JARS` in
  `renderer/src/cli/client.rs`, wie die übrigen Tabellen je Version. Die
  Adresse folgt aus dem SHA-1: `piston-data.mojang.com/v1/objects/<sha1>/client.jar`,
  über HTTP. Der SHA-1 sichert den Inhalt, ein verändertes Jar fällt an ihm
  auf. Keine Umleitung, höchstens die Grösse, höchstens 10 min.
- **Die Version** aus `Data.DataVersion` in `level.dat`, wie in
  [0065](0065-sicht-in-der-ecke-nach-der-version.md): die neueste bekannte,
  deren DataVersion nicht grösser ist; älter als alle die älteste, ohne
  Angabe die neueste. `--client-version` wählt selbst.
- **Ein Cache, einmal ausgepackt:** `assets/` und aus `data/` Biome,
  Bannermuster und Dimensionen, nach `client-<version>-<sha1>` unter
  `--cache-dir` oder dem Cache des Systems. Jede Datei bekommt die jüngste
  DOS-Zeit im Jar, seine Bauzeit: Neu ausgepackt bleibt der Fingerabdruck
  aus `stand.rs` gleich, ein anderes Jar gibt einen anderen. Ausgepackt
  wird in einen Ordner des Prozesses und dann umbenannt, so sieht kein Lauf
  einen halben Cache.
- **Die Basis zuerst:** `assets` und `data` des Jars stehen vor allen
  `--assets` und `--data`. Overlay-Packs und Datenpakete kommen wie bisher
  dazu. Ein eigener Schalter für die Basis entfällt: Wer `--assets` ohne
  den Schalter gibt, bekommt alles wie bisher.
- **Nie weitergegeben:** Liegt der Cache unter `--tiles`, bricht der Lauf ab,
  sonst lieferte `--serve` ihn aus.
- **Kaum neue Crates:** hyper bekommt das Feature `client`; mit ihm kommen
  `want` und `try-lock` dazu, beide MIT und klein. SHA-1 kommt aus ring,
  das schon mit HTTPS kam. Das ZIP liest ein eigener Leser über flate2: nur
  Stored und Deflate, ohne Zip64, mit Länge und CRC-32 je Datei und ohne
  Namen, die aus dem Ziel führen.
- **Schalter englisch** wie die übrigen: `--download-client-jar`,
  `--client-version`, `--cache-dir`. Den Schlüssel in `config.yml` legt das
  Plugin fest.

Wie es sich verhält, steht in [Assets und Biomdaten](../benutzung/assets.md),
„Von Mojang laden“.

## Verworfene Alternativen

- **Das Versionsverzeichnis von Mojang über HTTPS,** wie #147 es zuerst
  vorsah: Es ist nicht signiert, also bräuchte es TLS mit Wurzelzertifikaten.
  Mitgelieferte Wurzeln altern mit dem Binär, die des Systems fehlen in
  schlanken Containern, und beides brächte mehr Crates. Eine Version, die
  das Binär nicht kennt, könnte es zwar laden, aber für ihre Blöcke braucht
  es ohnehin ein neues Binär.
- **Das Jar direkt lesen statt auszupacken:** Jede Stelle, die heute eine
  Datei öffnet, liefe über eine zweite Quelle, mit eigenen Tests für die
  Regeln der Packs. Gewinn wären 11 081 Dateien weniger auf der Platte; das
  verlangt niemand (Regel 22).
- **`--data` aus dem Server-Jar,** das Paper ohnehin im Cache hat: Die Daten
  dort sind dieselben, aber Texturen, Modelle und Blockstates gibt es nur im
  Client-Jar. Der Download bliebe, mit zwei Quellen statt einer.
- **Eine Crate für ZIP:** mehr Code im Binär für ein Format, das ein
  geprüftes Jar braucht, und mehr Abhängigkeiten.
- **Die Zustimmung über `eula=true` des Servers:** verworfen vom
  Maintainer, siehe oben.
- **Als Zeit eine Konstante oder die des Auspackens:** Mit der Zeit des
  Auspackens bräche `--update` nach jedem neuen Cache ab. Eine Konstante gäbe
  einem anderen Jar mit denselben Pfaden und Grössen denselben
  Fingerabdruck, und `--update` mischte alte und neue Kacheln.

## Folgen

- **Grösse:** gepackt rund 0,08 MB je Binär, am Prototyp gemessen. Den
  Stand nennt [Weitergabe](../entwicklung/weitergabe.md), „Grenze“.
- **Netz:** Wo nur HTTPS hinausgeht, scheitert der Download. Dann bleibt der
  Weg von Hand, und die Meldung nennt ihn.
- **Mojang ändert die Adresse:** Dann scheitert jede Version im Binär
  zugleich, bis ein neues Binär kommt.
- **Eine neue Version** braucht einen Eintrag in `JARS`, Skill
  [`tabellen-neu-erzeugen`](../../skills/tabellen-neu-erzeugen/SKILL.md).
- **Erster Wechsel:** Wer von eigenen Wurzeln auf den Cache wechselt, bekommt
  einen neuen Fingerabdruck und einmal einen vollen Lauf.
