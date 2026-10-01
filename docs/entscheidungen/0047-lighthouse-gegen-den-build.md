---
title: "0047: Lighthouse gegen den Build, mit festen Schwellen"
description: Warum die CI die Karte mit Lighthouse gegen den fertigen Build prüft, mit festen Schwellen und ohne Crawler oder Baseline, und warum die Header beim Betreiber liegen.
status: gilt
date: 2026-10-01
issues: [70]
code:
  - .github/workflows/ci.yml
  - web/lighthouserc.cjs
  - web/vite.config.ts
  - web/tests/smoke.spec.ts
---

# 0047: Lighthouse gegen den Build, mit festen Schwellen

## Anlass

Der Job „Frontend“ prüft Typen, Lint, den Build und mit Playwright, ob die
Karte lädt und die Koordinaten stimmen. Wie die ausgelieferte Seite bei
Suchmaschinen, Barrierefreiheit und guter Praxis im Browser abschneidet,
prüfte niemand. Issue #70.

## Entscheidung

- Ein eigener Job lässt Lighthouse dreimal gegen den fertigen Build laufen,
  ausgeliefert wie beim Betreiber: die Kacheln als `tiles/` neben der
  Seite. Gewertet wird der Median.
- Die Schwellen in `web/lighthouserc.cjs` sind fest. Was der Stand nicht
  schafft, wird behoben, nicht gelockert. Performance und LCP sind nur
  Warnungen.
- Lighthouse und der Browser haben feste Versionen: `@lhci/cli` genau im
  `package-lock.json`, das Chromium von Playwright statt des Chrome des
  Runners.
- Die Header setzt der Server des Betreibers. Das Repository prüft nur,
  dass die Karte unter strengen Headern läuft: `vite preview` liefert den
  Build in allen Tests mit ihnen aus, und ein Test schlägt bei jeder
  Verletzung der Content-Security-Policy fehl.

## Streuung auf dem Runner

Zehn Läufe auf `ubuntu-latest`, Stand der PR zu #70:

| Wert | kleinster | grösster | Schwelle |
|---|---|---|---|
| CLS | 0 | 0 | ≤ 0,1, Fehler |
| TBT | 0 ms | 0 ms | ≤ 300 ms, Fehler |
| LCP | 1396 ms | 1506 ms | ≤ 4000 ms, Warnung |
| Performance, Barrierefreiheit, SEO | 1 | 1 | ≥ 0,5 Warnung, ≥ 0,9 Fehler |
| Best Practices | 0,96 | 0,96 | ≥ 0,9, Fehler |

- CLS und TBT streuten nicht. Die Schwellen liegen weit darüber; sie
  fangen ein Layout, das springt, oder ein Skript, das den Start blockiert,
  nicht das Rauschen des Runners.
- Best Practices verliert 0,04 an der Anfrage nach `/favicon.ico`, die ins
  Leere geht. Das Icon kommt mit #71.

## Verworfene Alternativen

- **Ein Crawler.** Er lohnt sich für Seiten mit vielen Unterseiten und
  Links. Die Karte ist eine einzige Seite ohne interne Links. Was er dort
  fände, decken Lighthouse (Status, Konsole, Metadaten, Barrierefreiheit)
  und der Smoke-Test (fehlende Kacheln, Fehler beim Laden) schon ab. Dazu
  kämen ein Binary von gut 40 MB je Lauf und eine Baseline.
- **Vergleich mit einer Baseline.** Sie muss bei jeder gewollten Änderung
  nachgezogen werden. Für eine Seite genügen feste Schwellen.
- **Die Header einer Produktion prüfen.** Das Repository kennt keine
  Produktion; die Header setzt der Server des Betreibers.
- **Ein eigener Server für die Prüfung**, etwa in Python. `vite preview`
  liefert den Build schon für den Smoke-Test aus, mit Headern.

## Folgen

- Eine neue Abhängigkeit für die Entwicklung, `@lhci/cli` mit Lighthouse,
  rund 320 Pakete. In den Build der Karte kommt nichts davon.
- Ein Job mehr, rund 2 bis 3 Minuten, parallel zu den übrigen.
- `index.html` hat eine Beschreibung, und `public/robots.txt` erlaubt
  alles: Ohne beides fiel der Stand durch.
