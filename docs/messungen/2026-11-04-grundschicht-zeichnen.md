---
title: Grundschicht beim Zeichnen
description: Bildabstände der Webkarte beim Ziehen und Zoomen, Grundschicht mit `scale(8)` gegen Kacheln von 2048 px, je drei Läufe auf der Testwelt, Software-Raster.
date: 2026-11-04
commits: [cdb73293, 0658daa0]
code:
  - web/src/main.ts
---

# Grundschicht beim Zeichnen

Kacheln von 2048 px statt `scale(8)` zeigen in je drei Läufen keinen
Unterschied über die Streuung: Beim Ziehen liegen p95 und p99 auf beiden
Seiten bei 33,4 ms, beim Hineinzoomen sind es 8–12 gegen 11–15 Bilder über
50 ms, beim Hinauszoomen 8–11 gegen 6–8. Die Bereiche überschneiden sich.
Das ist kein Beleg für Gleichheit und sagt nichts über eine echte
Grafikkarte, ein Telefon oder das Netz.

## Aufbau

- **Frage:** Was kostet die Grundschicht beim Zeichnen, seit sie Leaflets
  `zoomOffset` und eine Kachelgrösse von 2048 px nimmt, statt die Stufe in
  `_clampZoom` abzuziehen und 256 px mit `scale(8)` zu vergrössern?
- **Stände:** `cdb73293` (vor heroic-map-renderer#391) und `0658daa0`
  (mit #391). Unter `web/src` unterscheidet sich nur `main.ts`. Der Stand
  `0658daa0` ist aus `e530c3f0` gebaut, `web/` ist in beiden gleich.
- **Welt:** die Testwelt, Baum iso 4:3 aus Südost mit `cinematic`, scale 24,
  Stufen 0–7, 4414 Kacheln. Eine andere Welt als in
  [Grundschicht](2026-10-24-grundschicht.md); verglichen wird nur alt gegen
  neu.
- **Server:** ein kleiner Node-Server des Frontends liefert Build und Kacheln
  von der Platte, mit `no-cache`, ETag und 304 wie der Webserver. Ohne Netz
  und ohne Drossel, die Ladezeit der Kacheln spielt also kaum mit.
- **Browser:** Chromium aus Playwright, headless, also Software-Raster,
  1600 × 900, `devicePixelRatio` 1, normale Priorität, je Lauf ein frischer
  Browser.
- **Ansicht:** `at=-296,64,400`, `zoom=-3`, also feine Kacheln auf Stufe 4 und
  die Grundschicht auf Stufe 1.

## Ablauf

Unter der Sperre, ohne andere Last, abwechselnd alt und neu, je drei Läufe:

1. Laden, 3 s warten.
2. **Ziehen:** 3 × 500 px nach links, 3 × 500 px zurück, je 700 ms Pause.
3. **Hinein:** 3 Rasten, je eine Stufe, je 900 ms Pause.
4. **Hinaus:** 3 Rasten, ebenso.

Je Phase misst die Seite die Abstände zwischen zwei `requestAnimationFrame`
und die langen Aufgaben. Alte Stände hängen sich bei Sprüngen ab zwei Stufen
auf, darum nur einzelne Stufen. Ein Lauf zählt nur ohne Seitenfehler und wenn
die Adresse nach jeder Phase die erwartete Stufe nennt (−3, 0, −3). Das
trifft auf alle sechs zu. Die Zahlen sind die Ausgabe dieses Skripts vom
2026-11-04.

## Ergebnis

Bildabstände in ms, je Lauf 1 / 2 / 3. Das Raster ist 16,7 ms, feiner
unterscheidet die Messung nicht. Lange Aufgaben über 50 ms: 0 in allen
Phasen und Läufen.

| Phase | Stand | Median | p95 | p99 | Maximum | Bilder über 50 ms | Bilder |
|---|---|---|---|---|---|---|---|
| Ziehen | alt | 16,7 / 16,7 / 16,7 | 33,4 / 33,4 / 33,4 | 33,4 / 33,4 / 33,4 | 50,0 / 50,0 / 33,4 | 0 / 0 / 0 | 438 / 431 / 431 |
| Ziehen | neu | 16,7 / 16,7 / 16,7 | 33,4 / 33,4 / 33,4 | 33,4 / 33,4 / 33,4 | 50,0 / 50,0 / 50,0 | 0 / 0 / 0 | 438 / 427 / 432 |
| Hinein | alt | 16,7 / 16,7 / 16,7 | 50,1 / 50,0 / 50,0 | 66,7 / 66,7 / 66,7 | 83,3 / 66,8 / 116,8 | 12 / 9 / 8 | 213 / 210 / 210 |
| Hinein | neu | 16,7 / 16,7 / 16,7 | 66,7 / 50,1 / 50,1 | 116,6 / 83,4 / 66,7 | 116,6 / 116,6 / 66,8 | 15 / 11 / 11 | 195 / 203 / 211 |
| Hinaus | alt | 16,7 / 16,7 / 16,7 | 50,0 / 50,1 / 50,0 | 66,6 / 66,7 / 66,7 | 66,8 / 83,4 / 83,3 | 8 / 11 / 10 | 206 / 202 / 207 |
| Hinaus | neu | 16,7 / 16,7 / 16,7 | 50,0 / 50,0 / 50,0 | 66,7 / 66,7 / 66,7 | 66,8 / 66,8 / 66,7 | 6 / 8 / 8 | 207 / 208 / 210 |

## Schluss

- **Ziehen:** gleich, bis auf ein Maximum von 33,4 ms in einem Lauf von alt.
- **Hinein:** neu liegt leicht höher, die Bereiche überschneiden sich. Die
  höchsten Werte von neu liegen in seinem ersten Lauf, bei drei Läufen lässt
  sich daraus nichts schliessen.
- **Hinaus:** neu liegt leicht tiefer, ebenfalls überlappend.
- **Grenzen:** drei Läufe je Seite, Software-Raster, eine kleine Welt, kein
  Netz. Das Ergebnis schliesst nur einen groben Mehraufwand beim Zeichnen
  unter diesen Bedingungen aus.
