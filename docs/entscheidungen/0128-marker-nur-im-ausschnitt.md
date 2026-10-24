---
title: "0128: Marker nur im Ausschnitt"
description: Warum die Webkarte Nadeln und Banner nur im Fenster mit einem Rand von einem Viertel je Seite im DOM hält, ihre Icons erst beim ersten Sichtbarwerden in Stücken baut, Marker mit offener Tafel oder Fokus behält und was das für Tab heisst; dazu die verworfenen Wege Leinwand ohne Beschleunigung und ein Icon je Zoom.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
  - web/src/formen.ts
---

# 0128: Marker nur im Ausschnitt

## Anlass

Im Stresstest der Ebenen ruckelt die Karte mit 1000 Bannern und 1000
Nadeln, siehe
[Viele Nadeln und Banner](../messungen/2026-10-24-viele-nadeln-und-banner.md).
Leaflet hält jeden Marker im DOM, auch weit ausserhalb des Fensters. Die
Zeit geht in `Commit` und `Layerize` des Browsers, die mit den Elementen
wachsen. Plan des Frontends, freigegeben vom Reviewer am 24.10., als V3
nach den Versuchen V1 und V2.

## Entscheidung

- **Nur im Fenster mit Rand:** Ein Ort hat einen Marker im DOM, wenn er im
  Fenster liegt oder im Rand von einem Viertel des Fensters je Seite
  (`ORT_RAND` = 0,25). Der Rand trägt ein Ziehen, bevor neu gezählt wird.
- **Icons bei Bedarf:** `ladeEbene` rechnet die Lage aller Orte, samt
  Oberfläche. Icon und Marker baut `zeigeOrte` erst, wenn ein Ort zum
  ersten Mal in den Rahmen kommt. Ein gebauter Marker bleibt bis zum
  nächsten Laden der Ebene und wird nur aus- und eingehängt, höchstens
  `GRENZEN.nadeln` je Ebene.
- **In Stücken:** je 32 Marker zugleich, danach hängt `zeigeOrte` sie ein
  und gibt nach `STUECK_MS` (8 ms) den Hauptfaden frei. Ein neueres Zählen
  bricht ein älteres ab.
- **Wann gezählt wird:** bei `moveend`, gleich nach dem Laden einer Ebene,
  und beim Ziehen höchstens einmal je Frame, sobald das Fenster den inneren
  Rahmen verlässt, also den halben Rand.
- **Was bleibt:** Ein Marker mit offener Tafel bleibt hängen, sonst
  schlösse Leaflet sie. Ebenso einer mit dem Fokus.
- **Wer Marker sucht:** Keine andere Stelle der Karte sucht einen Marker
  per Name oder Index oder zählt sie; nur `weg` nimmt die ganze Gruppe.
  Kommt eine dazu, muss sie den Marker bei Bedarf bauen, über
  `orte.marker` und `orte.baue`.

## Folgen

- **Gemessen** in
  [Marker nur im Ausschnitt](../messungen/2026-10-24-marker-nur-im-ausschnitt.md):
  In einer weiten Ansicht mit 9 % der Orte im Fenster fällt p95 beim Ziehen
  bei dpr 3 und CPU 4× von 117 auf 33 ms, die langen Aufgaben fallen um
  94 %, das Laden um 67 %, die Leinwand der Icons im DOM von 66 auf
  12,5 MiB.
- **Hinauszoomen** kostet die längste Aufgabe 68 ms bei CPU 4×, trotz der
  Stücke. Ein Teil davon ist Leaflets eigenes Zoomen. So gelassen.
- **Fast alles sichtbar** bleibt teuer: Liegen fast alle Orte im Fenster,
  hilft das Ausdünnen nicht, p95 beim Ziehen bleibt 167 ms.
- **Tab** erreicht nur Nadeln und Banner im Fenster mit Rand. Ein Ort weit
  ausserhalb ist kein Ziel, bis die Karte dorthin geht. Die Reihe für Tab
  wird damit kürzer, nicht falsch.
- **Tests:** Ein ferner Ort hat kein Element, kommt mit dem Rand und beim
  Ziehen dazu, und Tafel und Fokus halten ihn. Tests, die Nadeln weit
  ausserhalb des Fensters massen, messen jetzt im Fenster.

## Verworfene Alternativen

- **Leinwand ohne Beschleunigung,** V1, `willReadFrequently`: wirkungslos,
  siehe [Icons als Leinwand oder Bild](../versuche/icons-leinwand-oder-bild.md).
- **`<img>` statt Leinwand,** V2: allein −30 %, nicht genug. Zusammen mit
  diesem Weg halbiert es p95 noch einmal; es kommt als eigener Schritt,
  nur für Banner.
