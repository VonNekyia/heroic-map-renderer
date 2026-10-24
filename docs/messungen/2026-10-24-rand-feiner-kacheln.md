---
title: Rand der feinen Kacheln
description: Kachelstrom der Webkarte auf der grossen Welt mit der Grundschicht, ohne und mit einem Rand von einer Kachel für die feinen Kacheln samt updateInterval 100 ms, gedrosselt auf 50 und 10 Mbit/s; dazu doppelte Anfragen nahe minNativeZoom.
date: 2026-10-24
commits: [0500976, cb05108]
code:
  - web/src/main.ts
---

# Rand der feinen Kacheln

Ein Rand von einer Kachel für die feinen Kacheln nimmt das Schwarz beim
Ziehen bei 50 Mbit/s fast ganz weg, 3,6 → 0,6 % des Fensters. Bei 10 Mbit/s
wird es schlechter, 6,0 → 8,9 %, denn die Kacheln des Rands nehmen den
sichtbaren die Bandbreite. Das kostet +60 % Bytes und +80 % Anfragen. Nahe
`minNativeZoom` gibt es keine doppelten Anfragen.

## Aufbau

Wie in [Grundschicht](2026-10-24-grundschicht.md): die grosse Welt, iso 4:3
mit `cinematic`, `zoom=-3`, der Webserver des Plugins direkt über einen
kleinen Node-Server des Frontends, der die Header durchreicht; Chromium
1600 × 900, gedrosselt über CDP auf 50 und 10 Mbit/s bei 50 ms RTT, dazu
ohne Drossel; derselbe Ablauf und dieselben Werte.

- **Stände:**
  - vorher: die Grundschicht, 3 Stufen ohne Ring (`0500976`);
  - nachher: dazu `rand` 1 an den feinen Kacheln über
    `_getTiledPixelBounds` und `updateInterval` 100 ms (`cb05108`, Tag
    `versuch/rand-feiner`).
- **Doppelte Anfragen:** je Stand ein weiterer Lauf auf `zoom=-9`, wo die
  Grundschicht auf `minNativeZoom` klemmt und dieselbe Stufe nimmt wie die
  feinen Kacheln; gezählt die Adressen, die je Aktion mehr als einmal
  angefragt wurden.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Je Stand zwei Läufe auf
`zoom=-3` abwechselnd, dann je einer auf `zoom=-9`, 15 s Pause dazwischen.

## Ergebnis

Mittel aus zwei Läufen.

| | schwarz beim Ziehen, ohne / 50 / 10 Mbit/s | Ziehen bis voll, 50 / 10 Mbit/s | Bytes der Reihe, 50 / 10 Mbit/s | Anfragen, 50 / 10 Mbit/s |
|---|---|---|---|---|
| Grundschicht | 0,9 / 3,6 / 6,0 % | 475 / 496 ms | 5,4 / 4,1 MB | 128 / 154 |
| Grundschicht + Rand | 0 / 0,6 / 8,9 % | 395 / 612 ms | 8,7 / 6,5 MB | 234 / 270 |

- Die Grundschicht lag in dieser Reihe bei 3,6 bzw. 6,0 %, in der Reihe
  davor bei 1,8 bzw. 5,3 %. So weit streut es zwischen zwei Reihen.
- **Auf `zoom=-9`:** 0 doppelte Adressen, in keiner Aktion und keinem Lauf.
  Chromium holt ein Bild mit derselben Adresse im selben Dokument nur
  einmal.

## Schluss

- **Der Rand lohnt so nicht:** Bei schmaler Leitung bremst er das
  Sichtbare aus und kostet viel. Verworfen, siehe
  [Rand der feinen Kacheln](../versuche/rand-feiner-kacheln.md).
- **Die Grundschicht** braucht nahe `minNativeZoom` nichts Eigenes.
