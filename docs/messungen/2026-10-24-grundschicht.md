---
title: Grundschicht
description: Kachelstrom der Webkarte auf der grossen Welt vor und mit einer gröberen Grundschicht unter den Kacheln, fünf Stände (2 und 3 Stufen, mit und ohne Ring), gedrosselt auf 50 und 10 Mbit/s; Anteil schwarz beim Ziehen und Zoomen, Bytes, Zeit bis zum vollen Bild.
date: 2026-10-24
commits: [76ea0cc]
code:
  - web/src/main.ts
---

# Grundschicht

Eine Grundschicht drei Stufen gröber, nur für das Fenster, nimmt das
Schwarz beim Hineinzoomen ganz weg und senkt es beim Ziehen von 8,2 auf
1,8 % des Fensters bei 50 Mbit/s und von 9,3 auf 5,3 % bei 10 Mbit/s. Das
volle Bild nach dem Laden kommt 45 bzw. 37 % früher. Das kostet 3–15 % mehr
Bytes. Ein Ring von einer Kachel nähme den Rest, für +26 bis +66 %.

## Aufbau

- **Welt:** die grosse Welt, der Baum iso 4:3 aus Südost mit `cinematic`,
  `zoom=-3`, die Mitte im Gebiet der Regionen.
- **Server:** der Webserver des Plugins direkt, ohne den Proxy der
  Website. Auf der Produktion hält nginx die Verbindung offen; der Proxy
  des Spiegels schliesst sie nach jeder Antwort und wäre für sie zu
  schwarz gemalt.
- **Seite:** je Stand ein Build, ausgeliefert von einem kleinen Node-Server
  des Frontends, der `/tiles/` mit allen Headern an den Webserver
  durchreicht, auf demselben Ursprung. So wirkt der HTTP-Cache wie dort;
  Playwright routet nichts. Kacheln kommen mit `no-cache` und ETag.
- **Browser:** Chromium aus Playwright, headless, 1600 × 900,
  `devicePixelRatio` 1. Gedrosselt über CDP auf 50 und 10 Mbit/s je
  Richtung bei 50 ms RTT, dazu ein Lauf ohne Drossel.
- **Ablauf je Drossel:** laden; sechsmal 500 px nach links ziehen; dreimal
  1 000 px zurück; zweimal mit dem Mausrad eine Stufe hinaus und wieder
  hinein; ein zweiter Besuch.
- **Werte:**
  - Schwarz: der Anteil eines Rasters von 40 × 22 Punkten im Fenster ohne
    geladene Kachel, je 100 ms bis voll oder 4 s, über die Zeit gemittelt.
  - Bytes: `transferSize` der Kacheln aus Resource Timing, ohne den zweiten
    Besuch.
  - Voll: Zeit bis 99,5 % des Rasters gedeckt sind.
- **Stände:** vorher `76ea0cc` (master); die übrigen auf dem Zweig der
  Grundschicht, je mit `GRUND.stufen` 2 oder 3 und mit oder ohne Ring von
  einer Kachel um das Fenster.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Je Stand zwei Läufe,
abwechselnd, 15 s Pause dazwischen. Skripte `m7/strom.mjs` und
`m7/server.mjs` des Frontends, nicht im Repo.

Die Spalte „laden“ der Skripte zählt bei 10 Mbit/s nur, was bis zum vollen
Bild und 500 ms danach kam; verglichen werden darum die Bytes der ganzen
Reihe.

## Ergebnis

Mittel aus zwei Läufen.

| Stand | schwarz beim Ziehen, 50 / 10 Mbit/s | schwarz beim Hineinzoomen, 10 Mbit/s | Bytes der Reihe, 50 / 10 Mbit/s | Laden bis voll, 50 / 10 Mbit/s |
|---|---|---|---|---|
| vorher | 8,2 / 9,3 % | 19,4 % | 4,8 / 4,0 MB | 960 / 1 773 ms |
| 2 Stufen ohne Ring | 1,6 / 5,8 % | 0,2 % | 5,3 / 4,1 MB | 810 / 1 426 ms |
| **3 Stufen ohne Ring** | **1,8 / 5,3 %** | **0** | **5,5 / 4,1 MB** | **528 / 1 122 ms** |
| 3 Stufen mit Ring | 0 / 1,3 % | 0 | 7,4 / 5,0 MB | 544 / 1 119 ms |
| 2 Stufen mit Ring | 0 / 0 | 0 | 8,0 / 5,7 MB | 754 / 1 452 ms |

Ohne Drossel ist das Bild in allen Ständen nach 150–260 ms voll, und
beim Ziehen fehlen vorher 5 %, mit jeder Grundschicht 0 %.

## Schluss

- **3 Stufen ohne Ring:** Mehr Stufen decken mit weniger Kacheln, und das
  volle Bild kommt am schnellsten. Das schwarze Hineinzoomen ist weg.
- **Der Ring ist teuer:** Eine grobe Kachel wiegt hier etwa so viel wie
  eine feine, rund 100 kB. Geschätzt waren +20–30 % Bytes für den Ring,
  gemessen sind es mit 2 Stufen +43 bis +66 %.
- **Offen:** Der Rest beim Ziehen, 2–5 % des Fensters. Ob ihn der Rand der
  feinen Kacheln oder der Ring der Grundschicht nimmt, entscheidet die
  nächste Messung.
