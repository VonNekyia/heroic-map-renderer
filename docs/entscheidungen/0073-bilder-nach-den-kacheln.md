---
title: "0073: Bilder des Skins nach den Kacheln"
description: Warum der Skin Tablett seine Bilder erst lädt, wenn die Ebene der Kacheln zum ersten Mal fertig ist, mit niedriger Priorität, und danach einblendet, statt sie gleich beim Start zu holen oder in einen Atlas zu packen.
status: gilt
date: 2026-10-04
issues: [112]
code:
  - web/skins/tablett/index.ts
  - web/skins/tablett/tablett.css
---

# 0073: Bilder des Skins nach den Kacheln

## Anlass

Mit den Bildern aus der Vorlage
([0070](0070-bilder-aus-der-vorlage.md)) warnte Lighthouse mit Skin vor dem
LCP: 5,5 s, Performance 0,80, im Demobaum mit `seaLevel` und `area`. Das
grösste Element ist eine Kachel. Sie wartete 3,8 s, weil der Skin vorher
seine 22 Bilder holte, zusammen 631 KB, und der Preview-Server HTTP/1.1 mit
6 Verbindungen je Host spricht. `fetch` mit `priority: 'low'` allein
änderte daran nichts (5,5 s). Gefunden im Review zu #135
(issuecomment-5976899037, Befund 2).

## Entscheidung

Vom Reviewer entschieden: Die Karte ist der Inhalt, das Tablett ist Schmuck.

- Der Skin lädt seine Bilder erst, wenn die Ebene der Kacheln zum ersten
  Mal `load` meldet und das Bild danach gemalt ist, nach zwei
  `requestAnimationFrame`. Er erkennt die Ebene an `layeradd`; die
  Grundkarte legt sie nach dem Skin an.
- Warum das gemalte Bild: Leaflet blendet eine geladene Kachel erst im
  nächsten Bild ein, und erst dann ist sie das grösste Element. Lighthouse
  rechnet zum LCP jede Anfrage, die davor beginnt.
- Der Skin holt jedes Bild mit `fetch(…, { priority: 'low' })`.
- Die Leinwände kommen mit dem ersten Bild auf die Karte, wenn die Bilder
  da sind, und blenden über 0,4 s ein (CSS-Animation
  `tablett-einblenden`, ohne bei `prefers-reduced-motion: reduce`).

## Verworfene Alternativen

- **Alle Bilder gleich beim Start,** wie bisher: Die erste Kachel wartete
  hinter ihnen, LCP 5,5 s.
- **Nur `priority: 'low'`:** Der Browser startete die Bilder trotzdem vor
  den Kacheln, LCP unverändert 5,5 s.
- **Gleich bei `load`:** Die Kachel war da noch nicht gemalt; LCP 5,5 bis
  5,6 s.
- **Ein Atlas statt 22 Dateien:** kommt nur dazu, wenn Lighthouse mit den
  Kacheln vorn noch warnt. Dann zählt die Zahl der Dateien kaum noch.

## Folgen

- Mit Skin hält Lighthouse jede Schwelle, auch die Warnung zum LCP:
  [Bilder des Skins nach den Kacheln](../messungen/2026-10-04-skin-bilder-nach-kacheln.md).
- Das Tablett erscheint nach der Karte, nicht mit ihr. Bis dahin liegt die
  Welt auf dem Grund der Karte.
- Meldet die Ebene der Kacheln nie `load`, etwa weil keine Kachel im Bild
  liegt, bleibt das Tablett aus. In der Gesamtansicht liegt immer die Welt
  im Bild.
- Ein Test prüft, dass keine Anfrage für ein Bild des Skins vor der ersten
  Kachel startet. Dass sie auch nach dem gemalten Bild startet, prüft keiner:
  Unter Last meldet der Browser die Zeit des Malens bis 10 ms nach dem Bild,
  in dem der Skin beginnt. Das zeigt nur Lighthouse mit Skin, und das läuft
  nicht in der CI.
