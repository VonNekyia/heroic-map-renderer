---
title: "0132: Relief als Schicht für den Mod"
description: Warum die Kacheln das Relief aus --relief eingebacken behalten und der Renderer daneben je Kachel eine Schicht mit 18 Codes je Pixel schreibt, aus der der Mod ohne, slope und shadows rechnet; das Format in map.json, warum auch slope bis zum Schatten zeichnet, ZEICHENSTAND_RELIEF 2, was der Mod nur genähert zurückrechnet und was verworfen ist; ergänzt 0122.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
---

# 0132: Relief als Schicht für den Mod

Ergänzt [0122](0122-relief-von-oben.md): Die Kacheln bleiben, wie 0122 sie
zeichnet, dazu kommt die Schicht.

## Anlass

Der User am 24.10.:
- `slope` wird der Standard der Karte von main, und die Webkarte zeigt nur
  ihn, ohne Wahl.
- Im Mod soll man alle drei wählen können: ohne, `slope` und `shadows`.

## Entscheidung

- **Die Kacheln** tragen weiter das Relief aus `--relief`. Webkarte, Server,
  Manifest und Strom bleiben, wie sie sind; alte Clients zeigen das Relief
  wie bisher.
- **Die Schicht:** Zu jeder Kachel schreibt der Renderer ein graues WebP
  mit einem Code je Pixel, in `relief/` des Baums. Wie der Code entsteht
  und was er heisst, steht in [Relief von oben](../renderer/relief.md),
  „Die Schicht“.
  - Die Pyramide der Schicht nimmt denselben Pixel wie die der Kacheln,
    darum ist sie auf jeder Stufe genau.
  - Zu jeder Kachel steht eine Datei, auch wo alles unverändert ist: Eine
    fehlende Datei gäbe auf der Webkarte ein 404 in der Konsole.
- **`map.json`:** `reliefTiles` neben `relief`, das Pfadmuster relativ zum
  Baum. `relief` bleibt ein String und nennt, was in den Kacheln steckt. Ein
  neues Format der Schicht bekäme ein neues Feld statt einer
  Versionsnummer. Abgestimmt mit Frontend und Mod am 24.10.
- **Auch `slope` zeichnet bis zum Schatten:** Die Schicht trägt den
  Schatten, denn der Mod soll `shadows` zeigen können. Ein Update reicht
  darum mit jedem Relief 2 Chunks nach Südosten, siehe
  [Updates](../benutzung/updates.md), „Wo ein Update zeichnet“.
- **`ZEICHENSTAND_RELIEF` steigt auf 2:** Bäume mit Relief aus einem Stand
  davor haben keine Schicht und brauchen einen vollen Lauf. Bäume ohne
  Relief behalten ihren Abdruck.
- **Ohne Relief** nimmt ein voller Lauf die Schicht weg.
- **Gemessen** auf der ganzen Testwelt bei scale 4, in
  [2026-10-24, Relief als Schicht](../messungen/2026-10-24-relief-als-schicht.md):
  bitgleich auf allen Stufen, die Schicht +7,7 % des Platzes der Karte ohne
  Relief.

## Was der Mod nur genähert rechnet

Der Mod rechnet die anderen Arten aus der Kachel mit `slope`: ohne durch
Teilen durch den Faktor, `shadows` mit · 0,75 ab Code 9. Gezählt an der
Basis der Testwelt bei scale 4, 1,02 Mrd. sichtbare Pixel, in f32 wie im
Renderer:

| | ±1 | über 1 | höchstens |
|---|---|---|---|
| `slope` schneidet bei 255 ab | | 2,40 % | |
| ohne | 9,1 % | 2,35 % | 49 |
| `shadows` | 5,6 % | 0,36 % | 46 |

- **Über 1** liegt jede Abweichung auf einem abgeschnittenen Pixel: helle
  Hänge zum Licht, Schnee und Sand. Ohne Abschneiden bleibt es bei ±1.
- **Darum der Rest:** An den abgeschnittenen Pixeln steht die Farbe ohne
  Relief in `relief/rest/`, nur zu Kacheln, die welche haben. Mit ihm
  trifft der Mod jede Art auf ±1, nach der Regel in
  [`map.json`](../benutzung/map-json.md), „Die Schicht lesen“. Reviewer am
  24.10., Mod einverstanden. Übersteigt der Rest 3 % des Baums, liefert ihn
  erst eine eigene Entscheidung aus.

## Verworfene Alternativen

- **Drei Bäume, einer je Art:** dreimal Platz, Zeit und Download.
- **Kacheln ohne Relief und die Schicht, die Webkarte mischt im Browser:**
  - Mit CSS geht es nicht: `mix-blend-mode` dunkelt nur ab und wird nie
    bitgleich.
  - Es bliebe eine Leinwand je Kachel statt `<img>`, mit Folgen für die
    Grundschicht, das Abbrechen beim Zoomen und den Speicher, siehe
    [0128](0128-marker-nur-im-ausschnitt.md).
  - Dazu je Kachel eine zweite Anfrage über dieselben sechs Verbindungen.
  - Der Wechsel des Standards ohne Render, den dieser Weg brächte, braucht
    niemand: Die Webkarte zeigt nur `slope`.
- **Kacheln ohne Relief, der Server mischt beim Ausliefern:** Kosten je
  Anfrage und ein Cache. Wer die Kacheln statisch ausliefert, zeigte kein
  Relief.
- **`relief` als Objekt** mit Standard und Schicht: Das Feld wechselte
  seinen Typ, und alte Clients läsen es falsch. Ein Feld daneben passt zu
  `heights` und `heightsCell`.
