use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use image::{Rgba, RgbaImage};

use crate::assets::Face;
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
    render_area_with(&mut ChunkCache::new(world, sprites), rect, y_range)
}

/// Wie [`render_area`], mit einem Cache, der über Kacheln hinweg lebt.
///
/// Aufeinanderfolgende Kacheln liegen untereinander und teilen sich fast
/// alle Chunks. Wer sie je Kachel neu lädt, gibt ein Drittel der Renderzeit
/// fürs Dekodieren aus, das er gerade erst gemacht hat.
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    chunks.next_tile();
    let sprites = chunks.sprites;
    let projection = sprites.projection();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    // Fast immer leer. Dann fällt die Suche nach Überhängen ganz weg.
    let ueberhaenge = !sprites.foreign_cells().is_empty();

    for y in y_range.0..=y_range.1 {
        for (x, z) in columns_at(projection, rect, y) {
            // Erst die Familie, ein Nachschlag. Das Sprite erst, wenn der
            // Block sichtbar ist: neun von zehn Blöcken liegen unter der
            // Oberfläche, und für die würde die Sprite-Wahl — Alternative
            // würfeln, Wasserflächen, Biom — ins Leere laufen.
            let family = chunks.family_at(x, y, z)?;
            if family.is_none() && !(ueberhaenge && anything_foreign(chunks, x, y, z)?) {
                continue;
            }
            if is_hidden(chunks, family, x, y, z)? {
                continue;
            }

            if let Some(id) = chunks.sprite_at(x, y, z)?
                && let Some(part) = sprites.part(id, OWN_CELL)
            {
                blit(&mut canvas, part, rect, projection, [x, y, z]);
            }
            if ueberhaenge {
                for &cell in sprites.foreign_cells() {
                    let anchor = anchor_of([x, y, z], cell);
                    let Some(id) = chunks.sprite_at(anchor[0], anchor[1], anchor[2])? else {
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
fn anything_foreign(chunks: &mut ChunkCache, x: i32, y: i32, z: i32) -> Result<bool> {
    let sprites = chunks.sprites;
    for &cell in sprites.foreign_cells() {
        let anchor = anchor_of([x, y, z], cell);
        if let Some(id) = chunks.sprite_at(anchor[0], anchor[1], anchor[2])?
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
    own: Option<&Family>,
    x: i32,
    y: i32,
    z: i32,
) -> Result<bool> {
    if own.is_some_and(|family| !family.contained) {
        return Ok(false);
    }
    for (dx, dy, dz) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
        match chunks.family_at(x + dx, y + dy, z + dz)? {
            Some(family) if family.opaque => {}
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

/// Wie viele Chunks ein Cache höchstens hält, bevor er verwirft, was die
/// vorige Kachel nicht gebraucht hat. Eine Kachel bei scale 32 berührt gut
/// hundert Chunks; die nächste liegt direkt darunter und teilt sich fast
/// alle davon.
// ponytail: Verfallsdatum je Kachel statt echtem LRU. Reicht, solange die
// Kacheln in Leseordnung kommen; sonst lädt jede Kachel ihre hundert neu.
const CACHE_CHUNKS: usize = 256;

/// Chunks, die während eines Renderlaufs gebraucht werden.
///
/// Ein Cache gehört zu einer Sprite-Tabelle: er hält je Paletteneintrag
/// den Familienindex daraus. Über Kacheln hinweg lebt er je Worker —
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
        let sprites = self.sprites;
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
        self.slots.push(Slot {
            key,
            loaded,
            used: self.tile,
        });
        let i = self.slots.len() - 1;
        self.index.insert(key, i);
        Ok(i)
    }

    /// Sprite an einer Weltkoordinate, oder `None` für Luft, fehlende
    /// Chunks und Blöcke ohne sichtbare Geometrie.
    ///
    /// Drei Entscheidungen fallen hier: welche Alternative die Position
    /// bekommt, welche Flüssigkeitsflächen die Nachbarn verdecken und
    /// welche Biomfassung gilt. Alles davon ist vorab gerastert.
    fn sprite_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<SpriteId>> {
        let sprites = self.sprites;
        let Some(family) = self.family_at(x, y, z)? else {
            return Ok(None);
        };
        let Some(mut id) = family.pick([x, y, z]) else {
            return Ok(None);
        };

        // Flächen zu einem Nachbarn mit derselben Flüssigkeit entfallen.
        // Sonst mischt sich jede innere Fläche eines Beckens mit dazu, und
        // ein Ozean wäre ein Raster aus doppelt gedecktem Wasser. Seitlich
        // nur, wenn der Nachbar mindestens so hoch steht; darüber verdeckt
        // jede Flüssigkeit die eigene Oberfläche.
        if let Some((fluid, height)) = family.fluid {
            let mut mask = 0u8;
            for (bit, [dx, dy, dz]) in [[1, 0, 0], [0, 1, 0], [0, 0, 1]].into_iter().enumerate() {
                if let Some(other) = self.family_at(x + dx, y + dy, z + dz)?
                    && let Some((other_fluid, other_height)) = other.fluid
                    && other_fluid == fluid
                    && (dy == 1 || other_height >= height)
                {
                    mask |= 1 << bit;
                }
            }
            // Die Oberfläche trägt die Deckkraft aller Schichten darunter:
            // durch einen Block Wasser sieht man den Grund, durch vier nicht
            // mehr. Gezählt wird nur, wenn es eine Oberfläche gibt.
            let mut depth = 0;
            while mask & mask_bit(Face::Up) == 0
                && depth + 1 < DEPTHS
                && self
                    .family_at(x, y - 1 - depth as i32, z)?
                    .and_then(|below| below.fluid)
                    .is_some_and(|(other, _)| other == fluid)
            {
                depth += 1;
            }
            match sprites.masked(id, mask, depth) {
                Some(masked) => id = masked,
                None => return Ok(None),
            }
        }

        // Das Biom kostet einen zweiten Nachschlag; `in_biome` fragt nur
        // fuer Sprites danach, die ueberhaupt Fassungen haben.
        let i = self.slot((x >> 4, z >> 4))?;
        let chunk = &self.slots[i].loaded.as_ref().expect("eben geladen").chunk;
        Ok(Some(sprites.in_biome(id, || chunk.biome_at(x, y, z))))
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
