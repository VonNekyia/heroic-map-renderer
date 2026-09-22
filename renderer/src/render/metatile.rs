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
///
/// Zwei Durchgänge. Der erste sammelt die Kandidaten — Blöcke, von denen
/// etwas zu sehen sein kann — aus den Bitmasken der Sections, ohne einen
/// einzigen Luftblock anzufassen. Der zweite sortiert sie in die
/// Zeichenreihenfolge und zeichnet. Das Ergebnis ist dasselbe wie beim
/// Ablaufen aller Blöcke im Band; nur die Reihenfolge, in der die Blöcke
/// *gefunden* werden, ist eine andere.
pub fn render_area_with(
    chunks: &mut ChunkCache,
    rect: ScreenRect,
    y_range: (i32, i32),
) -> Result<RgbaImage> {
    chunks.next_tile();
    let sprites = chunks.sprites;
    let projection = sprites.projection();
    let mut canvas = RgbaImage::new(rect.width, rect.height);
    let foreign: Vec<Cell> = sprites.foreign_cells().iter().copied().collect();

    let mut candidates = chunks.candidates(rect, y_range, &foreign)?;
    candidates.sort_unstable_by_key(|c| c.key);

    for c in &candidates {
        let (id, cell, anchor) = if c.kind == 0 {
            (chunks.sprite_at(c.x, c.y, c.z)?, OWN_CELL, [c.x, c.y, c.z])
        } else {
            let cell = foreign[c.kind as usize - 1];
            let anchor = anchor_of([c.x, c.y, c.z], cell);
            (
                chunks.sprite_at(anchor[0], anchor[1], anchor[2])?,
                cell,
                anchor,
            )
        };
        if let Some(id) = id
            && let Some(part) = sprites.part(id, cell)
        {
            blit(&mut canvas, part, rect, projection, anchor);
        }
    }

    Ok(canvas)
}

/// Der Block, dessen Modell in `cell` hineinragen würde.
fn anchor_of([x, y, z]: [i32; 3], cell: Cell) -> [i32; 3] {
    [x - cell[0], y - cell[1], z - cell[2]]
}

/// Ein Block, von dem etwas zu sehen sein kann, mit seinem Platz in der
/// Zeichenreihenfolge.
struct Candidate {
    /// `(y, v, u, kind)` in einem Wort, damit das Sortieren billig ist.
    key: u64,
    x: i32,
    y: i32,
    z: i32,
    /// 0: der Block selbst; sonst 1 + Index des fremden Würfels, in den
    /// ein Nachbarmodell hineinragt.
    kind: u8,
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
    /// Je Section ihre Bitmasken.
    masks: Vec<Masks>,
    /// Je Section die Kandidaten, sobald einmal berechnet — dafür müssen
    /// die Nachbarchunks da sein, deshalb nicht beim Laden.
    exposed: Vec<Option<Box<Exposed>>>,
}

/// Bitmasken einer Section: je Spalte `z * 16 + x` ein Wort, Bit `y`.
///
/// Damit ist die Frage "ist dieser Block von seinen drei Nachbarn
/// verdeckt?" für sechzehn Blöcke einer Spalte auf einmal ein paar
/// Wortoperationen — statt drei Nachschläge je Block, für neun von zehn
/// Blöcken, die dann doch unter der Oberfläche liegen.
struct Masks {
    /// Irgendeine Familie: der Block könnte etwas zeichnen.
    present: [u16; 256],
    /// Deckende Familie: verdeckt, was hinter ihr liegt.
    solid: [u16; 256],
    /// Volles, reines Wasser: verdeckt nur gleiches Wasser.
    water: [u16; 256],
    /// Familie, die nicht in ihrem Würfel bleibt: nie überspringen.
    loose: [u16; 256],
    /// Familie mit Teilen in Nachbarwürfeln.
    foreign: [u16; 256],
    /// Steht überhaupt etwas in der Section?
    any: bool,
}

/// Was in einer Section gezeichnet werden muss.
struct Exposed {
    /// Blöcke, von denen etwas zu sehen sein kann.
    own: [u16; 256],
    /// Würfel, deren drei kamerazugewandte Nachbarn deckend sind. Ein
    /// fremdes Modellteil in so einem Würfel wäre unsichtbar.
    hidden: [u16; 256],
}

impl Masks {
    fn of(section: &crate::world::Section, families: &[Option<u32>], sprites: &SpriteSet) -> Masks {
        let mut m = Masks {
            present: [0; 256],
            solid: [0; 256],
            water: [0; 256],
            loose: [0; 256],
            foreign: [0; 256],
            any: false,
        };
        // Je Paletteneintrag ein Bitfeld: 1 vorhanden, 2 deckend, 4 Wasser,
        // 8 nicht im Würfel, 16 mit fremden Teilen.
        let flags: Vec<u8> = families
            .iter()
            .map(|family| {
                family.map_or(0, |index| {
                    let f = sprites.family(index);
                    1 | (f.opaque as u8) << 1
                        | (f.water as u8) << 2
                        | (!f.contained as u8) << 3
                        | (f.foreign as u8) << 4
                })
            })
            .collect();
        let blocks = section.blocks();
        let mut set = |col: usize, bit: u16, flag: u8| {
            m.present[col] |= bit;
            if flag & 2 != 0 {
                m.solid[col] |= bit;
            }
            if flag & 4 != 0 {
                m.water[col] |= bit;
            }
            if flag & 8 != 0 {
                m.loose[col] |= bit;
            }
            if flag & 16 != 0 {
                m.foreign[col] |= bit;
            }
        };
        if blocks.is_uniform() {
            let flag = flags.first().copied().unwrap_or(0);
            if flag != 0 {
                for col in 0..256 {
                    set(col, u16::MAX, flag);
                }
                m.any = true;
            }
            return m;
        }
        let mut any = false;
        blocks.for_each_index(4096, |i, index| {
            // Ein Index über die Palette hinaus wäre ein kaputter Chunk;
            // der zählt wie Luft, genau wie beim Nachschlagen je Block.
            let flag = flags.get(index).copied().unwrap_or(0);
            if flag != 0 {
                set(i & 255, 1 << (i >> 8), flag);
                any = true;
            }
        });
        m.any = any;
        m
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
        let sprites = self.sprites;
        let loaded = chunk.map(|chunk| {
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
            let masks = chunk
                .sections()
                .iter()
                .zip(&families)
                .map(|(section, families)| Masks::of(section, families, sprites))
                .collect();
            let exposed = chunk.sections().iter().map(|_| None).collect();
            Loaded {
                chunk,
                families,
                masks,
                exposed,
            }
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

    /// Randspalten einer Nachbarsection: `(deckend, Wasser)` je Spalte am
    /// Rand `x = 0` (Index z) oder `z = 0` (Index x). Ohne Chunk oder
    /// Section ist das Luft.
    fn edge(&mut self, key: (i32, i32), section_y: i8, x_edge: bool) -> Result<[(u16, u16); 16]> {
        let i = self.slot(key)?;
        let mut out = [(0, 0); 16];
        if let Some(loaded) = &self.slots[i].loaded
            && let Some(s) = loaded.chunk.section_index(section_y)
        {
            let m = &loaded.masks[s];
            for (j, edge) in out.iter_mut().enumerate() {
                let col = if x_edge { j * 16 } else { j };
                *edge = (m.solid[col], m.water[col]);
            }
        }
        Ok(out)
    }

    /// Rechnet die Kandidaten einer Section aus, falls noch nicht geschehen.
    ///
    /// Ein Block ist verdeckt, wenn seine Nachbarn nach +x, +y und +z
    /// deckend sind. Für volles Wasser zählt auch gleiches Wasser als
    /// Deckung: seine Flächen dorthin entfallen ohnehin, und was an
    /// deckende Blöcke grenzt, übermalen diese danach. Nach +y ist das
    /// Bit des Nachbarn in derselben Spalte, eins höher — ein Shift; am
    /// oberen Rand kommt es aus der Section darüber, an den Rändern +x
    /// und +z aus dem Nachbarchunk.
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
        let above = loaded
            .chunk
            .section_index(section_y.saturating_add(1))
            .map(|i| &loaded.masks[i]);
        let m = &loaded.masks[s];
        let mut ex = Exposed {
            own: [0; 256],
            hidden: [0; 256],
        };
        for col in 0..256 {
            let (x, z) = (col & 15, col >> 4);
            let up = above.map_or((0, 0), |a| (a.solid[col] & 1, a.water[col] & 1));
            let (sx, wx) = if x < 15 {
                (m.solid[col + 1], m.water[col + 1])
            } else {
                nx[z]
            };
            let (sz, wz) = if z < 15 {
                (m.solid[col + 16], m.water[col + 16])
            } else {
                nz[x]
            };
            let sy = (m.solid[col] >> 1) | (up.0 << 15);
            let hidden = sx & sy & sz;
            let wy = sy | (m.water[col] >> 1) | (up.1 << 15);
            let water_hidden = (sx | wx) & wy & (sz | wz);
            ex.hidden[col] = hidden;
            ex.own[col] = (m.present[col] & !m.water[col] & !hidden)
                | (m.water[col] & !water_hidden)
                | m.loose[col];
        }
        loaded.exposed[s] = Some(Box::new(ex));
        Ok(())
    }

    /// Ist der Würfel von seinen drei Nachbarn verdeckt — und darf man
    /// ihn deshalb übergehen? Ein Block, der nicht in seinem Würfel
    /// bleibt, darf das nie.
    fn hidden_at(&mut self, x: i32, y: i32, z: i32) -> Result<bool> {
        let slot = self.slot((x >> 4, z >> 4))?;
        let Some(loaded) = &self.slots[slot].loaded else {
            return Ok(false);
        };
        let Some(s) = i8::try_from(y >> 4)
            .ok()
            .and_then(|sy| loaded.chunk.section_index(sy))
        else {
            return Ok(false);
        };
        self.expose(slot, s)?;
        let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
        let col = ((z & 15) * 16 + (x & 15)) as usize;
        let bit = 1u16 << (y & 15);
        let ex = loaded.exposed[s].as_ref().expect("eben berechnet");
        Ok(ex.hidden[col] & !loaded.masks[s].loose[col] & bit != 0)
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
        // Ein fremdes Teil kann von einem Block ausserhalb des Bands
        // hereinragen; so weit reicht die Suche über das Band hinaus.
        let pad = foreign
            .iter()
            .map(|c| c[0].abs() + c[2].abs())
            .max()
            .unwrap_or(0);
        let key_of = |y: i32, v: i32, u: i32, kind: u8| -> u64 {
            ((y - y_range.0) as u64) << 48
                | ((v - v_lo + pad) as u64 & 0xFFFF) << 32
                | ((u - u_min + pad) as u64 & 0xFFFF) << 8
                | kind as u64
        };
        let in_band = |y: i32, v: i32, u: i32| {
            if y < y_range.0 || y > y_range.1 || u < u_min || u > u_max {
                return false;
            }
            let (v_min, v_max) = v_window(projection, rect, y);
            v >= v_min && v <= v_max
        };

        let mut out = Vec::new();
        let mut anchors: Vec<[i32; 3]> = Vec::new();
        for key in band_chunks(u_min - pad, u_max + pad, v_lo - pad, v_hi + pad) {
            let slot = self.slot(key)?;
            let sections = match &self.slots[slot].loaded {
                Some(loaded) => loaded.chunk.sections().len(),
                None => continue,
            };
            for s in 0..sections {
                if self.slots[slot].loaded.as_ref().expect("geladen").masks[s].any {
                    self.expose(slot, s)?;
                }
            }
            let loaded = self.slots[slot].loaded.as_ref().expect("geladen");
            for (s, section) in loaded.chunk.sections().iter().enumerate() {
                let m = &loaded.masks[s];
                if !m.any {
                    continue;
                }
                let ex = loaded.exposed[s].as_ref().expect("eben berechnet");
                let sy = section.y as i32 * 16;
                for col in 0..256 {
                    let own = ex.own[col];
                    let fo = m.foreign[col];
                    if own == 0 && fo == 0 {
                        continue;
                    }
                    let x = key.0 * 16 + (col & 15) as i32;
                    let z = key.1 * 16 + (col >> 4) as i32;
                    let (u, v) = (x - z, x + z);
                    let mut bits = own;
                    while bits != 0 {
                        let y = sy + bits.trailing_zeros() as i32;
                        bits &= bits - 1;
                        if in_band(y, v, u) {
                            out.push(Candidate {
                                key: key_of(y, v, u, 0),
                                x,
                                y,
                                z,
                                kind: 0,
                            });
                        }
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
        // Band liegt und nicht verdeckt ist — genau wie ein eigener Block.
        for anchor in anchors {
            for (i, cell) in foreign.iter().enumerate() {
                let [x, y, z] = [
                    anchor[0] + cell[0],
                    anchor[1] + cell[1],
                    anchor[2] + cell[2],
                ];
                let (u, v) = (x - z, x + z);
                if in_band(y, v, u) && !self.hidden_at(x, y, z)? {
                    out.push(Candidate {
                        key: key_of(y, v, u, i as u8 + 1),
                        x,
                        y,
                        z,
                        kind: i as u8 + 1,
                    });
                }
            }
        }
        Ok(out)
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
