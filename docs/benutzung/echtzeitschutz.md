---
title: Echtzeitschutz unter Windows
description: Warum Microsoft Defender einen Export ausbremst, was --defender-exclusion tut und wie man die Ausnahme von Hand setzt und wieder entfernt.
code:
  - renderer/src/cli.rs
---

# Echtzeitschutz unter Windows

Unter Windows prüft der Echtzeitschutz von Microsoft Defender jede Datei,
die der Export schreibt, und ein Vollrender schreibt über drei Millionen.
Mit einer Ausnahme für den Kachelordner braucht ein Export ein Drittel
weniger Zeit, gemessen in
[2026-09-26, Echtzeitschutz](../messungen/2026-09-26-echtzeitschutz.md).
`--defender-exclusion` setzt die Ausnahme, nur mit Zustimmung und nur für
einen neuen, leeren oder Kachelordner (`setze_ausnahme` und
`warum_keine_ausnahme` in
[`renderer/src/cli.rs`](../../renderer/src/cli.rs)).

## `--defender-exclusion`

Die Ausnahme setzt `--defender-exclusion` beim Export. Welchen Ordner es
ausnimmt, sagt der Lauf, bevor Windows nach Adminrechten fragt; in der
Abfrage selbst steht der Befehl nur kodiert. Nur mit Zustimmung kommt das
Verzeichnis von `--tiles` dazu, und nur, wenn es neu, leer oder schon ein
Kachelbaum mit `map.json` ist, nie die Wurzel eines Laufwerks. Sonst nähme
ein Versehen in `--tiles` das Benutzerverzeichnis oder ein ganzes Laufwerk
vom Virenschutz aus. Den Befehl zum Entfernen nennt der Lauf am Anfang und
am Ende. Warum so eng:
[0022](../entscheidungen/0022-defender-ausnahme-nur-mit-zustimmung.md).

## Von Hand

Von Hand geht es in einer PowerShell als Administrator, dort kommt sie nach
dem Render auch wieder heraus:

```powershell
Add-MpPreference -ExclusionPath '<kachelordner>'
Remove-MpPreference -ExclusionPath '<kachelordner>'
```

Solange sie besteht, prüft Defender in diesem Ordner nichts, auch keine
Datei, die jemand anderes dort ablegt; der Export selbst legt dort nur
Kacheln und `map.json` ab.

## Der Hinweis beim ersten Export

Beim ersten Export in ein neues oder leeres Verzeichnis nennt der Lauf
beide Befehle für genau diesen Ordner. Ob die Ausnahme schon besteht, sieht
er ohne Adminrechte nicht, deshalb sagt er es nur dieses eine Mal.

## Ohne Ausnahme

Ohne Ausnahme geht es unter Windows 11 mit einem Dev Drive, einem eigenen
ReFS-Laufwerk, auch als virtuelle Festplatte auf einem vorhandenen. Der
Echtzeitschutz bleibt dort an, prüft aber im Leistungsmodus erst nach dem
Schreiben; die Rechenzeit dafür fällt trotzdem an. Gemessen ist das hier
nicht.
