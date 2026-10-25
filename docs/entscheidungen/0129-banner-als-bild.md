---
title: "0129: Banner als Bild"
description: Warum die Webkarte ein Banner als <img> mit der Adresse seines Sprites zeigt statt in einer eigenen Leinwand, warum Nadeln und Köpfe Leinwand bleiben und wie der Test auf dem Schirm prüft, dass kein Pixel verschmiert; ergänzt 0128.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
  - web/src/style.css
  - web/tests/ebenen-welt.ts
---

# 0129: Banner als Bild

## Anlass

Im Versuch V2 zum Stresstest der Ebenen senkte ein `<img>` statt der
Leinwand je Banner p95 beim Ziehen um 30 %, siehe
[Icons als Leinwand oder Bild](../versuche/icons-leinwand-oder-bild.md).
Mit [0128](0128-marker-nur-im-ausschnitt.md) zusammen halbiert es p95 in
der weiten Ansicht noch einmal, von 33 auf 17 ms, siehe
[Marker nur im Ausschnitt](../messungen/2026-10-24-marker-nur-im-ausschnitt.md).
Vom Reviewer am 24.10. als eigener Schritt nach 0128 bestimmt: nur Banner,
ohne Änderung an der CSP.

## Entscheidung

- **Banner als `<img>`:** Ist die Quelle eines Icons ein Bild mit Adresse,
  das Sprite aus dem Satz oder `image`, zeigt `ortIcon` ein `<img>` mit
  derselben Adresse. Grösse wie bisher die Leinwand, `b · k /
  devicePixelRatio` CSS-Pixel mit `k = max(1, round(devicePixelRatio))`,
  dazu `image-rendering: pixelated`.
- **Nadeln und Köpfe bleiben Leinwand:** Die Nadel färbt die Karte ein, der
  Kopf kommt als `ImageBitmap`; beide haben keine Adresse. Ein `blob:` oder
  `data:` verbietet die CSP der Karte, `default-src 'self'`.
- **Geprüft auf dem Schirm,** nicht in der Leinwand:
  `pixelAufPixel` in `web/tests/ebenen-welt.ts` liest einen Screenshot in
  Pixeln des Geräts. Jedes Pixel ganz im Bild muss genau die Farbe der
  Quelle haben, die Nächster-Nachbar trifft, bei dpr 1, 1,25, 1,5, 2 und 3.

## Folgen

- **Speicher:** Alle Banner mit demselben Bild teilen ein dekodiertes Bild,
  statt je eine Leinwand von b·k × h·k zu halten. In der weiten Ansicht
  sinkt die Leinwand im DOM von 12,5 auf 4,7 MiB, in der vollen von 39,6 auf
  15,4 MiB.
- **Neuer Zeichenweg bei anderem dpr:** Ändert sich `devicePixelRatio`,
  baut die Karte die Icons neu wie bisher; das `<img>` bekommt die neue
  Grösse.

## Verworfene Alternativen

- **Auch Nadeln als Bild:** brauchte `img-src 'self' blob:` in
  `web/headers.json` und damit ein Release des Renderers. Der Reviewer
  entscheidet darüber erst mit Zahlen, die zeigen, dass es sich lohnt.
- **Leinwand ohne Beschleunigung,** V1: wirkungslos, siehe den Versuch.
