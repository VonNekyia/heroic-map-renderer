use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use image::RgbaImage;

use crate::assets::baker::{BakedModel, Quad, box_quads};
use crate::assets::fluid::Fluid;
use crate::assets::{Assets, Face, Tints, fluid, models_of};
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
    /// Je Blockstate ihre Alternativen. Welche ein Block bekommt, wuerfelt
    /// seine Position, wie in Vanilla. Der Renderpfad haelt sich den Index
    /// je Paletteneintrag und spart sich das Hashen der Blockstate je Block.
    families: Vec<Family>,
    by_state: HashMap<BlockState, u32>,
    /// Fassungen einer Fluessigkeit: je Maske aus verdeckten Flaechen
    /// (`mask_bit`) eine, und fuer Masken mit Oberflaeche je Tiefe darunter
    /// eine — Index `mask + 8 * tiefe`. Eintrag 0 ist das Sprite selbst.
    by_mask: HashMap<SpriteId, Vec<Option<SpriteId>>>,
    /// Fassungen je Biom, nur fuer Sprites mit gefaerbten Flaechen. Das
    /// Sprite fuehrt zur Fassung des Standardklimas, von dort geht es
    /// ueber den Biomnamen weiter.
    by_biome: HashMap<SpriteId, HashMap<String, SpriteId>>,
    projection: Projection,
    foreign: BTreeSet<Cell>,
}

/// Die Alternativen einer Blockstate mit ihren Gewichten.
pub struct Family {
    alternatives: Vec<(u32, Option<SpriteId>)>,
    total: u32,
    /// Fluessigkeit samt Hoehe ihrer Oberflaeche in Blockeinheiten, falls
    /// die Blockstate eine enthaelt.
    pub fluid: Option<(Fluid, f32)>,
    /// Decken alle Alternativen den Blockumriss? Dann verdeckt der Block
    /// seine Nachbarn — egal, welche Drehung die Position wuerfelt.
    pub opaque: bool,
    /// Nur Fluessigkeit, keine eigene Geometrie: Wasser, Lava,
    /// Blasensaeule. Nur solche Bloecke zaehlen als Schicht hinter einer
    /// Oberflaeche; Kelp oder ein gefluteter Zaun sind etwas, das der
    /// Blickstrahl trifft.
    pub bare: bool,
}

impl Family {
    /// Die Alternative fuer einen Block — dieselbe, die der 26.2-Client
    /// wuerfelt: `ModelBlockRenderer` saet seinen Zufallsgenerator mit
    /// `Mth.getSeed` der Position, `WeightedList.getRandomOrThrow` zieht
    /// daraus `nextInt(total)` und zaehlt die Gewichte in Listenreihenfolge
    /// ab. Damit sieht die Karte aus wie das Spiel, und die Wahl haengt
    /// weder von Kachelgrenzen noch von der Renderreihenfolge ab.
    pub fn pick(&self, pos: [i32; 3]) -> Option<SpriteId> {
        if self.alternatives.len() == 1 {
            return self.alternatives[0].1;
        }
        let mut n = java_next_int(seed(pos), self.total as i32);
        for &(weight, id) in &self.alternatives {
            n -= weight as i32;
            if n < 0 {
                return id;
            }
        }
        None
    }
}

/// `Mth.getSeed`: Minecrafts Zufallssaat aus einer Blockposition. Die
/// erste Multiplikation laeuft in 32 Bit, alles danach in 64.
fn seed([x, y, z]: [i32; 3]) -> i64 {
    let l = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    let l = l
        .wrapping_mul(l)
        .wrapping_mul(42317861)
        .wrapping_add(l.wrapping_mul(11));
    l >> 16
}

/// `nextInt(bound)` eines frisch mit `seed` gesaeten Generators: der LCG
/// aus `java.util.Random`, den auch `SingleThreadedRandomSource` rechnet.
/// Bei einer Zweierpotenz die oberen Bits, sonst der Rest — mit der
/// Verwerfungsschleife, die Java gegen die Schieflage am oberen Ende hat.
/// Bis 1.21.4 nahm Minecraft stattdessen `abs((int) nextLong()) % total`.
fn java_next_int(seed: i64, bound: i32) -> i32 {
    const MULT: i64 = 0x5DEECE66D;
    const MASK: i64 = (1 << 48) - 1;
    let mut state = (seed ^ MULT) & MASK;
    let mut next31 = || {
        state = state.wrapping_mul(MULT).wrapping_add(0xB) & MASK;
        (state >> 17) as i32
    };
    if bound & (bound - 1) == 0 {
        return ((bound as i64 * next31() as i64) >> 31) as i32;
    }
    loop {
        let bits = next31();
        let value = bits % bound;
        if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
            return value;
        }
    }
}

/// Wie viele Schichten Wasser hinter einer Oberflaeche noch unterschieden
/// werden. Bei Alpha 180 laesst eine Schicht 29 Prozent durch, vier noch
/// 0,7 — dahinter sieht man nichts mehr, also gilt ab da dieselbe Fassung.
pub const DEPTHS: usize = 4;

/// Bit in der Verdeckungsmaske fuer eine Fluessigkeitsflaeche: die drei
/// Seiten, die die Kamera sieht, in der Reihenfolge der Nachbarn +x, +y, +z.
pub fn mask_bit(face: Face) -> u8 {
    match face {
        Face::East => 1,
        Face::Up => 2,
        Face::South => 4,
        _ => 0,
    }
}

/// Das Modell mit seiner Fluessigkeit auf voller Blockhoehe.
fn full_height(model: &BakedModel) -> BakedModel {
    let mut quads: Vec<Quad> = model
        .quads
        .iter()
        .filter(|q| q.fluid.is_none())
        .cloned()
        .collect();
    if let Some(q) = model.quads.iter().find(|q| q.fluid.is_some()) {
        let fluid = q.fluid.map(|(fluid, _)| fluid);
        quads.extend(box_quads(
            [0.0; 3],
            [16.0; 3],
            q.texture,
            q.tint_index,
            fluid,
        ));
    }
    BakedModel { quads }
}

/// Fluessigkeit eines Modells samt Oberflaechenhoehe.
fn fluid_of(model: &BakedModel) -> Option<(Fluid, f32)> {
    model.quads.iter().find_map(|q| match q.fluid {
        Some((fluid, Face::Up)) => Some((fluid, q.corners[0][1])),
        _ => None,
    })
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
            families: Vec::new(),
            by_state: HashMap::new(),
            by_mask: HashMap::new(),
            by_biome: HashMap::new(),
            projection,
            foreign: BTreeSet::new(),
        };

        for state in states {
            if state.is_air() || set.by_state.contains_key(state) {
                continue;
            }
            let models = models_of(assets, state)?;
            let fluid = models.first().and_then(|(_, model)| fluid_of(model));
            let alternatives: Vec<(u32, Option<SpriteId>)> = models
                .iter()
                .map(|(weight, model)| {
                    let id = set.insert_fluid(assets, state, model, fluid.is_some());
                    (*weight, id)
                })
                .collect();
            if alternatives.iter().all(|(_, id)| id.is_none()) {
                continue;
            }
            let total = alternatives.iter().map(|(weight, _)| *weight).sum();
            let opaque = alternatives
                .iter()
                .all(|(_, id)| id.is_some_and(|id| set.sprites[id.0 as usize].opaque));
            let bare = fluid.is_some()
                && models
                    .iter()
                    .all(|(_, model)| model.quads.iter().all(|q| q.fluid.is_some()));
            set.by_state
                .insert(state.clone(), set.families.len() as u32);
            set.families.push(Family {
                alternatives,
                total,
                fluid,
                opaque,
                bare,
            });
        }

        Ok(set)
    }

    /// Ein Modell mit allen Fassungen: bei einer Fluessigkeit je Maske aus
    /// verdeckten Flaechen eine, fuer die Oberflaeche je Tiefe darunter
    /// eine, und davon je Biom eine.
    fn insert_fluid(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        model: &BakedModel,
        has_fluid: bool,
    ) -> Option<SpriteId> {
        let base = self.insert_tinted(assets, state, model)?;
        if !has_fluid {
            return Some(base);
        }
        // Deckt die Textur schon, gibt es keine Tiefe zu zeichnen: Lava.
        let translucent = model.quads.iter().any(|q| {
            q.fluid.is_some()
                && assets
                    .textures()
                    .image(q.texture)
                    .pixels()
                    .any(|p| p.0[3] > 0 && p.0[3] < 255)
        });
        // Steht dieselbe Fluessigkeit darueber, reicht sie bis zur
        // Blockkante (`FlowingFluid.getHeight`); an der Oberflaeche endet
        // sie bei ihrer eigenen Hoehe.
        let voll = full_height(model);
        let mut variants = vec![None; 8 * DEPTHS];
        variants[0] = Some(base);
        for mask in 0..8u8 {
            let surface = mask & mask_bit(Face::Up) == 0;
            let depths = if surface && translucent { DEPTHS } else { 1 };
            let quelle = if surface { model } else { &voll };
            for depth in 0..depths {
                if mask == 0 && depth == 0 {
                    continue;
                }
                let quads = quelle
                    .quads
                    .iter()
                    .filter(|q| q.fluid.is_none_or(|(_, face)| mask & mask_bit(face) == 0))
                    .cloned()
                    .map(|mut q| {
                        if q.fluid.is_some_and(|(_, face)| face == Face::Up) {
                            q.layers = depth as u8 + 1;
                        }
                        q
                    })
                    .collect();
                variants[mask as usize + 8 * depth] =
                    self.insert_tinted(assets, state, &BakedModel { quads });
            }
        }
        self.by_mask.insert(base, variants);
        Some(base)
    }

    /// Rastert ein Modell in der Farbe des Standardklimas und, wenn es
    /// gefaerbte Flaechen hat, je Biom noch einmal.
    fn insert_tinted(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        model: &BakedModel,
    ) -> Option<SpriteId> {
        // Welche Faerbungen das Modell ueberhaupt traegt. Nur die
        // unterscheiden Fassungen — sonst bekaeme jeder Grasblock eine
        // Fassung je Wasserfarbe.
        let uses = model
            .quads
            .iter()
            .fold((false, false), |(block, water), q| match q.tint_index {
                None => (block, water),
                Some(fluid::TINT_INDEX) => (block, true),
                Some(_) => (true, water),
            });
        let tints = |biome: Option<&str>| {
            let t = assets.colors().tints(state.name(), biome);
            Tints {
                block: t.block.filter(|_| uses.0),
                water: t.water.filter(|_| uses.1),
            }
        };

        let default = tints(None);
        let sprite = render(model, assets.textures(), &self.projection, default)?;
        let id = self.insert(sprite, model);

        if default == Tints::default() {
            return Some(id);
        }
        // Eine Fassung je Biom; gleiche Farben teilen sich das Sprite.
        let mut by_tints = HashMap::from([(default, id)]);
        let mut by_biome = HashMap::new();
        for biome in assets.colors().biomes() {
            let tints = tints(Some(biome));
            let variant = match by_tints.get(&tints) {
                Some(&variant) => variant,
                None => {
                    let sprite = render(model, assets.textures(), &self.projection, tints)
                        .expect("dasselbe Modell, nur anders gefaerbt");
                    let variant = self.insert(sprite, model);
                    by_tints.insert(tints, variant);
                    variant
                }
            };
            by_biome.insert(biome.to_string(), variant);
        }
        self.by_biome.insert(id, by_biome);
        Some(id)
    }

    /// Zerlegt ein Sprite in seine Wuerfel und nimmt es in die Tabelle auf.
    fn insert(&mut self, sprite: Sprite, model: &BakedModel) -> SpriteId {
        let parts = split(sprite, model, self.projection);
        let opaque = parts
            .iter()
            .find(|(cell, _)| *cell == OWN_CELL)
            .is_some_and(|(_, sprite)| covers_cell(sprite, self.projection));
        let contained = parts
            .iter()
            .all(|(cell, sprite)| fits_cell(sprite, *cell, self.projection));

        self.foreign.extend(
            parts
                .iter()
                .map(|(cell, _)| *cell)
                .filter(|cell| *cell != OWN_CELL),
        );
        self.sprites.push(Entry {
            parts,
            opaque,
            contained,
        });
        SpriteId(self.sprites.len() as u32 - 1)
    }

    /// Das Sprite der ersten Alternative.
    pub fn id(&self, state: &BlockState) -> Option<SpriteId> {
        self.family_of(state).and_then(|f| f.alternatives[0].1)
    }

    pub fn family_of(&self, state: &BlockState) -> Option<&Family> {
        self.by_state.get(state).map(|&i| self.family(i))
    }

    /// Index der Familie einer Blockstate, fuer Caches je Paletteneintrag.
    pub fn family_index(&self, state: &BlockState) -> Option<u32> {
        self.by_state.get(state).copied()
    }

    pub fn family(&self, index: u32) -> &Family {
        &self.families[index as usize]
    }

    /// Die Fassung einer Fluessigkeit ohne die Flaechen zu Nachbarn mit
    /// derselben Fluessigkeit; `mask` traegt je Nachbar +x, +y, +z ein Bit,
    /// `depth` zaehlt die Schichten unter der Oberflaeche (0 = keine).
    /// `None`, wenn nichts uebrig bleibt — ein Wasserblock mitten im Meer.
    pub fn masked(&self, id: SpriteId, mask: u8, depth: usize) -> Option<SpriteId> {
        match self.by_mask.get(&id) {
            Some(variants) => {
                variants[mask as usize + 8 * depth.min(DEPTHS - 1)].or(variants[mask as usize])
            }
            None => Some(id),
        }
    }

    /// Die Fassung eines Sprites fuer ein Biom.
    ///
    /// `biome` wird nur befragt, wenn das Sprite Fassungen hat — fuer die
    /// allermeisten Bloecke kostet der Aufruf damit nur einen Nachschlag.
    /// Ein unbekanntes Biom bekommt die Fassung des Standardklimas.
    pub fn in_biome<'b>(&self, id: SpriteId, biome: impl FnOnce() -> Option<&'b str>) -> SpriteId {
        match self.by_biome.get(&id) {
            Some(variants) => biome()
                .and_then(|name| variants.get(name))
                .copied()
                .unwrap_or(id),
            None => id,
        }
    }

    /// Wie viele Sprites Fassungen sind: Alternativen, Biome, verdeckte
    /// Fluessigkeitsflaechen.
    pub fn variants(&self) -> usize {
        self.sprites.len() - self.families.len()
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

    // Eine Pixelbreite Rand bleibt aussen vor: die Texturmittelung kann
    // genau dort Alpha unter 255 lassen, und eine Blockkante um ein Pixel
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
/// Die eine Pixelbreite Toleranz entspricht der von `covers_cell`.
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
    use crate::assets::model_of;

    /// Referenzwerte aus den Klassen des 26.2-Clients selbst:
    /// `Mth.getSeed` und `SingleThreadedRandomSource.nextInt`, abgezaehlt
    /// wie `WeightedList`. Je Position die Saat und die Wahl bei den
    /// Gewichten [1, 1, 1, 1], [1, 1, 1], [1, 3] und [2, 1, 1, 1] — zwei
    /// Zweierpotenzen, zwei Reste.
    const CLIENT: [([i32; 3], i64, [u32; 4]); 7] = [
        ([0, 0, 0], 0, [2, 0, 1, 0]),
        ([1, 0, 0], 133076631897947, [0, 1, 0, 2]),
        ([-64, 64, 416], 435218090705, [1, 2, 1, 3]),
        ([12345, -3, -98765], 131000016891455, [1, 1, 1, 0]),
        ([2147483647, 319, -2147483648], 12517264342920, [0, 2, 0, 1]),
        ([100, 7, 100], -134188025211418, [3, 1, 1, 3]),
        ([-1, -64, -1], 52541653973741, [3, 0, 1, 0]),
    ];

    #[test]
    fn positionssaat_wie_im_client() {
        for (pos, saat, _) in CLIENT {
            assert_eq!(seed(pos), saat, "Saat fuer {pos:?}");
        }
    }

    #[test]
    fn gewichtete_wahl_wie_im_client() {
        let family = |weights: &[u32]| Family {
            alternatives: weights
                .iter()
                .enumerate()
                .map(|(i, &w)| (w, Some(SpriteId(i as u32))))
                .collect(),
            total: weights.iter().sum(),
            fluid: None,
            opaque: false,
            bare: false,
        };
        let listen = [
            family(&[1, 1, 1, 1]),
            family(&[1, 1, 1]),
            family(&[1, 3]),
            family(&[2, 1, 1, 1]),
        ];
        for (pos, _, erwartet) in CLIENT {
            for (liste, soll) in listen.iter().zip(erwartet) {
                assert_eq!(
                    liste.pick(pos),
                    Some(SpriteId(soll)),
                    "{pos:?} bei {} Alternativen",
                    liste.alternatives.len()
                );
            }
        }
    }
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
        assert_eq!(set.by_state.len(), 2, "einfarbig nur einmal");
        // stone liegt in der Fixture in zwei Alternativen vor
        assert_eq!(set.len(), 3);
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
            let model = model_of(&mut assets, &state(name)).unwrap();
            let ganz = render(&model, assets.textures(), &projection, Tints::default()).unwrap();

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
