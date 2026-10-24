---
title: Warteschlange der Kacheln
description: Kachelstrom der Webkarte auf der grossen Welt mit der Grundschicht, ohne und mit einer eigenen Warteschlange der Kacheln (Vorrang der Grundschicht, höchstens 6 zugleich, nicht mehr gebrauchte fallen heraus), laufende zu Ende geladen oder abgebrochen, gedrosselt auf 50 und 10 Mbit/s; Abbrüche, neue Verbindungen, Bytes.
date: 2026-10-24
commits: [0887686, 284fdd1, ac0eaa2]
code:
  - web/src/main.ts
---

# Warteschlange der Kacheln

Eine eigene Warteschlange der Kacheln bringt über HTTP/1.1 nichts, was
Chromium nicht schon tut. Ohne sie brechen bei 10 Mbit/s je Reihe 39,5
Kacheln ab, und doch entstehen nur 4 neue Verbindungen: Die Abbrüche treffen
Anfragen, die noch in Chromiums eigener Schlange standen. Mit ihr laufen nur
echte Anfragen; lädt die Karte die nicht mehr gebrauchten zu Ende, kostet das
+26–31 % Bytes, bricht sie sie ab, 10 statt 3 neue Verbindungen. Beim Ziehen
liegt der Gewinn im Rauschen.

## Aufbau

Wie in [Grundschicht](2026-10-24-grundschicht.md) und
[Rand der feinen Kacheln](2026-10-24-rand-feiner-kacheln.md): die grosse
Welt, iso 4:3 mit `cinematic`, `zoom=-3`, der Webserver des Plugins direkt
über einen kleinen Node-Server des Frontends, der die Header durchreicht;
Chromium 1600 × 900, gedrosselt über CDP auf 50 und 10 Mbit/s bei 50 ms
RTT; derselbe Ablauf.

- **Stände:**
  - D1: die Grundschicht, ohne Warteschlange (`0887686`);
  - D3: dazu die Warteschlange, laufende laden zu Ende (`284fdd1`);
  - D3 mit Abbruch: laufende, nicht mehr gebrauchte brechen ab, ohne `src`
    (`ac0eaa2`); beide im Tag `versuch/warteschlange`.
- **Neu gezählt:** abgebrochene Kacheln aus `requestfailed`; neue
  Verbindungen als Einträge von Resource Timing mit
  `connectEnd > connectStart`.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Zwei Reihen, je zwei Läufe je
Stand, abwechselnd, 15 s Pause dazwischen: D1 gegen D3, dann D3 gegen D3 mit
Abbruch. D3 lief so in beiden Reihen.

## Ergebnis

Mittel aus zwei Läufen.

| Stand | schwarz beim Ziehen, 50 / 10 Mbit/s | Ziehen bis voll, 50 / 10 Mbit/s | Abbrüche bei 10 Mbit/s | neue Verbindungen bei 10 Mbit/s | Bytes der Reihe, 50 / 10 Mbit/s |
|---|---|---|---|---|---|
| D1 | 4,2 / 6,4 % | 458 / 483 ms | 39,5 | 4 | 5,27 / 3,75 MB |
| D3, Reihe 1 | 2,5 / 6,8 % | 406 / 398 ms | 0 | 3 | 5,38 / 4,91 MB |
| D3, Reihe 2 | 1,8 / 5,3 % | 372 / 549 ms | 0 | 3 | 5,44 / 4,73 MB |
| D3 mit Abbruch | 2,1 / 4,7 % | 388 / 532 ms | 12 | 10 | 5,20 / 4,22 MB |

- Derselbe Build gab bei 10 Mbit/s einmal 398 und einmal 549 ms bis voll;
  so weit streut das Ziehen zwischen zwei Reihen.
- Die Anfragen sinken mit der Warteschlange bei 10 Mbit/s um 20–25 %, weil
  wartende herausfallen.

## Schluss

- **Chromium hat die Warteschlange schon:** Über HTTP/1.1 öffnet es je
  Host 6 Verbindungen und hält den Rest selbst zurück; ein Abbruch dort
  kostet nichts.
- **Mit eigener Warteschlange** kostet jede nicht mehr gebrauchte Kachel
  entweder Bytes oder eine Verbindung.
- Verworfen, siehe [Warteschlange der Kacheln](../versuche/warteschlange-der-kacheln.md).
