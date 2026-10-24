---
title: "0123: Formen im iso gemerkt je Stufe"
description: Warum die Webkarte Flächen, Ränder und Linien im iso je Ebene und Stufe merkt, eine kleine Ebene ganz statt im Ausschnitt rechnet, in Stücken rechnet und abbricht, wenn eine neuere Ansicht gilt, warum die Caches der Höhen so viel halten wie eine Ansicht lädt und wie `verdeckt` Blöcke überspringt, ohne das Ergebnis zu ändern; ergänzt 0111.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
  - web/src/formen.ts
  - web/src/gelaende.ts
  - web/src/hoehen.ts
---

# 0123: Formen im iso gemerkt je Stufe

## Anlass

Der User am 24.10. über die Webkarte auf der Produktion: Die Regionen im iso
laggen sehr und sollen mindestens 10-mal schneller werden. Gemessen auf dem
der grossen Welt, siehe
[Regionen im iso schneller](../messungen/2026-10-24-regionen-im-iso-schneller.md):
Je Ansicht rechnete die Karte jeden Rand im Ausschnitt neu, und `verdeckt`
schickte je Punkt einen Strahl in halben Zellen bis über das höchste
Gelände. Plan des Frontends, freigegeben vom Reviewer am 24.10.

## Entscheidung

- **Gemerkt je Ebene und Stufe:** Die letzte Rechnung bleibt, zusammen
  höchstens 16 MB, geschätzt mit 64 Byte je Punkt, die am längsten
  ungenutzte fällt zuerst. Sie gilt wieder, solange das Fenster in ihrem
  Rahmen liegt. Ein Zoom zurück auf eine Stufe rechnet so nichts. Eine neu
  geladene Ebene, also neue `version` oder ein neues `devicePixelRatio`,
  hat neue Einträge; ein neuer Baum lädt die Seite neu.
- **Die ganze Ebene, wenn sie klein ist:** Liegt ihr Rahmen auf dem Schirm,
  über alle Höhen, in 9 Fenstern und passen ihre Höhen ins Budget, rechnet
  die Karte sie ganz statt im Ausschnitt. Dann rechnet auf derselben Stufe
  kein Verschieben neu. Sonst gilt der Ausschnitt aus 0111.
- **In Stücken:** `zeichneAnsicht` rechnet je 8 ms und gibt dann den
  Hauptfaden frei. Gilt eine neuere Ansicht, bricht sie ab; sonst liefen
  zwei Rechnungen ineinander.
- **Caches der Höhen:** `heights` und `ground` halten je so viel, wie eine
  Ansicht laden darf, `HOEHEN_BYTES`, und verdrängen die am längsten nicht
  gefragte Karte. Vorher hielten sie 64 und 8 Karten und verdrängten die
  älteste; im weiten Blick lud und entpackte jede Ansicht sie neu.
- **Gleiche Werte, weniger Arbeit:**
  - Das Höchste einer Karte und je Block von 16 Zellen rechnet die Karte
    einmal je geladener Karte.
  - `verdeckt` überspringt die Schritte über einem Block, dessen Höchstes
    samt Nachbarn unter dem Strahl liegt, denn der Strahl steigt weiter.
  - Der Ersatz einer leeren Zelle wird je Gelände einmal gerechnet.
  - Ein Test vergleicht `verdeckt` bitgleich mit dem einfachen Strahl.

## Verworfene Alternativen

- **Ein Durchgang je Zeile mit laufendem Horizont:** Die Punkte eines Rands
  liegen nicht auf den Zellen und haben die geglättete Höhe; das Ergebnis
  wiche ab.
- **Blöcke aus den Zellen je Ansicht:** Gemessen langsamer als der einfache
  Strahl, denn die Blöcke kosten mehr als die Schritte, die sie sparen.
- **Immer die ganze Ebene:** Eine Ebene über Tausende Blöcke gäbe nah
  Millionen Felder im Netz.
- **Jede Ansicht neu, nur schneller:** reicht nicht; gemessen blieben
  0,82 s für 20 Ansichten.
- **Ein Worker:** Er nähme die Rechnung beim Zoomwechsel vom Hauptfaden.
  Er kommt als eigener Schritt, wenn das Ruckeln beim ersten Hinauszoomen
  stört.

## Folgen

- Gemessen auf der grossen Welt, je drei Läufe: Das Blockieren durch lange
  Tasks sinkt im Mittel von 3 952 ms auf 21 ms, das Rechnen der Ansichten
  von 3 828 ms auf 454 ms; siehe die Messung.
- Ganz gerechnet hängt ein Rand nicht mehr vom Ausschnitt ab. Am Rand des
  Ausschnitts sahen Glätten und Verdeckung vorher andere Nachbarn, und ein
  Rand konnte beim Verschieben springen. Für grosse Ebenen gilt das weiter.
- Der Speicher wächst um höchstens 16 MB für die gemerkten Rechnungen und
  bis `HOEHEN_BYTES` je Art für die Höhen; gemessen lag der JS-Heap vorher
  bei 50–123 MB, nachher bei 64–109 MB.
