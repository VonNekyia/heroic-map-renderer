---
title: "0131: Zoomgrenze je Ebene"
description: Wie die Webkarte `maxBlocksPerPixel` aus `layers.json` umsetzt, eine Ebene jenseits der Grenze nicht lädt, beim Zoomen aus- und einhängt statt neu zu laden, warum eine offene Tafel dabei schliesst, wohin der Fokus geht und wie die Liste es zeigt.
status: gilt
date: 2026-10-24
issues: []
code:
  - web/src/ebenen.ts
  - web/src/style.css
---

# 0131: Zoomgrenze je Ebene

## Anlass

Der User am 24.10., über den Reviewer: Objekte einer Ebene, besonders
Regionen, sollen ab einer Zoomstufe nicht mehr geladen und gezeigt werden;
das gibt das Plugin an. Plan mit Plugin Code Map, vom Reviewer
freigegeben. Der Vertrag steht in [Ebenen](../benutzung/ebenen.md),
„Zoomgrenze“; das Plugin schreibt das Feld ab 0.20.0.

## Entscheidung

- **Rechnung:** Blöcke je CSS-Pixel = 1 / (`blockBreite(blick)` ·
  2^(Zoom − maxZoom)), wie für die Namen in
  [0119](0119-namen-stufenlos-icons-fest.md). Sichtbar, solange der Wert
  ≤ `maxBlocksPerPixel` ist; mit 10⁻⁹ Spielraum, damit eine Grenze genau auf
  einer Stufe nicht an der Rundung hängt. Von oben und im iso dieselbe
  Rechnung.
- **Nicht laden:** `ladeEbene` kehrt jenseits der Grenze sofort um, vor jeder
  Anfrage, auch das Nachfragen alle 30 s. Ein Laden, das beim Hinauszoomen
  noch läuft, verwirft sich wie beim Abschalten (`gilt`).
- **Aus- und einhängen:** Nach jedem Zoom prüft `bereichAnwenden` jede
  angehakte Ebene. Eine geladene jenseits der Grenze nimmt sie mit Markern
  und Formen von der Karte, vergisst sie aber nicht. Kommt sie zurück, hängt
  die Karte sie wieder ein, ohne Anfrage; nur mit neuer `version` oder
  ungeladen lädt sie. Ausgehängt zählt die Karte weder Orte noch Formen neu.
- **Tafel schliesst:** Fällt eine Ebene über die Grenze, schliesst ihre offene
  Tafel, wie beim Abschalten. Die Ausnahme aus
  [0128](0128-marker-nur-im-ausschnitt.md), nach der ein Marker mit Tafel
  bleibt, gilt hier nicht: Die Grenze nimmt die ganze Ebene, nicht einen
  Ort. Gewollt, kein Fehler.
- **Fokus:** Lag er in der Ebene, geht er an die Karte.
- **Liste:** Die Ebene bleibt angehakt. Jenseits der Grenze steht ihre Zeile
  gedimmt, mit dem Text „(beim Hineinzoomen)“, so auch für Screenreader.
- **Aktionsebenen** wie jede Ebene.

## Folgen

- Eine Ebene mit Grenze kostet weit draussen weder Anfrage noch Elemente.
- Beim Hinauszoomen verschwindet sie am Ende des Zooms, beim Hineinzoomen
  erscheint sie danach, nicht mitten in der Animation.
- Ohne das Feld, etwa mit einem Plugin vor 0.20.0, bleibt alles wie bisher.

## Verworfene Alternativen

- **Grenze in Leaflet-Stufen:** hinge von `scale` und Kamera des Baums ab;
  dieselbe Ebene wäre in jedem Baum anders weit sichtbar.
- **Je Objektart:** Auf dem Spiegel liegen Regionen und Städte in getrennten
  Ebenen; eine Grenze je Ebene reicht.
- **Beim Hinauszoomen vergessen und neu laden:** kostet bei jedem Zoom über
  die Grenze eine Anfrage und den Bau aller Objekte.
