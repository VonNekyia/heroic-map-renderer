use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use anyhow::Result;

use crate::assets::baker::{BakedModel, Quad, box_quads};
use crate::assets::blockentity;
use crate::assets::blockstate::{self, KOLLISION, ModelRef, Nachbarregel, seite};
use crate::assets::colors::{Resolver, Source, Tint, source_of, tinted_below};
use crate::assets::fluid::Fluid;
use crate::assets::noise::JavaRandom;
use crate::assets::{Assets, CardinalLight, Face, Textures, Tints, fluid, models_of};
use crate::world::{BlockState, Blockdaten};

use super::kino::Kino;
use super::look::Look;
use super::metatile::ohne_null;
use super::rasterizer::{Lightmap, Raster, auf_den_vorderseiten, faces_camera, rastern};
use super::sonne::{Masken, Sonnenform};
use super::tint::BiomeTable;
use super::{Kamera, Projection, Richtung, Sprite, render};

/// Verweis in die Sprite-Tabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteId(u32);

/// Ein Blockwuerfel, relativ zu dem Block, dem das Modell gehoert.
pub type Cell = [i32; 3];

/// Der Wuerfel des Blocks selbst.
pub const OWN_CELL: Cell = [0, 0, 0];

/// Obergrenze fuer die Wuerfel, in die ein Modell zerfaellt; darueber bleibt
/// es ganz. Jeder Wuerfel kostet im Renderpfad je ueberhaengendem Block ein
/// Nachschlagen, und ein kaputtes Modell soll nicht hunderte davon bringen.
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
    /// Je Familie und Daten eines Blockentity, die ihr Bild ändern, die
    /// Familie mit diesen Daten, siehe [`SpriteSet::add_entities`].
    by_entity: HashMap<u32, HashMap<Blockdaten, u32>>,
    /// Fassungen einer Fluessigkeit: je Maske aus verdeckten Flaechen
    /// (`mask_bit`) eine, Index `mask`. Eintrag 0 ist das Sprite selbst.
    by_mask: HashMap<SpriteId, Vec<Option<SpriteId>>>,
    /// Sprites nach dem Hash ihrer Pixel samt Tönungskarte: pixelgleiche
    /// teilen sich den Eintrag.
    by_content: HashMap<u64, Vec<SpriteId>>,
    /// Streifen einer Seitenflaeche ueber einem niedrigeren Nachbarn
    /// derselben Fluessigkeit: je Art, eigener Hoehe und Nachbarhoehe in
    /// Neunteln und je Seite.
    strips: HashMap<(Fluid, u8, u8, Face), SpriteId>,
    projection: Projection,
    /// Wie die Seiten schattiert werden, nach dem Typ der Dimension aus
    /// [`Assets::dimension_type`].
    licht: CardinalLight,
    /// Die Lightmap nach demselben Typ.
    lightmap: Lightmap,
    /// Ob die Dimension Himmelslicht hat (`has_skylight`).
    himmel: bool,
    /// Die Pixel eines vollen Wuerfels bei diesem scale, gegen die Deckung
    /// geprueft wird.
    masks: Masks,
    foreign: BTreeSet<Cell>,
    /// Die Farben der Biome, mit denen die Sprites beim Zeichnen getönt
    /// werden.
    biomes: BiomeTable,
    /// Die Blöcke, die `blocks.txt` nicht kennt und die das Licht ganz aufhalten,
    /// entschieden im Raster in 2:1 beim scale der Basis, siehe
    /// [`SpriteSet::deckt_fuer_licht`]. `None` nur in einer Tabelle der Basis
    /// in 2:1: Dort entscheidet sie selbst.
    licht_deckend: Option<HashSet<BlockState>>,
    /// Womit Cinematic zeichnet; `None` für die Karte.
    kino: Option<Kino>,
    /// Nur für Cinematic: die Texelmasken der [`Sonnenform`]en und die
    /// Würfel um einen Block, in die irgendein Modell ragt, von und bis.
    masken: Masken,
    sonne_reich: [[i32; 3]; 2],
}

/// Die Pixel eines vollen Wuerfels relativ zum Blockursprung, gerastert wie
/// jedes Sprite und mit derselben Fuellregel: der ganze Umriss und die
/// Oberseite allein. Gegen sie prueft der Aufbau Pixel fuer Pixel, ohne
/// Toleranz am Rand, was ein Sprite deckt.
/// Siehe docs/renderer/sprites-und-deckung.md, „Wann ein Sprite deckt“.
struct Masks {
    outline: Vec<(i32, i32)>,
    /// Derselbe Umriss Zeile für Zeile, siehe [`SpriteSet::outline_rows`].
    rows: Vec<(i32, i32, i32)>,
    top: Vec<(i32, i32)>,
}

impl Masks {
    fn new(textures: &Textures, projection: Projection) -> Masks {
        let outline = pixels_of(textures, projection, block(16.0, false));
        let mut rows: BTreeMap<i32, (i32, i32)> = BTreeMap::new();
        for &(x, y) in &outline {
            let row = rows.entry(y).or_insert((x, x));
            *row = (row.0.min(x), row.1.max(x));
        }
        Masks {
            rows: rows.into_iter().map(|(y, (x0, x1))| (y, x0, x1)).collect(),
            outline,
            top: pixels_of(textures, projection, block(16.0, true)),
        }
    }

    /// Liegt jeder sichtbare Pixel des Sprites im Umriss? Gezählt statt
    /// nachgeschlagen: Jede Stelle des Umrisses kommt einmal vor, also sind
    /// die sichtbaren Pixel dort genau dann alle, wenn keiner daneben liegt.
    fn contains(&self, sprite: &Sprite) -> bool {
        let alle = sprite.image.pixels().filter(|p| p.0[3] > 0).count();
        let innen = self
            .outline
            .iter()
            .filter(|&&(x, y)| alpha_at(sprite, x, y) > 0);
        innen.count() == alle
    }
}

/// Ein deckender Block bis zur Hoehe `top`, wahlweise nur seine Oberseite.
fn block(top: f32, only_up: bool) -> BakedModel {
    let quads = box_quads([0.0; 3], [16.0, top, 16.0], Textures::MISSING, None, None)
        .filter(|quad| !only_up || quad.normal()[1] > 0.0)
        .collect();
    BakedModel {
        quads,
        ambient_occlusion: false,
    }
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
    /// Bleibt jede Alternative Pixel fuer Pixel im Umriss ihres eigenen
    /// Wuerfels, siehe `Entry::contained`? Nur dann darf ein verdeckter
    /// Block uebersprungen werden, ohne dass etwas von ihm haette
    /// herausragen koennen.
    pub contained: bool,
    /// Ragt eine Alternative in Nachbarwuerfel? Dann muss der Renderer
    /// von diesem Block aus auch dort zeichnen.
    pub foreign: bool,
    /// Liegt in jeder Alternative jede Fläche, die die Kamera sieht, auf
    /// einer der drei vorderen Seiten des Würfels ([`auf_den_vorderseiten`])?
    /// Dann kommt ein fremdes Teil in diesem Würfel vor den Block.
    /// Siehe docs/renderer/kamera.md, „Ein Teil im Würfel eines anderen Blocks“.
    pub wuerfelform: bool,
    /// Besteht jede Alternative nur aus der Fluessigkeit, ohne Modell
    /// daneben: Wasser, Lava, Blasensaeule. Dann bleibt vom Block nichts,
    /// wo ueber ihm dieselbe Fluessigkeit steht und zu beiden Seiten
    /// dieselbe mit derselben darueber oder ein deckender Block; sonst
    /// bleibt die Oberflaeche oder ein Streifen. Siehe `expose` im
    /// Metatile-Renderer.
    pub pure_fluid: bool,
    /// Welche Farbe des Bioms die gefärbten Flächen des Blocks tragen, wenn
    /// sie vom Biom kommt; den Anteil je Pixel trägt die Tönungskarte.
    pub resolver: Option<Resolver>,
    /// Nimmt der Block diese Farbe am Block darunter, siehe
    /// [`tinted_below`]?
    pub tint_below: bool,
    /// Wo die andere Hälfte einer Doppelkiste steht, relativ zum Block, im
    /// Blick, siehe [`doppelkiste`].
    pub doppelkiste: Option<[i32; 3]>,
    /// Zu welchen Nachbarn der Block Flächen weglässt, siehe
    /// [`blockstate::nachbarregel`].
    pub nachbarn: Option<Nachbarregel>,
    /// Die Seiten der Welt, an denen der Block voll deckt, siehe
    /// [`blockstate::volle_seiten`]: Dorthin lässt jeder Nachbar seine
    /// Flächen mit `cullface` weg.
    pub voll: u8,
    /// Die Seiten, zu denen das wirklich eine Fläche trifft: eine, die die
    /// Kamera sieht, mit ihrer `cullface` dort, und die Regel kann dorthin
    /// wirken oder ein Nachbar, der dort voll deckt, übermalt sie nicht
    /// ([`auf_der_wand`]). Bits nach [`seite`].
    seiten: u8,
    /// Je Alternative ihre Fassungen ohne die Flächen zu Nachbarn, wenn
    /// `seiten` nicht leer ist, siehe [`Family::ohne_nachbarn`].
    fassungen: Vec<Vec<Option<SpriteId>>>,
    /// Nur für Cinematic: was der Block dem Strahl zur Sonne in den Weg
    /// stellt.
    pub sonne: Option<Sonnenform>,
}

/// Die Seiten in der Reihenfolge von `Direction.values()`, wie [`seite`].
const SEITEN: [Face; 6] = [
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
    Face::West,
    Face::East,
];

impl Family {
    /// Das Grundbild einer Alternative, nach ihrem Platz aus
    /// [`Family::wahl`].
    pub fn sprite(&self, wahl: usize) -> Option<SpriteId> {
        self.alternatives[wahl].1
    }

    /// Die Alternative fuer einen Block an `pos` in der Welt, als Platz in
    /// der Liste, dieselbe, die der 26.2-Client wuerfelt: `nextInt(total)`
    /// aus `Mth.getSeed` der Position, die Gewichte in Listenreihenfolge
    /// abgezaehlt.
    /// Siehe docs/renderer/varianten.md, „Wie gewürfelt wird“.
    pub fn wahl(&self, pos: [i32; 3]) -> Option<usize> {
        if self.alternatives.len() == 1 {
            return Some(0);
        }
        let [dx, dy, dz] = self.seed_offset;
        let pos = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
        let mut n = java_next_int(seed(pos), self.total as i32);
        for (i, &(weight, _)) in self.alternatives.iter().enumerate() {
            n -= weight as i32;
            if n < 0 {
                return Some(i);
            }
        }
        None
    }

    /// Die Seiten, zu denen [`Family::ohne_nachbarn`] fragt, in dieser
    /// Reihenfolge, Seiten der Welt. Leer für die meisten Blöcke.
    pub fn nachbarseiten(&self) -> impl Iterator<Item = Face> + '_ {
        SEITEN
            .into_iter()
            .filter(|&face| self.seiten & seite(face) != 0)
    }

    /// Die Fassung einer Alternative ohne die Flächen zu Nachbarn, die sie
    /// verdecken: `nachbarn` trägt je Seite aus [`Family::nachbarseiten`] ein Bit, in
    /// deren Reihenfolge, `fluessig` die Maske der Flüssigkeit im Block wie
    /// bei [`SpriteSet::masked`]. `None`, wenn nichts bleibt.
    pub fn ohne_nachbarn(&self, wahl: usize, fluessig: u8, nachbarn: u8) -> Option<SpriteId> {
        let je_nachbar = if self.fluid.is_some() { 8 } else { 1 };
        self.fassungen[wahl][fluessig as usize + je_nachbar * nachbarn as usize]
    }

    /// Hat der Block Fassungen ohne Flächen zu Nachbarn?
    pub fn hat_nachbarn(&self) -> bool {
        self.seiten != 0
    }

    /// Ist der Block die obere Hälfte des Blocks darunter, siehe
    /// [`seed_offset`]?
    pub fn obere_haelfte(&self) -> bool {
        self.seed_offset == [0, -1, 0]
    }
}

/// Wo der Client die Saat einer Blockstate nimmt: obere Haelften von
/// Tueren und Doppelpflanzen bei der unteren, das Fussende eines Betts beim
/// Kopfende. Nur diese Bloecke tragen `half=upper` und `part=foot`.
/// Siehe docs/renderer/varianten.md, „Doppelblöcke“.
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
/// aus `java.util.Random`, den auch `SingleThreadedRandomSource` rechnet,
/// siehe [`JavaRandom::next_int`].
/// Siehe docs/renderer/varianten.md, „Wie gewürfelt wird“.
fn java_next_int(seed: i64, bound: i32) -> i32 {
    JavaRandom::new(seed).next_int(bound)
}

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
/// Menge der Fluessigkeit, wo die Wahl der Alternative ihre Saat nimmt und
/// was sein Blockentity zeichnet, dazu die volle Kollisionsform, siehe
/// [`kollision`], zu welchen Nachbarn er Flächen weglässt und wo er selbst
/// voll deckt. Die
/// Verweise reichen, die Modelle selbst laedt erst die Familie. Eine Truhe
/// hat in jeder Lage dasselbe Blockmodell, aber nicht dasselbe Bild aus
/// [`blockentity::bild`]; das trennt auch, wo die andere Hälfte einer
/// Doppelkiste steht.
type FamilyKey = (
    String,
    Vec<(u32, Vec<ModelRef>)>,
    Option<(Fluid, u8)>,
    [i32; 3],
    Option<usize>,
    bool,
    Option<Nachbarregel>,
    u8,
);

/// Was die Sprites einer Blockstate bestimmt. Das Licht gehört nicht dazu,
/// es kommt beim Zeichnen.
fn family_key(assets: &mut Assets, state: &BlockState) -> Result<FamilyKey> {
    let alternatives = assets.alternative_refs(state)?;
    Ok((
        state.name().to_string(),
        alternatives,
        fluid::key(state),
        seed_offset(state),
        blockentity::bild(state),
        kollision(state),
        blockstate::nachbarregel(state),
        blockstate::volle_seiten(state),
    ))
}

/// Zeigt die Fläche zur Seite `face`, und liegt jede Ecke auf der Wand
/// dorthin oder dahinter im Würfel des Nachbarn? Nur dann übermalt sie ein
/// Nachbar dort, der voll deckt.
/// Siehe docs/renderer/sprites-und-deckung.md, „Flächen vor einem vollen Nachbarn“.
fn auf_der_wand(quad: &Quad, face: Face) -> bool {
    let d = face.versatz();
    let achse = d.iter().position(|&a| a != 0).expect("eine Achse je Seite");
    let (richtung, wand) = if d[achse] > 0 {
        (1.0, 1.0)
    } else {
        (-1.0, 0.0)
    };
    quad.normal()[achse] * richtung > 0.0
        && quad
            .corners
            .iter()
            .all(|ecke| (-1e-5..=1.0).contains(&((ecke[achse] - wand) * richtung)))
}

/// Wo die andere Hälfte einer Doppelkiste steht, relativ zum Block, wie
/// `ChestBlock.getConnectedDirection` in 26.2: bei `type=left` im
/// Uhrzeigersinn neben `facing`, bei `right` dagegen. `type` mit `left` und
/// `right` hat in 26.2 jeder `ChestBlock` und sonst kein Block. Ihr Bild
/// zeichnet das Spiel im helleren Licht beider Hälften.
/// Siehe docs/renderer/wasser-und-licht.md, „Welches Licht ein Block bekommt“.
fn doppelkiste(state: &BlockState) -> Option<[i32; 3]> {
    // Norden, Osten, Süden, Westen: im Uhrzeigersinn.
    const RUNDUM: [(&str, [i32; 3]); 4] = [
        ("north", [0, 0, -1]),
        ("east", [1, 0, 0]),
        ("south", [0, 0, 1]),
        ("west", [-1, 0, 0]),
    ];
    let weiter = match state.prop("type")? {
        "left" => 1,
        "right" => 3,
        _ => return None,
    };
    let facing = state.prop("facing")?;
    let i = RUNDUM.iter().position(|&(name, _)| name == facing)?;
    Some(RUNDUM[(i + weiter) % 4].1)
}

/// Hat der Block volle Kollisionsform? Dann liegt jede ebene Fläche seines
/// Modells im Licht der Zelle davor, siehe [`KOLLISION`].
fn kollision(state: &BlockState) -> bool {
    blockstate::schatten(state) & KOLLISION != 0
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
    BakedModel {
        quads,
        ambient_occlusion: model.ambient_occlusion,
    }
}

/// Zeilenmasken eines Sprites für die Deckungsmaske der CPU: je Zeile ein
/// Bit je Pixel, ob er etwas zeichnet (Alpha über 0) und ob er deckt
/// (Alpha 255). Bit `x % 64` von Wort `x / 64` steht für Spalte `x`.
pub struct Rows {
    words: usize,
    any: Vec<u64>,
    full: Vec<u64>,
}

impl Rows {
    pub(super) fn of(sprite: &Sprite) -> Rows {
        let (w, h) = sprite.image.dimensions();
        let words = (w as usize).div_ceil(64);
        let mut rows = Rows {
            words,
            any: vec![0; words * h as usize],
            full: vec![0; words * h as usize],
        };
        for (x, y, pixel) in sprite.image.enumerate_pixels() {
            let (i, bit) = (y as usize * words + x as usize / 64, 1 << (x % 64));
            if pixel.0[3] > 0 {
                rows.any[i] |= bit;
            }
            if pixel.0[3] == 255 {
                rows.full[i] |= bit;
            }
        }
        rows
    }

    /// Was Zeile `y` zeichnet und was davon deckt.
    pub fn row(&self, y: usize) -> (&[u64], &[u64]) {
        let r = y * self.words..(y + 1) * self.words;
        (&self.any[r.clone()], &self.full[r])
    }
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
    /// Bleibt der eigene Teil Pixel fuer Pixel im gerasterten Umriss seines
    /// Wuerfels? Nur dann darf der Block verdeckt wegfallen: was daneben
    /// liegt, deckt kein Nachbar sicher. Schlaegt die Zerlegung fehl, gilt das
    /// erst recht.
    /// Siehe docs/renderer/sprites-und-deckung.md, „Verdeckte Würfel“.
    contained: bool,
    /// Je Teil seine Zeilenmasken, erst wenn die CPU sie braucht.
    rows: OnceLock<Vec<Rows>>,
    /// Welche Farben die Tönungskarte trägt: [`TINT_BLOCK`], [`TINT_WATER`].
    tints: u8,
    /// Hat ein Teil eine AO-Karte mit Pixeln ohne Seite? Die liegen im Licht
    /// der eigenen Zelle, siehe [`Sprite::ao`].
    innen: bool,
    /// Welche Plätze der AO-Karte sichtbare Pixel haben, Bit `p` für Platz
    /// `p`, siehe [`Sprite::ao`].
    plaetze: u8,
}

/// Die Tönungskarte trägt einen Anteil der Farbe des Blocks.
pub const TINT_BLOCK: u8 = 1;
/// Die Tönungskarte trägt einen Anteil der Farbe des Wassers.
pub const TINT_WATER: u8 = 2;

impl SpriteSet {
    /// Backt und rastert jede Blockstate genau einmal, gefärbte Flächen als
    /// Tönungskarte, siehe [`Sprite::tint`]. Blockstates ohne sichtbare
    /// Geometrie — Luft, Barriere, End-Portal — landen nicht in der Tabelle
    /// und werden beim Rendern uebersprungen. Die Farben der Biome kommen
    /// aus `assets`, gemischt mit Radius 2 und ohne Seed, siehe
    /// [`SpriteSet::set_biomes`].
    /// Siehe docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
    pub fn build_in<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        SpriteSet::build_mit_licht(assets, states, projection, None, None)
    }

    /// Wie [`SpriteSet::build_in`], nur kommt aus `licht_deckend`, welche
    /// Blöcke, die `blocks.txt` nicht kennt, das Licht ganz aufhalten
    /// ([`SpriteSet::licht_deckend`]). Eine native Stufe nimmt so die Antwort
    /// der Basis, damit ihr Licht nicht am scale hängt. Mit `look` für
    /// Cinematic.
    pub fn build_mit_licht<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
        licht_deckend: Option<HashSet<BlockState>>,
        look: Option<Look>,
    ) -> Result<SpriteSet> {
        let typ = assets.dimension_type();
        let biomes = BiomeTable::new(assets.colors());
        let kino = look.map(|look| Kino::new(look, &typ, &biomes, projection.kamera()));
        let mut set = SpriteSet {
            sprites: Vec::new(),
            families: Vec::new(),
            by_state: HashMap::new(),
            by_entity: HashMap::new(),
            by_mask: HashMap::new(),
            by_content: HashMap::new(),
            strips: HashMap::new(),
            projection,
            licht: typ.cardinal_light,
            lightmap: Lightmap::new(&typ),
            himmel: typ.has_skylight,
            masks: Masks::new(assets.textures(), projection),
            foreign: BTreeSet::new(),
            biomes,
            licht_deckend: None,
            kino,
            masken: Masken::default(),
            sonne_reich: [[0; 3]; 2],
        };

        // Erst gruppieren: Blockstates, die sich nur in Eigenschaften ohne
        // Einfluss aufs Bild unterscheiden — Laub nach Entfernung, Kelp nach
        // Alter, Wasser nach Fallstufe —, teilen sich eine Familie.
        let mut groups: Vec<Vec<&'a BlockState>> = Vec::new();
        let mut index: HashMap<FamilyKey, usize> = HashMap::new();
        let mut seen: HashSet<&BlockState> = HashSet::new();
        for state in states {
            if state.is_air() || !seen.insert(state) {
                continue;
            }
            let key = family_key(assets, state)?;
            match index.get(&key) {
                Some(&i) => groups[i].push(state),
                None => {
                    index.insert(key, groups.len());
                    groups.push(vec![state]);
                }
            }
        }

        let mut fluids: BTreeSet<Fluid> = BTreeSet::new();
        for members in groups {
            let state = members[0];
            let models = models_of(assets, state, None)?;
            for member in &members[1..] {
                assets.skip_like(member, state);
            }
            let pflanze = set.kino.is_some() && assets.bodenpflanze(state)?;
            let Some(family) = set.rastere_familie(assets, state, &models, pflanze) else {
                continue;
            };
            if let Some((fluid, _)) = family.fluid {
                fluids.insert(fluid);
            }
            let index = set.families.len() as u32;
            for member in members {
                set.by_state.insert(member.clone(), index);
            }
            set.families.push(family);
        }

        for fluid in fluids {
            set.insert_strips(assets, fluid);
        }

        // Ob ein Block, den `blocks.txt` nicht kennt, das Licht aufhält, entscheidet
        // das Raster in 2:1 aus der Vorgabe-Richtung, damit das Licht nicht
        // an der Kamera hängt: von oben deckte schon eine flache Platte den
        // ganzen Umriss. 2:1 nimmt nur Vielfache von 4; sonst rastert es
        // beim nächsten darüber.
        if licht_deckend.is_some() {
            set.licht_deckend = licht_deckend;
        } else if projection.kamera() != Kamera::ZWEI_ZU_EINS
            || projection.richtung() != Richtung::default()
        {
            let unbekannt: BTreeSet<BlockState> = seen
                .into_iter()
                .filter(|state| blockstate::Definition::of(state.name()).is_none())
                .cloned()
                .collect();
            if !unbekannt.is_empty() {
                let raster = Projection::new(projection.scale().next_multiple_of(4));
                let zwei = SpriteSet::build_in(assets, &unbekannt, raster)?;
                set.licht_deckend = Some(zwei.licht_deckend(&unbekannt));
            }
        }
        Ok(set)
    }

    /// Nimmt die Blöcke auf, deren Blockentity mit seinen Daten ein anderes
    /// Bild gibt: je Familie und Daten eine eigene Familie, die der
    /// Renderpfad an der Stelle des Blocks nimmt, siehe
    /// [`SpriteSet::variante`]. Nach [`SpriteSet::build_in`] mit denselben
    /// Blockstates. Liefert, was an den Daten unbekannt war und deshalb
    /// fehlt, siehe [`blockentity::unbekannt`].
    /// Siehe docs/renderer/blockentities.md, „Daten aus dem Chunk“.
    pub fn add_entities<'a>(
        &mut self,
        assets: &mut Assets,
        entities: impl IntoIterator<Item = &'a (BlockState, Blockdaten)>,
    ) -> Result<BTreeSet<String>> {
        let mut unbekannt = BTreeSet::new();
        for (state, daten) in entities {
            let Some(basis) = self.family_index(state) else {
                continue;
            };
            let bekannt = self
                .by_entity
                .get(&basis)
                .is_some_and(|varianten| varianten.contains_key(daten));
            if bekannt || !blockentity::aendert(state, daten) {
                continue;
            }
            unbekannt.extend(blockentity::unbekannt(daten, assets));
            let models = models_of(assets, state, Some(daten))?;
            let pflanze = self.kino.is_some() && assets.bodenpflanze(state)?;
            let Some(family) = self.rastere_familie(assets, state, &models, pflanze) else {
                continue;
            };
            // Muster und Scherben liegen auf den Flächen des Modells ohne
            // Daten: Was der Block verdeckt, bleibt gleich, und die Masken
            // der Sections und die Bits für die Sonne dürfen weiter nach
            // der Palette gehen.
            let ohne = self.family(basis);
            debug_assert!(
                (
                    family.opaque,
                    family.covers_floor,
                    family.contained,
                    family.foreign
                ) == (ohne.opaque, ohne.covers_floor, ohne.contained, ohne.foreign),
                "{state}: Daten ändern die Deckung"
            );
            let form = |f: &Family| f.sonne.as_ref().map(|s| (s.wuerfel, s.leer, s.zellen));
            debug_assert!(
                form(&family) == form(ohne),
                "{state}: Daten ändern die Form für die Sonne"
            );
            let index = self.families.len() as u32;
            self.families.push(family);
            self.by_entity
                .entry(basis)
                .or_default()
                .insert(daten.clone(), index);
        }
        Ok(unbekannt)
    }

    /// Die Familie eines Blocks, dessen Blockentity diese Daten trägt, falls
    /// sie sein Bild ändern, siehe [`SpriteSet::add_entities`].
    pub fn variante(&self, family: u32, daten: &Blockdaten) -> Option<u32> {
        self.by_entity.get(&family)?.get(daten).copied()
    }

    /// Rastert die Alternativen einer Blockstate zu einer Familie, `None`,
    /// wenn keine etwas zeichnet. `pflanze`: Der Block ist eine Bodenpflanze
    /// ([`Assets::bodenpflanze`]).
    fn rastere_familie(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        models: &[(u32, BakedModel)],
        pflanze: bool,
    ) -> Option<Family> {
        let fluid = fluid::key(state);
        let nachbarn = blockstate::nachbarregel(state);
        let projection = self.projection;
        let seiten = models
            .iter()
            .flat_map(|(_, model)| &model.quads)
            .filter(|q| faces_camera(q, &projection))
            .filter_map(|q| q.cullface.map(|face| (q, face)))
            .filter(|&(q, face)| {
                nachbarn.is_some_and(|regel| regel.wirkt(face)) || !auf_der_wand(q, face)
            })
            .fold(0, |seiten, (_, face)| seiten | seite(face));
        // Mit Seiten trägt die Familie die Fassungen der Flüssigkeit selbst,
        // zusammen mit denen ohne Flächen zu Nachbarn.
        let alternatives: Vec<(u32, Option<SpriteId>)> = models
            .iter()
            .map(|(weight, model)| {
                let id = self.insert_fluid(assets, state, model, fluid.is_some() && seiten == 0);
                (*weight, id)
            })
            .collect();
        let fassungen = match seiten {
            0 => Vec::new(),
            _ => models
                .iter()
                .map(|(_, model)| {
                    self.insert_nachbarn(assets, state, model, fluid.is_some(), seiten)
                })
                .collect(),
        };
        if alternatives.iter().all(|(_, id)| id.is_none()) {
            return None;
        }
        let entries = || {
            alternatives
                .iter()
                .map(|(_, id)| id.map(|id| &self.sprites[id.0 as usize]))
        };
        let all = |test: fn(&Entry) -> bool| entries().all(|e| e.is_some_and(test));
        // Eine Alternative ohne Bild zeichnet nichts, bleibt also im
        // Wuerfel. Die Fassungen einer Fluessigkeit sind Teile ihres
        // Modells oder dessen voller Wuerfel: sie bleiben, wo das Modell
        // bleibt.
        let contained = entries().all(|e| e.is_none_or(|e| e.contained));
        let foreign =
            entries().any(|e| e.is_some_and(|e| e.parts.iter().any(|(cell, _)| *cell != OWN_CELL)));
        let pure_fluid = fluid.is_some()
            && models
                .iter()
                .all(|(_, model)| model.quads.iter().all(|q| q.fluid.is_some()));
        let sonne = self.kino.is_some().then(|| {
            let lava = fluid.is_some_and(|(art, _)| art == Fluid::Lava);
            let form = Sonnenform::new(
                models,
                lava,
                full_height,
                projection.richtung(),
                assets.textures(),
                &mut self.masken,
                pflanze,
                ohne_null(self.kino.as_ref().expect("Cinematic").sonne()),
            );
            for k in 0..3 {
                self.sonne_reich[0][k] = self.sonne_reich[0][k].min(form.zellen[0][k]);
                self.sonne_reich[1][k] = self.sonne_reich[1][k].max(form.zellen[1][k]);
            }
            form
        });
        Some(Family {
            total: alternatives.iter().map(|(weight, _)| *weight).sum(),
            seed_offset: seed_offset(state),
            opaque: all(|e| e.opaque),
            covers_floor: all(|e| e.covers_floor),
            contained,
            foreign,
            wuerfelform: models
                .iter()
                .all(|(_, model)| auf_den_vorderseiten(model, &projection)),
            pure_fluid,
            fluid,
            resolver: match source_of(state.name()) {
                Some(Source::Biome(resolver)) => Some(resolver),
                _ => None,
            },
            tint_below: tinted_below(state.name(), state.prop("half")),
            doppelkiste: doppelkiste(state).map(|d| projection.richtung().versatz_in_den_blick(d)),
            nachbarn,
            voll: blockstate::volle_seiten(state),
            seiten,
            fassungen,
            alternatives,
            sonne,
        })
    }

    /// Die Farben der Biome für das Zeichnen, mit dem Radius der Mischung
    /// und dem Seed der Welt, siehe [`BiomeTable`].
    pub fn set_biomes(&mut self, biomes: BiomeTable) {
        if let Some(kino) = &mut self.kino {
            kino.mit_biomen(&biomes);
        }
        self.biomes = biomes;
    }

    pub fn biomes(&self) -> &BiomeTable {
        &self.biomes
    }

    /// Womit Cinematic zeichnet; `None` für die Karte.
    pub fn kino(&self) -> Option<&Kino> {
        self.kino.as_ref()
    }

    /// Die Texelmasken für [`Sonnenform::trifft`].
    pub(crate) fn masken(&self) -> &Masken {
        &self.masken
    }

    /// Die Würfel um einen Block, in die irgendein Modell der Tabelle ragt,
    /// relativ zu ihm im Blick, von und bis einschliesslich.
    pub(crate) fn sonne_reich(&self) -> [[i32; 3]; 2] {
        self.sonne_reich
    }

    /// Hält ein Block, den `blocks.txt` nicht kennt, das Licht ganz auf? Wenn sein
    /// Sprite den ganzen Umriss deckt, und zwar im Raster in 2:1 beim scale
    /// der Basis: Eine andere Kamera nimmt die Antwort von dort, eine native
    /// Stufe die der Basis ([`SpriteSet::build_mit_licht`]), damit das Licht
    /// weder an der Kamera noch am scale hängt.
    /// Siehe docs/renderer/wasser-und-licht.md, „Was bleibt eine Näherung“.
    pub fn deckt_fuer_licht(&self, state: &BlockState) -> bool {
        match &self.licht_deckend {
            Some(deckend) => deckend.contains(state),
            None => self
                .family_index(state)
                .is_some_and(|i| self.families[i as usize].opaque),
        }
    }

    /// Die Zustände aus `states`, die `blocks.txt` nicht kennt und die nach dieser
    /// Tabelle das Licht ganz aufhalten, für [`SpriteSet::build_mit_licht`].
    pub fn licht_deckend(&self, states: &BTreeSet<BlockState>) -> HashSet<BlockState> {
        states
            .iter()
            .filter(|state| {
                blockstate::Definition::of(state.name()).is_none() && self.deckt_fuer_licht(state)
            })
            .cloned()
            .collect()
    }

    /// Streifen der Seitenflaechen ueber niedrigeren Nachbarn derselben
    /// Fluessigkeit, je Paar aus eigener Hoehe und Nachbarhoehe in Neunteln
    /// und je Seite im Blick — der Renderer haengt sie an, wo eine
    /// Oberflaeche an eine hoehere Saeule oder eine Stufe fliessenden
    /// Wassers stoesst. Gebaut wird der Streifen der Seite der Welt, die
    /// dort liegt.
    fn insert_strips(&mut self, assets: &mut Assets, fluid: Fluid) {
        let state = fluid.source();
        let richtung = self.projection.richtung();
        for own in 2..=fluid::FULL {
            for below in 1..own {
                for face in [Face::East, Face::South] {
                    let welt = richtung.seite_in_die_welt(face);
                    let model = fluid::strip(assets, fluid, welt, below, own);
                    if let Some(id) = self.insert_tinted(assets, &state, &model) {
                        self.strips.insert((fluid, own, below, face), id);
                    }
                }
            }
        }
    }

    /// Ein Modell mit allen Fassungen: bei einer Fluessigkeit je Maske aus
    /// verdeckten Flaechen eine.
    fn insert_fluid(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        model: &BakedModel,
        has_fluid: bool,
    ) -> Option<SpriteId> {
        let base = self.insert_tinted(assets, state, model)?;
        // Teilen sich zwei Familien das Bild, teilen sie sich auch die
        // Fassungen, und die erste hat sie schon eingetragen: Blasensaeule
        // und Wasser sehen gleich aus.
        if !has_fluid || self.by_mask.contains_key(&base) {
            return Some(base);
        }
        // Steht dieselbe Fluessigkeit darueber, reicht sie bis zur
        // Blockkante (`FlowingFluid.getHeight`); an der Oberflaeche endet
        // sie bei ihrer eigenen Hoehe.
        let voll = full_height(model);
        let richtung = self.projection.richtung();
        let mut variants = vec![None; 8];
        variants[0] = Some(base);
        for mask in 1..8u8 {
            let quelle = if mask & mask_bit(Face::Up) == 0 {
                model
            } else {
                &voll
            };
            // Die Maske nennt Seiten im Blick, das Modell die der Welt.
            let quads = quelle
                .quads
                .iter()
                .filter(|q| {
                    q.fluid.is_none_or(|(_, face)| {
                        mask & mask_bit(richtung.seite_in_den_blick(face)) == 0
                    })
                })
                .cloned()
                .collect();
            variants[mask as usize] = self.insert_tinted(
                assets,
                state,
                &BakedModel {
                    quads,
                    ambient_occlusion: model.ambient_occlusion,
                },
            );
        }
        self.by_mask.insert(base, variants);
        Some(base)
    }

    /// Die Fassungen eines Modells für [`Family::ohne_nachbarn`]: je Maske
    /// der Flüssigkeit, falls der Block eine führt, und je Maske über die
    /// `seiten` eine, ohne die Flächen, deren `cullface` zu einer Seite der
    /// Maske zeigt. Mit Flüssigkeit kommt deren Maske wie in
    /// [`SpriteSet::masked`] dazu. Eintrag 0 ist das Grundbild.
    /// Siehe docs/renderer/sprites-und-deckung.md, „Flächen zu gleichen Nachbarn“.
    fn insert_nachbarn(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        model: &BakedModel,
        has_fluid: bool,
        seiten: u8,
    ) -> Vec<Option<SpriteId>> {
        let voll = full_height(model);
        let richtung = self.projection.richtung();
        let liste: Vec<Face> = SEITEN
            .into_iter()
            .filter(|&face| seiten & seite(face) != 0)
            .collect();
        let je_nachbar = if has_fluid { 8 } else { 1 };
        (0..je_nachbar << liste.len())
            .map(|i| {
                let fluessig = (i % je_nachbar) as u8;
                let weg = liste
                    .iter()
                    .enumerate()
                    .filter(|&(k, _)| (i / je_nachbar) >> k & 1 != 0)
                    .fold(0, |weg, (_, &face)| weg | seite(face));
                let quelle = if fluessig & mask_bit(Face::Up) == 0 {
                    model
                } else {
                    &voll
                };
                let quads = quelle
                    .quads
                    .iter()
                    .filter(|q| {
                        q.fluid.is_none_or(|(_, face)| {
                            fluessig & mask_bit(richtung.seite_in_den_blick(face)) == 0
                        })
                    })
                    .filter(|q| q.cullface.is_none_or(|face| weg & seite(face) == 0))
                    .cloned()
                    .collect();
                self.insert_tinted(
                    assets,
                    state,
                    &BakedModel {
                        quads,
                        ambient_occlusion: model.ambient_occlusion,
                    },
                )
            })
            .collect()
    }

    /// Rastert ein Modell, gefärbte Flächen als Tönungskarte: Die Farbe des
    /// Bioms kommt erst beim Zeichnen dazu, eine feste gleich hier.
    fn insert_tinted(
        &mut self,
        assets: &Assets,
        state: &BlockState,
        model: &BakedModel,
    ) -> Option<SpriteId> {
        // Welche Faerbungen das Modell ueberhaupt traegt. Gezaehlt wird nur,
        // was der Rasterizer zeichnet: ein gefluteter Zaun mitten im Wasser
        // behaelt vom Wasserwuerfel nur die abgewandten Seiten, und die
        // braeuchten sonst eine Karte, die nichts traegt.
        let projection = self.projection;
        let (block, water) = model
            .quads
            .iter()
            .filter(|q| faces_camera(q, &projection))
            .fold((false, false), |(block, water), q| match q.tint_index {
                None => (block, water),
                Some(fluid::TINT_INDEX) => (block, true),
                Some(_) => (true, water),
            });
        let source = source_of(state.name()).filter(|_| block);
        let biome = matches!(source, Some(Source::Biome(_)));
        let fixed = match source {
            Some(Source::Fixed(tint)) => Some(tint),
            _ => None,
        };
        let tints = |block: Tint, wasser: Tint| Tints {
            block: fixed.or(biome.then_some(block)),
            water: water.then_some(wasser),
        };
        const SCHWARZ: Tint = [0; 3];
        const WEISS: Tint = [255; 3];
        let raster = |tints| {
            let (textures, projection) = (assets.textures(), &self.projection);
            rastern(
                model,
                textures,
                projection,
                tints,
                self.licht,
                kollision(state),
                self.kino.is_some(),
            )
        };
        let schwarz = raster(tints(SCHWARZ, SCHWARZ))?;
        let weiss = |ja: bool, tints| {
            ja.then(|| raster(tints).expect("dasselbe Modell, nur anders gefaerbt"))
        };
        let block = weiss(biome, tints(WEISS, SCHWARZ));
        let wasser = weiss(water, tints(SCHWARZ, WEISS));
        // Die Tönungskarte aus den drei Rastern, je Teil aus denselben
        // Fragmenten: Die Farbe ändert weder Füllregel noch Alpha-Test.
        let getoent = |mut sprite: Sprite, block: Option<Sprite>, wasser: Option<Sprite>| {
            if block.is_some() || wasser.is_some() {
                let karte = tint_map(&sprite, block.as_ref(), wasser.as_ref());
                if karte.iter().any(|&w| w != 0) {
                    sprite.tint = Some(karte);
                }
            }
            sprite
        };
        let ganz = schwarz.ganz();
        let zellen = schwarz.zellen();
        let eigen = zellen.iter().all(|&zelle| zelle == OWN_CELL);
        // Was bis auf eine Pixelbreite in seinem Umriss bleibt, bleibt ganz,
        // wie Wandfackeln und Korallenfächer. Von oben hat die Höhe im Bild
        // keine Ausdehnung; dort gilt der Spielraum nur im eigenen Würfel.
        // Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
        let passt =
            fits_cell(&ganz, OWN_CELL, self.projection) && (self.projection.b() > 0.0 || eigen);
        if passt || eigen || zellen.len() > MAX_CELLS {
            let sprite = getoent(
                ganz,
                block.as_ref().map(Raster::ganz),
                wasser.as_ref().map(Raster::ganz),
            );
            return Some(self.insert(vec![(OWN_CELL, sprite)], passt));
        }
        // Die AO-Karte braucht nur das schwarze Raster; die anderen geben
        // nur Farben für die Tönungskarte.
        let mut block = block.map(|r| r.teile(false).into_iter());
        let mut wasser = wasser.map(|r| r.teile(false).into_iter());
        let parts = schwarz
            .teile(ganz.ao.is_some())
            .into_iter()
            .map(|(cell, sprite)| {
                let weiter = |teile: &mut Option<std::vec::IntoIter<(Cell, Sprite)>>| {
                    let (zelle, sprite) = teile.as_mut()?.next()?;
                    debug_assert_eq!(zelle, cell, "dieselben Fragmente");
                    Some(sprite)
                };
                (
                    cell,
                    getoent(sprite, weiter(&mut block), weiter(&mut wasser)),
                )
            })
            .collect();
        Some(self.insert(parts, false))
    }

    /// Nimmt die Teile eines Sprites in die Tabelle auf, siehe
    /// [`Raster::teile`].
    ///
    /// Pixelgleiche Sprites teilen sich den Eintrag: die Maskenfassungen
    /// einer gefluteten oberen Platte sind gleich, wo ihr Wasser in der
    /// deckenden Haelfte liegt, und eine Blasensaeule sieht aus wie Wasser.
    /// Nur fuer Sprites, die in ihren Umriss passen (`passt`) — die
    /// Zerlegung eines ueberhaengenden haengt am Modell, nicht nur am Bild.
    /// Welche Farbe des Bioms eine Tönungskarte trägt, hängt an der Familie,
    /// nicht am Sprite.
    fn insert(&mut self, parts: Vec<(Cell, Sprite)>, passt: bool) -> SpriteId {
        let key = passt.then(|| content_hash(&parts[0].1));
        if let Some(key) = key
            && let Some(ids) = self.by_content.get(&key)
            && let Some(&id) = ids
                .iter()
                .find(|&&id| same_image(&self.sprites[id.0 as usize].parts[0].1, &parts[0].1))
        {
            return id;
        }

        let tints = parts
            .iter()
            .fold(0, |kinds, (_, sprite)| kinds | tint_kinds(sprite));
        let own = parts
            .iter()
            .find(|(cell, _)| *cell == OWN_CELL)
            .map(|(_, sprite)| sprite);
        // Der Boden ist die Oberseite des Blocks darunter, eine Blockhöhe
        // `b` tiefer im Bild; von oben liegt er an derselben Stelle.
        let floor = self.projection.b() as i32;
        let opaque = own.is_some_and(|sprite| covers_all(sprite, &self.masks.outline, 0));
        let covers_floor = own.is_some_and(|sprite| covers_all(sprite, &self.masks.top, floor));
        let contained = own.is_none_or(|sprite| self.masks.contains(sprite));

        self.foreign.extend(
            parts
                .iter()
                .map(|(cell, _)| *cell)
                .filter(|cell| *cell != OWN_CELL),
        );
        let innen = parts.iter().any(|(_, sprite)| ohne_seite(sprite));
        let plaetze = parts
            .iter()
            .fold(0, |acc, (_, sprite)| acc | plaetze(sprite));
        self.sprites.push(Entry {
            parts,
            opaque,
            covers_floor,
            contained,
            rows: OnceLock::new(),
            tints,
            innen,
            plaetze,
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

    /// Hat das Sprite eine AO-Karte, wird es also weich beleuchtet?
    pub fn has_ao(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize]
            .parts
            .iter()
            .any(|(_, sprite)| sprite.ao.is_some())
    }

    /// Hat die AO-Karte des Sprites Pixel ohne Seite, siehe [`Entry::innen`]?
    pub fn innen(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].innen
    }

    /// Welche Plätze der AO-Karte das Sprite zeigt, siehe [`Entry::plaetze`].
    pub fn plaetze(&self, id: SpriteId) -> u8 {
        self.sprites[id.0 as usize].plaetze
    }

    /// Erlaubt das Modell weiche Beleuchtung, siehe [`Sprite::weich`]?
    pub fn weich(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize]
            .parts
            .iter()
            .any(|(_, sprite)| sprite.weich)
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
    /// derselben Fluessigkeit; `mask` traegt je Nachbar +x, +y, +z ein Bit.
    /// `None`, wenn nichts uebrig bleibt — ein Wasserblock mitten im Meer.
    pub fn masked(&self, id: SpriteId, mask: u8) -> Option<SpriteId> {
        match self.by_mask.get(&id) {
            Some(variants) => variants[mask as usize],
            None => Some(id),
        }
    }

    /// Welche Farben seine Tönungskarte trägt, [`TINT_BLOCK`] und
    /// [`TINT_WATER`]; 0 ohne Karte.
    pub fn tints(&self, id: SpriteId) -> u8 {
        self.sprites[id.0 as usize].tints
    }

    /// Wie viele Sprites Fassungen sind: Masken, Streifen und Bilder ohne
    /// Flächen zu Nachbarn — alles, was nicht das Grundbild einer
    /// Alternative ist.
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

    /// Wie [`part`](Self::part), dazu die Zeilenmasken des Teils. Sie
    /// entstehen beim ersten Aufruf, einmal je Sprite; die Grafikkarte
    /// braucht sie nicht.
    pub fn part_rows(&self, id: SpriteId, cell: Cell) -> Option<(&Sprite, &Rows)> {
        let entry = &self.sprites[id.0 as usize];
        let i = entry.parts.iter().position(|(c, _)| *c == cell)?;
        let rows = entry.rows.get_or_init(|| {
            entry
                .parts
                .iter()
                .map(|(_, sprite)| Rows::of(sprite))
                .collect()
        });
        Some((&entry.parts[i].1, &rows[i]))
    }

    /// Der Umriss eines vollen Blocks Zeile für Zeile: je Pixelzeile
    /// relativ zum Blockursprung die erste und die letzte Spalte. Diagonal
    /// ist das Sechseck konvex, genordet ist der Umriss ein Rechteck; hätte
    /// eine Zeile Lücken, verlangte die Deckungsmaske nur mehr, nie weniger.
    pub fn outline_rows(&self) -> &[(i32, i32, i32)] {
        &self.masks.rows
    }

    pub fn is_opaque(&self, id: SpriteId) -> bool {
        self.sprites[id.0 as usize].opaque
    }

    /// Wie weit der Umriss eines vollen Blocks um den Blockursprung reicht,
    /// in Pixeln: kleinstes und grösstes x, dann y. Jedes Sprite, das im
    /// Würfel bleibt (`contained`), liegt darin.
    pub fn outline_box(&self) -> (i32, i32, i32, i32) {
        let pixels = &self.masks.outline;
        let (xs, ys) = (pixels.iter().map(|p| p.0), pixels.iter().map(|p| p.1));
        (
            xs.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            ys.max().unwrap_or(0),
        )
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

    /// Die Lightmap der Dimension, für die die Sprites gebaut sind.
    pub fn lightmap(&self) -> &Lightmap {
        &self.lightmap
    }

    /// Ob diese Dimension Himmelslicht hat (`has_skylight`).
    pub fn himmel(&self) -> bool {
        self.himmel
    }
}

/// Der Umriss eines vollen Blocks ist diagonal ein Sechseck mit den Ecken
/// `(0, -b)`, `(h, a - b)`, `(h, a)`, `(0, 2a)`, `(-h, a)`, `(-h, a - b)`;
/// die vier schraegen Kanten haben die Steigung plus/minus a/h, bei 2:1
/// ein halb. Von oben ist es die Raute der Oberseite. Genordet ist er das
/// Rechteck von `(0, -b)` bis `(h, a)`: die Oberseite, bei `north-45`
/// darunter die Südwand. `slack` dehnt den Umriss nach aussen, negative
/// Werte schrumpfen ihn.
///
/// `px`, `py` sind Pixelmittelpunkte relativ zum Bild der Ecke mit den
/// kleinsten Koordinaten.
fn in_outline(px: f32, py: f32, projection: Projection, slack: f32) -> bool {
    let (h, a, b) = (
        projection.h() as f32,
        projection.a() as f32,
        projection.b() as f32,
    );
    if projection.kamera().genordet() {
        return (-slack..=h + slack).contains(&px) && (-b - slack..=a + slack).contains(&py);
    }
    let (x, m) = (px.abs(), a / h);
    x <= h + slack && py >= m * x - b - slack && py <= 2.0 * a - m * x + slack
}

/// Pixelmittelpunkt relativ zum Blockursprung.
fn pixel_center(sprite: &Sprite, x: u32, y: u32) -> (f32, f32) {
    (
        sprite.offset.0 as f32 + x as f32 + 0.5,
        sprite.offset.1 as f32 + y as f32 + 0.5,
    )
}

/// Der Bildpunkt der Ecke eines Würfels mit den kleinsten Koordinaten,
/// relativ zum Blockursprung: der Ursprung seines Umrisses.
fn cell_origin(cell: Cell, projection: Projection) -> (f32, f32) {
    let (x, y) = projection.project_block(cell);
    (x as f32, y as f32)
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

/// Bild samt AO-Karte und Tönungskarte: Ein Würfel, den das Spiel nicht
/// weich beleuchtet, sieht im Sprite aus wie einer, den es weich beleuchtet,
/// und ein gefärbter Pixel ohne seinen Anteil schwarz wie ein schwarzer.
fn content_hash(sprite: &Sprite) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    sprite.offset.hash(&mut hasher);
    sprite.image.dimensions().hash(&mut hasher);
    sprite.image.as_raw().hash(&mut hasher);
    sprite.ao.hash(&mut hasher);
    sprite.weich.hash(&mut hasher);
    sprite.tint.hash(&mut hasher);
    for g in sprite.geometrie.iter().flatten() {
        g.tiefe.to_bits().hash(&mut hasher);
        g.normale.map(f32::to_bits).hash(&mut hasher);
        g.wasser.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn same_image(a: &Sprite, b: &Sprite) -> bool {
    a.offset == b.offset
        && a.image.dimensions() == b.image.dimensions()
        && a.image.as_raw() == b.image.as_raw()
        && a.ao == b.ao
        && a.weich == b.weich
        && a.tint == b.tint
        && a.geometrie == b.geometrie
}

/// Die Tönungskarte aus drei Rastern desselben Modells: `schwarz` mit
/// Schwarz für jede Farbe aus dem Biom, `block` und `wasser` mit Weiss für
/// die eine und Schwarz für die andere. Mischen und Licht im Sprite sind
/// linear in der Farbe, der Anteil einer Farbe ist also je Kanal der
/// Unterschied zum Raster in Schwarz, und das Raster in Schwarz selbst der
/// Rest. Gekappt, sodass Rest und Anteile zusammen nie über 255 kommen.
/// Siehe docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
fn tint_map(schwarz: &Sprite, block: Option<&Sprite>, wasser: Option<&Sprite>) -> Vec<u32> {
    let rest = schwarz.image.as_raw();
    let mut karte = vec![0u32; rest.len() / 2];
    for (i, pixel) in rest.as_chunks::<4>().0.iter().enumerate() {
        let mut frei = [0, 1, 2].map(|c| 255 - pixel[c] as u32);
        for (slot, weiss) in [block, wasser].into_iter().enumerate() {
            let Some(weiss) = weiss else { continue };
            let hell = &weiss.image.as_raw()[4 * i..][..4];
            for c in 0..3 {
                let anteil = (hell[c] as u32)
                    .saturating_sub(pixel[c] as u32)
                    .min(frei[c]);
                frei[c] -= anteil;
                karte[2 * i + slot] |= anteil << (8 * c);
            }
        }
    }
    karte
}

/// Hat die AO-Karte eines Sprites einen sichtbaren Pixel ohne Seite?
fn ohne_seite(sprite: &Sprite) -> bool {
    sprite.ao.as_ref().is_some_and(|karte| {
        karte
            .iter()
            .zip(sprite.image.pixels())
            .any(|(&w, pixel)| w >> 24 == 0 && pixel.0[3] != 0)
    })
}

/// Die Plätze der AO-Karte eines Sprites mit sichtbaren Pixeln, Bit `p`
/// für Platz `p`.
fn plaetze(sprite: &Sprite) -> u8 {
    sprite.ao.as_ref().map_or(0, |karte| {
        karte
            .iter()
            .zip(sprite.image.pixels())
            .filter(|&(&w, pixel)| w >> 24 != 0 && pixel.0[3] != 0)
            .fold(0, |acc, (&w, _)| acc | 1 << ((w >> 24) - 1))
    })
}

/// Welche Farben die Tönungskarte eines Sprites trägt.
fn tint_kinds(sprite: &Sprite) -> u8 {
    let Some(karte) = &sprite.tint else {
        return 0;
    };
    let traegt = |slot: usize| karte.iter().skip(slot).step_by(2).any(|&w| w != 0);
    (traegt(0) as u8 * TINT_BLOCK) | (traegt(1) as u8 * TINT_WATER)
}

/// Prueft, ob ein Sprite ganz im Umriss eines Wuerfels bleibt, bis auf
/// eine Pixelbreite Toleranz: mehr verschiebt die Rundung beim Rastern
/// nicht.
fn fits_cell(sprite: &Sprite, cell: Cell, projection: Projection) -> bool {
    let (cx, cy) = cell_origin(cell, projection);
    sprite
        .image
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel.0[3] > 0)
        .all(|(x, y, _)| {
            let (px, py) = pixel_center(sprite, x, y);
            in_outline(px - cx, py - cy, projection, 1.0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::model_of;
    use crate::render::rasterizer::over;
    use crate::world::Muster;
    use image::RgbaImage;

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
            contained: true,
            foreign: false,
            wuerfelform: false,
            pure_fluid: false,
            seed_offset: [0, 0, 0],
            resolver: None,
            tint_below: false,
            doppelkiste: None,
            nachbarn: None,
            voll: 0,
            seiten: 0,
            fassungen: Vec::new(),
            sonne: None,
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
                    liste.wahl(pos),
                    Some(soll as usize),
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

    fn build<'a>(
        assets: &mut Assets,
        states: impl IntoIterator<Item = &'a BlockState>,
        projection: Projection,
    ) -> Result<SpriteSet> {
        SpriteSet::build_in(assets, states, projection)
    }

    /// Die Seiten einer Familie, zu denen sie Flächen weglassen kann.
    fn seiten_von(set: &SpriteSet, text: &str) -> Vec<Face> {
        set.family_of(&state(text))
            .unwrap()
            .nachbarseiten()
            .collect()
    }

    /// Eine Scheibe der Fixtures, verbunden zu den genannten Seiten.
    fn scheibe(name: &str, seiten: &[&str], wasser: bool) -> String {
        let an = |seite| seiten.contains(&seite);
        format!(
            "minecraft:{name}[east={},north={},south={},waterlogged={wasser},west={}]",
            an("east"),
            an("north"),
            an("south"),
            an("west")
        )
    }

    /// Eis lässt nach oben, Süden und Osten Flächen weg, die die Kamera
    /// sieht. Ohne Nachbar bleibt das Grundbild, mit allen dreien nichts,
    /// mit einem eine eigene Fassung.
    #[test]
    fn eis_hat_je_maske_eine_fassung() {
        let mut assets = assets();
        let states = [state("minecraft:ice")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(
            seiten_von(&set, "minecraft:ice"),
            [Face::Up, Face::South, Face::East]
        );
        let family = set.family_of(&states[0]).unwrap();
        let base = family.sprite(0).unwrap();
        assert_eq!(family.ohne_nachbarn(0, 0, 0), Some(base));
        assert_eq!(family.ohne_nachbarn(0, 0, 0b111), None);
        let einzeln: HashSet<SpriteId> = (0..3)
            .map(|k| family.ohne_nachbarn(0, 0, 1 << k).unwrap())
            .collect();
        assert_eq!(einzeln.len(), 3);
        assert!(!einzeln.contains(&base));
    }

    /// Die `cullface` dreht sich mit der Variante: Der Arm nach Osten ist der
    /// nach Norden um 90 Grad, sein Ende zeigt nach Osten und lässt seine
    /// Fläche dorthin weg, der um 180 Grad nach Süden. Enden nach Norden und
    /// Westen sieht die Kamera nicht, und der Pfosten hat keine `cullface`.
    #[test]
    fn scheiben_drehen_die_cullface_mit() {
        let mut assets = assets();
        let osten = scheibe("glass_pane", &["east"], false);
        let sueden = scheibe("glass_pane", &["south"], false);
        let hinten = scheibe("glass_pane", &["north", "west"], false);
        let allein = scheibe("glass_pane", &[], false);
        let geflutet = scheibe("glass_pane", &["east"], true);
        let states = [&osten, &sueden, &hinten, &allein, &geflutet].map(|s| state(s));
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(seiten_von(&set, &osten), [Face::East]);
        assert_eq!(seiten_von(&set, &sueden), [Face::South]);
        assert_eq!(seiten_von(&set, &hinten), []);
        assert_eq!(seiten_von(&set, &allein), []);
        assert!(!set.family_of(&states[3]).unwrap().hat_nachbarn());
        // Mit Wasser je Maske der Flüssigkeit und je Maske der Seiten eine.
        let family = set.family_of(&states[4]).unwrap();
        assert_eq!(family.fassungen[0].len(), 8 << 1);

        let models = models_of(&mut assets, &states[0], None).unwrap();
        let enden: Vec<&Quad> = models[0]
            .1
            .quads
            .iter()
            .filter(|q| q.cullface.is_some())
            .collect();
        assert_eq!(enden.len(), 1);
        assert_eq!(enden[0].cullface, Some(Face::East));
        assert!(enden[0].normal()[0] > 0.0, "die Fläche zeigt nach Osten");
    }

    /// Vor einem vollen Nachbarn zählen nur Seiten, deren Flächen er nicht
    /// übermalt. Der Spawner der Fixtures hat das innere Element von
    /// `cube_all_inner_faces` aus 26.2, in x von 15,998 nach 0,002: Seine
    /// Flächen zeigen nach innen, die Wände in z mit der `cullface` der Wand
    /// gegenüber, die übrigen mit der eigenen. Die Kamera sieht Boden und
    /// zwei Wände, deren `cullface` aus `se` unten, Süden und Westen sind,
    /// aus `sw` unten, Süden und Osten, aus `nw` unten, Norden und Osten, aus
    /// `ne` unten, Norden und Westen; von oben nur den Boden. Ein voller
    /// Würfel hat jede Fläche auf ihrer Wand und keine Seite. Bei den Wurzeln
    /// liegt die Schicht im Osten auf ihrer Wand, oben und unten zählen über
    /// die Regel.
    #[test]
    fn seiten_vor_vollen_nachbarn() {
        let mut assets = assets();
        let (spawner, stein) = ("minecraft:spawner", "minecraft:stone");
        let wurzeln = "minecraft:mangrove_roots[waterlogged=false]";
        let states = [spawner, stein, wurzeln].map(state);
        let aus = |kamera: &str, richtung: &str| {
            let kamera = Kamera::parse(kamera).unwrap();
            Projection::mit_kamera(16, kamera).aus(Richtung::parse(richtung, kamera).unwrap())
        };
        for (projection, seiten) in [
            (
                aus("2:1", "se"),
                [Face::Down, Face::South, Face::West].as_slice(),
            ),
            (aus("2:1", "sw"), &[Face::Down, Face::South, Face::East]),
            (aus("2:1", "nw"), &[Face::Down, Face::North, Face::East]),
            (aus("2:1", "ne"), &[Face::Down, Face::North, Face::West]),
            (aus("top", "se"), &[Face::Down]),
        ] {
            let set = build(&mut assets, &states, projection).unwrap();
            assert_eq!(seiten_von(&set, spawner), seiten, "{projection:?}");
            assert_eq!(seiten_von(&set, stein), [], "{projection:?}");
        }
        let set = build(&mut assets, &states, aus("2:1", "se")).unwrap();
        assert_eq!(seiten_von(&set, wurzeln), [Face::Down, Face::Up]);
    }

    /// Wo ein Block voll deckt, gehört zum Schlüssel seiner Familie. Das
    /// Pack `assets-platten` zeichnet jede Platte mit dem Modell der unteren;
    /// die obere deckt trotzdem nach oben, die untere nach unten, und beide
    /// haben dieselbe Kollisionsform.
    #[test]
    fn volle_seiten_trennen_familien() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let packs = vec![
            fixtures.join("assets-base"),
            fixtures.join("assets-platten"),
        ];
        let mut assets = Assets::open(packs).unwrap();
        let oben = state("minecraft:oak_slab[type=top,waterlogged=false]");
        let unten = state("minecraft:oak_slab[type=bottom,waterlogged=false]");
        let set = build(&mut assets, [&oben, &unten], Projection::new(16)).unwrap();
        assert_eq!(set.family_of(&oben).unwrap().voll, seite(Face::Up));
        assert_eq!(set.family_of(&unten).unwrap().voll, seite(Face::Down));
    }

    /// Mit den sechs Schichten aus 26.2 (Pack `assets-wurzeln`) haben
    /// Mangrovenwurzeln aus jeder Richtung zwei waagrechte Seiten: die
    /// inneren Flächen der hinteren Schichten, 0,002 vor ihrer Wand. Aus `se`
    /// Norden und Westen. Oben und unten zählen über die Regel. Das gibt je
    /// Alternative 16 Fassungen, geflutet 128.
    #[test]
    fn wurzeln_mit_den_schichten_aus_26_2() {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let packs = vec![
            fixtures.join("assets-base"),
            fixtures.join("assets-wurzeln"),
        ];
        let mut assets = Assets::open(packs).unwrap();
        let trocken = "minecraft:mangrove_roots[waterlogged=false]";
        let states = [trocken, "minecraft:mangrove_roots[waterlogged=true]"].map(state);
        let kamera = Kamera::parse("2:1").unwrap();
        for (richtung, waagrecht) in [
            ("se", [Face::North, Face::West]),
            ("sw", [Face::North, Face::East]),
            ("nw", [Face::South, Face::East]),
            ("ne", [Face::South, Face::West]),
        ] {
            let projection =
                Projection::mit_kamera(16, kamera).aus(Richtung::parse(richtung, kamera).unwrap());
            let set = build(&mut assets, &states, projection).unwrap();
            let seiten = [[Face::Down, Face::Up], waagrecht].concat();
            assert_eq!(seiten_von(&set, trocken), seiten, "{richtung}");
            let fassungen = states
                .each_ref()
                .map(|s| set.family_of(s).unwrap().fassungen[0].len());
            assert_eq!(fassungen, [16, 128], "{richtung}");
        }
    }

    /// Nur eine Fläche auf der Wand zu ihrer Seite oder dahinter, die dorthin
    /// zeigt, übermalt ein voller Nachbar dort: die Nordseite eines Würfels
    /// zum Norden, auch 0,02 hinter der Wand wie der Sockel eines Hebels in
    /// 26.2. Nach innen gewendet nicht, 0,002 vor der Wand wie die Wände
    /// eines Spawners auch nicht.
    #[test]
    fn nur_flaechen_auf_der_wand_uebermalt_der_nachbar() {
        let nord = |z: f32| {
            box_quads([0.0, 0.0, z], [16.0; 3], Textures::MISSING, None, None)
                .find(|q| q.normal()[2] < 0.0)
                .unwrap()
        };
        let aussen = nord(0.0);
        assert!(auf_der_wand(&aussen, Face::North));
        assert!(!auf_der_wand(&aussen, Face::South));
        let mut innen = aussen.clone();
        innen.corners.reverse();
        assert!(!auf_der_wand(&innen, Face::North), "nach innen gewendet");
        assert!(!auf_der_wand(&nord(0.002), Face::North), "vor der Wand");
        assert!(auf_der_wand(&nord(-0.02), Face::North), "hinter der Wand");
    }

    /// Mangrovenwurzeln lassen zu sich selbst nur oben und unten weg, auch über die innere
    /// Fläche mit `cullface` unten, die zur Kamera zeigt. Mit Wasser im Block
    /// gibt es 8 Masken der Flüssigkeit je Maske der zwei Seiten.
    #[test]
    fn wurzeln_nur_senkrecht() {
        let mut assets = assets();
        let states = [
            state("minecraft:mangrove_roots[waterlogged=false]"),
            state("minecraft:mangrove_roots[waterlogged=true]"),
        ];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(
            seiten_von(&set, "minecraft:mangrove_roots[waterlogged=false]"),
            [Face::Down, Face::Up]
        );
        let geflutet = set.family_of(&states[1]).unwrap();
        assert_eq!(geflutet.fassungen[0].len(), 32);
        // Oben und unten ein Nachbar, mitten im Wasser: Es bleibt die Seite
        // nach Osten, und sie ist ein anderes Bild als ohne die Nachbarn.
        let innen = geflutet.ohne_nachbarn(0, 7, 0b11).unwrap();
        assert_ne!(Some(innen), geflutet.ohne_nachbarn(0, 7, 0));
    }

    /// Zu welchen Nachbarn ein Block Flächen weglässt, gehört zum Schlüssel
    /// der Familie. Ein Pack, das die Verbindungen eines Gitters nicht
    /// zeichnet, gibt allen Zuständen dasselbe Modell, aber nicht dieselben
    /// Seiten: Nur verbunden lässt das Gitter waagrecht etwas weg.
    #[test]
    fn verbindungen_trennen_familien() {
        let pack = tempfile::tempdir().unwrap();
        let dir = pack.path().join("minecraft/blockstates");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("iron_bars.json"),
            r#"{ "variants": { "": { "model": "minecraft:block/eis" } } }"#,
        )
        .unwrap();
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        let mut assets = Assets::open(vec![base, pack.path().to_path_buf()]).unwrap();
        let ost = scheibe("iron_bars", &["east"], false);
        let ohne = scheibe("iron_bars", &[], false);
        let states = [state(&ost), state(&ohne)];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(seiten_von(&set, &ost), [Face::Up, Face::East]);
        assert_eq!(seiten_von(&set, &ohne), [Face::Up]);
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
    /// deckend gelten, auch nicht bei scale 2 und 3, wo sein Umriss nur
    /// vier und sechs Pixel hat.
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

    /// Gegenprobe: ein voller Würfel deckt auf jeder Stufe ab, auch bei
    /// scale 2 und 3.
    #[test]
    fn voller_wuerfel_deckt_auf_jeder_stufe_ab() {
        for scale in [2, 3, 4, 16, 64] {
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
            assert!(set.sprites[id.0 as usize].contained, "{name}");
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
        let entry = &set.sprites[id.0 as usize];
        assert!(
            entry
                .parts
                .iter()
                .all(|(cell, sprite)| fits_cell(sprite, *cell, set.projection)),
            "nach der Zerlegung bleibt jeder Teil in seinem Würfel"
        );
        assert!(
            entry.contained,
            "im Raum zugeordnet bleibt der eigene Teil in seinem Umriss"
        );
    }

    /// Ob ein Block, den `blocks.txt` nicht kennt, das Licht aufhält, entscheidet bei
    /// jeder Kamera das Raster in 2:1: Von oben deckte schon eine flache
    /// Seerose den ganzen Umriss, und ihr Würfel bliebe dunkel. Bei einem
    /// scale, den 2:1 nicht nimmt, etwa 6 oder ungerade, rastert es beim
    /// nächsten Vielfachen von 4 darüber.
    #[test]
    fn licht_unbekannter_bloecke_haengt_nicht_an_der_kamera() {
        let namen = [
            "einfarbig",
            "seerose",
            "laub",
            "ackerboden",
            "untere_platte",
            "teppich",
        ];
        let states: Vec<BlockState> = namen.iter().map(|name| state(name)).collect();
        let zwei = |scale| build(&mut assets(), &states, Projection::new(scale)).unwrap();
        let (zwei_4, zwei_8, zwei_16, zwei_32) = (zwei(4), zwei(8), zwei(16), zwei(32));
        assert!(zwei_16.deckt_fuer_licht(&state("einfarbig")));
        assert!(!zwei_16.deckt_fuer_licht(&state("seerose")));
        for (kamera, scale, vergleich) in [
            ("4:3", 16, &zwei_16),
            ("1:1", 16, &zwei_16),
            ("top", 16, &zwei_16),
            ("5:3", 30, &zwei_32),
            ("top-north", 16, &zwei_16),
            ("north-45", 16, &zwei_16),
            ("1:1", 6, &zwei_8),
            ("top", 6, &zwei_8),
            ("top-north", 4, &zwei_4),
            ("top-north", 6, &zwei_8),
            ("top-north", 7, &zwei_8),
            ("north-45", 5, &zwei_8),
            ("north-45", 6, &zwei_8),
            ("north-45", 7, &zwei_8),
            ("north-45", 8, &zwei_8),
        ] {
            let projection = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
            let set = build(&mut assets(), &states, projection).unwrap();
            for st in &states {
                assert_eq!(
                    set.deckt_fuer_licht(st),
                    vergleich.deckt_fuer_licht(st),
                    "{kamera} bei {scale}, {st:?}"
                );
            }
        }
    }

    /// Ein fremdes Teil kommt vor einen Block, dessen Flächen alle auf den
    /// Vorderseiten seines Würfels liegen: ein voller Würfel, Laub mit
    /// Löchern, ein Grasblock mit Overlay. Ackerboden endet darunter, Schleim
    /// hat einen Würfel darin, Feuer steht quer im Würfel. Eine Familie nur,
    /// wenn jede Alternative die Bedingung erfüllt, also nicht aus vollem
    /// Würfel und Ackerboden gemischt. Die Vorderseiten liegen im Blick.
    #[test]
    fn wuerfelform_nur_mit_flaechen_auf_den_vorderseiten() {
        let mut assets = assets();
        let namen = [
            ("einfarbig", true),
            ("laub", true),
            ("mit_overlay", true),
            ("ackerboden", false),
            ("schleim", false),
            ("hochfeuer", false),
            ("seerose", false),
            ("gemischt", false),
        ];
        let states: Vec<BlockState> = namen.iter().map(|(name, _)| state(name)).collect();
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        for (name, soll) in namen {
            let family = set.family_of(&state(name)).unwrap();
            assert_eq!(family.wuerfelform, soll, "{name}");
        }
        // Nach den Seiten im Blick: Der Kerbe fehlt die untere Ecke im
        // Nordwesten. Aus Südosten liegt alles, was die Kamera sieht, auf den
        // Vorderseiten, aus Nordwesten auch eine Fläche der Lücke im Innern.
        for (richtung, soll) in [("se", true), ("nw", false)] {
            let richtung = Richtung::parse(richtung, Kamera::ZWEI_ZU_EINS).unwrap();
            let kerbe = state("kerbe");
            let set = build(&mut assets, [&kerbe], Projection::new(16).aus(richtung)).unwrap();
            let family = set.family_of(&kerbe).unwrap();
            assert_eq!(family.wuerfelform, soll, "Kerbe aus {richtung:?}");
        }
    }

    /// Ein Teil ohne Fläche auf dem Rand behält die AO-Karte des Modells:
    /// Seine Fläche nach Süden liegt im Innern, Platz 4, gezählt ab der
    /// Zelle des Blocks. Ohne Seite ist kein Pixel, `innen` gilt nicht.
    #[test]
    fn teil_ohne_seite_liegt_im_innern_des_blocks() {
        let mut assets = assets();
        let set = build(&mut assets, &[state("blech")], Projection::new(16)).unwrap();
        let id = set.id(&state("blech")).unwrap();
        let entry = &set.sprites[id.0 as usize];
        let cells: Vec<Cell> = entry.parts.iter().map(|(cell, _)| *cell).collect();
        assert_eq!(cells, [[-1, 0, 0], OWN_CELL]);
        assert!(
            !ohne_seite(&entry.parts[1].1),
            "der eigene Teil hat überall eine Seite"
        );
        assert_eq!(plaetze(&entry.parts[0].1), 1 << 4, "das Blech");
        assert_eq!(set.plaetze(id), 0b10111);
        assert!(!set.innen(id));
    }

    /// Der Umriss folgt der Kamera: Ein voller Würfel passt bei jeder in
    /// seinen eigenen, ein Modell, das zur Seite hinausragt, bei keiner. Ein
    /// Turm doppelter Höhe passt nur von oben, wo die Höhe nicht ins Bild
    /// geht, auch genordet.
    #[test]
    fn umriss_folgt_der_kamera() {
        let mut assets = assets();
        for (kamera, scale) in [
            ("2:1", 32),
            ("8:5", 32),
            ("4:3", 32),
            ("1:1", 32),
            ("top", 32),
            ("5:3", 30),
            ("top-north", 16),
            ("north-45", 16),
            ("north-45", 7),
        ] {
            let projection = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
            let oben = projection.b() == 0.0;
            for (name, passt) in [("einfarbig", true), ("ueberhang", false), ("turm", oben)] {
                let model = model_of(&mut assets, &state(name)).unwrap();
                let bild =
                    render(&model, assets.textures(), &projection, Tints::default()).unwrap();
                assert_eq!(
                    fits_cell(&bild, OWN_CELL, projection),
                    passt,
                    "{name}, {kamera}"
                );
            }
        }
    }

    /// Beim Verdecken zählt ein Nachbar nach +x oder +z genau dann, wenn
    /// sein Umriss den eigenen überlappt: diagonal schräg beide, genordet
    /// schräg nur der nach +z, von oben keiner. Geprüft an Pixelmitten, um
    /// ein Viertel Pixel geschrumpft, damit Umrisse, die sich nur berühren,
    /// nicht zählen.
    #[test]
    fn verdeckende_seiten_ueberlappen_den_umriss() {
        for (kamera, scale) in [
            ("2:1", 32),
            ("4:3", 32),
            ("1:1", 32),
            ("top", 32),
            ("5:3", 30),
            ("top-north", 16),
            ("north-45", 16),
            ("north-45", 7),
        ] {
            let projection = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
            let ueberlappt = |nachbar: [i32; 3]| {
                let (dx, dy) = projection.project_block(nachbar);
                let s = 2 * scale as i32;
                (-s..s).any(|y| {
                    (-s..s).any(|x| {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        in_outline(px, py, projection, -0.25)
                            && in_outline(px - dx as f32, py - dy as f32, projection, -0.25)
                    })
                })
            };
            assert_eq!(
                projection.verdeckende_seiten(),
                (ueberlappt([1, 0, 0]), ueberlappt([0, 0, 1])),
                "{kamera} bei {scale}"
            );
        }
    }

    /// Spielraum: Was bis auf eine Pixelbreite in seinen Umriss passt, bleibt
    /// ganz, und erst was darüber hinausragt, zerfällt. Ein Rand von 0,5/16
    /// Block um den Würfel bleibt bis scale 32 ganz, Getreide mit 1/16 Block
    /// in seinem Boden bis scale 16; bei scale 64 zerfallen beide.
    #[test]
    fn knapper_ueberstand_bleibt_ganz() {
        let mut ganz_bis = std::collections::HashMap::new();
        for scale in (4..=64).step_by(4) {
            let mut assets = assets();
            let projection = Projection::new(scale);
            for name in ["rand", "getreide"] {
                let set = build(&mut assets, &[state(name)], projection).unwrap();
                let id = set.id(&state(name)).unwrap();
                let parts = &set.sprites[id.0 as usize].parts;
                let model = model_of(&mut assets, &state(name)).unwrap();
                let bild =
                    render(&model, assets.textures(), &projection, Tints::default()).unwrap();
                let passt = fits_cell(&bild, OWN_CELL, projection);
                assert_eq!(parts.len() == 1, passt, "{name}, scale {scale}");
                if passt {
                    ganz_bis.insert(name, scale);
                }
            }
        }
        assert!(ganz_bis["rand"] >= 32, "{ganz_bis:?}");
        assert!(ganz_bis["getreide"] >= 16, "{ganz_bis:?}");
        assert!(ganz_bis.values().all(|&bis| bis < 64), "{ganz_bis:?}");
    }

    /// Ein zwei Blöcke hohes Modell ebenso — der obere Teil gehört in den
    /// Würfel darüber, sonst wird er zu früh gezeichnet. Bei jedem scale
    /// genau in die Würfel seiner Geometrie, ohne Splitter in einem
    /// Nachbarwürfel an den eigenen Kanten, auch wo ein Modell kaum
    /// hinausragt wie Feuer.
    #[test]
    fn hohes_modell_zerfaellt_nach_oben() {
        for scale in (4..=64).step_by(4) {
            let mut assets = assets();
            let projection = Projection::new(scale);
            for (name, soll) in [
                ("turm", [OWN_CELL, [0, 1, 0]]),
                ("hochfeuer", [OWN_CELL, [0, 1, 0]]),
                ("ueberhang", [[-1, 0, 0], OWN_CELL]),
            ] {
                let set = build(&mut assets, &[state(name)], projection).unwrap();
                let id = set.id(&state(name)).unwrap();
                let cells: Vec<Cell> = set.sprites[id.0 as usize]
                    .parts
                    .iter()
                    .map(|(cell, _)| *cell)
                    .collect();
                let model = model_of(&mut assets, &state(name)).unwrap();
                let bild =
                    render(&model, assets.textures(), &projection, Tints::default()).unwrap();
                if fits_cell(&bild, OWN_CELL, projection) {
                    assert_eq!(cells, [OWN_CELL], "{name}, scale {scale}");
                } else {
                    assert_eq!(cells, soll, "{name}, scale {scale}");
                }
                assert!(
                    set.sprites[id.0 as usize]
                        .parts
                        .iter()
                        .all(|(cell, sprite)| fits_cell(sprite, *cell, projection)),
                    "{name}, scale {scale}"
                );
            }
        }
    }

    /// Liegen alle Fragmente in einem einzigen fremden Würfel, wird das
    /// Modell ein Teil dort, nicht ein ganzes im eigenen: wie die Stiele und
    /// Karotten von Vanilla bei manchen scales im Würfel darunter. Ganz
    /// bleibt es nur im Spielraum einer Pixelbreite, schräg; von oben nie.
    /// Bei jeder Kamera und jedem scale.
    #[test]
    fn ein_fremder_wuerfel_wird_ein_teil() {
        let kameras = ["2:1", "8:5", "4:3", "1:1", "top", "top-north", "north-45"]
            .map(|k| Kamera::parse(k).unwrap());
        let projektionen = (4..=64)
            .step_by(2)
            .flat_map(|scale| kameras.map(|k| Projection::mit_kamera(scale, k)))
            .filter(Projection::ganze_pixel);
        let mut zerfallen = Vec::new();
        for projection in projektionen {
            let (kamera, scale) = (projection.kamera(), projection.scale());
            let mut assets = assets();
            let set = build(&mut assets, &[state("unter_dem_boden")], projection).unwrap();
            let id = set.id(&state("unter_dem_boden")).unwrap();
            let cells: Vec<Cell> = set.sprites[id.0 as usize]
                .parts
                .iter()
                .map(|(cell, _)| *cell)
                .collect();
            let model = model_of(&mut assets, &state("unter_dem_boden")).unwrap();
            let bild = render(&model, assets.textures(), &projection, Tints::default()).unwrap();
            let spielraum = projection.b() > 0.0 && fits_cell(&bild, OWN_CELL, projection);
            let soll = if spielraum { OWN_CELL } else { [0, -1, 0] };
            assert_eq!(cells, [soll], "{kamera}, scale {scale}");
            if !spielraum {
                zerfallen.push((kamera.to_string(), scale));
            }
        }
        assert!(
            zerfallen.contains(&("2:1".to_string(), 64)),
            "{zerfallen:?}"
        );
        assert!(zerfallen.contains(&("top".to_string(), 4)), "{zerfallen:?}");
    }

    /// Von oben hat die Höhe im Bild keine Ausdehnung: Ein Turm doppelter
    /// Höhe passt in seinen Umriss, zerfällt aber trotzdem. Zu sehen ist nur
    /// seine Oberseite, und die liegt im Würfel darüber. Genordet ebenso.
    #[test]
    fn von_oben_zerfaellt_der_turm() {
        for kamera in [Kamera::Oben, Kamera::ObenNord] {
            for scale in (4..=64).step_by(2) {
                let mut assets = assets();
                let projection = Projection::mit_kamera(scale, kamera);
                let set = build(&mut assets, &[state("turm")], projection).unwrap();
                let id = set.id(&state("turm")).unwrap();
                let cells: Vec<Cell> = set.sprites[id.0 as usize]
                    .parts
                    .iter()
                    .map(|(cell, _)| *cell)
                    .collect();
                assert_eq!(cells, [[0, 1, 0]], "{kamera}, scale {scale}");
            }
        }
    }

    /// Ein Modell, das knapp über seinen Würfel ragt, zerfällt nicht, liegt
    /// aber auch nicht im Umriss: seine Randpixel deckt kein Nachbar, und
    /// verdeckt fallen darf es deshalb nie. So liegen Schilder, Weizen oder
    /// Schienen in Vanilla.
    #[test]
    fn knapper_ueberstand_liegt_nicht_im_umriss() {
        let mut assets = assets();
        let states = [state("rand"), state("einfarbig")];
        for scale in [16, 32] {
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
            let rand = set.id(&state("rand")).unwrap();
            let einfarbig = set.id(&state("einfarbig")).unwrap();
            assert!(set.foreign_cells().is_empty(), "scale {scale}: zerfallen");
            assert!(!set.sprites[rand.0 as usize].contained, "scale {scale}");
            assert!(set.sprites[einfarbig.0 as usize].contained, "scale {scale}");
        }
    }

    /// Im Umriss liegt ein Sprite, das ihn genau füllt, auch mit einem
    /// durchsichtigen Pixel darin. Ein einziger sichtbarer daneben genügt,
    /// und es liegt nicht mehr darin.
    #[test]
    fn ein_pixel_neben_dem_umriss_genuegt() {
        use image::Rgba;
        let masks = Masks::new(&Textures::new(), Projection::new(16));
        let x0 = masks.outline.iter().map(|p| p.0).min().unwrap();
        let y0 = masks.outline.iter().map(|p| p.1).min().unwrap();
        let x1 = masks.outline.iter().map(|p| p.0).max().unwrap();
        let y1 = masks.outline.iter().map(|p| p.1).max().unwrap();
        let offset = (x0 - 1, y0 - 1);
        let mut image = RgbaImage::new((x1 - x0 + 3) as u32, (y1 - y0 + 3) as u32);
        let stelle = |(x, y): (i32, i32)| ((x - offset.0) as u32, (y - offset.1) as u32);
        for &pos in &masks.outline {
            let (x, y) = stelle(pos);
            image.put_pixel(x, y, Rgba([9, 9, 9, 255]));
        }
        let mut sprite = Sprite {
            image,
            offset,
            ao: None,
            weich: false,
            tint: None,
            geometrie: None,
        };
        assert!(masks.contains(&sprite));
        let (x, y) = stelle(masks.outline[0]);
        sprite.image.put_pixel(x, y, Rgba([0; 4]));
        assert!(masks.contains(&sprite));
        sprite.image.put_pixel(0, 0, Rgba([9, 9, 9, 1]));
        assert!(!masks.contains(&sprite));
    }

    /// Keine Naht: Die Teile eines Modells, in der Reihenfolge ihrer Würfel
    /// übereinander gelegt, sind ohne Nachbarn Pixel für Pixel das ganze
    /// Modell, bei jedem scale.
    #[test]
    fn zerlegtes_modell_ist_ohne_nachbarn_das_ganze() {
        let kameras = ["2:1", "8:5", "4:3", "1:1", "top", "top-north", "north-45"]
            .map(|k| Kamera::parse(k).unwrap());
        let projektionen = (4..=64)
            .step_by(2)
            .flat_map(|scale| kameras.map(|k| Projection::mit_kamera(scale, k)))
            .filter(Projection::ganze_pixel);
        for projection in projektionen {
            let (scale, kamera) = (projection.scale(), projection.kamera());
            for name in ["turm", "ueberhang", "hochfeuer", "einfarbig", "seerose"] {
                let mut assets = assets();
                let model = model_of(&mut assets, &state(name)).unwrap();
                let Some(raster) = rastern(
                    &model,
                    assets.textures(),
                    &projection,
                    Tints::default(),
                    CardinalLight::Default,
                    false,
                    false,
                ) else {
                    // Von oben steht Feuer ganz auf der Kante.
                    assert!(
                        name == "hochfeuer" && projection.b() == 0.0,
                        "{name}, {kamera}, scale {scale}"
                    );
                    continue;
                };
                let ganz = raster.ganz();
                let mut teile = raster.teile(ganz.ao.is_some());
                // Wie die Kandidaten: nach Höhe, Tiefe, Spalte.
                teile.sort_by_key(|&([x, y, z], _)| {
                    let (u, v) = projection.uv(x, z);
                    (y, v, u)
                });
                let mut bild = RgbaImage::new(ganz.image.width(), ganz.image.height());
                for (_, teil) in &teile {
                    for (x, y, pixel) in teil.image.enumerate_pixels() {
                        let (bx, by) = (
                            (teil.offset.0 + x as i32 - ganz.offset.0) as u32,
                            (teil.offset.1 + y as i32 - ganz.offset.1) as u32,
                        );
                        let unten = bild.get_pixel(bx, by).0;
                        bild.put_pixel(bx, by, image::Rgba(over(pixel.0, unten)));
                    }
                }
                assert_eq!(
                    bild.as_raw(),
                    ganz.image.as_raw(),
                    "{name}, {kamera}, scale {scale}"
                );
            }
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
    /// sie brauchen keine Tönungskarte.
    #[test]
    fn abgewandte_flaechen_faerben_nicht() {
        let mut assets = assets();
        let states = [state("oak_fence[waterlogged=true]")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let base = set.families[0].alternatives[0].1.unwrap();
        assert_eq!(set.tints(base), TINT_WATER, "Wasser sichtbar");
        let innen = set.by_mask[&base][7].unwrap();
        assert_eq!(set.tints(innen), 0, "Maske 7 zeigt kein Wasser");
        assert!(set.part(innen, OWN_CELL).unwrap().tint.is_none());
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
            assert_eq!(family.wahl(pos), Some(erwartet[2] as usize), "{pos:?}");
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

    /// Die Tönungskarte gibt das Bild in jeder Farbe wieder: [`tinted`]
    /// mit einer Farbe des Blocks und einer des Wassers gleicht bis auf die
    /// Rundung dem Raster, das die Farben gleich trägt, beide ohne Licht:
    /// beim Wasser mit seiner halb durchsichtigen Oberfläche, beim Grasblock
    /// der Fixture mit gefärbter Oberseite und ungefärbten Seiten, bei einem
    /// gefluteten Zaun, bei einem gefluteten gefärbten Kreuz, in dessen
    /// Pixeln sich beide Farben treffen, und bei einem gefluteten
    /// Sculk-Sensor. Das Raster rundet an jeder Schicht, die Karte einmal je
    /// Pixel; auseinander liegen sie höchstens um 1, siehe
    /// docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
    ///
    /// [`tinted`]: super::super::rasterizer::tinted
    #[test]
    fn toenungskarte_gibt_jede_farbe_wieder() {
        use super::super::rasterizer::{pack, tinted};
        let mut assets = assets();
        let texte = [
            "water[level=0]",
            "grass_block",
            "oak_fence[waterlogged=true]",
            "jungle_leaves[distance=1,persistent=false,waterlogged=true]",
            "sculk_sensor[power=0,sculk_sensor_phase=active,waterlogged=true]",
        ];
        let states: Vec<BlockState> = texte.iter().map(|t| state(t)).collect();
        for scale in [4, 16, 32] {
            let projection = Projection::new(scale);
            let set = build(&mut assets, &states, projection).unwrap();
            for st in &states {
                let id = set.id(st).unwrap();
                let sprite = set.part(id, OWN_CELL).unwrap();
                let karte = sprite.tint.as_ref().expect("Tönungskarte");
                let model = model_of(&mut assets, st).unwrap();
                let gefaerbt = source_of(st.name()).is_some();
                for (block, wasser) in [
                    ([255, 255, 255], [255, 255, 255]),
                    ([0x91, 0xBD, 0x59], [0x3F, 0x76, 0xE4]),
                    ([7, 0, 250], [250, 7, 0]),
                    ([0, 0, 0], [0, 255, 0]),
                ] {
                    let direkt = render(
                        &model,
                        assets.textures(),
                        &projection,
                        Tints {
                            block: gefaerbt.then_some(block),
                            water: Some(wasser),
                        },
                    )
                    .unwrap();
                    assert_eq!(direkt.image.dimensions(), sprite.image.dimensions());
                    let farben = [pack(block), pack(wasser)];
                    let mut beide = false;
                    for (i, (ist, soll)) in
                        sprite.image.pixels().zip(direkt.image.pixels()).enumerate()
                    {
                        beide |= karte[2 * i] != 0 && karte[2 * i + 1] != 0;
                        let ist = tinted(ist.0, [karte[2 * i], karte[2 * i + 1]], farben);
                        assert_eq!(ist[3], soll.0[3], "{st:?}, scale {scale}: Alpha");
                        for c in 0..3 {
                            let d = (ist[c] as i32 - soll.0[c] as i32).abs();
                            assert!(
                                d <= 1,
                                "{st:?}, scale {scale}, Farben {block:?} und {wasser:?}, \
                                 Pixel {i}: {ist:?} gegen {:?}",
                                soll.0
                            );
                        }
                    }
                    if st.name() == "minecraft:jungle_leaves" {
                        assert!(beide, "scale {scale}: kein Pixel mit beiden Farben");
                    }
                }
            }
        }
    }

    /// Die Tönungskarte an allen Vanilla-Blöcken, die gefärbt oder geflutet
    /// sein können: je Block aus `blocks.txt` bis zu 24 Zustände, die ersten
    /// und die letzten zwölf, geflutete immer mit Wasser, bei scale 4, 8, 12,
    /// 16, 24, 32 und 48, mit drei Paaren aus
    /// Block- und Wasserfarbe, gegen das Raster, das die Farben gleich trägt,
    /// beide ohne Licht. Braucht die Asset-Wurzeln wie `--assets`, als
    /// Pfadliste in `ASSETS`, deshalb `#[ignore]`; unter Windows trennt `;`:
    ///
    /// ```bash
    /// ASSETS="$PWD/vanilla-assets:$PWD/assets" cargo test --release --manifest-path renderer/Cargo.toml --lib toenungskarte_an_allen_vanilla_bloecken -- --ignored --nocapture
    /// ```
    ///
    /// Siehe docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
    #[test]
    #[ignore]
    fn toenungskarte_an_allen_vanilla_bloecken() {
        use super::super::rasterizer::{pack, tinted};
        let wurzeln = std::env::var_os("ASSETS").expect("ASSETS auf die Asset-Wurzeln setzen");
        let mut assets = Assets::open(std::env::split_paths(&wurzeln).collect()).unwrap();
        let mut states: Vec<BlockState> = Vec::new();
        for zeile in include_str!("../assets/blocks.txt").lines() {
            let mut teile = zeile.split_whitespace();
            let Some(name) = teile.next() else { continue };
            let props: Vec<(&str, Vec<&str>)> = teile
                .filter_map(|t| t.split_once('='))
                .map(|(k, v)| (k, v.split(',').collect()))
                .collect();
            if !props.iter().any(|(k, _)| *k == "waterlogged") && source_of(name).is_none() {
                continue;
            }
            // Die Zustände der Reihe nach wie ein Zählwerk, das letzte
            // Merkmal läuft innen; geflutet ist immer `true`.
            let mut index = vec![0usize; props.len()];
            let mut alle: Vec<BlockState> = Vec::new();
            'zustand: loop {
                let merkmale: Vec<String> = props
                    .iter()
                    .zip(&index)
                    .map(|((k, v), &i)| {
                        format!("{k}={}", if *k == "waterlogged" { "true" } else { v[i] })
                    })
                    .collect();
                let text = if merkmale.is_empty() {
                    name.to_string()
                } else {
                    format!("{name}[{}]", merkmale.join(","))
                };
                if let Ok(st) = BlockState::parse(&text)
                    && !alle.contains(&st)
                {
                    alle.push(st);
                }
                for s in (0..props.len()).rev() {
                    index[s] += 1;
                    if index[s] < props[s].1.len() {
                        continue 'zustand;
                    }
                    index[s] = 0;
                }
                break;
            }
            // Die ersten und die letzten zwölf: Hinten stehen die Zustände
            // mit `false`, etwa Leuchtflechte ohne Fläche, die mit allen
            // sechs Flächen gezeichnet wird, aber nicht leuchtet.
            let n = alle.len();
            states.extend(
                alle.into_iter()
                    .enumerate()
                    .filter(|(i, _)| *i < 12 || i + 12 >= n)
                    .map(|(_, st)| st),
            );
        }
        let paare = [
            ([0x91, 0xBD, 0x59], [0x3F, 0x76, 0xE4]),
            ([7, 0, 250], [250, 7, 0]),
            ([255, 255, 255], [0, 128, 0]),
        ];
        let (mut raster, mut ueber_eins, mut groesste, mut ragen) = (0, 0, 0, 0);
        let mut je_block: BTreeMap<&str, i32> = BTreeMap::new();
        for scale in [4, 8, 12, 16, 24, 32, 48] {
            let projection = Projection::new(scale);
            let set = build(&mut assets, &states, projection).unwrap();
            for st in &states {
                let Some(sprite) = set.id(st).and_then(|id| set.part(id, OWN_CELL)) else {
                    continue;
                };
                let Some(karte) = sprite.tint.as_ref() else {
                    continue;
                };
                let model = model_of(&mut assets, st).unwrap();
                for (b, w) in paare {
                    let block = match source_of(st.name()) {
                        Some(Source::Biome(_)) => Some(b),
                        Some(Source::Fixed(fest)) => Some(fest),
                        None => None,
                    };
                    let tints = Tints {
                        block,
                        water: Some(w),
                    };
                    let direkt = render(&model, assets.textures(), &projection, tints).unwrap();
                    // Ragt das Modell über seinen Würfel, ist das Sprite
                    // nur das Stück darin; solche zählt der Test nur.
                    if (sprite.offset, sprite.image.dimensions())
                        != (direkt.offset, direkt.image.dimensions())
                    {
                        ragen += 1;
                        continue;
                    }
                    raster += 1;
                    let mut max = 0;
                    for (i, (ist, soll)) in
                        sprite.image.pixels().zip(direkt.image.pixels()).enumerate()
                    {
                        let ist =
                            tinted(ist.0, [karte[2 * i], karte[2 * i + 1]], [pack(b), pack(w)]);
                        for (a, s) in ist.iter().zip(soll.0).take(3) {
                            max = max.max((*a as i32 - s as i32).abs());
                        }
                    }
                    ueber_eins += (max > 1) as u32;
                    groesste = groesste.max(max);
                    let eintrag = je_block.entry(st.name()).or_default();
                    *eintrag = (*eintrag).max(max);
                }
            }
        }
        println!(
            "{} Blockstates, {raster} Raster mit Karte, {ueber_eins} über 1, höchstens \
             {groesste}; {ragen} ragen über ihren Würfel und fehlen",
            states.len()
        );
        for (name, max) in je_block.iter().filter(|(_, max)| **max > 1) {
            println!("  {name}: {max}");
        }
        assert!(groesste <= 1, "höchstens {groesste}");
    }

    /// Von oben steht jede senkrechte Fläche auf der Kante, bei `north-45`
    /// jede nach Osten und Westen. Keine Fläche eines Vanilla-Blocks liegt
    /// zwischen dem Rauschen unter `EDGE_ON` und einer echten Neigung, sonst
    /// bliebe sie als Haarlinie im Bild: Jede, die die Kamera sieht, steht
    /// mit dem Kosinus ihres Winkels zur Achse über 1e-3. Von oben ist das
    /// n_y; die steilste ist die Fahne der Banner, um 0,45° geneigt wie im
    /// Modell des Spiels, mit n_y 0,0079. Alle Zustände aus `blocks.txt`.
    /// Braucht die Asset-Wurzeln in `ASSETS` wie
    /// [`toenungskarte_an_allen_vanilla_bloecken`], deshalb `#[ignore]`:
    ///
    /// ```bash
    /// ASSETS="$PWD/vanilla-assets:$PWD/assets" cargo test --release --manifest-path renderer/Cargo.toml --lib keine_haarlinie_an_allen_vanilla_bloecken -- --ignored --nocapture
    /// ```
    ///
    /// Siehe docs/renderer/kamera.md, „Von oben“.
    /// Siehe docs/renderer/kamera.md, „Genordet“.
    #[test]
    #[ignore]
    fn keine_haarlinie_an_allen_vanilla_bloecken() {
        let wurzeln = std::env::var_os("ASSETS").expect("ASSETS auf die Asset-Wurzeln setzen");
        let mut assets = Assets::open(std::env::split_paths(&wurzeln).collect()).unwrap();
        let zustaende = vanilla_zustaende();
        for kamera in [Kamera::Oben, Kamera::Nord45] {
            let projection = Projection::mit_kamera(32, kamera);
            let achse = projection.achse();
            let laenge = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let mut flaechen = 0;
            // Je Block die steilste Fläche, die die Kamera sieht.
            let mut je_block: BTreeMap<&str, f32> = BTreeMap::new();
            for st in &zustaende {
                for (_, model) in models_of(&mut assets, st, None).unwrap() {
                    for quad in &model.quads {
                        if !faces_camera(quad, &projection) {
                            continue;
                        }
                        flaechen += 1;
                        let n = quad.normal();
                        let kosinus = (n[0] * achse[0] + n[1] * achse[1] + n[2] * achse[2])
                            / (laenge(n) * laenge(achse));
                        let steilste = je_block.entry(st.name()).or_insert(1.0);
                        *steilste = steilste.min(kosinus);
                    }
                }
            }
            let mut steilste: Vec<(f32, &str)> = je_block.iter().map(|(&b, &n)| (n, b)).collect();
            steilste.sort_by(|a, b| a.0.total_cmp(&b.0));
            println!(
                "{kamera}: {} Zustände, {flaechen} Flächen; die steilsten:",
                zustaende.len()
            );
            for (n, block) in &steilste[..3] {
                println!("  {block}: Kosinus zur Achse {n}");
            }
            let steil = steilste[0].0;
            assert!(
                steil > 1e-3,
                "{kamera}: eine Fläche steht fast auf der Kante, Kosinus {steil}"
            );
        }
    }

    /// Alle Zustände aus `blocks.txt`, wie ein Zählwerk, das letzte Merkmal
    /// läuft innen.
    fn vanilla_zustaende() -> Vec<BlockState> {
        let mut out = Vec::new();
        for zeile in include_str!("../assets/blocks.txt").lines() {
            let mut teile = zeile.split_whitespace();
            let Some(name) = teile.next() else { continue };
            let props: Vec<(&str, Vec<&str>)> = teile
                .filter_map(|t| t.split_once('='))
                .map(|(k, v)| (k, v.split(',').collect()))
                .collect();
            let mut index = vec![0usize; props.len()];
            'zustand: loop {
                let merkmale: Vec<String> = props
                    .iter()
                    .zip(&index)
                    .map(|((k, v), &i)| format!("{k}={}", v[i]))
                    .collect();
                let text = if merkmale.is_empty() {
                    name.to_string()
                } else {
                    format!("{name}[{}]", merkmale.join(","))
                };
                out.extend(BlockState::parse(&text).ok());
                for s in (0..props.len()).rev() {
                    index[s] += 1;
                    if index[s] < props[s].1.len() {
                        continue 'zustand;
                    }
                    index[s] = 0;
                }
                break;
            }
        }
        out
    }

    /// In 2:1 liegen bei manchen scales alle Fragmente eines Modells in einem
    /// einzigen fremden Würfel. Je Vielfaches von 4 bis 64 nennt der Test
    /// die Zustände von Vanilla, die dort ein Teil werden, und die, die im
    /// Spielraum ganz bleiben. Braucht die Asset-Wurzeln in `ASSETS` wie
    /// [`keine_haarlinie_an_allen_vanilla_bloecken`], deshalb
    /// `#[ignore]`:
    ///
    /// ```bash
    /// ASSETS="$PWD/vanilla-assets:$PWD/assets" cargo test --release --manifest-path renderer/Cargo.toml --lib zwei_zu_eins_in_einem_fremden_wuerfel -- --ignored --nocapture
    /// ```
    ///
    /// Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
    #[test]
    #[ignore]
    fn zwei_zu_eins_in_einem_fremden_wuerfel() {
        let wurzeln = std::env::var_os("ASSETS").expect("ASSETS auf die Asset-Wurzeln setzen");
        let mut assets = Assets::open(std::env::split_paths(&wurzeln).collect()).unwrap();
        let zustaende = vanilla_zustaende();
        let mut anders = Vec::new();
        for scale in (4..=64).step_by(4) {
            let projection = Projection::new(scale);
            let (mut teil, mut ganz) = (BTreeSet::new(), BTreeSet::new());
            for st in &zustaende {
                for (_, model) in models_of(&mut assets, st, None).unwrap() {
                    let Some(raster) = rastern(
                        &model,
                        assets.textures(),
                        &projection,
                        Tints::default(),
                        CardinalLight::Default,
                        kollision(st),
                        false,
                    ) else {
                        continue;
                    };
                    let zellen = raster.zellen();
                    let [zelle] = zellen.iter().collect::<Vec<_>>()[..] else {
                        continue;
                    };
                    if *zelle == OWN_CELL {
                        continue;
                    }
                    let eintrag = format!("{st} {zelle:?}");
                    if fits_cell(&raster.ganz(), OWN_CELL, projection) {
                        ganz.insert(eintrag);
                    } else {
                        teil.insert(eintrag);
                    }
                }
            }
            println!(
                "scale {scale}: {} ein Teil, {} im Spielraum ganz",
                teil.len(),
                ganz.len()
            );
            for eintrag in &teil {
                println!("  Teil {eintrag}");
            }
            for eintrag in &ganz {
                println!("  ganz {eintrag}");
            }
            // So steht es in kamera.md: ein Teil werden nur die 32 stehenden
            // Banner bei scale 16.
            // Die schliessende Klammer hält rotation=12 und Wandbanner draussen.
            let banner = teil
                .iter()
                .all(|e| e.contains("_banner[rotation=2]") || e.contains("_banner[rotation=10]"));
            if teil.len() != if scale == 16 { 32 } else { 0 } || !banner {
                anders.push(scale);
            }
        }
        assert!(
            anders.is_empty(),
            "anders als in der Doku bei scale {anders:?}"
        );
    }

    /// Ein Pack darf Zustände mit verschiedener Kollisionsform auf dasselbe
    /// Modell legen; die Fixtures zeichnen die doppelte Platte wie die
    /// untere. Deren Oberseite liegt im Innern, Platz 3, bei der doppelten
    /// mit voller Kollisionsform auf dem Rand, Platz 0: zwei Familien mit
    /// verschiedenen Sprites.
    #[test]
    fn volle_kollisionsform_trennt_die_familie() {
        let mut assets = assets();
        let states = [
            state("minecraft:oak_slab[type=bottom,waterlogged=false]"),
            state("minecraft:oak_slab[type=double,waterlogged=false]"),
        ];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert_eq!(set.families.len(), 2);
        let [unten, doppelt] = states.map(|st| set.id(&st).unwrap());
        assert_eq!(set.plaetze(unten), 0b1110);
        assert_eq!(set.plaetze(doppelt), 0b111);
    }

    /// Die andere Hälfte einer Doppelkiste liegt wie in
    /// `ChestBlock.getConnectedDirection`: bei `left` im Uhrzeigersinn neben
    /// `facing`, bei `right` dagegen. Eine einzelne Kiste und eine Platte
    /// haben keine.
    #[test]
    fn doppelkiste_wie_im_spiel() {
        let kiste = |text: &str| doppelkiste(&state(text));
        for (facing, links, rechts) in [
            ("north", [1, 0, 0], [-1, 0, 0]),
            ("east", [0, 0, 1], [0, 0, -1]),
            ("south", [-1, 0, 0], [1, 0, 0]),
            ("west", [0, 0, -1], [0, 0, 1]),
        ] {
            let text = |typ: &str| {
                format!("minecraft:chest[facing={facing},type={typ},waterlogged=false]")
            };
            assert_eq!(kiste(&text("left")), Some(links), "{facing}");
            assert_eq!(kiste(&text("right")), Some(rechts), "{facing}");
            assert_eq!(kiste(&text("single")), None, "{facing}");
        }
        assert_eq!(
            kiste("minecraft:trapped_chest[facing=north,type=left,waterlogged=false]"),
            Some([1, 0, 0])
        );
        assert_eq!(
            kiste("minecraft:oak_slab[type=top,waterlogged=false]"),
            None
        );
    }

    /// Wo die andere Hälfte einer Doppelkiste steht, braucht keinen Platz im
    /// Schlüssel der Familie: Zwei Zustände eines `ChestBlock` mit demselben
    /// Bild aus dem Blockentity haben sie an derselben Stelle.
    #[test]
    fn das_bild_trennt_die_haelften_einer_doppelkiste() {
        let mut je_bild = HashMap::new();
        for zeile in include_str!("../assets/blocks.txt").lines() {
            let Some((name, _)) = zeile
                .split_once(' ')
                .filter(|_| zeile.contains(" type=single,left,right "))
            else {
                continue;
            };
            for facing in ["north", "south", "west", "east"] {
                for typ in ["single", "left", "right"] {
                    for nass in ["true", "false"] {
                        let st = state(&format!(
                            "minecraft:{name}[facing={facing},type={typ},waterlogged={nass}]"
                        ));
                        let bild = blockentity::bild(&st).expect("Bild");
                        let alt = je_bild.insert(bild, doppelkiste(&st));
                        assert!(alt.is_none_or(|alt| alt == doppelkiste(&st)), "{st}");
                    }
                }
            }
        }
        assert!(je_bild.len() >= 12, "{} Bilder", je_bild.len());
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

    /// Lava endet bei 8/9: sie deckt den Umriss nicht, den Block darunter
    /// aber schon. Frei bleiben bei scale 32 64 von 768 Pixeln des
    /// Umrisses, bei 16 16 von 192 und bei 8 4 von 48. Erst bei scale 4 ist
    /// der Streifen ueber ihr keinen Pixel hoch, und sie deckt ihren Umriss
    /// tatsaechlich. Ein Zaunpfosten deckt fast nichts, ein voller Wuerfel
    /// alles, eine Druckplatte ihren Boden nicht: ihr Rand ist zu sehen.
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
        // Nur Lava deckt bei scale 4 auch ihren Umriss.
        for scale in [48, 32, 24, 16, 12, 8, 4] {
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
            let flags = |text: &str| {
                let f = set.family_of(&state(text)).unwrap();
                (f.opaque, f.covers_floor)
            };
            assert_eq!(flags("einfarbig"), (true, true), "scale {scale}");
            assert_eq!(flags("water"), (false, false), "scale {scale}");
            assert_eq!(flags("lava"), (scale == 4, true), "scale {scale}");
        }
        let set = build(&mut assets, &states, Projection::new(32)).unwrap();
        let flags = |text: &str| {
            let f = set.family_of(&state(text)).unwrap();
            (f.opaque, f.covers_floor)
        };
        assert_eq!(flags("oak_fence[north=true]"), (false, false));
        assert_eq!(
            flags("water"),
            (false, false),
            "durchscheinend deckt nichts"
        );
        assert_eq!(flags("druckplatte"), (false, false), "Rand frei");
        assert_eq!(flags("teppich"), (false, true), "Boden ganz");

        // Von oben liegt der Boden an derselben Stelle wie der Umriss, und
        // jede volle Oberseite deckt beide, auch die flache von Lava und
        // Teppich. Die Druckplatte lässt ihren Rand frei.
        let set = build(
            &mut assets,
            &states,
            Projection::mit_kamera(32, Kamera::Oben),
        )
        .unwrap();
        let flags = |text: &str| {
            let f = set.family_of(&state(text)).unwrap();
            (f.opaque, f.covers_floor)
        };
        assert_eq!(flags("einfarbig"), (true, true), "von oben");
        assert_eq!(flags("lava"), (true, true), "von oben");
        assert_eq!(flags("teppich"), (true, true), "von oben");
        assert_eq!(flags("druckplatte"), (false, false), "von oben");
        assert_eq!(flags("water"), (false, false), "von oben");

        // Genordet von oben wie von oben. Von Süden deckt die flache
        // Oberseite von Teppich den oberen Rand des Umrisses nicht, mit der
        // Südwand darunter aber den Boden. Lava lässt oben b/9 frei, wie in
        // 2:1 erst bei scale 4 keinen Pixel.
        for kamera in [Kamera::ObenNord, Kamera::Nord45] {
            let oben = kamera == Kamera::ObenNord;
            for scale in [48, 32, 24, 16, 12, 8, 4] {
                let projection = Projection::mit_kamera(scale, kamera);
                let set = build(&mut assets, &states, projection).unwrap();
                let flags = |text: &str| {
                    let f = set.family_of(&state(text)).unwrap();
                    (f.opaque, f.covers_floor)
                };
                assert_eq!(flags("einfarbig"), (true, true), "{kamera} bei {scale}");
                assert_eq!(flags("water"), (false, false), "{kamera} bei {scale}");
                assert_eq!(
                    flags("lava"),
                    (oben || scale == 4, true),
                    "{kamera} bei {scale}"
                );
                assert_eq!(flags("teppich"), (oben, true), "{kamera} bei {scale}");
                if scale == 16 {
                    assert_eq!(flags("druckplatte"), (false, false), "{kamera}");
                    assert_eq!(flags("oak_fence[north=true]"), (false, false), "{kamera}");
                }
            }
        }
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

    /// Bleibt eine Familie im Würfel, bleibt jede ihrer Fassungen im Umriss:
    /// Masken, Biome, die Streifen ihrer Flüssigkeit und die Fassungen ohne
    /// Flächen zu Nachbarn. `contained`
    /// prüft nur die Grundbilder, darauf bauen aber die Deckungsmaske
    /// (`bedeckt`) und die Kandidatensuche (`touches`): Ein enthaltener
    /// Block fällt weg, wenn sein Umriss bedeckt ist oder die Kachel nicht
    /// berührt. Geprüft an den Blöcken der Testszenen, Eis, gefluteten
    /// Scheiben und Wurzeln, Wasser und Lava in jeder Höhe, bei jedem scale
    /// von 4 bis 32.
    #[test]
    fn fassungen_enthaltener_familien_bleiben_im_umriss() {
        let mut assets = assets();
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base");
        assets.load_biomes(&data).unwrap();
        let mut states: Vec<BlockState> = [
            "grass_block",
            "einfarbig",
            "durchsichtig",
            "oak_fence[waterlogged=true]",
            "bubble_column",
            "ueberhang",
            "seerose",
            "ackerboden",
            "turm",
            "rand",
            "boden",
            "obere_platte",
            "untere_platte",
            "kuchen",
            "saeule",
            "druckplatte",
            "teppich",
            "mit_overlay",
            "ice",
            "glass_pane[east=true,north=false,south=true,waterlogged=true,west=false]",
            "mangrove_roots[waterlogged=true]",
        ]
        .into_iter()
        .map(state)
        .collect();
        for level in 0..8 {
            states.push(state(&format!("water[level={level}]")));
            states.push(state(&format!("lava[level={level}]")));
        }
        for scale in (4..=32).step_by(4) {
            let set = build(&mut assets, &states, Projection::new(scale)).unwrap();
            let mut geprueft = 0;
            for family in set.families.iter().filter(|f| f.contained) {
                let mut ids: Vec<SpriteId> = family
                    .alternatives
                    .iter()
                    .filter_map(|&(_, id)| id)
                    .collect();
                let masken: Vec<SpriteId> = ids
                    .iter()
                    .filter_map(|id| set.by_mask.get(id))
                    .flat_map(|fassungen| fassungen.iter().flatten())
                    .copied()
                    .collect();
                ids.extend(masken);
                ids.extend(family.fassungen.iter().flatten().flatten());
                if let Some((fluid, _)) = family.fluid {
                    ids.extend(
                        set.strips
                            .iter()
                            .filter(|(schluessel, _)| schluessel.0 == fluid)
                            .map(|(_, &id)| id),
                    );
                }
                for id in ids {
                    assert!(
                        set.sprites[id.0 as usize].contained,
                        "scale {scale}: Sprite {} einer enthaltenen Familie ragt heraus",
                        id.0
                    );
                    geprueft += 1;
                }
            }
            assert!(geprueft > 1000, "scale {scale}: nur {geprueft} Fassungen");
        }
    }

    /// Was Wasser zeichnet, bleibt im Umriss seines Blocks, jede Fassung und
    /// jeder Streifen: Die Kandidatensuche verwirft einen Block samt seinen
    /// Streifen, wenn sein Umriss die Kachel nicht berührt.
    #[test]
    fn wasser_bleibt_im_umriss() {
        let mut assets = assets();
        for scale in [2, 4, 6, 16, 32] {
            let set = build(&mut assets, [&state("water")], Projection::new(scale)).unwrap();
            assert!(!set.strips.is_empty(), "scale {scale}: keine Streifen");
            for (i, entry) in set.sprites.iter().enumerate() {
                assert!(entry.contained, "scale {scale}: Sprite {i}");
            }
        }
    }

    /// Blöcke ohne sichtbare Geometrie tauchen gar nicht erst auf.
    #[test]
    fn modell_ohne_flaechen_faellt_heraus() {
        let mut assets = assets();
        let states = [state("nur_partikel")];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        assert!(set.is_empty());
    }

    /// Jede Gruppe der Blockentities bekommt ihr Bild aus der Tabelle, auch
    /// mit einem Blockmodell ohne Elemente: einfache und doppelte Truhen,
    /// Kupfer- und Endertruhe, Shulkerkiste, Banner stehend und an der Wand,
    /// Köpfe ebenso, Krug, Glocke, Aquisator, Statue, die Bücher. Jedes Bild
    /// ist ein anderes.
    #[test]
    fn jede_gruppe_hat_ein_bild() {
        let mut assets = assets();
        let states: Vec<BlockState> = [
            "minecraft:chest[facing=north,type=single,waterlogged=false]",
            "minecraft:chest[facing=north,type=left,waterlogged=false]",
            "minecraft:chest[facing=north,type=right,waterlogged=false]",
            "minecraft:copper_chest[facing=east,type=single,waterlogged=false]",
            "minecraft:ender_chest[facing=south,waterlogged=false]",
            "minecraft:red_shulker_box[facing=up]",
            "minecraft:white_banner[rotation=3]",
            "minecraft:white_wall_banner[facing=north]",
            "minecraft:player_head[powered=false,rotation=5]",
            "minecraft:skeleton_wall_skull[facing=east,powered=false]",
            "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]",
            "minecraft:bell[attachment=floor,facing=north,powered=false]",
            "minecraft:conduit[waterlogged=false]",
            "minecraft:copper_golem_statue[copper_golem_pose=star,facing=north,waterlogged=false]",
            "minecraft:lectern[facing=north,has_book=true,powered=false]",
            "minecraft:enchanting_table",
        ]
        .map(state)
        .into();
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let bilder: Vec<_> = states
            .iter()
            .map(|s| pixel(&set, set.id(s).unwrap_or_else(|| panic!("{s}: kein Bild"))))
            .collect();
        let verschieden: HashSet<_> = bilder.iter().collect();
        assert_eq!(verschieden.len(), bilder.len());
    }

    /// Die Pixel eines Sprites über alle Würfel, in die es fällt. Ein Sprite,
    /// das über seinen Würfel ragt wie ein Banner, teilt sich keinen Eintrag,
    /// auch mit gleichem Bild nicht (`insert`); verschiedene IDs sagen dann
    /// nichts über das Bild.
    fn pixel(set: &SpriteSet, id: SpriteId) -> Vec<(Cell, (u32, u32), Vec<u8>)> {
        set.sprites[id.0 as usize]
            .parts
            .iter()
            .map(|(zelle, teil)| (*zelle, teil.image.dimensions(), teil.image.as_raw().clone()))
            .collect()
    }

    /// Eine Truhe hat in jeder Lage dasselbe Blockmodell ohne Flächen, aber
    /// ein anderes Bild aus ihrem Blockentity: je Lage eine Familie. Geflutet
    /// trägt sie ihr Wasser und ihr Bild.
    #[test]
    fn truhen_je_lage_eigene_familie() {
        let mut assets = assets();
        let truhe = |lage: &str, nass: bool| {
            state(&format!(
                "minecraft:chest[facing={lage},type=single,waterlogged={nass}]"
            ))
        };
        let states = [
            truhe("north", false),
            truhe("east", false),
            truhe("north", true),
        ];
        let set = build(&mut assets, &states, Projection::new(16)).unwrap();
        let familien: HashSet<u32> = states
            .iter()
            .map(|s| set.family_index(s).unwrap())
            .collect();
        assert_eq!(familien.len(), 3);
        assert_ne!(set.id(&states[0]), set.id(&states[1]));
        let nass = model_of(&mut assets, &states[2]).unwrap();
        assert!(nass.quads.iter().any(|q| q.fluid.is_some()));
        assert!(nass.quads.iter().any(|q| q.entity.is_some()));
    }

    /// Ein Banner mit Mustern bekommt eine eigene Familie mit anderem Bild,
    /// dieselben Daten nur eine; Daten, die nicht zum Block passen, keine.
    /// Was an den Daten unbekannt ist, meldet der Aufbau.
    #[test]
    fn daten_geben_eigene_familie() {
        let mut assets = assets();
        let banner = state("minecraft:white_banner[rotation=0]");
        let truhe = state("minecraft:chest[facing=north,type=single,waterlogged=false]");
        let mut set = build(&mut assets, [&banner, &truhe], Projection::new(16)).unwrap();
        let muster = Blockdaten::Banner(vec![
            (Muster::Id("stripe_top".to_string()), "red".to_string()),
            (Muster::Id("stripe_top".to_string()), "lila".to_string()),
        ]);
        let eintraege = [
            (banner.clone(), muster.clone()),
            (banner.clone(), muster.clone()),
            (truhe.clone(), muster.clone()),
        ];
        let vorher = set.families.len();
        let unbekannt = set.add_entities(&mut assets, &eintraege).unwrap();
        assert_eq!(unbekannt, BTreeSet::from(["Farbstoff lila".to_string()]));
        assert_eq!(set.families.len(), vorher + 1, "eine für den Banner");
        let basis = set.family_index(&banner).unwrap();
        let variante = set.variante(basis, &muster).expect("Familie mit Mustern");
        let bild = |familie: u32| pixel(&set, set.family(familie).alternatives[0].1.unwrap());
        assert_ne!(bild(variante), bild(basis), "das Muster fehlt im Bild");
        assert_eq!(
            set.variante(set.family_index(&truhe).unwrap(), &muster),
            None
        );
    }
}
