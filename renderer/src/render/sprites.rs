use std::collections::HashMap;

use anyhow::Result;

use crate::assets::{Assets, bake};
use crate::world::BlockState;

use super::{Projection, Sprite, render};

/// Verweis in die Sprite-Tabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteId(u32);

/// Alle Sprites, die für einen Renderlauf gebraucht werden.
///
/// Gebaut wird die Tabelle einmal aus den Blockstates, die in der Welt
/// tatsächlich vorkommen. Danach ist sie unveränderlich: der Renderpfad
/// schlägt nur noch nach und kopiert Pixel, ohne Sperren und ohne
/// Dateizugriffe.
pub struct SpriteSet {
    sprites: Vec<Entry>,
    by_state: HashMap<BlockState, SpriteId>,
    projection: Projection,
}

struct Entry {
    sprite: Sprite,
    /// Deckt das Sprite den Blockumriss lückenlos ab? Nur dann darf es
    /// dahinterliegende Blöcke verdecken.
    opaque: bool,
    /// Bleibt das Sprite innerhalb des eigenen Blockumrisses? Nur dann
    /// können die drei Nachbarn es überhaupt vollständig verdecken.
    contained: bool,
}

impl SpriteSet {
    /// Backt und rastert jede Blockstate genau einmal.
    ///
    /// Blockstates ohne sichtbare Geometrie — Luft, Truhen, Deckenfeuer —
    /// landen nicht in der Tabelle und werden beim Rendern übersprungen.
    pub fn build<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        let mut set = SpriteSet {
            sprites: Vec::new(),
            by_state: HashMap::new(),
            projection,
        };

        for state in states {
            if state.is_air() || set.by_state.contains_key(state) {
                continue;
            }
            let variants = assets.variants(state)?;
            let Some(sprite) = render(&bake(&variants), assets.textures(), &projection) else {
                continue;
            };
            let opaque = covers_block(&sprite, projection);
            let contained = fits_block(&sprite, projection);
            set.by_state
                .insert(state.clone(), SpriteId(set.sprites.len() as u32));
            set.sprites.push(Entry {
                sprite,
                opaque,
                contained,
            });
        }

        Ok(set)
    }

    pub fn id(&self, state: &BlockState) -> Option<SpriteId> {
        self.by_state.get(state).copied()
    }

    pub fn sprite(&self, id: SpriteId) -> &Sprite {
        &self.sprites[id.0 as usize].sprite
    }

    pub fn is_opaque(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].opaque
    }

    /// Liegt das ganze Sprite im Umriss seines eigenen Blocks?
    pub fn is_contained(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].contained
    }

    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }

    pub fn projection(&self) -> Projection {
        self.projection
    }
}

/// Der Umriss eines vollen Blocks ist ein Sechseck mit den Ecken
/// `(0, -s/2)`, `(s/2, -s/4)`, `(s/2, s/4)`, `(0, s/2)`, `(-s/2, s/4)`,
/// `(-s/2, -s/4)`; die vier schrägen Kanten haben die Steigung plus/minus
/// ein halb. `slack` dehnt das Sechseck nach aussen, negative Werte
/// schrumpfen es.
///
/// `px`, `py` sind Pixelmittelpunkte relativ zum Blockmittelpunkt.
fn in_outline(px: f32, py: f32, half: f32, slack: f32) -> bool {
    let limit = half + slack;
    px.abs() <= limit && (py + px / 2.0).abs() <= limit && (py - px / 2.0).abs() <= limit
}

/// Pixelmittelpunkt relativ zum Blockmittelpunkt.
fn pixel_center(sprite: &Sprite, x: u32, y: u32) -> (f32, f32) {
    (
        sprite.offset.0 as f32 + x as f32 + 0.5,
        sprite.offset.1 as f32 + y as f32 + 0.5,
    )
}

/// Prüft, ob ein Sprite den Umriss eines vollen Blocks lückenlos und
/// undurchsichtig ausfüllt.
///
/// Das ist die Bedingung dafür, dass der Block etwas dahinter verdecken
/// darf. Geprüft wird am fertigen Bild statt am Modell: ein Würfel mit
/// durchsichtiger Textur wie Glas fällt so von selbst heraus.
fn covers_block(sprite: &Sprite, projection: Projection) -> bool {
    let scale = projection.scale();
    let half = scale as i32 / 2;
    if sprite.image.dimensions() != (scale, scale) || sprite.offset != (-half, -half) {
        return false;
    }

    // Eine Pixelbreite Rand bleibt aussen vor: die Überabtastung lässt
    // genau dort Alpha unter 255 zurück, und eine Blockkante um ein Pixel
    // durchscheinen zu lassen ist harmlos.
    let mut geprueft = 0u32;
    for (x, y, pixel) in sprite.image.enumerate_pixels() {
        let (px, py) = pixel_center(sprite, x, y);
        if !in_outline(px, py, half as f32, -1.0) {
            continue;
        }
        if pixel.0[3] < 255 {
            return false;
        }
        geprueft += 1;
    }

    // Unter scale 4 schrumpft das Sechseck auf nichts zusammen: kein
    // Pixelmittelpunkt liegt mehr darin, und die Schleife oben würde
    // wortlos "deckend" melden. Eine leere Prüfmenge beweist nichts.
    geprueft > 0
}

/// Prüft, ob ein Sprite ganz im Umriss seines eigenen Blocks bleibt.
///
/// Nur dann verdecken drei deckende Nachbarn es vollständig — ihre
/// Umrisse setzen genau den eigenen zusammen, mehr nicht. Ein Sprite, das
/// darüber hinausragt (Feuer, Zäune, Modelle mit negativem `from`), bleibt
/// sichtbar und darf nicht übersprungen werden.
///
/// Die eine Pixelbreite Toleranz entspricht der von `covers_block`: an der
/// Sechseckkante lässt die Überabtastung ohnehin Teilalpha zurück.
fn fits_block(sprite: &Sprite, projection: Projection) -> bool {
    let half = projection.scale() as f32 / 2.0;
    sprite
        .image
        .enumerate_pixels()
        .filter(|(_, _, p)| p.0[3] > 0)
        .all(|(x, y, _)| {
            let (px, py) = pixel_center(sprite, x, y);
            in_outline(px, py, half, 1.0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets() -> Assets {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        Assets::open(vec![base]).unwrap()
    }

    fn state(text: &str) -> BlockState {
        BlockState::parse(text).unwrap()
    }

    #[test]
    fn luft_kommt_nicht_in_die_tabelle() {
        let mut assets = assets();
        let states = [state("minecraft:air"), state("einfarbig")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.len(), 1);
        assert!(set.id(&state("minecraft:air")).is_none());
        assert!(set.id(&state("einfarbig")).is_some());
    }

    #[test]
    fn jede_blockstate_nur_einmal() {
        let mut assets = assets();
        let states = [state("einfarbig"), state("einfarbig"), state("stone")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn voller_wuerfel_gilt_als_deckend() {
        let mut assets = assets();
        let states = [state("einfarbig")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("einfarbig")).unwrap();
        assert!(set.is_opaque(id), "ein voller Würfel deckt ab");
    }

    /// Ein flaches Seerosenblatt füllt den Blockumriss nicht aus und darf
    /// deshalb nichts verdecken.
    #[test]
    fn flaches_modell_deckt_nicht_ab() {
        let mut assets = assets();
        let states = [state("seerose")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("seerose")).unwrap();
        assert!(!set.is_opaque(id));
    }

    /// Ein Würfel mit vollständig durchsichtiger Textur darf nie als
    /// deckend gelten — auch nicht bei der kleinsten erlaubten
    /// Skalierung, wo das Blocksechseck keinen Pixelmittelpunkt mehr
    /// enthält und die Prüfschleife leer durchläuft.
    #[test]
    fn durchsichtiger_wuerfel_deckt_nie_ab() {
        for scale in [2, 3, 4, 16, 64] {
            let mut assets = assets();
            let states = [state("durchsichtig")];
            let set = SpriteSet::build(&mut assets, &states, Projection::new(scale)).unwrap();
            let id = set.id(&state("durchsichtig")).unwrap();
            assert!(!set.is_opaque(id), "bei scale {scale}");
        }
    }

    /// Gegenprobe: ein voller Würfel deckt bei jeder brauchbaren
    /// Skalierung ab. Unter scale 4 verzichtet die Prüfung bewusst
    /// darauf, weil ihr die Auflösung fehlt.
    #[test]
    fn voller_wuerfel_deckt_ab_sobald_die_aufloesung_reicht() {
        for scale in [4, 16, 64] {
            let mut assets = assets();
            let states = [state("einfarbig")];
            let set = SpriteSet::build(&mut assets, &states, Projection::new(scale)).unwrap();
            let id = set.id(&state("einfarbig")).unwrap();
            assert!(set.is_opaque(id), "bei scale {scale}");
        }
    }

    #[test]
    fn voller_wuerfel_bleibt_im_blockumriss() {
        let mut assets = assets();
        let states = [state("einfarbig"), state("seerose"), state("oak_fence")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        for name in ["einfarbig", "seerose", "oak_fence"] {
            let id = set.id(&state(name)).unwrap();
            assert!(set.is_contained(id), "{name} ragt nicht heraus");
        }
    }

    /// Ein Modell mit negativem `from` ragt seitlich aus dem Block heraus.
    /// Drei deckende Nachbarn verdecken es deshalb nicht.
    #[test]
    fn ueberhaengendes_modell_verlaesst_den_blockumriss() {
        let mut assets = assets();
        let states = [state("ueberhang")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("ueberhang")).unwrap();
        assert!(!set.is_contained(id));
    }

    /// Blöcke ohne sichtbare Geometrie tauchen gar nicht erst auf.
    #[test]
    fn modell_ohne_flaechen_faellt_heraus() {
        let mut assets = assets();
        let states = [state("chest")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        assert!(set.is_empty());
    }
}
