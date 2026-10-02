---
title: "0022: Die Ausnahme im Echtzeitschutz nur mit Zustimmung"
description: Warum der Export die Defender-Ausnahme nur auf Wunsch, nach der Adminabfrage und nur für einen neuen, leeren oder Kachelordner setzt.
status: gilt
date: 2026-09-26
issues: [10]
code:
  - renderer/src/cli.rs
---

# 0022: Die Ausnahme im Echtzeitschutz nur mit Zustimmung

[0054](0054-baeume-unter-einer-wurzel.md) löst einen Teil ab: Hinweis und
Ausnahme gibt es auch für eine Wurzel mit `trees.json` oder mit einem Baum
darin.

## Anlass

Unter Windows prüft Microsoft Defender jede Datei, die der Export schreibt.
Mit einer Ausnahme für den Kachelordner braucht ein Export ein Drittel
weniger Zeit, siehe
[2026-09-26, Echtzeitschutz](../messungen/2026-09-26-echtzeitschutz.md).

## Entscheidung

Der erste Export in ein neues oder leeres Verzeichnis nennt die Befehle für
eine Ausnahme für genau diesen Ordner, ohne anzuhalten.
`--defender-exclusion` setzt die Ausnahme, nachdem Windows nach Adminrechten
gefragt hat; den Ordner nennt der Lauf vorher, in der Abfrage selbst steht
der Befehl nur kodiert. Ohne Zustimmung läuft der Export ohne sie. Hinweis
und Ausnahme gibt es nur für einen Ordner, den es noch nicht gibt, der leer
ist oder schon eine `map.json` hat, nie für die Wurzel eines Laufwerks. Den
Befehl zum Entfernen nennt der Lauf am Anfang und am Ende. Siehe
[Echtzeitschutz unter Windows](../benutzung/echtzeitschutz.md).

## Verworfene Alternativen

- **Die Ausnahme für jedes `--tiles`.** Ein Versehen in `--tiles` nähme das
  Benutzerverzeichnis oder ein ganzes Laufwerk vom Virenschutz aus.

## Folgen

- Entfernen muss man die Ausnahme selbst.
- Ob sie schon besteht, sieht der Lauf ohne Adminrechte nicht; der Hinweis
  kommt deshalb nur beim ersten Export in einen Ordner.
- Solange sie besteht, prüft Defender in diesem Ordner nichts, auch keine
  fremde Datei.
