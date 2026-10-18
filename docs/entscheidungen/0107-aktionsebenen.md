---
title: "0107: Aktionsebenen und Knöpfe je Spieler"
description: Warum Knöpfe an Objekten hängen und nicht in den Dateien der Ebenen stehen, warum eine Aktionsebene je Spieler eigene Objekte hat, wie sich ein Spieler auf der Webkarte mit einem Link aus dem Spiel anmeldet, warum der Server des Renderers die Sitzung selbst ausstellt und Klicks als Zeile auf stdout ans Plugin gibt; was verworfen ist und was es kostet.
status: gilt
date: 2026-10-18
issues: [294]
code:
  - renderer/src/cli/token.rs
  - renderer/tests/fixtures/token.json
---

# 0107: Aktionsebenen und Knöpfe je Spieler

## Anlass

Der User will Knöpfe in den Tafeln der Ebenen, deren Klick als Ereignis an
ein Plugin geht, mit Inhalten je Spieler (#294). Seine Entscheide:

- Je Spieler: Jeder schaltet selbst und sieht nur seine Inhalte.
- Im Spiel **und** auf der Webkarte; die Webkarte braucht dafür eine
  Anmeldung.
- Wir liefern nur die Werkzeuge. Was ein Knopf bewirkt, baut das Plugin,
  dem er gehört.

Der erste Anwender ist ein Plugin für Schiffsrouten: ein Knopf an der
Stadt eines anderen Plugins und Routen zwischen den Städten, die der
Spieler eingeschaltet hat. Plugin, Frontend und Backend haben den Plan
gemeinsam geschrieben; der Reviewer hat ihn am 18.10. freigegeben.

## Entscheidung

- **Knöpfe hängen an Objekten.** Ein Plugin legt einen Knopf an und hängt
  ihn an ein Objekt einer beliebigen Ebene, auch einer fremden. Die Ansicht
  setzt ihn unter die Tafel, mit einem Zustand an oder aus je Spieler. In
  Dateien unter `layers/` steht er nie, denn die sind für alle gleich.
- **Aktionsebene:** eine Ebene mit `"player": true` in `layers.json`, die
  je Spieler zu den gemeinsamen Objekten eigene hat. Im Mod schickt das
  Plugin jedem seine Fassung; im Web liefert sie der Server des Renderers
  unter `api/`, nur mit Sitzung.
- **Ein Klick** ist ein Ereignis im Hauptthread des Spielservers mit Spieler,
  Ebene, Objekt und Knopf. Im Spiel kommt er über den Kanal. Im Web schickt
  ihn die Seite an `POST api/aktion`; der Server prüft ihn und schreibt
  eine Zeile `Aktion: {…}` auf stdout, die das Plugin liest.
- **Anmeldung im Web:** `/karte anmelden` im Spiel schickt einen Link mit
  einem Anmelde-Token im Fragment. Die Seite gibt es an `POST api/anmelden`;
  der Server nimmt jedes einmal und setzt ein Cookie mit einem
  Sitzungs-Token, das er selbst ausstellt. Beide Tokens haben die Form des
  Download-Tokens, dasselbe Geheimnis und eigene Fassungen, 2 und 3, mit
  einem Zähler je Spieler; `/karte abmelden` erhöht ihn und beendet so alle
  Sitzungen. Byte für Byte in [Plugin](../plugin.md), „Anmeldung und
  Sitzung“.
- **Was der Reviewer festgelegt hat:**
  - Eine Sitzung gilt 7 Tage.
  - Knöpfe und Inhalte je Spieler an Ebenen mit `permission` gibt es nur im
    Mod: Die Rechte eines Spielers kennt das Plugin nur, solange er online
    ist.
  - Jedes Plugin darf Knöpfe an fremde Objekte hängen, nach Absprache mit
    deren Besitzer. Geprüft wird die Absprache nicht; der Besitzer muss
    nichts tun.
  - Die Abrufe je Spieler gehen mit `Cache-Control: private, no-store`,
    wegen geteilter Rechner.

Das Format steht in [Ebenen](../benutzung/ebenen.md), „Aktionsebenen und
Knöpfe“.

## Verworfene Alternativen

- **Knöpfe in der Datei der Ebene:** Ihr Zustand ist je Spieler, die Datei
  für alle gleich. Je Spieler eine Kopie jeder Ebene wäre gross und
  verriete über die Dateinamen, wer welche hat.
- **Ein Haken im Plugin der fremden Ebene,** über den es Knöpfe annimmt:
  Jedes Paar von Plugins bräuchte dann eigenen Code.
- **Ein eigener Dienst im Plugin,** mit zweitem Port, für Anmeldung und
  Klicks: Der Server des Renderers liefert die Karte schon, am selben
  Ursprung. Ein zweiter Port bräuchte eigenes TLS, eigene Freigaben im
  Proxy und CORS.
- **Push zum Browser,** etwa über WebSocket: eine neue Abhängigkeit für
  Daten, die klein sind und alle 30 s reichen (Regel 22).
- **Die Sitzung stellt das Plugin aus:** Der Server müsste dann bei jeder
  Anmeldung nachfragen. Er hat das Geheimnis schon; die Fassung im
  unterschriebenen Inhalt trennt die Arten.
- **Das Token in der Query:** Es stünde in Logs und im Referer. Im Fragment
  geht es nie zum Server.
- **`private, no-cache` mit ETag:** Der Browser behielte die Inhalte eines
  Spielers auf der Platte, auch auf einem geteilten Rechner.

## Folgen

- **Teile:**
  1. Renderer: diese Entscheidung, das Format, die Tokens mit Testvektoren.
  2. Plugin: die API mit Inhalten je Spieler, Knöpfen und dem Ereignis; der
     Kanal für den Mod.
  3. Renderer: der Server mit `--player-dir` und `api/`.
  4. Plugin: Anmelden und Abmelden im Spiel, Dateien je Spieler, die Zeilen
     `Aktion:` lesen.
  5. Webkarte: Anmeldung, Aktionsebenen, Knöpfe.
  6. Mod: Knöpfe in der Tafel.
- **Nur im Speicher des Servers:** benutzte Anmeldungen und abgemeldete
  Sitzungen. Nach einem Neustart ginge ein Link innerhalb seiner 5 min ein
  zweites Mal, und ein abgemeldetes Cookie gälte wieder bis zum Ablauf.
  Dagegen hilft `/karte abmelden`, das über den Zähler alle Sitzungen
  beendet.
- **Ohne HTTPS** gehen Token und Cookie im Klartext, wie beim Download.
- **Kosten nach Regel 26,** geschätzt:
  - keine Renderzeit, nichts davon zeichnet Kacheln;
  - im Speicher des Plugins die Inhalte je Spieler und die Zustände der
    Knöpfe, beim ersten Anwender einige Linien je Spieler;
  - auf der Platte kleine JSON-Dateien je Spieler mit Inhalt.
