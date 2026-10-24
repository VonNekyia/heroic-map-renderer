---
title: Rand der feinen Kacheln
description: Versuch zu D2 aus dem Plan zum Kachelstrom, gemessen und nicht übernommen. Die feinen Kacheln laden eine Kachel über das Fenster hinaus; bei 50 Mbit/s fehlt beim Ziehen fast nichts mehr, bei 10 Mbit/s mehr als ohne, für +60 % Bytes. Wann es sich lohnen könnte und wo der Code liegt.
date: 2026-10-24
issues: []
tag: versuch/rand-feiner
code:
  - web/src/main.ts
---

# Rand der feinen Kacheln

Unter der Grundschicht, [Frontend](../frontend.md), „Grundschicht“, fehlen beim Ziehen noch 1–6 % des
Fensters, bis die feinen Kacheln da sind. Der Versuch lädt die feinen eine
Kachel über das Fenster hinaus. Bei 50 Mbit/s fehlt dann fast nichts mehr,
bei 10 Mbit/s mehr als ohne, und die Reihe kostet +60 % Bytes. Der Reviewer
verwarf ihn am 24.10.; der Ring der Grundschicht ist aus demselben Grund
verworfen.

## Was der Versuch tut

In [`web/src/main.ts`](../../web/src/main.ts), Tag `versuch/rand-feiner`:

- **`rand` an `Kacheln`:** `_getTiledPixelBounds` legt `rand` Kacheln um
  die Grenzen, die Leaflet für das Fenster rechnet; die feinen Kacheln
  bekommen `rand` 1.
- **`updateInterval` 100 ms** statt 200: Beim Ziehen fragt Leaflet öfter
  nach, welche Kacheln fehlen.
- Die Reihenfolge bleibt die von Leaflet, die Mitte zuerst; der Rand lädt
  also mit derselben Dringlichkeit wie alles andere, das nicht in der Mitte
  liegt.

## Gemessen

In [Rand der feinen Kacheln](../messungen/2026-10-24-rand-feiner-kacheln.md),
gegen die Grundschicht:

| | schwarz beim Ziehen, 50 / 10 Mbit/s | Bytes der Reihe, 50 / 10 Mbit/s |
|---|---|---|
| Grundschicht | 3,6 / 6,0 % | 5,4 / 4,1 MB |
| Grundschicht + Rand | 0,6 / 8,9 % | 8,7 / 6,5 MB |

## Warum nicht übernommen

- **Bei schmaler Leitung schlechter:** Die Kacheln des Rands laden neben den
  sichtbaren und nehmen ihnen die Bandbreite; was man sieht, ist später voll.
- **Teuer:** +60 % Bytes und +80 % Anfragen, jedes Mal, auch für Ränder, zu
  denen niemand zieht.

## Wann es sich lohnen könnte

Mit einer eigenen Warteschlange, die das Sichtbare vor dem Rand lädt und
eine Kachel des Rands aus der Schlange nimmt, sobald sie nicht mehr gebraucht
wird, nähme der Rand dem Sichtbaren nichts. Nach dieser Warteschlange (D3)
wird er noch einmal gemessen.
