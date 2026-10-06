---
title: "0048: Adresse und Angaben der Seite beim Build"
description: Warum Adresse, Titel, Beschreibung und Vorschaubild der Karte beim Build hineinkommen und nicht aus dem Repository oder dem Renderer, und warum Sitemap, strukturierte Daten, Manifest und hreflang fehlen.
status: gilt
date: 2026-10-01
issues: [71]
code:
  - web/vite.config.ts
  - web/index.html
  - web/tests/seite.spec.ts
---

# 0048: Adresse und Angaben der Seite beim Build

In Teilen abgelöst durch [0085](0085-seitenangaben-zur-laufzeit.md): Die
Tags stehen mit Markern in `index.html` statt im Plugin `seite`, und der
Server des Renderers setzt die Angaben auch zur Laufzeit ein. Beim Build
gilt weiter, was hier steht.

## Anlass

Die Seite hatte nur einen festen Titel. Wer ihren Link teilte, sah keine
Vorschau, und Suchmaschinen fanden eine Seite ohne Text. `canonical`,
`og:url` und `og:image` brauchen die absolute Adresse, unter der die Karte
läuft. Die darf nicht im Repository stehen. Issue #71.

## Entscheidung

- Der Betreiber setzt Adresse, Titel, Beschreibung und Vorschaubild beim
  Build: `SITE_URL`, `SITE_TITLE`, `SITE_DESCRIPTION`, `SITE_IMAGE`. Das
  Plugin `seite` in `web/vite.config.ts` füllt damit den Kopf von
  `index.html` und schreibt `robots.txt`.
- Ohne `SITE_URL` fehlt alles, was eine absolute Adresse braucht; Titel,
  Beschreibung und Icon bleiben.
- `robots.txt` erlaubt alles ausser `tiles/`, ab der Wurzel der Domain
  gerechnet; `tiles/map.json` bleibt erlaubt, sonst rendert eine
  Suchmaschine statt der Seite eine Fehlermeldung.
- Tests und Doku nennen nur `https://example.org/` (RFC 2606).

## Verworfene Alternativen

- **Ein Platzhalter, den der Renderer füllt.** Der Renderer schreibt
  Kacheln und `map.json`, nie die Seite; Seite und Kacheln liegen getrennt
  nebeneinander. Er müsste dafür künftig die Seite anfassen. So bleibt
  das Backend unberührt, und `map.json` ändert sich nicht.
- **Die Adresse zur Laufzeit aus `location`.** Vorschauen in Chats und die
  meisten Crawler führen kein Skript aus; sie lesen nur den Kopf der Seite.
- **`sitemap.xml`.** Sie hilft bei vielen Seiten. Eine einzelne Seite findet
  eine Suchmaschine über den Link auf sie; die Adresse trägt `canonical`.
- **Strukturierte Daten (JSON-LD).** Für eine Karte gibt es kein Rich
  Result. `WebSite` mit Namen zählt nur auf der Startseite einer Domain,
  und die Karte ist oft eine Unterseite.
- **`manifest.json`.** Die Karte als App zu installieren, braucht niemand
  (`AGENTS.md`, Regel 22).
- **`hreflang`.** Es gibt eine Sprache.

## Folgen

- Für jede Adresse ein eigener Build. Die Seite ist klein, der Build dauert
  rund zwei Sekunden (in der CI 1,9 s).
- Lighthouse am Build ohne `SITE_URL`: alle vier Kategorien 1, auch Best
  Practices, seit das Icon `/favicon.ico` erspart.
