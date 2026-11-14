---
title: Grundschicht nur deckend
description: Bildabstände der Webkarte beim Ziehen und Zoomen, Grundschicht ohne Schwelle gegen einen Filter je Bild und gegen eine Leinwand, die einmal je Kachel schwellt; je drei Läufe auf der Testwelt, Software-Raster.
date: 2026-11-14
commits: [da0f853b, 48732feb]
code:
  - web/src/main.ts
---

# Grundschicht nur deckend

Ein Filter je Bild kostet unter Software-Raster in allen drei Paaren: je Phase
4–15 % weniger Bilder, beim Zoomen 7–16 statt 1–3 Bilder über 50 ms. Die Leinwand,
die einmal je Kachel schwellt, kostet nichts über die Streuung; in allen
drei Paaren liegt sie gleich oder besser als ohne Schwelle, aber „ohne“
streut in dieser Reihe stark. Das schliesst nur einen Mehraufwand unter
diesen Bedingungen aus und sagt nichts über eine echte Grafikkarte.

## Aufbau

- **Frage:** Was kostet es beim Zeichnen, wenn die Grundschicht nur Pixel
  mit Alpha 255 zeigt, siehe
  [0141](../entscheidungen/0141-grundschicht-nur-deckend.md)?
- **Reihe 1, Filter:** „mit“ ist `da0f853b`, ein `feComponentTransfer` mit
  `feFuncA` auf `.grund .leaflet-tile`. „ohne“ ist derselbe Stand ohne diese
  Regel in `style.css`.
- **Reihe 2, Leinwand:** „mit“ ist `48732feb`, die Grundschicht als `Grund`.
  „ohne“ ist derselbe Stand mit `new Kacheln(` statt `new Grund(`, also die
  Grundschicht als `img` wie vor 0141.
  In `48732feb` liest die Leinwand ihre Pixel einmal zurück und schwellt sie.
  Der endgültige Stand malt stattdessen zwölfmal mit `destination-in`, siehe
  0141. Das unterscheidet sich nur im einmaligen Schritt je Kachel; was je
  Bild gezeichnet wird, ist dasselbe.
- **Sonst wie** [Grundschicht beim Zeichnen](2026-11-04-grundschicht-zeichnen.md):
  die Testwelt, Baum iso 4:3 aus Südost mit `cinematic`, scale 24; ein
  kleiner Node-Server des Frontends liefert Build und Kacheln von der Platte;
  Chromium aus Playwright, headless, also Software-Raster, 1600 × 900,
  `devicePixelRatio` 1, je Lauf ein frischer Browser; `at=-296,64,400`,
  `zoom=-3`.
- **Nur iso:** Einen Baum von oben der Testwelt gab es lokal nicht. Die
  Schwelle wirkt je Kachel der Grundschicht, gleich welche Kamera.

## Ablauf

Unter der Sperre, ohne andere Last, abwechselnd ohne und mit, je drei Läufe,
Reihe 1 und Reihe 2 nacheinander am 2026-11-14. Je Lauf wie am 04.11.:

1. Laden, 3 s warten.
2. **Ziehen:** 3 × 500 px nach links, 3 × 500 px zurück, je 700 ms Pause.
3. **Hinein:** 3 Rasten, je 900 ms Pause.
4. **Hinaus:** 3 Rasten, ebenso.

Je Phase misst die Seite die Abstände zwischen zwei `requestAnimationFrame`
und die langen Aufgaben. Ein Lauf zählt nur ohne Seitenfehler und wenn die
Adresse nach jeder Phase die erwartete Stufe nennt (−3, 0, −3). Das trifft auf
alle zwölf zu. Die Zahlen sind die Ausgabe des Messskripts.

Dazu zählte das Skript `LayerTree.layerPainted` über das Chrome DevTools
Protocol, um zu sehen, ob eine Ebene je Bild neu gemalt wird. Headless meldet
die Domain in allen Läufen 0, auch ohne Schwelle; die Zahl sagt also nichts
und steht nicht in den Tabellen.

## Ergebnis

Bildabstände in ms, je Lauf 1 / 2 / 3. Das Raster ist 16,7 ms. Lange
Aufgaben über 50 ms: 0 in allen Phasen und Läufen beider Reihen.

### Reihe 1, Filter je Bild

| Phase | Stand | p95 | p99 | Maximum | Bilder über 50 ms | Bilder |
|---|---|---|---|---|---|---|
| Ziehen | ohne | 16,8 / 33,3 / 33,3 | 33,4 / 33,4 / 33,4 | 33,4 / 33,4 / 33,4 | 0 / 0 / 0 | 469 / 457 / 467 |
| Ziehen | Filter | 50,0 / 33,4 / 50,0 | 50,1 / 50,0 / 50,1 | 66,6 / 50,1 / 66,7 | 8 / 2 / 8 | 430 / 437 / 422 |
| Hinein | ohne | 33,4 / 49,9 / 33,4 | 50,0 / 50,0 / 50,0 | 66,6 / 50,1 / 50,1 | 2 / 1 / 1 | 227 / 227 / 228 |
| Hinein | Filter | 50,0 / 66,6 / 66,7 | 66,7 / 66,7 / 83,4 | 66,7 / 83,3 / 83,4 | 7 / 13 / 16 | 213 / 206 / 200 |
| Hinaus | ohne | 33,4 / 49,9 / 33,4 | 50,0 / 50,1 / 50,0 | 50,1 / 66,7 / 66,7 | 2 / 3 / 1 | 224 / 220 / 225 |
| Hinaus | Filter | 50,0 / 50,0 / 66,7 | 66,7 / 66,7 / 83,4 | 83,3 / 66,7 / 100,0 | 8 / 9 / 16 | 205 / 205 / 191 |

### Reihe 2, Leinwand einmal je Kachel

| Phase | Stand | p95 | p99 | Maximum | Bilder über 50 ms | Bilder |
|---|---|---|---|---|---|---|
| Ziehen | ohne | 33,3 / 16,8 / 33,4 | 33,4 / 33,4 / 50,0 | 33,4 / 33,4 / 50,1 | 0 / 0 / 2 | 460 / 465 / 430 |
| Ziehen | Leinwand | 16,7 / 16,7 / 16,7 | 16,8 / 16,8 / 16,8 | 16,8 / 16,8 / 16,8 | 0 / 0 / 0 | 470 / 471 / 471 |
| Hinein | ohne | 33,4 / 50,0 / 66,7 | 50,0 / 66,7 / 83,4 | 50,1 / 66,7 / 100,0 | 1 / 6 / 14 | 231 / 217 / 200 |
| Hinein | Leinwand | 33,3 / 33,4 / 33,3 | 33,4 / 50,0 / 33,4 | 33,4 / 50,0 / 49,9 | 0 / 0 / 0 | 243 / 235 / 242 |
| Hinaus | ohne | 33,4 / 50,0 / 50,1 | 50,1 / 50,0 / 66,7 | 66,6 / 66,7 / 66,7 | 5 / 2 / 11 | 228 / 215 / 203 |
| Hinaus | Leinwand | 33,3 / 33,3 / 33,3 | 49,9 / 33,4 / 33,4 | 50,1 / 50,1 / 33,4 | 2 / 1 / 0 | 244 / 236 / 243 |

## Schluss

- **Filter je Bild:** in jeder Phase und jedem Paar schlechter als ohne,
  über die Streuung hinaus. Verworfen in 0141.
- **Leinwand:** in jedem Paar gleich oder besser als ohne. „Ohne“ streut in
  Reihe 2 stark, beim Hineinzoomen 1 bis 14 Bilder über 50 ms; dass die
  Leinwand schneller zeichnet, folgt daraus nicht. Ein Mehraufwand zeigt
  sich nicht.
- **Grenzen:** drei Läufe je Stand, Software-Raster, eine kleine Welt, kein
  Netz, nur iso, nur Chromium. Ob die Grundschicht auf einer echten
  Grafikkarte als Ebene läuft, zeigt diese Messung nicht.
