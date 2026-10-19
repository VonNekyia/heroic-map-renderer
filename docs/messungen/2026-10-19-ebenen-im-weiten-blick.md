---
title: Ebenen im weiten Blick
description: Was die Ebenen eines Servers mit einer Ebene grosser Kreise auf der Webkarte an Punkten in SVG-Pfaden, JS-Heap und langen Aufgaben beim Verschieben kosten, vor und nach Kreisen ohne Gelände, gedeckelten Proben und der Wand erst ab 1 px; weit und nah, in Chromium unter Playwright.
date: 2026-10-19
commits: [ce4720b, ecfaba8]
code:
  - web/src/formen.ts
  - web/src/gelaende.ts
  - web/src/ebenen.ts
  - web/tests/messung-ebenen.spec.ts
---

# Ebenen im weiten Blick

Mit einer Ebene aus 68 Kreisen, 34 davon gefüllt mit 2000 Blöcken Radius,
hatte die Karte im weiten Blick 13,1 Millionen Punkte in ihren SVG-Pfaden und
1,66 GB im JS-Heap; jedes Verschieben kostete 1,2 bis 2,7 s lange Aufgaben.
Fast alles davon war die Wand am Rand der gefüllten Kreise. Nach dem Fix
sind es 99 000 Punkte, 43 MB und keine langen Aufgaben. Übrig bleibt im
weiten Blick das Netz der Flächen, 94 000 Punkte. Eine Regel, die die
Punkte nach dem Fehler auf dem Schirm bemisst, ist geplant.

## Aufbau

- **Stand:** vorher `ce4720b` (master), nachher `ecfaba8`. Gebaut von
  Playwright wie die Tests, `vite build` und `vite preview`.
- **Browser:** Chromium aus Playwright, headless.
- **Daten:** eine Kopie der Ebenen eines Servers, nicht im Repository: 34
  Regionen mit 456 Ecken und 41 184 Blöcken Umfang, 34 Banner, 68 Kreise (34
  mit Rand und 750 Blöcken Radius, 34 gefüllt mit gestricheltem Rand und 2000
  Blöcken Radius). Dazu `map.json` eines Baums `4:3`, scale 8, `maxZoom` 9,
  `groundCell` 1. Die Höhen eben auf 64, `ground` fehlt; Kacheln keine.
- **Weit:** `zoom=-8`. **Nah:** `zoom=0` in der Mitte der Region mit dem
  längsten Umfang.
- **Gemessen:** Punkte als `M` und `L` in allen Pfaden der Ebenen; JS-Heap
  über CDP nach `collectGarbage`; die Summe der langen Aufgaben während
  dreimal 300 px Verschieben; dazu der Heap nach zehnmal Zoomen.

```bash
cd web
KOPIE=<ordner> PW_PORT=4293 npx playwright test --project=grund tests/messung-ebenen.spec.ts --workers=1
```

## Vorher, `ce4720b`, weit

| | ohne Kreise | mit Kreisen |
|---|---|---|
| Punkte | 616 082 | 13 078 862 |
| davon Wand | 521 928 | 11 576 232 |
| davon Netz der Flächen | 93 924 | 1 501 230 |
| JS-Heap | 83–86 MB | 1 658 MB |
| nach zehnmal Zoomen | 71 MB | 1 362 MB |
| Laden, bis die Pfade stehen | 4,4 s | 17 s |
| lange Aufgaben, Fenster 712 × 1213 | 50–71 ms | 1 159 ms |
| lange Aufgaben, Fenster 1278 × 1214 | 152 ms | 2 738 ms |

Der Heap ist nach der Bereinigung bei beiden Fenstern gleich. Was mit der
Fläche des Fensters wächst, sind die Arbeit je Verschieben und der Abfall bis
zur nächsten Bereinigung, nicht die gehaltenen Daten.

## Nachher, `ecfaba8`, Fenster 1278 × 1214

| | weit, ohne | weit, mit Kreisen | nah, ohne | nah, mit Kreisen |
|---|---|---|---|---|
| Punkte | 94 155 | 99 377 | 4 061 | 4 562 |
| davon Netz | 93 924 | 98 061 | 1 101 | 1 144 |
| davon Wand | 0 | 0 | 2 916 | 2 916 |
| JS-Heap | 42 MB | 43 MB | 71 MB | 72 MB |
| nach zehnmal Zoomen | 40 MB | 41 MB | 71 MB | 73 MB |
| Laden | 4,4 s | 4,4 s | 4,4 s | 4,5 s |
| lange Aufgaben | 0 | 0 | 0 | 0 |

## Deutung

- **Die Kreise** liegen ganz auf `seaLevel`: Für die Ebene bräuchte die Karte
  mehr Regionen Höhen, als sie lädt. Ohne Gelände gibt es nichts abzutasten
  und nichts zu verdecken; ein gefüllter Kreis ist jetzt ein 128-Eck ohne
  Netz und ohne Wand.
- **Die Wand** der Regionen fällt im weiten Blick weg: Sie wäre dort unter
  1 px hoch.
- **Das Netz** bleibt im weiten Blick gross, weil es je Feld rechnet, gleich
  wie klein die Felder auf dem Schirm sind.
- **Kein Leck:** Nach dem Zoomen ist der Heap gleich oder kleiner.
- **Rechner:** Werte gelten für diese Maschine und dieses Chromium; verglichen
  wird nur innerhalb einer Serie.
