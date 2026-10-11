---
title: "0106: Der Download als ein Strom im ZIP"
description: Warum der Server einen Baum für den Mod unter /download/<baum>.zip als einen Strom liefert, ein ZIP ohne Kompression und ohne zentrales Verzeichnis mit dem Eintrag ende, in fester Reihenfolge von grob nach fein, mit mehreren Threads voraus gelesen und mit ?nach= fortzusetzen; was verworfen ist und was es kostet.
status: gilt
date: 2026-10-11
issues: [289]
code:
  - renderer/src/cli/server.rs
  - renderer/src/cli/server/strom.rs
---

# 0106: Der Download als ein Strom im ZIP

## Anlass

Der Mod holte einen Baum Kachel für Kachel unter `/download/`. Ein grosser
Baum hat bei 4 px rund 193 000 Kacheln und 5 GB; der Download dauerte rund
20 min. Der User will Sekunden (#289). Zwei Gründe:

- **Lesen kalter Dateien:** Unter Windows kostet jede Datei beim ersten
  Lesen nach dem Schreiben einige Millisekunden, die nicht auf die Platte
  warten, nach einer Probe vom 11.10. rund 6 ms. Der Server liest mit den
  Threads aus `--threads`, beim Plugin bisher einer, also eine Datei zur
  Zeit. Mit 8 Threads lasen sich kalte Kacheln in der Probe rund sieben Mal
  so schnell.
- **Eine Verbindung je Kachel** hinter einem Proxy, der je Anfrage eine neue
  Verbindung öffnet: Hunderttausende kurze Verbindungen füllen Tabellen
  und kosten je Kachel eine Laufzeit über das Netz.

## Entscheidung

- **Ein Strom je Baum:** `GET /download/<baum>.zip` mit demselben Token wie
  die Einzelabrufe gibt alles in einer Antwort, chunked. Die Einzelabrufe
  bleiben für den Abgleich mit ETag.
- **ZIP ohne Kompression:** WebP ist schon gepackt. Je Eintrag nur der
  lokale Kopf mit CRC-32 und Grösse; der Server liest jede Datei erst ganz,
  so braucht er keinen Data Descriptor. Der Mod liest mit dem
  `ZipInputStream` des JDK und braucht keinen eigenen Parser.
- **Kein zentrales Verzeichnis, dafür `ende`:** Der Mod liest der Reihe nach.
  Ohne Verzeichnis sähe ein Strom, der zwischen zwei Dateien abbricht, ganz
  aus; erst der leere letzte Eintrag `ende` zeigt, dass er vollständig ist.
- **Feste Reihenfolge, von grob nach fein:** `map.json`, `manifest`, dann
  die Kacheln Stufe für Stufe ab 0. So zeigt der Mod früh die ganze Welt
  grob. Höhen und Boden sind nicht dabei: Der Mod legt sie nicht ab, der
  Teleport holt sie frisch über `/tiles/`.
- **Fortsetzen mit `?nach=<z>/<x>/<y>`:** Bricht ein Strom ab, setzt der Mod
  hinter der letzten abgelegten Kachel fort. Der Deckel des Tokens zählt
  nur, was geht; ein Abbruch bei der Hälfte kostet ihn nicht noch einmal.
- **Voraus lesen:** 8 Threads je Strom lesen voraus, höchstens 64 Dateien
  vor dem Senden; gesendet wird in der festen Reihenfolge. Die Threads
  laufen neben denen aus `--threads`, damit ein langer Download die Karte
  nicht anhält.
- **Grenzen:** ein Strom je Zufall eines Tokens, zusammen höchstens 4.
  Prüfungen und Deckel wie bei den Einzelabrufen.
- **Eine Zeile im Log je Strom** mit Baum, Stufe, Spieler, Dateien, MB,
  Dauer und dem Grund eines Abbruchs, für die nächste Diagnose.

Entscheid des Reviewers am 11.10. auf Wunsch des Users, abgestimmt mit dem
Mod. Die Einzelheiten stehen in [Server](../benutzung/server.md),
„Download als Strom“.

## Verworfene Alternativen

- **tar:** einfach zu schreiben, aber der Mod bräuchte einen eigenen Leser;
  das JDK liest ZIP.
- **ZIP mit zentralem Verzeichnis:** Über 65 535 Einträge und 4 GiB braucht
  es ZIP64, und der Server müsste das Verzeichnis aller Einträge bis zum
  Ende im Speicher halten. Der Mod liest es nicht.
- **Deflate:** WebP wird kaum kleiner, und das Packen kostet CPU neben dem
  Spiel.
- **`Content-Length`:** Dafür müsste der Server jede Datei vor dem ersten
  Byte kennen; ändert der Renderer eine, stimmte die Länge nicht mehr.
  `X-Heroic-Bytes` nennt die erwartete Grösse für die Anzeige.
- **Nur mehr Threads für die Einzelabrufe:** hilft gegen das Lesen, nicht
  gegen eine Verbindung je Kachel. Das Plugin startet `--serve` trotzdem
  mit `--threads 4`, für Updates und ältere Mods.
- **Seriell lesen im Strom:** bliebe so langsam wie heute, denn das Lesen
  kalter Dateien ist der grösste Teil der Zeit.
- **Höhen und Boden im Strom:** kämen vor der ersten Kachel, auf der grossen
  Welt rund 86 MB, und der Mod legt sie nicht ab.

## Kosten nach Regel 26

Geschätzt, nicht gemessen:

- **Live-Rendern und erster Render:** keine; der Server zeichnet nichts.
  Der Zeichenstand bleibt.
- **Arbeitsspeicher:** je Strom höchstens 64 gelesene und 16 wartende
  Dateien, bei Kacheln von 24 bis 60 KiB wenige MB; bei 4 Strömen unter
  20 MB.
- **CPU:** Die Leser warten meist auf das System. Je laufendem Strom 9
  Threads, bei 4 Strömen 36.
- **Platz:** keiner; der Strom entsteht beim Senden, ohne Zwischendatei.

## Folgen

- Das Plugin ändert am Protokoll nichts; die `freigabe` bleibt. Es startet
  `--serve` mit `--threads 4` (heroic-map-renderer-plugin#75).
- Der Mod lädt über den Strom und fällt bei `404`, also einem Renderer ohne
  ihn, auf die Einzelabrufe zurück.
