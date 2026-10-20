---
title: "0111: Geometrie der Ebenen nach Bildschirmfehler"
description: Warum die Webkarte Ränder, Linien, Kreise und Wände der Ebenen je Ansicht so fein rechnet, dass ihr Fehler auf dem Schirm unter ε = 0,5 px bleibt, nur im Ausschnitt und neu nach Zoom und Verschieben, statt einmal je version in Blöcken; warum die Wand Bänder ab 8 px hat (Wahl des Users am 20.10.); was verworfen ist, darunter eine feste Grenze an Punkten, der schwebende Vorhang und Canvas; und was für die Flächen folgt.
status: gilt
date: 2026-10-20
issues: [320]
code:
  - web/src/gelaende.ts
  - web/src/formen.ts
  - web/src/ebenen.ts
---

# 0111: Geometrie der Ebenen nach Bildschirmfehler

## Anlass

Auf einem Server lagte die Karte, sobald eine Ebene grosser Kreise an war.
Gemessen am 19.10. mit einer Kopie der Ebenen im Test, siehe
[Ebenen im weiten Blick](../messungen/2026-10-19-ebenen-im-weiten-blick.md):

| weit, Fenster 1278 × 1214 | ohne Kreise | mit Kreisen |
|---|---|---|
| Punkte in SVG-Pfaden | 616 082 | 13 078 862 |
| davon Wand | 521 928 | 11 576 232 |
| JS-Heap nach Bereinigung | 86 MB | 1 658 MB |
| lange Aufgaben je dreimal Verschieben | 152 ms | 2 738 ms |

Die Formen wurden einmal je `version` gerechnet, in Blöcken: an jeder Zelle
abgetastet, die Wand mit 12 Bändern je Probe, das Netz je Feld. Die Zahl
der Punkte wuchs mit der Grösse der Welt und dem Radius in Blöcken, nicht
mit dem, was man sieht. Ein Zwischenschritt (#310) deckelte Kreise und
Proben; der User wünscht eine Lösung, die für jede Welt und jede Ebene
trägt, keine feste Zahl (20.10.).

## Entscheidung

**Regel:** Ränder, Linien, Kreise und Wände werden so fein gezeichnet, dass
ihr Fehler auf dem Schirm unter ε bleibt, und nur, was im Ausschnitt
liegen kann. Neu gerechnet wird nach `zoomend` und `moveend`, 150 ms nach
der letzten Bewegung; solange der Zoom gleich ist und das Fenster im
gerechneten Ausschnitt liegt, bleibt das Gerechnete. Die Regeln im
Einzelnen stehen in [Frontend](../frontend.md), „Ebenen“.

- **ε = 0,5 Pixel des Schirms.** Darunter glättet die Kantenglättung den
  Unterschied weg; mit 1 px wären Kreise nah als Vieleck zu sehen.
- **Ausschnitt:** das Fenster plus ein halbes Fenster je Seite. Eine Form,
  die bei keiner Höhe des Geländes darin liegen kann, gibt keinen Punkt;
  eine Strecke ausserhalb nur ihren Anfang. Die Höhen lädt die Karte nur für
  die Regionen, die im Ausschnitt liegen können.
- **Ränder und Linien:** im Ausschnitt an den Zellen abgetastet, geglättet
  und auf Verdeckung geprüft wie bisher, danach auf dem Schirm vereinfacht
  (Douglas-Peucker mit ε). Jeder Wechsel von verdeckt oder gestrichelt
  bleibt als fester Punkt. So weicht der Zug höchstens ε von dem ab, was das
  Abtasten an den Zellen gäbe.
- **Kreise:** so viele Seiten, dass die Pfeilhöhe jeder Seite auf dem Schirm
  unter ε bleibt, `n = ⌈π / acos(1 − ε / r)⌉` mit r dem Radius auf dem
  Schirm, mindestens 12.
- **Wand:** je Band mindestens 8 px hoch, höchstens 12 Bänder, unter 1 px
  keine. Der User hat das am 20.10. an Bildern der Testwelt gewählt. Mit der
  Höhe von 6 Blöcken aus 0104 bei scale 16 in 2:1:

  | Zoom ab der feinsten Stufe | Höhe der Wand | Bänder |
  |---|---|---|
  | 1 | 96 px | 12 |
  | 0 | 48 px | 6 |
  | −1 | 24 px | 3 |
  | −2 | 12 px | 1 |
  | −6 | 0,75 px | keine |

- **Flächen,** im iso nur Ziel für Name und Tafel, nach derselben Regel
  (zweiter Schritt, 20.10., Reviewer): das Netz je Ansicht, nur im
  Ausschnitt, in Feldern von mindestens 16 px auf dem Schirm, die Zelle so
  oft verdoppelt, wie es dafür braucht. Verdeckt nach der Oberfläche in den
  Mitten der Felder; jede Kante an den Zellen abgetastet und auf ε
  vereinfacht, die Ecken der Stücke fest, sonst liessen zwei Felder an
  ihrer gemeinsamen Kante einen Spalt (gemessen bis 1,55 px). Der Rand des
  Ziels liegt höchstens ε neben dem der Fläche; nur die Grenze zum
  Verdeckten folgt den Feldern, auf dem Schirm also bis 16 px grob. Vorher
  war sie eine Zelle, bei scale 16 auf Zoom 0 rund 36 px. Die Ziele bleiben
  je `version`; je Ansicht wechseln nur ihre Ringe, so bleiben Tooltip,
  Tafel und Fokus.

  16 px hat der Reviewer am 20.10. gewählt, nach dieser Messung, siehe die
  Seite [Flächen nach 0111](../messungen/2026-10-20-flaechen-nach-0111.md),
  Punkte im DOM:

  | | weit | weit, mit Kreisen | nah | nah, mit Kreisen |
  |---|---|---|---|---|
  | nur Schritt A | 94 141 | 99 401 | 1 178 | 2 213 |
  | Felder ab 4 px | 1 139 | 2 919 | 2 394 | 5 332 |
  | Felder ab 16 px | 781 | 2 561 | 670 | 2 168 |

  Gerechnet nah mit Kreisen 71 867 gegen 25 167 Punkte, JS-Heap 13,7 gegen
  6,8 MB. Nah mehr als nach A, weil die Karte jetzt nur die Regionen im
  Ausschnitt lädt und so die feinere Zelle von `ground` nimmt.
- **Renderer:** SVG bleibt.

## Verworfene Alternativen

- **Wie heute, 12 Bänder auf jedem Zoom:** 24 Punkte je Probe des Rands
  auch dort, wo die Wand nur wenige Pixel hoch ist; das war die Masse der
  Punkte.
- **Bänder ab 4 px:** auf Zoom 0 gleich wie heute, weiter weg halb so viele
  Punkte wie mit 12. Empfohlen vom Frontend; der User nahm 8 px und die
  leichten Stufen im Verlauf nah und mittel in Kauf.
- **Der schwebende Vorhang,** Idee des Users: eine Wand fester Höhe an den
  rohen Ecken, ohne Abtasten. Die Webkarte hat keine Tiefe; damit nichts
  davor liegt, muss er über dem höchsten Boden am Rand stehen. Im Tal und
  am Hang schwebt er so sichtbar vom Rand weg, am Hang der Bilder bis rund
  60 Blöcke.
- **Eine feste Grenze an Punkten** je Ansicht, erst 50 000, dann 3 000: hält
  nur für die gemessene Welt.
- **Gröbere Proben in Blöcken,** gedeckelt je Ring (#310): senkt die Kreise,
  wächst aber weiter mit der Welt.
- **Abtasten in festen Pixeln mit Teilen nach der Mitte:** gemessen über
  ε, auf Zoom −4 0,55 px; die Mitte einer Strecke zeigt nicht jede Kuppe.
- **Vereinfachen vor dem Glätten:** Der Median sah die Proben dann zu
  dünn, und ein Haus am Weg hob die Linie wieder.
- **Ring plus `pick` für die Flächen** (Vorschlag von Frontend und
  Reviewer, am 20.10. verworfen): Die Flächen wären keine Ziele mehr; die
  Karte suchte bei jedem Zeigen per `pick` gegen `ground` den sichtbaren
  Block und die Fläche darunter. Hover, Tafel, Tippen, Tastatur und
  „Klick daneben“ hängen an den Ereignissen der Pfade von Leaflet und
  hätten einen eigenen Verteiler gebraucht, jedes Zeigen ein Laden von
  `ground`. Das Netz nach derselben Regel braucht davon nichts.
- **Felder unter 4 px:** Jedes Feld am Rand gibt sein eigenes Stück; bei
  kleinen Feldern wären das mehr Punkte als der Rand nach ε.
- **Felder ab 4 px:** nah viermal so viele Punkte wie mit 16 px und der
  doppelte Heap, nur für eine feinere Grenze zum Verdeckten an einem Ziel,
  das nichts zeigt.
- **`L.canvas()`:** spart Elemente im DOM, nicht die Arbeit je Punkt;
  Tastatur und Fokus auf Flächen gingen verloren. Mit Punkten nach der
  Länge auf dem Schirm ohne Gewinn.

## Folgen

- **Punkte** wachsen mit der Länge der Ränder auf dem Schirm geteilt durch
  ε, nicht mit der Welt; Herauszoomen senkt sie. Das gilt auch für die
  Flächen.
- **Höhen** lädt die Karte für Flächen und Ränder in einem je Ansicht. Die
  Grenzen von 64 Regionen `ground` und 1024 Regionen Höhen gelten so für
  den Ausschnitt, nicht für die Ebene.
- **Rechnen** je Zoom und je neuem Ausschnitt statt einmal je `version`;
  es wächst mit der Länge der Ränder im Ausschnitt in Zellen.
- **Tests** prüfen Eigenschaften, keine Zahlen: Formen ausserhalb ändern
  nichts, Herauszoomen erhöht nie, je Zug höchstens 2 · Länge auf dem
  Schirm / ε und die Ecken, die Abweichung zum Abtasten an den Zellen unter
  ε, Kreise mit Pfeilhöhe unter ε.
- **Ergänzt 0104:** Lage, Glätten, Höhe und Deckkraft der Wand gelten
  weiter; die Zahl ihrer Bänder steht hier.
