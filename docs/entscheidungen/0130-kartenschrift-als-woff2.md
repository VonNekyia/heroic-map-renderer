---
title: "0130: Kartenschrift als WOFF2"
description: Warum die Webkarte die Kartenschrift IM FELL English SC als WOFF2 statt als TTF ausliefert, warum sie dabei ihren reservierten Namen behalten darf und was das spart; ergänzt 0096.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/schrift.ts
  - web/src/ebenen/schrift/IMFeENsc28P.woff2
  - NOTICE
---

# 0130: Kartenschrift als WOFF2

## Anlass

Der User am 24.10., über den Reviewer: Die Kartenschrift kommt als WOFF2,
mit dem Text der OFL daneben. [0096](0096-formen-und-schrift-im-browser.md)
hatte WOFF2 verworfen: Die OFL nenne einen Wechsel des Formats eine
geänderte Fassung, und die dürfe den reservierten Namen nicht tragen. Die
Schrift hat einen, `IM FELL English SC`.

## Entscheidung

- **WOFF2 statt TTF:** `web/src/ebenen/schrift/IMFeENsc28P.woff2`, geladen
  wie bisher mit `FontFace` in `ladeSchrift`. Die TTF verlässt das
  Repository; wie man sie wieder bekommt und die WOFF2 baut, steht in
  [Kartenschrift als WOFF2](../entwicklung/kartenschrift.md).
- **Der Name bleibt:** Die FAQ zur OFL von SIL erlaubt in Q2.2.1 eine WOFF-
  oder WOFF2-Fassung ohne neuen Namen, „only if the original font data
  remains unchanged except for WOFF compression“, und wenn WOFF-Metadaten
  fehlen oder die des Originals unverändert tragen. Beides ist geprüft, siehe
  die Seite oben.
- **Lizenz:** `OFL.txt` bleibt neben der Schrift, `NOTICE` nennt das Format,
  `lizenzen.txt` der Webkarte enthält die OFL wie bisher.

## Folgen

- **Kleiner:** 85 776 statt 183 932 Byte, −53 %, wo nichts komprimiert, wie
  beim Server des Renderers. Hinter einem Proxy mit gzip −22 %, gegen
  110 094 Byte.
- **Richtiger Typ:** Der Server des Renderers liefert `.woff2` als
  `font/woff2`, die TTF ging als `application/octet-stream`.
- **Gleiches Bild:** Namen und Kartenschrift sind auf dem Schirm bitgleich,
  siehe [Kartenschrift als WOFF2](../messungen/2026-10-24-kartenschrift-als-woff2.md).
- **Neu bauen** nur mit einem Werkzeug, das nichts ausser der Kompression
  ändert; die FAQ warnt in Q2.2.2 vor solchen, die mehr tun.

## Verworfene Alternativen

- **TTF behalten:** doppelt so viele Bytes für dieselbe Schrift.
- **Untermenge der Zeichen:** wäre eine geänderte Fassung und bräuchte einen
  neuen Namen; Namen von Städten können jedes Zeichen der Schrift brauchen.
