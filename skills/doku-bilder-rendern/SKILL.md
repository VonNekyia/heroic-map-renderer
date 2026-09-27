---
name: doku-bilder-rendern
description: Rendert die Bilder unter docs/bilder/ aus der Testwelt neu, mit den Befehlen, aus denen sie stammen. Nutzen, wenn sich das Bild des Renderers sichtbar geändert hat oder ein Bild im README oder in docs/ veraltet ist.
---

# Doku-Bilder rendern

Bilder in `docs/bilder/` zeigen nur die Testwelt, nie die grosse Welt
([`AGENTS.md`](../../AGENTS.md), Regel 20). Die Testwelt liegt unter
`./world`, die Assets wie in
[`docs/benutzung/assets.md`](../../docs/benutzung/assets.md).

## Ablauf

1. **Release-Build** des Stands, den das Bild zeigen soll.
2. **Rendern** aus der Wurzel des Repositorys:

   | Bild | Befehl |
   |---|---|
   | `sprites.png` | der Befehl in [`docs/benutzung/schalter.md`](../../docs/benutzung/schalter.md), „Sprites rastern: `--sprite`“ |
   | `map.png` | `--render docs/bilder/map.png --center -64 416 --size 900 --scale 16`, mit `--world ./world`, beiden `--assets` und `--data ./vanilla-data` |
   | `map-wide.png` | wie `map.png`, mit `--center 0 0 --scale 4` |

   ```bash
   cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render docs/bilder/map.png --center -64 416 --size 900 --scale 16
   cargo run --release --manifest-path renderer/Cargo.toml -- --world ./world --assets ./vanilla-assets --assets ./assets --data ./vanilla-data --render docs/bilder/map-wide.png --center 0 0 --size 900 --scale 4
   ```

3. **Die übrigen Bilder** `kacheln.png`, `zoomstufen.png`,
   `zeichenreihenfolge.png` und `frontend.png` entstanden in den Schritten
   5, 6, 4 und 7; wie genau, ist nicht festgehalten. Wer eines neu macht,
   trägt den Befehl hier ein.
4. **Ansehen**, bevor es eingecheckt wird: Zeigt es, was der Text daneben
   sagt? Die Bildunterschriften in `docs/` und im README nennen Ausschnitt,
   scale und Stand; sie mit dem Bild nachziehen.
5. **Einchecken** mit einer Commit-Nachricht, die den Stand nennt.
   Unveränderter Stand, unverändertes Bild: Die Befehle sind
   reproduzierbar, ein erneuter Lauf ändert kein Byte.
