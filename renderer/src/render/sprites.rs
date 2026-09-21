use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use image::RgbaImage;

use crate::assets::baker::BakedModel;
use crate::assets::{Assets, bake};
use crate::world::BlockState;

use super::{Projection, Sprite, render};

/// Verweis in die Sprite-Tabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteId(u32);

/// Ein Blockwuerfel, relativ zu dem Block, dem das Modell gehoert.
pub type Cell = [i32; 3];

/// Der Wuerfel des Blocks selbst.
pub const OWN_CELL: Cell = [0, 0, 0];

/// Obergrenze fuer die Wuerfel, die ein Modell belegen darf. Ein kaputtes
/// Modell soll hier nicht in eine Schleife ueber Millionen Zellen laufen.
const MAX_CELLS: usize = 64;

/// Alle Sprites, die fuer einen Renderlauf gebraucht werden.
///
/// Gebaut wird die Tabelle einmal aus den Blockstates, die in der Welt
/// tatsaechlich vorkommen. Danach ist sie unveraenderlich: der Renderpfad
/// schlaegt nur noch nach und kopiert Pixel, ohne Sperren und ohne
/// Dateizugriffe.
pub struct SpriteSet {
    sprites: Vec<Entry>,
    by_state: HashMap<BlockState, SpriteId>,
    projection: Projection,
    foreign: BTreeSet<Cell>,
}

struct Entry {
    /// Das Sprite, zerlegt nach den Wuerfeln, in denen seine Geometrie
    /// liegt. Fast immer genau ein Teil in `OWN_CELL`.
    parts: Vec<(Cell, Sprite)>,
    /// Deckt der eigene Teil den Blockumriss lueckenlos ab? Nur dann darf
    /// der Block etwas dahinter verdecken.
    opaque: bool,
    /// Bleibt jeder Teil im Umriss seines eigenen Wuerfels? Nach der
    /// Zerlegung ist das der Normalfall; schlaegt sie fehl, verzichtet der
    /// Renderer auf die Verdeckungsabkuerzung.
    contained: bool,
}

impl SpriteSet {
    /// Backt und rastert jede Blockstate genau einmal.
    ///
    /// Blockstates ohne sichtbare Geometrie — Luft, Truhen, Deckenfeuer —
    /// landen nicht in der Tabelle und werden beim Rendern uebersprungen.
    pub fn build<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        let mut set = SpriteSet {
            sprites: Vec::new(),
            by_state: HashMap::new(),
            projection,
            foreign: BTreeSet::new(),
        };

        for state in states {
            if state.is_air() || set.by_state.contains_key(state) {
                continue;
            }
            let variants = assets.variants(state)?;
            let model = bake(&variants);
            let Some(sprite) = render(&model, assets.textures(), &projection) else {
                continue;
            };

            let parts = split(sprite, &model, projection);
            let opaque = parts
                .iter()
                .find(|(cell, _)| *cell == OWN_CELL)
                .is_some_and(|(_, sprite)| covers_cell(sprite, projection));
            let contained = parts
                .iter()
                .all(|(cell, sprite)| fits_cell(sprite, *cell, projection));

            set.foreign.extend(
                parts
                    .iter()
                    .map(|(cell, _)| *cell)
                    .filter(|cell| *cell != OWN_CELL),
            );
            set.by_state
                .insert(state.clone(), SpriteId(set.sprites.len() as u32));
            set.sprites.push(Entry {
                parts,
                opaque,
                contained,
            });
        }

        Ok(set)
    }

    pub fn id(&self, state: &BlockState) -> Option<SpriteId> {
        self.by_state.get(state).copied()
    }

    /// Der Teil dieses Sprites, der in `cell` liegt.
    ///
    /// Die Pixelposition bleibt relativ zu dem Block, dem das Modell
    /// gehoert — gezeichnet wird also weiterhin dort, nur zu dem
    /// Zeitpunkt, der zu `cell` gehoert.
    pub fn part(&self, id: SpriteId, cell: Cell) -> Option<&Sprite> {
        self.sprites[id.0 as usize]
            .parts
            .iter()
            .find(|(c, _)| *c == cell)
            .map(|(_, sprite)| sprite)
    }

    pub fn is_opaque(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].opaque
    }

    /// Bleibt jeder Teil in seinem Wuerfel? Nur dann duerfen drei deckende
    /// Nachbarn das Sprite ueberspringen.
    pub fn is_contained(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].contained
    }

    /// Alle Wuerfel ausser dem eigenen, in denen irgendein Sprite Teile
    /// hat.
    ///
    /// Leer, solange kein Modell seinen Blockwuerfel verlaesst — und dann
    /// kostet die Suche danach im Renderpfad nichts.
    pub fn foreign_cells(&self) -> &BTreeSet<Cell> {
        &self.foreign
    }

    /// Wie viele Sprites ihren eigenen Blockwürfel verlassen.
    pub fn overhanging(&self) -> usize {
        self.sprites.iter().filter(|e| e.parts.len() > 1).count()
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
/// `(-s/2, -s/4)`; die vier schraegen Kanten haben die Steigung plus/minus
/// ein halb. `slack` dehnt das Sechseck nach aussen, negative Werte
/// schrumpfen es.
///
/// `px`, `py` sind Pixelmittelpunkte relativ zum Mittelpunkt des Umrisses.
fn in_outline(px: f32, py: f32, half: f32, slack: f32) -> bool {
    let limit = half + slack;
    px.abs() <= limit && (py + px / 2.0).abs() <= limit && (py - px / 2.0).abs() <= limit
}

/// Pixelmittelpunkt relativ zum Blockursprung.
fn pixel_center(sprite: &Sprite, x: u32, y: u32) -> (f32, f32) {
    (
        sprite.offset.0 as f32 + x as f32 + 0.5,
        sprite.offset.1 as f32 + y as f32 + 0.5,
    )
}

/// Bildschirmmittelpunkt eines Wuerfels, relativ zum Blockursprung.
fn cell_center(cell: Cell, projection: Projection) -> (f32, f32) {
    let (x, y) = projection.project_block(cell);
    (x as f32, y as f32)
}

/// Prueft, ob ein Sprite den Umriss eines vollen Blocks lueckenlos und
/// undurchsichtig ausfuellt.
///
/// Das ist die Bedingung dafuer, dass der Block etwas dahinter verdecken
/// darf. Geprueft wird am fertigen Bild statt am Modell: ein Wuerfel mit
/// durchsichtiger Textur wie Glas faellt so von selbst heraus.
fn covers_cell(sprite: &Sprite, projection: Projection) -> bool {
    let scale = projection.scale();
    let half = scale as i32 / 2;
    if sprite.image.dimensions() != (scale, scale) || sprite.offset != (-half, -half) {
        return false;
    }

    // Eine Pixelbreite Rand bleibt aussen vor: die Ueberabtastung laesst
    // genau dort Alpha unter 255 zurueck, und eine Blockkante um ein Pixel
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
    // Pixelmittelpunkt liegt mehr darin, und die Schleife oben wuerde
    // wortlos "deckend" melden. Eine leere Pruefmenge beweist nichts.
    geprueft > 0
}

/// Prueft, ob ein Sprite ganz im Umriss eines Wuerfels bleibt.
///
/// Die eine Pixelbreite Toleranz entspricht der von `covers_cell`: an der
/// Sechseckkante laesst die Ueberabtastung ohnehin Teilalpha zurueck.
fn fits_cell(sprite: &Sprite, cell: Cell, projection: Projection) -> bool {
    let half = projection.scale() as f32 / 2.0;
    let (cx, cy) = cell_center(cell, projection);
    sprite
        .image
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel.0[3] > 0)
        .all(|(x, y, _)| {
            let (px, py) = pixel_center(sprite, x, y);
            in_outline(px - cx, py - cy, half, 1.0)
        })
}

/// Zerlegt ein Sprite in die Blockwuerfel, in denen seine Geometrie liegt.
///
/// Der Maleralgorithmus sortiert nach Wuerfeln. Ein Modell, das ueber
/// seinen eigenen Wuerfel hinausragt, muss deshalb zerfallen — sonst wird
/// der herausragende Teil zur falschen Zeit gezeichnet: ein Block, der vor
/// ihm liegt, aber einen hoeheren Ursprung hat, kaeme spaeter und
/// uebermalte ihn.
///
/// Zugeordnet wird ueber den Bildschirm. Die Umrisse benachbarter Wuerfel
/// kacheln die Ebene lueckenlos, ein Pixel liegt also in genau einem — bis
/// auf die Blickachse: Wuerfel, die sich um ein Vielfaches von (1, 1, 1)
/// unterscheiden, fallen aufeinander. Dort gewinnt der vordere, und genau
/// dessen Geometrie hat auch der Tiefenpuffer des Rasterizers stehen
/// lassen.
fn split(sprite: Sprite, model: &BakedModel, projection: Projection) -> Vec<(Cell, Sprite)> {
    if fits_cell(&sprite, OWN_CELL, projection) {
        return vec![(OWN_CELL, sprite)];
    }
    let cells = cells_of(model, projection);
    if cells.len() < 2 {
        return vec![(OWN_CELL, sprite)];
    }

    let half = projection.scale() as f32 / 2.0;
    let breite = sprite.image.width();
    let mut owner = vec![usize::MAX; (breite * sprite.image.height()) as usize];
    for (x, y, pixel) in sprite.image.enumerate_pixels() {
        if pixel.0[3] == 0 {
            continue;
        }
        let (px, py) = pixel_center(&sprite, x, y);
        // Vorderste Zelle zuerst: an den Umrisskanten gewinnt sie.
        owner[(y * breite + x) as usize] = cells
            .iter()
            .position(|&(_, cx, cy)| in_outline(px - cx, py - cy, half, 1.0))
            .unwrap_or(0);
    }

    cells
        .iter()
        .enumerate()
        .filter_map(|(index, &(cell, _, _))| Some((cell, extract(&sprite, &owner, index)?)))
        .collect()
}

/// Schneidet die einem Wuerfel zugeordneten Pixel als eigenes Sprite
/// heraus.
fn extract(sprite: &Sprite, owner: &[usize], index: usize) -> Option<Sprite> {
    let breite = sprite.image.width();
    let gehoert = |x: u32, y: u32| owner[(y * breite + x) as usize] == index;

    let mut umriss: Option<(u32, u32, u32, u32)> = None;
    for (x, y, _) in sprite.image.enumerate_pixels() {
        if !gehoert(x, y) {
            continue;
        }
        umriss = Some(match umriss {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    let (x0, y0, x1, y1) = umriss?;

    let mut image = RgbaImage::new(x1 - x0 + 1, y1 - y0 + 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if gehoert(x, y) {
                image.put_pixel(x - x0, y - y0, *sprite.image.get_pixel(x, y));
            }
        }
    }
    Some(Sprite {
        image,
        offset: (sprite.offset.0 + x0 as i32, sprite.offset.1 + y0 as i32),
    })
}

/// Die Wuerfel, die das Modell beruehrt — je Bildschirmposition einer, und
/// zwar der vorderste. Sortiert von vorne nach hinten.
fn cells_of(model: &BakedModel, projection: Projection) -> Vec<(Cell, f32, f32)> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for quad in &model.quads {
        for corner in &quad.corners {
            for achse in 0..3 {
                min[achse] = min[achse].min(corner[achse]);
                max[achse] = max[achse].max(corner[achse]);
            }
        }
    }

    let bereich = |achse: usize| {
        let lo = min[achse].floor() as i32;
        let hi = (max[achse].ceil() as i32 - 1).max(lo);
        lo..=hi
    };
    let anzahl = bereich(0).count() * bereich(1).count() * bereich(2).count();
    if anzahl == 0 || anzahl > MAX_CELLS {
        return Vec::new();
    }

    // Wuerfel entlang der Blickachse landen auf derselben Bildschirmstelle.
    // Von denen kann nur der vorderste sichtbar sein.
    let mut vorderste: HashMap<(i32, i32), Cell> = HashMap::new();
    for dx in bereich(0) {
        for dy in bereich(1) {
            for dz in bereich(2) {
                let stelle = (dx - dz, dx + dz - 2 * dy);
                let eintrag = vorderste.entry(stelle).or_insert([dx, dy, dz]);
                if dx + dy + dz > eintrag[0] + eintrag[1] + eintrag[2] {
                    *eintrag = [dx, dy, dz];
                }
            }
        }
    }

    let mut cells: Vec<Cell> = vorderste.into_values().collect();
    cells.sort_by_key(|cell| (-(cell[0] + cell[1] + cell[2]), *cell));
    cells
        .into_iter()
        .map(|cell| {
            let (cx, cy) = cell_center(cell, projection);
            (cell, cx, cy)
        })
        .collect()
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

    /// Modelle, die in ihrem Würfel bleiben, ergeben genau einen Teil —
    /// und damit kostet die Suche nach Überhängen im Renderpfad nichts.
    #[test]
    fn gewoehnliche_modelle_haben_nur_den_eigenen_wuerfel() {
        let mut assets = assets();
        let states = [state("einfarbig"), state("seerose"), state("oak_fence")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        for name in ["einfarbig", "seerose", "oak_fence"] {
            let id = set.id(&state(name)).unwrap();
            assert!(set.part(id, OWN_CELL).is_some(), "{name}");
            assert!(set.is_contained(id), "{name}");
        }
        assert!(set.foreign_cells().is_empty());
    }

    /// Ein Modell mit negativem `from` ragt seitlich aus dem Block heraus
    /// und muss in zwei Teile zerfallen, einen je Würfel.
    #[test]
    fn ueberhaengendes_modell_zerfaellt_in_wuerfel() {
        let mut assets = assets();
        let states = [state("ueberhang")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("ueberhang")).unwrap();

        assert!(set.part(id, OWN_CELL).is_some(), "eigener Würfel");
        assert!(set.part(id, [-1, 0, 0]).is_some(), "Würfel westlich davon");
        assert_eq!(
            set.foreign_cells().iter().copied().collect::<Vec<_>>(),
            vec![[-1, 0, 0]]
        );
        assert!(
            set.is_contained(id),
            "nach der Zerlegung bleibt jeder Teil drin"
        );
    }

    /// Ein zwei Blöcke hohes Modell ebenso — der obere Teil gehört in den
    /// Würfel darüber, sonst wird er zu früh gezeichnet.
    #[test]
    fn hohes_modell_zerfaellt_nach_oben() {
        let mut assets = assets();
        let states = [state("turm")];
        let set = SpriteSet::build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("turm")).unwrap();

        assert!(set.part(id, OWN_CELL).is_some());
        assert!(set.part(id, [0, 1, 0]).is_some());
        assert!(set.is_contained(id));
    }

    /// Die Zerlegung ist eine Aufteilung: kein Pixel darf verloren gehen
    /// und keines doppelt vergeben werden.
    #[test]
    fn zerlegung_erhaelt_jedes_pixel() {
        let projection = Projection::new(16);
        for name in ["turm", "ueberhang", "einfarbig", "seerose", "oak_fence"] {
            let mut assets = assets();
            let variants = assets.variants(&state(name)).unwrap();
            let model = bake(&variants);
            let ganz = render(&model, assets.textures(), &projection).unwrap();

            let sichtbar = |sprite: &Sprite| {
                let offset = sprite.offset;
                sprite
                    .image
                    .enumerate_pixels()
                    .filter(|(_, _, p)| p.0[3] > 0)
                    .map(|(x, y, p)| ((x as i32 + offset.0, y as i32 + offset.1), *p))
                    .collect::<Vec<_>>()
            };

            let mut vorher = sichtbar(&ganz);
            let teile = split(ganz, &model, projection);
            let mut nachher: Vec<_> = teile.iter().flat_map(|(_, s)| sichtbar(s)).collect();

            vorher.sort_by_key(|(pos, _)| *pos);
            nachher.sort_by_key(|(pos, _)| *pos);
            assert_eq!(vorher, nachher, "{name}");
        }
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
