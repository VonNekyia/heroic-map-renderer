---
title: Eingabedaten
description: Welche Eingabedaten nicht im Repository liegen, woher sie kommen und was stattdessen für Tests mitkommt.
code:
  - .gitignore
  - renderer/tests/fixtures
  - web/public/tiles-demo
---

# Eingabedaten

`world/`, `assets/`, `vanilla-assets/` und `vanilla-data/` stehen in
[`.gitignore`](../../.gitignore), die Testwelt allein ist 2,7 GB. Sie werden
dem Renderer über `--world`, `--assets` und `--data` übergeben. Woher Assets
und Biomdaten kommen: [Assets und Biomdaten](../benutzung/assets.md).

## Was im Repository liegt

- Kleine Fixtures für die Tests unter `renderer/tests/fixtures/`: eine
  Region mit 2×2 Chunks, 4×4 Chunks mit dem Licht aus einem Lauf von
  Vanilla, ein synthetischer Assetbaum, Biome und das Goldbild, siehe
  [Tests](tests.md), „Fixtures“.
- Ein kleiner Kachelbaum unter `web/public/tiles-demo/`, Fixture des
  Smoke-Tests, siehe [Frontend](../frontend.md).
- Die aus dem Spiel erzeugten Tabellen unter `renderer/src/assets/`, siehe
  [Erzeugte Tabellen](tabellen.md).

## Bilder

Bilder in `docs/bilder/` zeigen nur die Testwelt oder Szenen, die ein
Test baut, siehe [`AGENTS.md`](../../AGENTS.md), Regel 20. Wie sie entstehen: Skill
[`doku-bilder-rendern`](../../skills/doku-bilder-rendern/SKILL.md).
