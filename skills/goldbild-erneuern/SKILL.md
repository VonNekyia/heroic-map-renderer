---
name: goldbild-erneuern
description: Erneuert das Goldbild unter renderer/tests/fixtures/golden/ nach einer gewollten Änderung am Bild. Nutzen, wenn goldbild_bleibt_gleich fällt und die Änderung an Projektion, Baking, Rasterizer, Licht oder Maleralgorithmus beabsichtigt ist; sonst ist der Fehler zu beheben.
---

# Goldbild erneuern

Das Goldbild hält fest, wie ein kleiner Ausschnitt aussieht; jede Änderung
am Bild fällt damit auf. Siehe
[`docs/entwicklung/tests.md`](../../docs/entwicklung/tests.md), „Goldbild“.

## Ablauf

1. **Ist die Änderung gewollt?** Fällt `goldbild_bleibt_gleich`, liegt das
   Ist-Bild als `renderer/tests/fixtures/golden/metatile-ist.png` neben dem
   Goldbild, in der CI als Artefakt `goldbild-ist-<os>` am Lauf. Beide Bilder
   nebeneinander ansehen. Gehört die Änderung nicht zur PR, ist es ein
   Fehler: beheben, nicht erneuern.
2. **Erneuern** in `renderer/`:

   ```bash
   UPDATE_GOLDEN=1 cargo test --test metatile
   ```

   Der Test schreibt dann das neue Goldbild und prüft nichts.
3. **Prüfen:** `cargo nextest run --all-targets` ohne `UPDATE_GOLDEN`; der
   Test muss grün sein.
4. **Einchecken:** nur `metatile.png`; `metatile-ist.png` steht in
   `.gitignore`. Die Commit-Nachricht sagt, was sich am Bild geändert hat
   und warum.
5. **Doku:** Ändert sich, was der Renderer zeichnet, die Seite des Themas
   nachziehen, Skill [`doku-pflegen`](../doku-pflegen/SKILL.md). Die Bilder
   in `docs/bilder/` erneuert der Skill
   [`doku-bilder-rendern`](../doku-bilder-rendern/SKILL.md), nicht dieser.
