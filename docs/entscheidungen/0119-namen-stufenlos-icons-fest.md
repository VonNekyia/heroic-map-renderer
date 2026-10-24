---
title: "0119: Namen stufenlos, Icons fest"
description: Warum Nadeln, Köpfe und Banner auf der Webkarte wieder auf jeder Stufe gleich gross sind und unter 16 Blöcken je Pixel nur ihr Name kleiner wird, stufenlos mit dem Massstab und mit dem Zoom animiert, und zwischen 8 und 5 Pixel Schrift weich ausblendet; löst 0116 ab.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
  - web/src/style.css
---

# 0119: Namen stufenlos, Icons fest

## Anlass

Der User am 24.10. über die Webkarte auf der Produktion, nach
[0116](0116-nadeln-und-banner-unter-16-bloecken-je-pixel.md): Die Banner
sehen weit draussen komisch aus und nicht weich. Beim Hinauszoomen werden
sie kleiner, statt ihre Grösse zu behalten. Er wünscht klare, gut
aussehende Übergänge. Plan des Frontends, freigegeben vom Reviewer am
24.10.

## Entscheidung

- **Icons fest:** Nadeln, Köpfe und Banner haben auf jeder Stufe ihre volle
  Grösse, wie unter
  [0097](0097-banner-feste-groesse-tafel-beim-zeigen.md), „Entscheidung“.
  Eine Regel für alle drei.
- **Nur der Name wird kleiner, stufenlos:** Faktor
  `t = min(1, Breite eines Blocks / (1/16 px))`. Bis 16 Blöcke je CSS-Pixel
  ist er also 1, darunter folgt er dem Massstab. Die Grenze und die Breite
  eines Blocks (`blockBreite`) sind die aus 0116.
- **Weich ausblenden:** Bei 8 Pixel Schrift und mehr deckt der Name ganz,
  bei 5 Pixel und weniger nicht mehr, dazwischen linear. Ohne Deckung ist er
  kein Ziel für den Zeiger.
- **Mit dem Zoom animiert:** Grösse und Deckung gehen in 250 ms an ihr Ziel,
  mit der Kurve, mit der Leaflet zoomt. Das Ziel gilt ab dem Start des Zooms.
  Sichtbar bleibt der Name, bis er ausgeblendet ist.
- **Gerade und im Bogen** werden gleich behandelt: der gerade Name um seine
  Oberkante, der Bogen um den Fuss.
- **Der Mod** hat weiter die feste Grösse aus 0097.

## Verworfene Alternativen

- **Die Stufen aus 0116:** Die Halbierung springt, und die Banner werden
  kleiner. Das ist der Befund des Users.
- **Ein neues Icon je Zoom:** Damit kostete jeder Zoom Leinwände und
  Elemente. Hier ändern sich nur zwei CSS-Variablen am Container.
- **Namen ausblenden, die sich überlappen:** aus demselben Grund verworfen
  wie in 0116.

## Folgen

- Löst 0116 ab. 0097 gilt auf der Webkarte wieder ganz, für den Namen
  ergänzt um diese Regel.
- Beim Zoomen baut die Karte keine Icons mehr neu; `groesseNachZoom` ist weg.
- Mit gebrochenem Zoom, falls er kommt, folgt der Name ohne weitere Änderung.
