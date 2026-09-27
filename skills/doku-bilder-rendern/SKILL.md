---
name: doku-bilder-rendern
description: Rendert die Bilder unter docs/bilder/ aus der Testwelt neu, samt Galerie und Banner des README, mit den Befehlen, aus denen sie stammen. Nutzen, wenn sich das Bild des Renderers sichtbar geändert hat oder ein Bild im README oder in docs/ veraltet ist.
---

# Doku-Bilder rendern

Bilder in `docs/bilder/` zeigen nur die Testwelt oder Szenen, die ein Test
baut, nie die grosse Welt ([`AGENTS.md`](../../AGENTS.md), Regel 20). Die
Testwelt liegt unter `./world`, die Assets wie in
[`docs/benutzung/assets.md`](../../docs/benutzung/assets.md).

## Ablauf

1. **Release-Build** des Stands, den die Bilder zeigen sollen.
2. **Rendern** aus der Wurzel des Repositorys, mit Python und Pillow:

   ```bash
   python skills/doku-bilder-rendern/bilder-rendern.py renderer/target/release/terranova-render
   ```

   | Bild | Herkunft |
   |---|---|
   | `map.png`, `map-wide.png` | ungeschnitten, Tabelle `DOKU` in [`bilder-rendern.py`](bilder-rendern.py) |
   | `welt.webp`, `dorf.webp`, `ufer.webp`, `eis.webp`, `savanne.webp` | zugeschnitten, verlustfrei, Tabelle `README` |
   | `banner.webp` | der Ausschnitt `BANNER` der Übersicht unter `quellen/banner-ebenen.png` |
   | `sprites.png` | der Befehl in [`docs/benutzung/schalter.md`](../../docs/benutzung/schalter.md), „Sprites rastern: `--sprite`“ |

   Liegen `world/` und die Assets nicht in der Wurzel, nennt ein zweites
   Argument ihren Ordner.
3. **Die Ebenen des Banners** liegen in
   [`docs/bilder/quellen/banner.aseprite`](../../docs/bilder/quellen/banner.aseprite):
   Verlauf, Schatten, Insel, Titel, Untertitel und Merkmale, ohne Karte als
   `banner-ebenen.png` exportiert. Die Insel ist eine Szene aus
   [`renderer/tests/logo_welt.rs`](../../renderer/tests/logo_welt.rs):

   ```bash
   LOGO_WELT=<ordner> cargo test --release --manifest-path renderer/Cargo.toml --test logo_welt -- --ignored
   cargo run --release --manifest-path renderer/Cargo.toml -- --world <ordner> --assets ./vanilla-assets --data ./vanilla-data --render insel.png --center 8 8 --size 576 --scale 36
   ```

   Wer Insel oder Schrift ändert, ändert die Ebenen in Aseprite und
   exportiert sie ohne Karte neu.
4. **Die übrigen Bilder** `kacheln.png`, `zoomstufen.png`,
   `zeichenreihenfolge.png` und `frontend.png` entstanden in den Schritten
   5, 6, 4 und 7; wie genau, ist nicht festgehalten. Wer eines neu macht,
   trägt den Befehl hier ein.
5. **Ansehen**, bevor es eingecheckt wird: Zeigt es, was der Text daneben
   sagt? Die Bildunterschriften in `docs/` und im README nennen Ausschnitt,
   scale und Stand; sie mit dem Bild nachziehen.
6. **Einchecken** mit einer Commit-Nachricht, die den Stand nennt.
   Unveränderter Stand, unverändertes Bild: Die Befehle sind
   reproduzierbar, ein erneuter Lauf ändert kein Byte.
