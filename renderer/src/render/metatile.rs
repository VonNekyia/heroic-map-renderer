use std::collections::HashMap;

use anyhow::Result;
use image::{Rgba, RgbaImage};

use crate::assets::Face;
use crate::assets::fluid;
use crate::world::{Chunk, REGION, Region, World};

use super::rasterizer::over;
use super::sprites::{DEPTHS, Family, mask_bit};
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
/// Ein globaler Tiefenpuffer ist damit unnötig. `columns_at` liefert die
/// Spalten bereits in dieser Reihenfolge.
pub fn render_area(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    draw(world, sprites, rect, y_range, true)
}

/// Wie [`render_area`], aber ohne die Abkürzung über verdeckte Würfel: die
/// Referenz, gegen die Tests die Abkürzung prüfen. Sie darf kein Pixel
/// ändern.
pub fn render_area_without_culling(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    draw(world, sprites, rect, y_range, false)
}

fn draw(
    world: &World,
    sprites: &SpriteSet,
    rect: ScreenRect,
    y_range: (i32, i32),
    verdecken: bool,
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
            if own.is_empty() && !(ueberhaenge && anything_foreign(&mut chunks, sprites, x, y, z)?)
            {
                continue;
            }
            if verdecken && is_hidden(&mut chunks, sprites, own.sprite, x, y, z)? {
                continue;
            }

            if let Some(id) = own.sprite
                && let Some(part) = sprites.part(id, OWN_CELL)
            {
                blit(&mut canvas, part, rect, projection, [x, y, z]);
            }
            // Die Streifen nach dem Block: sie liegen auf seiner Grenze,
            // also vor allem, was er selbst enthält.
            for id in own.strips.into_iter().flatten() {
                if let Some(part) = sprites.part(id, OWN_CELL) {
                    blit(&mut canvas, part, rect, projection, [x, y, z]);
                }
            }
            if ueberhaenge {
                for &cell in sprites.foreign_cells() {
                    let anchor = anchor_of([x, y, z], cell);
                    let Some(id) = chunks
                        .sprite_at(sprites, anchor[0], anchor[1], anchor[2])?
                        .sprite
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
        if let Some(id) = chunks
            .sprite_at(sprites, anchor[0], anchor[1], anchor[2])?
            .sprite
            && sprites.part(id, cell).is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Was an einem Würfel zu zeichnen ist: das Sprite des Blocks, dazu die
/// Streifen seiner Flüssigkeit über niedrigeren Nachbarn.
#[derive(Default, Clone, Copy)]
struct Drawn {
    sprite: Option<SpriteId>,
    strips: [Option<SpriteId>; 2],
}

impl Drawn {
    fn is_empty(&self) -> bool {
        self.sprite.is_none() && self.strips.iter().all(Option::is_none)
    }
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
/// ihn ganz decken: deren Umrisse setzen genau den eigenen zusammen. Der
/// Ost- und der Südnachbar müssen dafür ihren ganzen Umriss deckend
/// füllen, dem Nachbarn darüber genügt sein Boden — Lava endet bei 8/9 und
/// deckt trotzdem den Block darunter. Geprüft ist beides Pixel für Pixel
/// gegen einen vollen Würfel, siehe `SpriteSet`.
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
        let deckt = |family: &Family| {
            if dy == 1 {
                family.covers_floor
            } else {
                family.opaque
            }
        };
        match chunks.family_at(sprites, x + dx, y + dy, z + dz)? {
            Some(family) if deckt(family) => {}
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

/// Chunks, die während eines Renderlaufs gebraucht werden.
///
/// Jeder Worker bekommt später seinen eigenen Cache; geteilt würde er eine
/// Sperre im Renderpfad bedeuten.
struct ChunkCache<'a> {
    world: &'a World,
    /// Offene Regionsdateien. `World::chunk` würde die Datei für jeden
    /// Chunk neu öffnen — bei rund fünfzig Chunks je Kachel sind das
    /// fünfzig Öffnungen statt einer Handvoll.
    regions: HashMap<(i32, i32), Option<Region>>,
    chunks: HashMap<(i32, i32), Option<Loaded>>,
}

/// Ein Chunk samt der Familie je Paletteneintrag. Die Blockstate wird
/// damit einmal je Section gehasht statt einmal je Block — im Renderpfad
/// war das der teuerste Schritt.
struct Loaded {
    chunk: Chunk,
    families: Vec<Vec<Option<u32>>>,
}

impl<'a> ChunkCache<'a> {
    fn new(world: &'a World) -> ChunkCache<'a> {
        ChunkCache {
            world,
            regions: HashMap::new(),
            chunks: HashMap::new(),
        }
    }

    fn load(&mut self, sprites: &SpriteSet, key: (i32, i32)) -> Result<()> {
        let region_key = (key.0.div_euclid(REGION), key.1.div_euclid(REGION));
        if !self.regions.contains_key(&region_key) {
            let region = self.world.region(region_key.0, region_key.1)?;
            self.regions.insert(region_key, region);
        }
        let chunk = match self.regions.get_mut(&region_key) {
            Some(Some(region)) => region.chunk(key.0, key.1)?,
            _ => None,
        };
        let loaded = chunk.map(|chunk| {
            let families = chunk
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
            Loaded { chunk, families }
        });
        self.chunks.insert(key, loaded);
        Ok(())
    }

    /// Was an einer Weltkoordinate zu zeichnen ist — nichts für Luft,
    /// fehlende Chunks und Blöcke ohne sichtbare Geometrie.
    ///
    /// Drei Entscheidungen fallen hier: welche Alternative die Position
    /// bekommt, welche Flüssigkeitsflächen die Nachbarn verdecken und
    /// welche Biomfassung gilt. Alles davon ist vorab gerastert.
    fn sprite_at(&mut self, sprites: &SpriteSet, x: i32, y: i32, z: i32) -> Result<Drawn> {
        let Some(family) = self.family_at(sprites, x, y, z)? else {
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
            let above = same(self.family_at(sprites, x, y + 1, z)?);
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
                let Some(other) = self.family_at(sprites, x + dx, y, z + dz)? else {
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
                    let below = if same(self.family_at(sprites, x + dx, y + 1, z + dz)?) {
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
            // Zählung — der Grund, eine Platte, das Ufer —, gemessen an der
            // Höhe dieser Oberfläche. Dünne Modelle im Wasser lässt er
            // durch: Seegras oder ein Pfosten machen die Fläche nicht flach.
            let mut depth = 0;
            while !above && depth + 1 < DEPTHS {
                let d = 1 + depth as i32;
                let behind = self.family_at(sprites, x - d, y - d, z - d)?;
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
        let chunk = &self.chunks[&(x >> 4, z >> 4)]
            .as_ref()
            .expect("eben geladen")
            .chunk;
        let tint = |id: SpriteId| sprites.in_biome(id, || chunk.biome_at(x, y, z));
        Ok(Drawn {
            sprite: sprite.map(tint),
            strips: strips.map(|strip| strip.map(tint)),
        })
    }

    /// Die Familie des Blocks an einer Weltkoordinate — ein Nachschlag im
    /// Chunk-Cache und zwei Indizes, ohne die Blockstate zu hashen.
    fn family_at<'s>(
        &mut self,
        sprites: &'s SpriteSet,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<Option<&'s Family>> {
        let key = (x >> 4, z >> 4);
        // Ein Hash je Nachschlag, nicht zwei: im Renderpfad fragt jeder
        // Wasserblock bis zu acht Nachbarn, und der Schlüssel ist fast immer
        // schon da.
        if let Some(loaded) = self.chunks.get(&key) {
            return Ok(Self::lookup(loaded.as_ref(), sprites, x, y, z));
        }
        self.load(sprites, key)?;
        Ok(Self::lookup(self.chunks[&key].as_ref(), sprites, x, y, z))
    }

    fn lookup<'s>(
        loaded: Option<&Loaded>,
        sprites: &'s SpriteSet,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<&'s Family> {
        let loaded = loaded?;
        let (section, slot) = loaded.chunk.slot(x, y, z)?;
        loaded
            .families
            .get(section)
            .and_then(|families| families.get(slot))
            .copied()
            .flatten()
            .map(|index| sprites.family(index))
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
