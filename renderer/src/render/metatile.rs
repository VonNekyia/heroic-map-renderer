use std::collections::HashMap;

use anyhow::Result;
use image::RgbaImage;

use crate::assets::Face;
use crate::assets::fluid;
use crate::assets::fluid::Fluid;
use crate::world::{Chunk, REGION, Region, Section, World};

use super::rasterizer::over;
use super::sprites::{Cover, DEPTHS, Family, mask_bit};
use super::{Cell, OWN_CELL, Projection, Sprite, SpriteId, SpriteSet};

/// Reserve um das Zielrechteck herum, in Blockbreiten.
///
/// Sprites dürfen über den Blockumriss hinausragen — Feuer ist höher als
/// ein Block, Zäune breiter. Ohne diese Reserve fehlen an den Rändern
/// Blöcke, deren Ursprung knapp ausserhalb liegt.
pub const BLEED_BLOCKS: i32 = 3;

/// Ein rechteckiger Ausschnitt der projizierten Ebene, in Pixeln.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ScreenRect {
    /// Rechteck mit dem Blockursprung `(0, 0, 0)` im Mittelpunkt.
    pub fn centered(width: u32, height: u32) -> ScreenRect {
        ScreenRect {
            x: -(width as i32) / 2,
            y: -(height as i32) / 2,
            width,
            height,
        }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }
}

/// Rendert einen Ausschnitt der Welt.
///
/// Gezeichnet wird nach dem Maleralgorithmus, und zwar je Blockwürfel:
/// erst nach Höhe `y`, innerhalb einer Höhe nach Tiefe `v = x + z`. Beides
/// zusammen ist eine gültige Reihenfolge:
///
/// - Verdeckt Würfel B den Würfel A, dann liegt B nie tiefer
///   (`y_B >= y_A`). Sonst wäre der senkrechte Abstand auf dem Bild
///   mindestens eine Blockhöhe, und die Umrisse berührten sich höchstens.
/// - Auf gleicher Höhe heisst "verdeckt" genau `v_B > v_A`, denn
///   `depth = x + y + z = v + y`. Der Südnachbar `(x, y, z+1)` verdeckt die
///   Südfläche von `(x, y, z)`, der Ostnachbar `(x+1, y, z)` die Ostfläche.
///
/// Entscheidend ist "je Würfel" und nicht "je Block": ein Modell, das über
/// seinen Blockwürfel hinausragt, ist in `SpriteSet` bereits in Teile
/// zerlegt, und jeder Teil wird zu dem Zeitpunkt gezeichnet, der zu seinem
/// eigenen Würfel gehört. Sonst käme ein hohes Modell zu früh, und ein
/// Block dahinter mit höherem Ursprung übermalte es.
///
/// Ein globaler Tiefenpuffer ist damit unnötig. Die Reihenfolge kommt aus
/// dem Schlüssel `(y, v, u, Teil)`, nach dem die Kandidaten sortiert
/// werden, siehe [`render_area_with`].
pub fn render_area(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    render_area_with(&mut ChunkCache::new(world, sprites), rect, y_range)
}

/// Wie [`render_area`], mit einem Cache, der über Kacheln hinweg lebt.
///
/// Aufeinanderfolgende Kacheln liegen untereinander und teilen sich fast
/// alle Chunks. Wer sie je Kachel neu lädt, gibt ein Drittel der Renderzeit
/// fürs Dekodieren aus, das er gerade erst gemacht hat.
///
/// Zwei Durchgänge. Der erste sammelt die Kandidaten — Blöcke, von denen
/// etwas zu sehen sein kann — aus den Bitmasken der Sections, ohne einen
/// einzigen Luftblock anzufassen. Der zweite sortiert sie in die
/// Zeichenreihenfolge und zeichnet. Das Bild ist dasselbe wie das von
/// [`render_area_without_culling`], das jeden Block im Band abläuft.
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let cover = chunks.sprites.cover();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    draw_all(&mut canvas, &draw_list(chunks, rect, y_range)?, cover);
    Ok(canvas)
}

/// Zeichnet eine Liste auf die Leinwand — der CPU-Weg, an dem sich der
/// GPU-Weg messen lassen muss.
pub fn draw_all(canvas: &mut RgbaImage, draws: &[Draw], cover: &Cover) {
    for d in draws {
        blit(canvas, d.sprite, d.origin, cover, d.skip);
    }
}

/// Ein Sprite-Teil an seinem Platz auf der Leinwand.
///
/// Der Renderlauf stellt je Kachel diese Liste auf, fertig sortiert, und
/// zeichnet sie dann selbst ([`render_area_with`]) oder gibt sie an die
/// Grafikkarte ([`super::gpu::Worker`]). Beide malen dasselbe Bild.
#[derive(Clone, Copy)]
pub struct Draw<'a> {
    pub sprite: &'a Sprite,
    /// Linke obere Ecke des Sprites in Leinwandpixeln; darf über den Rand
    /// hinausragen.
    pub origin: (i32, i32),
    /// Nachbarn (`mask_bit`), deren Umriss der Blit auslassen darf.
    pub skip: u8,
}

/// Die Zeichenliste eines Ausschnitts, in Zeichenreihenfolge.
pub fn draw_list<'a>(
    chunks: &mut ChunkCache<'a>,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<Vec<Draw<'a>>> {
    chunks.next_tile();
    let sprites: &'a SpriteSet = chunks.sprites;
    let projection = sprites.projection();
    let foreign: Vec<Cell> = sprites.foreign_cells().iter().copied().collect();

    let mut candidates = chunks.candidates(rect, y_range, &foreign)?;
    candidates.sort_unstable_by_key(|c| c.key);

    let mut draws = Vec::with_capacity(candidates.len());
    let mut zeichne = |id: SpriteId, cell: Cell, anchor: [i32; 3], skip: u8| {
        if let Some(part) = sprites.part(id, cell) {
            draws.push(Draw {
                sprite: part,
                origin: origin_of(projection, rect, anchor, part),
                skip,
            });
        }
    };
    for c in &candidates {
        let pos = [c.x, c.y, c.z];
        if c.kind == 0 {
            for id in chunks.sprite_at(c.x, c.y, c.z)?.ids() {
                zeichne(id, OWN_CELL, pos, c.skip);
            }
        } else {
            let cell = foreign[c.kind as usize - 1];
            let anchor = anchor_of(pos, cell);
            if let Some(id) = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?.sprite {
                // Fremde Teile liegen in einem anderen Würfel als dem Anker;
                // die Deckungstabelle gilt nur für den eigenen.
                zeichne(id, cell, anchor, 0);
            }
        }
    }

    Ok(draws)
}

/// Wie [`render_area`], aber Block für Block über das ganze Band, ohne
/// Kandidaten, ohne verdeckte Würfel auszulassen und ohne Pixel zu
/// überspringen: die Referenz, gegen die Tests den schnellen Weg prüfen.
/// Er darf kein Pixel ändern.
pub fn render_area_without_culling(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let projection = sprites.projection();
    let cover = sprites.cover();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    let mut chunks = ChunkCache::new(world, sprites);

    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            for id in chunks.sprite_at(x, y, z)?.ids() {
                if let Some(part) = sprites.part(id, OWN_CELL) {
                    let origin = origin_of(projection, rect, [x, y, z], part);
                    blit(&mut canvas, part, origin, cover, 0);
                }
            }
            for &cell in sprites.foreign_cells() {
                let anchor = anchor_of([x, y, z], cell);
                if let Some(id) = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?.sprite
                    && let Some(part) = sprites.part(id, cell)
                {
                    let origin = origin_of(projection, rect, anchor, part);
                    blit(&mut canvas, part, origin, cover, 0);
                }
            }
        }
    }

    Ok(canvas)
}

/// Linke obere Ecke eines Sprites auf der Leinwand, wenn sein Block bei
/// `anchor` steht; sie darf über den Rand hinausragen.
fn origin_of(
    projection: Projection,
    rect: ScreenRect,
    anchor: [i32; 3],
    sprite: &Sprite,
) -> (i32, i32) {
    let (sx, sy) = projection.project_block(anchor);
    (
        sx.round() as i32 + sprite.offset.0 - rect.x,
        sy.round() as i32 + sprite.offset.1 - rect.y,
    )
}

/// Der Block, dessen Modell in `cell` hineinragen würde.
fn anchor_of([x, y, z]: [i32; 3], cell: Cell) -> [i32; 3] {
    [x - cell[0], y - cell[1], z - cell[2]]
}

/// Was an einem Würfel zu zeichnen ist: das Sprite des Blocks, dazu die
/// Streifen seiner Flüssigkeit über niedrigeren Nachbarn.
#[derive(Default, Clone, Copy)]
struct Drawn {
    sprite: Option<SpriteId>,
    strips: [Option<SpriteId>; 2],
}

impl Drawn {
    /// In Zeichenreihenfolge: die Streifen nach dem Block, sie liegen auf
    /// seiner Grenze, also vor allem, was er selbst enthält.
    fn ids(self) -> impl Iterator<Item = SpriteId> {
        self.sprite
            .into_iter()
            .chain(self.strips.into_iter().flatten())
    }
}

/// Ein Block, von dem etwas zu sehen sein kann, mit seinem Platz in der
/// Zeichenreihenfolge.
struct Candidate {
    /// `(y, v, u, kind)` in einem Wort, damit das Sortieren billig ist:
    /// 10, 22, 22 und 10 Bit, von `y_range.0` und vom Rand des Bands an
    /// gezählt.
    key: u64,
    x: i32,
    y: i32,
    z: i32,
    /// 0: der Block selbst; sonst 1 + Index des fremden Würfels, in den
    /// ein Nachbarmodell hineinragt.
    kind: u16,
    /// Nachbarn (`mask_bit`), die `PLAIN` sind, deckend und ohne
    /// Flüssigkeit, und gezeichnet werden: was in ihrem Umriss liegt,
    /// übermalen sie ohnehin.
    skip: u8,
}

/// Bereich von `u = x - z`, dessen Spalten in das Rechteck fallen können.
///
/// `screen_x = u * scale/2`. f64, weil rect und Weltkoordinaten bis knapp
/// 30 Millionen gehen: siehe Projection::project_block.
fn u_window(projection: Projection, rect: ScreenRect) -> (i32, i32) {
    let scale = projection.scale() as f64;
    let bleed = BLEED_BLOCKS as f64 * scale;
    (
        ((rect.x as f64 - bleed) / (scale / 2.0)).floor() as i32,
        ((rect.right() as f64 + bleed) / (scale / 2.0)).ceil() as i32,
    )
}

/// Bereich von `v = x + z` auf dieser Höhe: `screen_y = v * scale/4 - y *
/// scale/2`.
fn v_window(projection: Projection, rect: ScreenRect, y: i32) -> (i32, i32) {
    let scale = projection.scale() as f64;
    let bleed = BLEED_BLOCKS as f64 * scale;
    let offset = y as f64 * scale / 2.0;
    (
        ((rect.y as f64 - bleed + offset) / (scale / 4.0)).floor() as i32,
        ((rect.bottom() as f64 + bleed + offset) / (scale / 4.0)).ceil() as i32,
    )
}

/// Alle Blockspalten, deren Sprite auf dieser Höhe in das Rechteck fallen
/// kann — in Zeichenreihenfolge, so wie sie die Referenz abläuft.
///
/// Statt über x und z zu laufen, läuft die Schleife über die beiden
/// Bildschirmachsen: `u = x - z` steuert die waagerechte, `v = x + z` die
/// senkrechte Position. Damit ist der Bereich je Höhe ein schmales Band
/// statt der gesamten Grundfläche.
///
/// `v` läuft aussen, und das ist kein Geschmack: `v` ist auf einer Höhe
/// genau die Tiefe entlang der Blickachse (`depth = x + y + z`). Zwei
/// Blöcke derselben Höhe überdecken einander sehr wohl — der Südnachbar
/// `(x, y, z+1)` verdeckt die Südfläche von `(x, y, z)`. Liefe `u` aussen,
/// käme er zu früh und würde übermalt.
fn columns_at(
    projection: Projection,
    rect: ScreenRect,
    y: i32,
) -> impl Iterator<Item = (i32, i32)> {
    let (u_min, u_max) = u_window(projection, rect);
    let (v_min, v_max) = v_window(projection, rect, y);

    (v_min..=v_max).flat_map(move |v| {
        // x und z sind ganzzahlig, also haben u und v dieselbe Parität.
        let start = u_min + (u_min - v).rem_euclid(2);
        (start..=u_max)
            .step_by(2)
            .map(move |u| ((u + v) / 2, (v - u) / 2))
    })
}

/// Zeichnet ein Sprite an seinen Block — ohne die Pixel, die ein Nachbar
/// aus `skip` ohnehin übermalt.
///
/// Ein sichtbarer Block zeichnet sonst alle drei Flächen, auch die, die der
/// deckende Nachbar gleich darüberlegt: auf flachem Gelände zwei von drei.
/// Übersprungen wird nur, was im Umriss eines Nachbarn liegt, der deckend
/// ist, keine Flüssigkeit enthält (`PLAIN`) *und* in dieser Kachel
/// gezeichnet wird — dann ist der Pixel danach Alpha 255 vom Nachbarn, egal
/// was vorher da stand. Das Bild ist dasselbe.
fn blit(
    canvas: &mut RgbaImage,
    sprite: &Sprite,
    (origin_x, origin_y): (i32, i32),
    cover: &Cover,
    skip: u8,
) {
    let (w, h) = (sprite.image.width() as i32, sprite.image.height() as i32);
    let (cw, ch) = (canvas.width() as i32, canvas.height() as i32);

    // Der Teil des Sprites, der auf die Leinwand fällt.
    let (x0, x1) = ((-origin_x).max(0), (cw - origin_x).min(w));
    let (y0, y1) = ((-origin_y).max(0), (ch - origin_y).min(h));
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    // Zeilenanfänge in usize: bei einer Leinwand über 23 170 Pixel Kante
    // liefe `y * Breite * 4` in i32 über.
    let (w, cw) = (w as usize, cw as usize);
    let src = sprite.image.as_raw();
    let dst: &mut [u8] = canvas;
    for py in y0..y1 {
        let row = &src[py as usize * w * 4..][..w * 4];
        let drow = &mut dst[(origin_y + py) as usize * cw * 4..][..cw * 4];
        for px in x0..x1 {
            let s = &row[px as usize * 4..][..4];
            if s[3] == 0 {
                continue;
            }
            if skip != 0 && cover.at(sprite.offset.0 + px, sprite.offset.1 + py) & skip != 0 {
                continue;
            }
            let d = &mut drow[(origin_x + px) as usize * 4..][..4];
            if s[3] == 255 {
                d.copy_from_slice(s);
            } else {
                let out = over([s[0], s[1], s[2], s[3]], [d[0], d[1], d[2], d[3]]);
                d.copy_from_slice(&out);
            }
        }
    }
}

/// Ab wie vielen Chunks ein Cache verwirft, was die vorige Kachel nicht
/// gebraucht hat. Eine Kachel bei scale 32 berührt gut hundert Chunks; die
/// nächste liegt direkt darunter und teilt sich fast alle davon. Bei
/// kleinerem scale berührt eine Kachel mehr, bei scale 4 einige hundert,
/// und der Cache hält dann entsprechend mehr.
// ponytail: Verfallsdatum je Kachel statt echtem LRU. Reicht, solange die
// Kacheln in Leseordnung kommen; sonst lädt jede Kachel ihre hundert neu.
const CACHE_CHUNKS: usize = 256;

/// Chunks, die während eines Renderlaufs gebraucht werden.
///
/// Ein Cache gehört zu einer Sprite-Tabelle: er hält je Paletteneintrag
/// den Familienindex daraus. Über Kacheln hinweg lebt er je Stapel
/// aufeinanderfolgender Kacheln, die ein Worker nacheinander rendert —
/// geteilt zwischen Workern wäre er eine Sperre im Renderpfad.
pub struct ChunkCache<'a> {
    world: &'a World,
    sprites: &'a SpriteSet,
    /// Offene Regionsdateien. `World::chunk` würde die Datei für jeden
    /// Chunk neu öffnen — bei rund hundert Chunks je Kachel sind das
    /// hundert Öffnungen statt einer Handvoll.
    regions: HashMap<(i32, i32), Option<Region>>,
    slots: Vec<Slot>,
    index: HashMap<(i32, i32), usize>,
    /// Der zuletzt benutzte Slot. Benachbarte Blöcke liegen fast immer im
    /// selben Chunk; der Merker spart das Hashen.
    last: usize,
    /// Laufende Kachelnummer — das Verfallsdatum der Slots.
    tile: u32,
}

struct Slot {
    key: (i32, i32),
    loaded: Option<Loaded>,
    used: u32,
}

/// Ein Chunk samt der Familie je Paletteneintrag. Die Blockstate wird
/// damit einmal je Section gehasht statt einmal je Block — im Renderpfad
/// war das der teuerste Schritt.
struct Loaded {
    chunk: Chunk,
    families: Vec<Vec<Option<u32>>>,
    /// Je Section ihre Bitmasken, `None` für eine Section ohne Familie.
    masks: Vec<Option<Box<Masks>>>,
    /// Je Section die Kandidaten, sobald einmal berechnet — dafür müssen
    /// die Nachbarchunks da sein, deshalb nicht beim Laden.
    exposed: Vec<Option<Box<Exposed>>>,
}

/// Die Eigenschaften einer Familie, die über Verdeckung entscheiden, je
/// als Bit einer Maske in [`Masks`].
const PRESENT: usize = 0;
/// Deckt den ganzen Umriss: verdeckt, was hinter ihm liegt.
const SOLID: usize = 1;
/// Deckt den Umriss und zeichnet genau das Sprite ihrer Alternative, ohne
/// Flüssigkeit: nur vor so einem Nachbarn dürfen Pixel entfallen. Eine
/// Flüssigkeit zeichnet eine Fassung ohne die Flächen zu ihresgleichen.
const PLAIN: usize = 2;
/// Deckt den Boden des Würfels, die Oberseite des Blocks darunter.
const FLOOR: usize = 3;
/// Enthält Wasser, [`LAVA`] Lava: für dieselbe Flüssigkeit nebenan
/// dieselbe.
const WATER: usize = 4;
const LAVA: usize = 5;
/// Nur Wasser, [`PURE_LAVA`] nur Lava, ohne Modell daneben.
const PURE_WATER: usize = 6;
const PURE_LAVA: usize = 7;
/// Bleibt nicht in ihrem Würfel: nie überspringen.
const LOOSE: usize = 8;
/// Hat Teile in Nachbarwürfeln.
const FOREIGN: usize = 9;
const FLAGS: usize = 10;
/// Je Flüssigkeit, in der Reihenfolge von [`Masks::up`]: das Bit "enthält
/// sie" und das Bit "nur sie".
const FLUIDS: [(usize, usize); 2] = [(WATER, PURE_WATER), (LAVA, PURE_LAVA)];

/// Bitmasken einer Section: je Eigenschaft und Spalte `z * 16 + x` ein
/// Wort, Bit `y`.
///
/// Damit ist die Frage "ist dieser Block von seinen drei Nachbarn
/// verdeckt?" für sechzehn Blöcke einer Spalte auf einmal ein paar
/// Wortoperationen — statt drei Nachschläge je Block, für neun von zehn
/// Blöcken, die dann doch unter der Oberfläche liegen.
struct Masks {
    bits: [[u16; 256]; FLAGS],
    /// Je Flüssigkeit aus [`FLUIDS`]: liegt über dem Block dieselbe, auch
    /// aus der Section darüber?
    up: [[u16; 256]; 2],
    /// Ragt irgendetwas in Nachbarwürfel?
    any_foreign: bool,
}

/// Eine Randspalte für [`ChunkCache::expose`]: deckend, dazu je
/// Flüssigkeit aus [`FLUIDS`] "enthält sie" und "dieselbe darüber".
type Rand = (u16, [u16; 2], [u16; 2]);

fn rand(m: &Masks, col: usize) -> Rand {
    (
        m.bits[SOLID][col],
        [m.bits[WATER][col], m.bits[LAVA][col]],
        [m.up[0][col], m.up[1][col]],
    )
}

/// Was in einer Section gezeichnet werden muss.
struct Exposed {
    /// Blöcke, von denen etwas zu sehen sein kann.
    own: [u16; 256],
    /// Je Richtung: der Nachbar ist `PLAIN` und selbst Kandidat, wird also
    /// gezeichnet und übermalt seinen Umriss. An Section- und Chunkrändern
    /// vorsichtshalber nie — dort müsste der Nachbar erst berechnet werden.
    skip_x: [u16; 256],
    skip_y: [u16; 256],
    skip_z: [u16; 256],
    /// Gibt es in der Section überhaupt einen Kandidaten? Unter der
    /// Oberfläche meist nicht — dann entfällt die Schleife über 256
    /// Spalten.
    any_own: bool,
}

/// Die Bits einer Familie für [`Masks`].
fn flags(family: &Family) -> u16 {
    let fluid = |kind: Fluid| family.fluid.is_some_and(|(k, _)| k == kind);
    let bit = |set: bool, flag: usize| (set as u16) << flag;
    bit(true, PRESENT)
        | bit(family.opaque, SOLID)
        | bit(family.opaque && family.fluid.is_none(), PLAIN)
        | bit(family.covers_floor, FLOOR)
        | bit(fluid(Fluid::Water), WATER)
        | bit(fluid(Fluid::Lava), LAVA)
        | bit(fluid(Fluid::Water) && family.pure_fluid, PURE_WATER)
        | bit(fluid(Fluid::Lava) && family.pure_fluid, PURE_LAVA)
        | bit(!family.contained, LOOSE)
        | bit(family.foreign, FOREIGN)
}

impl Masks {
    /// `None`, wenn in der Section keine Familie steht.
    fn of(section: &Section, families: &[Option<u32>], sprites: &SpriteSet) -> Option<Box<Masks>> {
        let flags: Vec<u16> = families
            .iter()
            .map(|family| family.map_or(0, |index| flags(sprites.family(index))))
            .collect();
        let union = flags.iter().fold(0, |acc, f| acc | f);
        if union == 0 {
            return None;
        }
        let mut m = Box::new(Masks {
            bits: [[0; 256]; FLAGS],
            up: [[0; 256]; 2],
            any_foreign: false,
        });
        let blocks = section.blocks();
        if blocks.is_uniform() {
            let flag = flags.first().copied().unwrap_or(0);
            for (b, mask) in m.bits.iter_mut().enumerate() {
                if flag >> b & 1 != 0 {
                    *mask = [u16::MAX; 256];
                }
            }
        } else {
            // Die Familien einer Section fallen in wenige Klassen gleicher
            // Bits: Luft, deckender Stein, Wasser, eine Blume. Je Block
            // genügt ein OR in die Maske seiner Klasse; die Masken je
            // Eigenschaft setzen sich danach aus den Klassen zusammen.
            let mut klassen: Vec<u16> = Vec::new();
            let klasse: Vec<usize> = flags
                .iter()
                .map(|&flag| {
                    if flag == 0 {
                        return usize::MAX;
                    }
                    klassen.iter().position(|&k| k == flag).unwrap_or_else(|| {
                        klassen.push(flag);
                        klassen.len() - 1
                    })
                })
                .collect();
            let mut je_klasse = vec![[0u16; 256]; klassen.len()];
            blocks.for_each_index(4096, |i, index| {
                // Ein Index über die Palette hinaus wäre ein kaputter Chunk;
                // der zählt wie Luft, genau wie beim Nachschlagen je Block.
                if let Some(maske) = klasse.get(index).and_then(|&k| je_klasse.get_mut(k)) {
                    maske[i & 255] |= 1 << (i >> 8);
                }
            });
            for (&flag, maske) in klassen.iter().zip(&je_klasse) {
                for (b, bits) in m.bits.iter_mut().enumerate() {
                    if flag >> b & 1 != 0 {
                        for (bits, spalte) in bits.iter_mut().zip(maske) {
                            *bits |= spalte;
                        }
                    }
                }
            }
        }
        if !m.bits[PRESENT].iter().any(|&p| p != 0) {
            return None;
        }
        m.any_foreign = m.bits[FOREIGN].iter().any(|&f| f != 0);
        Some(m)
    }
}

impl Loaded {
    fn new(chunk: Chunk, sprites: &SpriteSet) -> Loaded {
        let families: Vec<Vec<Option<u32>>> = chunk
            .sections()
            .iter()
            .map(|section| {
                section
                    .blocks()
                    .palette()
                    .iter()
                    .map(|state| sprites.family_index(state))
                    .collect()
            })
            .collect();
        let mut masks: Vec<Option<Box<Masks>>> = chunk
            .sections()
            .iter()
            .zip(&families)
            .map(|(section, families)| Masks::of(section, families, sprites))
            .collect();
        // Flüssigkeit über dem obersten Block einer Section steht in der
        // Section darüber, im selben Chunk.
        for s in 0..masks.len() {
            let above = chunk.sections()[s]
                .y
                .checked_add(1)
                .and_then(|y| chunk.section_index(y))
                .and_then(|i| masks[i].as_ref().map(|a| [a.bits[WATER], a.bits[LAVA]]));
            if let Some(m) = &mut masks[s] {
                for (f, &(bit, _)) in FLUIDS.iter().enumerate() {
                    for col in 0..256 {
                        let top = above.map_or(0, |a| a[f][col] & 1);
                        m.up[f][col] = (m.bits[bit][col] >> 1) | (top << 15);
                    }
                }
            }
        }
        let exposed = chunk.sections().iter().map(|_| None).collect();
        Loaded {
            chunk,
            families,
            masks,
            exposed,
        }
    }
}

impl<'a> ChunkCache<'a> {
    pub fn new(world: &'a World, sprites: &'a SpriteSet) -> ChunkCache<'a> {
        ChunkCache {
            world,
            sprites,
            regions: HashMap::new(),
            slots: Vec::new(),
            index: HashMap::new(),
            last: usize::MAX,
            tile: 0,
        }
    }

    /// Beginnt eine neue Kachel. Ist der Cache voll, geht alles, was die
    /// vorige Kachel nicht gebraucht hat.
    fn next_tile(&mut self) {
        self.tile += 1;
        self.last = usize::MAX;
        if self.slots.len() <= CACHE_CHUNKS {
            return;
        }
        let tile = self.tile;
        self.slots.retain(|slot| slot.used + 1 >= tile);
        self.index = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| (slot.key, i))
            .collect();
    }

    /// Slot des Chunks, geladen falls nötig.
    fn slot(&mut self, key: (i32, i32)) -> Result<usize> {
        if let Some(slot) = self.slots.get(self.last)
            && slot.key == key
        {
            return Ok(self.last);
        }
        let i = match self.index.get(&key) {
            Some(&i) => i,
            None => self.load(key)?,
        };
        self.slots[i].used = self.tile;
        self.last = i;
        Ok(i)
    }

    fn load(&mut self, key: (i32, i32)) -> Result<usize> {
        let region_key = (key.0.div_euclid(REGION), key.1.div_euclid(REGION));
        if !self.regions.contains_key(&region_key) {
            let region = self.world.region(region_key.0, region_key.1)?;
            self.regions.insert(region_key, region);
        }
        let chunk = match self.regions.get_mut(&region_key) {
            Some(Some(region)) => region.chunk(key.0, key.1)?,
            _ => None,
        };
        let loaded = chunk.map(|chunk| Loaded::new(chunk, self.sprites));
        self.slots.push(Slot {
            key,
            loaded,
            used: self.tile,
        });
        let i = self.slots.len() - 1;
        self.index.insert(key, i);
        Ok(i)
    }

    /// Randspalten einer Nachbarsection ([`Rand`]) je Spalte am Rand `x = 0`
    /// (Index z) oder `z = 0` (Index x). Ohne Chunk oder Section ist das
    /// Luft.
    fn edge(&mut self, key: (i32, i32), section_y: i8, x_edge: bool) -> Result<[Rand; 16]> {
        let i = self.slot(key)?;
        let mut out = [(0, [0; 2], [0; 2]); 16];
        if let Some(loaded) = &self.slots[i].loaded
            && let Some(s) = loaded.chunk.section_index(section_y)
            && let Some(m) = &loaded.masks[s]
        {
            for (j, edge) in out.iter_mut().enumerate() {
                *edge = rand(m, if x_edge { j * 16 } else { j });
            }
        }
        Ok(out)
    }

    /// Rechnet die Kandidaten einer Section aus, falls noch nicht geschehen.
    ///
    /// Ein Block ist verdeckt wie in [`render_area`] beschrieben: die
    /// Nachbarn nach +x und +z decken ihren ganzen Umriss, dem nach +y
    /// genügt sein Boden. Nach +y ist das Bit des Nachbarn in derselben
    /// Spalte, eins höher — ein Shift; am oberen Rand kommt es aus der
    /// Section darüber, an den Rändern +x und +z aus dem Nachbarchunk.
    ///
    /// Reine Flüssigkeit, Wasser wie Lava, zeichnet ausserdem nichts, wo
    /// über ihr dieselbe steht und sie zu beiden Seiten an dieselbe grenzt,
    /// die selbst dieselbe über sich hat: die Flächen dorthin entfallen, und
    /// ein Streifen über einem niedrigeren Nachbarn kann nicht entstehen.
    /// Seitlich darf statt der Flüssigkeit auch ein deckender Nachbar stehen;
    /// mit derselben darüber reicht die Seitenfläche bis zur Kante, und der
    /// Nachbar übermalt sie danach. Oben dagegen nicht: ohne dieselbe
    /// darüber endet die Oberfläche bei ihrer Höhe, tiefer als der Boden des
    /// Blocks darüber, und ragt in die Seiten hinein. So kommt das Innere
    /// eines Ozeans oder eines Lavasees gar nicht erst zur Sprite-Wahl;
    /// Lava deckt nur bei scale 4, sonst fiele dort kein Block weg.
    fn expose(&mut self, slot: usize, s: usize) -> Result<()> {
        let (key, section_y) = {
            let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
            if loaded.exposed[s].is_some() {
                return Ok(());
            }
            (self.slots[slot].key, loaded.chunk.sections()[s].y)
        };
        let nx = self.edge((key.0 + 1, key.1), section_y, true)?;
        let nz = self.edge((key.0, key.1 + 1), section_y, false)?;

        let loaded = self.slots[slot].loaded.as_mut().expect("geladen");
        let above = section_y
            .checked_add(1)
            .and_then(|y| loaded.chunk.section_index(y))
            .and_then(|i| loaded.masks[i].as_deref());
        let m = loaded.masks[s]
            .as_deref()
            .expect("nur Sections mit Familie");
        let mut ex = Box::new(Exposed {
            own: [0; 256],
            skip_x: [0; 256],
            skip_y: [0; 256],
            skip_z: [0; 256],
            any_own: false,
        });
        for col in 0..256 {
            let (x, z) = (col & 15, col >> 4);
            let (sx, fx, ux) = if x < 15 { rand(m, col + 1) } else { nx[z] };
            let (sz, fz, uz) = if z < 15 { rand(m, col + 16) } else { nz[x] };
            let top = above.map_or(0, |a| a.bits[FLOOR][col] & 1);
            let floor_up = (m.bits[FLOOR][col] >> 1) | (top << 15);
            let hidden = sx & floor_up & sz;
            let mut fluid_hidden = 0;
            for (f, &(_, pure)) in FLUIDS.iter().enumerate() {
                fluid_hidden |= m.bits[pure][col]
                    & m.up[f][col]
                    & (sx | (fx[f] & ux[f]))
                    & (sz | (fz[f] & uz[f]));
            }
            ex.own[col] = m.bits[PRESENT][col] & (m.bits[LOOSE][col] | !(hidden | fluid_hidden));
        }
        ex.any_own = ex.own.iter().any(|&o| o != 0);
        // Deckende Kandidaten: die übermalen, was in ihrem Umriss liegt.
        let drawn = |col: usize| ex.own[col] & m.bits[PLAIN][col];
        for col in 0..256 {
            let (x, z) = (col & 15, col >> 4);
            ex.skip_x[col] = if x < 15 { drawn(col + 1) } else { 0 };
            ex.skip_z[col] = if z < 15 { drawn(col + 16) } else { 0 };
            ex.skip_y[col] = drawn(col) >> 1;
        }
        loaded.exposed[s] = Some(ex);
        Ok(())
    }

    /// Erster Durchgang: alle Blöcke im Band, von denen etwas zu sehen
    /// sein kann, samt der Würfel, in die fremde Modellteile hineinragen.
    fn candidates(
        &mut self,
        rect: ScreenRect,
        y_range: (i32, i32),
        foreign: &[Cell],
    ) -> Result<Vec<Candidate>> {
        let projection = self.sprites.projection();
        let (u_min, u_max) = u_window(projection, rect);
        let v_lo = v_window(projection, rect, y_range.0).0;
        let v_hi = v_window(projection, rect, y_range.1).1;
        debug_assert!(y_range.1 - y_range.0 < 1 << 10);
        debug_assert!(v_hi - v_lo < 1 << 22 && u_max - u_min < 1 << 22);
        debug_assert!(foreign.len() < 1 << 10);
        // Jeder Kandidat liegt im Band, also nie vor dessen Rand.
        let key_of = |y: i32, v: i32, u: i32, kind: u16| -> u64 {
            ((y - y_range.0) as u64) << 54
                | ((v - v_lo) as u64) << 32
                | ((u - u_min) as u64) << 10
                | kind as u64
        };
        let in_band = |y: i32, v: i32, u: i32| {
            if y < y_range.0 || y > y_range.1 || u < u_min || u > u_max {
                return false;
            }
            let (v_min, v_max) = v_window(projection, rect, y);
            v >= v_min && v <= v_max
        };
        // Ein fremdes Teil kann von einem Block ausserhalb des Bands
        // hereinragen; so weit reicht die Suche über das Band hinaus.
        let pad = foreign
            .iter()
            .map(|c| c[0].abs() + c[2].abs())
            .max()
            .unwrap_or(0);

        let pad_y = foreign.iter().map(|c| c[1].abs()).max().unwrap_or(0);
        let scale = projection.scale() as f64;
        let bleed = BLEED_BLOCKS as f64 * scale;
        // Höhen, die das Band in einem Chunk erreichen kann: die Umkehrung
        // von `v_window` für die kleinste und grösste Tiefe `v` des Chunks,
        // grosszügig gerundet. Entscheidend bleibt `in_band` je Block; das
        // hier spart nur die Schleife über Sections, die das Band in
        // diesem Chunk gar nicht berührt — von 24 sind es meist drei.
        let y_span = |va: i32, vb: i32| {
            let lo = ((va - 1) as f64 * scale / 4.0 - rect.bottom() as f64 - bleed) / (scale / 2.0);
            let hi = ((vb + 1) as f64 * scale / 4.0 - rect.y as f64 + bleed) / (scale / 2.0);
            (lo.floor() as i32 - 1 - pad_y, hi.ceil() as i32 + 1 + pad_y)
        };

        let mut out = Vec::with_capacity(8192);
        let mut anchors: Vec<[i32; 3]> = Vec::new();
        for key in band_chunks(u_min - pad, u_max + pad, v_lo - pad, v_hi + pad) {
            let slot = self.slot(key)?;
            let sections = match &self.slots[slot].loaded {
                Some(loaded) => loaded.chunk.sections().len(),
                None => continue,
            };
            let v0 = key.0 * 16 + key.1 * 16;
            let (y_lo, y_hi) = y_span(v0, v0 + 30);
            let in_reach = |sy: i8| {
                let base = sy as i32 * 16;
                base + 15 >= y_lo && base <= y_hi
            };
            for s in 0..sections {
                let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
                if loaded.masks[s].is_some() && in_reach(loaded.chunk.sections()[s].y) {
                    self.expose(slot, s)?;
                }
            }
            let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
            for (s, section) in loaded.chunk.sections().iter().enumerate() {
                let Some(m) = &loaded.masks[s] else {
                    continue;
                };
                if !in_reach(section.y) {
                    continue;
                }
                let ex = loaded.exposed[s].as_ref().expect("eben berechnet");
                if !ex.any_own && !m.any_foreign {
                    continue;
                }
                let sy = section.y as i32 * 16;
                for col in 0..256 {
                    let own = ex.own[col];
                    let fo = m.bits[FOREIGN][col];
                    if own == 0 && fo == 0 {
                        continue;
                    }
                    let x = key.0 * 16 + (col & 15) as i32;
                    let z = key.1 * 16 + (col >> 4) as i32;
                    let (u, v) = (x - z, x + z);
                    let mut bits = own;
                    while bits != 0 {
                        let b = bits.trailing_zeros();
                        let y = sy + b as i32;
                        bits &= bits - 1;
                        if !in_band(y, v, u) {
                            continue;
                        }
                        // Der Nachbar übermalt nur, was diese Kachel auch
                        // zeichnet: er muss selbst im Band liegen.
                        let mut skip = 0;
                        if ex.skip_x[col] >> b & 1 != 0 && in_band(y, v + 1, u + 1) {
                            skip |= mask_bit(Face::East);
                        }
                        if ex.skip_y[col] >> b & 1 != 0 && in_band(y + 1, v, u) {
                            skip |= mask_bit(Face::Up);
                        }
                        if ex.skip_z[col] >> b & 1 != 0 && in_band(y, v + 1, u - 1) {
                            skip |= mask_bit(Face::South);
                        }
                        out.push(Candidate {
                            key: key_of(y, v, u, 0),
                            x,
                            y,
                            z,
                            kind: 0,
                            skip,
                        });
                    }
                    let mut bits = fo;
                    while bits != 0 {
                        anchors.push([x, sy + bits.trailing_zeros() as i32, z]);
                        bits &= bits - 1;
                    }
                }
            }
        }

        // Fremde Teile: vom Anker aus in jeden Würfel, den ein Modell der
        // Familie belegen kann. Gezeichnet wird dort, wenn der Würfel im
        // Band liegt, auch wenn er verdeckt ist: die Zerlegung lässt jedem
        // Teil eine Pixelbreite Spielraum über seinen Würfel hinaus, und den
        // deckt kein Nachbar sicher, die nach +x und +z höchstens zum Teil.
        for anchor in anchors {
            for (i, cell) in foreign.iter().enumerate() {
                let [x, y, z] = [
                    anchor[0] + cell[0],
                    anchor[1] + cell[1],
                    anchor[2] + cell[2],
                ];
                let (u, v) = (x - z, x + z);
                if in_band(y, v, u) {
                    let kind = i as u16 + 1;
                    out.push(Candidate {
                        key: key_of(y, v, u, kind),
                        x,
                        y,
                        z,
                        kind,
                        skip: 0,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Was an einer Weltkoordinate zu zeichnen ist — nichts für Luft,
    /// fehlende Chunks und Blöcke ohne sichtbare Geometrie.
    ///
    /// Drei Entscheidungen fallen hier: welche Alternative die Position
    /// bekommt, welche Flüssigkeitsflächen die Nachbarn verdecken und
    /// welche Biomfassung gilt. Alles davon ist vorab gerastert.
    fn sprite_at(&mut self, x: i32, y: i32, z: i32) -> Result<Drawn> {
        let sprites = self.sprites;
        let Some(family) = self.family_at(x, y, z)? else {
            return Ok(Drawn::default());
        };
        let Some(id) = family.pick([x, y, z]) else {
            return Ok(Drawn::default());
        };
        let mut sprite = Some(id);
        let mut strips = [None; 2];

        if let Some((fluid, amount)) = family.fluid {
            let same = |other: Option<&Family>| {
                other.is_some_and(|other| other.fluid.is_some_and(|(kind, _)| kind == fluid))
            };
            // Steht dieselbe Flüssigkeit darüber, reicht die eigene bis zur
            // Kante, und die Oberseite entfällt.
            let above = same(self.family_at(x, y + 1, z)?);
            let own = if above { fluid::FULL } else { amount };
            let mut mask = if above { mask_bit(Face::Up) } else { 0 };

            // Zur selben Flüssigkeit nebenan nie eine Seitenfläche, wie
            // `shouldRenderFace` im Spiel. Sonst mischt sich jede innere
            // Fläche eines Beckens mit dazu, und ein Ozean wäre ein Raster
            // aus doppelt gedecktem Wasser. Steht der Nachbar tiefer,
            // bleibt über ihm ein Streifen der eigenen Seite frei: am Fuss
            // eines Wasserfalls, an jeder Stufe fliessenden Wassers.
            for (slot, (face, [dx, dz])) in [(Face::East, [1, 0]), (Face::South, [0, 1])]
                .into_iter()
                .enumerate()
            {
                let Some(other) = self.family_at(x + dx, y, z + dz)? else {
                    continue;
                };
                let Some((kind, other_amount)) = other.fluid else {
                    continue;
                };
                if kind != fluid {
                    continue;
                }
                mask |= mask_bit(face);
                if other_amount < own {
                    let below = if same(self.family_at(x + dx, y + 1, z + dz)?) {
                        fluid::FULL
                    } else {
                        other_amount
                    };
                    if below < own {
                        strips[slot] = sprites.strip(fluid, own, below, face);
                    }
                }
            }

            // Die Oberfläche trägt die Deckkraft des Wassers dahinter:
            // durch einen Block Wasser sieht man den Grund, durch vier nicht
            // mehr. Gezählt wird entlang des Blickstrahls, nicht senkrecht:
            // hinter der Oberseite von (x, y, z) liegt auf denselben Pixeln
            // die von (x-1, y-1, z-1). Was den Strahl aufhält, beendet die
            // Zählung — der Grund, das Ufer, ein Stein. Ob ein Block das
            // tut, hängt an der Höhe dieser Oberfläche, siehe
            // `Family::covers`: unter einer Quelle lassen Seegras und ein
            // Zaunpfosten den Strahl durch, vor flachem fliessendem Wasser
            // hält Seegras ihn auf.
            let mut depth = 0;
            while !above && depth + 1 < DEPTHS {
                let d = 1 + depth as i32;
                let behind = self.family_at(x - d, y - d, z - d)?;
                if !same(behind) || behind.is_some_and(|b| b.covers(own)) {
                    break;
                }
                depth += 1;
            }
            match sprites.masked(id, mask, depth) {
                Some(masked) => sprite = Some(masked),
                None if strips.iter().all(Option::is_none) => return Ok(Drawn::default()),
                None => sprite = None,
            }
        }

        // Das Biom kostet einen zweiten Nachschlag; `in_biome` fragt nur
        // fuer Sprites danach, die ueberhaupt Fassungen haben.
        let i = self.slot((x >> 4, z >> 4))?;
        let chunk = &self.slots[i].loaded.as_ref().expect("eben geladen").chunk;
        let tint = |id: SpriteId| sprites.in_biome(id, || chunk.biome_at(x, y, z));
        Ok(Drawn {
            sprite: sprite.map(tint),
            strips: strips.map(|strip| strip.map(tint)),
        })
    }

    /// Die Familie des Blocks an einer Weltkoordinate — ein Nachschlag im
    /// Chunk-Cache und zwei Indizes, ohne die Blockstate zu hashen.
    fn family_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<&'a Family>> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(None);
        };
        let Some((section, slot)) = loaded.chunk.slot(x, y, z) else {
            return Ok(None);
        };
        Ok(loaded
            .families
            .get(section)
            .and_then(|families| families.get(slot))
            .copied()
            .flatten()
            .map(|index| self.sprites.family(index)))
    }
}

/// Alle Chunks, die das Band `u ∈ [u_min, u_max]`, `v ∈ [v_lo, v_hi]`
/// berühren können — grob über die Hüllbox, dann je Chunk gegen das Band.
/// Ein paar Chunks zu viel schaden nicht: jeder Kandidat wird ohnehin
/// einzeln gegen das Band geprüft.
fn band_chunks(u_min: i32, u_max: i32, v_lo: i32, v_hi: i32) -> Vec<(i32, i32)> {
    // x = (u + v) / 2, z = (v - u) / 2
    let x0 = (u_min + v_lo).div_euclid(2) - 1;
    let x1 = (u_max + v_hi).div_euclid(2) + 1;
    let z0 = (v_lo - u_max).div_euclid(2) - 1;
    let z1 = (v_hi - u_min).div_euclid(2) + 1;
    let mut out = Vec::new();
    for cz in (z0 >> 4)..=(z1 >> 4) {
        for cx in (x0 >> 4)..=(x1 >> 4) {
            let (bx0, bx1, bz0, bz1) = (cx * 16, cx * 16 + 15, cz * 16, cz * 16 + 15);
            if bx1 - bz0 < u_min || bx0 - bz1 > u_max || bx1 + bz1 < v_lo || bx0 + bz0 > v_hi {
                continue;
            }
            out.push((cx, cz));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> ScreenRect {
        ScreenRect::centered(64, 64)
    }

    #[test]
    fn rechteck_liegt_um_den_ursprung() {
        let r = ScreenRect::centered(64, 32);
        assert_eq!((r.x, r.y), (-32, -16));
        assert_eq!((r.right(), r.bottom()), (32, 16));
    }

    /// u und v müssen dieselbe Parität haben, sonst wären x und z nicht
    /// ganzzahlig.
    #[test]
    fn spalten_sind_ganzzahlig_und_eindeutig() {
        let spalten: Vec<(i32, i32)> = columns_at(Projection::new(16), rect(), 0).collect();
        assert!(!spalten.is_empty());

        let mut gesehen = std::collections::HashSet::new();
        for (x, z) in &spalten {
            assert!(gesehen.insert((*x, *z)), "Spalte ({x}, {z}) doppelt");
        }
    }

    /// Jede Spalte, die im Rechteck landet, muss auch besucht werden.
    #[test]
    fn spalten_decken_das_rechteck_ab() {
        let projection = Projection::new(16);
        let rect = rect();
        let y = 5;
        let besucht: std::collections::HashSet<(i32, i32)> =
            columns_at(projection, rect, y).collect();

        for x in -40..40 {
            for z in -40..40 {
                let (sx, sy) = projection.project([x as f32, y as f32, z as f32]);
                let drin = sx >= rect.x as f32
                    && sx < rect.right() as f32
                    && sy >= rect.y as f32
                    && sy < rect.bottom() as f32;
                if drin {
                    assert!(besucht.contains(&(x, z)), "({x}, {z}) fehlt");
                }
            }
        }
    }

    /// Die Referenz verlässt sich darauf, dass die Tiefe `v = x + z`
    /// innerhalb einer Höhe nie fällt; der Schlüssel der Kandidaten sortiert
    /// genauso.
    #[test]
    fn spalten_kommen_nach_tiefe_sortiert() {
        let mut vorher = i32::MIN;
        let mut gesehen = 0;
        for (x, z) in columns_at(Projection::new(16), rect(), 7) {
            assert!(x + z >= vorher, "v fällt von {vorher} auf {}", x + z);
            vorher = x + z;
            gesehen += 1;
        }
        assert!(gesehen > 100, "nur {gesehen} Spalten geprüft");
    }

    /// Eine höhere Ebene verschiebt das Band nach unten in der Welt.
    #[test]
    fn hoehere_ebene_verschiebt_das_band() {
        let mitte = |y| {
            let v: Vec<(i32, i32)> = columns_at(Projection::new(16), rect(), y).collect();
            let summe: i32 = v.iter().map(|(x, z)| x + z).sum();
            summe / v.len() as i32
        };
        assert!(mitte(64) > mitte(0));
    }

    /// Jede Chunkspalte, die das Band berührt, ist dabei: sonst fehlten der
    /// Kachel Kandidaten, und die Referenz zeichnete sie.
    #[test]
    fn band_chunks_decken_das_band_ab() {
        let projection = Projection::new(16);
        let rect = rect();
        let (u_min, u_max) = u_window(projection, rect);
        let v_lo = v_window(projection, rect, -64).0;
        let v_hi = v_window(projection, rect, 319).1;
        let chunks: std::collections::HashSet<(i32, i32)> =
            band_chunks(u_min, u_max, v_lo, v_hi).into_iter().collect();
        for y in [-64, 0, 100, 319] {
            for (x, z) in columns_at(projection, rect, y) {
                assert!(chunks.contains(&(x >> 4, z >> 4)), "({x}, {z}) auf {y}");
            }
        }
    }

    #[test]
    fn deckendes_pixel_ersetzt_den_untergrund() {
        assert_eq!(
            over([10, 20, 30, 255], [200, 200, 200, 255]),
            [10, 20, 30, 255]
        );
    }

    #[test]
    fn halbdurchsichtiges_pixel_mischt() {
        let out = over([0, 0, 0, 128], [255, 255, 255, 255]);
        assert_eq!(out[3], 255);
        assert!((120..=135).contains(&out[0]), "{out:?}");
    }

    #[test]
    fn auf_leerem_grund_bleibt_die_quelle() {
        assert_eq!(over([10, 20, 30, 128], [0, 0, 0, 0]), [10, 20, 30, 128]);
    }
}
