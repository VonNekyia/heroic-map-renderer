---
title: "0141: Die Grundschicht zeigt nur deckende Pixel"
description: Warum die Grundschicht der Webkarte über einen Filter nur Pixel zeigt, die voll decken, statt nach dem Laden der feinen Kacheln ausgeblendet zu werden; der Saum am Rand der Welt, was der Besucher beim Laden sieht, der Rückfall ohne Filter, und was verworfen ist.
status: gilt
date: 2026-11-13
issues: []
code:
  - web/index.html
  - web/src/style.css
---

# 0141: Die Grundschicht zeigt nur deckende Pixel

Die [Grundschicht](../frontend.md) zeigt nur noch Pixel mit Alpha 255. Ein
halb gedecktes Pixel ragte am Rand der Welt als gezackter Saum unter den
feinen Kacheln heraus. Die Regeln im Einzelnen stehen in
[Frontend](../frontend.md), „Grundschicht“.

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

`renderer/src/render/pyramid.rs` mittelt Alpha je 2 × 2 und rundet auf. Ein
Pixel der Grundschicht, drei Stufen gröber, deckt 8 × 8 Pixel der feinen
Kacheln und hat nur dann Alpha 255, wenn alle 64 darunter voll decken. Jedes
Pixel, das über den Rand der Welt reicht, ist also halb durchsichtig. Wo die
feine Kachel durchsichtig ist, scheint es durch, gemischt mit dem Grund der
Karte. Das ist der Saum, und darum hat er einen anderen Ton.

In 2D liegen die Kanten auf Chunkgrenzen. Fällt eine nicht auf das Raster
der groben Pixel, entsteht derselbe Saum als gerade Stufe.

## Entscheidung

- **Ein Filter auf jeder Kachel der Grundschicht:** `feComponentTransfer`
  mit `feFuncA type="linear" slope="255" intercept="-254"`. Alpha 255 bleibt
  255, alles darunter wird 0.
- **`color-interpolation-filters="sRGB"`:** Sonst rechnet der Browser die
  Farben über `linearRGB` hin und zurück, und mitten in der Welt weichen sie
  um eine Stufe ab.
- **Wo:** Der Filter steht als Element in `web/index.html`, die Regel
  `.grund .leaflet-tile { filter: url(#nur-deckend) }` in
  `web/src/style.css`. Keine Ereignisse, kein Zeitgeber, kein Eingriff in
  Leaflet. Die Content-Security-Policy bleibt, wie sie ist: Der Filter ist
  kein `<style>`.

## Was der Besucher sieht

| Ort | beim Laden | geladen |
|---|---|---|
| mitten in der Welt | die Grundschicht, unverändert | die feinen Kacheln, wie bisher |
| am Rand der Welt | die Grundschicht endet bis zu einem groben Pixel innerhalb der Kante, dort der Grund der Karte | eine gerade Kante wie ohne Grundschicht |
| Inseln unter 8 × 8 px in Welten mit Leere | fehlen in der Grundschicht | da, aus den feinen Kacheln |

Wo die feinen Kacheln selbst halb durchsichtig sind, etwa Glas vor dem
Himmel, scheint nur noch ein voll deckendes Pixel der Grundschicht durch.
Das liegt näher am Bild ohne Grundschicht als bisher.

## Rückfall

Kann ein Browser den Filter nicht, gilt die Regel nicht, und die Grundschicht
zeigt alles wie vor 0141, nie eine leere Schicht:

- Ein Wert von `filter`, den der Browser nicht versteht, verwirft er beim
  Lesen der Regel, CSS Syntax.
- Das Element mit `id="nur-deckend"` steht fest in `index.html`, vor dem
  Skript. Ein Verweis ins Leere kann also nur durch einen Fehler im Code
  entstehen, und den fängt der Test.

## Verworfene Alternativen

- **Die Grundschicht ausblenden, sobald die feinen Kacheln geladen sind,**
  über `load` und `loading`:
  - Leaflet feuert `load`, bevor `fadeAnimation` die Kacheln ganz
    eingeblendet hat. Danach wäre es mitten in der Welt unter den halb
    eingeblendeten Kacheln kurz dunkel.
  - Erscheint die Grundschicht erst nach einer Verzögerung wieder, ist es
    beim Ziehen für diese Zeit schwarz, obwohl die grobe Kachel meist schon
    geladen ist.
  - Ohne Verzögerung blitzt der Saum bei jedem Schritt am Rand auf.
- **Randpixel der Pyramide im Renderer wegnehmen:** Das hiesse einen neuen
  Zeichenstand, und in Welten mit Leere fielen kleine Inseln aus allen
  groben Stufen, nicht nur aus der Grundschicht.

## Folgen

- Je Kachel der Grundschicht ein Filter beim Zeichnen.
- **Tests:** `web/tests/saum.spec.ts` baut eine Welt in einer Farbe, deren
  Rand nicht auf dem Raster der groben Pixel liegt, schräg wie im iso und
  gerade wie in 2D, mit Alpha wie `pyramid.rs`.
  - Nach dem Laden ist der Ausschnitt mit Grundschicht pixelgleich dem ohne.
  - Beim Laden zeigt er nur die Farbe der Welt, unverändert, und den Grund
    der Karte, keine Mischung.
  - Gegenproben: Filter weg, `intercept` 0, ohne `sRGB`.
- **Nicht belegt:** ob die Schicht mit Filter auf einer echten Grafikkarte
  in der Zoom-Animation als Ebene läuft oder je Bild neu gezeichnet wird. Das
  zeigt erst der Spiegel.
