---
title: Start der Webkarte
description: Zeit von der Navigation bis zur ersten Kachel und zum ersten Objekt einer Ebene auf der grossen Welt, vor und nach dem vorgezogenen Laden von layers.json, api/ich und map.json aus ?tree=, ohne und mit 150 ms Umlaufzeit, je drei kalte Läufe.
date: 2026-10-24
commits: [dbbc38a]
code:
  - web/src/main.ts
  - web/src/ebenen.ts
---

# Start der Webkarte

Seit `layers.json` und `api/ich` gleich zu Beginn losgehen, steht das erste
Objekt einer Ebene bei 150 ms Umlaufzeit 330 ms früher da, 1 864 statt
2 193 ms. Mit `?tree=`, wo auch `map.json` mit `trees.json` losgeht, sind es
530 ms, 1 536 statt 2 063 ms, und die erste Kachel kommt 180 ms früher. Ohne
Latenz ändert sich nichts. Den Rest bestimmen die Datei der Ebene,
`satz.json`, die Sprites und die Kartenschrift, in dieser Reihe.

## Aufbau

- **Welt und Server:** die grosse Welt mit ihren Ebenen, der Webserver des
  Plugins direkt, über den kleinen Node-Server des Frontends, der `/tiles/`
  und `/api/` samt Headern durchreicht.
- **Seite:** je Stand ein Build. Ohne `?tree=` öffnet sie den ersten Baum,
  iso 4:3 mit `cinematic`; mit `?tree=top-north-s` den Baum von oben.
- **Browser:** Chromium aus Playwright, headless, 1600 × 900, je Lauf ein
  frischer Kontext ohne Cache. Latenz über CDP 0 oder 150 ms je Anfrage,
  Bandbreite frei.
- **Werte:** ab dem Beginn der Navigation, aus einem `MutationObserver` in
  der Seite: die erste geladene Kachel, die erste Nadel und der erste Pfad
  einer Ebene. Dazu die Anfragen bis dahin aus Resource Timing.
- **Stände:** vorher die Basis des Zweigs (master), nachher `dbbc38a`.

## Ablauf

Am 24.10. unter `messung.lock`, ohne Spiel. Je Fall drei Läufe, die Stände
abwechselnd, 3 s Pause dazwischen. Skript `m7/start.mjs` des Frontends,
nicht im Repo.

## Ergebnis

Median aus drei Läufen, in ms ab der Navigation.

| Fall | erste Kachel vorher | nachher | erstes Objekt einer Ebene vorher | nachher |
|---|---|---|---|---|
| ohne `?tree=`, 0 ms | 100 | 102 | 137 | 132 |
| ohne `?tree=`, 150 ms | 935 | 924 | 2 193 | 1 864 |
| mit `?tree=`, 0 ms | 91 | 99 | 132 | 127 |
| mit `?tree=`, 150 ms | 918 | 737 | 2 063 | 1 536 |

Die Läufe eines Falls liegen bei 150 ms innerhalb von 50 ms.

**Die Reihe nachher**, ohne `?tree=`, 150 ms, ein Lauf:

| Anfragen | von – bis |
|---|---|
| `trees.json`, `layers.json`, `api/ich` | 368 – 548 ms |
| `map.json` | 549 – 720 ms |
| die Datei der Ebene der Städte | 764 – 1 046 ms |
| `satz.json` der Banner | 1 049 – 1 528 ms |
| die Sprites | 1 530 – 1 853 ms |
| die Kartenschrift | 1 686 – 1 854 ms |

## Schluss

- **`api/ich` liegt nicht mehr im Weg:** Es ist nach 533 ms fertig, die
  erste Ebene beginnt nach 764 ms. Ebenen ohne `player` früher zu laden,
  ohne auf `pruefe` zu warten, brächte darum nichts und unterbleibt.
- **Der Rest:** `satz.json` wartet auf die Datei der Ebene, die Sprites auf
  `satz.json`, und die Kartenschrift lädt erst, wenn ein Banner mit Namen
  sie braucht. Die Schrift ist der nächste Schritt.
