---
title: "0141: Die Grundschicht zeigt nur deckende Pixel"
description: Warum die Grundschicht der Webkarte ihre Kacheln einmal nach dem Laden auf eine Leinwand malt und dort nur Pixel stehen lässt, die voll decken, statt nach dem Laden der feinen Kacheln ausgeblendet zu werden oder einen Filter je Bild zu rechnen; der Saum am Rand der Welt, was der Besucher beim Laden sieht, der Rückfall, und was verworfen ist.
status: gilt
date: 2026-11-14
issues: []
code:
  - web/src/main.ts
---

# 0141: Die Grundschicht zeigt nur deckende Pixel

Die [Grundschicht](../frontend.md) zeigt nur noch Pixel mit Alpha 255: Jede
ihrer Kacheln kommt einmal nach dem Laden auf eine Leinwand, und dort wird
jedes Alpha unter 255 zu 0. Ein halb gedecktes Pixel ragte am Rand der Welt
als gezackter Saum unter den feinen Kacheln heraus. Die Regeln im Einzelnen
stehen in [Frontend](../frontend.md), „Grundschicht“.

## Anlass

Der User am 13.11.2026 zu zwei Bildschirmfotos aus dem Baum iso mit
Cinematic: „was sind das für komische chunkfehler aussen?“ Zu sehen war
entlang der schrägen Kante der Welt ein Saum in etwas anderem Blau, mit
regelmässigen Stufen nach aussen.

Der Reviewer hat es an der öffentlichen Karte nachgestellt, drei Stufen
unter der feinsten:

- Mit beiden Schichten: Zacken, je Stufe 8 px.
- `.grund` ausgeblendet: eine gerade Kante.
- Nur `.grund`: Die Kante ihrer einen Kachel ist die Treppe.

## Ursache

`renderer/src/render/pyramid.rs` mittelt Alpha je 2 × 2 und rundet auf,
`alpha.div_ceil(4)`. Ein Pixel der Grundschicht, drei Stufen gröber, deckt
8 × 8 Pixel der feinen Kacheln. Wo die feinste Stufe nur Alpha 0 oder 255
hat, hat es nur dann Alpha 255, wenn alle 64 darunter voll decken. Mit Werten
dazwischen genügt fast voll: Je 2 × 2 ergibt schon eine Summe ab 1017 wieder
255, etwa 255, 255, 255 und 252. Am Rand der Welt grenzen Alpha 255 und 0
aneinander, jedes Pixel, das über ihn reicht, ist also halb durchsichtig. Wo die
feine Kachel durchsichtig ist, scheint es durch, gemischt mit dem Grund der
Karte. Das ist der Saum, und darum hat er einen anderen Ton.

In 2D liegen die Kanten auf Chunkgrenzen. Fällt eine nicht auf das Raster
der groben Pixel, entsteht derselbe Saum als gerade Stufe.

## Entscheidung

- **Die Grundschicht zeigt Leinwände,** `Grund` in `web/src/main.ts`, eine
  Unterklasse von `Kacheln`. `createTile` legt eine Leinwand an und lädt das
  Bild dahinter wie `L.TileLayer`; die Leinwand trägt es als `bild`.
- **Einmal nach dem Laden** malt `nurDeckend` das Bild auf die Leinwand, in
  seiner eigenen Grösse, meist 256 × 256, und setzt jedes Alpha unter 255
  auf 0. Erst dann meldet die Kachel sich bei Leaflet fertig. Beim Zeichnen
  kostet es danach nichts mehr; vergrössert wird die Leinwand wie ein Bild,
  mit `image-rendering: pixelated` über `.leaflet-tile`.
- **Abbrechen bleibt, wie es ist:** `Kacheln` nimmt dem Bild hinter der
  Leinwand das `src` und beendet so die Anfrage, `quelle` in `main.ts`.
- Die feinen Kacheln bleiben `img`.

## Was der Besucher sieht

| Ort | beim Laden | geladen |
|---|---|---|
| mitten in der Welt | die Grundschicht, unverändert | die feinen Kacheln, wie bisher |
| am Rand der Welt | die Grundschicht endet bis zu einem groben Pixel innerhalb der Kante, dort der Grund der Karte | eine gerade Kante wie ohne Grundschicht |
| Inseln unter 8 × 8 px in Welten mit Leere | fehlen in der Grundschicht | da, aus den feinen Kacheln |

Wo die feinen Kacheln selbst halb durchsichtig sind, etwa Glas vor dem
Himmel, scheint nur noch ein voll deckendes Pixel der Grundschicht durch.
Das liegt näher am Bild ohne Grundschicht als bisher, ist aber nicht
pixelgleich: Wegen des Aufrundens kann dort ein grobes Pixel Alpha 255
haben.

## Rückfall

Wirft das Lesen der Pixel, bleibt das Bild auf der Leinwand ungeschwellt
stehen, nie leer: `nurDeckend` malt es zuerst und liest erst danach. Werfen
kann `getImageData` nur bei einem Bild fremder Herkunft. Unter der
Content-Security-Policy des Builds, `default-src 'self'`, lädt die Seite keine
Bilder von anderswo, der Fall tritt dort also nicht ein und hat keinen Test.

## Verworfene Alternativen

- **Ein Filter je Bild,** `feComponentTransfer` mit `feFuncA` auf
  `.grund .leaflet-tile`: dieselbe Wirkung, aber der Browser rechnet ihn bei
  jedem Bild. Unter Software-Raster kamen beim Ziehen und Zoomen in allen
  drei Paaren 4–15 % weniger Bilder, und die Bilder über 50 ms stiegen beim
  Zoomen von 1–3 auf 7–16, siehe
  [Grundschicht nur deckend](../messungen/2026-11-14-grundschicht-nur-deckend.md).
  Das Zoomen war gerade erst die Beschwerde des Users.
- **Die Grundschicht ausblenden, sobald die feinen Kacheln geladen sind,**
  über `load` und `loading`:
  - Leaflet feuert `load`, bevor `fadeAnimation` die Kacheln ganz
    eingeblendet hat. Danach wäre es mitten in der Welt unter den halb
    eingeblendeten Kacheln kurz dunkel.
  - Erscheint die Grundschicht erst nach einer Verzögerung wieder, ist es
    beim Ziehen für diese Zeit schwarz, obwohl die grobe Kachel meist schon
    geladen ist.
  - Ohne Verzögerung blitzt der Saum bei jedem Schritt am Rand auf.
- **Den Filter nur in Ruhe:** Dann blitzte der Saum genau beim Ziehen und
  Zoomen auf.
- **Randpixel der Pyramide im Renderer wegnehmen:** Das hiesse einen neuen
  Zeichenstand, und in Welten mit Leere fielen kleine Inseln aus allen
  groben Stufen, nicht nur aus der Grundschicht.

## Folgen

- Je Kachel der Grundschicht einmal Malen, Lesen und Schreiben von meist
  256 × 256 Pixeln und eine Leinwand von 256 KiB.
- **Tests:** `web/tests/saum.spec.ts` baut eine Welt in einer Farbe, deren
  Rand nicht auf dem Raster der groben Pixel liegt, schräg wie im iso und
  gerade wie in 2D, mit Alpha wie `pyramid.rs`.
  - Nach dem Laden ist der Ausschnitt mit Grundschicht pixelgleich dem ohne.
  - Beim Laden zeigt er nur die Farbe der Welt, unverändert, und den Grund
    der Karte, keine Mischung.
  - `smoke.spec.ts`: Zoomen bricht auch laufende Kacheln der Grundschicht ab.
  - Gegenproben: ohne Schwelle, Schwelle bei 0, Leinwand nicht gezeigt,
    Abbruch an der Leinwand statt am Bild.
- **Nicht belegt:**
  - Wie die Leinwände auf einer echten Grafikkarte beim Zoomen laufen. Das
    zeigt erst der Spiegel.
  - Firefox und WebKit: Alle Projekte von Playwright laufen in Chromium.
