---
title: Icons als Leinwand oder Bild
description: Versuch V1 und V2 zum Stresstest der Ebenen, gemessen und nicht übernommen. Die Icons der Nadeln und Banner als Leinwand ohne Beschleunigung, oder die Banner als <img> mit der Adresse ihres Sprites; was es bringt, warum es nicht reicht und wo der Code liegt.
date: 2026-10-24
issues: []
tag: versuch/icons-leinwand-img
code:
  - web/src/ebenen.ts
  - web/src/style.css
---

# Icons als Leinwand oder Bild

Mit 2000 Nadeln und Bannern ruckelt die Karte beim Ziehen, siehe
[Viele Nadeln und Banner](../messungen/2026-10-24-viele-nadeln-und-banner.md).
Der Versuch prüft zwei kleine Wege an den Icons. Keiner kommt unter die
Schwelle von 33 ms für p95; darum ist keiner übernommen. Die Zahlen stehen
in der Messung, „Versuch V1 und V2“.

## Was der Versuch tut

In `ortIcon` in [`web/src/ebenen.ts`](../../web/src/ebenen.ts), Tag
`versuch/icons-leinwand-img`, geschaltet mit `?versuch=v1`, `v2` oder
`v1v2`:

- **V1:** Die Leinwand des Icons mit `getContext('2d', {
  willReadFrequently: true })`, also im Speicher der CPU.
- **V2:** Ist die Quelle ein Bild mit Adresse, also ein Sprite oder
  `image` eines Banners, ein `<img>` mit derselben Adresse statt der
  Leinwand. Gross b·k/dpr CSS-Pixel, `image-rendering: pixelated` wie die
  Leinwand in [`web/src/style.css`](../../web/src/style.css). Alle Icons
  eines Sprites teilen so das dekodierte Bild.
- **Nadeln** bleiben in V2 Leinwand. Sie werden eingefärbt gezeichnet und
  haben keine Adresse, und die CSP der Karte erlaubt Bilder nur von
  `'self'`, ohne `blob:` und `data:`.
- **Die Messung** dazu: `web/tests/messung-viele-orte.spec.ts` mit
  `VERSUCH`, `LAGEN` für die Zahl der Ebenen über `LayerTree` und
  `LAEUFE`.

## Ergebnis

- **V1** wirkt nicht. Es bleiben 4 Ebenen, und `Commit` und `Layerize`
  bleiben gleich.
- **V2** halbiert `Commit` und senkt p95 beim Ziehen von 167 auf 117 ms. Die
  Leinwand der Icons sinkt von 66 auf 26 MiB. Der Pinch bleibt gleich.
- **Zusammen** nicht besser als V2 allein.

## Wann es sich lohnen könnte

- **V2** hilft beim Speicher auch neben V3, nur Marker im Ausschnitt. Bei
  dpr 3 kosten die Banner sonst 40 MiB Leinwand. Mit `img-src 'self' blob:`
  in `web/headers.json` liesse es sich auf die Nadeln ausdehnen. Das ist eine
  Änderung am Server des Renderers, der die Header per `include_str!`
  einbaut, und an jedem Proxy mit eigenen Headern.
- **Pixel auf Pixel** ist für V2 nicht geprüft. Vor einer Übernahme braucht
  es einen Test bei dpr 1, 1,25, 1,5 und 3.
