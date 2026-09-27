---
title: CI
description: Welche Jobs die CI bei jedem Push und jeder PR laufen lässt, mit welchen Adaptern die GPU-Tests laufen und wie die Doku-Prüfung Verweise, Links und code:-Pfade prüft.
code:
  - .github/workflows/ci.yml
  - .github/pruefe-doku.sh
  - renderer/deny.toml
---

# CI

Die CI in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) läuft
bei jedem Push auf `master` und bei jeder PR: Rust unter Ubuntu und
Windows, das Frontend, die Lizenzen der Abhängigkeiten, die Coverage und die
Doku. Ein fehlender GPU-Adapter ist dort ein Fehler, kein übergangener Test.
Dieselben Befehle lokal: [Tests](tests.md), „Laufen lassen“.

## Jobs

| Job | Läuft auf | Was |
|---|---|---|
| Rust | Ubuntu und Windows | `cargo fmt --all --check` (nur Ubuntu), `cargo clippy --all-targets -- -D warnings`, `cargo nextest run --all-targets`, unter Ubuntu auch in Release |
| Frontend | Ubuntu | `npm run check`, `npm run lint`, der Build mit einer Attrappe unter `public/tiles`, der Smoke-Test mit Playwright |
| Dependencies | Ubuntu | `cargo deny check` mit [`renderer/deny.toml`](../../renderer/deny.toml) |
| Coverage | Ubuntu | `cargo llvm-cov --all-targets` |
| Doku | Ubuntu | `bash .github/pruefe-doku.sh`, dazu eine Probe, dass sie anschlägt |

Die Tests laufen in Release, weil sich die Überlauf-Semantik zwischen Debug
und Release unterscheidet; die Prüfungen im Regionsleser müssen in beiden
greifen. Fällt das Goldbild, liegt das Ist-Bild als Artefakt am Lauf, siehe
[Tests](tests.md), „Goldbild“.

## GPU-Tests in der CI

Die GPU-Tests brauchen einen Adapter. Auf Windows ist WARP dabei, auf
Ubuntu liefert Mesa mit lavapipe eine Vulkan-Implementierung in Software:
langsam, aber derselbe Shader-Weg wie auf einer Karte. Rust und Coverage
setzen `TERRANOVA_GPU_PFLICHT`; fehlt der Adapter, ist das ein Fehler.

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
- Jeder Pfad unter `code:` in der Frontmatter einer Seite existiert.
- Jede Seite in `docs/` steht in `docs/index.md`.

Jeder Fehler steht als Zeile `::error file=…::…` da, die GitHub an die
Datei heftet. Danach verbiegt die CI in einer Probe je einen Verweis, eine
Überschrift, einen Link und einen Pfad unter `code:` und verlangt genau
vier Meldungen; so fällt auf, wenn die Prüfung nichts mehr findet. Lokal aus der Wurzel des Repositorys:

```bash
bash .github/pruefe-doku.sh
```

Die Prüfung sieht nur, was Git verfolgt: eine neue Seite erst nach
`git add`. Der zweite Schritt des Jobs baut ein kleines Repository mit einem
kaputten Verweis, einem kaputten Link und einem kaputten Pfad unter `code:`
und verlangt, dass die Prüfung alle drei meldet.
