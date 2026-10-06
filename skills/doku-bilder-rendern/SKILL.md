---
name: doku-bilder-rendern
description: Rendert die Bilder unter docs/bilder/ aus der Testwelt neu, samt Galerie und Banner des README und Vorschaubild und Icons der Karte, mit den Befehlen, aus denen sie stammen. Nutzen, wenn sich das Bild des Renderers sichtbar geändert hat oder ein Bild im README, in docs/ oder unter web/public veraltet ist.
---

# Doku-Bilder rendern

Bilder in `docs/bilder/` zeigen nur die Testwelt oder Szenen, die ein Test
baut, nie die grosse Welt ([`AGENTS.md`](../../AGENTS.md), Regel 20). Die
Testwelt liegt unter `./world`, die Assets wie in
[`docs/benutzung/assets.md`](../../docs/benutzung/assets.md).

## Ablauf

1. **Release-Build** des Stands, den die Bilder zeigen sollen. Die
   Bilder zu Cinematic baut das Skript selbst per `cargo test` aus dem
   Checkout, nicht aus dem übergebenen Binär: Der Checkout muss also der
   Stand sein, den sie zeigen sollen.
2. **Rendern** aus der Wurzel des Repositorys, mit Python und Pillow:

   ```bash
   python skills/doku-bilder-rendern/bilder-rendern.py renderer/target/release/heroic-map-renderer
   ```

   | Bild | Herkunft |
   |---|---|
   | `map.png`, `map-wide.png` | ungeschnitten, Tabelle `DOKU` in [`bilder-rendern.py`](bilder-rendern.py) |
   | `welt.webp`, `dorf.webp`, `ufer.webp`, `eis.webp`, `savanne.webp` | zugeschnitten, verlustfrei, Tabelle `README` |
   | `banner.webp` | der Ausschnitt `BANNER` der Übersicht unter `quellen/banner-ebenen.png` |
   | `biomgrenze-savanne.webp`, `biomgrenze-ozean.webp` | je zweimal gerendert, links `--biome-blend 0`, rechts `2`, zugeschnitten und nebeneinander, Tabelle `GRENZEN` |
   | `kameras.webp` | dasselbe Dorf in 2:1, 4:3, 1:1 und `top`, zwei mal zwei, je mit `--camera` und einem `--center`, das den Punkt `ZIEL` in die Mitte legt, `KAMERAS` mit `ZIEL` und `FELD` |
   | `genordet.webp` | dasselbe Dorf in `top-north` und `north-45` nebeneinander, ebenso, `GENORDET` |
   | `karte-cinematic.webp` | das Dorf aus dem README zweimal gerendert, links die Karte, rechts mit `--cinematic`, zugeschnitten und nebeneinander, `KINO`; Cinematic hier mit den Werten, die der Schalter setzt |
   | `web/public/vorschau.jpg`, `favicon.png`, `apple-touch-icon.png` | [`web-bilder.py`](web-bilder.py): die Mitte von `welt.webp` auf 1200 × 630, die Icons aus der Ebene „Insel“ des Banners, siehe Schritt 3 |
   | `marmor.webp` | der Skin Tablett, vorerst nur Marmor, an einem Kachelbaum der Testwelt in 2:1 mit `--scale 8`: in `web/` erst `SKIN=./skins/tablett npx vite build --outDir dist-skin`, dann `node ../skills/doku-bilder-rendern/marmor-bild.mjs <kachelwurzel> ../docs/bilder/marmor.webp` ([`marmor-bild.mjs`](marmor-bild.mjs)) |
   | `sprites.png` | der Befehl in [`docs/benutzung/schalter.md`](../../docs/benutzung/schalter.md), „Sprites rastern: `--sprite`“ |
   | `cinematic-renderer-waerme.webp`, `cinematic-renderer-pflanzen.webp` | Cinematic mit Werten des Looks, die kein Schalter bietet: Das Skript ruft den ignorierten Test `bilder_zu_cinematic` in [`renderer/tests/kennzahlen.rs`](../../renderer/tests/kennzahlen.rs), dort stehen Szenen, Ausschnitte und Werte. Gebaut aus dem Checkout mit `cargo test`, nicht aus dem übergebenen Binär |
   | `cinematic-waerme.webp`, `cinematic-pflanzen.webp` | nicht neu: die Bilder des Prototyps zu #89, an denen 0058 entschieden ist, Quelle in [`quellen/cinematic-look.json`](../../docs/bilder/quellen/cinematic-look.json); sie bleiben wie die Entscheidung (Regel 9) |

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
   exportiert sie ohne Karte neu. Danach und nach einem neuen `welt.webp`
   die Bilder der Karte unter `web/public`:

   ```bash
   aseprite -b --layer Insel docs/bilder/quellen/banner.aseprite --save-as insel.png
   python skills/doku-bilder-rendern/web-bilder.py insel.png
   ```
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
