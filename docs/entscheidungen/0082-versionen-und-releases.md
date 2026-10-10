---
title: "0082: Versionen nach Cargo.toml, Releases als Entwurf"
description: Warum die Versionen SemVer aus Cargo.toml folgen, ein Tag vX.Y.Z den Release-Workflow startet, der nur einen Entwurf anlegt, und warum Linux gegen glibc 2.28 baut, ohne musl.
status: gilt
date: 2026-10-06
issues: [150]
code:
  - .github/workflows/release.yml
  - renderer/Cargo.toml
---

# 0082: Versionen nach Cargo.toml, Releases als Entwurf

## Anlass

Das Werkzeug wird auch als CLI veröffentlicht (#150). Dafür braucht es ein
Versionsschema und einen Weg vom Tag zum Release.

## Entscheidung

- **Version:** SemVer, wie `version` in `renderer/Cargo.toml`. Das Binär
  nennt sie mit `--version`. Ein Release-Tag heisst wie die Version mit `v`
  davor; der Workflow bricht ab, wenn beide nicht gleich sind.
- **Release:** Ein Tag `v*` startet `.github/workflows/release.yml`. Er baut
  beide Binärs, prüft sie, packt sie mit den Lizenzen und legt einen
  Entwurf an. Veröffentlicht wird der Entwurf von Hand.
- **Linux** baut gegen glibc 2.28 im Container `manylinux_2_28`, so läuft
  das Binär auch auf älteren Systemen. Kein musl: Dort ist `dlopen` ein
  Stummel, `libvulkan` lädt nicht, und es gäbe keine Grafikkarte.
- **Prüfen ohne Tag:** Ändert eine PR den Workflow, baut er die Pakete,
  ohne zu veröffentlichen.

## Verworfene Alternativen

- **Version aus `git describe`:** Ein Build ohne Git, etwa aus einem
  Quellarchiv, hätte keine. Die Version in `Cargo.toml` hat jeder Build.
- **Gleich veröffentlichen:** Ein Release geht an alle, die es abonniert
  haben. Ein Entwurf lässt die Pakete erst prüfen.
- **Den Build der CI nehmen:** `ubuntu-latest` hat eine neuere glibc; jedes
  Binär von dort verlangt mindestens glibc 2.34.
- **Ein zweites Binär mit musl:** Zwei Linux-Binärs passen nicht ins Jar
  des Plugins, siehe [Weitergabe](../entwicklung/weitergabe.md), „Grenze“.

## Folgen

- Vor einem Release wird `version` in `Cargo.toml` gehoben, im selben PR
  wie die Änderungen, die es trägt.
- Wer veröffentlicht, prüft den Entwurf: Pakete, Prüfsummen, Notizen.
- Die Notizen sagen, ob jeder Baum einen vollen Lauf braucht: ob sich seit
  dem letzten Tag `ZEICHENSTAND` oder eine Tabelle unter
  `renderer/src/assets/` geändert hat
  ([`voller-lauf.sh`](../../.github/voller-lauf.sh), seit
  [0098](0098-der-zeichenstand-statt-des-builds.md)).
- Alpine und andere Systeme mit musl brauchen ein Image mit glibc.
