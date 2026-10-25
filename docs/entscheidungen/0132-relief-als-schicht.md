---
title: "0132: Relief als Schicht für den Mod"
description: Warum die Kacheln das Relief aus --relief eingebacken behalten und der Renderer daneben je Kachel eine Schicht mit 18 Codes je Pixel schreibt, aus der der Mod ohne, slope und shadows rechnet; das Format in map.json, warum auch slope bis zum Schatten zeichnet, ZEICHENSTAND_RELIEF 2, was der Mod nur genähert zurückrechnet, wie Schicht und Rest mit eigenem Manifest und Strom ausgeliefert werden und was verworfen ist; ergänzt 0122.
status: gilt
date: 2026-10-24
issues: []
code:
  - renderer/src/render/metatile/relief.rs
  - renderer/src/render/metatile.rs
  - renderer/src/render/stand.rs
  - renderer/src/render/pyramid.rs
  - renderer/src/cli.rs
  - renderer/src/cli/manifest.rs
  - renderer/src/cli/server.rs
  - renderer/src/cli/server/strom.rs
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
  erst eine eigene Entscheidung aus. Auf der Testwelt sind es 4,6 %, 60,3 MB
  in 60 % der Basiskacheln, gemessen in
  [2026-10-25, Update mit der Schicht des Reliefs](../messungen/2026-10-25-update-mit-relief-schicht.md).
  Der User entschied am 25.10.: der Rest ganz, mit Pyramide, statt nur auf
  der Basis oder ohne.

## Ausliefern

Abgestimmt mit dem Reviewer, dem Mod und dem Plugin am 24. und 25.10. Die
Einzelheiten stehen in [Server](../benutzung/server.md), „Die Schicht des
Reliefs“.

- **Wie je ein Baum:** Schicht und Rest haben je ein eigenes Manifest und
  einen eigenen Strom, `/download/<baum>/relief.zip` und
  `/download/<baum>/relief/rest.zip`, ohne `map.json`. Der Mod nimmt dafür
  denselben Weg wie für einen Baum, nur mit anderem Pfad.
- **Der Baum bleibt Byte für Byte:** Manifest und Strom des Baums nennen
  nichts aus der Schicht. Ältere Mods brechen bei fremden Zeilen im
  Manifest oder Einträgen im Strom ab.
- **Prüfsummen:** Das Plugin bietet die Manifeste von Schicht und Rest mit
  eigener Prüfsumme an, neben der des Baums.

## Was es kostet

Gemessen in
[2026-10-25, Update mit der Schicht des Reliefs](../messungen/2026-10-25-update-mit-relief-schicht.md),
auf der ganzen Testwelt bei scale 4 gegen eingebackenes `slope` ohne Schicht:

- **Ein Update** mit 16 Chunks auf einem Thread: +10 %, 5,81 statt 5,28 s,
  74 statt 64 Kacheln im Gebiet. Das liegt unter den 15 %, ab denen der
  Reviewer im Ring des Schattens nur die Schicht neu rechnen liesse.
- **Ein voller Lauf** mit allen Threads: +17 % in je einem Lauf, +0,25 GiB an
  der Spitze.
- **Platz:** die Schicht 6,7 %, der Rest 4,6 % des Baums mit `slope`.

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
- **Schicht und Rest in Manifest und Strom des Baums:** Ältere Mods brächen
  ab, siehe „Ausliefern“.
- **Der Rest im Strom der Schicht:** Namen wie `rest/<z>/…` und `?nach=`
  bräuchten im Mod einen eigenen Weg. Zwei Ströme in der Form eines Baums
  liest er mit dem Weg, den er schon hat.
- **`relief` als Objekt** mit Standard und Schicht: Das Feld wechselte
  seinen Typ, und alte Clients läsen es falsch. Ein Feld daneben passt zu
  `heights` und `heightsCell`.
