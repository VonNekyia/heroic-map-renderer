use std::collections::{HashMap, HashSet};

use anyhow::Result;
use image::RgbaImage;

use crate::assets::Face;
use crate::assets::blockstate::{self, DUNKELT, Leuchten, SICHT};
use crate::assets::colors::Resolver;
use crate::assets::fluid;
use crate::assets::fluid::Fluid;
use crate::world::{Chunk, REGION, Region, Section, World};

use super::rasterizer::{FULL_LIGHT, Light, NO_AO, ao_factor, darken, over, pack, tinted, with_ao};
use super::sprites::{Family, Rows, TINT_BLOCK, TINT_WATER, mask_bit};
use super::{Cell, OWN_CELL, Projection, Sprite, SpriteId, SpriteSet};

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
/// erst nach Höhe `y`, innerhalb einer Höhe nach Tiefe `v = x + z`. Ein
/// Würfel, der einen anderen verdeckt, liegt nie tiefer, und auf gleicher
/// Höhe verdeckt er ihn genau bei grösserem `v`. Ein Modell, das über
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
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let deckung = von_vorn(chunks, rect, y_range)?;
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    for &(sprite, origin, ref sicht, licht) in chunks.sichtbar.iter().rev() {
        blit_sichtbar(&mut canvas, sprite, origin, licht, sicht, &deckung.vis);
    }
    chunks.vis = deckung.vis;
    Ok(canvas)
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
        for id in ids.ids().rev() {
            if let Some((sprite, rows)) = sprites.part_rows(id, cell) {
                let origin = origin_of(projection, rect, anchor, sprite);
                if let Some(sicht) = deckung.zeichne(sprite, rows, origin) {
                    sichtbar.push((sprite, origin, sicht, (ids.light, ids.ao, ids.tint)));
                }
            }
        }
    }
    chunks.sichtbar = sichtbar;
    Ok(deckung)
}

/// Zeichnet eine Liste auf die Leinwand, Draw für Draw ganz: die
/// Vergleichsgrösse für die Karte bei Listen, die kein Ausschnitt liefert.
pub fn draw_all(canvas: &mut RgbaImage, draws: &[Draw]) {
    for d in draws {
        blit(canvas, d.sprite, d.origin, (d.light, d.ao, d.tint));
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
    /// Das Licht seines Blocks, siehe [`ChunkCache::light_at`].
    pub light: Light,
    /// Die weiche Beleuchtung an den Ecken seiner Seiten, siehe
    /// [`ChunkCache::ao_at`].
    pub ao: [u32; 3],
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
        .map(|&(sprite, origin, _, (light, ao, tint))| Draw {
            sprite,
            origin,
            light,
            ao,
            tint,
        })
        .collect();
    chunks.vis = deckung.vis;
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
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    let mut chunks = ChunkCache::new(world, sprites);

    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            let drawn = chunks.sprite_at(x, y, z)?;
            for id in drawn.ids() {
                if let Some(part) = sprites.part(id, OWN_CELL) {
                    let origin = origin_of(projection, rect, [x, y, z], part);
                    blit(
                        &mut canvas,
                        part,
                        origin,
                        (drawn.light, drawn.ao, drawn.tint),
                    );
                }
            }
            for &cell in sprites.foreign_cells() {
                let anchor = anchor_of([x, y, z], cell);
                let drawn = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?;
                if let Some(id) = drawn.sprite
                    && let Some(part) = sprites.part(id, cell)
                {
                    let origin = origin_of(projection, rect, anchor, part);
                    blit(
                        &mut canvas,
                        part,
                        origin,
                        (drawn.light, drawn.ao, drawn.tint),
                    );
                }
            }
        }
    }

    Ok(canvas)
}

/// Wo der Ursprung eines Blocks auf der Leinwand liegt; dort sitzt die
/// Mitte seines Umrisses.
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

/// Die vier Nachbarn in der Waagrechten.
const SEITEN: [[i32; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];

/// Eine Seite aus [`AO_FACES`](super::rasterizer::AO_FACES), wie
/// `BlockModelLighter` sie sieht, per javap am 26.2-Client.
struct AoSeite {
    /// Wohin sie zeigt.
    richtung: [i32; 3],
    /// Die vier Nachbarn in ihrer Ebene, wie `AdjacencyInfo.corners`.
    nachbarn: [[i32; 3]; 4],
    /// An welche Ecke aus `FaceInfo` der Wert `i` geht:
    /// `AmbientVertexRemap`.
    remap: [usize; 4],
}

const AO_SEITEN: [AoSeite; 3] = [
    // Oben: Osten, Westen, Norden, Süden.
    AoSeite {
        richtung: [0, 1, 0],
        nachbarn: [[1, 0, 0], [-1, 0, 0], [0, 0, -1], [0, 0, 1]],
        remap: [2, 3, 0, 1],
    },
    // Süden: Westen, Osten, unten, oben.
    AoSeite {
        richtung: [0, 0, 1],
        nachbarn: [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0]],
        remap: [0, 1, 2, 3],
    },
    // Osten: unten, oben, Norden, Süden.
    AoSeite {
        richtung: [1, 0, 0],
        nachbarn: [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]],
        remap: [1, 2, 3, 0],
    },
];
/// Der Wert einer Ecke, wenn so viele ihrer vier Blöcke abdunkeln:
/// `ARGB.gray` des Mittels aus 1 und 0,2, in f32 wie im Spiel.
const AO_WERTE: [u32; 5] = [255, 204, 153, 102, 51];

/// Das Himmelslicht unter so vielen Stufen, siehe
/// [`ChunkCache::column_above`].
fn dimmed(stufen: u32) -> u8 {
    FULL_LIGHT - stufen.min(u32::from(FULL_LIGHT)) as u8
}

/// Die Bits der Section mit dem y `sy`, die über `oben` liegen.
fn ueber(oben: i32, sy: i8) -> u16 {
    let n = oben.saturating_sub(i32::from(sy) * 16 - 1).clamp(0, 16) as u32;
    u16::MAX.checked_shl(n).unwrap_or(0)
}

/// Führt die Familie Wasser, als Quelle, fliessend oder geflutet?
fn is_water(family: Option<&Family>) -> bool {
    family.is_some_and(|f| f.fluid.is_some_and(|(kind, _)| kind == Fluid::Water))
}

/// Was an einem Würfel zu zeichnen ist: das Sprite des Blocks, dazu die
/// Streifen seiner Flüssigkeit über niedrigeren Nachbarn, alles im
/// Licht des Blocks, siehe [`ChunkCache::light_at`], mit der weichen
/// Beleuchtung an seinen Ecken, siehe [`ChunkCache::ao_at`], und in den
/// Farben seines Bioms, siehe [`ChunkCache::tints_at`].
#[derive(Clone, Copy)]
struct Drawn {
    sprite: Option<SpriteId>,
    strips: [Option<SpriteId>; 2],
    light: Light,
    ao: [u32; 3],
    tint: [u32; 2],
}

impl Default for Drawn {
    fn default() -> Drawn {
        Drawn {
            sprite: None,
            strips: [None; 2],
            light: Light::FULL,
            ao: NO_AO,
            tint: [0; 2],
        }
    }
}

impl Drawn {
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

    (v_min..=v_max).flat_map(move |v| {
        // x und z sind ganzzahlig, also haben u und v dieselbe Parität.
        let start = u_min + (u_min - v).rem_euclid(2);
        (start..=u_max)
            .step_by(2)
            .map(move |u| ((u + v) / 2, (v - u) / 2))
    })
}

/// Das Licht eines Blocks, die weiche Beleuchtung an den Ecken seiner
/// Seiten und die Farben für seine Tönungskarte, wie [`Drawn`] sie trägt.
type Licht = (Light, [u32; 3], [u32; 2]);

/// Die Faktoren für [`darken`] an Pixel `i` eines Sprites: das Licht je
/// Kanal, mit der AO-Karte des Sprites dazu die weiche Beleuchtung an den
/// Ecken `ao`.
#[inline]
fn faktor(karte: Option<&[u32]>, i: usize, light: [u32; 3], ao: [u32; 3]) -> [u32; 3] {
    match karte {
        Some(karte) => {
            let a = ao_factor(karte[i], ao);
            // Bei vollem Licht ist `with_ao(255, a)` genau `a`.
            if light == [255; 3] {
                [a; 3]
            } else {
                light.map(|f| with_ao(f, a))
            }
        }
        None => light,
    }
}

/// Die AO-Karte eines Sprites, wenn sie etwas abdunkelt.
fn karte(sprite: &Sprite, ao: [u32; 3]) -> Option<&[u32]> {
    sprite.ao.as_deref().filter(|_| ao != NO_AO)
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
    (light, ao, farben): Licht,
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
    let factor = light.factors();
    let karte = karte(sprite, ao);
    for py in y0..y1 {
        let row = &src[py as usize * w * 4..][..w * 4];
        let drow = &mut dst[(origin_y + py) as usize * cw * 4..][..cw * 4];
        for px in x0..x1 {
            let s = &row[px as usize * 4..][..4];
            if s[3] != 0 {
                let i = py as usize * w + px as usize;
                let f = faktor(karte, i, factor, ao);
                let t = anteile(sprite, i, farben);
                mische(&mut drow[(origin_x + px) as usize * 4..][..4], s, f, t);
            }
        }
    }
}

/// Legt einen Pixel in den Farben seines Blocks und im Licht `factor` aus
/// [`Light::factors`] über den darunter; deckende direkt statt durch
/// [`over`]. Erst die Farbe, dann das Licht, wie im Spiel.
#[inline]
fn mische(d: &mut [u8], s: &[u8], factor: [u32; 3], tint: Option<([u32; 2], [u32; 2])>) {
    let mut s = [s[0], s[1], s[2], s[3]];
    if let Some((anteile, farben)) = tint {
        s = tinted(s, anteile, farben);
    }
    if factor != [255; 3] {
        s = darken(s, factor);
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
    (light, ao, farben): Licht,
    sicht: &Sicht,
    vis: &[u64],
) {
    let (w, cw) = (sprite.image.width() as usize, canvas.width() as usize);
    let src = sprite.image.as_raw();
    let dst: &mut [u8] = canvas;
    let factor = light.factors();
    let karte = karte(sprite, ao);
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
                    faktor(karte, i, factor, ao),
                    anteile(sprite, i, farben),
                );
            }
        }
    }
}

/// Wie viele Kachelspalten ein Streifen höchstens breit ist. Der Export
/// rendert Streifen Zeile für Zeile, und [`ChunkCache`] behält, was die
/// letzte Zeile gebraucht hat.
///
/// Eine Kachel ist ein schräger Schnitt durch die volle Bauhöhe: ein Chunk
/// liegt im Band von drei bis vier Kachelspalten und gut zwanzig Zeilen.
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
    index: HashMap<(i32, i32), usize>,
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
    sichtbar: Vec<(&'a Sprite, (i32, i32), Sicht, Licht)>,
    /// Je Chunk und Höhe das Biom jedes Blocks nach [`BiomeTable::quart`],
    /// `u16::MAX`, solange es nicht gerechnet ist; siehe
    /// [`ChunkCache::biome_of`].
    ///
    /// [`BiomeTable::quart`]: super::BiomeTable::quart
    biome_layers: Vec<BiomeLayer>,
    biome_index: HashMap<(i32, i32, i32), usize>,
    biome_last: usize,
}

/// Chunk und Höhe, dazu das Biom je Block der Schicht.
type BiomeLayer = ((i32, i32, i32), Box<[u16; 256]>);

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
    /// Je Section und Paletteneintrag, wie hell der Block selbst leuchtet:
    /// [`blockstate::leuchten`].
    leuchten: Vec<Vec<Leuchten>>,
    /// Je Section ihre Bitmasken, `None` für eine Section ohne Familie und
    /// ohne Block, der abdunkelt oder die Sicht nimmt.
    masks: Vec<Option<Box<Masks>>>,
    /// Je Spalte `z * 16 + x` das y des obersten Blocks mit Wasser,
    /// `i32::MIN` ohne: Eine Lücke darunter liegt nicht im Licht, siehe
    /// [`ChunkCache::luecke_daneben`].
    oberstes_wasser: [i32; 256],
    /// Je Section die Kandidaten, sobald einmal berechnet — dafür müssen
    /// die Nachbarchunks da sein, deshalb nicht beim Laden.
    exposed: Vec<Option<Box<Exposed>>>,
    /// Je Section und Paletteneintrag der Biome die Nummer des Bioms in der
    /// [`BiomeTable`](super::BiomeTable).
    biomes: Vec<Vec<u16>>,
    /// Je Block, dessen Blockentity mit seinen Daten ein anderes Bild gibt,
    /// die Familie dafür ([`SpriteSet::variante`]), nach Lage sortiert.
    varianten: Vec<([i32; 3], u32)>,
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
/// Dunkelt ab ([`DUNKELT`]), [`VIEW`] nimmt die Sicht ([`SICHT`]): die Bits
/// aus [`blockstate::schatten`] für [`ChunkCache::ao_at`], auch für Blöcke
/// ohne Familie.
const DARK: usize = 9;
const VIEW: usize = 10;
const FLAGS: usize = 11;
/// Je Flüssigkeit, in der Reihenfolge von [`Masks::up`]: das Bit "enthält
/// sie" und das Bit "nur sie".
const FLUIDS: [(usize, usize); 2] = [(WATER, PURE_WATER), (LAVA, PURE_LAVA)];

/// Bitmasken einer Section: je Eigenschaft und Spalte `z * 16 + x` ein
/// Wort, Bit `y`. Ob ein Block von seinen drei Nachbarn verdeckt ist, sind
/// damit für sechzehn Blöcke einer Spalte auf einmal ein paar
/// Wortoperationen.
/// Siehe docs/renderer/renderpfad.md, „Bitmasken“.
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

impl Masks {
    /// `None`, wenn in der Section weder eine Familie steht noch ein Block,
    /// der abdunkelt oder die Sicht nimmt. `schatten` hat je Paletteneintrag
    /// die Bits aus [`blockstate::schatten`].
    fn of(
        section: &Section,
        families: &[Option<u32>],
        schatten: &[u8],
        sprites: &SpriteSet,
    ) -> Option<Box<Masks>> {
        let bit = |set: bool, flag: usize| (set as u16) << flag;
        let flags: Vec<u16> = families
            .iter()
            .zip(schatten)
            .map(|(family, &s)| {
                family.map_or(0, |index| flags(sprites.family(index)))
                    | bit(s & DUNKELT != 0, DARK)
                    | bit(s & SICHT != 0, VIEW)
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
        if [PRESENT, DARK, VIEW]
            .iter()
            .all(|&e| m.bits[e].iter().all(|&w| w == 0))
        {
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
        let leuchten = chunk
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
            .map(|(section, families)| {
                let schatten: Vec<u8> = section
                    .blocks()
                    .palette()
                    .iter()
                    .map(blockstate::schatten)
                    .collect();
                Masks::of(section, families, &schatten, sprites)
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
        // Die Sections liegen aufsteigend, die oberste mit Wasser gewinnt.
        let mut oberstes_wasser = [i32::MIN; 256];
        for (section, m) in chunk.sections().iter().zip(&masks) {
            let Some(m) = m else { continue };
            for (oben, &nass) in oberstes_wasser.iter_mut().zip(&m.bits[WATER]) {
                if nass != 0 {
                    *oben = i32::from(section.y) * 16 + 15 - nass.leading_zeros() as i32;
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
            oberstes_wasser,
            exposed,
            biomes,
            varianten,
        }
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
            index: HashMap::new(),
            last: usize::MAX,
            tile: 0,
            keep: tiles as u32,
            grenze: CACHE_CHUNKS,
            vis: Vec::new(),
            sichtbar: Vec::new(),
            biome_layers: Vec::new(),
            biome_index: HashMap::new(),
            biome_last: usize::MAX,
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
        let regionen: HashSet<(i32, i32)> =
            self.slots.iter().map(|slot| region_of(slot.key)).collect();
        self.regions.retain(|key, _| regionen.contains(key));
        // Biome nach dem Zoom nur für Chunks, die bleiben.
        self.biome_layers
            .retain(|((cx, _, cz), _)| self.index.contains_key(&(*cx, *cz)));
        self.biome_index = self
            .biome_layers
            .iter()
            .enumerate()
            .map(|(i, (key, _))| (*key, i))
            .collect();
        self.biome_last = usize::MAX;
        self.grenze = (self.slots.len() + self.slots.len() / 4).max(CACHE_CHUNKS);
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
        let region_key = region_of(key);
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
    /// Ein Block ist verdeckt, wenn die Nachbarn nach +x und +z ihren ganzen
    /// Umriss decken und der nach +y seinen Boden. Nach +y ist das ein Shift
    /// in derselben Spalte; am oberen Rand kommt das Bit aus der Section
    /// darüber, an den Rändern +x und +z aus dem Nachbarchunk.
    ///
    /// Reine Flüssigkeit, Wasser wie Lava, zeichnet ausserdem nichts, wo über
    /// ihr dieselbe steht und sie zu beiden Seiten an dieselbe mit derselben
    /// darüber grenzt oder an einen deckenden Nachbarn. Oben genügt ein
    /// deckender Block nicht: ohne dieselbe darüber ragt die Oberfläche in die
    /// Seiten hinein. Lava deckt nur bei scale 4, sonst fiele dort kein Block
    /// weg.
    ///
    /// Beides gilt nur bei einem Vielfachen von 4 als scale, wenn jeder Block
    /// auf ganzen Pixeln liegt; bei anderen, die nur die Bibliothek annimmt,
    /// verdeckt kein Nachbar.
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
        let nx = self.edge((key.0 + 1, key.1), section_y, true)?;
        let nz = self.edge((key.0, key.1 + 1), section_y, false)?;
        let verdecken = self.sprites.projection().scale().is_multiple_of(4);

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
        // Das Fenster von `v` verschiebt sich je Höhe um genau 2.
        let (v0_min, v0_max) = v_window(projection, rect, 0);
        let (v_lo, v_hi) = (v0_min + 2 * y_range.0, v0_max + 2 * y_range.1);
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
        let in_y = |y: i32| (y_range.0..=y_range.1).contains(&y);
        let in_band = |y: i32, v: i32, u: i32| {
            in_y(y)
                && (u_min..=u_max).contains(&u)
                && (v0_min + 2 * y..=v0_max + 2 * y).contains(&v)
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
        let scale = projection.scale() as f64;
        let bleed = BLEED_BLOCKS as f64 * scale;
        // Höhen, die das Band in einem Chunk erreichen kann: die Umkehrung
        // von `v_window` für die kleinste und grösste Tiefe `v` des Chunks,
        // grosszügig gerundet. Entscheidend bleibt die Prüfung je Block
        // (`touches`, `in_band`); das hier spart nur die Schleife über
        // Sections, die das Band in diesem Chunk gar nicht berührt.
        // Siehe docs/renderer/renderpfad.md, „Kandidaten“.
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
                            key: key_of(y, v, u, 0),
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
                        loose: false,
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
    /// bekommt, welche Flüssigkeitsflächen die Nachbarn verdecken und in
    /// welchen Farben sein Biom den Block tönt. Die Bilder sind vorab
    /// gerastert, die Farben kommen beim Zeichnen dazu.
    fn sprite_at(&mut self, x: i32, y: i32, z: i32) -> Result<Drawn> {
        let sprites = self.sprites;
        let Some((family, leuchten)) = self.block_at(x, y, z)? else {
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

            match sprites.masked(id, mask) {
                Some(masked) => sprite = Some(masked),
                None if strips.iter().all(Option::is_none) => return Ok(Drawn::default()),
                None => sprite = None,
            }
        }

        // Was selbst leuchtet, bringt sein Blocklicht mit
        // (`LightCoordsUtil.getLightCoords`) und bekommt im Spiel keine
        // weiche Beleuchtung (`ModelBlockRenderer.tesselateBlock`); den
        // Schein auf die Nachbarn rechnet der Renderer nicht.
        let light = match leuchten {
            Leuchten::Voll(_) => Light {
                sky: FULL_LIGHT,
                block: FULL_LIGHT,
            },
            Leuchten::Stufe(block) => Light {
                sky: self.light_at([x, y, z], family)?,
                block,
            },
        };
        let ao = match sprite {
            Some(id) if sprites.has_ao(id) && leuchten == Leuchten::Stufe(0) => {
                self.ao_at([x, y, z])?
            }
            _ => NO_AO,
        };
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
            light,
            ao,
            tint,
        })
    }

    /// Die Farben eines Blocks für seine Tönungskarte, gepackt wie sie: die
    /// seines Blocks aus dem Resolver der Familie, wenn `kinds` [`TINT_BLOCK`]
    /// trägt, bei `tint_below` am Block darunter; die des Wassers mit
    /// [`TINT_WATER`], am Block selbst. Beide gemischt wie im Client
    /// ([`BiomeTable::blend`](super::BiomeTable::blend)). 0, wo keine Karte
    /// sie braucht.
    fn tints_at(&mut self, [x, y, z]: [i32; 3], family: &Family, kinds: u8) -> Result<[u32; 2]> {
        let table = self.sprites.biomes();
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

    /// Das Biom eines Blocks als Nummer der [`BiomeTable`](super::BiomeTable):
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
        let slot = self.slot((qx >> 2, qz >> 2))?;
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

    /// Die weiche Beleuchtung an den Ecken der drei sichtbaren Seiten eines
    /// Blocks, wie `BlockModelLighter.prepareQuadAmbientOcclusion` in 26.2
    /// sie für eine volle Seite rechnet: je Ecke das Mittel aus dem Block vor
    /// der Seite, ihren zwei Nachbarn in dieser Schicht und dem Block in der
    /// Ecke, 1 oder 0,2 für einen, der [`DUNKELT`]. Der Block in der Ecke zählt
    /// nur, wenn hinter keinem der beiden Nachbarn ein Block mit [`SICHT`]
    /// steht. Eine Seite, die ihr Nachbar ganz deckt ([`SOLID`]), bleibt ohne
    /// Werte.
    /// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
    fn ao_at(&mut self, block: [i32; 3]) -> Result<[u32; 3]> {
        let [fest, dunkelt, sicht] = self.umgebung(block)?;
        // Alles relativ zum Block, siehe `umgebung`.
        let bit = |ebene: u64, [dx, dy, dz]: [i32; 3]| {
            ebene >> (dy + 1 + 4 * (dx + 1) + 16 * (dz + 1)) & 1 != 0
        };
        let bei = |p: [i32; 3], o: [i32; 3]| [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
        let mut out = NO_AO;
        for (seite, s) in AO_SEITEN.iter().enumerate() {
            let (d, nachbarn) = (s.richtung, s.nachbarn);
            let vor = d;
            if bit(fest, vor) {
                continue;
            }
            let mut dunkel = [false; 4];
            let mut frei = [false; 4];
            for (k, &n) in nachbarn.iter().enumerate() {
                dunkel[k] = bit(dunkelt, bei(vor, n));
                frei[k] = !bit(sicht, bei(bei(vor, n), d));
            }
            let ecke = |a: usize, b: usize| {
                if frei[a] || frei[b] {
                    bit(dunkelt, bei(bei(vor, nachbarn[a]), nachbarn[b]))
                } else {
                    dunkel[0]
                }
            };
            let (e03, e02, e12, e13) = (ecke(0, 3), ecke(0, 2), ecke(1, 2), ecke(1, 3));
            let davor = bit(dunkelt, vor);
            let werte = [
                [dunkel[3], dunkel[0], e03, davor],
                [dunkel[2], dunkel[0], e02, davor],
                [dunkel[2], dunkel[1], e12, davor],
                [dunkel[3], dunkel[1], e13, davor],
            ];
            let mut ecken = [0u32; 4];
            for (wert, &ziel) in werte.iter().zip(&s.remap) {
                ecken[ziel] = AO_WERTE[wert.iter().filter(|&&d| d).count()];
            }
            out[seite] = ecken[0] | ecken[1] << 8 | ecken[2] << 16 | ecken[3] << 24;
        }
        Ok(out)
    }

    /// Die Ebenen [`SOLID`], [`DARK`] und [`VIEW`] um einen Block, so weit
    /// [`ChunkCache::ao_at`] fragt: je Ebene ein Bit für jede Zelle
    /// `(x + dx, y + dy, z + dz)` mit `dx`, `dy` und `dz` von -1 bis 2, an
    /// Stelle `dy + 1 + 4 · (dx + 1) + 16 · (dz + 1)`. Die vier Zellen einer
    /// Spalte kommen aus einem Wort je Ebene, an einer Sectionsgrenze aus
    /// zweien. Ausserhalb der Welt und in fehlenden Chunks steht nichts, wie
    /// für Luft.
    fn umgebung(&mut self, [x, y, z]: [i32; 3]) -> Result<[u64; 3]> {
        let mut out = [0; 3];
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
                        .map_or([0; 3], |m| {
                            [SOLID, DARK, VIEW].map(|e| u32::from(m.bits[e][col]))
                        })
                };
                let unten = y - 1;
                let sy = unten >> 4;
                let lo = woerter(sy);
                let hi = if (y + 2) >> 4 != sy {
                    woerter(sy + 1)
                } else {
                    [0; 3]
                };
                let stelle = 4 * (dx + 1) + 16 * (dz + 1);
                for ((o, l), h) in out.iter_mut().zip(lo).zip(hi) {
                    *o |= u64::from((l | h << 16) >> (unten & 15) & 15) << stelle;
                }
            }
        }
        Ok(out)
    }

    /// Das Himmelslicht, in dem das Spiel den Block an `(x, y, z)` zeichnet:
    /// das der Zelle vor seinen Flächen, gezählt wie in
    /// [`ChunkCache::column_above`], eine Zahl je Block. Unter freiem Himmel
    /// ist es 15.
    ///
    /// - Führt der Block selbst Wasser, liegt er im Licht dieses Wassers, mit
    ///   Luft im Licht daneben im Licht 14.
    /// - Sonst gilt die Zelle über ihm, wenn über ihm Wasser steht; an Land
    ///   bleibt alles im Licht 15.
    /// - Verdeckt der Block darüber die Oberseite, gilt das hellere Wasser vor
    ///   der Ost- und der Südseite.
    ///
    /// Siehe docs/renderer/wasser-und-licht.md, „Welches Licht ein Block bekommt“.
    /// Siehe docs/renderer/wasser-und-licht.md, „Licht von der Seite“.
    fn light_at(&mut self, [x, y, z]: [i32; 3], family: &Family) -> Result<u8> {
        if is_water(Some(family)) {
            let stufen = self.column_above([x, y + 1, z])?.1;
            if stufen == 0 {
                return Ok(FULL_LIGHT);
            }
            // Reines Wasser zeichnet `FluidRenderer` im helleren Licht aus
            // seiner Zelle und der darüber, und die hat eine Stufe mehr; ein
            // deckender Block darüber hat selbst kein Licht. Ein Modell im
            // Wasser, Seegras oder ein gefluteter Zaun, liegt im Licht der
            // Zelle.
            let frei = family.pure_fluid
                && !self
                    .family_at(x, y + 1, z)?
                    .is_some_and(|above| above.opaque);
            // Luft daneben, die selbst im Licht liegt, hebt es auf 14.
            for [dx, dz] in SEITEN {
                if self.luecke_at([x + dx, y, z + dz])? {
                    return Ok(FULL_LIGHT - 1);
                }
            }
            return Ok(dimmed(if frei { stufen } else { stufen + 1 }));
        }
        if !self
            .family_at(x, y + 1, z)?
            .is_some_and(|above| above.covers_floor)
        {
            let (wasser, stufen) = self.column_above([x, y + 1, z])?;
            return Ok(match wasser {
                0 => FULL_LIGHT,
                _ => dimmed(stufen),
            });
        }
        // Die Oberseite ist verdeckt, die Zählung über ihm braucht es nicht.
        let mut light = None;
        for [dx, dz] in [[1, 0], [0, 1]] {
            if is_water(self.family_at(x + dx, y, z + dz)?) {
                let seite = dimmed(self.column_above([x + dx, y, z + dz])?.1);
                light = Some(light.map_or(seite, |l: u8| l.max(seite)));
            }
        }
        Ok(light.unwrap_or(FULL_LIGHT))
    }

    /// Wie viele Blöcke Wasser über `(x, y, z)` stehen, die Zelle selbst
    /// mitgezählt, und wie viele Stufen Himmelslicht sie samt den deckenden
    /// Blöcken nehmen, nach oben gezählt, bis das Licht von der Seite kommt:
    /// neben Luft ohne Wasser darüber und in einer Lücke unter einem deckenden
    /// Block. Jeder Block Wasser und jeder deckende nimmt eine Stufe, alles
    /// andere lässt das Licht durch. Luft und Lücke heisst weder Wasser noch
    /// deckend. Gezählt wird in den Bitmasken der Sections; das Licht aus der
    /// Welt liest der Renderer nicht.
    /// Siehe docs/renderer/wasser-und-licht.md, „Wie gezählt wird“.
    fn column_above(&mut self, [x, y, z]: [i32; 3]) -> Result<(u32, u32)> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok((0, 0));
        };
        let col = ((z & 15) * 16 + (x & 15)) as usize;
        let spalte = |loaded: &Loaded, s: usize| {
            loaded.masks[s]
                .as_ref()
                .map_or((0, 0), |m| (m.bits[WATER][col], m.bits[SOLID][col]))
        };
        // Gezählt wird ab der Section, die bis y reicht. Von der darunter
        // zählt nur, ob unter Bit 0 eine Lücke liegt; unter der untersten
        // nicht, dort endet die Welt.
        let sections = loaded.chunk.sections();
        let start = sections.partition_point(|s| i32::from(s.y) * 16 + 15 < y);
        let mut vorige = start.checked_sub(1).map(|s| sections[s].y);
        let mut luecke_darunter = start.checked_sub(1).map_or(0, |s| {
            let (nass, fest) = spalte(loaded, s);
            !(nass | fest) >> 15
        });
        let (mut wasser, mut stufen) = (0, 0);
        for s in start..sections.len() {
            // Die Nachbarn laden weitere Chunks, deshalb je Section neu
            // geliehen; der Index bleibt bis zur nächsten Kachel gültig.
            let loaded = self.slots[i].loaded.as_ref().expect("eben geladen");
            let sy = loaded.chunk.sections()[s].y;
            let (nass, fest) = spalte(loaded, s);
            // Fehlt eine Section dazwischen, steht dort Luft.
            if vorige.is_some_and(|v| i32::from(v) + 1 != i32::from(sy)) {
                luecke_darunter = 1;
            }
            vorige = Some(sy);
            let luecke = !(nass | fest);
            let unten = i32::from(sy) * 16;
            let ab = if unten < y {
                u16::MAX << (y - unten)
            } else {
                u16::MAX
            };
            let mut ende = fest & (luecke << 1 | luecke_darunter) & ab;
            if nass & ab != 0 {
                ende |= nass & self.luecke_daneben((i, s, sy), x, z)? & ab;
            }
            let zaehlt = (nass | fest) & ab;
            if ende != 0 {
                // Bis einschliesslich des untersten Blocks, an dem sie endet.
                let bis = ende ^ (ende - 1);
                return Ok((
                    wasser + (nass & ab & bis).count_ones(),
                    stufen + (zaehlt & bis).count_ones(),
                ));
            }
            wasser += (nass & ab).count_ones();
            stufen += zaehlt.count_ones();
            luecke_darunter = luecke >> 15;
        }
        Ok((wasser, stufen))
    }

    /// Die Lücken im Licht in den vier Spalten neben `(x, z)` auf der Höhe
    /// der Section `s` mit dem y `sy` des Chunks in Slot `i`, je `y` ein
    /// Bit: weder Wasser noch deckend, und darüber steht in der Spalte kein
    /// Wasser. Eine Section ohne Familie ist Luft; ein Chunk, der fehlt,
    /// hat keine Lücke.
    fn luecke_daneben(&mut self, (i, s, sy): (usize, usize, i8), x: i32, z: i32) -> Result<u16> {
        let mut luecke = 0;
        for [dx, dz] in SEITEN {
            let (nx, nz) = (x + dx, z + dz);
            let col = ((nz & 15) * 16 + (nx & 15)) as usize;
            let (masken, oben) = if (nx >> 4, nz >> 4) == (x >> 4, z >> 4) {
                // Im selben Chunk dieselbe Section, ohne Nachschlag.
                let loaded = self.slots[i].loaded.as_ref().expect("eben geladen");
                (
                    Some(loaded.masks[s].as_deref()),
                    loaded.oberstes_wasser[col],
                )
            } else {
                let j = self.slot((nx >> 4, nz >> 4))?;
                let Some(loaded) = self.slots[j].loaded.as_ref() else {
                    continue;
                };
                let masken = loaded
                    .chunk
                    .section_index(sy)
                    .map(|s| loaded.masks[s].as_deref());
                (masken, loaded.oberstes_wasser[col])
            };
            let frei = match masken {
                Some(Some(m)) => !(m.bits[WATER][col] | m.bits[SOLID][col]),
                _ => u16::MAX,
            };
            luecke |= frei & ueber(oben, sy);
        }
        Ok(luecke)
    }

    /// Ist an einer Weltkoordinate eine Lücke im Licht wie in
    /// [`luecke_daneben`]?
    fn luecke_at(&mut self, [x, y, z]: [i32; 3]) -> Result<bool> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(false);
        };
        let Ok(sy) = i8::try_from(y >> 4) else {
            return Ok(false);
        };
        let col = ((z & 15) * 16 + (x & 15)) as usize;
        if y <= loaded.oberstes_wasser[col] {
            return Ok(false);
        }
        Ok(
            match loaded
                .chunk
                .section_index(sy)
                .map(|s| loaded.masks[s].as_deref())
            {
                Some(Some(m)) => (m.bits[WATER][col] | m.bits[SOLID][col]) >> (y & 15) & 1 == 0,
                _ => true,
            },
        )
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

/// Die Region eines Chunks.
fn region_of((cx, cz): (i32, i32)) -> (i32, i32) {
    (cx.div_euclid(REGION), cz.div_euclid(REGION))
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
            tint: None,
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
