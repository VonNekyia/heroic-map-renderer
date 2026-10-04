//! Die Bildebene in Kacheln zerlegen und herausfinden, welche davon etwas
//! zeigen.

use std::collections::{BTreeSet, HashSet};

use std::ffi::c_int;
use std::sync::{Arc, Once};

use anyhow::{Result, anyhow, ensure};
use image::RgbaImage;
use libwebp_sys as webp;
use rayon::prelude::*;

use crate::world::{BlockState, Blockdaten, Chunk, REGION, World};

use super::heights::{Heights, RegionHeights};
use super::look::Look;
use super::stand::{Aenderung, Inhalt};
use super::{BLEED_BLOCKS, Projection, ScreenRect};

/// Kantenlänge einer Kachel in Pixeln. 256 ist, was Leaflet ohne
/// Zusatzeinstellung erwartet.
pub const TILE: u32 = 256;

/// Blöcke je Chunkkante.
const CHUNK: i32 = 16;

/// Eine Kachel der Bildebene.
///
/// Die Koordinaten dürfen negativ sein: der Blockursprung liegt mitten in
/// der Welt und nicht an ihrem Rand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId {
    pub x: i32,
    pub y: i32,
}

impl TileId {
    pub fn rect(&self) -> ScreenRect {
        ScreenRect {
            x: self.x * TILE as i32,
            y: self.y * TILE as i32,
            width: TILE,
            height: TILE,
        }
    }
}

/// Alle Kacheln, die ein Bildrechteck berühren.
pub fn covering(rect: ScreenRect) -> impl Iterator<Item = TileId> {
    raster(rect, TILE)
}

/// Alle Quadrate der Kantenlänge `kante` in Pixeln, die ein Bildrechteck
/// berühren, in ihren eigenen Koordinaten.
fn raster(rect: ScreenRect, kante: u32) -> impl Iterator<Item = TileId> {
    let kante = kante as i32;
    let x0 = rect.x.div_euclid(kante);
    let x1 = (rect.right() - 1).div_euclid(kante);
    let y0 = rect.y.div_euclid(kante);
    let y1 = (rect.bottom() - 1).div_euclid(kante);
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| TileId { x, y }))
}

/// Die Fläche eines Laufs: ganze Kacheln der Stufe, die `stufen` Stufen
/// über der Basis liegt, Quadrate von `TILE << stufen` Pixeln der Basis.
/// Ein Ausschnitt ist ein Rechteck daraus, ein Update viele Stücke.
/// Siehe docs/benutzung/kacheln.md, „Ein Ausschnitt“.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gebiet {
    stufen: u32,
    /// Auf der Stufe `stufen`, nach Spalte, dann Zeile sortiert.
    kacheln: Arc<BTreeSet<TileId>>,
}

impl Gebiet {
    /// Die Kacheln der Stufe `stufen`, die das Rechteck berühren: das
    /// Rechteck aufgerundet auf ihr Raster.
    pub fn rechteck(rect: ScreenRect, stufen: u32) -> Gebiet {
        let kacheln = if rect.width == 0 || rect.height == 0 {
            BTreeSet::new()
        } else {
            raster(rect, TILE << stufen).collect()
        };
        Gebiet::aus(stufen, kacheln)
    }

    /// Diese Kacheln der Stufe `stufen`.
    pub fn aus(stufen: u32, kacheln: BTreeSet<TileId>) -> Gebiet {
        Gebiet {
            stufen,
            kacheln: Arc::new(kacheln),
        }
    }

    pub fn stufen(&self) -> u32 {
        self.stufen
    }

    /// Die Kacheln auf der Stufe [`Gebiet::stufen`].
    pub fn kacheln(&self) -> &BTreeSet<TileId> {
        &self.kacheln
    }

    /// Ob ein Rechteck in Pixeln der Basis eine Kachel des Gebiets schneidet.
    pub fn beruehrt(&self, rect: ScreenRect) -> bool {
        let (Some(erste), Some(letzte)) = (self.kacheln.first(), self.kacheln.last()) else {
            return false;
        };
        if rect.width == 0 || rect.height == 0 {
            return false;
        }
        let kante = (TILE << self.stufen) as i32;
        let x0 = rect.x.div_euclid(kante).max(erste.x);
        let x1 = (rect.right() - 1).div_euclid(kante).min(letzte.x);
        let y0 = rect.y.div_euclid(kante);
        let y1 = (rect.bottom() - 1).div_euclid(kante);
        (x0..=x1).any(|x| {
            self.kacheln
                .range(TileId { x, y: y0 }..=TileId { x, y: y1 })
                .next()
                .is_some()
        })
    }

    /// Ob eine Basiskachel im Gebiet liegt.
    pub fn enthaelt(&self, tile: TileId) -> bool {
        self.flaeche_fein(self.stufen).enthaelt(&tile)
    }

    /// Die Kacheln der Stufe `hoch` Stufen über der Basis, die etwas aus dem
    /// Gebiet zeigen. Bis zur Stufe des Gebiets liegen sie ganz darin,
    /// darüber schneiden sie es vielleicht nur an.
    pub fn flaeche(&self, hoch: u32) -> Flaeche {
        match hoch.checked_sub(self.stufen) {
            Some(d) if d > 0 => Flaeche {
                shift: 0,
                kacheln: Arc::new(
                    self.kacheln
                        .iter()
                        .map(|t| TileId {
                            x: t.x >> d,
                            y: t.y >> d,
                        })
                        .collect(),
                ),
            },
            _ => self.flaeche_fein(self.stufen - hoch),
        }
    }

    /// Die Fläche `shift` Stufen unter der des Gebiets.
    fn flaeche_fein(&self, shift: u32) -> Flaeche {
        Flaeche {
            shift,
            kacheln: Arc::clone(&self.kacheln),
        }
    }
}

/// Die Kacheln einer Stufe, die ein [`Gebiet`] berührt, siehe
/// [`Gebiet::flaeche`].
#[derive(Clone, Debug)]
pub struct Flaeche {
    /// Um so viele Stufen ist die Stufe feiner als `kacheln`.
    shift: u32,
    kacheln: Arc<BTreeSet<TileId>>,
}

impl Flaeche {
    pub fn enthaelt(&self, tile: &TileId) -> bool {
        self.kacheln.contains(&TileId {
            x: tile.x >> self.shift,
            y: tile.y >> self.shift,
        })
    }

    /// Die Spalten der Stufe, in denen eine Kachel der Fläche liegen kann,
    /// aufsteigend.
    pub fn spalten(&self) -> Vec<i32> {
        let mut grob: Vec<i32> = self.kacheln.iter().map(|t| t.x).collect();
        grob.dedup();
        let breite = 1i32 << self.shift;
        grob.into_iter()
            .flat_map(|x| (x << self.shift)..(x << self.shift) + breite)
            .collect()
    }
}

/// Rundet ein Rechteck auf ganze Kacheln auf.
///
/// Ausgegeben werden immer vollständige Kacheln. Wer den Vorlauf auf den
/// angeforderten Ausschnitt begrenzt, lässt Blöcke weg, die in derselben
/// Kachel liegen — und die fehlen dann stillschweigend im Bild.
pub fn snap_to_tiles(rect: ScreenRect) -> ScreenRect {
    snap_to_grid(rect, TILE)
}

/// Rundet ein Rechteck nach aussen auf ein Raster der Kantenlänge `edge`
/// — auf ganze Kacheln einer gröberen Stufe, wenn die nativ aus der Welt
/// gerendert wird: dann muss auch die Basis so weit reichen, sonst zeigen
/// die Stufen verschiedene Weltstände.
pub fn snap_to_grid(rect: ScreenRect, edge: u32) -> ScreenRect {
    if rect.width == 0 || rect.height == 0 {
        return rect;
    }
    let edge = edge as i32;
    let x = rect.x.div_euclid(edge) * edge;
    let y = rect.y.div_euclid(edge) * edge;
    let right = (rect.right() - 1).div_euclid(edge) * edge + edge;
    let bottom = (rect.bottom() - 1).div_euclid(edge) * edge + edge;
    ScreenRect {
        x,
        y,
        width: (right - x) as u32,
        height: (bottom - y) as u32,
    }
}

/// Die vier Eckkacheln eines Bildbereichs.
///
/// Für die Tiefe der Zoompyramide genügen sie: Halbieren ist monoton, also
/// entscheiden allein die Extreme, wann es nichts mehr zusammenfasst.
pub fn corner_tiles(rect: ScreenRect) -> BTreeSet<TileId> {
    let tile = TILE as i32;
    let x0 = rect.x.div_euclid(tile);
    let x1 = (rect.right() - 1).div_euclid(tile);
    let y0 = rect.y.div_euclid(tile);
    let y1 = (rect.bottom() - 1).div_euclid(tile);
    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
        .into_iter()
        .map(|(x, y)| TileId { x, y })
        .collect()
}

/// Bildbereich, in dem die ganze Welt liegen kann; mit einem Bereich
/// ([`World::mit_bereich`]) der, in dem er liegt.
///
/// Gelesen werden nur die Regionsdateinamen, kein einziger Chunk. Das
/// reicht für die Zoomstufen: die Nummerierung darf nicht davon abhängen,
/// welchen Ausschnitt gerade jemand exportiert, sonst passen zwei Läufe
/// derselben Welt nicht zusammen.
pub fn world_box(
    world: &World,
    projection: Projection,
    y_range: (i32, i32),
) -> Result<Option<ScreenRect>> {
    let regionen = world.regions()?;
    if let Some([x0, z0, x1, z1]) = world.bereich() {
        if regionen.is_empty() {
            return Ok(None);
        }
        // Die Spalten eines Rechtecks liegen im Kasten seiner Eckchunks.
        let ecke =
            |cx: i32, cz: i32| column_box(projection, cx * CHUNK, cz * CHUNK, y_range, CHUNK);
        let ecken = [
            ecke(x0, z0),
            ecke(x1 - 1, z0),
            ecke(x0, z1 - 1),
            ecke(x1 - 1, z1 - 1),
        ];
        return Ok(ecken.into_iter().reduce(union));
    }
    let kante = REGION * CHUNK;
    let mut ganz: Option<ScreenRect> = None;
    for (rx, rz) in regionen {
        let rect = column_box(projection, rx * kante, rz * kante, y_range, kante);
        ganz = Some(match ganz {
            None => rect,
            Some(bisher) => union(bisher, rect),
        });
    }
    Ok(ganz)
}

fn union(a: ScreenRect, b: ScreenRect) -> ScreenRect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    ScreenRect {
        x,
        y,
        width: (a.right().max(b.right()) - x) as u32,
        height: (a.bottom().max(b.bottom()) - y) as u32,
    }
}

/// Was ein Kachellauf vorher wissen muss.
#[derive(Default)]
pub struct Survey {
    /// Kacheln, in denen etwas liegen kann.
    pub tiles: Vec<TileId>,
    /// Blockstates, die vorkommen. Daraus entsteht die Sprite-Tabelle.
    pub states: BTreeSet<BlockState>,
    /// Blockstates mit den Daten ihres Blockentity, wo diese das Bild
    /// ändern können: Banner mit Mustern, Krüge mit Scherben.
    pub entities: BTreeSet<(BlockState, Blockdaten)>,
    /// Biome, die vorkommen — um zu melden, welche keine Definition haben.
    pub biomes: BTreeSet<String>,
    /// Chunks, die gelesen wurden.
    pub chunks: usize,
    /// Die Höhen jeder Region, von der der Lauf Chunks liest, siehe
    /// [`super::heights`].
    // ponytail: hält die Höhen aller Regionen bis zum Schreiben, 32 KiB je
    // Region, bei 2500 Regionen 80 MiB. Wird das zu viel, gepackt halten
    // (ein Sechstel) oder jede Region schon im Vorlauf schreiben.
    pub heights: Vec<RegionHeights>,
    /// Chunks, die der Lauf läse, die aber nicht fertig erzeugt sind:
    /// gezeichnet wird nur, was [`crate::world::Region::chunk`] liefert.
    /// Ihre Höhen bleiben leer.
    pub unfinished: usize,
    /// Mit [`Reach::mit_inhalt`] je gelesenem Chunk, was der Renderer aus
    /// ihm zeichnet, für den Stand eines vollen Laufs.
    pub inhalte: Vec<([i32; 2], Inhalt)>,
}

/// Welche Chunks der Vorlauf liest: die, deren Spalte über die ganze
/// Welthöhe das Gebiet berührt; ohne Gebiet alle. Mit Cinematic
/// dazu die, aus denen ein Strahl zur Sonne liest, siehe
/// [`Reach::mit_sonne`].
#[derive(Clone)]
pub struct Reach {
    projection: Projection,
    hoehe: (i32, i32),
    gebiet: Option<Gebiet>,
    /// Um wie viele Chunks im Blick ein Strahl zur Sonne von einem Chunk
    /// im Ausschnitt aus Blöcke liest: `[von, bis]` je `[x, z]`.
    sonne: Option<[[i32; 2]; 2]>,
    /// Ob der Vorlauf je Chunk seinen [`Inhalt`] sammelt.
    inhalt: bool,
}

impl Reach {
    /// Über die Basiskacheln, die `bounds` berührt; ohne alle.
    pub fn new(projection: Projection, y_range: (i32, i32), bounds: Option<ScreenRect>) -> Reach {
        // Auf ganze Kacheln runden, bevor irgendetwas ausgeschlossen wird:
        // gerendert wird die ganze Kachel, also muss auch der Vorlauf sie
        // ganz abdecken.
        Reach::im_gebiet(projection, y_range, bounds.map(|b| Gebiet::rechteck(b, 0)))
    }

    /// Über die Kacheln des Gebiets; ohne alle.
    pub fn im_gebiet(projection: Projection, y_range: (i32, i32), gebiet: Option<Gebiet>) -> Reach {
        Reach {
            projection,
            // Bis zur Oberkante des obersten Blocks, wie der genaue Kasten
            // je Chunk im Vorlauf.
            hoehe: (y_range.0, y_range.1 + 1),
            gebiet,
            sonne: None,
            inhalt: false,
        }
    }

    /// Der Vorlauf sammelt je Chunk im Gebiet seinen [`Inhalt`], siehe
    /// [`Survey::inhalte`].
    pub fn mit_inhalt(self) -> Reach {
        Reach {
            inhalt: true,
            ..self
        }
    }

    /// Mit `look` liest der Vorlauf auch die Chunks, durch die ein Strahl
    /// zur Sonne aus dem Ausschnitt läuft, samt zwei Chunks rundum: Ein
    /// Modell am Rand beginnt den Strahl im Chunk daneben, und in jeden
    /// Chunk auf dem Weg ragen Modelle aus seinen Nachbarn. Ihre
    /// Blockstates kennt dann die Sprite-Tabelle, sonst wären sie Luft.
    /// Siehe docs/renderer/cinematic.md, „Der Vorlauf“.
    pub fn mit_sonne(self, look: Option<&Look>) -> Reach {
        let (Some(look), Some(_)) = (look, &self.gebiet) else {
            return self;
        };
        Reach {
            sonne: Some(zur_sonne(self.projection, look)),
            ..self
        }
    }

    /// Ob der Lauf Chunks der Region (rx, rz) liest.
    pub fn region(&self, rx: i32, rz: i32) -> bool {
        let kante = REGION * CHUNK;
        // Mit der Sonne rundum so weit, wie ein Strahl Chunks entfernt
        // liest; welche davon, entscheidet `Reach::zur_sonne`.
        let rand = self.sonne.map_or(0, |[von, bis]| {
            CHUNK * von.iter().chain(&bis).map(|c| c.abs()).max().unwrap_or(0)
        });
        self.column(rx * kante - rand, rz * kante - rand, kante + 2 * rand)
    }

    /// Ob der Lauf den Chunk (cx, cz) liest.
    pub fn chunk(&self, cx: i32, cz: i32) -> bool {
        self.column(cx * CHUNK, cz * CHUNK, CHUNK)
    }

    /// Ob der Lauf Blöcke des Chunks (cx, cz) liest, auch für einen Strahl
    /// zur Sonne.
    pub fn liest(&self, cx: i32, cz: i32) -> bool {
        self.chunk(cx, cz) || self.zur_sonne(cx, cz)
    }

    fn column(&self, x: i32, z: i32, kante: i32) -> bool {
        self.gebiet
            .as_ref()
            .is_none_or(|g| g.beruehrt(column_box(self.projection, x, z, self.hoehe, kante)))
    }

    /// Ob der Lauf diese Basiskachel ausgibt.
    fn zeigt(&self, tile: TileId) -> bool {
        self.gebiet.as_ref().is_none_or(|g| g.enthaelt(tile))
    }

    /// Ob ein Strahl zur Sonne aus einem Chunk, den der Lauf liest, Blöcke
    /// im Chunk (cx, cz) liest; ohne [`Reach::mit_sonne`] nie.
    fn zur_sonne(&self, cx: i32, cz: i32) -> bool {
        let Some([von, bis]) = self.sonne else {
            return false;
        };
        let richtung = self.projection.richtung();
        let [x, z] = richtung.in_den_blick([cx, cz]);
        (von[1]..=bis[1]).any(|dz| {
            (von[0]..=bis[0]).any(|dx| {
                let [cx, cz] = richtung.in_die_welt([x - dx, z - dz]);
                self.chunk(cx, cz)
            })
        })
    }

    /// Wo die Blöcke eines gelesenen Chunks landen. Nur von Chunks im
    /// Ausschnitt sammelt der Vorlauf die Blockstates; nur deren kennt die
    /// Sprite-Tabelle alle.
    fn content(&self, chunk: &Chunk) -> Content {
        // Nur Sections, in denen etwas steht. Die leeren ober- und
        // unterhalb des Geländes machen sonst jede Spalte so hoch wie die
        // ganze Welt.
        let mut belegt = chunk
            .sections()
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.y as i32 * CHUNK);
        let Some(unten) = belegt.next() else {
            return Content::Empty;
        };
        let oben = belegt.next_back().unwrap_or(unten) + CHUNK;
        let (x, z) = (chunk.x * CHUNK, chunk.z * CHUNK);
        let rect = column_box(self.projection, x, z, (unten, oben), CHUNK);
        match &self.gebiet {
            Some(gebiet) if !gebiet.beruehrt(rect) => Content::Outside,
            _ => Content::Inside(rect),
        }
    }
}

/// Um wie viele Chunks im Blick ein Strahl zur Sonne von einem Chunk aus
/// Blöcke liest, samt zwei Chunks rundum: `[von, bis]` je `[x, z]`, siehe
/// [`Reach::mit_sonne`].
fn zur_sonne(projection: Projection, look: &Look) -> [[i32; 2]; 2] {
    let d = look.sonne_im_blick(projection.kamera());
    // So viele Chunkgrenzen wie `ChunkCache::horizont`.
    let reicht = |c: f32| {
        let n = (f64::from(look.sonne_weite) * f64::from(c.abs()) / 16.0).floor() as i32 + 1;
        if c < 0.0 { -n } else { n }
    };
    let (x, z) = (reicht(d[0]), reicht(d[2]));
    [[x.min(0) - 2, z.min(0) - 2], [x.max(0) + 2, z.max(0) + 2]]
}

/// Wo ein Update zeichnet: die Kacheln der Stufe `stufen`, auf die ein
/// geänderter Chunk wirken kann. Das sind seine Blöcke und die bis zu
/// 16 Blöcke daneben, so weit reichen Licht (15) und weiche Beleuchtung (1),
/// die Mischung der Biome liegt darin; in der Höhe von `unten`, der
/// Unterkante der Dimension, bis 16 Blöcke über [`Aenderung::oben`], denn
/// Himmelslicht fällt in einer Spalte beliebig tief. Mit Cinematic dazu
/// jeder Chunk, dessen Strahlen zur Sonne ihn lesen ([`Reach::mit_sonne`]
/// umgekehrt), und der Rand des Bloom auf jeder nativen Stufe.
/// Siehe docs/benutzung/updates.md, „Wo ein Update zeichnet“.
pub fn gebiet_der_aenderungen(
    projection: Projection,
    stufen: u32,
    unten: i32,
    aenderungen: &[Aenderung],
    look: Option<&Look>,
) -> Gebiet {
    let richtung = projection.richtung();
    let [von, bis] = look.map_or([[-1, -1], [1, 1]], |look| zur_sonne(projection, look));
    let bloom = look.map_or(0, |look| {
        (0..=stufen)
            .map(|k| (3 * look.bloom_radius(projection.scale() >> k) as i32) << k)
            .max()
            .unwrap_or(0)
    });
    let mut kacheln = BTreeSet::new();
    for aenderung in aenderungen {
        // Gelesen wird ein Chunk X von jedem Chunk X − d mit d in [von, bis].
        let [x, z] = richtung.in_den_blick(aenderung.chunk);
        let a = richtung.in_die_welt([x - bis[0], z - bis[1]]);
        let b = richtung.in_die_welt([x - von[0], z - von[1]]);
        let rect = block_box(
            projection,
            [a[0].min(b[0]) * CHUNK, a[1].min(b[1]) * CHUNK],
            [(a[0].max(b[0]) + 1) * CHUNK, (a[1].max(b[1]) + 1) * CHUNK],
            // Bis zur Oberkante des obersten Blocks, wie in `Reach::new`.
            (unten, aenderung.oben + CHUNK + 1),
        );
        let rect = ScreenRect {
            x: rect.x - bloom,
            y: rect.y - bloom,
            width: rect.width + 2 * bloom as u32,
            height: rect.height + 2 * bloom as u32,
        };
        kacheln.extend(raster(rect, TILE << stufen));
    }
    Gebiet::aus(stufen, kacheln)
}

/// Was ein Chunk im Ausschnitt eines Laufs zeigt, siehe [`Reach::content`].
enum Content {
    /// Keine Section, in der etwas steht.
    Empty,
    /// Seine Blöcke landen ausserhalb des Ausschnitts.
    Outside,
    /// Seine Blöcke können in diesem Rechteck landen, samt der Reserve für
    /// überstehende Sprites; es berührt das Gebiet.
    Inside(ScreenRect),
}

/// Liest jeden Chunk einmal und sammelt ein, welche Blockstates vorkommen,
/// welche Kacheln überhaupt etwas zeigen und die Höhen je Region.
///
/// `bounds` schränkt auf einen Bildausschnitt ein; ohne Angabe ist es die
/// ganze Welt. Der Renderlauf liest die Chunks danach ein zweites Mal.
/// Siehe docs/entscheidungen/0003-vorlauf-vor-dem-rendern.md.
// ponytail: liest die Welt zweimal. Eine Blockstate-Liste neben der Welt
// spart den ersten Lauf, sobald er weh tut.
pub fn survey(
    world: &World,
    projection: Projection,
    y_range: (i32, i32),
    bounds: Option<ScreenRect>,
) -> Result<Survey> {
    survey_in(world, Reach::new(projection, y_range, bounds))
}

/// [`survey`] über die Chunks, die `reach` nennt.
pub fn survey_in(world: &World, reach: Reach) -> Result<Survey> {
    let regions = world.regions()?;
    let teile: Vec<Survey> = regions
        .par_iter()
        .filter(|&&(rx, rz)| reach.region(rx, rz))
        .map(|&(rx, rz)| survey_region(world, &reach, rx, rz))
        .collect::<Result<_>>()?;

    let mut tiles: BTreeSet<TileId> = BTreeSet::new();
    let mut survey = Survey::default();
    for teil in teile {
        tiles.extend(teil.tiles);
        survey.states.extend(teil.states);
        survey.entities.extend(teil.entities);
        survey.biomes.extend(teil.biomes);
        survey.chunks += teil.chunks;
        survey.heights.extend(teil.heights);
        survey.unfinished += teil.unfinished;
        survey.inhalte.extend(teil.inhalte);
    }
    survey.tiles = tiles.into_iter().collect();
    Ok(survey)
}

fn survey_region(world: &World, reach: &Reach, rx: i32, rz: i32) -> Result<Survey> {
    let mut survey = Survey::default();
    let Some(mut region) = world.region(rx, rz)? else {
        return Ok(survey);
    };

    let mut states: HashSet<BlockState> = HashSet::new();
    let mut biomes: HashSet<String> = HashSet::new();

    let mut tiles = BTreeSet::new();
    let mut hoehen = Heights::default();
    let mut gelesen = vec![false; (REGION * REGION) as usize];
    for local_z in 0..REGION {
        for local_x in 0..REGION {
            let (cx, cz) = (rx * REGION + local_x, rz * REGION + local_z);
            // Was den Ausschnitt über die ganze Welthöhe nicht berührt,
            // wird gar nicht erst dekodiert, ausser ein Strahl zur Sonne
            // liest es.
            let im_bild = reach.chunk(cx, cz);
            if !im_bild && !reach.zur_sonne(cx, cz) {
                continue;
            }
            if im_bild {
                gelesen[(local_z * REGION + local_x) as usize] = true;
            }
            let Some(chunk) = region.stored_chunk(cx, cz)? else {
                continue;
            };
            if reach.inhalt && im_bild {
                survey.inhalte.push(([cx, cz], Inhalt::von(Some(&chunk))));
            }
            if !chunk.is_generated() {
                survey.unfinished += usize::from(im_bild);
                continue;
            }
            survey.chunks += 1;
            // Die Höhen hängen nicht an der Sprite-Tabelle: auch ein Chunk,
            // dessen Blöcke ausserhalb landen, bekommt seine.
            if im_bild {
                hoehen.record(&chunk);
            }

            // Erst ausschliessen, dann Paletten sammeln. Sonst verlangt ein
            // kleiner Ausschnitt die Assets für jeden Block der Region, und
            // ein einziger unbekannter Block weit draussen bricht den ganzen
            // Export ab.
            match reach.content(&chunk) {
                Content::Inside(rect) => tiles.extend(covering(rect).filter(|t| reach.zeigt(*t))),
                _ if reach.zur_sonne(cx, cz) => {}
                _ => continue,
            }

            for section in chunk.sections() {
                for biome in section.biomes().palette() {
                    if !biomes.contains(biome) {
                        biomes.insert(biome.clone());
                    }
                }
                for state in section.blocks().palette() {
                    if !states.contains(state) {
                        states.insert(state.clone());
                    }
                }
            }
            for ([x, y, z], daten) in chunk.blockentities() {
                if let Some(state) = chunk.block_at(x, y, z) {
                    survey.entities.insert((state.clone(), daten.clone()));
                }
            }
        }
    }

    survey.biomes = biomes.into_iter().collect();
    survey.states = states.into_iter().collect();
    survey.tiles = tiles.into_iter().collect();
    if gelesen.contains(&true) {
        survey.heights.push(RegionHeights {
            x: rx,
            z: rz,
            heights: hoehen,
            read: gelesen,
        });
    }
    Ok(survey)
}

/// Bildrechteck, in dem eine Blockspalte der Welt mit der Kantenlänge
/// `kante` ab `(x, z)` landen kann — die Reserve für überstehende Sprites
/// eingerechnet. Ihre Ecken dreht es in den Blick.
fn column_box(
    projection: Projection,
    x: i32,
    z: i32,
    y_range: (i32, i32),
    kante: i32,
) -> ScreenRect {
    block_box(projection, [x, z], [x + kante, z + kante], y_range)
}

/// Wie [`column_box`] für die Blöcke von `von` bis vor `bis` in x und z.
fn block_box(
    projection: Projection,
    von: [i32; 2],
    bis: [i32; 2],
    y_range: (i32, i32),
) -> ScreenRect {
    let mut min = (i32::MAX, i32::MAX);
    let mut max = (i32::MIN, i32::MIN);
    let richtung = projection.richtung();
    for &cx in &[von[0], bis[0]] {
        for &cz in &[von[1], bis[1]] {
            let [bx, _, bz] = richtung.versatz_in_den_blick([cx, 0, cz]);
            for &cy in &[y_range.0, y_range.1] {
                let (sx, sy) = projection.project_block([bx, cy, bz]);
                min = (min.0.min(sx.floor() as i32), min.1.min(sy.floor() as i32));
                max = (max.0.max(sx.ceil() as i32), max.1.max(sy.ceil() as i32));
            }
        }
    }
    let bleed = BLEED_BLOCKS * projection.scale() as i32;
    ScreenRect {
        x: min.0 - bleed,
        y: min.1 - bleed,
        width: (max.0 - min.0 + 2 * bleed) as u32,
        height: (max.1 - min.1 + 2 * bleed) as u32,
    }
}

#[cfg(test)]
fn overlaps(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Kodiert ein Bild als verlustfreies WebP, mit libwebp auf Stufe 0.
/// `exact` behält die Farbe voll durchsichtiger Pixel, sonst setzt libwebp
/// sie auf 0. Eigene Threads braucht libwebp nicht, die Kacheln verteilt
/// schon `rendere`.
pub fn encode_webp(image: &RgbaImage) -> Result<Vec<u8>> {
    richte_libwebp_ein();
    kodiere(image)
}

/// Dekodiert ein WebP mit libwebp nach RGBA. Nennt sein Kopf eine andere
/// Grösse als `groesse`, ist es ein Fehler, bevor Speicher für das Bild
/// angelegt ist: Ein WebP darf bis 16 383 × 16 383 Pixel gross sein.
pub fn decode_webp(daten: &[u8], groesse: (u32, u32)) -> Result<RgbaImage> {
    richte_libwebp_ein();
    dekodiere(daten, groesse)
}

/// Das erste Kodieren und Dekodieren im Prozess läuft allein, die übrigen
/// Threads warten darauf: libwebp 1.6.0 richtet dabei seine Tabellen ein,
/// unter Windows ohne Sperre, und zwei Threads zugleich können sie
/// verderben.
/// Siehe docs/renderer/renderpfad.md, „Kodieren“.
fn richte_libwebp_ein() {
    static EINGERICHTET: Once = Once::new();
    // Scheitert es, scheitert das Bild danach mit demselben Fehler.
    EINGERICHTET.call_once(|| {
        if let Ok(daten) = kodiere(&RgbaImage::new(16, 16)) {
            drop(dekodiere(&daten, (16, 16)));
        }
    });
}

/// [`decode_webp`] ohne das Warten beim ersten Mal.
fn dekodiere(daten: &[u8], groesse: (u32, u32)) -> Result<RgbaImage> {
    let (mut breite, mut hoehe) = (0, 0);
    // SAFETY: libwebp liest `daten.len()` Bytes und schreibt zwei Zahlen.
    let erkannt =
        unsafe { webp::WebPGetInfo(daten.as_ptr(), daten.len(), &mut breite, &mut hoehe) };
    ensure!(erkannt != 0, "kein WebP");
    ensure!(
        (breite as u32, hoehe as u32) == groesse,
        "{breite} × {hoehe} Pixel statt {} × {}",
        groesse.0,
        groesse.1
    );
    let mut bild = RgbaImage::new(breite as u32, hoehe as u32);
    let laenge = bild.len();
    // SAFETY: `bild` hat `laenge` Bytes, `4 * breite` je Zeile und `hoehe`
    // Zeilen; libwebp schreibt nur dort hinein.
    let raus = unsafe {
        webp::WebPDecodeRGBAInto(
            daten.as_ptr(),
            daten.len(),
            bild.as_mut_ptr(),
            laenge,
            4 * breite,
        )
    };
    ensure!(!raus.is_null(), "libwebp dekodiert es nicht");
    Ok(bild)
}

/// [`encode_webp`] ohne das Warten beim ersten Mal.
fn kodiere(image: &RgbaImage) -> Result<Vec<u8>> {
    let passt_nicht = |()| anyhow!("libwebp passt nicht zu seinen Headern");
    let mut config = webp::WebPConfig::new().map_err(passt_nicht)?;
    let mut bild = webp::WebPPicture::new().map_err(passt_nicht)?;
    config.exact = 1;
    config.thread_level = 0;
    bild.use_argb = 1;
    bild.width = image.width() as c_int;
    bild.height = image.height() as c_int;
    let mut out = Vec::new();
    bild.writer = Some(anhaengen);
    bild.custom_ptr = (&raw mut out).cast();
    // SAFETY: `config` und `bild` hat libwebp angelegt, `image` hat
    // `4 * width` Bytes je Zeile, und `custom_ptr` zeigt auf `out`, das
    // bis nach `WebPPictureFree` an seinem Platz bleibt.
    let gelungen = unsafe {
        webp::WebPConfigLosslessPreset(&mut config, 0) != 0
            && webp::WebPPictureImportRGBA(&mut bild, image.as_raw().as_ptr(), 4 * bild.width) != 0
            && webp::WebPEncode(&config, &mut bild) != 0
    };
    let fehler = bild.error_code;
    // SAFETY: gibt frei, was der Import angelegt hat; danach wird `bild`
    // nicht mehr benutzt.
    unsafe { webp::WebPPictureFree(&mut bild) };
    ensure!(gelungen, "libwebp kodiert die Kachel nicht: {fehler:?}");
    Ok(out)
}

/// Hängt an, was libwebp beim Kodieren schreibt; `custom_ptr` zeigt auf den
/// Puffer aus [`encode_webp`].
unsafe extern "C" fn anhaengen(
    daten: *const u8,
    laenge: usize,
    bild: *const webp::WebPPicture,
) -> c_int {
    if laenge > 0 {
        // SAFETY: libwebp übergibt `laenge` lesbare Bytes, und `custom_ptr`
        // setzt `encode_webp` auf seinen `Vec<u8>`.
        unsafe {
            let out = &mut *(*bild).custom_ptr.cast::<Vec<u8>>();
            out.extend_from_slice(std::slice::from_raw_parts(daten, laenge));
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kachel_rechteck_kachelt_lueckenlos() {
        let a = TileId { x: 0, y: 0 }.rect();
        let b = TileId { x: 1, y: 0 }.rect();
        assert_eq!(a.right(), b.x);
        assert_eq!(a.width, TILE);
    }

    /// Negative Kachelkoordinaten müssen funktionieren: der Blockursprung
    /// liegt mitten in der Welt.
    #[test]
    fn negative_kacheln_liegen_links_oben() {
        let t = TileId { x: -1, y: -2 }.rect();
        assert_eq!((t.x, t.y), (-(TILE as i32), -2 * TILE as i32));
        assert_eq!(t.right(), 0);
    }

    #[test]
    fn covering_trifft_genau_die_beruehrten_kacheln() {
        let eine = ScreenRect {
            x: 10,
            y: 10,
            width: 4,
            height: 4,
        };
        assert_eq!(
            covering(eine).collect::<Vec<_>>(),
            vec![TileId { x: 0, y: 0 }]
        );

        // Ein Rechteck, das genau auf der Grenze endet, greift nicht in
        // die nächste Kachel.
        let bis_kante = ScreenRect {
            x: 0,
            y: 0,
            width: TILE,
            height: TILE,
        };
        assert_eq!(
            covering(bis_kante).collect::<Vec<_>>(),
            vec![TileId { x: 0, y: 0 }]
        );

        let ueber_kante = ScreenRect {
            x: -1,
            y: 0,
            width: TILE + 1,
            height: 1,
        };
        assert_eq!(
            ueber_kante.right(),
            TILE as i32,
            "Testaufbau: reicht bis zur Kante"
        );
        assert_eq!(
            covering(ueber_kante).collect::<Vec<_>>(),
            vec![TileId { x: -1, y: 0 }, TileId { x: 0, y: 0 }]
        );
    }

    /// Aufrunden muss das Rechteck immer vergrössern, nie verkleinern,
    /// und genau dieselben Kacheln treffen.
    #[test]
    fn aufrunden_deckt_dieselben_kacheln_ab() {
        for rect in [
            ScreenRect {
                x: 10,
                y: 10,
                width: 4,
                height: 4,
            },
            ScreenRect {
                x: -10,
                y: -300,
                width: 1,
                height: 1,
            },
            ScreenRect {
                x: 0,
                y: 0,
                width: TILE,
                height: TILE,
            },
            ScreenRect {
                x: -1,
                y: -1,
                width: 2 * TILE,
                height: 3 * TILE,
            },
        ] {
            let gerundet = snap_to_tiles(rect);
            assert!(gerundet.x <= rect.x && gerundet.y <= rect.y, "{rect:?}");
            assert!(
                gerundet.right() >= rect.right() && gerundet.bottom() >= rect.bottom(),
                "{rect:?}"
            );
            assert_eq!(gerundet.width % TILE, 0, "{rect:?}");
            assert_eq!(gerundet.height % TILE, 0, "{rect:?}");
            assert_eq!(
                covering(gerundet).collect::<Vec<_>>(),
                covering(rect).collect::<Vec<_>>(),
                "{rect:?} trifft nach dem Aufrunden andere Kacheln"
            );
        }
    }

    /// Ein aufgerundetes Rechteck ist schon rund.
    #[test]
    fn aufrunden_ist_idempotent() {
        let einmal = snap_to_tiles(ScreenRect {
            x: -7,
            y: 300,
            width: 9,
            height: 9,
        });
        assert_eq!(snap_to_tiles(einmal), einmal);
    }

    /// Ein Ausschnitt wird auf ganze Kacheln der gröbsten nativen Stufe
    /// aufgerundet, bei drei Stufen über der Basis also auf 2048 Pixel.
    #[test]
    fn ausschnitt_rundet_auf_das_grobe_raster() {
        let rect = ScreenRect {
            x: -8704,
            y: 1792,
            width: 2048,
            height: 2048,
        };
        assert_eq!(
            snap_to_grid(rect, TILE << 3),
            ScreenRect {
                x: -10240,
                y: 0,
                width: 4096,
                height: 4096
            }
        );
        assert_eq!(snap_to_grid(rect, TILE), rect, "schon auf Kacheln gerundet");
    }

    /// Das Gebiet einer Änderung enthält jede Basiskachel, in die ein Block
    /// bis 16 Blöcke neben dem Chunk und von der Unterkante bis 16 über
    /// seinem höchsten Block reicht: So weit wirken Licht und weiche
    /// Beleuchtung. Je Ecke jedes Blocks, aus jeder Richtung dreier Kameras,
    /// auf der Basis und bei drei Stufen.
    /// Siehe docs/benutzung/updates.md, „Wo ein Update zeichnet“.
    #[test]
    fn gebiet_reicht_16_bloecke_um_die_aenderung() {
        use crate::render::{Kamera, Richtung};
        let (unten, oben, chunk) = (-64, 40, [2, -3]);
        let aenderung = Aenderung {
            chunk,
            oben,
            bleibt: true,
        };
        for kamera in ["2:1", "top-north", "north-45"] {
            let kamera = Kamera::parse(kamera).unwrap();
            let namen = if kamera.genordet() {
                ["s", "w", "n", "e"]
            } else {
                ["se", "sw", "nw", "ne"]
            };
            for name in namen {
                let projection =
                    Projection::mit_kamera(16, kamera).aus(Richtung::parse(name, kamera).unwrap());
                let richtung = projection.richtung();
                for stufen in [0, 3] {
                    let gebiet =
                        gebiet_der_aenderungen(projection, stufen, unten, &[aenderung], None);
                    for x in chunk[0] * 16 - 16..chunk[0] * 16 + 32 {
                        for z in chunk[1] * 16 - 16..chunk[1] * 16 + 32 {
                            for y in [unten, oben + 16] {
                                for ecke in 0..8 {
                                    let p = [x + (ecke & 1), y + (ecke >> 1 & 1), z + (ecke >> 2)];
                                    let (sx, sy) =
                                        projection.project_block(richtung.versatz_in_den_blick(p));
                                    let tile = TileId {
                                        x: (sx / TILE as f64).floor() as i32,
                                        y: (sy / TILE as f64).floor() as i32,
                                    };
                                    assert!(
                                        gebiet.enthaelt(tile),
                                        "{kamera:?} {name} {stufen} Stufen, Ecke {p:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Ein Gebiet aus einem Rechteck verhält sich wie das Rechteck, auf
    /// sein Raster gerundet: Es berührt dieselben Rechtecke, enthält
    /// dieselben Basiskacheln, und auf jeder Stufe sind seine Fläche und
    /// ihre Spalten die Bereiche, die der gerundete Ausschnitt dort trifft.
    /// Auch links oben vom Ursprung.
    #[test]
    fn gebiet_aus_rechteck_gleicht_dem_gerundeten_rechteck() {
        let mut zufall = 0x2545_f491_4f6c_dd1du64;
        let mut zahl = |bis: i32| {
            zufall = zufall
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (zufall >> 33) as i32 % (2 * bis) - bis
        };
        let mut rechteck = |weit: i32, gross: i32| ScreenRect {
            x: zahl(weit),
            y: zahl(weit),
            width: zahl(gross).unsigned_abs() + 1,
            height: zahl(gross).unsigned_abs() + 1,
        };
        let ausschnitte: Vec<ScreenRect> = (0..20).map(|_| rechteck(3000, 2000)).collect();
        let proben: Vec<ScreenRect> = (0..200).map(|_| rechteck(6000, 900)).collect();
        for stufen in 0..3 {
            for &ausschnitt in &ausschnitte {
                let gebiet = Gebiet::rechteck(ausschnitt, stufen);
                let gerundet = snap_to_grid(ausschnitt, TILE << stufen);
                for &probe in &proben {
                    assert_eq!(
                        gebiet.beruehrt(probe),
                        overlaps(gerundet, probe),
                        "{ausschnitt:?} bei {stufen} Stufen, {probe:?}"
                    );
                }
                let rand = ScreenRect {
                    x: gerundet.x - TILE as i32,
                    y: gerundet.y - TILE as i32,
                    width: gerundet.width + 2 * TILE,
                    height: gerundet.height + 2 * TILE,
                };
                for tile in covering(rand) {
                    let r = tile.rect();
                    let innen = r.x >= gerundet.x
                        && r.right() <= gerundet.right()
                        && r.y >= gerundet.y
                        && r.bottom() <= gerundet.bottom();
                    assert_eq!(gebiet.enthaelt(tile), innen, "{tile:?}");
                }
                for hoch in 0..6 {
                    let stufe = |px: i32| px.div_euclid(TILE as i32) >> hoch;
                    let spalten = stufe(gerundet.x)..=stufe(gerundet.right() - 1);
                    let zeilen = stufe(gerundet.y)..=stufe(gerundet.bottom() - 1);
                    let flaeche = gebiet.flaeche(hoch);
                    assert_eq!(
                        flaeche.spalten(),
                        spalten.clone().collect::<Vec<_>>(),
                        "{ausschnitt:?} bei {stufen} Stufen, {hoch} über der Basis"
                    );
                    for y in zeilen.start() - 1..=zeilen.end() + 1 {
                        for x in spalten.start() - 1..=spalten.end() + 1 {
                            assert_eq!(
                                flaeche.enthaelt(&TileId { x, y }),
                                spalten.contains(&x) && zeilen.contains(&y),
                                "({x}, {y}) {hoch} über der Basis"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Ein Gebiet aus verstreuten Stücken berührt nur, was eines von ihnen
    /// trifft, auch zwischen ihnen nichts.
    #[test]
    fn gebiet_aus_stuecken_beruehrt_nur_sie() {
        let kante = TILE as i32 * 2;
        let gebiet = Gebiet::aus(
            1,
            BTreeSet::from([TileId { x: -3, y: 2 }, TileId { x: 4, y: -1 }]),
        );
        let quadrat = |x: i32, y: i32| ScreenRect {
            x: x * kante + 10,
            y: y * kante + 10,
            width: 5,
            height: 5,
        };
        assert!(gebiet.beruehrt(quadrat(-3, 2)));
        assert!(gebiet.beruehrt(quadrat(4, -1)));
        assert!(!gebiet.beruehrt(quadrat(0, 0)), "dazwischen");
        assert!(
            !gebiet.beruehrt(quadrat(-3, -1)),
            "Spalte des einen, Zeile des anderen"
        );
        let quer = ScreenRect {
            x: -3 * kante,
            y: 0,
            width: (8 * kante) as u32,
            height: kante as u32,
        };
        assert!(
            !gebiet.beruehrt(quer),
            "über beide Spalten, in keiner Zeile"
        );
        assert_eq!(
            gebiet.flaeche(0).spalten(),
            vec![-6, -5, 8, 9],
            "Basis: je Stück zwei Spalten"
        );
        assert_eq!(
            gebiet.flaeche(3).spalten(),
            vec![-1, 1],
            "zwei Stufen darüber"
        );
        assert!(gebiet.enthaelt(TileId { x: -5, y: 5 }));
        assert!(!gebiet.enthaelt(TileId { x: -5, y: 6 }));
    }

    #[test]
    fn spaltenkasten_waechst_mit_der_hoehe() {
        let p = Projection::new(16);
        let flach = column_box(p, 0, 0, (0, 1), CHUNK);
        let hoch = column_box(p, 0, 0, (0, 256), CHUNK);
        assert!(hoch.height > flach.height);
        assert_eq!(hoch.width, flach.width, "Höhe verschiebt nur senkrecht");
    }

    #[test]
    fn ueberlappung_ist_halboffen() {
        let a = ScreenRect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let daneben = ScreenRect {
            x: 10,
            y: 0,
            width: 10,
            height: 10,
        };
        assert!(!overlaps(a, daneben), "Berührung ist keine Überlappung");
        assert!(overlaps(
            a,
            ScreenRect {
                x: 9,
                y: 9,
                width: 10,
                height: 10
            }
        ));
    }
}
