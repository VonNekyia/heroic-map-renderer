use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

use anyhow::Result;
use image::RgbaImage;

use crate::assets::Face;
use crate::assets::blockstate::{self, DUNKELT, Leuchten, Lichtweg, SICHT, SICHT_262, seite};
use crate::assets::colors::Resolver;
use crate::assets::fluid;
use crate::assets::fluid::Fluid;
use crate::world::{BlockState, Chunk, REGION, Region, Section, World};

use super::kino::{Bloompuffer, Himmelsfarben, Kino, Lichtstufe};
use super::licht::{Ausbreitung, ChunkLicht, Eingabe};
use super::projection::Umkehrung;
use super::pyramid::LINEAR;
use super::rasterizer::{
    AO_FACES, AO_PLAETZE, Ecken, Geometrie, Light, VOLL_HELL, darken, ecken_faktor, over, pack,
    smooth_blend, tinted, tinted_im_licht,
};
use super::sonne::texel_mitte;
use super::sprites::{Family, Rows, TINT_BLOCK, TINT_WATER, mask_bit};
use super::{Cell, OWN_CELL, Projection, Richtung, Sprite, SpriteId, SpriteSet};

mod strahl;

pub(crate) use strahl::{Versatz, ohne_null, versaetze};

/// Reserve um das Zielrechteck herum, in Blockbreiten.
///
/// Sprites dürfen über den Blockumriss hinausragen — Feuer ist höher als
/// ein Block, Zäune breiter. Ohne diese Reserve fehlen an den Rändern
/// Blöcke, deren Ursprung knapp ausserhalb liegt. Sie gilt nur für solche
/// Sprites und für Teile in fremden Würfeln; alles, was im Umriss seines
/// Würfels bleibt, prüft die Kandidatensuche gegen den Umriss.
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
/// erst nach Höhe `y`, innerhalb einer Höhe nach Tiefe `v`, diagonal
/// `x + z`, genordet `z`. Ein Würfel, der einen anderen verdeckt, liegt nie
/// tiefer, und auf gleicher Höhe verdeckt er ihn genau bei grösserem `v`. Ein Modell, das über
/// seinen Würfel hinausragt, ist in `SpriteSet` bereits in Teile je Würfel
/// zerlegt. Die Reihenfolge kommt aus dem Schlüssel `(y, v, u, Teil)`,
/// nach dem die Kandidaten sortiert werden, siehe [`render_area_with`].
/// Siehe docs/renderer/kamera.md, „Zeichenreihenfolge“.
/// Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
///
/// Grössere Ausschnitte als [`STUECK`] entstehen Stück für Stück, Zeile für
/// Zeile, und werden zusammengesetzt. Ein Pixel hängt nur an der Welt und an
/// seinem Platz, nicht am Rechteck; darauf beruhen auch die Kacheln.
pub fn render_area(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let spalten = rect.width.div_ceil(STUECK) as usize;
    let mut chunks = ChunkCache::with_row(world, sprites, spalten);
    if rect.width <= STUECK && rect.height <= STUECK {
        return render_area_with(&mut chunks, rect, y_range);
    }
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    for y in (0..rect.height).step_by(STUECK as usize) {
        for x in (0..rect.width).step_by(STUECK as usize) {
            let stueck = ScreenRect {
                x: rect.x + x as i32,
                y: rect.y + y as i32,
                width: STUECK.min(rect.width - x),
                height: STUECK.min(rect.height - y),
            };
            let bild = render_area_with(&mut chunks, stueck, y_range)?;
            image::imageops::replace(&mut canvas, &bild, i64::from(x), i64::from(y));
        }
    }
    Ok(canvas)
}

/// Kantenlänge der Stücke von [`render_area`], in Pixeln. Kandidaten,
/// Deckungsmaske und die sichtbaren Pixel der Draws wachsen mit der Fläche
/// eines Stücks, nicht mit der des ganzen Bilds.
/// Siehe docs/renderer/renderpfad.md, „Grosse Ausschnitte“.
pub const STUECK: u32 = 1024;

/// Wie [`render_area`], mit einem Cache, der über Kacheln hinweg lebt:
/// Kacheln, die nacheinander kommen, teilen sich fast alle Chunks.
///
/// Drei Durchgänge. Der erste sammelt die Kandidaten — Blöcke, von denen
/// etwas zu sehen sein kann — aus den Bitmasken der Sections, ohne einen
/// einzigen Luftblock anzufassen, und sortiert sie in die
/// Zeichenreihenfolge. Der zweite läuft rückwärts, von vorn nach hinten,
/// über eine Maske je Leinwandpixel: "hier liegt schon ein deckender
/// Pixel" ([`Deckung`]). Ein Block, dessen ganzer Umriss bedeckt ist,
/// bekommt keine Sprite-Wahl; ein Sprite, von dem nichts mehr durchscheint,
/// fällt weg; von den anderen merkt er sich die sichtbaren Pixel. Der
/// dritte zeichnet nur die, in der alten Reihenfolge. Übersprungen wird
/// also nur, was ein späterer Draw ohnehin mit Alpha 255 übermalt, und das
/// Bild ist dasselbe wie das von [`render_area_without_culling`], das jeden
/// Block im Band abläuft.
/// Siehe docs/renderer/sprites-und-deckung.md, „Deckungsmaske“.
///
/// Für Cinematic zeichnet derselbe dritte Durchgang in HDR, siehe
/// [`render_hdr_with`].
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let sprites = chunks.sprites;
    if let Some(kino) = sprites.kino() {
        // Mit einem Rand für den Bloom, siehe `Hdr::bild`.
        let scale = sprites.projection().scale();
        let rand = 3 * kino.bloom_radius(scale) as u32;
        let gross = ScreenRect {
            x: rect.x - rand as i32,
            y: rect.y - rand as i32,
            width: rect.width + 2 * rand,
            height: rect.height + 2 * rand,
        };
        // Ohne leuchtenden Block bleibt das Leuchten überall 0 und der Bloom
        // leer: Der Rand trägt nichts bei, innen ist jeder Pixel derselbe.
        let (rect, rand) = match rand > 0 && chunks.leuchtet_im_band(gross, y_range)? {
            true => (gross, rand),
            false => (rect, 0),
        };
        // Leinwand und Puffer des Bloom bleiben im Cache für die nächste
        // Kachel.
        let mut hdr = std::mem::take(&mut chunks.hdr);
        render_hdr(chunks, rect, y_range, false, rand, &mut hdr)?;
        let bild = hdr.bild(kino, scale, rand, &mut chunks.bloom);
        chunks.hdr = hdr;
        return Ok(bild);
    }
    let deckung = von_vorn(chunks, rect, y_range)?;
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    for &(sprite, origin, ref sicht, licht) in chunks.sichtbar.iter().rev() {
        blit_sichtbar(&mut canvas, sprite, origin, licht, sicht, &deckung.vis);
    }
    chunks.vis = deckung.vis;
    Ok(canvas)
}

/// Wie [`render_area_with`] für Cinematic, vor dem Ton: dieselben Draws mit
/// denselben sichtbaren Pixeln, gezeichnet in HDR ([`Hdr`]). Die
/// Sprite-Tabelle muss einen Look haben.
/// Siehe docs/renderer/cinematic.md, „Zeichnen in HDR“.
pub fn render_hdr_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<Hdr> {
    let mut hdr = Hdr::default();
    render_hdr(chunks, rect, y_range, false, 0, &mut hdr)?;
    Ok(hdr)
}

/// Wie [`render_hdr_with`], nur mit dem langsamen Bezug des Strahls zur
/// Sonne ([`ChunkCache::sonne_bezug`]): für Tests, die den schnellen Gang
/// gegen ihn prüfen.
pub fn render_hdr_bezug(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<Hdr> {
    let mut hdr = Hdr::default();
    render_hdr(chunks, rect, y_range, true, 0, &mut hdr)?;
    Ok(hdr)
}

/// Wie [`render_hdr_with`], in `hdr`; `rand` Pixel am Rand zeichnet es nur
/// für den Bloom, ohne Strahlen zur Sonne.
fn render_hdr(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
    bezug: bool,
    rand: u32,
    hdr: &mut Hdr,
) -> Result<()> {
    let sprites = chunks.sprites;
    let kino = sprites.kino().expect("eine Sprite-Tabelle für Cinematic");
    let umkehrung = sprites.projection().umkehrung();
    let deckung = von_vorn(chunks, rect, y_range)?;
    hdr.leeren(rect.width, rect.height);
    let innen = ScreenRect {
        x: rand as i32,
        y: rand as i32,
        width: rect.width - 2 * rand,
        height: rect.height - 2 * rand,
    };
    // Die Draws aus dem Cache genommen, denn der Strahl zur Sonne braucht ihn.
    let sichtbar = std::mem::take(&mut chunks.sichtbar);
    let kinodaten = std::mem::take(&mut chunks.kinodaten);
    debug_assert_eq!(sichtbar.len(), kinodaten.len());
    for (&(sprite, origin, ref sicht, licht), &daten) in sichtbar.iter().zip(&kinodaten).rev() {
        blit_hdr(
            hdr,
            (kino, sprites.projection(), &umkehrung, &innen),
            sprite,
            origin,
            licht,
            daten,
            sicht,
            &deckung.vis,
            &mut |p0, eigen| {
                if bezug {
                    chunks.sonne_bezug(p0, eigen)
                } else {
                    chunks.sonne(p0, eigen)
                }
            },
        )?;
    }
    chunks.sichtbar = sichtbar;
    chunks.kinodaten = kinodaten;
    chunks.vis = deckung.vis;
    Ok(())
}

/// Die Leinwand von Cinematic: je Pixel die Farbe in linearem Licht,
/// vormultipliziert, samt Alpha, und die Tiefe des vordersten Pixels darauf
/// entlang der Blickachse wie [`Projection::depth`], `-∞` ohne Pixel.
#[derive(Default)]
pub struct Hdr {
    pub width: u32,
    pub farbe: Vec<[f32; 4]>,
    /// In f64: so ist auch die Strecke durch Wasser weit draussen genau.
    pub tiefe: Vec<f64>,
    /// Die Wärme des vordersten Pixels, aus dem Biom seines Blocks, siehe
    /// [`Kino::waerme`]; 1 ohne Pixel.
    pub waerme: Vec<f32>,
    /// Das Leuchten je Pixel, linear und vormultipliziert, gemischt wie die
    /// Farbe: die Quelle des Bloom.
    pub leuchten: Vec<[f32; 3]>,
}

impl Hdr {
    /// Leer für ein Bild aus `width` × `height` Pixeln, im Speicher von
    /// vorher.
    fn leeren(&mut self, width: u32, height: u32) {
        let n = width as usize * height as usize;
        self.width = width;
        self.farbe.clear();
        self.farbe.resize(n, [0.0; 4]);
        self.tiefe.clear();
        self.tiefe.resize(n, f64::NEG_INFINITY);
        self.waerme.clear();
        self.waerme.resize(n, 1.0);
        self.leuchten.clear();
        self.leuchten.resize(n, [0.0; 3]);
    }

    /// Das Bild nach dem Ton aus [`Kino::ton`], ohne einen Rand von `rand`
    /// Pixeln: Der Rand trägt nur sein Leuchten zum Bloom bei
    /// ([`Kino::bloom`] bei `scale`, in `puffer`). Alpha bleibt, ein Pixel
    /// ohne Block durchsichtig; auf ihn fällt kein Bloom.
    /// Siehe docs/renderer/cinematic.md, „Bloom“.
    pub fn bild(&self, kino: &Kino, scale: u32, rand: u32, puffer: &mut Bloompuffer) -> RgbaImage {
        let w = self.width as usize;
        let h = self.farbe.len() / w.max(1);
        let rand = rand as usize;
        let bloom = kino.bloom(&self.leuchten, &self.waerme, w, scale, puffer);
        let mut bild = RgbaImage::new((w - 2 * rand) as u32, (h - 2 * rand) as u32);
        for (i, pixel) in bild.pixels_mut().enumerate() {
            let p = (i / (w - 2 * rand) + rand) * w + i % (w - 2 * rand) + rand;
            let [r, g, b, a] = self.farbe[p];
            if a <= 0.0 {
                continue;
            }
            let zusatz = bloom.as_ref().map_or([0.0; 3], |b| b[p]);
            let [r, g, b] = kino.ton([r / a, g / a, b / a], self.waerme[p], zusatz);
            pixel.0 = [r, g, b, (a * 255.0).round() as u8];
        }
        bild
    }
}

/// Der erste und der zweite Durchgang von [`render_area_with`]: die
/// Kandidaten von vorn nach hinten über die Deckungsmaske. Wer bleibt,
/// steht danach von vorn nach hinten in `chunks.sichtbar`, seine sichtbaren
/// Pixel in der Maske.
fn von_vorn<'a>(
    chunks: &mut ChunkCache<'a>,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<Deckung<'a>> {
    let (candidates, foreign) = chunks.sorted_candidates(rect, y_range)?;
    let sprites: &'a SpriteSet = chunks.sprites;
    let projection = sprites.projection();
    let mut deckung = Deckung::new(
        rect,
        sprites.outline_rows(),
        std::mem::take(&mut chunks.vis),
    );
    let mut sichtbar = std::mem::take(&mut chunks.sichtbar);
    sichtbar.clear();
    let mut kinodaten = std::mem::take(&mut chunks.kinodaten);
    kinodaten.clear();
    let kino = sprites.kino();
    for c in candidates.iter().rev() {
        let (anchor, cell) = match c.kind {
            0 => ([c.x, c.y, c.z], OWN_CELL),
            kind => {
                let cell = foreign[kind as usize - 1];
                (anchor_of([c.x, c.y, c.z], cell), cell)
            }
        };
        // Nur Teile, die in ihrem Würfel bleiben, liegen ganz im Umriss.
        if c.kind == 0 && !c.loose && deckung.bedeckt(block_origin(projection, rect, anchor)) {
            continue;
        }
        let drawn = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?;
        // Fremde Teile ohne die Streifen, die gehören zum eigenen Würfel.
        // Von vorn nach hinten kommen die Streifen vor dem Block.
        let ids = if c.kind == 0 {
            drawn
        } else {
            Drawn {
                strips: [None; 2],
                ..drawn
            }
        };
        // Für Cinematic die Tiefe des Blockursprungs, siehe `Hdr::tiefe`,
        // und der Block, von dem der Strahl zur Sonne ausgeht.
        let tiefe = if kino.is_some() {
            projection.depth_block(anchor)
        } else {
            0.0
        };
        let mut himmel = None;
        for id in ids.ids().rev() {
            if let Some((sprite, rows)) = sprites.part_rows(id, cell) {
                let origin = origin_of(projection, rect, anchor, sprite);
                if let Some(sicht) = deckung.zeichne(sprite, rows, origin) {
                    sichtbar.push((sprite, origin, sicht, ids.licht()));
                    if let Some(kino) = kino {
                        let himmel = match himmel {
                            Some(h) => h,
                            None => *himmel.insert(chunks.himmel_at(kino, anchor)?),
                        };
                        kinodaten.push(Kinodaten {
                            himmel,
                            ursprung: tiefe,
                            anker: anchor,
                            leuchten: ids.leuchten,
                        });
                    }
                }
            }
        }
    }
    chunks.sichtbar = sichtbar;
    chunks.kinodaten = kinodaten;
    Ok(deckung)
}

/// Zeichnet eine Liste auf die Leinwand, Draw für Draw ganz: die
/// Vergleichsgrösse für die Karte bei Listen, die kein Ausschnitt liefert.
pub fn draw_all(canvas: &mut RgbaImage, draws: &[Draw]) {
    for d in draws {
        blit(
            canvas,
            d.sprite,
            d.origin,
            (d.licht, d.ecken, d.wasser, d.tint),
        );
    }
}

/// Ein Sprite-Teil an seinem Platz auf der Leinwand, für die Grafikkarte
/// ([`draw_list`], [`super::gpu::Worker`]). Sie zeichnet dasselbe Bild wie
/// die CPU.
#[derive(Clone, Copy)]
pub struct Draw<'a> {
    pub sprite: &'a Sprite,
    /// Linke obere Ecke des Sprites in Leinwandpixeln; darf über den Rand
    /// hinausragen.
    pub origin: (i32, i32),
    /// Das Licht seines Blocks für Pixel ohne Seite, je Kanal in 255steln,
    /// Rot zuerst
    /// ([`Lightmap::factors`](super::rasterizer::Lightmap::factors)),
    /// siehe [`ChunkCache::licht_fuer`].
    pub licht: [u32; 3],
    /// Das Licht an den Ecken seiner Seiten, siehe [`ChunkCache::ecken_at`];
    /// `None`, wo es überall `licht` gleicht.
    pub ecken: Option<Ecken>,
    /// Das Licht für den Anteil des Wassers in der Tönungskarte, wenn es
    /// nicht `licht` ist: bei einem gefluteten Block an der Oberfläche.
    pub wasser: Option<[u32; 3]>,
    /// Die Farben seines Blocks für die Tönungskarte, Block und Wasser,
    /// siehe [`ChunkCache::tints_at`].
    pub tint: [u32; 2],
}

/// Die Zeichenliste eines Ausschnitts, in Zeichenreihenfolge, für die
/// Grafikkarte: dieselben Draws, die [`render_area_with`] behält. Die Karte
/// zeichnet jeden ganz; was davon verdeckt ist, übermalt ein späterer Draw
/// mit Alpha 255, und das Bild bleibt dasselbe.
pub fn draw_list<'a>(
    chunks: &mut ChunkCache<'a>,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<Vec<Draw<'a>>> {
    let deckung = von_vorn(chunks, rect, y_range)?;
    let draws = chunks
        .sichtbar
        .iter()
        .rev()
        .map(|&(sprite, origin, _, (licht, ecken, wasser, tint))| Draw {
            sprite,
            origin,
            licht,
            ecken,
            wasser,
            tint,
        })
        .collect();
    chunks.vis = deckung.vis;
    Ok(draws)
}

/// Wie [`render_area`], aber Block für Block über das ganze Band, ohne
/// Kandidaten, ohne verdeckte Würfel auszulassen und ohne Pixel zu
/// überspringen: die Referenz, gegen die Tests den schnellen Weg prüfen.
/// Er darf kein Pixel ändern. Nur für die Karte: Cinematic zeichnet
/// dieselben Draws, das prüft ein eigener Test.
pub fn render_area_without_culling(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    assert!(
        sprites.kino().is_none(),
        "die Referenz zeichnet nur die Karte"
    );
    let projection = sprites.projection();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    let mut chunks = ChunkCache::new(world, sprites);

    // Die Teile anderer Blöcke in diesem Würfel.
    let fremde = |chunks: &mut ChunkCache, canvas: &mut RgbaImage, pos: [i32; 3]| -> Result<()> {
        for &cell in sprites.foreign_cells() {
            let anchor = anchor_of(pos, cell);
            let drawn = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?;
            if let Some(id) = drawn.sprite
                && let Some(part) = sprites.part(id, cell)
            {
                let origin = origin_of(projection, rect, anchor, part);
                blit(canvas, part, origin, drawn.licht());
            }
        }
        Ok(())
    };
    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            // Wie in `candidates`: vor einem Block in Würfelform, sonst nach ihm.
            let vor = chunks.family_at(x, y, z)?.is_some_and(|f| f.wuerfelform);
            if vor {
                fremde(&mut chunks, &mut canvas, [x, y, z])?;
            }
            let drawn = chunks.sprite_at(x, y, z)?;
            for id in drawn.ids() {
                if let Some(part) = sprites.part(id, OWN_CELL) {
                    let origin = origin_of(projection, rect, [x, y, z], part);
                    blit(&mut canvas, part, origin, drawn.licht());
                }
            }
            if !vor {
                fremde(&mut chunks, &mut canvas, [x, y, z])?;
            }
        }
    }

    Ok(canvas)
}

/// Wo der Ursprung eines Blocks auf der Leinwand liegt: der Bildpunkt
/// seiner Ecke mit den kleinsten Koordinaten (`Projection::project_block`).
fn block_origin(projection: Projection, rect: ScreenRect, anchor: [i32; 3]) -> (i32, i32) {
    let (sx, sy) = projection.project_block(anchor);
    (sx.round() as i32 - rect.x, sy.round() as i32 - rect.y)
}

/// Linke obere Ecke eines Sprites auf der Leinwand, wenn sein Block bei
/// `anchor` steht; sie darf über den Rand hinausragen.
fn origin_of(
    projection: Projection,
    rect: ScreenRect,
    anchor: [i32; 3],
    sprite: &Sprite,
) -> (i32, i32) {
    let (bx, by) = block_origin(projection, rect, anchor);
    (bx + sprite.offset.0, by + sprite.offset.1)
}

/// Der Block, dessen Modell in `cell` hineinragen würde.
fn anchor_of([x, y, z]: [i32; 3], cell: Cell) -> [i32; 3] {
    [x - cell[0], y - cell[1], z - cell[2]]
}

/// Eine Seite, wie `BlockModelLighter` sie sieht, per javap am 26.2-Client.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AoSeite {
    /// Wohin sie zeigt.
    richtung: [i32; 3],
    /// Die vier Nachbarn in ihrer Ebene, wie `AdjacencyInfo.corners`.
    nachbarn: [[i32; 3]; 4],
    /// An welche Ecke aus `FaceInfo` der Wert `i` geht:
    /// `AmbientVertexRemap`.
    remap: [usize; 4],
}

/// Die Seiten der Welt, die eine Kamera aus einer ihrer Richtungen sieht:
/// oben und die vier rundum.
const AO_WELT: [(Face, AoSeite); 5] = [
    // Oben: Osten, Westen, Norden, Süden.
    (
        Face::Up,
        AoSeite {
            richtung: [0, 1, 0],
            nachbarn: [[1, 0, 0], [-1, 0, 0], [0, 0, -1], [0, 0, 1]],
            remap: [2, 3, 0, 1],
        },
    ),
    // Norden: oben, unten, Osten, Westen.
    (
        Face::North,
        AoSeite {
            richtung: [0, 0, -1],
            nachbarn: [[0, 1, 0], [0, -1, 0], [1, 0, 0], [-1, 0, 0]],
            remap: [3, 0, 1, 2],
        },
    ),
    // Süden: Westen, Osten, unten, oben.
    (
        Face::South,
        AoSeite {
            richtung: [0, 0, 1],
            nachbarn: [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0]],
            remap: [0, 1, 2, 3],
        },
    ),
    // Westen: oben, unten, Norden, Süden.
    (
        Face::West,
        AoSeite {
            richtung: [-1, 0, 0],
            nachbarn: [[0, 1, 0], [0, -1, 0], [0, 0, -1], [0, 0, 1]],
            remap: [3, 0, 1, 2],
        },
    ),
    // Osten: unten, oben, Norden, Süden.
    (
        Face::East,
        AoSeite {
            richtung: [1, 0, 0],
            nachbarn: [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]],
            remap: [1, 2, 3, 0],
        },
    ),
];

/// Die Seiten aus [`AO_FACES`] im Blick aus `richtung`: je Seite die der
/// Welt, die dort liegt, mit ihren Nachbarn in den Blick gedreht. Ihre
/// Werte legt [`ChunkCache::ecken_at`] auf die Ecken derselben Seite der
/// Welt, wie `rasterizer::ecken_im_blick` sie im Blick zeigt.
/// Siehe docs/renderer/weiche-beleuchtung.md, „Aus jeder Richtung“.
fn ao_seiten(richtung: Richtung) -> [AoSeite; 3] {
    AO_FACES.map(|blick| {
        let welt = richtung.seite_in_die_welt(blick);
        let (_, seite) = AO_WELT
            .iter()
            .find(|(face, _)| *face == welt)
            .expect("oben oder rundum");
        AoSeite {
            richtung: richtung.versatz_in_den_blick(seite.richtung),
            nachbarn: seite.nachbarn.map(|n| richtung.versatz_in_den_blick(n)),
            remap: seite.remap,
        }
    })
}
/// Der Wert einer Ecke, wenn so viele ihrer vier Blöcke abdunkeln:
/// `ARGB.gray` des Mittels aus 1 und 0,2, in f32 wie im Spiel.
const AO_WERTE: [u32; 5] = [255, 204, 153, 102, 51];

/// Was an einem Würfel zu zeichnen ist: das Sprite des Blocks, dazu die
/// Streifen seiner Flüssigkeit über niedrigeren Nachbarn, alles im Licht
/// des Blocks, siehe [`ChunkCache::licht_fuer`], an den Ecken seiner Seiten
/// weich beleuchtet, siehe [`ChunkCache::ecken_at`], und in den Farben
/// seines Bioms, siehe [`ChunkCache::tints_at`].
#[derive(Clone, Copy)]
struct Drawn {
    sprite: Option<SpriteId>,
    strips: [Option<SpriteId>; 2],
    licht: [u32; 3],
    ecken: Option<Ecken>,
    wasser: Option<[u32; 3]>,
    tint: [u32; 2],
    /// `getLightEmission` / 15, für Cinematic.
    leuchten: f32,
}

impl Default for Drawn {
    fn default() -> Drawn {
        Drawn {
            sprite: None,
            strips: [None; 2],
            licht: [255; 3],
            ecken: None,
            wasser: None,
            tint: [0; 2],
            leuchten: 0.0,
        }
    }
}

impl Drawn {
    /// Sein Licht für [`blit`].
    fn licht(self) -> Licht {
        (self.licht, self.ecken, self.wasser, self.tint)
    }

    /// In Zeichenreihenfolge: die Streifen nach dem Block, sie liegen auf
    /// seiner Grenze, also vor allem, was er selbst enthält.
    fn ids(self) -> impl DoubleEndedIterator<Item = SpriteId> {
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
    /// Die Familie bleibt nicht in ihrem Würfel (`LOOSE`): kein Test des
    /// Umrisses gegen die Deckungsmaske.
    loose: bool,
}

/// Bereich von `u`, dessen Spalten in das Rechteck fallen können.
///
/// `screen_x = u * h`. f64, weil rect und Weltkoordinaten bis knapp
/// 30 Millionen gehen: siehe Projection::project_block.
fn u_window(projection: Projection, rect: ScreenRect) -> (i32, i32) {
    let bleed = BLEED_BLOCKS as f64 * projection.scale() as f64;
    let h = projection.h();
    (
        ((rect.x as f64 - bleed) / h).floor() as i32,
        ((rect.right() as f64 + bleed) / h).ceil() as i32,
    )
}

/// Bereich von `v` auf dieser Höhe: `screen_y = v * a - y * b`,
/// nach aussen gerundet. Von oben ist er für jede Höhe derselbe.
fn v_window(projection: Projection, rect: ScreenRect, y: i32) -> (i32, i32) {
    let bleed = BLEED_BLOCKS as f64 * projection.scale() as f64;
    let offset = y as f64 * projection.b();
    let a = projection.a();
    (
        ((rect.y as f64 - bleed + offset) / a).floor() as i32,
        ((rect.bottom() as f64 + bleed + offset) / a).ceil() as i32,
    )
}

/// Alle Blockspalten, deren Sprite auf dieser Höhe in das Rechteck fallen
/// kann — in Zeichenreihenfolge, so wie sie die Referenz abläuft.
///
/// Statt über x und z zu laufen, läuft die Schleife über die beiden
/// Bildschirmachsen: `u` steuert die waagerechte, `v` die senkrechte
/// Position, diagonal `x - z` und `x + z`, genordet `x` und `z`. Damit ist
/// der Bereich je Höhe diagonal ein schmales Band statt der gesamten
/// Grundfläche.
///
/// `v` läuft aussen: Auf einer Höhe ist `v` die Tiefe entlang der
/// Blickachse, und liefe `u` aussen, käme der Südnachbar zu früh und würde
/// übermalt.
/// Siehe docs/renderer/kamera.md, „Zeichenreihenfolge“.
fn columns_at(
    projection: Projection,
    rect: ScreenRect,
    y: i32,
) -> impl Iterator<Item = (i32, i32)> {
    let (u_min, u_max) = u_window(projection, rect);
    let (v_min, v_max) = v_window(projection, rect, y);
    let genordet = projection.kamera().genordet();

    (v_min..=v_max).flat_map(move |v| {
        // Diagonal sind x und z ganzzahlig, also haben u und v dieselbe
        // Parität.
        let (start, schritt) = if genordet {
            (u_min, 1)
        } else {
            (u_min + (u_min - v).rem_euclid(2), 2)
        };
        (start..=u_max).step_by(schritt).map(move |u| {
            if genordet {
                (u, v)
            } else {
                ((u + v) / 2, (v - u) / 2)
            }
        })
    })
}

/// Das Licht eines Blocks, das an den Ecken seiner Seiten, das seines
/// Wassers und die Farben für seine Tönungskarte, wie [`Drawn`] sie trägt.
type Licht = ([u32; 3], Option<Ecken>, Option<[u32; 3]>, [u32; 2]);

/// [`Licht`] ohne die Farben, wie [`ChunkCache::licht_fuer`] es gibt.
type Lichter = ([u32; 3], Option<Ecken>, Option<[u32; 3]>);

/// Was ein Draw für Cinematic dazu trägt.
#[derive(Clone, Copy)]
struct Kinodaten {
    /// Die Farben des Himmels an seinem Block ([`ChunkCache::himmel_at`]).
    himmel: Himmelsfarben,
    /// Die Tiefe des Blockursprungs, siehe [`Hdr::tiefe`].
    ursprung: f64,
    /// Der Block, dem das Modell gehört, im Blick.
    anker: [i32; 3],
    /// Wie hell der Block leuchtet, `getLightEmission` / 15.
    leuchten: f32,
}

/// Ein Draw, der bleibt, wie [`von_vorn`] ihn ablegt: der Sprite-Teil,
/// seine linke obere Ecke auf der Leinwand, seine sichtbaren Pixel und sein
/// Licht. Was Cinematic dazu braucht, steht daneben in [`Kinodaten`].
type Sichtbar<'a> = (&'a Sprite, (i32, i32), Sicht, Licht);

/// Was Cinematic statt der Helligkeit in die drei Kanäle von [`Ecken`] und
/// des Lichts eines Draws legt: Himmels- und Blocklicht getrennt, in
/// Sechzehnteln einer Stufe wie [`smooth_blend`] und [`Light::packed`] sie
/// liefern, und den Schatten der weichen Beleuchtung in 255steln. Die Farbe
/// des Lichts kommt erst beim Zeichnen dazu.
/// Siehe docs/renderer/cinematic.md, „Licht an den Ecken“.
fn kino_kanaele(licht: u32, schatten: u32) -> [u32; 3] {
    [licht >> 16 & 255, licht & 255, schatten]
}

/// Je Licht das hellere zweier Zellen, gepackt wie [`Light::packed`]:
/// `LightCoordsUtil.max`.
fn hellstes(a: u32, b: u32) -> u32 {
    (a & 0xf0).max(b & 0xf0) | (a & 0xf0_0000).max(b & 0xf0_0000)
}

/// Die Faktoren für [`darken`] an Pixel `i` eines Sprites: auf einer Seite
/// der AO-Karte das Licht ihrer Ecken, sonst `licht`.
#[inline]
fn faktor(karte: Option<(&[u32], &Ecken)>, i: usize, licht: [u32; 3]) -> [u32; 3] {
    match karte {
        Some((karte, ecken)) if karte[i] >> 24 != 0 => {
            ecken.map(|kanal| ecken_faktor(karte[i], kanal))
        }
        _ => licht,
    }
}

/// Die AO-Karte eines Sprites mit dem Licht an den Ecken, wenn das nicht
/// überall gleich ist.
fn karte<'s>(sprite: &'s Sprite, ecken: &'s Option<Ecken>) -> Option<(&'s [u32], &'s Ecken)> {
    sprite.ao.as_deref().zip(ecken.as_ref())
}

/// Die Tönungskarte an Pixel `i` eines Sprites samt den Farben des Blocks,
/// für [`tinted`].
#[inline]
fn anteile(sprite: &Sprite, i: usize, farben: [u32; 2]) -> Option<([u32; 2], [u32; 2])> {
    let karte = sprite.tint.as_deref()?;
    Some(([karte[2 * i], karte[2 * i + 1]], farben))
}

/// Zeichnet ein Sprite an seinen Block, ganz, im Licht, mit der weichen
/// Beleuchtung und in den Farben seines Blocks.
fn blit(
    canvas: &mut RgbaImage,
    sprite: &Sprite,
    (origin_x, origin_y): (i32, i32),
    (licht, ecken, wasser, farben): Licht,
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
    let karte = karte(sprite, &ecken);
    for py in y0..y1 {
        let row = &src[py as usize * w * 4..][..w * 4];
        let drow = &mut dst[(origin_y + py) as usize * cw * 4..][..cw * 4];
        for px in x0..x1 {
            let s = &row[px as usize * 4..][..4];
            if s[3] != 0 {
                let i = py as usize * w + px as usize;
                let f = faktor(karte, i, licht);
                let t = anteile(sprite, i, farben);
                mische(
                    &mut drow[(origin_x + px) as usize * 4..][..4],
                    s,
                    f,
                    wasser,
                    t,
                );
            }
        }
    }
}

/// Legt einen Pixel in den Farben seines Blocks und im Licht `factor` aus
/// [`Lightmap::factors`](super::rasterizer::Lightmap::factors)
/// über den darunter, den Anteil des Wassers in seiner Tönungskarte im
/// Licht `wasser`, wenn es eines hat; deckende direkt statt durch
/// [`over`]. Erst die Farbe, dann das Licht, wie im Spiel.
#[inline]
fn mische(
    d: &mut [u8],
    s: &[u8],
    factor: [u32; 3],
    wasser: Option<[u32; 3]>,
    tint: Option<([u32; 2], [u32; 2])>,
) {
    let mut s = [s[0], s[1], s[2], s[3]];
    match (tint, wasser) {
        (Some((anteile, farben)), Some(wasser)) => {
            s = tinted_im_licht(s, anteile, farben, factor, wasser);
        }
        _ => {
            if let Some((anteile, farben)) = tint {
                s = tinted(s, anteile, farben);
            }
            if factor != [255; 3] {
                s = darken(s, factor);
            }
        }
    }
    if s[3] == 255 {
        d.copy_from_slice(&s);
    } else {
        let out = over(s, [d[0], d[1], d[2], d[3]]);
        d.copy_from_slice(&out);
    }
}

/// Die Deckungsmaske von [`render_area_with`]: je Leinwandpixel ein Bit,
/// "hier liegt schon ein deckender Pixel von weiter vorn", dazu die
/// sichtbaren Pixel jedes Draws, der bleibt.
struct Deckung<'a> {
    width: i32,
    height: i32,
    /// Wörter je Leinwandzeile; Bit `x % 64` von Wort `x / 64` steht für
    /// Spalte `x`.
    words: usize,
    bits: Vec<u64>,
    /// Die Zeilen eines Blockumrisses, siehe [`SpriteSet::outline_rows`].
    umriss: &'a [(i32, i32, i32)],
    /// Die sichtbaren Pixel aller Draws, die bleiben, je Draw eine [`Sicht`].
    vis: Vec<u64>,
}

/// Wo die sichtbaren Pixel eines Draws in [`Deckung::vis`] stehen: für
/// jede Leinwandzeile `y0..y1` die Wörter `k0..k0 + nk`.
struct Sicht {
    start: usize,
    y0: i32,
    y1: i32,
    k0: usize,
    nk: usize,
}

impl<'a> Deckung<'a> {
    /// Eine leere Maske über `rect`; `vis` ist ein Puffer aus einer
    /// früheren Kachel, er wächst sonst je Kachel von null an.
    fn new(rect: ScreenRect, umriss: &'a [(i32, i32, i32)], mut vis: Vec<u64>) -> Deckung<'a> {
        vis.clear();
        let words = (rect.width as usize).div_ceil(64);
        Deckung {
            width: rect.width as i32,
            height: rect.height as i32,
            words,
            bits: vec![0; words * rect.height as usize],
            umriss,
            vis,
        }
    }

    /// Liegt jeder Pixel des Umrisses um diesen Blockursprung schon unter
    /// einem deckenden, oder neben der Leinwand?
    fn bedeckt(&self, (bx, by): (i32, i32)) -> bool {
        self.umriss.iter().all(|&(dy, dx0, dx1)| {
            let y = by + dy;
            let (x0, x1) = ((bx + dx0).max(0), (bx + dx1).min(self.width - 1));
            if y < 0 || y >= self.height || x0 > x1 {
                return true;
            }
            let row = &self.bits[y as usize * self.words..][..self.words];
            (x0 >> 6..=x1 >> 6).all(|k| {
                let lo = (x0 - 64 * k).max(0) as u32;
                let hi = (x1 - 64 * k).min(63) as u32;
                let maske = (u64::MAX >> (63 - hi)) & (u64::MAX << lo);
                row[k as usize] & maske == maske
            })
        })
    }

    /// Nimmt ein Sprite von vorn nach hinten auf: merkt sich, was davon
    /// noch zu sehen ist, und deckt mit seinen deckenden Pixeln, was
    /// dahinter kommt. `None`, wenn nichts mehr zu sehen ist.
    fn zeichne(&mut self, sprite: &Sprite, rows: &Rows, (ox, oy): (i32, i32)) -> Option<Sicht> {
        let (w, h) = (sprite.image.width() as i32, sprite.image.height() as i32);
        let (x0, x1) = (ox.max(0), (ox + w).min(self.width));
        let (y0, y1) = (oy.max(0), (oy + h).min(self.height));
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let (k0, k1) = ((x0 >> 6) as usize, ((x1 - 1) >> 6) as usize);
        // Das letzte Wort einer Zeile reicht über die Leinwand hinaus.
        let rand = |k: usize| match self.width % 64 {
            r if r != 0 && k == self.words - 1 => (1 << r) - 1,
            _ => u64::MAX,
        };
        let start = self.vis.len();
        let mut zu_sehen = false;
        for y in y0..y1 {
            let (any, full) = rows.row((y - oy) as usize);
            let row = &mut self.bits[y as usize * self.words..][..self.words];
            for (k, deckt) in row.iter_mut().enumerate().take(k1 + 1).skip(k0) {
                let v = wort(any, ox, k) & rand(k) & !*deckt;
                zu_sehen |= v != 0;
                self.vis.push(v);
                *deckt |= wort(full, ox, k) & rand(k);
            }
        }
        if !zu_sehen {
            self.vis.truncate(start);
            return None;
        }
        Some(Sicht {
            start,
            y0,
            y1,
            k0,
            nk: k1 - k0 + 1,
        })
    }
}

/// Die Bits einer Sprite-Zeile, die in Leinwandwort `k` fallen, wenn das
/// Sprite bei Leinwandspalte `ox` beginnt.
fn wort(row: &[u64], ox: i32, k: usize) -> u64 {
    let start = 64 * k as i32 - ox;
    let (j, s) = (start.div_euclid(64), start.rem_euclid(64) as u32);
    let at = |j: i32| {
        usize::try_from(j)
            .ok()
            .and_then(|j| row.get(j))
            .copied()
            .unwrap_or(0)
    };
    match s {
        0 => at(j),
        s => at(j) >> s | at(j + 1) << (64 - s),
    }
}

/// Zeichnet die sichtbaren Pixel eines Draws, die [`Deckung::zeichne`]
/// gemerkt hat, im Licht seines Blocks wie [`blit`].
fn blit_sichtbar(
    canvas: &mut RgbaImage,
    sprite: &Sprite,
    (ox, oy): (i32, i32),
    (licht, ecken, wasser, farben): Licht,
    sicht: &Sicht,
    vis: &[u64],
) {
    let (w, cw) = (sprite.image.width() as usize, canvas.width() as usize);
    let src = sprite.image.as_raw();
    let dst: &mut [u8] = canvas;
    let karte = karte(sprite, &ecken);
    let zeilen = vis[sicht.start..].chunks(sicht.nk);
    for (y, woerter) in (sicht.y0..sicht.y1).zip(zeilen) {
        let row = &src[(y - oy) as usize * w * 4..][..w * 4];
        let drow = &mut dst[y as usize * cw * 4..][..cw * 4];
        for (j, &bits) in woerter.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let x = (sicht.k0 + j) * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let sx = (x as i32 - ox) as usize;
                let i = (y - oy) as usize * w + sx;
                mische(
                    &mut drow[x * 4..][..4],
                    &row[sx * 4..][..4],
                    faktor(karte, i, licht),
                    wasser,
                    anteile(sprite, i, farben),
                );
            }
        }
    }
}

/// Zeichnet die sichtbaren Pixel eines Draws für Cinematic in HDR, wie
/// [`blit_sichtbar`] sie für die Karte zeichnet: je Pixel die Farbe in den
/// Farben des Bioms, linear, mal ihr Licht, über den darunter gelegt wie
/// [`over`], vormultipliziert. Das Licht rechnet [`Kino::licht`] wie das
/// Spiel je Ecke ([`EckenLicht`]); an einem Pixel auf einer Seite der
/// AO-Karte mischen sich die der Ecken mit denselben Anteilen wie bei der
/// Karte, ungerundet. Die Sonne kommt dazu, so weit sie durchkommt: `sonne`
/// gibt das für einen Punkt im Blick und den Block des Draws, siehe
/// [`ChunkCache::sonne`]. Die Tiefe ist die des vordersten gezeichneten
/// Pixels.
/// Siehe docs/renderer/cinematic.md, „Zeichnen in HDR“.
#[allow(clippy::too_many_arguments)]
fn blit_hdr(
    hdr: &mut Hdr,
    (kino, projection, umkehrung, innen): (&Kino, Projection, &Umkehrung, &ScreenRect),
    sprite: &Sprite,
    (ox, oy): (i32, i32),
    (licht, ecken, wasser, farben): Licht,
    Kinodaten {
        himmel,
        ursprung,
        anker,
        leuchten,
    }: Kinodaten,
    sicht: &Sicht,
    vis: &[u64],
    sonne: &mut dyn FnMut([f64; 3], [i32; 3]) -> Result<f32>,
) -> Result<()> {
    let (w, cw) = (sprite.image.width() as usize, hdr.width as usize);
    let src = sprite.image.as_raw();
    // Wasser hat keine Seite in der AO-Karte und liegt vorn im Licht des
    // Blocks, wie im Spiel ohne weiche Beleuchtung.
    let stufen = licht.map(|c| c as f32);
    let licht = EckenLicht::new(kino, licht, ecken);
    let waerme = kino.look().waerme(himmel.temperatur);
    let leuchten = leuchten * kino.look().leuchten;
    let karte = sprite.ao.as_deref();
    let nass = wasser.map(|[s, b, a]| kino.licht(s as f32, b as f32, a as f32));
    // Das Licht des Wassers, beim ersten Pixel aus Wasser gerechnet.
    let mut wasserlicht: Option<Wasserlicht> = None;
    let linear = &*LINEAR;
    let zeilen = vis[sicht.start..].chunks(sicht.nk);
    // Pixel auf demselben Texel beginnen am selben Punkt.
    let mut letzter: Option<([f64; 3], f32)> = None;
    // Der Blick vom Auge in die Szene, Länge 1, und wie viel Tiefe ein Block
    // Strecke entlang des Blicks ist.
    let achse = projection.achse();
    let je_block = (achse[0] * achse[0] + achse[1] * achse[1] + achse[2] * achse[2]).sqrt();
    let blick = achse.map(|c| -c / je_block);
    for (y, woerter) in (sicht.y0..sicht.y1).zip(zeilen) {
        let row = &src[(y - oy) as usize * w * 4..][..w * 4];
        for (j, &bits) in woerter.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let x = (sicht.k0 + j) * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let sx = (x as i32 - ox) as usize;
                let i = (y - oy) as usize * w + sx;
                let s = &row[sx * 4..][..4];
                let p = y as usize * cw + x;
                let mut sonnenlicht = [0.0; 3];
                // Im Rand für den Bloom zählt nur das Leuchten.
                let drin = (innen.x..innen.right()).contains(&(x as i32))
                    && (innen.y..innen.bottom()).contains(&y);
                if let Some(g) = sprite.geometrie.as_deref().map(|g| &g[i]).filter(|_| drin) {
                    sonnenlicht = kino.sonnenlicht(g);
                    if sonnenlicht != [0.0; 3] {
                        let p0 = startpunkt(umkehrung, sprite, (sx, (y - oy) as usize), g, anker);
                        let frei = match letzter {
                            Some((q, frei)) if q == p0 => frei,
                            _ => sonne(p0, anker)?,
                        };
                        letzter = Some((p0, frei));
                        sonnenlicht = sonnenlicht.map(|c| c * frei);
                    }
                }
                let s = [s[0], s[1], s[2], s[3]];
                let tint = anteile(sprite, i, farben);
                // Über leerem Grund mischt Wasser wie bei der Karte: Ohne Grund
                // gibt es keine Strecke, und der Pixel bleibt so offen.
                let grund = hdr.tiefe[p] > f64::NEG_INFINITY;
                match (sprite.geometrie.as_deref().map(|g| &g[i]), tint) {
                    (Some(g), Some(tint)) if g.wasser > 0.0 && grund => {
                        // Bis zum Grund: der vorderste Pixel darunter.
                        let dahinter = ursprung + f64::from(g.tiefe) - hdr.tiefe[p];
                        let strecke = (dahinter as f32 / je_block).max(0.0);
                        let unten = Unten {
                            licht: wasserlicht
                                .get_or_insert_with(|| Wasserlicht::new(kino, stufen, wasser)),
                            sonne: sonnenlicht,
                            leuchten,
                        };
                        let wasser = Wasserpixel {
                            alpha: g.wasser,
                            normale: g.normale,
                            blick,
                            strecke,
                        };
                        mische_wasser(
                            (&mut hdr.farbe[p], &mut hdr.leuchten[p]),
                            kino,
                            s,
                            tint,
                            &himmel,
                            unten,
                            wasser,
                        );
                    }
                    _ => mische_hdr(
                        (&mut hdr.farbe[p], &mut hdr.leuchten[p]),
                        kino,
                        linear,
                        s,
                        licht.am(karte, i),
                        nass,
                        tint,
                        (sonnenlicht, leuchten),
                    ),
                }
                if let Some(geometrie) = &sprite.geometrie {
                    hdr.tiefe[p] = ursprung + f64::from(geometrie[i].tiefe);
                }
                hdr.waerme[p] = waerme;
            }
        }
    }
    Ok(())
}

/// Das Licht eines Pixels für [`mische_wasser`]: das des Draws, die Sonne
/// und wie stark der Block leuchtet, mal [`Look::leuchten`].
///
/// [`Look::leuchten`]: super::look::Look::leuchten
struct Unten<'a> {
    licht: &'a Wasserlicht,
    sonne: [f32; 3],
    leuchten: f32,
}

/// Das Licht eines Draws für [`mische_wasser`], einmal je Draw: das des
/// Blocks, das des Wassers, das Streulicht und die Stufen, in denen es den
/// Himmel spiegelt.
struct Wasserlicht {
    licht: [f32; 3],
    nass: [f32; 3],
    streu: [f32; 3],
    spiegel: Lichtstufe,
}

impl Wasserlicht {
    /// Aus Himmels-, Blocklicht und Schatten des Blocks in den Kanälen von
    /// [`kino_kanaele`] und dem des Wassers, falls es in einem anderen
    /// liegt, wie bei [`mische_hdr`].
    fn new(kino: &Kino, [sky, block, schatten]: [f32; 3], wasser: Option<[u32; 3]>) -> Wasserlicht {
        // Das Wasser liegt im Licht seiner Zelle, ein gefluteter Block an der
        // Oberfläche im helleren darüber.
        let [ws, wb, wa] = wasser.map_or([sky, block, schatten], |w| w.map(|c| c as f32));
        Wasserlicht {
            licht: kino.licht(sky, block, schatten),
            nass: kino.licht(ws, wb, wa),
            streu: kino.licht(ws, 0.0, wa),
            spiegel: kino.lichtstufe(ws, 0.0, wa),
        }
    }
}

/// Was [`mische_wasser`] über die Wasserfläche eines Pixels weiss: ihr
/// Alpha und ihre Normale aus [`Geometrie`], der Blick vom Auge in die
/// Szene, Länge 1, und die Strecke bis zum Pixel darunter in Blöcken,
/// unendlich ohne.
struct Wasserpixel {
    alpha: f32,
    normale: [f32; 3],
    blick: [f32; 3],
    strecke: f32,
}

/// Legt einen Pixel, dessen vorderstes Fragment Wasser ist, für Cinematic
/// über den darunter, wie der Prototyp aus #89 von vorn nach hinten: Die
/// Fläche spiegelt den Himmel nach Fresnel ([`Kino::spiegel`]); den Rest
/// deckt die Textur des Wassers mit [`Look::wasser_textur`] ihres Alphas;
/// was dahinter im selben Sprite liegt, etwa ein gefluteter Block, folgt
/// ohne Strecke; darunter dämpft das Wasser den Pixel darunter nach der
/// Strecke ([`Kino::wasser_dichte`]) und füllt mit `water_fog_color` im
/// Himmelslicht. Spiegelung und Streulicht liegen im Himmelslicht des
/// Wassers. Wasser und Rest trennt die Tönungskarte wie in
/// [`mische_hdr`].
/// Siehe docs/renderer/cinematic.md, „Wasser“.
///
/// [`Look::wasser_textur`]: super::look::Look::wasser_textur
fn mische_wasser(
    (d, l): (&mut [f32; 4], &mut [f32; 3]),
    kino: &Kino,
    s: [u8; 4],
    (anteile, farben): ([u32; 2], [u32; 2]),
    himmel: &Himmelsfarben,
    unten: Unten,
    w: Wasserpixel,
) {
    let linear = &*LINEAR;
    let Wasserlicht {
        licht,
        nass,
        streu,
        spiegel: stufe,
    } = *unten.licht;
    let (f, spiegel) = kino.spiegel(himmel, w.blick, w.normale);
    let spiegel = stufe.mal(spiegel);
    let farbe = tinted(s, anteile, farben);
    let a_s = f32::from(s[3]) / 255.0;
    let ([block_k, water_k], [b, wf]) = (anteile, farben);
    // Vormultipliziert: was vom Wasser kommt und was dahinter liegt.
    let mut nass_p = [0.0f32; 3];
    let mut rest_p = [0.0f32; 3];
    for c in 0..3 {
        let byte = |word: u32| (word >> (8 * c) & 255) as f32;
        let rest = f32::from(s[c]) + byte(block_k) * byte(b) / 255.0;
        let anteil = byte(water_k) * byte(wf) / 255.0;
        let summe = rest + anteil;
        let q = if summe > 0.0 { anteil / summe } else { 0.0 };
        let p = linear[farbe[c] as usize] * a_s;
        nass_p[c] = p * q;
        rest_p[c] = p * (1.0 - q);
    }
    let a_w = w.alpha.min(a_s);
    let farbe_w = nass_p.map(|c| c / a_w);
    let sigma = kino.wasser_dichte(farbe_w);
    let a1 = a_w * kino.look().wasser_textur;
    let (innen, a_m) = if a_w < 1.0 {
        (
            rest_p.map(|c| c / (1.0 - a_w)),
            ((a_s - a_w) / (1.0 - a_w)).clamp(0.0, 1.0),
        )
    } else {
        ([0.0; 3], 0.0)
    };
    let durch = sigma.map(|sg| (-sg * w.strecke).exp());
    // Ein gefluteter Block, der leuchtet, leuchtet auch unter Wasser.
    let e = match unten.leuchten > 0.0 && a_m > 0.0 {
        true => unten.leuchten * kino.look().leuchtet(innen.map(|c| c / a_m)),
        false => 0.0,
    };
    // Das Streulicht füllt nur, wo darunter etwas deckt; Alpha wie `over`,
    // damit der Pixel so offen bleibt wie bei der Karte.
    for c in 0..3 {
        let grund = durch[c] * d[c] + (1.0 - durch[c]) * himmel.wassernebel[c] * streu[c] * d[3];
        let unter = innen[c] * (licht[c] + unten.sonne[c] + e) + (1.0 - a_m) * grund;
        let wasser = a1 * farbe_w[c] * (nass[c] + unten.sonne[c]) + (1.0 - a1) * unter;
        d[c] = f * spiegel[c] + (1.0 - f) * wasser;
        // Das Leuchten darunter dämpft das Wasser wie die Farbe.
        l[c] = (1.0 - f) * (1.0 - a1) * (innen[c] * e + (1.0 - a_m) * durch[c] * l[c]);
    }
    d[3] = a_s + d[3] * (1.0 - a_s);
}

/// Wo der Strahl zur Sonne für Pixel `(sx, sy)` eines Sprites beginnt, im
/// Blick: am Punkt der vordersten Fläche dort, auf einer achsparallelen
/// Fläche in der Mitte seines Sechzehntels, ein Tausendstel davor.
/// Siehe docs/renderer/cinematic.md, „Schatten“.
fn startpunkt(
    umkehrung: &Umkehrung,
    sprite: &Sprite,
    (sx, sy): (usize, usize),
    g: &Geometrie,
    anker: [i32; 3],
) -> [f64; 3] {
    let bildpunkt = (
        f64::from(sprite.offset.0) + sx as f64 + 0.5,
        f64::from(sprite.offset.1) + sy as f64 + 0.5,
    );
    let p = texel_mitte(umkehrung.punkt(bildpunkt, g.tiefe.into()), g.normale);
    std::array::from_fn(|k| f64::from(anker[k]) + p[k] + f64::from(g.normale[k]) * 1e-3)
}

/// Das Licht eines Draws für Cinematic in HDR, wie das Spiel es je Ecke
/// aus der Lightmap liest (`terrain.vsh`): das des Blocks und je Seite der
/// AO-Karte das an ihren vier Ecken, aus den Kanälen von [`kino_kanaele`].
/// Über eine Seite verläuft dann das fertige Licht, nicht die Stufen.
/// Siehe docs/renderer/cinematic.md, „Licht in HDR“.
struct EckenLicht {
    block: [f32; 3],
    ecken: Option<[[[f32; 3]; 4]; AO_PLAETZE]>,
}

impl EckenLicht {
    fn new(kino: &Kino, licht: [u32; 3], ecken: Option<Ecken>) -> EckenLicht {
        let hdr = |[s, b, a]: [u32; 3]| kino.licht(s as f32, b as f32, a as f32);
        EckenLicht {
            block: hdr(licht),
            ecken: ecken.map(|ecken| {
                std::array::from_fn(|seite| {
                    std::array::from_fn(|i| hdr(ecken.map(|kanal| kanal[seite] >> (8 * i) & 255)))
                })
            }),
        }
    }

    /// Das Licht an Pixel `i` eines Sprites mit der AO-Karte `karte`: auf
    /// einer ihrer Seiten das der Ecken mit den Anteilen wie
    /// [`ecken_faktor`], ungerundet; sonst das des Blocks.
    #[inline]
    fn am(&self, karte: Option<&[u32]>, i: usize) -> [f32; 3] {
        match (karte, &self.ecken) {
            (Some(karte), Some(ecken)) if karte[i] >> 24 != 0 => {
                let word = karte[i];
                let seite = &ecken[(word >> 24) as usize - 1];
                let [w0, w1, w2] = [word & 255, word >> 8 & 255, word >> 16 & 255];
                let w = [w0, w1, w2, 255 - w0 - w1 - w2].map(|w| w as f32 / 255.0);
                std::array::from_fn(|c| (0..4).map(|k| w[k] * seite[k][c]).sum())
            }
            _ => self.block,
        }
    }
}

/// Legt einen Pixel für Cinematic über den darunter, siehe [`blit_hdr`]:
/// erst die Farbe des Bioms wie bei der Karte ([`tinted`]), dann linear mal
/// das Licht `licht`, das der Sonne (`sonne`) und das Leuchten: `leuchten`
/// ist die Stufe des Blocks / 15 mal [`Look::leuchten`], wie stark der Texel
/// leuchtet, sagt [`Look::leuchtet`]. Hat der Block Wasser in einem anderen
/// Licht (`nass`), liegt der Anteil des Wassers an der Farbe in dessen
/// Licht, wie [`tinted_im_licht`] es für die Karte rechnet.
/// Siehe docs/renderer/cinematic.md, „Leuchten“.
///
/// [`Look::leuchten`]: super::look::Look::leuchten
/// [`Look::leuchtet`]: super::look::Look::leuchtet
#[inline]
#[allow(clippy::too_many_arguments)]
fn mische_hdr(
    (d, l): (&mut [f32; 4], &mut [f32; 3]),
    kino: &Kino,
    linear: &[f32; 256],
    s: [u8; 4],
    mut licht: [f32; 3],
    nass: Option<[f32; 3]>,
    tint: Option<([u32; 2], [u32; 2])>,
    (sonne, leuchten): ([f32; 3], f32),
) {
    let mut farbe = s;
    if let Some((anteile, farben)) = tint {
        farbe = tinted(s, anteile, farben);
        if let Some(nass) = nass {
            let ([block, water], [b, w]) = (anteile, farben);
            licht = std::array::from_fn(|c| {
                let byte = |word: u32| (word >> (8 * c) & 255) as f32;
                let rest = s[c] as f32 + byte(block) * byte(b) / 255.0;
                let anteil = byte(water) * byte(w) / 255.0;
                let summe = rest + anteil;
                if summe > 0.0 {
                    (rest * licht[c] + anteil * nass[c]) / summe
                } else {
                    licht[c]
                }
            });
        }
    }
    let a = s[3] as f32 / 255.0;
    let lin = farbe.map(|c| linear[c as usize]);
    let e = match leuchten > 0.0 {
        true => leuchten * kino.look().leuchtet([lin[0], lin[1], lin[2]]),
        false => 0.0,
    };
    for c in 0..3 {
        d[c] = lin[c] * (licht[c] + sonne[c] + e) * a + d[c] * (1.0 - a);
        l[c] = lin[c] * e * a + l[c] * (1.0 - a);
    }
    d[3] = a + d[3] * (1.0 - a);
}

/// Wie viele Kachelspalten ein Streifen höchstens breit ist. Der Export
/// rendert Streifen Zeile für Zeile, und [`ChunkCache`] behält, was die
/// letzte Zeile gebraucht hat.
///
/// Diagonal ist eine Kachel ein schräger Schnitt durch die volle Bauhöhe:
/// ein Chunk liegt im Band von drei bis vier Kachelspalten und gut zwanzig
/// Zeilen. Genordet reicht das Band bei `north-45` in z über die Bauhöhe,
/// bei `top-north` liegt es unter der Kachel; die Breite ist dort nicht
/// eigens gemessen.
/// Spalte für Spalte lädt deshalb jede Kachel die Chunks am unteren Rand
/// ihrer ganzen Breite neu, samt Rand für Modelle, die überstehen. Über
/// mehrere Spalten nebeneinander teilen sich die Kacheln einer Zeile diesen
/// Rand. Breiter als acht Chunks in der Welt wird ein Streifen nicht: bei
/// scale 32 acht Spalten, ab scale 4 eine, immer eine Zweierpotenz.
/// Siehe docs/renderer/renderpfad.md, „Speicher“.
pub fn streifenbreite(scale: u32) -> usize {
    1 << (scale as usize / 4).max(1).ilog2()
}

/// Ab wie vielen Chunks ein Cache frühestens aufräumt. Er behält dann nur,
/// was eine Zeile eines Streifens gebraucht hat, die letzten `keep`
/// Kacheln; die nächste Zeile teilt sich fast alle davon. Danach räumt er
/// erst wieder, wenn ein Viertel dazugekommen ist: Eine Zeile braucht bei
/// scale 32 mehr als diese Grenze, und sonst räumte er vor jeder Kachel.
/// Mehr als diese Zeile, ein Viertel davon und die Chunks der laufenden
/// Kachel hält er nicht.
// ponytail: Verfallsdatum je Kachel statt echtem LRU. Reicht, solange die
// Kacheln in Streifen kommen; sonst lädt jede Kachel ihre hundert neu.
const CACHE_CHUNKS: usize = 256;

/// Hasht die Schlüssel des Chunk-Caches mit einer Multiplikation je Wort.
/// Siehe docs/renderer/renderpfad.md, „Streifen und Cache je Thread“.
#[derive(Default)]
pub(super) struct Streuer(u64);

impl Hasher for Streuer {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0xf135_7aea_2e62_a9c5);
    }

    fn write_i32(&mut self, n: i32) {
        self.write_u64(u64::from(n as u32));
    }

    fn write_i8(&mut self, n: i8) {
        self.write_u64(u64::from(n as u8));
    }

    /// Die oberen Bits der Multiplikation nach unten, wo die Tabelle den
    /// Eimer wählt.
    fn finish(&self) -> u64 {
        self.0.rotate_left(26)
    }
}

/// Eine Tabelle mit [`Streuer`].
pub(super) type Tabelle<K, V> = HashMap<K, V, BuildHasherDefault<Streuer>>;

/// Chunks, die während eines Renderlaufs gebraucht werden.
///
/// Ein Cache gehört zu einer Sprite-Tabelle: er hält je Paletteneintrag
/// den Familienindex daraus. Über Kacheln hinweg lebt er je Thread, der
/// Streifen Zeile für Zeile rendert.
/// Siehe docs/entscheidungen/0025-streifen-und-cache-je-thread.md.
pub struct ChunkCache<'a> {
    world: &'a World,
    sprites: &'a SpriteSet,
    /// Offene Regionsdateien. `World::chunk` würde die Datei für jeden
    /// Chunk neu öffnen — bei rund hundert Chunks je Kachel sind das
    /// hundert Öffnungen statt einer Handvoll.
    regions: HashMap<(i32, i32), Option<Region>>,
    slots: Vec<Slot>,
    index: Tabelle<(i32, i32), usize>,
    /// Der zuletzt benutzte Slot. Benachbarte Blöcke liegen fast immer im
    /// selben Chunk; der Merker spart das Hashen.
    last: usize,
    /// Laufende Kachelnummer — das Verfallsdatum der Slots.
    tile: u32,
    /// Wie viele Kacheln ein Slot überlebt, der nicht mehr gebraucht wird:
    /// eine Zeile eines Streifens.
    keep: u32,
    /// Ab wie vielen Slots `next_tile` wieder aufräumt, siehe
    /// [`CACHE_CHUNKS`].
    grenze: usize,
    /// Puffer der Deckungsmaske über Kacheln hinweg: die sichtbaren Pixel
    /// und die Draws, die bleiben.
    vis: Vec<u64>,
    sichtbar: Vec<Sichtbar<'a>>,
    /// Nur für Cinematic: je Draw in `sichtbar` seine [`Kinodaten`], die
    /// Leinwand in HDR und die Puffer des Bloom, über Kacheln hinweg.
    kinodaten: Vec<Kinodaten>,
    hdr: Hdr,
    bloom: Bloompuffer,
    /// Je Chunk und Höhe das Biom jedes Blocks nach [`BiomeTable::quart`],
    /// `u16::MAX`, solange es nicht gerechnet ist; siehe
    /// [`ChunkCache::biome_of`].
    ///
    /// [`BiomeTable::quart`]: super::BiomeTable::quart
    biome_layers: Vec<BiomeLayer>,
    biome_index: HashMap<(i32, i32, i32), usize>,
    biome_last: usize,
    /// Der Arbeitsplatz der Ausbreitung, siehe [`ChunkCache::licht_slot`].
    ausbreitung: Ausbreitung,
    /// Die Dimension hat Himmelslicht (`has_skylight`).
    himmel: bool,
    /// Nur für die nativen Stufen: dekodierte Chunks und ihr Licht über den
    /// Wechsel der Sprite-Tabelle hinweg, siehe [`ChunkCache::mit_vorrat`].
    vorrat: Option<Vorrat>,
    /// Woher die Kamera schaut. Slots, Bitmasken, Kandidaten und alle
    /// Koordinaten der Nachschläge liegen im Blick; Chunks, Licht, Biome und
    /// die Saat der Alternativen in der Welt.
    /// Siehe docs/renderer/richtungen.md.
    richtung: Richtung,
    /// Die Seiten für [`ChunkCache::ecken_at`], siehe [`ao_seiten`].
    ao_seiten: [AoSeite; 3],
}

/// Was von einem Chunk nicht am scale hängt: er selbst und sein Licht,
/// denn Blöcke, die `blocks.txt` nicht kennt, halten es nach dem Raster der Basis
/// auf ([`SpriteSet::deckt_fuer_licht`]), mit Cinematic dazu seine Bits
/// „frei zur Sonne“. Ein Cache der nativen Stufen
/// behält alles über [`ChunkCache::wechsle`] und über die Kacheln eines
/// Bands hinweg bis ins nächste; was ein ganzes Band lang niemand
/// brauchte, geht ([`ChunkCache::neues_band`]).
/// Siehe docs/entscheidungen/0043-native-stufen-in-baendern.md.
#[derive(Default)]
struct Vorrat {
    chunks: HashMap<(i32, i32), Gemerkt>,
    /// Laufende Nummer des Bands, das Verfallsdatum der Einträge.
    band: u32,
}

struct Gemerkt {
    /// `None` für einen Chunk, der fehlt oder nicht fertig ist.
    chunk: Option<Rc<Chunk>>,
    licht: Option<Rc<ChunkLicht>>,
    /// Nur für Cinematic: seine Bits „frei zur Sonne“.
    frei: Option<Rc<strahl::Frei>>,
    band: u32,
}

/// Chunk und Höhe, dazu das Biom je Block der Schicht.
type BiomeLayer = ((i32, i32, i32), Box<[u16; 256]>);

struct Slot {
    /// Der Chunk im Blick.
    key: (i32, i32),
    loaded: Option<Loaded>,
    used: u32,
}

/// Ein Chunk samt der Familie je Paletteneintrag. Die Blockstate wird
/// damit einmal je Section gehasht statt einmal je Block — im Renderpfad
/// war das der teuerste Schritt.
struct Loaded {
    chunk: Rc<Chunk>,
    families: Vec<Vec<Option<u32>>>,
    /// Je Section und Paletteneintrag, wie hell der Block selbst leuchtet:
    /// [`blockstate::leuchten`].
    leuchten: Vec<Vec<Leuchten>>,
    /// Je Section ihre Bitmasken, `None` für eine Section ohne Familie und
    /// ohne Block, der abdunkelt, die Sicht nimmt, das Licht aufhält oder
    /// leuchtet, siehe [`Masks::of`].
    masks: Vec<Option<Box<Masks>>>,
    /// Je Section die Kandidaten, sobald einmal berechnet — dafür müssen
    /// die Nachbarchunks da sein, deshalb nicht beim Laden.
    exposed: Vec<Option<Box<Exposed>>>,
    /// Das ausgebreitete Licht, sobald einmal berechnet, siehe
    /// [`ChunkCache::licht_slot`]: Dafür müssen die Nachbarn da sein.
    licht: Option<Rc<ChunkLicht>>,
    /// Je Section und Paletteneintrag der Biome die Nummer des Bioms in der
    /// [`BiomeTable`](super::BiomeTable).
    biomes: Vec<Vec<u16>>,
    /// Je Block, dessen Blockentity mit seinen Daten ein anderes Bild gibt,
    /// die Familie dafür ([`SpriteSet::variante`]), nach Lage sortiert.
    varianten: Vec<([i32; 3], u32)>,
    /// Nur für Cinematic: die Blöcke, deren Modell für die Sonne aus dem
    /// Würfel ragt ([`strahl::ragende`]), und die Säule des schnellen Gangs,
    /// beide sobald gebraucht.
    ragende: Option<Rc<[[i32; 3]]>>,
    sonne: Option<Box<strahl::Saeule>>,
}

/// Die Eigenschaften einer Familie, die über Verdeckung entscheiden, je
/// als Bit einer Maske in [`Masks`].
const PRESENT: usize = 0;
/// Deckt den ganzen Umriss: verdeckt, was hinter ihm liegt.
const SOLID: usize = 1;
/// Deckt den Boden des Würfels, die Oberseite des Blocks darunter.
const FLOOR: usize = 2;
/// Enthält Wasser, [`LAVA`] Lava: für dieselbe Flüssigkeit nebenan
/// dieselbe.
const WATER: usize = 3;
const LAVA: usize = 4;
/// Nur Wasser, [`PURE_LAVA`] nur Lava, ohne Modell daneben.
const PURE_WATER: usize = 5;
const PURE_LAVA: usize = 6;
/// Bleibt nicht in ihrem Würfel: nie überspringen.
const LOOSE: usize = 7;
/// Hat Teile in Nachbarwürfeln.
const FOREIGN: usize = 8;
/// Dunkelt ab ([`DUNKELT`]), [`VIEW`] nimmt in der Ecke die Sicht
/// ([`SICHT`], in einer Welt aus 26.2 [`SICHT_262`]), [`OPAQUE`] ist
/// `solidRender` ([`SICHT`]): die Bits aus [`blockstate::schatten`] für
/// [`ChunkCache::ecken_at`], auch für Blöcke ohne Familie.
const DARK: usize = 9;
const VIEW: usize = 10;
/// Dämpft das Licht ([`Lichtweg::daempfung`] nicht 0), [`DICHT`] lässt
/// keines hinein (15), auch für Blöcke ohne Familie: für die Ausbreitung,
/// siehe [`super::licht`].
const DAEMPFT: usize = 11;
const DICHT: usize = 12;
/// Zeichnet das Spiel voll hell ([`Leuchten::Voll`]), auch als Nachbar in
/// der weichen Beleuchtung.
const VOLL: usize = 13;
const OPAQUE: usize = 14;
const FLAGS: usize = 15;
/// Bit im Schlüssel einer Klasse in [`Masks::of`], über den Ebenen: ein
/// Eintrag mit einer Fläche aus [`Lichtweg::formen`] oder einer Quelle.
/// Seine Blöcke liest die Ausbreitung aus der Maske seiner Klasse.
const EINZELN: usize = FLAGS;
const _: () = assert!(EINZELN < u16::BITS as usize);
/// Je Flüssigkeit, in der Reihenfolge von [`Masks::up`]: das Bit "enthält
/// sie" und das Bit "nur sie".
const FLUIDS: [(usize, usize); 2] = [(WATER, PURE_WATER), (LAVA, PURE_LAVA)];

/// Bitmasken einer Section: je Eigenschaft und Spalte `z * 16 + x` ein
/// Wort, Bit `y`. Ob ein Block von seinen drei Nachbarn verdeckt ist, sind
/// damit für sechzehn Blöcke einer Spalte auf einmal ein paar
/// Wortoperationen. Die Spalten liegen im Blick, nur die für die
/// Ausbreitung, [`DAEMPFT`] und [`DICHT`], in der Welt wie `formen` und
/// `quellen`.
/// Siehe docs/renderer/renderpfad.md, „Bitmasken“.
struct Masks {
    bits: [[u16; 256]; FLAGS],
    /// Je Flüssigkeit aus [`FLUIDS`]: liegt über dem Block dieselbe, auch
    /// aus der Section darüber?
    up: [[u16; 256]; 2],
    /// Ragt irgendetwas in Nachbarwürfel?
    any_foreign: bool,
    /// Für die Ausbreitung: die Blöcke mit einer Fläche aus
    /// [`Lichtweg::formen`] und die, die leuchten, mit ihrer Stufe. Index
    /// `y << 8 | z << 4 | x`.
    formen: Vec<(u16, [u8; 6])>,
    quellen: Vec<(u16, u8)>,
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
        | bit(family.covers_floor, FLOOR)
        | bit(fluid(Fluid::Water), WATER)
        | bit(fluid(Fluid::Lava), LAVA)
        | bit(fluid(Fluid::Water) && family.pure_fluid, PURE_WATER)
        | bit(fluid(Fluid::Lava) && family.pure_fluid, PURE_LAVA)
        | bit(!family.contained, LOOSE)
        | bit(family.foreign, FOREIGN)
}

/// Wie ein Zustand das Licht aufhält: aus der Tabelle des Spiels,
/// [`blockstate::lichtweg`]. Einen Block, den `blocks.txt` nicht kennt, kennt sie
/// nicht; deckt sein Modell den ganzen Umriss
/// ([`SpriteSet::deckt_fuer_licht`]), lässt er wie ein Block mit voller
/// Form kein Licht hinein, sonst lässt er es durch.
fn lichtweg(state: &BlockState, sprites: &SpriteSet) -> Lichtweg {
    if blockstate::Definition::of(state.name()).is_some() {
        return blockstate::lichtweg(state);
    }
    Lichtweg {
        daempfung: if sprites.deckt_fuer_licht(state) {
            15
        } else {
            0
        },
        formen: [0; 6],
    }
}

impl Masks {
    /// `None`, wenn in der Section weder eine Familie steht noch ein Block,
    /// der abdunkelt, die Sicht nimmt, das Licht aufhält oder leuchtet.
    /// Je Paletteneintrag hat `schatten` die Bits aus
    /// [`blockstate::schatten`], `wege` den [`Lichtweg`] und `leuchten` das
    /// [`Leuchten`]. `blick` nennt je Spalte der Welt die im Blick, aus der
    /// Vorgabe-Richtung keine, siehe [`spalten_im_blick`]. `ecke` ist das Bit
    /// für [`VIEW`].
    fn of(
        section: &Section,
        families: &[Option<u32>],
        (schatten, wege, leuchten): (&[u8], &[Lichtweg], &[Leuchten]),
        (sprites, ecke): (&SpriteSet, u8),
        blick: Option<&[u8; 256]>,
    ) -> Option<Box<Masks>> {
        let bit = |set: bool, flag: usize| (set as u16) << flag;
        let flags: Vec<u16> = (0..families.len())
            .map(|p| {
                families[p].map_or(0, |index| flags(sprites.family(index)))
                    | bit(schatten[p] & DUNKELT != 0, DARK)
                    | bit(schatten[p] & ecke != 0, VIEW)
                    | bit(schatten[p] & SICHT != 0, OPAQUE)
                    | bit(wege[p].daempfung != 0, DAEMPFT)
                    | bit(wege[p].daempfung == 15, DICHT)
                    | bit(matches!(leuchten[p], Leuchten::Voll(_)), VOLL)
                    // Was die Ausbreitung Block für Block braucht: eine
                    // Fläche, eine Quelle.
                    | bit(wege[p].formen != [0; 6] || leuchten[p].stufe() > 0, EINZELN)
            })
            .collect();
        let union = flags.iter().fold(0, |acc, f| acc | f);
        if union == 0 {
            return None;
        }
        let mut m = Box::new(Masks {
            bits: [[0; 256]; FLAGS],
            up: [[0; 256]; 2],
            any_foreign: false,
            formen: Vec::new(),
            quellen: Vec::new(),
        });
        let (mut formen, mut quellen) = (Vec::new(), Vec::new());
        let mut nimm = |i: usize, p: usize| {
            if wege[p].formen != [0; 6] {
                formen.push((i as u16, wege[p].formen));
            }
            if leuchten[p].stufe() > 0 {
                quellen.push((i as u16, leuchten[p].stufe()));
            }
        };
        let blocks = section.blocks();
        if blocks.is_uniform() {
            let flag = flags.first().copied().unwrap_or(0);
            if flag >> EINZELN & 1 != 0 {
                (0..4096).for_each(|i| nimm(i, 0));
            }
            for (b, mask) in m.bits.iter_mut().enumerate() {
                if flag >> b & 1 != 0 {
                    *mask = [u16::MAX; 256];
                }
            }
        } else {
            // Die Familien einer Section fallen in wenige Klassen gleicher
            // Bits: Luft, deckender Stein, Wasser, eine Blume. Je Block
            // genügt ein OR in die Maske seiner Klasse; die Masken je
            // Eigenschaft setzen sich danach aus den Klassen zusammen. Die
            // Blöcke der Klassen mit EINZELN liest die Ausbreitung danach
            // Bit für Bit aus deren Masken, mit ihrem Paletteneintrag.
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
                let gedreht = blick.map(|blick| {
                    let mut gedreht = [0u16; 256];
                    for (col, &spalte) in maske.iter().enumerate() {
                        gedreht[blick[col] as usize] = spalte;
                    }
                    gedreht
                });
                for (b, bits) in m.bits.iter_mut().enumerate() {
                    if flag >> b & 1 != 0 {
                        let quelle = match &gedreht {
                            Some(gedreht) if b != DAEMPFT && b != DICHT => gedreht,
                            _ => maske,
                        };
                        for (bits, spalte) in bits.iter_mut().zip(quelle) {
                            *bits |= spalte;
                        }
                    }
                }
                if flag >> EINZELN & 1 != 0 {
                    for (col, &spalte) in maske.iter().enumerate() {
                        let mut rest = spalte;
                        while rest != 0 {
                            let i = (rest.trailing_zeros() as usize) << 8 | col;
                            nimm(i, blocks.index(i));
                            rest &= rest - 1;
                        }
                    }
                }
            }
        }
        (m.formen, m.quellen) = (formen, quellen);
        if [PRESENT, DARK, VIEW, DAEMPFT, DICHT, VOLL, OPAQUE]
            .iter()
            .all(|&e| m.bits[e].iter().all(|&w| w == 0))
            && m.formen.is_empty()
            && m.quellen.is_empty()
        {
            return None;
        }
        m.any_foreign = m.bits[FOREIGN].iter().any(|&f| f != 0);
        Some(m)
    }
}

/// Je Spalte `z * 16 + x` eines Chunks der Welt die Spalte im Blick aus
/// `richtung`; aus der Vorgabe-Richtung `None`.
fn spalten_im_blick(richtung: Richtung) -> Option<[u8; 256]> {
    (richtung != Richtung::default()).then(|| {
        std::array::from_fn(|col| {
            let [x, z] = richtung
                .in_den_blick([col as i32 & 15, col as i32 >> 4])
                .map(|c| c & 15);
            (z * 16 + x) as u8
        })
    })
}

impl Loaded {
    /// `ecke` wie in [`Masks::of`].
    fn new(chunk: Rc<Chunk>, sprites: &SpriteSet, ecke: u8) -> Loaded {
        let blick = spalten_im_blick(sprites.projection().richtung());
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
        let leuchten: Vec<Vec<Leuchten>> = chunk
            .sections()
            .iter()
            .map(|section| {
                section
                    .blocks()
                    .palette()
                    .iter()
                    .map(blockstate::leuchten)
                    .collect()
            })
            .collect();
        let mut masks: Vec<Option<Box<Masks>>> = chunk
            .sections()
            .iter()
            .zip(&families)
            .zip(&leuchten)
            .map(|((section, families), leuchten)| {
                let palette = section.blocks().palette();
                let schatten: Vec<u8> = palette.iter().map(blockstate::schatten).collect();
                let wege: Vec<Lichtweg> = palette
                    .iter()
                    .map(|state| lichtweg(state, sprites))
                    .collect();
                Masks::of(
                    section,
                    families,
                    (&schatten, &wege, leuchten),
                    (sprites, ecke),
                    blick.as_ref(),
                )
            })
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
        let biomes = chunk
            .sections()
            .iter()
            .map(|section| {
                section
                    .biomes()
                    .palette()
                    .iter()
                    .map(|name| sprites.biomes().id(name))
                    .collect()
            })
            .collect();
        let mut varianten: Vec<([i32; 3], u32)> = chunk
            .blockentities()
            .filter_map(|([x, y, z], daten)| {
                let (s, slot) = chunk.slot(x, y, z)?;
                let family = families[s][slot]?;
                Some(([x, y, z], sprites.variante(family, daten)?))
            })
            .collect();
        varianten.sort_unstable_by_key(|&(pos, _)| pos);
        Loaded {
            chunk,
            families,
            leuchten,
            masks,
            exposed,
            licht: None,
            biomes,
            varianten,
            ragende: None,
            sonne: None,
        }
    }
}

impl Loaded {
    /// Was die Ausbreitung aus diesem Chunk liest: die Sections mit
    /// Bitmasken, von unten nach oben.
    fn eingabe(&self) -> Vec<Eingabe<'_>> {
        self.chunk
            .sections()
            .iter()
            .zip(&self.masks)
            .filter_map(|(section, m)| {
                let m = m.as_deref()?;
                Some(Eingabe {
                    y: section.y,
                    dicht: &m.bits[DICHT],
                    daempft: &m.bits[DAEMPFT],
                    formen: &m.formen,
                    quellen: &m.quellen,
                })
            })
            .collect()
    }
}

impl<'a> ChunkCache<'a> {
    /// Ein Cache für Kacheln, die untereinander kommen.
    pub fn new(world: &'a World, sprites: &'a SpriteSet) -> ChunkCache<'a> {
        ChunkCache::with_row(world, sprites, 1)
    }

    /// Ein Cache für Streifen aus `tiles` Spalten, Zeile für Zeile: er
    /// behält, was die letzte Zeile gebraucht hat.
    pub fn with_row(world: &'a World, sprites: &'a SpriteSet, tiles: usize) -> ChunkCache<'a> {
        ChunkCache {
            world,
            sprites,
            regions: HashMap::new(),
            slots: Vec::new(),
            index: Tabelle::default(),
            last: usize::MAX,
            tile: 0,
            keep: tiles as u32,
            grenze: CACHE_CHUNKS,
            vis: Vec::new(),
            sichtbar: Vec::new(),
            kinodaten: Vec::new(),
            hdr: Hdr::default(),
            bloom: Bloompuffer::default(),
            biome_layers: Vec::new(),
            biome_index: HashMap::new(),
            biome_last: usize::MAX,
            ausbreitung: Ausbreitung::default(),
            himmel: sprites.himmel(),
            vorrat: None,
            richtung: sprites.projection().richtung(),
            ao_seiten: ao_seiten(sprites.projection().richtung()),
        }
    }

    /// Ein Cache für die nativen Stufen, die bandweise laufen: Er behält
    /// jeden Chunk und sein Licht im Vorrat, auch wenn er mit
    /// [`ChunkCache::wechsle`] zur nächsten Stufe geht.
    /// Siehe docs/entscheidungen/0043-native-stufen-in-baendern.md.
    pub fn mit_vorrat(world: &'a World, sprites: &'a SpriteSet, tiles: usize) -> ChunkCache<'a> {
        ChunkCache {
            vorrat: Some(Vorrat::default()),
            ..ChunkCache::with_row(world, sprites, tiles)
        }
    }

    /// Wechselt zur Sprite-Tabelle eines anderen scale, für Kacheln in
    /// Zeilen von `tiles`. Was an der Tabelle hängt, geht mit den Slots:
    /// Familien, Masken, Kandidaten und Varianten. Chunks und Licht bleiben
    /// im Vorrat. Mit derselben Tabelle bleibt alles.
    pub fn wechsle(&mut self, sprites: &'a SpriteSet, tiles: usize) {
        self.keep = tiles as u32;
        if std::ptr::eq(self.sprites, sprites) {
            return;
        }
        // Richtung, Seiten der weichen Beleuchtung und die Schlüssel im
        // Vorrat gelten im Blick: Alle Stufen eines Laufs schauen aus
        // derselben Richtung.
        debug_assert_eq!(sprites.projection().richtung(), self.richtung);
        self.sprites = sprites;
        self.himmel = sprites.himmel();
        self.slots.clear();
        self.index.clear();
        self.last = usize::MAX;
        self.grenze = CACHE_CHUNKS;
        self.biome_layers.clear();
        self.biome_index.clear();
        self.biome_last = usize::MAX;
    }

    /// Beginnt ein neues Band. Was das Band davor nicht gebraucht hat, geht
    /// aus dem Vorrat; was es gebraucht hat, bleibt, denn einen Teil davon
    /// braucht das neue.
    pub fn neues_band(&mut self) {
        if let Some(vorrat) = &mut self.vorrat {
            vorrat.band += 1;
            let band = vorrat.band;
            vorrat.chunks.retain(|_, gemerkt| gemerkt.band + 1 >= band);
        }
    }

    /// Beginnt eine neue Kachel. Ist der Cache voll, geht alles, was keine
    /// der letzten `keep` Kacheln gebraucht hat, und jede Regionsdatei, die
    /// kein Slot mehr braucht: ein Thread wandert über die ganze Welt, und
    /// unter Linux sind 1024 offene Dateien je Prozess üblich.
    fn next_tile(&mut self) {
        self.tile += 1;
        self.last = usize::MAX;
        if self.slots.len() <= self.grenze {
            return;
        }
        let (tile, keep) = (self.tile, self.keep);
        self.slots.retain(|slot| slot.used + keep >= tile);
        self.index = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| (slot.key, i))
            .collect();
        let regionen: HashSet<(i32, i32)> = self
            .slots
            .iter()
            .map(|slot| region_of(self.in_die_welt(slot.key)))
            .collect();
        self.regions.retain(|key, _| regionen.contains(key));
        // Biome nach dem Zoom nur für Chunks, die bleiben.
        let richtung = self.richtung;
        self.biome_layers.retain(|((cx, _, cz), _)| {
            let [bx, bz] = richtung.in_den_blick([*cx, *cz]);
            self.index.contains_key(&(bx, bz))
        });
        self.biome_index = self
            .biome_layers
            .iter()
            .enumerate()
            .map(|(i, (key, _))| (*key, i))
            .collect();
        self.biome_last = usize::MAX;
        self.grenze = (self.slots.len() + self.slots.len() / 4).max(CACHE_CHUNKS);
    }

    /// Wo ein Chunk im Blick in der Welt liegt.
    fn in_die_welt(&self, (cx, cz): (i32, i32)) -> (i32, i32) {
        let [x, z] = self.richtung.in_die_welt([cx, cz]);
        (x, z)
    }

    /// Slot des Chunks im Blick, geladen falls nötig.
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
        let gemerkt = self.vorrat.as_mut().and_then(|vorrat| {
            let band = vorrat.band;
            vorrat.chunks.get_mut(&key).map(|gemerkt| {
                gemerkt.band = band;
                (gemerkt.chunk.clone(), gemerkt.licht.clone())
            })
        });
        let (chunk, licht) = match gemerkt {
            Some(paar) => paar,
            None => {
                let welt = self.in_die_welt(key);
                let region_key = region_of(welt);
                if !self.regions.contains_key(&region_key) {
                    let region = self.world.region(region_key.0, region_key.1)?;
                    self.regions.insert(region_key, region);
                }
                let chunk = match self.regions.get_mut(&region_key) {
                    Some(Some(region)) => region.chunk(welt.0, welt.1)?.map(Rc::new),
                    _ => None,
                };
                if let Some(vorrat) = &mut self.vorrat {
                    let gemerkt = Gemerkt {
                        chunk: chunk.clone(),
                        licht: None,
                        frei: None,
                        band: vorrat.band,
                    };
                    vorrat.chunks.insert(key, gemerkt);
                }
                (chunk, None)
            }
        };
        let ecke = if self.world.ecke_wie_26_2() {
            SICHT_262
        } else {
            SICHT
        };
        let loaded = chunk.map(|chunk| Loaded {
            licht,
            ..Loaded::new(chunk, self.sprites, ecke)
        });
        self.slots.push(Slot {
            key,
            loaded,
            used: self.tile,
        });
        let i = self.slots.len() - 1;
        self.index.insert(key, i);
        Ok(i)
    }

    /// Der Slot des Chunks im Blick, mit seinem ausgebreiteten Licht, beim
    /// ersten Mal gerechnet; dafür lädt er die acht Nachbarn, so wie sie in
    /// der Welt um ihn liegen. `None`, wenn der Chunk fehlt oder nicht
    /// fertig ist.
    /// Siehe docs/renderer/wasser-und-licht.md, „Licht ausbreiten“.
    fn licht_slot(&mut self, key: (i32, i32)) -> Result<Option<usize>> {
        let i = self.slot(key)?;
        match &self.slots[i].loaded {
            None => return Ok(None),
            Some(loaded) if loaded.licht.is_some() => return Ok(Some(i)),
            Some(_) => {}
        }
        let [wx, wz] = self.richtung.in_die_welt([key.0, key.1]);
        let mut nachbarn = [0; 9];
        for (k, n) in nachbarn.iter_mut().enumerate() {
            let (dx, dz) = (k as i32 % 3 - 1, k as i32 / 3 - 1);
            let [bx, bz] = self.richtung.in_den_blick([wx + dx, wz + dz]);
            *n = self.slot((bx, bz))?;
        }
        let eingaben: Vec<Option<Vec<Eingabe>>> = nachbarn
            .iter()
            .map(|&n| self.slots[n].loaded.as_ref().map(Loaded::eingabe))
            .collect();
        let chunks = std::array::from_fn(|k| eingaben[k].as_deref());
        let licht = self.ausbreitung.chunk(&chunks, self.himmel);
        let licht = Rc::new(licht);
        if let Some(gemerkt) = self.vorrat.as_mut().and_then(|v| v.chunks.get_mut(&key)) {
            gemerkt.licht = Some(Rc::clone(&licht));
        }
        if let Some(loaded) = self.slots[i].loaded.as_mut() {
            loaded.licht = Some(licht);
        }
        self.last = i;
        Ok(Some(i))
    }

    /// Himmels- und Blocklicht der Zelle an `(x, y, z)` im Blick, so wie
    /// das Spiel es ausbreitet und speichert. In einem Chunk, der fehlt,
    /// keines.
    pub fn licht_at(&mut self, p: [i32; 3]) -> Result<(u8, u8)> {
        let Light { sky, block } = Light::from_packed(self.zelle(p)?.0);
        Ok((sky, block))
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
    /// Ein Block ist verdeckt, wenn die Nachbarn nach +x und +z ihren ganzen
    /// Umriss decken und der nach +y seinen Boden. Nach +y ist das ein Shift
    /// in derselben Spalte; am oberen Rand kommt das Bit aus der Section
    /// darüber, an den Rändern +x und +z aus dem Nachbarchunk. Genordet
    /// liegt der Nachbar nach +x neben dem Umriss und zählt nicht; welche
    /// zählen, sagt `Projection::verdeckende_seiten`.
    ///
    /// Reine Flüssigkeit, Wasser wie Lava, zeichnet ausserdem nichts, wo über
    /// ihr dieselbe steht und sie zu beiden Seiten an dieselbe mit derselben
    /// darüber grenzt oder an einen deckenden Nachbarn. Oben genügt ein
    /// deckender Block nicht: ohne dieselbe darüber ragt die Oberfläche in die
    /// Seiten hinein. Lava deckt nur bei scale 4, sonst fiele dort kein Block
    /// weg.
    ///
    /// Von oben stehen die Seiten auf der Kante; dort verdeckt der Block
    /// darüber allein, mit seinem Boden.
    ///
    /// Beides gilt nur, wenn jeder Block auf ganzen Pixeln liegt
    /// (`Projection::ganze_pixel`); bei anderen scales, die nur die
    /// Bibliothek annimmt, verdeckt kein Nachbar.
    /// Siehe docs/renderer/sprites-und-deckung.md, „Verdeckte Würfel“.
    /// Siehe docs/renderer/renderpfad.md, „Bitmasken“.
    fn expose(&mut self, slot: usize, s: usize) -> Result<()> {
        let (key, section_y) = {
            let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
            if loaded.exposed[s].is_some() {
                return Ok(());
            }
            (self.slots[slot].key, loaded.chunk.sections()[s].y)
        };
        let projection = self.sprites.projection();
        let verdecken = projection.ganze_pixel();
        // Ein Nachbar, der neben dem Umriss liegt, zählt, als wäre er deckend:
        // von oben beide, genordet der nach +x. Seinen Rand liest es nicht.
        let (mit_x, mit_z) = projection.verdeckende_seiten();
        let seite_x = if mit_x { 0 } else { u16::MAX };
        let seite_z = if mit_z { 0 } else { u16::MAX };
        let luft = [(0, [0; 2], [0; 2]); 16];
        let nx = if verdecken && mit_x {
            self.edge((key.0 + 1, key.1), section_y, true)?
        } else {
            luft
        };
        let nz = if verdecken && mit_z {
            self.edge((key.0, key.1 + 1), section_y, false)?
        } else {
            luft
        };

        let loaded = self.slots[slot].loaded.as_mut().expect("geladen");
        let above = section_y
            .checked_add(1)
            .and_then(|y| loaded.chunk.section_index(y))
            .and_then(|i| loaded.masks[i].as_deref());
        let m = loaded.masks[s].as_deref().expect("nur Sections mit Masken");
        let mut ex = Box::new(Exposed {
            own: [0; 256],
            any_own: false,
        });
        for col in 0..256 {
            let (x, z) = (col & 15, col >> 4);
            let (sx, fx, ux) = if x < 15 { rand(m, col + 1) } else { nx[z] };
            let (sz, fz, uz) = if z < 15 { rand(m, col + 16) } else { nz[x] };
            let (sx, sz) = (sx | seite_x, sz | seite_z);
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
            let verdeckt = if verdecken { hidden | fluid_hidden } else { 0 };
            ex.own[col] = m.bits[PRESENT][col] & (m.bits[LOOSE][col] | !verdeckt);
        }
        ex.any_own = ex.own.iter().any(|&o| o != 0);
        loaded.exposed[s] = Some(ex);
        Ok(())
    }

    /// Beginnt eine Kachel: ihre Kandidaten in Zeichenreihenfolge, dazu die
    /// fremden Würfel, auf die `Candidate::kind` zeigt.
    fn sorted_candidates(
        &mut self,
        rect: ScreenRect,
        y_range: (i32, i32),
    ) -> Result<(Vec<Candidate>, Vec<Cell>)> {
        self.next_tile();
        let foreign: Vec<Cell> = self.sprites.foreign_cells().iter().copied().collect();
        let mut candidates = self.candidates(rect, y_range, &foreign)?;
        candidates.sort_unstable_by_key(|c| c.key);
        Ok((candidates, foreign))
    }

    /// Ob im Band von `rect` ein Block leuchtet ([`Masks::quellen`]), ob zu
    /// sehen oder nicht: grosszügig wie das Band von
    /// [`ChunkCache::candidates`], samt der Reserve für fremde Teile.
    /// Siehe docs/renderer/cinematic.md, „Bloom“.
    fn leuchtet_im_band(&mut self, rect: ScreenRect, y_range: (i32, i32)) -> Result<bool> {
        let projection = self.sprites.projection();
        let (u_min, u_max) = u_window(projection, rect);
        let (v_lo, v_hi) = (
            v_window(projection, rect, y_range.0).0,
            v_window(projection, rect, y_range.1).1,
        );
        let pad = self
            .sprites
            .foreign_cells()
            .iter()
            .map(|c| c[0].abs() + c[1].abs() + c[2].abs())
            .max()
            .unwrap_or(0);
        let genordet = projection.kamera().genordet();
        for key in band_chunks(genordet, u_min - pad, u_max + pad, v_lo - pad, v_hi + pad) {
            let slot = self.slot(key)?;
            let Some(loaded) = &self.slots[slot].loaded else {
                continue;
            };
            let (cx, cz) = (loaded.chunk.x * 16, loaded.chunk.z * 16);
            for (section, masks) in loaded.chunk.sections().iter().zip(&loaded.masks) {
                for &(i, _) in masks.iter().flat_map(|m| &m.quellen) {
                    let y = i32::from(section.y) * 16 + i32::from(i >> 8);
                    let welt = [cx + i32::from(i & 15), cz + i32::from(i >> 4 & 15)];
                    let [x, z] = self.richtung.in_den_blick(welt);
                    let (u, v) = projection.uv(x, z);
                    let (lo, hi) = v_window(projection, rect, y);
                    if (y_range.0 - pad..=y_range.1 + pad).contains(&y)
                        && (u_min - pad..=u_max + pad).contains(&u)
                        && (lo - pad..=hi + pad).contains(&v)
                    {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
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
        // Das Fenster von `v` je Höhe; es verschiebt sich um b/a je Höhe, bei
        // 2:1 um genau 2, von oben gar nicht.
        let fenster: Vec<(i32, i32)> = (y_range.0..=y_range.1)
            .map(|y| v_window(projection, rect, y))
            .collect();
        let (v_lo, v_hi) = (
            v_window(projection, rect, y_range.0).0,
            v_window(projection, rect, y_range.1).1,
        );
        debug_assert!(y_range.1 - y_range.0 < 1 << 10);
        debug_assert!(v_hi - v_lo < 1 << 22 && u_max - u_min < 1 << 22);
        debug_assert!(foreign.len() < 1 << 9);
        // Jeder Kandidat liegt im Band, also nie vor dessen Rand. Im selben
        // Würfel ordnet der Rang: fremde Teile vor dem Block, der Block,
        // fremde Teile nach ihm.
        let key_of = |y: i32, v: i32, u: i32, rang: u16| -> u64 {
            ((y - y_range.0) as u64) << 54
                | ((v - v_lo) as u64) << 32
                | ((u - u_min) as u64) << 10
                | rang as u64
        };
        let eigen = foreign.len() as u16;
        let in_y = |y: i32| (y_range.0..=y_range.1).contains(&y);
        let in_band = |y: i32, v: i32, u: i32| {
            in_y(y) && (u_min..=u_max).contains(&u) && {
                let (lo, hi) = fenster[(y - y_range.0) as usize];
                (lo..=hi).contains(&v)
            }
        };
        // Das Band hat Reserve für Modelle, die aus ihrem Würfel ragen. Alle
        // anderen bleiben in dessen Umriss (`contained`) und zählen nur, wenn
        // der die Kachel berührt.
        // Siehe docs/renderer/renderpfad.md, „Kandidaten“.
        let (x_min, x_max, y_min, y_max) = self.sprites.outline_box();
        let (width, height) = (rect.width as i32, rect.height as i32);
        let touches = |x: i32, y: i32, z: i32| {
            let (bx, by) = block_origin(projection, rect, [x, y, z]);
            in_y(y)
                && bx + x_max >= 0
                && bx + x_min < width
                && by + y_max >= 0
                && by + y_min < height
        };
        // Ein fremdes Teil kann von einem Block ausserhalb des Bands
        // hereinragen; so weit reicht die Suche über das Band hinaus.
        let pad = foreign
            .iter()
            .map(|c| c[0].abs() + c[2].abs())
            .max()
            .unwrap_or(0);

        let pad_y = foreign.iter().map(|c| c[1].abs()).max().unwrap_or(0);
        let bleed = BLEED_BLOCKS as f64 * projection.scale() as f64;
        let (a, b) = (projection.a(), projection.b());
        // Höhen, die das Band in einem Chunk erreichen kann: die Umkehrung
        // von `v_window` für die kleinste und grösste Tiefe `v` des Chunks,
        // grosszügig gerundet. Entscheidend bleibt die Prüfung je Block
        // (`touches`, `in_band`); das hier spart nur die Schleife über
        // Sections, die das Band in diesem Chunk gar nicht berührt.
        // Siehe docs/renderer/renderpfad.md, „Kandidaten“.
        // Von oben erreicht das Band jede Höhe.
        let y_span = |va: i32, vb: i32| {
            if b == 0.0 {
                return (i32::MIN / 4, i32::MAX / 4);
            }
            let lo = ((va - 1) as f64 * a - rect.bottom() as f64 - bleed) / b;
            let hi = ((vb + 1) as f64 * a - rect.y as f64 + bleed) / b;
            (lo.floor() as i32 - 1 - pad_y, hi.ceil() as i32 + 1 + pad_y)
        };

        let mut out = Vec::with_capacity(8192);
        let mut anchors: Vec<[i32; 3]> = Vec::new();
        let genordet = projection.kamera().genordet();
        for key in band_chunks(genordet, u_min - pad, u_max + pad, v_lo - pad, v_hi + pad) {
            let slot = self.slot(key)?;
            let sections = match &self.slots[slot].loaded {
                Some(loaded) => loaded.chunk.sections().len(),
                None => continue,
            };
            // Die kleinste und grösste Tiefe `v` im Chunk.
            let (y_lo, y_hi) = if genordet {
                y_span(key.1 * 16, key.1 * 16 + 15)
            } else {
                let v0 = key.0 * 16 + key.1 * 16;
                y_span(v0, v0 + 30)
            };
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
                    let (u, v) = projection.uv(x, z);
                    let mut bits = own;
                    while bits != 0 {
                        let b = bits.trailing_zeros();
                        let y = sy + b as i32;
                        bits &= bits - 1;
                        let loose = m.bits[LOOSE][col] >> b & 1 != 0;
                        let drin = if loose {
                            in_band(y, v, u)
                        } else {
                            touches(x, y, z)
                        };
                        if !drin {
                            continue;
                        }
                        out.push(Candidate {
                            key: key_of(y, v, u, eigen),
                            x,
                            y,
                            z,
                            kind: 0,
                            loose,
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
        // Band liegt, auch wenn er verdeckt ist; was dann verdeckt ist,
        // lässt die Deckungsmaske fallen. Ein Teil liegt in seinem Würfel,
        // also hinter jeder Fläche auf dessen Vorderseiten: Hat der Block
        // dort nur solche (`wuerfelform`), kommt es vor ihm, sonst nach ihm.
        // Siehe docs/renderer/kamera.md, „Ein Teil im Würfel eines anderen Blocks“.
        for anchor in anchors {
            for (i, cell) in foreign.iter().enumerate() {
                let [x, y, z] = [
                    anchor[0] + cell[0],
                    anchor[1] + cell[1],
                    anchor[2] + cell[2],
                ];
                let (u, v) = projection.uv(x, z);
                if in_band(y, v, u) {
                    let kind = i as u16 + 1;
                    let vor = self.family_at(x, y, z)?.is_some_and(|f| f.wuerfelform);
                    out.push(Candidate {
                        key: key_of(y, v, u, if vor { kind - 1 } else { eigen + kind }),
                        x,
                        y,
                        z,
                        kind,
                        loose: false,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Was an einer Stelle im Blick zu zeichnen ist — nichts für Luft,
    /// fehlende Chunks und Blöcke ohne sichtbare Geometrie.
    ///
    /// Vier Entscheidungen fallen hier: welche Alternative die Position
    /// bekommt, welche Flüssigkeitsflächen die Nachbarn verdecken, welche
    /// Flächen zu Nachbarn entfallen und in welchen Farben sein
    /// Biom den Block tönt. Die Bilder sind vorab gerastert, die Farben
    /// kommen beim Zeichnen dazu.
    fn sprite_at(&mut self, x: i32, y: i32, z: i32) -> Result<Drawn> {
        let sprites = self.sprites;
        let Some((family, leuchten)) = self.block_at(x, y, z)? else {
            return Ok(Drawn::default());
        };
        // Gewürfelt wird in der Welt.
        let [wx, wz] = self.richtung.in_die_welt([x, z]);
        let Some(wahl) = family.wahl([wx, y, wz]) else {
            return Ok(Drawn::default());
        };
        let Some(id) = family.sprite(wahl) else {
            return Ok(Drawn::default());
        };
        let mut strips = [None; 2];
        let mut fluessig = 0;

        if let Some((fluid, amount)) = family.fluid {
            let same = |other: Option<&Family>| {
                other.is_some_and(|other| other.fluid.is_some_and(|(kind, _)| kind == fluid))
            };
            // Steht dieselbe Flüssigkeit darüber, reicht die eigene bis zur
            // Kante, und die Oberseite entfällt.
            let above = same(self.family_at(x, y + 1, z)?);
            let own = if above { fluid::FULL } else { amount };
            if above {
                fluessig |= mask_bit(Face::Up);
            }

            // Zur selben Flüssigkeit nebenan nie eine Seitenfläche, wie
            // `shouldRenderFace` im Spiel; steht der Nachbar tiefer, bleibt
            // über ihm ein Streifen der eigenen Seite.
            // Siehe docs/renderer/wasser-und-licht.md, „Flächen zu gleichem Wasser“.
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
                fluessig |= mask_bit(face);
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
        }

        // Flächen zu Nachbarn entfallen wie in `Block.shouldRenderFace`: vor
        // einer Seite, die voll deckt, und nach `skipRendering`. Bleibt
        // nichts, fällt der Block weg.
        // Siehe docs/renderer/sprites-und-deckung.md, „Flächen vor einem vollen Nachbarn“.
        let sprite = if family.hat_nachbarn() {
            let mut nachbarn = 0;
            for (k, face) in family.nachbarseiten().enumerate() {
                let [dx, dy, dz] = self.richtung.versatz_in_den_blick(face.versatz());
                let Some(nachbar) = self.family_at(x + dx, y + dy, z + dz)? else {
                    continue;
                };
                let voll = nachbar.voll & seite(face.gegenueber()) != 0;
                let regel = family
                    .nachbarn
                    .zip(nachbar.nachbarn)
                    .is_some_and(|(regel, nachbar)| regel.verdeckt(&nachbar, face));
                if voll || regel {
                    nachbarn |= 1 << k;
                }
            }
            family.ohne_nachbarn(wahl, fluessig, nachbarn)
        } else if family.fluid.is_some() {
            sprites.masked(id, fluessig)
        } else {
            Some(id)
        };
        if sprite.is_none() && strips.iter().all(Option::is_none) {
            return Ok(Drawn::default());
        }

        let (licht, ecken, wasser) = self.licht_fuer([x, y, z], family, leuchten, sprite)?;
        // Gemischt wird nur für Sprites mit Tönungskarte, und nur die
        // Farben, die sie trägt.
        let kinds = sprite
            .into_iter()
            .chain(strips.into_iter().flatten())
            .fold(0, |kinds, id| kinds | sprites.tints(id));
        let tint = self.tints_at([x, y, z], family, kinds)?;
        Ok(Drawn {
            sprite,
            strips,
            licht,
            ecken,
            wasser,
            tint,
            leuchten: f32::from(leuchten.stufe()) / 15.0,
        })
    }

    /// Die Farben des Himmels für das Wasser und die Temperatur am Block
    /// `(x, y, z)` im Blick für Cinematic, die Farben linear mit der Stärke
    /// 1: je Biom aus [`Kino::himmel`], gemischt über dasselbe Quadrat um
    /// den Block wie die Farben des Bioms
    /// ([`BiomeTable::blend`](super::BiomeTable::blend)), aber in linearem
    /// Licht und ungerundet.
    /// Siehe docs/renderer/cinematic.md, „Farbe des Himmels“.
    fn himmel_at(&mut self, kino: &Kino, [x, y, z]: [i32; 3]) -> Result<Himmelsfarben> {
        let [x, z] = self.richtung.in_die_welt([x, z]);
        let (mut summe, mut n) = (Himmelsfarben::default(), 0.0);
        for block in self.sprites.biomes().quadrat([x, y, z]) {
            summe = summe.je_farbe(kino.himmel(self.biome_of(block)?), |a, b| a + b);
            n += 1.0;
        }
        Ok(summe.je_farbe(summe, |a, _| a / n))
    }

    /// Die Farben eines Blocks für seine Tönungskarte, gepackt wie sie: die
    /// seines Blocks aus dem Resolver der Familie, wenn `kinds` [`TINT_BLOCK`]
    /// trägt, bei `tint_below` am Block darunter; die des Wassers mit
    /// [`TINT_WATER`], am Block selbst. Beide gemischt wie im Client
    /// ([`BiomeTable::blend`](super::BiomeTable::blend)). 0, wo keine Karte
    /// sie braucht. `(x, y, z)` liegt im Blick, gemischt wird in der Welt.
    fn tints_at(&mut self, [x, y, z]: [i32; 3], family: &Family, kinds: u8) -> Result<[u32; 2]> {
        let table = self.sprites.biomes();
        let [x, z] = self.richtung.in_die_welt([x, z]);
        let mut farbe = |resolver, block| {
            Ok::<_, anyhow::Error>(pack(table.blend(resolver, block, |p| self.biome_of(p))?))
        };
        Ok([
            match family.resolver {
                Some(resolver) if kinds & TINT_BLOCK != 0 => {
                    farbe(resolver, [x, y - family.tint_below as i32, z])?
                }
                _ => 0,
            },
            if kinds & TINT_WATER != 0 {
                farbe(Resolver::Water, [x, y, z])?
            } else {
                0
            },
        ])
    }

    /// Das Biom eines Blocks der Welt als Nummer der [`BiomeTable`](super::BiomeTable):
    /// das der Viertelposition aus
    /// [`BiomeTable::quart`](super::BiomeTable::quart), einmal je Block
    /// gerechnet und dann behalten, denn die Mischung fragt jeden Block bis
    /// zu (2 · Radius + 1)² Mal.
    fn biome_of(&mut self, [x, y, z]: [i32; 3]) -> Result<u16> {
        let key = (x >> 4, y, z >> 4);
        let l = match self.biome_layers.get(self.biome_last) {
            Some((k, _)) if *k == key => self.biome_last,
            _ => match self.biome_index.get(&key) {
                Some(&l) => l,
                None => {
                    self.biome_layers.push((key, Box::new([u16::MAX; 256])));
                    let l = self.biome_layers.len() - 1;
                    self.biome_index.insert(key, l);
                    l
                }
            },
        };
        self.biome_last = l;
        let i = ((z & 15) * 16 + (x & 15)) as usize;
        let biome = self.biome_layers[l].1[i];
        if biome != u16::MAX {
            return Ok(biome);
        }
        let biome = self.noise_biome(self.sprites.biomes().quart([x, y, z]))?;
        self.biome_layers[l].1[i] = biome;
        Ok(biome)
    }

    /// Das gespeicherte Biom einer Viertelposition, wie
    /// `ChunkAccess.getNoiseBiome`: die Höhe auf die des Chunks geklemmt. Ein
    /// fehlender Chunk ist plains wie im Client
    /// (`ClientLevel.getUncachedNoiseBiome`); eine Section ohne Biome macht
    /// der Renderer ebenso zu plains, als Ersatz.
    /// Siehe docs/renderer/biomfarben.md, „Biom je Block“.
    fn noise_biome(&mut self, [qx, qy, qz]: [i32; 3]) -> Result<u16> {
        let plains = self.sprites.biomes().plains();
        let [cx, cz] = self.richtung.in_den_blick([qx >> 2, qz >> 2]);
        let slot = self.slot((cx, cz))?;
        let Some(loaded) = &self.slots[slot].loaded else {
            return Ok(plains);
        };
        let chunk = &loaded.chunk;
        let qy = qy.clamp(chunk.y_min() >> 2, chunk.y_max() >> 2);
        let Some(s) = i8::try_from(qy >> 2)
            .ok()
            .and_then(|sy| chunk.section_index(sy))
        else {
            return Ok(plains);
        };
        let index = chunk.sections()[s]
            .biomes()
            .index(((qy & 3) * 16 + (qz & 3) * 4 + (qx & 3)) as usize);
        Ok(loaded.biomes[s].get(index).copied().unwrap_or(plains))
    }

    /// In welchem Licht das Spiel den Block an `p` zeichnet: das Licht für
    /// Pixel ohne Seite je Kanal
    /// ([`Lightmap::factors`](super::rasterizer::Lightmap::factors)),
    /// mit einer AO-Karte des Sprites das an den Ecken seiner Seiten, siehe
    /// [`ChunkCache::ecken_at`], und das seines Wassers, wo es ein anderes
    /// ist. Voll hell (`emissiveRendering`) ist alles 15. Ein Pixel ohne
    /// Seite liegt im Licht seiner Zelle, das eigene Blocklicht steckt
    /// darin. Eine Flüssigkeit liegt im helleren Licht ihrer Zelle und der
    /// darüber (`FluidRenderer.getLightCoords`), auch unter gleicher
    /// Flüssigkeit; ein Modell mit eigener Flüssigkeit liegt im Licht seiner
    /// Zelle, seine Flüssigkeit ebenso im helleren. Für Cinematic stehen
    /// statt der Helligkeit die Kanäle aus [`kino_kanaele`].
    /// Siehe docs/renderer/wasser-und-licht.md, „Welches Licht ein Block bekommt“.
    fn licht_fuer(
        &mut self,
        [x, y, z]: [i32; 3],
        family: &Family,
        leuchten: Leuchten,
        sprite: Option<SpriteId>,
    ) -> Result<Lichter> {
        let lightmap = self.sprites.lightmap();
        let kino = self.sprites.kino().is_some();
        // Das Licht einer ganzen Stufe, gepackt wie `Light::packed`.
        let stufe = |licht: u32| match kino {
            true => kino_kanaele(licht, 255),
            false => lightmap.factors(Light::from_packed(licht)),
        };
        if let Leuchten::Voll(_) = leuchten {
            return Ok((stufe(VOLL_HELL), None, None));
        }
        let (licht, ecken) = match sprite.filter(|&id| self.sprites.has_ao(id)) {
            Some(id) => {
                // Was leuchtet, zeichnet das Spiel ohne weiche Beleuchtung
                // (`ModelBlockRenderer.tesselateBlock`).
                let weich = self.sprites.weich(id) && leuchten.stufe() == 0;
                let innen = self.sprites.innen(id).then_some(family.doppelkiste);
                let plaetze = self.sprites.plaetze(id);
                self.ecken_at([x, y, z], weich, leuchten.stufe(), innen, plaetze)?
            }
            None => {
                let mut eigen = self.lichtwert([x, y, z])?;
                if let Some([dx, dy, dz]) = family.doppelkiste {
                    eigen = hellstes(eigen, self.lichtwert([x + dx, y + dy, z + dz])?);
                }
                (stufe(eigen), None)
            }
        };
        if family.fluid.is_none() {
            return Ok((licht, ecken, None));
        }
        let eigen = self.lichtwert([x, y, z])?;
        let oben = self.lichtwert([x, y + 1, z])?;
        let hell = stufe(hellstes(eigen, oben));
        if family.pure_fluid {
            return Ok((hell, None, None));
        }
        Ok((licht, ecken, (hell != licht).then_some(hell)))
    }

    /// Das Licht an den Ecken der Plätze eines Blocks je Kanal ([`Ecken`]),
    /// wie das Spiel es in 26.2 setzt, und das Licht für Pixel ohne Platz:
    /// mit `innen` das der eigenen Zelle, bei einer Doppelkiste das hellere
    /// ihrer und der Zelle der anderen Hälfte, relativ zum Block; ohne
    /// `innen` gibt es keine, und es ist das der ersten Seite.
    /// Gerechnet werden nur die Plätze aus `plaetze`.
    /// - **Auf dem Rand,** Platz 0 bis 2, mit `weich`: je Ecke das Licht der
    ///   Zelle vor der Seite, ihrer zwei Nachbarn in dieser Schicht und des
    ///   Blocks in der Ecke, gemischt nach [`smooth_blend`], dazu die weiche
    ///   Beleuchtung aus denselben Blöcken
    ///   (`BlockModelLighter.prepareQuadAmbientOcclusion`); die Lightmap liest
    ///   das Spiel dort linear gefiltert, `Lightmap::linear`. Der Block in
    ///   der Ecke zählt nur, wenn hinter einem der beiden Nachbarn nichts
    ///   die Sicht nimmt, sonst gilt der erste Nachbar aus
    ///   `AdjacencyInfo.corners`. Sonst, wie `prepareQuadFlat` für eine Seite
    ///   mit `cullface`, das Licht der Zelle vor der Seite mit dem eigenen
    ///   Blocklicht `stufe`. Eine Seite, die ihr Nachbar deckt, ist nicht zu
    ///   sehen und nimmt die Werte einer anderen.
    /// - **Im Innern,** Platz 3 bis 5: dasselbe ab der
    ///   eigenen Zelle, der Schatten der Mitte vom Block selbst, ihr Licht
    ///   aus der Zelle vor der Seite, ausser deren Block ist
    ///   `isSolidRender`; ohne `weich` das Licht der eigenen Zelle.
    ///
    /// Haben alle Ecken dasselbe Licht wie die Pixel ohne Platz, gilt es für
    /// das ganze Sprite, ohne Ecken. Für Cinematic stehen statt der
    /// Helligkeit die Kanäle aus [`kino_kanaele`].
    /// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
    fn ecken_at(
        &mut self,
        block: [i32; 3],
        weich: bool,
        stufe: u8,
        innen: Option<Option<[i32; 3]>>,
        plaetze: u8,
    ) -> Result<([u32; 3], Option<Ecken>)> {
        let lightmap = self.sprites.lightmap();
        let kino = self.sprites.kino().is_some();
        // Das Licht einer ganzen Stufe, gepackt wie `Light::packed`.
        let ganz = |licht: u32| match kino {
            true => kino_kanaele(licht, 255),
            false => lightmap.factors(Light::from_packed(licht)),
        };
        let [fest, dunkelt, sicht, opak] = self.umgebung(block)?;
        let (roh, voll) = self.lichter_um(block)?;
        // Alles relativ zum Block, siehe `umgebung` und `lichter_um`.
        let bit = |ebene: u64, [dx, dy, dz]: [i32; 3]| {
            ebene >> (dy + 1 + 4 * (dx + 1) + 16 * (dz + 1)) & 1 != 0
        };
        let stelle = |[dx, dy, dz]: [i32; 3]| (dx + 1 + 3 * (dy + 1) + 9 * (dz + 1)) as usize;
        // Das Licht einer Zelle, wie ihr eigener Block es angibt
        // (`LightCoordsUtil.getLightCoords`).
        let licht = |p: [i32; 3]| match voll >> stelle(p) & 1 {
            0 => roh[stelle(p)],
            _ => VOLL_HELL,
        };
        let bei = |p: [i32; 3], o: [i32; 3]| [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
        // Die vier Werte einer Seite ab der Zelle `start`: auf dem Rand der
        // vor ihr, im Innern der eigenen. Den Schatten der Mitte gibt der
        // Block in `start`, ihr Licht kommt aus `mitte`.
        let weich_ab = |s: &AoSeite, start: [i32; 3], mitte: u32| {
            let (d, nachbarn) = (s.richtung, s.nachbarn);
            let mut dunkel = [false; 4];
            let mut frei = [false; 4];
            let mut nah = [0; 4];
            for (k, &n) in nachbarn.iter().enumerate() {
                dunkel[k] = bit(dunkelt, bei(start, n));
                frei[k] = !bit(sicht, bei(bei(start, n), d));
                nah[k] = licht(bei(start, n));
            }
            let ecke = |a: usize, b: usize| {
                if frei[a] || frei[b] {
                    let q = bei(bei(start, nachbarn[a]), nachbarn[b]);
                    (bit(dunkelt, q), licht(q))
                } else {
                    (dunkel[0], nah[0])
                }
            };
            let (e03, e02, e12, e13) = (ecke(0, 3), ecke(0, 2), ecke(1, 2), ecke(1, 3));
            let davor = bit(dunkelt, start);
            let je_ecke = [
                (
                    [dunkel[3], dunkel[0], e03.0, davor],
                    [nah[3], nah[0], e03.1],
                ),
                (
                    [dunkel[2], dunkel[0], e02.0, davor],
                    [nah[2], nah[0], e02.1],
                ),
                (
                    [dunkel[2], dunkel[1], e12.0, davor],
                    [nah[2], nah[1], e12.1],
                ),
                (
                    [dunkel[3], dunkel[1], e13.0, davor],
                    [nah[3], nah[1], e13.1],
                ),
            ];
            let mut werte = [[0; 3]; 4];
            for ((schatten, [a0, a1, a2]), &ziel) in je_ecke.iter().zip(&s.remap) {
                let ao = AO_WERTE[schatten.iter().filter(|&&d| d).count()];
                let l = smooth_blend(*a0, *a1, *a2, mitte);
                werte[ziel] = match kino {
                    true => kino_kanaele(l, ao),
                    false => lightmap.linear(l).map(|l| (l * ao + 127) / 255),
                };
            }
            werte
        };
        let eigen_licht = ganz(licht([0; 3]));
        let mut seiten: [Option<[[u32; 3]; 4]>; AO_PLAETZE] = [None; AO_PLAETZE];
        for (seite, s) in self.ao_seiten.iter().enumerate() {
            let d = s.richtung;
            if plaetze & 1 << seite != 0 && !bit(fest, d) {
                seiten[seite] = Some(if weich {
                    weich_ab(s, d, licht(d))
                } else {
                    let Light { sky, block } = Light::from_packed(roh[stelle(d)]);
                    let eigen = Light {
                        sky,
                        block: block.max(stufe),
                    };
                    [ganz(eigen.packed()); 4]
                });
            }
            // Im Innern: ab der eigenen Zelle; das Licht der Mitte aus der
            // Zelle davor, ausser ihr Block ist `isSolidRender`, [`OPAQUE`].
            // Flach im Licht der eigenen Zelle (`prepareQuadFlat`).
            if plaetze & 1 << (seite + 3) != 0 {
                seiten[seite + 3] = Some(if weich {
                    let mitte = if bit(opak, d) {
                        licht([0; 3])
                    } else {
                        licht(d)
                    };
                    weich_ab(s, [0; 3], mitte)
                } else {
                    [eigen_licht; 4]
                });
            }
        }
        let eigen = innen.map(|kiste| match kiste {
            None => eigen_licht,
            Some(q) => ganz(hellstes(licht([0; 3]), licht(q))),
        });
        let Some(erste) = seiten.iter().flatten().flatten().next().copied() else {
            return Ok((eigen.unwrap_or([255; 3]), None));
        };
        let rest = eigen.unwrap_or(erste);
        if seiten.iter().flatten().flatten().all(|&w| w == rest) {
            return Ok((rest, None));
        }
        let mut ecken = [[0; AO_PLAETZE]; 3];
        for (seite, werte) in seiten.iter().enumerate() {
            let werte = werte.unwrap_or([erste; 4]);
            for (c, kanal) in ecken.iter_mut().enumerate() {
                kanal[seite] =
                    werte[0][c] | werte[1][c] << 8 | werte[2][c] << 16 | werte[3][c] << 24;
            }
        }
        Ok((rest, Some(ecken)))
    }

    /// Das Licht der 27 Zellen um einen Block, `(dx, dy, dz)` je von -1 bis
    /// 1, an Stelle `dx + 1 + 3 · (dy + 1) + 9 · (dz + 1)`, gepackt wie
    /// [`Light::packed`], dazu je Stelle ein Bit für einen Block, den das
    /// Spiel voll hell zeichnet.
    fn lichter_um(&mut self, [x, y, z]: [i32; 3]) -> Result<([u32; 27], u32)> {
        let mut roh = [0; 27];
        let mut voll = 0;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let k = (dx + 1 + 3 * (dy + 1) + 9 * (dz + 1)) as usize;
                    let (licht, hell) = self.zelle([x + dx, y + dy, z + dz])?;
                    roh[k] = licht;
                    voll |= u32::from(hell) << k;
                }
            }
        }
        Ok((roh, voll))
    }

    /// Das Licht der Zelle an `(x, y, z)` im Blick, gepackt wie
    /// [`Light::packed`], und ob das Spiel den Block dort voll hell zeichnet
    /// ([`VOLL`]). In einem Chunk, der fehlt, keines.
    fn zelle(&mut self, [x, y, z]: [i32; 3]) -> Result<(u32, bool)> {
        let Some(i) = self.licht_slot((x >> 4, z >> 4))? else {
            return Ok((0, false));
        };
        let loaded = self.slots[i].loaded.as_ref().expect("eben geladen");
        // Das Licht liegt in der Welt, die Masken im Blick.
        let [wx, wz] = self.richtung.in_die_welt([x, z]);
        let (lx, lz) = ((wx & 15) as usize, (wz & 15) as usize);
        let wert = loaded.licht.as_deref().map_or(0, |l| l.at(lx, y, lz));
        let col = ((z & 15) * 16 + (x & 15)) as usize;
        let voll = i8::try_from(y >> 4)
            .ok()
            .and_then(|sy| loaded.chunk.section_index(sy))
            .and_then(|s| loaded.masks[s].as_deref())
            .is_some_and(|m| m.bits[VOLL][col] >> (y & 15) & 1 != 0);
        let licht = Light {
            sky: wert >> 4,
            block: wert & 15,
        };
        Ok((licht.packed(), voll))
    }

    /// Das Licht einer Zelle, wie ihr eigener Block es angibt
    /// (`LightCoordsUtil.getLightCoords`): voll hell mit
    /// `emissiveRendering`, sonst ihres. Das eigene Blocklicht steckt darin,
    /// denn als Quelle beginnt die Zelle mit ihm.
    fn lichtwert(&mut self, p: [i32; 3]) -> Result<u32> {
        let (licht, voll) = self.zelle(p)?;
        Ok(if voll { VOLL_HELL } else { licht })
    }

    /// Die Ebenen [`SOLID`], [`DARK`], [`VIEW`] und [`OPAQUE`] um einen Block, so weit
    /// [`ChunkCache::ecken_at`] fragt: je Ebene ein Bit für jede Zelle
    /// `(x + dx, y + dy, z + dz)` mit `dx`, `dy` und `dz` von -1 bis 2, an
    /// Stelle `dy + 1 + 4 · (dx + 1) + 16 · (dz + 1)`. Die vier Zellen einer
    /// Spalte kommen aus einem Wort je Ebene, an einer Sectionsgrenze aus
    /// zweien. Ausserhalb der Welt und in fehlenden Chunks steht nichts, wie
    /// für Luft.
    fn umgebung(&mut self, [x, y, z]: [i32; 3]) -> Result<[u64; 4]> {
        let mut out = [0; 4];
        for dz in -1..=2 {
            for dx in -1..=2 {
                // Diese Spalte fragt keine der drei Seiten.
                if (dx, dz) == (2, 2) {
                    continue;
                }
                let (px, pz) = (x + dx, z + dz);
                let i = self.slot((px >> 4, pz >> 4))?;
                let Some(loaded) = self.slots[i].loaded.as_ref() else {
                    continue;
                };
                let col = ((pz & 15) * 16 + (px & 15)) as usize;
                let woerter = |sy: i32| {
                    i8::try_from(sy)
                        .ok()
                        .and_then(|sy| loaded.chunk.section_index(sy))
                        .and_then(|s| loaded.masks[s].as_deref())
                        .map_or([0; 4], |m| {
                            [SOLID, DARK, VIEW, OPAQUE].map(|e| u32::from(m.bits[e][col]))
                        })
                };
                let unten = y - 1;
                let sy = unten >> 4;
                let lo = woerter(sy);
                let hi = if (y + 2) >> 4 != sy {
                    woerter(sy + 1)
                } else {
                    [0; 4]
                };
                let stelle = 4 * (dx + 1) + 16 * (dz + 1);
                for ((o, l), h) in out.iter_mut().zip(lo).zip(hi) {
                    *o |= u64::from((l | h << 16) >> (unten & 15) & 15) << stelle;
                }
            }
        }
        Ok(out)
    }

    /// Wie [`ChunkCache::family_at`], dazu wie hell der Block selbst
    /// leuchtet, aus demselben Nachschlag. Ändern die Daten seines
    /// Blockentity das Bild, die Familie mit diesen Daten; für seine
    /// Nachbarn zählt er wie ohne, sie verdecken dasselbe.
    fn block_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<(&'a Family, Leuchten)>> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(None);
        };
        let [x, z] = self.richtung.in_die_welt([x, z]);
        let Some((section, slot)) = loaded.chunk.slot(x, y, z) else {
            return Ok(None);
        };
        let Some(index) = loaded
            .families
            .get(section)
            .and_then(|families| families.get(slot))
            .copied()
            .flatten()
        else {
            return Ok(None);
        };
        let index = match loaded
            .varianten
            .binary_search_by_key(&[x, y, z], |&(pos, _)| pos)
        {
            Ok(i) => loaded.varianten[i].1,
            Err(_) => index,
        };
        let leuchten = loaded
            .leuchten
            .get(section)
            .and_then(|l| l.get(slot))
            .copied()
            .unwrap_or(Leuchten::Stufe(0));
        Ok(Some((self.sprites.family(index), leuchten)))
    }

    /// Die Familie des Blocks an einer Stelle im Blick — ein Nachschlag im
    /// Chunk-Cache und zwei Indizes, ohne die Blockstate zu hashen.
    fn family_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<&'a Family>> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(None);
        };
        let [x, z] = self.richtung.in_die_welt([x, z]);
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

/// Die Region eines Chunks.
fn region_of((cx, cz): (i32, i32)) -> (i32, i32) {
    (cx.div_euclid(REGION), cz.div_euclid(REGION))
}

/// Alle Chunks, die das Band `u ∈ [u_min, u_max]`, `v ∈ [v_lo, v_hi]`
/// berühren können — diagonal grob über die Hüllbox, dann je Chunk gegen
/// das Band; genordet ist das Band ein Rechteck in x und z. Ein paar Chunks
/// zu viel schaden nicht: jeder Kandidat wird ohnehin einzeln gegen das
/// Band geprüft.
fn band_chunks(genordet: bool, u_min: i32, u_max: i32, v_lo: i32, v_hi: i32) -> Vec<(i32, i32)> {
    if genordet {
        return ((v_lo >> 4)..=(v_hi >> 4))
            .flat_map(|cz| ((u_min >> 4)..=(u_max >> 4)).map(move |cx| (cx, cz)))
            .collect();
    }
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
    use crate::assets::DimensionType;
    use crate::assets::colors::Colors;
    use crate::render::look::LOOK;
    use crate::render::{BiomeTable, Kamera};

    /// Über eine Seite verläuft das Licht wie im Spiel: Gemischt wird das
    /// fertige Licht der Ecken, nicht ihre Stufen. Halb zwischen Himmel 15
    /// im vollen Licht der weichen Beleuchtung und Himmel 13 mit 153 liegt
    /// so rund 0,686 des Himmels; mit den Stufen gemischt wären es 0,62.
    /// Ohne Seite der AO-Karte gilt das Licht des Blocks.
    #[test]
    fn licht_zwischen_den_ecken_wie_im_spiel() {
        let kino = Kino::new(
            LOOK,
            &DimensionType::oberwelt(),
            &BiomeTable::new(&Colors::default()),
            Kamera::ZWEI_ZU_EINS,
        );
        let ecke = |a: u32, b: u32| a | a << 8 | b << 16 | b << 24;
        let ecken: Ecken = [
            [ecke(240, 208); AO_PLAETZE],
            [0; AO_PLAETZE],
            [ecke(255, 153); AO_PLAETZE],
        ];
        let licht = EckenLicht::new(&kino, [240, 0, 255], Some(ecken));
        // Seite 1, je zur Hälfte Ecke 0 und Ecke 2.
        let karte = [1 << 24 | 128 | 127 << 16, 0];
        let mitte = licht.am(Some(&karte), 0);
        let umgebung = 10.0 / 255.0;
        // Im Blau ist das Himmelslicht der Oberwelt 1. Himmel 13:
        // getBrightness(13/15) · 3 = 1,857143.
        let soll = (128.0 * (3.0 + umgebung) + 127.0 * (1.857_143 + umgebung) * 0.6) / 255.0;
        assert!((mitte[2] - soll).abs() < 1e-5, "{mitte:?} statt {soll}");
        assert!((mitte[2] / 3.0 - 0.686).abs() < 0.02, "{mitte:?}");
        assert_eq!(licht.am(Some(&karte), 1), licht.am(None, 0));
    }

    fn rect() -> ScreenRect {
        ScreenRect::centered(64, 64)
    }

    /// Je Azimut eine Kamera: diagonal 2:1, genordet `north-45` und
    /// `top-north`.
    fn azimute() -> [Projection; 3] {
        [
            Projection::new(16),
            Projection::mit_kamera(16, Kamera::Nord45),
            Projection::mit_kamera(16, Kamera::ObenNord),
        ]
    }

    /// [`AO_WERTE`] wie im Spiel: das Mittel aus vier Werten 1 oder 0,2 in
    /// f32, in jeder Reihenfolge addiert, mal 0,25, dann `ARGB.gray`, das
    /// über `Mth.floor(f * 255)` abrundet.
    #[test]
    fn ao_werte_wie_im_spiel() {
        for maske in 0..16u32 {
            let v = |i: u32| if maske >> i & 1 == 1 { 0.2f32 } else { 1.0 };
            for [a, b, c, d] in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2], [2, 0, 3, 1]] {
                let mittel = (v(a) + v(b) + v(c) + v(d)) * 0.25;
                assert_eq!(
                    (mittel * 255.0).floor() as u32,
                    AO_WERTE[maske.count_ones() as usize],
                    "{maske:04b}"
                );
            }
        }
    }

    /// Je Richtung und Seite im Blick gehört jeder Wert von `ecken_at` an
    /// die Ecke, an der seine beiden Nachbarn aus `AdjacencyInfo.corners`
    /// zusammenstossen: `AmbientVertexRemap` legt ihn auf die Ecke aus
    /// `FaceInfo`, die `ecken_im_blick` dort zeigt. Aus der Vorgabe sind die
    /// Seiten die aus dem Spiel für oben, Süden und Osten.
    #[test]
    fn ecken_im_blick_passen_zu_den_nachbarn() {
        use crate::render::rasterizer::ecken_im_blick;
        let vorgabe = ao_seiten(Richtung::default());
        assert_eq!(
            vorgabe.map(|s| s.richtung),
            [[0, 1, 0], [0, 0, 1], [1, 0, 0]]
        );
        assert_eq!(
            vorgabe[0].nachbarn,
            [[1, 0, 0], [-1, 0, 0], [0, 0, -1], [0, 0, 1]]
        );
        assert_eq!(
            vorgabe.map(|s| s.remap),
            [[2, 3, 0, 1], [0, 1, 2, 3], [1, 2, 3, 0]]
        );
        // Aus Nordwesten zeigt die Seite im Süden den Norden: oben, unten,
        // Osten und Westen der Welt, im Blick Westen und Osten getauscht.
        let nw = Richtung::parse("nw", Kamera::ZWEI_ZU_EINS).unwrap();
        assert_eq!(
            ao_seiten(nw)[1],
            AoSeite {
                richtung: [0, 0, 1],
                nachbarn: [[0, 1, 0], [0, -1, 0], [-1, 0, 0], [1, 0, 0]],
                remap: [3, 0, 1, 2],
            }
        );
        // Aus Südwesten zeigt oben Osten, Westen, Norden und Süden der Welt
        // im Blick im Norden, Süden, Westen und Osten, die Seite im Süden den
        // Westen und die im Osten den Süden.
        let sw = Richtung::parse("sw", Kamera::ZWEI_ZU_EINS).unwrap();
        assert_eq!(
            ao_seiten(sw),
            [
                AoSeite {
                    richtung: [0, 1, 0],
                    nachbarn: [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]],
                    remap: [2, 3, 0, 1],
                },
                AoSeite {
                    richtung: [0, 0, 1],
                    nachbarn: [[0, 1, 0], [0, -1, 0], [-1, 0, 0], [1, 0, 0]],
                    remap: [3, 0, 1, 2],
                },
                AoSeite {
                    richtung: [1, 0, 0],
                    nachbarn: [[0, 0, 1], [0, 0, -1], [0, -1, 0], [0, 1, 0]],
                    remap: [0, 1, 2, 3],
                },
            ]
        );
        // Je Wert von `ecken_at` die zwei Nachbarn seiner Ecke.
        const PAARE: [(usize, usize); 4] = [(3, 0), (2, 0), (2, 1), (3, 1)];
        for name in ["se", "sw", "nw", "ne"] {
            let richtung = Richtung::parse(name, Kamera::ZWEI_ZU_EINS).unwrap();
            for (s, seite) in ao_seiten(richtung).iter().enumerate() {
                assert_eq!(seite.richtung, vorgabe[s].richtung, "{name}, Seite {s}");
                let ecken = ecken_im_blick(richtung, s);
                for (i, &(a, b)) in PAARE.iter().enumerate() {
                    // Die Ecke im Würfel 0..1, in den Koordinaten der Seite.
                    let ecke: [f32; 3] = std::array::from_fn(|k| {
                        let n = seite.nachbarn[a][k] + seite.nachbarn[b][k] + seite.richtung[k];
                        0.5 + 0.5 * n as f32
                    });
                    let [x, y, z] = ecke;
                    let soll = match s {
                        0 => [x, z],
                        1 => [x, y],
                        _ => [z, y],
                    };
                    assert_eq!(ecken[seite.remap[i]], soll, "{name}, Seite {s}, Wert {i}");
                }
            }
        }
    }

    #[test]
    fn rechteck_liegt_um_den_ursprung() {
        let r = ScreenRect::centered(64, 32);
        assert_eq!((r.x, r.y), (-32, -16));
        assert_eq!((r.right(), r.bottom()), (32, 16));
    }

    /// Diagonal müssen u und v dieselbe Parität haben, sonst wären x und z
    /// nicht ganzzahlig. Keine Spalte kommt zweimal.
    #[test]
    fn spalten_sind_ganzzahlig_und_eindeutig() {
        for projection in azimute() {
            let spalten: Vec<(i32, i32)> = columns_at(projection, rect(), 0).collect();
            assert!(!spalten.is_empty());

            let mut gesehen = std::collections::HashSet::new();
            for (x, z) in &spalten {
                assert!(gesehen.insert((*x, *z)), "Spalte ({x}, {z}) doppelt");
            }
        }
    }

    /// Jede Spalte, die im Rechteck landet, muss auch besucht werden.
    #[test]
    fn spalten_decken_das_rechteck_ab() {
        for projection in azimute() {
            let kamera = projection.kamera();
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
                        assert!(besucht.contains(&(x, z)), "{kamera}: ({x}, {z}) fehlt");
                    }
                }
            }
        }
    }

    /// Die Referenz verlässt sich darauf, dass die Tiefe `v` innerhalb
    /// einer Höhe nie fällt; der Schlüssel der Kandidaten sortiert genauso.
    /// Die Schwelle liegt je Kamera knapp unter den 431 und 121 Spalten, die
    /// `columns_at` heute liefert, damit ein Verlust auffällt.
    #[test]
    fn spalten_kommen_nach_tiefe_sortiert() {
        for (projection, mindestens) in azimute().into_iter().zip([420, 118, 118]) {
            let kamera = projection.kamera();
            let mut vorher = i32::MIN;
            let mut gesehen = 0;
            for (x, z) in columns_at(projection, rect(), 7) {
                let v = projection.uv(x, z).1;
                assert!(v >= vorher, "{kamera}: v fällt von {vorher} auf {v}");
                vorher = v;
                gesehen += 1;
            }
            assert!(
                gesehen >= mindestens,
                "{kamera}: nur {gesehen} Spalten geprüft"
            );
        }
    }

    /// Eine höhere Ebene verschiebt das Band nach unten in der Welt, ausser
    /// von oben.
    #[test]
    fn hoehere_ebene_verschiebt_das_band() {
        for projection in azimute() {
            let mitte = |y| {
                let v: Vec<(i32, i32)> = columns_at(projection, rect(), y).collect();
                let summe: i32 = v.iter().map(|&(x, z)| projection.uv(x, z).1).sum();
                summe / v.len() as i32
            };
            if projection.b() > 0.0 {
                assert!(mitte(64) > mitte(0), "{}", projection.kamera());
            } else {
                assert_eq!(mitte(64), mitte(0), "{}", projection.kamera());
            }
        }
    }

    /// Jede Chunkspalte, die das Band berührt, ist dabei: sonst fehlten der
    /// Kachel Kandidaten, und die Referenz zeichnete sie.
    #[test]
    fn band_chunks_decken_das_band_ab() {
        for projection in azimute() {
            let kamera = projection.kamera();
            let rect = rect();
            let (u_min, u_max) = u_window(projection, rect);
            let v_lo = v_window(projection, rect, -64).0;
            let v_hi = v_window(projection, rect, 319).1;
            let chunks: std::collections::HashSet<(i32, i32)> =
                band_chunks(kamera.genordet(), u_min, u_max, v_lo, v_hi)
                    .into_iter()
                    .collect();
            for y in [-64, 0, 100, 319] {
                for (x, z) in columns_at(projection, rect, y) {
                    assert!(
                        chunks.contains(&(x >> 4, z >> 4)),
                        "{kamera}: ({x}, {z}) auf {y}"
                    );
                }
            }
        }
    }

    /// Die Deckungsmaske von vorn nach hinten, auf einer Leinwand von
    /// 100 px, zwei Wörter je Zeile. Ein halb durchsichtiges Sprite vorn
    /// deckt nichts; ein deckendes deckt seinen Umriss, und vom gleichen
    /// dahinter ist nichts mehr zu sehen. Eine Spalte daneben scheint durch,
    /// auch jenseits der Wortgrenze; was neben der Leinwand liegt, gilt als
    /// bedeckt.
    #[test]
    fn deckung_von_vorn_nach_hinten() {
        let rect = ScreenRect {
            x: 0,
            y: 0,
            width: 100,
            height: 10,
        };
        let sprite = |alpha| Sprite {
            image: RgbaImage::from_pixel(8, 4, image::Rgba([1, 2, 3, alpha])),
            offset: (0, 0),
            ao: None,
            weich: false,
            tint: None,
            geometrie: None,
        };
        let (deckend, halb) = (sprite(255), sprite(128));
        let (voll, durch) = (Rows::of(&deckend), Rows::of(&halb));
        let umriss: Vec<_> = (0..4).map(|dy| (dy, 0, 7)).collect();
        let mut d = Deckung::new(rect, &umriss, Vec::new());
        let ort = (60, 2);
        assert!(!d.bedeckt(ort));
        assert!(d.zeichne(&halb, &durch, ort).is_some());
        assert!(!d.bedeckt(ort), "halb durchsichtig deckt nichts");
        assert!(d.zeichne(&deckend, &voll, ort).is_some());
        assert!(d.bedeckt(ort));
        assert!(d.zeichne(&deckend, &voll, ort).is_none(), "ganz verdeckt");
        let sicht = d.zeichne(&deckend, &voll, (61, 2)).expect("Spalte 68");
        assert_eq!((sicht.y0, sicht.y1, sicht.k0, sicht.nk), (2, 6, 0, 2));
        assert_eq!(&d.vis[sicht.start..], [0, 1 << 4].repeat(4));
        assert!(d.bedeckt((200, 2)));
        assert!(d.bedeckt((50, -20)));
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
