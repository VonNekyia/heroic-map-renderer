---
title: CI
description: Welche Jobs die CI bei jedem Push und jeder PR laufen lässt, mit welchen Adaptern die GPU-Tests laufen, welche Schwellen Lighthouse an die Karte anlegt und wie die Doku-Prüfung Verweise, Links und Frontmatter prüft.
code:
  - .github/workflows/ci.yml
  - .github/pruefe-doku.sh
  - renderer/deny.toml
  - web/lighthouserc.cjs
---

# CI

Die CI in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) läuft
bei jedem Push auf `master` und bei jeder PR: Rust unter Ubuntu und
Windows, das Frontend samt Lighthouse, die Lizenzen der Abhängigkeiten, die Coverage und die
Doku. Ein fehlender GPU-Adapter ist dort ein Fehler, kein übergangener Test.
Dieselben Befehle lokal: [Tests](tests.md), „Laufen lassen“.

## Jobs

| Job | Läuft auf | Was |
|---|---|---|
| Rust | Ubuntu und Windows | `cargo fmt --all --check` (nur Ubuntu), `cargo clippy --all-targets -- -D warnings`, `cargo nextest run --all-targets`, unter Ubuntu auch in Release; dazu das Binär für die Weitergabe: unter Linux Grösse und glibc, unter Windows das Debug-Binär ohne VC++-Laufzeit, siehe [Weitergabe](weitergabe.md) |
| Frontend | Ubuntu | `npm run check`, `npm run lint`, der Build mit einer Attrappe unter `public/tiles`, der Smoke-Test mit Playwright |
| Lighthouse | Ubuntu | Lighthouse gegen den Build, mit Schwellen, siehe unten |
| Dependencies | Ubuntu | `cargo deny check` mit [`renderer/deny.toml`](../../renderer/deny.toml), die Lizenzen der Abhängigkeiten und der eigenen Crate, siehe [0078](../entscheidungen/0078-apache-2-0.md); dazu `renderer/drittlizenzen.py`, siehe [Drittlizenzen](drittlizenzen.md) |
| Coverage | Ubuntu | `cargo llvm-cov --all-targets` |
| Doku | Ubuntu | `bash .github/pruefe-doku.sh`, dazu eine Probe, dass sie anschlägt |

Die Schritte, die unseren Code laufen lassen, haben ein Zeitlimit
(`timeout-minutes` in `ci.yml`): die Tests, die Coverage, der Smoke-Test,
Lighthouse und die Doku-Prüfung. Es beträgt mindestens das Fünffache der
üblichen Dauer des Schritts, mindestens 5 Minuten. Hängen kann nur unser
Code, etwa ein Test in einer Sperre. Ohne Limit wartet GitHub bis zu
6 Stunden und hält so lange jeden Merge auf. Die Installation von Paketen
und Browsern läuft ohne eigenes Limit: An einem langsamen Tag braucht
`apt` dafür über eine halbe Stunde, und ein langsamer Spiegel soll die CI
nicht rot machen. Gegen eine Umgebung, die ganz stehen bleibt, hat jeder
Job eine Sicherung von 120 Minuten.

Die Tests laufen in Release, weil sich die Überlauf-Semantik zwischen Debug
und Release unterscheidet; die Prüfungen im Regionsleser müssen in beiden
greifen. Ein Test läuft nur dort, siehe [Tests](tests.md), „Laufen
lassen“. Fällt ein Goldbild, liegt das Ist-Bild als Artefakt am Lauf, siehe
[Tests](tests.md), „Goldbild“.

## Release

Ein zweiter Workflow,
[`.github/workflows/release.yml`](../../.github/workflows/release.yml), baut
die Pakete für [Installation und Releases](../benutzung/installation.md).
Warum so: [0082](../entscheidungen/0082-versionen-und-releases.md).

| Job | Läuft auf | Was |
|---|---|---|
| Linux x64 | Container `quay.io/pypa/manylinux_2_28_x86_64` auf Ubuntu | Release-Build, glibc höchstens 2.28, gepackt unter der Grenze je Binär, `--version`, Paket als `.tar.gz` |
| Windows x64 | Windows | Release-Build, ohne VC++-Laufzeit, gepackt unter der Grenze je Binär, `--version`, Paket als `.zip` |
| Budget beider Binärs | Ubuntu | die Summe beider gepackten Binärs unter der Grenze für das Jar |
| Release-Entwurf | Ubuntu | nur auf einem Tag: `SHA256SUMS` und ein Entwurf des Releases mit beiden Paketen; die Notizen verlinken die Installation und nehmen den Hinweis von Mojang und den Herausgeber aus `NOTICE` |

- **Auslöser:** ein Tag `v*`. Auf einem Tag prüfen beide Builds, dass die
  Version in `renderer/Cargo.toml` dem Tag gleicht.
- **Ohne Tag:** Ändert eine PR `release.yml`, `renderer/Cargo.toml` oder
  `renderer/Cargo.lock`, laufen die beiden Builds samt Paketen und das
  Budget, aber kein Entwurf. Neue Abhängigkeiten lassen die Grösse
  springen; so sieht die PR die Summe und das Windows-Binär vor dem ersten
  Tag, entschieden am 06.10. im Review zu #183. Die Pakete liegen als
  Artefakte am Lauf.
- **Veröffentlichen** ist Sache des Maintainers, aus dem Entwurf heraus.
- **Rechte:** Der Workflow liest nur; allein der Job für den Entwurf darf
  schreiben.
- Die Grenzen und die Prüfungen stehen in [Weitergabe](weitergabe.md),
  „Grenze“; hier gelten sie am Release-Binär beider Systeme.

## GPU-Tests in der CI

Die GPU-Tests brauchen einen Adapter. Auf Windows ist WARP dabei, auf
Ubuntu liefert Mesa mit lavapipe eine Vulkan-Implementierung in Software:
langsam, aber derselbe Shader-Weg wie auf einer Karte. Rust und Coverage
setzen `HEROIC_GPU_PFLICHT`; fehlt der Adapter, ist das ein Fehler.

## Lighthouse

Der Job prüft die Karte so, wie ein Betreiber sie ausliefert: `npm run
build`, die Kacheln aus `public/tiles-demo` als `dist/tiles` daneben,
ausgeliefert mit `vite preview` unter den Headern aus `web/headers.json`, gelesen von `preview.headers` in
[`web/vite.config.ts`](../../web/vite.config.ts). Lighthouse lädt die Seite
dreimal; gewertet wird der Median.

- **Versionen fest:** `@lhci/cli` steht genau im `package-lock.json`, mit
  Prüfsumme. Der Browser ist das Chromium, das Playwright in seiner festen
  Version mitbringt, nicht das Chrome des Runners, das wechselt.
- **Schwellen** in [`web/lighthouserc.cjs`](../../web/lighthouserc.cjs). Fehler
  sind SEO, Barrierefreiheit und Best Practices, Layout-Sprünge (CLS) und
  blockierende Skripte (TBT), dazu Titel, Beschreibung, HTTP-Status,
  Indexierbarkeit und `robots.txt`. Performance und LCP sind nur Warnungen:
  Auf einem geteilten Runner schwanken sie stark, und die Karte lädt ihre
  Kacheln erst nach dem Skript.
- **Abstand zur Streuung:** Die Schwellen für CLS und TBT halten Abstand zu
  dem, was zehn Läufe auf dem Runner streuten, siehe
  [0047](../entscheidungen/0047-lighthouse-gegen-den-build.md).
- **Bericht:** Der Schritt „Werte je Lauf“ schreibt Kategorien, CLS, TBT
  und LCP jedes Laufs ins Log. Die HTML-Berichte liegen als Artefakt
  `lighthouse` am Lauf, 14 Tage, auch wenn er grün ist.

Lokal, aus `web/`, mit dem Chromium von Playwright:

```bash
npm run build && cp -r dist/tiles-demo dist/tiles
export CHROME_PATH="$(node --input-type=module -e "import {chromium} from '@playwright/test'; console.log(chromium.executablePath())")"
npx lhci collect --config=./lighthouserc.cjs
npx lhci assert --config=./lighthouserc.cjs
```

## Die Doku-Prüfung

[`.github/pruefe-doku.sh`](../../.github/pruefe-doku.sh) prüft mit ein paar
Zeilen Bash:

- Jeder Verweis im Code, „Siehe“, eine Seite unter `docs/` und wahlweise
  eine Überschrift in „…“, zeigt auf eine bestehende Seite und
  Überschrift. Geprüft wird jede Datei, die Git verfolgt, ausser Markdown;
  die Form steht in [`AGENTS.md`](../../AGENTS.md), Regel 18.
- Relative Links in `docs/`, `skills/`, `README.md` und `AGENTS.md` zeigen
  auf bestehende Dateien; folgt einem Link auf eine Seite eine Überschrift
  in „…“, gibt es sie dort. Links in Codeblöcken zählen nicht.
- Jede Seite in `docs/` beginnt mit Frontmatter zwischen zwei `---`, mit
  `title`, `description` und `code:`, dieses mit mindestens einem Pfad;
  nur `docs/index.md` hat keinen. Jeder Pfad unter `code:` existiert.
- Jede Seite in `docs/` steht in `docs/index.md`.

Anker hinter `#` prüft sie nicht; auf eine Überschrift zeigt ein Link mit
„…“ dahinter, wie oben.

Jeder Fehler steht als Zeile `::error file=…::…` da, die GitHub an die
Datei heftet. Danach verbiegt die CI in einer Probe je einen Verweis, eine
Überschrift, einen Link und einen Pfad unter `code:`, nimmt einer Seite den
`title` und verlangt genau fünf Meldungen; so fällt auf, wenn die Prüfung
nichts mehr findet. Lokal aus der Wurzel des Repositorys:

```bash
bash .github/pruefe-doku.sh
```

Die Prüfung sieht nur, was Git verfolgt: eine neue Seite erst nach
`git add`.
