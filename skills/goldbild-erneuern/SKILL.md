---
name: goldbild-erneuern
description: Erneuert die Goldbilder unter renderer/tests/fixtures/golden/ nach einer gewollten Änderung am Bild. Nutzen, wenn goldbild_bleibt_gleich fällt und die Änderung an Projektion, Baking, Rasterizer, Licht oder Maleralgorithmus beabsichtigt ist; sonst ist der Fehler zu beheben.
---

# Goldbild erneuern

Die Goldbilder halten fest, wie kleine Ausschnitte aussehen: in 2:1
(`metatile.png`), in 4:3 (`metatile-4x3.png`) und von oben
(`metatile-top.png`). Jede Änderung am Bild fällt damit auf. Siehe
[`docs/entwicklung/tests.md`](../../docs/entwicklung/tests.md), „Goldbild“.

## Ablauf

1. **Ist die Änderung gewollt?** Fällt `goldbild_bleibt_gleich`, liegt das
   Ist-Bild als `renderer/tests/fixtures/golden/<name>-ist.png` neben dem
   Goldbild, etwa `metatile-ist.png`, in der CI als Artefakt
   `goldbild-ist-<os>` am Lauf. Der Test bricht beim ersten abweichenden
   ab; nach dem Erneuern zeigt ein zweiter Lauf die übrigen. Beide Bilder
   nebeneinander ansehen. Gehört die Änderung nicht zur PR, ist es ein
   Fehler: beheben, nicht erneuern.
2. **Erneuern** in `renderer/`:

   ```bash
   UPDATE_GOLDEN=1 cargo test --test metatile
   ```

   Der Test schreibt dann alle Goldbilder neu und prüft nichts.
3. **Prüfen:** `cargo nextest run --all-targets` ohne `UPDATE_GOLDEN`; der
   Test muss grün sein.
4. **Einchecken:** nur die Goldbilder, die sich gewollt geändert haben;
   die `metatile*-ist.png` stehen in `.gitignore`. Die Commit-Nachricht sagt, was sich am Bild geändert hat
   und warum.
5. **Doku:** Ändert sich, was der Renderer zeichnet, die Seite des Themas
   nachziehen, Skill [`doku-pflegen`](../doku-pflegen/SKILL.md). Die Bilder
   in `docs/bilder/` erneuert der Skill
   [`doku-bilder-rendern`](../doku-bilder-rendern/SKILL.md), nicht dieser.
