use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

use anyhow::Result;
use image::RgbaImage;

use crate::assets::baker::{BakedModel, Quad, box_quads};
use crate::assets::blockstate::ModelRef;
use crate::assets::fluid::Fluid;
use crate::assets::{Assets, Face, Textures, Tints, fluid, models_of};
use crate::world::BlockState;

use super::rasterizer::faces_camera;
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
    /// ueber den Index des Bioms weiter.
    by_biome: HashMap<SpriteId, Vec<SpriteId>>,
    /// Index je Biomname, in der Reihenfolge von `Colors::biomes`. Eine
    /// Karte je Sprite mit allen Biomnamen als Schluessel waren bei
    /// Interconnect gut zwei Millionen Strings.
    biome_index: HashMap<String, usize>,
    /// Sprites nach dem Hash ihrer Pixel und ihrer Faerbung: pixelgleiche
    /// teilen sich den Eintrag, wenn sie sich in jedem Biom gleich faerben.
    by_content: HashMap<(u64, u64), Vec<SpriteId>>,
    /// Streifen einer Seitenflaeche ueber einem niedrigeren Nachbarn
    /// derselben Fluessigkeit: je Art, eigener Hoehe und Nachbarhoehe in
    /// Neunteln und je Seite.
    strips: HashMap<(Fluid, u8, u8, Face), SpriteId>,
    projection: Projection,
    /// Die Pixel eines vollen Wuerfels bei diesem scale, gegen die Deckung
    /// geprueft wird.
    masks: Masks,
    /// Die Oberseite einer Wasseroberflaeche bei scale 32, fuer `covers`.
    cover_top: Vec<(i32, i32)>,
    foreign: BTreeSet<Cell>,
}

/// Die Pixel eines vollen Wuerfels relativ zum Blockursprung, gerastert wie
/// jedes Sprite und mit derselben Fuellregel: der ganze Umriss und die
/// Oberseite allein.
///
/// Gegen sie prueft der Aufbau Pixel fuer Pixel, was ein Sprite deckt. Mit
/// einer Pixelbreite Toleranz am Rand galten flache Modelle mit schmalem
/// Rand — Druckplatten, Kuchen — als bodendeckend, der Block darunter fiel
/// weg, und sein sichtbarer Rand wurde zum Loch. Bei scale 4 blieb vom
/// geschrumpften Boden gar kein Pixel uebrig.
struct Masks {
    outline: Vec<(i32, i32)>,
    top: Vec<(i32, i32)>,
}

impl Masks {
    fn new(textures: &Textures, projection: Projection) -> Masks {
        Masks {
            outline: pixels_of(textures, projection, block(16.0, false)),
            top: pixels_of(textures, projection, block(16.0, true)),
        }
    }
}

/// Die Oberseite einer Wasseroberflaeche bei 8/9: durch sie treten die
/// Strahlen in die Tiefe ein, deren Ende `covers` misst.
fn surface_top(textures: &Textures, projection: Projection) -> Vec<(i32, i32)> {
    pixels_of(textures, projection, block(16.0 * 8.0 / 9.0, true))
}

/// Ein deckender Block bis zur Hoehe `top`, wahlweise nur seine Oberseite.
fn block(top: f32, only_up: bool) -> BakedModel {
    let quads = box_quads([0.0; 3], [16.0, top, 16.0], Textures::MISSING, None, None)
        .filter(|quad| !only_up || quad.normal()[1] > 0.0)
        .collect();
    BakedModel { quads }
}

/// Die Pixel, die ein Modell belegt, relativ zum Blockursprung.
fn pixels_of(textures: &Textures, projection: Projection, model: BakedModel) -> Vec<(i32, i32)> {
    let sprite = render(&model, textures, &projection, Tints::default())
        .expect("ein Block hat sichtbare Flaechen");
    sprite
        .image
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel.0[3] > 0)
        .map(|(x, y, _)| (sprite.offset.0 + x as i32, sprite.offset.1 + y as i32))
        .collect()
}

/// Deckt `sprite` jeden dieser Pixel undurchsichtig, um `dy` nach unten
/// verschoben? Eine leere Maske deckt nichts: sonst gaelte bei einem
/// Raster ohne Pixel jeder Block als deckend.
fn covers_all(sprite: &Sprite, pixels: &[(i32, i32)], dy: i32) -> bool {
    !pixels.is_empty()
        && pixels
            .iter()
            .all(|&(x, y)| alpha_at(sprite, x, y + dy) == 255)
}

/// Deckt `sprite` mehr als die Haelfte dieser Pixel undurchsichtig? Bei
/// genau der Haelfte nicht: dann geht ebenso viel an ihm vorbei, wie an ihm
/// endet, und ein heller Fleck ueber hohem Seegras faellt mehr auf als ein
/// fast verschwundener Halm.
fn covers_most(sprite: &Sprite, pixels: &[(i32, i32)]) -> bool {
    let deckend = pixels
        .iter()
        .filter(|&&(x, y)| alpha_at(sprite, x, y) == 255)
        .count();
    2 * deckend > pixels.len()
}

/// Die Alternativen einer Blockstate mit ihren Gewichten.
pub struct Family {
    alternatives: Vec<(u32, Option<SpriteId>)>,
    total: u32,
    /// Wo der Client die Saat nimmt, relativ zum Block: siehe
    /// `seed_offset`.
    seed_offset: [i32; 3],
    /// Fluessigkeit samt Menge in Neunteln der Blockhoehe, falls die
    /// Blockstate eine enthaelt.
    pub fluid: Option<(Fluid, u8)>,
    /// Decken alle Alternativen den Blockumriss? Dann verdeckt der Block
    /// seine Nachbarn — egal, welche Drehung die Position wuerfelt.
    pub opaque: bool,
    /// Decken alle Alternativen den Boden ihres Wuerfels, also die
    /// Oberseite des Blocks darunter? Lava endet bei 8/9 und deckt den
    /// Umriss nicht mehr, den Block darunter aber schon.
    pub covers_floor: bool,
    /// Decken alle Alternativen mehr als die Haelfte der Oberseite ihres
    /// Wuerfels? Dort treffen die Strahlen hinter einer Wasseroberflaeche
    /// den Block auf der Diagonalen, und die meisten enden an ihm. Sonst
    /// laufen sie hindurch: Seegras, Kelp, ein gefluteter Zaunpfosten und
    /// eine untere Platte zaehlen wie das Wasser um sie herum. Gemessen wird
    /// immer bei scale 32, damit die nativen Stufen dieselbe Tiefe zaehlen
    /// wie die Basis.
    pub covers: bool,
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
        let [dx, dy, dz] = self.seed_offset;
        let pos = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
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

/// Wo der Client die Saat einer Blockstate nimmt. Obere Haelften von
/// Tueren und Doppelpflanzen wuerfeln mit der Position der unteren, das
/// Fussende eines Betts mit der des Kopfendes, beide Haelften passen so
/// immer zusammen: `DoorBlock`, `DoublePlantBlock` und `BedBlock`
/// ueberschreiben `getSeed`, per javap am 26.2-Client. Nur diese Bloecke
/// tragen `half=upper` und `part=foot`.
fn seed_offset(state: &BlockState) -> [i32; 3] {
    if state.prop("half") == Some("upper") {
        return [0, -1, 0];
    }
    if state.prop("part") == Some("foot") {
        return match state.prop("facing") {
            Some("north") => [0, 0, -1],
            Some("south") => [0, 0, 1],
            Some("west") => [-1, 0, 0],
            Some("east") => [1, 0, 0],
            _ => [0, 0, 0],
        };
    }
    [0, 0, 0]
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

/// Alles, was das Bild einer Blockstate bestimmt: der Name (er entscheidet
/// die Faerbung), die Modellverweise samt Drehung und Gewicht, Art und
/// Menge der Fluessigkeit, und wo die Wahl der Alternative ihre Saat
/// nimmt. Die Verweise reichen, die Modelle selbst laedt erst die Familie.
type FamilyKey = (
    String,
    Vec<(u32, Vec<ModelRef>)>,
    Option<(Fluid, u8)>,
    [i32; 3],
);

fn family_key(assets: &mut Assets, state: &BlockState) -> Result<FamilyKey> {
    let alternatives = assets.alternative_refs(state)?;
    Ok((
        state.name().to_string(),
        alternatives,
        fluid::key(state),
        seed_offset(state),
    ))
}

/// Die Biome, mit denen eine Familie vorkommt.
type Biomes<'a> = BTreeSet<&'a str>;

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

struct Entry {
    /// Das Sprite, zerlegt nach den Wuerfeln, in denen seine Geometrie
    /// liegt. Fast immer genau ein Teil in `OWN_CELL`.
    parts: Vec<(Cell, Sprite)>,
    /// Deckt der eigene Teil den Blockumriss lueckenlos ab? Nur dann darf
    /// der Block etwas dahinter verdecken.
    opaque: bool,
    /// Deckt der eigene Teil den Boden des Wuerfels — die Oberseite des
    /// Blocks darunter?
    covers_floor: bool,
    /// Bleibt jeder Teil im Umriss seines eigenen Wuerfels? Nach der
    /// Zerlegung ist das der Normalfall; schlaegt sie fehl, verzichtet der
    /// Renderer auf die Verdeckungsabkuerzung.
    contained: bool,
}

impl SpriteSet {
    /// Backt und rastert jede Blockstate genau einmal, gefaerbte Fassungen
    /// nur fuer die Biome, mit denen sie im Vorlauf eine Section teilt. Auf
    /// der ganzen Welt kommen alle Biome vor, aber nicht jeder Block in
    /// jedem: Wasser hat in elf Wasserfarben keinen Sinn, wo es nur in
    /// dreien steht.
    ///
    /// Blockstates ohne sichtbare Geometrie — Luft, Truhen, Deckenfeuer —
    /// landen nicht in der Tabelle und werden beim Rendern uebersprungen.
    pub fn build_in<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = (&'a BlockState, &'a BTreeSet<String>)>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        let mut set = SpriteSet {
            sprites: Vec::new(),
            families: Vec::new(),
            by_state: HashMap::new(),
            by_mask: HashMap::new(),
            by_biome: HashMap::new(),
            biome_index: assets
                .colors()
                .biomes()
                .enumerate()
                .map(|(i, biome)| (biome.to_string(), i))
                .collect(),
            by_content: HashMap::new(),
            strips: HashMap::new(),
            projection,
            masks: Masks::new(assets.textures(), projection),
            cover_top: surface_top(assets.textures(), cover_projection()),
            foreign: BTreeSet::new(),
        };

        // Erst gruppieren: Blockstates, die sich nur in Eigenschaften ohne
        // Einfluss aufs Bild unterscheiden — Laub nach Entfernung, Kelp nach
        // Alter, Wasser nach Fallstufe —, teilen sich eine Familie, und die
        // Familie bekommt die Biome aller ihrer Blockstates.
        let mut groups: Vec<(Vec<&'a BlockState>, Biomes<'a>)> = Vec::new();
        let mut index: HashMap<FamilyKey, usize> = HashMap::new();
        let mut seen: HashSet<&BlockState> = HashSet::new();
        for (state, biomes) in states {
            if state.is_air() || !seen.insert(state) {
                continue;
            }
            let key = family_key(assets, state)?;
            let biomes = biomes.iter().map(String::as_str);
            match index.get(&key) {
                Some(&i) => {
                    groups[i].0.push(state);
                    groups[i].1.extend(biomes);
                }
                None => {
                    index.insert(key, groups.len());
                    groups.push((vec![state], biomes.collect()));
                }
            }
        }

        let mut fluids: BTreeMap<Fluid, Biomes<'a>> = BTreeMap::new();
        for (members, biomes) in groups {
            let state = members[0];
            let models = models_of(assets, state)?;
            for member in &members[1..] {
                assets.skip_like(member, state);
            }
            let fluid = fluid::key(state);
            let alternatives: Vec<(u32, Option<SpriteId>)> = models
                .iter()
                .map(|(weight, model)| {
                    let id = set.insert_fluid(assets, state, model, fluid.is_some(), &biomes);
                    (*weight, id)
                })
                .collect();
            if alternatives.iter().all(|(_, id)| id.is_none()) {
                continue;
            }
            let entries = || {
                alternatives
                    .iter()
                    .map(|(_, id)| id.map(|id| &set.sprites[id.0 as usize]))
            };
            let all = |test: fn(&Entry) -> bool| entries().all(|e| e.is_some_and(test));
            let covers = models
                .iter()
                .zip(&alternatives)
                .all(|((_, model), &(_, id))| set.covers_rays(assets, model, id));
            let family = Family {
                total: alternatives.iter().map(|(weight, _)| *weight).sum(),
                seed_offset: seed_offset(state),
                opaque: all(|e| e.opaque),
                covers_floor: all(|e| e.covers_floor),
                covers,
                fluid,
                alternatives,
            };
            let index = set.families.len() as u32;
            for member in members {
                set.by_state.insert(member.clone(), index);
            }
            set.families.push(family);
            if let Some((fluid, _)) = fluid {
                fluids.entry(fluid).or_default().extend(biomes);
            }
        }

        for (fluid, biomes) in fluids {
            set.insert_strips(assets, fluid, &biomes);
        }
        Ok(set)
    }

    /// Deckt ein Modell mehr als die Haelfte dessen, was eine
    /// Wasseroberflaeche an seiner Stelle belegen wuerde, gemessen bei
    /// scale 32? Bei scale 32 misst das fertige Sprite, sonst eine eigene
    /// Rasterung dafuer.
    fn covers_rays(&self, assets: &Assets, model: &BakedModel, id: Option<SpriteId>) -> bool {
        let reference = cover_projection();
        let own = id
            .filter(|_| self.projection.scale() == reference.scale())
            .map(|id| &self.sprites[id.0 as usize].parts)
            .filter(|parts| parts.len() == 1)
            .map(|parts| &parts[0].1);
        let gerastert;
        let sprite = match own {
            Some(sprite) => sprite,
            None => {
                gerastert = render(model, assets.textures(), &reference, Tints::default());
                match &gerastert {
                    Some(sprite) => sprite,
                    None => return false,
                }
            }
        };
        covers_most(sprite, &self.cover_top)
    }

    /// Streifen der Seitenflaechen ueber niedrigeren Nachbarn derselben
    /// Fluessigkeit, je Paar aus eigener Hoehe und Nachbarhoehe in Neunteln
    /// und je Seite — der Renderer haengt sie an, wo eine Oberflaeche an
    /// eine hoehere Saeule oder eine Stufe fliessenden Wassers stoesst.
    fn insert_strips(&mut self, assets: &mut Assets, fluid: Fluid, biomes: &BTreeSet<&str>) {
        let state = fluid.source();
        for own in 2..=fluid::FULL {
            for below in 1..own {
                for face in [Face::East, Face::South] {
                    let model = fluid::strip(assets, fluid, face, below, own);
                    if let Some(id) = self.insert_tinted(assets, &state, &model, biomes) {
                        self.strips.insert((fluid, own, below, face), id);
                    }
                }
            }
        }
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
        biomes: &BTreeSet<&str>,
    ) -> Option<SpriteId> {
        let base = self.insert_tinted(assets, state, model, biomes)?;
        // Teilen sich zwei Familien das Bild, teilen sie sich auch die
        // Fassungen, und die erste hat sie schon eingetragen: Blasensaeule
        // und geflutete Truhe sehen aus wie Wasser.
        if !has_fluid || self.by_mask.contains_key(&base) {
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
                    self.insert_tinted(assets, state, &BakedModel { quads }, biomes);
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
        biomes: &BTreeSet<&str>,
    ) -> Option<SpriteId> {
        // Welche Faerbungen das Modell ueberhaupt traegt. Nur die
        // unterscheiden Fassungen — sonst bekaeme jeder Grasblock eine
        // Fassung je Wasserfarbe. Gezaehlt wird nur, was der Rasterizer
        // zeichnet: ein gefluteter Zaun mitten im Wasser behaelt vom
        // Wasserwuerfel nur die abgewandten Seiten, und die gaeben sonst
        // je Wasserfarbe eine pixelgleiche Fassung.
        let uses = model.quads.iter().filter(|q| faces_camera(q)).fold(
            (false, false),
            |(block, water), q| match q.tint_index {
                None => (block, water),
                Some(fluid::TINT_INDEX) => (block, true),
                Some(_) => (true, water),
            },
        );
        let tints = |biome: Option<&str>| {
            let t = assets.colors().tints(state.name(), biome);
            Tints {
                block: t.block.filter(|_| uses.0),
                water: t.water.filter(|_| uses.1),
            }
        };

        let default = tints(None);
        let sprite = render(model, assets.textures(), &self.projection, default)?;
        // Die Faerbung je Biom als Signatur. Zwei Familien mit gleichem Bild
        // teilen sich das Sprite samt seinen Biomfassungen — das darf nur,
        // wer sich in jedem Biom gleich faerbt, sonst bekaeme Wasser die
        // Fassungen einer Blasensaeule aus weniger Biomen.
        let allowed = |biome: &str| biomes.contains(biome);
        let class = if default == Tints::default() {
            0
        } else {
            let mut hasher = std::hash::DefaultHasher::new();
            for biome in assets.colors().biomes() {
                allowed(biome).then(|| tints(Some(biome))).hash(&mut hasher);
            }
            hasher.finish() | 1
        };
        let id = self.insert(sprite, model, class);

        // Ein geteilter Eintrag hat seine Biomfassungen schon: gleiche
        // Klasse heisst gleiche Faerbung in jedem Biom.
        if class == 0 || self.by_biome.contains_key(&id) {
            return Some(id);
        }
        // Eine Fassung je Biom; gleiche Farben teilen sich das Sprite.
        let mut by_tints = HashMap::from([(default, id)]);
        let mut by_biome = Vec::with_capacity(self.biome_index.len());
        for biome in assets.colors().biomes() {
            // Biome, mit denen die Blockstate nie zusammen vorkommt, zeigen
            // auf das Standardklima und werden nie gefragt.
            if !allowed(biome) {
                by_biome.push(id);
                continue;
            }
            let tints = tints(Some(biome));
            let variant = match by_tints.get(&tints) {
                Some(&variant) => variant,
                None => {
                    let sprite = render(model, assets.textures(), &self.projection, tints)
                        .expect("dasselbe Modell, nur anders gefaerbt");
                    let variant = self.insert(sprite, model, 0);
                    by_tints.insert(tints, variant);
                    variant
                }
            };
            by_biome.push(variant);
        }
        self.by_biome.insert(id, by_biome);
        Some(id)
    }

    /// Zerlegt ein Sprite in seine Wuerfel und nimmt es in die Tabelle auf.
    ///
    /// Pixelgleiche Sprites teilen sich den Eintrag: die Tiefenfassungen
    /// einer gefluteten oberen Platte sind gleich, weil ihr Wasser in der
    /// deckenden Haelfte liegt, und eine Blasensaeule sieht aus wie Wasser.
    /// Nur fuer Sprites im eigenen Wuerfel — die Zerlegung eines
    /// ueberhaengenden haengt am Modell, nicht nur am Bild.
    ///
    /// `class` ist die Faerbungs-Signatur aus `insert_tinted`: nur Sprites
    /// derselben Klasse teilen sich den Eintrag. Fassungen je Biom haben
    /// die Klasse 0 wie ungefaerbte Sprites; Biomfassungen haengen nur am
    /// Sprite der Standardfarbe.
    fn insert(&mut self, sprite: Sprite, model: &BakedModel, class: u64) -> SpriteId {
        let key =
            fits_cell(&sprite, OWN_CELL, self.projection).then(|| (content_hash(&sprite), class));
        if let Some(key) = key
            && let Some(ids) = self.by_content.get(&key)
            && let Some(&id) = ids
                .iter()
                .find(|&&id| same_image(&self.sprites[id.0 as usize].parts[0].1, &sprite))
        {
            return id;
        }

        let parts = split(sprite, model, self.projection);
        let own = parts
            .iter()
            .find(|(cell, _)| *cell == OWN_CELL)
            .map(|(_, sprite)| sprite);
        // Der Boden ist die Oberseite des Blocks darunter, eine halbe
        // Blockhoehe tiefer im Bild.
        let floor = self.projection.scale() as i32 / 2;
        let opaque = own.is_some_and(|sprite| covers_all(sprite, &self.masks.outline, 0));
        let covers_floor = own.is_some_and(|sprite| covers_all(sprite, &self.masks.top, floor));
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
            covers_floor,
            contained,
        });
        let id = SpriteId(self.sprites.len() as u32 - 1);
        if let Some(key) = key {
            self.by_content.entry(key).or_default().push(id);
        }
        id
    }

    /// Der Streifen einer Seite zwischen der Hoehe eines niedrigeren
    /// Nachbarn und der eigenen, beide in Neunteln.
    pub fn strip(&self, fluid: Fluid, own: u8, below: u8, face: Face) -> Option<SpriteId> {
        self.strips.get(&(fluid, own, below, face)).copied()
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
                .and_then(|name| self.biome_index.get(name))
                .map_or(id, |&i| variants[i]),
            None => id,
        }
    }

    /// Wie viele Sprites Fassungen sind: Masken, Tiefen, Biome und
    /// Streifen — alles, was nicht das Grundbild einer Alternative ist.
    /// Familien teilen sich pixelgleiche Grundbilder, es kann also mehr
    /// Familien geben als Sprites.
    pub fn variants(&self) -> usize {
        let grundbilder: HashSet<SpriteId> = self
            .families
            .iter()
            .flat_map(|family| family.alternatives.iter().filter_map(|&(_, id)| id))
            .collect();
        self.sprites.len() - grundbilder.len()
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

/// Der scale, bei dem `covers` misst: der Standard. So zaehlen alle
/// Zoomstufen die Tiefe hinter einer Oberflaeche gleich.
fn cover_projection() -> Projection {
    Projection::new(Projection::DEFAULT_SCALE)
}

/// Alpha eines Pixelmittelpunkts relativ zum Blockursprung; ausserhalb des
/// Bilds ist nichts.
fn alpha_at(sprite: &Sprite, x: i32, y: i32) -> u8 {
    let (sx, sy) = (x - sprite.offset.0, y - sprite.offset.1);
    if sx < 0 || sy < 0 || sx >= sprite.image.width() as i32 || sy >= sprite.image.height() as i32 {
        return 0;
    }
    sprite.image.get_pixel(sx as u32, sy as u32).0[3]
}

fn content_hash(sprite: &Sprite) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    sprite.offset.hash(&mut hasher);
    sprite.image.dimensions().hash(&mut hasher);
    sprite.image.as_raw().hash(&mut hasher);
    hasher.finish()
}

fn same_image(a: &Sprite, b: &Sprite) -> bool {
    a.offset == b.offset
        && a.image.dimensions() == b.image.dimensions()
        && a.image.as_raw() == b.image.as_raw()
}

/// Prueft, ob ein Sprite ganz im Umriss eines Wuerfels bleibt, bis auf
/// eine Pixelbreite Toleranz: mehr verschiebt die Rundung beim Rastern
/// nicht.
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
            covers_floor: false,
            covers: false,
            seed_offset: [0, 0, 0],
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

    /// Die Tabelle fuer Blockstates, die in jedem geladenen Biom vorkommen.
    fn build<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        let alle: BTreeSet<String> = assets.colors().biomes().map(str::to_string).collect();
        SpriteSet::build_in(assets, states.into_iter().map(|s| (s, &alle)), projection)
    }

    #[test]
    fn luft_kommt_nicht_in_die_tabelle() {
        let mut assets = assets();
        let states = [state("minecraft:air"), state("einfarbig")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.len(), 1);
        assert!(set.id(&state("minecraft:air")).is_none());
        assert!(set.id(&state("einfarbig")).is_some());
    }

    #[test]
    fn jede_blockstate_nur_einmal() {
        let mut assets = assets();
        let states = [state("einfarbig"), state("einfarbig"), state("stone")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.by_state.len(), 2, "einfarbig nur einmal");
        // stone liegt in der Fixture in zwei Alternativen vor, um 180 Grad
        // gedreht. Die Textur ist dafuer symmetrisch, beide sehen gleich
        // aus und teilen sich das Sprite.
        let stone = set.family_of(&state("stone")).unwrap();
        assert_eq!(stone.alternatives.len(), 2);
        assert_eq!(stone.alternatives[0].1, stone.alternatives[1].1);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn voller_wuerfel_gilt_als_deckend() {
        let mut assets = assets();
        let states = [state("einfarbig")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let id = set.id(&state("einfarbig")).unwrap();
        assert!(set.is_opaque(id), "ein voller Würfel deckt ab");
    }

    /// Ein flaches Seerosenblatt füllt den Blockumriss nicht aus und darf
    /// deshalb nichts verdecken.
    #[test]
    fn flaches_modell_deckt_nicht_ab() {
        let mut assets = assets();
        let states = [state("seerose")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
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
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
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
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
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
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
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
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
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
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
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

    /// Blockstates mit demselben Bild teilen sich die Familie: Eigenschaften,
    /// die kein Modell auswaehlt, und Wasser gleicher Menge.
    #[test]
    fn gleiche_bilder_teilen_sich_die_familie() {
        let mut assets = assets();
        let states = [
            state("einfarbig[alter=1]"),
            state("einfarbig[alter=2]"),
            state("water[level=0]"),
            state("water[level=8]"),
            state("water[level=3]"),
        ];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let index = |text: &str| set.family_index(&state(text)).unwrap();
        assert_eq!(index("einfarbig[alter=1]"), index("einfarbig[alter=2]"));
        assert_eq!(
            index("water[level=0]"),
            index("water[level=8]"),
            "Quelle und Fall haben dieselbe Menge"
        );
        assert_ne!(index("water[level=0]"), index("water[level=3]"));
        assert_eq!(set.families.len(), 3);
    }

    /// Mitten im Wasser zeigt ein gefluteter Zaun kein Wasser mehr. Die
    /// abgewandten Seiten des Wasserwuerfels zeichnet der Rasterizer nicht,
    /// ihre Farbe darf keine Fassungen je Biom erzeugen.
    #[test]
    fn abgewandte_flaechen_faerben_nicht() {
        let mut assets = assets();
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base");
        assets.load_biomes(&data).unwrap();
        let states = [state("oak_fence[waterlogged=true]")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let base = set.families[0].alternatives[0].1.unwrap();
        assert!(set.by_biome.contains_key(&base), "Wasser sichtbar");
        let innen = set.by_mask[&base][7].unwrap();
        assert!(
            !set.by_biome.contains_key(&innen),
            "Maske 7 zeigt kein Wasser"
        );
    }

    /// Eine Alternative mit fehlendem Modell bleibt als Missing-Wuerfel in
    /// der Liste und behaelt ihr Gewicht: die Wahl je Position bleibt die
    /// des Clients. Fiele sie weg, zeigte der Block an jeder Position die
    /// uebrige Alternative.
    #[test]
    fn kaputte_alternative_behaelt_ihr_gewicht() {
        let mut assets = assets();
        let set = build(&mut assets, [&state("halb_kaputt")], Projection::new(16)).unwrap();
        let family = set.family_of(&state("halb_kaputt")).unwrap();
        assert_eq!(family.total, 4);
        assert_eq!(family.alternatives.len(), 2);
        for (pos, _, erwartet) in CLIENT {
            assert_eq!(
                family.pick(pos),
                family.alternatives[erwartet[2] as usize].1,
                "{pos:?}"
            );
        }
    }

    /// Die Saat der oberen Haelfte liegt einen Block tiefer, die des
    /// Fussendes eines Betts einen Schritt in Blickrichtung — beim
    /// Kopfende. Alles andere wuerfelt an der eigenen Position.
    #[test]
    fn saat_wie_getseed_im_client() {
        let faelle = [
            ("tall_grass[half=upper]", [0, -1, 0]),
            ("tall_grass[half=lower]", [0, 0, 0]),
            (
                "oak_door[facing=east,half=upper,hinge=left,open=false]",
                [0, -1, 0],
            ),
            ("red_bed[facing=north,part=foot]", [0, 0, -1]),
            ("red_bed[facing=south,part=foot]", [0, 0, 1]),
            ("red_bed[facing=west,part=foot]", [-1, 0, 0]),
            ("red_bed[facing=east,part=foot]", [1, 0, 0]),
            ("red_bed[facing=east,part=head]", [0, 0, 0]),
            ("oak_slab[type=top]", [0, 0, 0]),
            ("oak_stairs[half=top]", [0, 0, 0]),
        ];
        for (text, erwartet) in faelle {
            assert_eq!(seed_offset(&state(text)), erwartet, "{text}");
        }
    }

    /// Zwei Blockstates mit demselben Modell, wie `copper_block` und
    /// `waxed_copper_block`: zwei Familien, ein Sprite, keine Fassung. Die
    /// Zahl der Fassungen war Sprites minus Familien und lief hier unter
    /// null.
    #[test]
    fn geteilte_grundbilder_sind_keine_fassungen() {
        let mut assets = assets();
        let states = [state("einfarbig"), state("einfarbig_gewachst")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.families.len(), 2);
        assert_eq!(set.len(), 1);
        assert_eq!(set.variants(), 0);
    }

    /// Eine Familie loest ihre Modelle nur fuer ihr erstes Mitglied auf.
    /// Den Missing-Wuerfel zeichnen aber alle, und alle stehen in der
    /// Liste — Laub mit einer kaputten Alternative sieben Mal, nicht einmal.
    #[test]
    fn jede_blockstate_der_familie_steht_in_der_liste() {
        let mut assets = assets();
        let states = [
            state("halb_kaputt[distance=1]"),
            state("halb_kaputt[distance=2]"),
            state("kaputt[distance=1]"),
            state("kaputt[distance=2]"),
        ];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.families.len(), 2, "je Block eine Familie");
        assert_eq!(assets.skipped().len(), 4, "{:?}", assets.skipped());
        // Auch eine Blockstate ganz ohne heiles Modell bricht nichts ab.
        assert!(set.family_of(&state("kaputt[distance=1]")).is_some());
    }

    /// Gefaerbte Fassungen nur fuer die Biome, mit denen die Blockstate
    /// vorkommt; die anderen zeigen auf das Standardklima.
    #[test]
    fn faerbung_nur_fuer_biome_aus_dem_vorlauf() {
        let mut assets = assets();
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base");
        assets.load_biomes(&data).unwrap();
        let wasser = state("water[level=0]");
        let alle = build(&mut assets, [&wasser], Projection::new(16)).unwrap();
        let nur_frozen: BTreeSet<String> = ["minecraft:frozen".to_string()].into();
        let eines = SpriteSet::build_in(&mut assets, [(&wasser, &nur_frozen)], Projection::new(16))
            .unwrap();
        assert!(
            eines.len() < alle.len(),
            "{} gegen {}",
            eines.len(),
            alle.len()
        );
        let base = eines.id(&wasser).unwrap();
        assert_ne!(eines.in_biome(base, || Some("minecraft:frozen")), base);
        assert_eq!(eines.in_biome(base, || Some("terranova:heide")), base);
        assert_ne!(
            alle.in_biome(alle.id(&wasser).unwrap(), || Some("terranova:heide")),
            alle.id(&wasser).unwrap()
        );
    }

    /// Teilen sich zwei Familien ein Bild, aber nicht die Biome, bleiben
    /// die Sprites getrennt: sonst bestimmte die zuerst gebaute Familie die
    /// Fassungen der anderen. Die Blasensaeule steht alphabetisch vor dem
    /// Wasser.
    #[test]
    fn verschiedene_biome_trennen_gleiche_bilder() {
        let mut assets = assets();
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base");
        assets.load_biomes(&data).unwrap();
        let wasser = state("water[level=0]");
        let saeule = state("bubble_column");
        let frozen: BTreeSet<String> = ["minecraft:frozen".to_string()].into();
        let beide: BTreeSet<String> = ["minecraft:frozen", "minecraft:swamp"]
            .map(String::from)
            .into();
        let set = SpriteSet::build_in(
            &mut assets,
            [(&saeule, &frozen), (&wasser, &beide)],
            Projection::new(16),
        )
        .unwrap();
        let id = set.id(&wasser).unwrap();
        assert_ne!(
            set.in_biome(id, || Some("minecraft:swamp")),
            id,
            "Wasser im Sumpf hat seine eigene Farbe"
        );
    }

    /// Pixelgleiche Sprites teilen sich den Eintrag, auch ueber Familien
    /// hinweg: eine Blasensaeule sieht aus wie Wasser.
    #[test]
    fn pixelgleiche_sprites_teilen_sich_den_eintrag() {
        let mut assets = assets();
        let states = [state("water[level=0]"), state("bubble_column")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.families.len(), 2);
        assert_eq!(set.id(&states[0]), set.id(&states[1]));
        assert_eq!(
            set.len(),
            build(&mut assets, &states[..1], Projection::new(16))
                .unwrap()
                .len()
        );
    }

    /// Lava endet bei 8/9: sie deckt den Umriss nicht mehr, den Block
    /// darunter aber schon. Ein Zaunpfosten deckt fast nichts, ein voller
    /// Wuerfel alles, eine Druckplatte ihren Boden nicht: ihr Rand ist zu
    /// sehen. Bei scale 32, denn bei 16 ist der Streifen ueber der Lava
    /// keinen Pixel hoch — dann deckt sie ihren Umriss tatsaechlich.
    #[test]
    fn deckung_nach_bereich() {
        let mut assets = assets();
        let states = [
            state("lava"),
            state("einfarbig"),
            state("oak_fence[north=true]"),
            state("water"),
            state("druckplatte"),
            state("teppich"),
        ];
        // Auf jeder Stufe gleich, auch bei scale 4: dort blieb vom
        // geschrumpften Boden frueher kein Pixel, und nichts wurde verdeckt.
        for scale in [32, 16, 8, 4] {
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
            let flags = |text: &str| {
                let f = set.family_of(&state(text)).unwrap();
                (f.opaque, f.covers_floor, f.covers)
            };
            assert_eq!(flags("einfarbig"), (true, true, true), "scale {scale}");
            assert_eq!(flags("water"), (false, false, false), "scale {scale}");
        }
        let set = build(&mut assets, &states, Projection::new(32)).unwrap();
        let flags = |text: &str| {
            let f = set.family_of(&state(text)).unwrap();
            (f.opaque, f.covers_floor, f.covers)
        };
        assert_eq!(flags("lava"), (false, true, true));
        assert_eq!(flags("oak_fence[north=true]"), (false, false, false));
        assert_eq!(
            flags("water"),
            (false, false, false),
            "durchscheinend deckt nichts"
        );
        assert_eq!(flags("druckplatte"), (false, false, false), "Rand frei");
        assert_eq!(flags("teppich"), (false, true, false), "Boden ganz");
    }

    /// Ob der Strahl hinter einer Wasseroberflaeche an einem Block endet,
    /// entscheidet, was er von ihrer Oberseite deckt, und zwar auf jeder
    /// Stufe gleich. Eine untere Platte deckt dort 100 von 256 Pixeln, eine
    /// obere alles; am ganzen Umriss gemessen deckte die untere zwei Drittel
    /// und beendete die Zaehlung. Ein schmales Brett an der Westkante deckt
    /// bei scale 32 154 von 256, im eigenen Raster bei scale 4 aber nur
    /// einen von vier Pixeln — gemessen wird deshalb immer bei scale 32.
    #[test]
    fn strahlen_enden_an_der_oberseite() {
        let mut assets = assets();
        let states = [
            state("untere_platte[waterlogged=true]"),
            state("obere_platte[waterlogged=true]"),
            state("oak_fence[north=true,waterlogged=true]"),
            state("einfarbig"),
            state("schmal"),
        ];
        for scale in [32, 16, 8, 4] {
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
            let covers = |text: &str| set.family_of(&state(text)).unwrap().covers;
            assert!(covers("schmal"), "scale {scale}");
            assert!(!covers("untere_platte[waterlogged=true]"), "scale {scale}");
            assert!(covers("obere_platte[waterlogged=true]"), "scale {scale}");
            assert!(
                !covers("oak_fence[north=true,waterlogged=true]"),
                "scale {scale}"
            );
            assert!(covers("einfarbig"), "scale {scale}");
        }
    }

    /// Genau die Haelfte haelt den Strahl nicht auf, eins mehr schon. Hohes
    /// Seegras deckt bei scale 32 genau 128 der 256 Pixel; mit
    /// "mindestens die Haelfte" beendete es die Zaehlung, und ueber ihm
    /// stuende ein heller Fleck.
    #[test]
    fn gleichstand_zaehlt_als_wasser() {
        let pixels: Vec<(i32, i32)> = (0..4).map(|x| (x, 0)).collect();
        let mut sprite = Sprite {
            image: RgbaImage::new(4, 1),
            offset: (0, 0),
        };
        for x in 0..2 {
            sprite.image.put_pixel(x, 0, image::Rgba([0, 0, 0, 255]));
        }
        assert!(!covers_most(&sprite, &pixels), "zwei von vier");
        sprite.image.put_pixel(2, 0, image::Rgba([0, 0, 0, 255]));
        assert!(covers_most(&sprite, &pixels), "drei von vier");
    }

    /// Streifen gibt es je Paar aus eigener Hoehe und Nachbarhoehe, fuer
    /// beide sichtbaren Seiten, und sie liegen ueber der Nachbarhoehe.
    #[test]
    fn streifen_fuer_jede_stufe() {
        let mut assets = assets();
        let set = build(&mut assets, [&state("water")], Projection::new(16)).unwrap();
        assert!(set.strip(Fluid::Water, 9, 8, Face::East).is_some());
        assert!(set.strip(Fluid::Water, 8, 1, Face::South).is_some());
        assert!(
            set.strip(Fluid::Water, 8, 8, Face::East).is_none(),
            "kein Streifen ohne Hoehenunterschied"
        );
        assert!(
            set.strip(Fluid::Lava, 9, 8, Face::East).is_none(),
            "keine Lava in der Welt"
        );
        let id = set.strip(Fluid::Water, 9, 8, Face::East).unwrap();
        let sprite = set.part(id, OWN_CELL).unwrap();
        // Ein Neuntel Blockhoehe ist bei scale 16 knapp ein Pixel hoch: je
        // Spalte hoechstens zwei Pixel, schraeg ueber die ganze Seite.
        let (w, h) = sprite.image.dimensions();
        for x in 0..w {
            let dicke = (0..h)
                .filter(|&y| sprite.image.get_pixel(x, y).0[3] > 0)
                .count();
            assert!(dicke <= 2, "Spalte {x}: {dicke} Pixel");
        }
        assert!(sprite.image.pixels().any(|p| p.0[3] > 0));
    }

    /// Blöcke ohne sichtbare Geometrie tauchen gar nicht erst auf.
    #[test]
    fn modell_ohne_flaechen_faellt_heraus() {
        let mut assets = assets();
        let states = [state("chest")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert!(set.is_empty());
    }
}
