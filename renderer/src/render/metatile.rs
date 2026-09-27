use std::collections::{HashMap, HashSet};

use anyhow::Result;
use image::RgbaImage;

use crate::assets::Face;
use crate::assets::blockstate::{self, Leuchten};
use crate::assets::fluid;
use crate::assets::fluid::Fluid;
use crate::world::{Chunk, REGION, Region, Section, World};

use super::rasterizer::{FULL_LIGHT, Light, darken, over};
use super::sprites::{Family, Rows, mask_bit};
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
/// eines Stücks, nicht mit der des ganzen Bilds: bei `--render --size 16384`
/// wären es sonst 1 bis 14 GB mehr, je nach scale.
pub const STUECK: u32 = 1024;

/// Wie [`render_area`], mit einem Cache, der über Kacheln hinweg lebt.
///
/// Kacheln, die nacheinander kommen, liegen nebeneinander oder
/// untereinander und teilen sich fast alle Chunks. Wer sie je Kachel neu
/// lädt, gibt ein Drittel der Renderzeit fürs Dekodieren aus, das er gerade
/// erst gemacht hat.
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
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let deckung = von_vorn(chunks, rect, y_range)?;
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    for &(sprite, origin, ref sicht, light) in chunks.sichtbar.iter().rev() {
        blit_sichtbar(&mut canvas, sprite, origin, light, sicht, &deckung.vis);
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
                    sichtbar.push((sprite, origin, sicht, ids.light));
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
        blit(canvas, d.sprite, d.origin, d.light);
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
        .map(|&(sprite, origin, _, light)| Draw {
            sprite,
            origin,
            light,
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
                    blit(&mut canvas, part, origin, drawn.light);
                }
            }
            for &cell in sprites.foreign_cells() {
                let anchor = anchor_of([x, y, z], cell);
                let drawn = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?;
                if let Some(id) = drawn.sprite
                    && let Some(part) = sprites.part(id, cell)
                {
                    let origin = origin_of(projection, rect, anchor, part);
                    blit(&mut canvas, part, origin, drawn.light);
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

/// Das Himmelslicht unter so vielen Stufen, siehe
/// [`ChunkCache::column_above`].
fn dimmed(stufen: u32) -> u8 {
    FULL_LIGHT - stufen.min(u32::from(FULL_LIGHT)) as u8
}

/// Führt die Familie Wasser, als Quelle, fliessend oder geflutet?
fn is_water(family: Option<&Family>) -> bool {
    family.is_some_and(|f| f.fluid.is_some_and(|(kind, _)| kind == Fluid::Water))
}

/// Was an einem Würfel zu zeichnen ist: das Sprite des Blocks, dazu die
/// Streifen seiner Flüssigkeit über niedrigeren Nachbarn, alles im
/// Licht des Blocks, siehe [`ChunkCache::light_at`].
#[derive(Clone, Copy)]
struct Drawn {
    sprite: Option<SpriteId>,
    strips: [Option<SpriteId>; 2],
    light: Light,
}

impl Default for Drawn {
    fn default() -> Drawn {
        Drawn {
            sprite: None,
            strips: [None; 2],
            light: Light::FULL,
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

/// Zeichnet ein Sprite an seinen Block, ganz, im Licht `light`.
fn blit(canvas: &mut RgbaImage, sprite: &Sprite, (origin_x, origin_y): (i32, i32), light: Light) {
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
    for py in y0..y1 {
        let row = &src[py as usize * w * 4..][..w * 4];
        let drow = &mut dst[(origin_y + py) as usize * cw * 4..][..cw * 4];
        for px in x0..x1 {
            let s = &row[px as usize * 4..][..4];
            if s[3] != 0 {
                mische(&mut drow[(origin_x + px) as usize * 4..][..4], s, factor);
            }
        }
    }
}

/// Legt einen Pixel im Licht `factor` aus [`Light::factors`] über den
/// darunter; deckende direkt statt durch [`over`].
#[inline]
fn mische(d: &mut [u8], s: &[u8], factor: [u32; 3]) {
    let mut s = [s[0], s[1], s[2], s[3]];
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
/// gemerkt hat, im Licht `light`.
fn blit_sichtbar(
    canvas: &mut RgbaImage,
    sprite: &Sprite,
    (ox, oy): (i32, i32),
    light: Light,
    sicht: &Sicht,
    vis: &[u64],
) {
    let (w, cw) = (sprite.image.width() as usize, canvas.width() as usize);
    let src = sprite.image.as_raw();
    let dst: &mut [u8] = canvas;
    let factor = light.factors();
    let zeilen = vis[sicht.start..].chunks(sicht.nk);
    for (y, woerter) in (sicht.y0..sicht.y1).zip(zeilen) {
        let row = &src[(y - oy) as usize * w * 4..][..w * 4];
        let drow = &mut dst[y as usize * cw * 4..][..cw * 4];
        for (j, &bits) in woerter.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let x = (sicht.k0 + j) * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                mische(
                    &mut drow[x * 4..][..4],
                    &row[(x as i32 - ox) as usize * 4..][..4],
                    factor,
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
/// scale 32 acht Spalten, ab scale 4 eine, immer eine Zweierpotenz. Auf der
/// grossen Serverwelt hielt ein Thread damit höchstens 430 bis 520 Chunks
/// bei scale 32 und 825 bei scale 4, samt dem Viertel Spielraum aus
/// [`CACHE_CHUNKS`].
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
/// Streifen Zeile für Zeile rendert — geteilt zwischen Threads wäre er eine
/// Sperre im Renderpfad.
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
    sichtbar: Vec<(&'a Sprite, (i32, i32), Sicht, Light)>,
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
    /// Je Section und Paletteneintrag, wie hell der Block selbst leuchtet:
    /// [`blockstate::leuchten`].
    leuchten: Vec<Vec<Leuchten>>,
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
const FLAGS: usize = 9;
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
            leuchten,
            masks,
            exposed,
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
    ///
    /// Beides setzt voraus, dass die Umrisse benachbarter Blöcke lückenlos
    /// aneinanderstossen, und das tun sie nur, wenn jeder Block auf ganzen
    /// Pixeln liegt: bei einem Vielfachen von 4 als scale. Bei anderen, die
    /// nur die Bibliothek annimmt, verdeckt kein Nachbar; bei scale 6 blieben
    /// sonst Spalten von einem Pixel.
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
        let m = loaded.masks[s]
            .as_deref()
            .expect("nur Sections mit Familie");
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
        // der die Kachel berührt: im Band lag sonst mehr als die Hälfte der
        // Kandidaten neben der Kachel, und jeder bekam eine Sprite-Wahl.
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
        // Sections, die das Band in diesem Chunk gar nicht berührt — von 24
        // sind es meist drei.
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

            match sprites.masked(id, mask) {
                Some(masked) => sprite = Some(masked),
                None if strips.iter().all(Option::is_none) => return Ok(Drawn::default()),
                None => sprite = None,
            }
        }

        // Was selbst leuchtet, bringt sein Blocklicht mit
        // (`LightCoordsUtil.getLightCoords`); den Schein auf die Nachbarn
        // rechnet der Renderer nicht.
        let sky = self.light_at([x, y, z], family)?;
        let light = match self.leuchten_at([x, y, z])? {
            Leuchten::Voll => Light {
                sky: FULL_LIGHT,
                block: FULL_LIGHT,
            },
            Leuchten::Stufe(block) => Light { sky, block },
        };
        // Das Biom kostet einen zweiten Nachschlag; `in_biome` fragt nur
        // fuer Sprites danach, die ueberhaupt Fassungen haben.
        let i = self.slot((x >> 4, z >> 4))?;
        let chunk = &self.slots[i].loaded.as_ref().expect("eben geladen").chunk;
        let tint = |id: SpriteId| sprites.in_biome(id, || chunk.biome_at(x, y, z));
        Ok(Drawn {
            sprite: sprite.map(tint),
            strips: strips.map(|strip| strip.map(tint)),
            light,
        })
    }

    /// Das Himmelslicht, in dem das Spiel den Block an `(x, y, z)` zeichnet:
    /// das der Zelle vor seinen Flächen, gezählt wie in
    /// [`ChunkCache::column_above`]. Unter freiem Himmel ist es 15.
    ///
    /// - Führt der Block selbst Wasser, Seegras etwa oder ein gefluteter
    ///   Zaun, liegt er im Licht dieses Wassers. An der Oberfläche zeichnet
    ///   er sein Wasser selbst, und was darunter liegt, im Licht 14, siehe
    ///   `rasterizer::Canvas::into_image`. Reines Wasser zeichnet
    ///   `FluidRenderer` im helleren Licht aus seiner Zelle und der darüber,
    ///   unter einer Brücke also im Licht der Luft darunter. Hat ein Block
    ///   Wasser Luft neben sich, liegt er mindestens im Licht dieser Luft
    ///   weniger eins, denn das Licht der Zelle kommt im Spiel auch von der
    ///   Seite. Ein Wasserfall liegt so unter freiem Himmel im Licht 14.
    /// - Sonst gilt die Zelle über ihm, aber nur, wenn über ihm Wasser
    ///   steht: der Grund eines Sees, auch in einer Luftblase darunter. An
    ///   Land bleibt alles im Licht 15, auch unter einem Überhang.
    /// - Verdeckt der Block darüber die Oberseite, sieht man nur die Seiten
    ///   nach Osten und Süden, und die liegen im Licht des Wassers davor:
    ///   ein Schiffsrumpf, eine Klippe unter Wasser.
    ///
    /// Eine Zahl je Block: Die Seiten eines Blocks unter Wasser liegen im
    /// Spiel eine Stufe dunkler als seine Oberseite, und am Ufer liegt die
    /// Seite unter der Oberfläche im Licht 14, die Oberseite trocken im
    /// Licht 15.
    fn light_at(&mut self, [x, y, z]: [i32; 3], family: &Family) -> Result<u8> {
        let (wasser, stufen) = self.column_above([x, y + 1, z])?;
        if is_water(Some(family)) {
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
            let mut light = dimmed(if frei { stufen } else { stufen + 1 });
            for [dx, dz] in SEITEN {
                let luft = [x + dx, y, z + dz];
                if self.luecke_at(luft)? {
                    let (w, s) = self.column_above(luft)?;
                    let hell = if w == 0 { FULL_LIGHT } else { dimmed(s) };
                    light = light.max(hell.saturating_sub(1));
                }
            }
            return Ok(light);
        }
        if !self
            .family_at(x, y + 1, z)?
            .is_some_and(|above| above.covers_floor)
        {
            return Ok(match wasser {
                0 => FULL_LIGHT,
                _ => dimmed(stufen),
            });
        }
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
    ///
    /// - Jeder Block Wasser nimmt eine Stufe, denn
    ///   `LiquidBlock.propagatesSkylightDown` ist falsch: Der oberste liegt
    ///   im Licht 14, ab 15 Stufen ist es 0. Ein deckender Block nimmt
    ///   ebenso eine. Alles andere lässt das Licht durch, Seegras, Glas,
    ///   Laub, Luft. So bleiben eine geflutete Höhle unter dem Meeresboden
    ///   und der Grund unter einem Stein im See dunkel.
    /// - Hat ein Block Wasser Luft neben sich, liegt er im Licht 14 wie ein
    ///   Wasserfall, und mit seiner Stufe endet die Zählung: Unter einem Fall
    ///   liegt der Grund eines Beckens eine Stufe tiefer als daneben.
    /// - Liegt unter einem deckenden Block eine Lücke, kommt das Licht dort
    ///   von der Seite, und mit seiner Stufe endet die Zählung: Was über
    ///   einer Brücke oder einem Überhang liegt, ändert darunter nichts.
    ///
    /// Luft und Lücke heisst weder Wasser noch deckend. Gezählt wird in den
    /// Bitmasken der Sections; das Licht aus der Welt liest der Renderer
    /// nicht.
    fn column_above(&mut self, [x, y, z]: [i32; 3]) -> Result<(u32, u32)> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok((0, 0));
        };
        let col = ((z & 15) * 16 + (x & 15)) as usize;
        let (mut wasser, mut stufen) = (0, 0);
        // Liegt unter Bit 0 der nächsten Section eine Lücke? Unter der
        // untersten nicht, dort endet die Welt.
        let mut luecke_darunter = 0u16;
        let mut vorige: Option<i8> = None;
        for s in 0..loaded.chunk.sections().len() {
            // Die Nachbarn laden weitere Chunks, deshalb je Section neu
            // geliehen; der Index bleibt bis zur nächsten Kachel gültig.
            let loaded = self.slots[i].loaded.as_ref().expect("eben geladen");
            let sy = loaded.chunk.sections()[s].y;
            let (nass, fest) = loaded.masks[s]
                .as_ref()
                .map_or((0, 0), |m| (m.bits[WATER][col], m.bits[SOLID][col]));
            // Fehlt eine Section dazwischen, steht dort Luft.
            if vorige.is_some_and(|v| i32::from(v) + 1 != i32::from(sy)) {
                luecke_darunter = 1;
            }
            vorige = Some(sy);
            let luecke = !(nass | fest);
            let unten = i32::from(sy) * 16;
            if unten + 15 < y {
                luecke_darunter = luecke >> 15;
                continue;
            }
            let ab = if unten < y {
                u16::MAX << (y - unten)
            } else {
                u16::MAX
            };
            let mut ende = fest & (luecke << 1 | luecke_darunter) & ab;
            if nass & ab != 0 {
                ende |= nass & self.luecke_daneben(x, z, sy)? & ab;
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

    /// Die Lücken in den vier Spalten neben `(x, z)` in Section `sy`, je
    /// `y` ein Bit: weder Wasser noch deckend. Eine Section ohne Familie ist
    /// Luft; ein Chunk, der fehlt, hat keine Lücke.
    fn luecke_daneben(&mut self, x: i32, z: i32, sy: i8) -> Result<u16> {
        let mut luecke = 0;
        for [dx, dz] in SEITEN {
            let (nx, nz) = (x + dx, z + dz);
            let i = self.slot((nx >> 4, nz >> 4))?;
            let Some(loaded) = self.slots[i].loaded.as_ref() else {
                continue;
            };
            let col = ((nz & 15) * 16 + (nx & 15)) as usize;
            luecke |= match loaded
                .chunk
                .section_index(sy)
                .map(|s| loaded.masks[s].as_deref())
            {
                Some(Some(m)) => !(m.bits[WATER][col] | m.bits[SOLID][col]),
                _ => u16::MAX,
            };
        }
        Ok(luecke)
    }

    /// Ist an einer Weltkoordinate eine Lücke wie in [`luecke_daneben`]?
    fn luecke_at(&mut self, [x, y, z]: [i32; 3]) -> Result<bool> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(false);
        };
        let Ok(sy) = i8::try_from(y >> 4) else {
            return Ok(false);
        };
        let col = ((z & 15) * 16 + (x & 15)) as usize;
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

    /// Wie hell der Block an einer Weltkoordinate selbst leuchtet.
    fn leuchten_at(&mut self, [x, y, z]: [i32; 3]) -> Result<Leuchten> {
        let i = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(Leuchten::Stufe(0));
        };
        let Some((section, slot)) = loaded.chunk.slot(x, y, z) else {
            return Ok(Leuchten::Stufe(0));
        };
        Ok(loaded
            .leuchten
            .get(section)
            .and_then(|l| l.get(slot))
            .copied()
            .unwrap_or(Leuchten::Stufe(0)))
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
