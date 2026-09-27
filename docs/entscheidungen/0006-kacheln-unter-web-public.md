---
title: "0006: Die Kacheln liegen unter web/public/tiles"
description: Warum das Frontend seine Kacheln ohne Konfiguration unter web/public/tiles findet und warum der Build sie auslässt.
status: gilt
date: 2026-09-22
issues: [7, 9]
code:
  - web/vite.config.ts
  - web/src/main.ts
  - .github/workflows/ci.yml
---

# 0006: Die Kacheln liegen unter web/public/tiles

## Anlass

Schritt 7 baut das Frontend. Es muss die Kacheln finden, die der Renderer
schreibt.

## Entscheidung

Der Renderer schreibt die Kacheln dorthin, wo der Devserver sie ausliefert,
nach `web/public/tiles`. Ohne Angabe lädt die Seite `tiles/` neben sich;
`?tiles=` nennt einen anderen Pfad, für den Smoke-Test und für mehrere
Karten auf demselben Server. Seit #9 kopiert der Build `public/` selbst, mit
aufgelösten Links, nur ohne `tiles`, und der Watcher des Devservers lässt
die Kacheln aus. Siehe [Frontend](../frontend.md).

## Verworfene Alternativen

- **Ein Proxy im Devserver oder eine konfigurierbare Basis-URL.** Das wäre
  Konfiguration, die jemand pflegen muss.
- **Vites eigene Kopie von `public/`.** Sie folgte einem Link auf den
  Kachelordner, auch unter `npm test`: nach einem Vollrender 300 GB.
- **Der Watcher auch über den Kacheln.** Er beobachtete jede der Millionen
  Dateien und verbrannte Kerne, die der Render braucht.

## Folgen

- Beim Ausliefern gehören die Kacheln als `tiles/` neben die Seite.
- Wer einem Render zusehen will, setzt einen Link; aus einem echten
  Verzeichnis liefert Vite nur, was beim Start dalag.
- Die CI prüft mit einer Attrappe, dass der Build `public/tiles` auslässt.
