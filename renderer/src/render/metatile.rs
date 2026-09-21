use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use image::{Rgba, RgbaImage};

use crate::world::{Chunk, World};

use super::{Cell, OWN_CELL, Projection, Sprite, SpriteId, SpriteSet};

/// Reserve um das Zielrechteck herum, in Blockbreiten.
///
/// Sprites dürfen über den Blockumriss hinausragen — Feuer ist höher als
/// ein Block, Zäune breiter. Ohne diese Reserve fehlen an den Rändern
/// Blöcke, deren Ursprung knapp ausserhalb liegt.
const BLEED_BLOCKS: i32 = 3;

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

    fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    fn bottom(&self) -> i32 {
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
/// Ein globaler Tiefenpuffer ist damit unnötig. `columns_at` liefert die
/// Spalten bereits in dieser Reihenfolge.
pub fn render_area(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    let projection = sprites.projection();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    let mut chunks = ChunkCache::new(world);
    // Fast immer leer. Dann fällt die Suche nach Überhängen ganz weg.
    let ueberhaenge = !sprites.foreign_cells().is_empty();

    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            let own = chunks.sprite_at(sprites, x, y, z)?;

            // Erst suchen, dann auf Verdeckung prüfen: der Test kostet drei
            // Nachschläge und lohnt nur, wenn hier überhaupt etwas liegt.
            if own.is_none() && !(ueberhaenge && anything_foreign(&mut chunks, sprites, x, y, z)?) {
                continue;
            }
            if is_hidden(&mut chunks, sprites, own, x, y, z)? {
                continue;
            }

            if let Some(id) = own
                && let Some(part) = sprites.part(id, OWN_CELL)
            {
                blit(&mut canvas, part, rect, projection, [x, y, z]);
            }
            if ueberhaenge {
                for &cell in sprites.foreign_cells() {
                    let anchor = anchor_of([x, y, z], cell);
                    let Some(id) = chunks.sprite_at(sprites, anchor[0], anchor[1], anchor[2])?
                    else {
                        continue;
                    };
                    if let Some(part) = sprites.part(id, cell) {
                        blit(&mut canvas, part, rect, projection, anchor);
                    }
                }
            }
        }
    }

    Ok(canvas)
}

/// Der Block, dessen Modell in `cell` hineinragen würde.
fn anchor_of([x, y, z]: [i32; 3], cell: Cell) -> [i32; 3] {
    [x - cell[0], y - cell[1], z - cell[2]]
}

/// Ragt irgendein Nachbarmodell in diesen Würfel?
///
/// `foreign_cells` ist leer, solange kein Modell seinen Blockwürfel
/// verlässt — dann kostet das hier nichts. Sonst ist es ein Nachschlagen
/// je Versatz und Würfel.
// ponytail: unbedingte Suche je leerem Würfel. Erst nötig, wenn eine Welt
// mit Feuer das Budget sprengt; dann eine Bitmaske je Section.
fn anything_foreign(
    chunks: &mut ChunkCache,
    sprites: &SpriteSet,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool> {
    for &cell in sprites.foreign_cells() {
        let anchor = anchor_of([x, y, z], cell);
        if let Some(id) = chunks.sprite_at(sprites, anchor[0], anchor[1], anchor[2])?
            && sprites.part(id, cell).is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Alle Chunks, deren Blöcke in das Rechteck fallen können.
///
/// Der Bereich ist ein schmales diagonales Band, kein Rechteck in x und z.
/// Wer stattdessen die Hüllbox nimmt, lädt für einen 1024er Ausschnitt rund
/// das Sechzehnfache an Chunks.
pub fn chunks_for(
    projection: Projection,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> BTreeSet<(i32, i32)> {
    let mut out = BTreeSet::new();
    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            out.insert((x >> 4, z >> 4));
        }
    }
    out
}

/// Alle Blockspalten, deren Sprite auf dieser Höhe in das Rechteck fallen
/// kann — in Zeichenreihenfolge.
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
    // f64, weil rect und Weltkoordinaten bis knapp 30 Millionen gehen:
    // siehe Projection::project_block.
    let scale = projection.scale() as f64;
    let bleed = BLEED_BLOCKS as f64 * scale;

    // screen_x = u * scale/2
    let u_min = ((rect.x as f64 - bleed) / (scale / 2.0)).floor() as i32;
    let u_max = ((rect.right() as f64 + bleed) / (scale / 2.0)).ceil() as i32;

    // screen_y = v * scale/4 - y * scale/2
    let offset = y as f64 * scale / 2.0;
    let v_min = ((rect.y as f64 - bleed + offset) / (scale / 4.0)).floor() as i32;
    let v_max = ((rect.bottom() as f64 + bleed + offset) / (scale / 4.0)).ceil() as i32;

    (v_min..=v_max).flat_map(move |v| {
        // x und z sind ganzzahlig, also haben u und v dieselbe Parität.
        let start = u_min + (u_min - v).rem_euclid(2);
        (start..=u_max)
            .step_by(2)
            .map(move |u| ((u + v) / 2, (v - u) / 2))
    })
}

/// Ein Würfel ist unsichtbar, wenn seine drei kamerazugewandten Nachbarn
/// volle, deckende Blöcke sind: deren Umrisse setzen genau den eigenen
/// zusammen.
///
/// Das gilt für alles, was in diesem Würfel liegt — auch für Teile fremder
/// Modelle, denn die Zerlegung in `SpriteSet` hält jeden Teil in seinem
/// Würfel. Wo sie das nicht schafft, meldet `is_contained` es, und die
/// Abkürzung entfällt.
fn is_hidden(
    chunks: &mut ChunkCache,
    sprites: &SpriteSet,
    own: Option<SpriteId>,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool> {
    if own.is_some_and(|id| !sprites.is_contained(id)) {
        return Ok(false);
    }
    for (dx, dy, dz) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
        match chunks.sprite_at(sprites, x + dx, y + dy, z + dz)? {
            Some(id) if sprites.is_opaque(id) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn blit(
    canvas: &mut RgbaImage,
    sprite: &Sprite,
    rect: ScreenRect,
    projection: Projection,
    [x, y, z]: [i32; 3],
) {
    let (sx, sy) = projection.project_block([x, y, z]);
    let origin_x = sx.round() as i32 + sprite.offset.0 - rect.x;
    let origin_y = sy.round() as i32 + sprite.offset.1 - rect.y;

    for (px, py, pixel) in sprite.image.enumerate_pixels() {
        if pixel.0[3] == 0 {
            continue;
        }
        let tx = origin_x + px as i32;
        let ty = origin_y + py as i32;
        if tx < 0 || ty < 0 || tx >= canvas.width() as i32 || ty >= canvas.height() as i32 {
            continue;
        }
        let under = canvas.get_pixel(tx as u32, ty as u32).0;
        canvas.put_pixel(tx as u32, ty as u32, Rgba(over(pixel.0, under)));
    }
}

fn over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    if src[3] == 255 {
        return src;
    }
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let value = (src[c] as f32 * sa + dst[c] as f32 * da * (1.0 - sa)) / out_a;
        out[c] = value.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (out_a * 255.0).round() as u8;
    out
}

/// Chunks, die während eines Renderlaufs gebraucht werden.
///
/// Jeder Worker bekommt später seinen eigenen Cache; geteilt würde er eine
/// Sperre im Renderpfad bedeuten.
struct ChunkCache<'a> {
    world: &'a World,
    chunks: HashMap<(i32, i32), Option<Chunk>>,
}

impl<'a> ChunkCache<'a> {
    fn new(world: &'a World) -> ChunkCache<'a> {
        ChunkCache {
            world,
            chunks: HashMap::new(),
        }
    }

    /// Sprite an einer Weltkoordinate, oder `None` für Luft, fehlende
    /// Chunks und Blöcke ohne sichtbare Geometrie.
    // ponytail: schlägt je Block in der Hashtabelle nach. Schneller wäre,
    // beim Laden eines Chunks je Section einmal Palettenindex -> SpriteId
    // abzulegen. Lohnt sich, sobald der Vollrender misst.
    fn sprite_at(
        &mut self,
        sprites: &SpriteSet,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<Option<SpriteId>> {
        let key = (x >> 4, z >> 4);
        if !self.chunks.contains_key(&key) {
            let chunk = self.world.chunk(key.0, key.1)?;
            self.chunks.insert(key, chunk);
        }
        let Some(chunk) = self.chunks[&key].as_ref() else {
            return Ok(None);
        };
        Ok(chunk.block_at(x, y, z).and_then(|block| sprites.id(block)))
    }
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

    /// Die Reihenfolge ist Teil des Vertrags: der Maleralgorithmus
    /// verlässt sich darauf, dass die Tiefe `v = x + z` innerhalb einer
    /// Höhe nie fällt.
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
