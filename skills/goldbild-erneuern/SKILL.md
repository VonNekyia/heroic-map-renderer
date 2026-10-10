---
name: goldbild-erneuern
description: Erneuert die Goldbilder unter renderer/tests/fixtures/golden/ nach einer gewollten Änderung am Bild. Nutzen, wenn goldbild_bleibt_gleich fällt und die Änderung an Projektion, Baking, Rasterizer, Licht oder Maleralgorithmus beabsichtigt ist; sonst ist der Fehler zu beheben.
---

# Goldbild erneuern

Die Goldbilder halten fest, wie kleine Ausschnitte aussehen; jede
Änderung am Bild fällt damit auf. Welche es gibt, steht in
[`docs/entwicklung/tests.md`](../../docs/entwicklung/tests.md), „Goldbild“.

## Ablauf

1. **Ist die Änderung gewollt?** Fällt `goldbild_bleibt_gleich`, liegt das
   Ist-Bild jedes abweichenden als
   `renderer/tests/fixtures/golden/<name>-ist.png` neben seinem Goldbild,
   etwa `metatile-ist.png`, in der CI als Artefakt `goldbild-ist-<os>` am
   Lauf. Beide Bilder nebeneinander ansehen. Gehört die Änderung nicht zur
   PR, ist es ein Fehler: beheben, nicht erneuern.
2. **Erneuern** in `renderer/`:

   ```bash
   UPDATE_GOLDEN=1 cargo test --test metatile
   ```

   Der Test schreibt dann alle Goldbilder neu und prüft nichts.
3. **Zeichenstand:** Danach fällt `zeichenstand_folgt_den_goldbildern` in
   `renderer/src/render/stand.rs` und nennt den neuen Wert. Zeichnet der
   Renderer anders, `ZEICHENSTAND` dort um eins heben. In jedem Fall
   `GOLDBILDER` auf den genannten Wert setzen. Ändert sich nur die Szene
   eines Tests oder kommt ein Goldbild dazu, bleibt der Zeichenstand.
   Warum: [0098](../../docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md).
4. **Prüfen:** `cargo nextest run --all-targets` ohne `UPDATE_GOLDEN`; der
   Test muss grün sein.
5. **Einchecken:** nur die Goldbilder, die sich gewollt geändert haben;
   die `metatile*-ist.png` stehen in `.gitignore`. Die Commit-Nachricht sagt, was sich am Bild geändert hat
   und warum.
6. **Doku:** Ändert sich, was der Renderer zeichnet, die Seite des Themas
   nachziehen, Skill [`doku-pflegen`](../doku-pflegen/SKILL.md). Die Bilder
   in `docs/bilder/` erneuert der Skill
   [`doku-bilder-rendern`](../doku-bilder-rendern/SKILL.md), nicht dieser.
