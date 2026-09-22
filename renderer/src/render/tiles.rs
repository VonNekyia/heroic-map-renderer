//! Die Bildebene in Kacheln zerlegen und herausfinden, welche davon etwas
//! zeigen.

use std::collections::BTreeSet;

use anyhow::Result;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use rayon::prelude::*;

use crate::world::{BlockState, REGION, World};

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
    let tile = TILE as i32;
    let x0 = rect.x.div_euclid(tile);
    let x1 = (rect.right() - 1).div_euclid(tile);
    let y0 = rect.y.div_euclid(tile);
    let y1 = (rect.bottom() - 1).div_euclid(tile);
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| TileId { x, y }))
}

/// Rundet ein Rechteck auf ganze Kacheln auf.
///
/// Ausgegeben werden immer vollständige Kacheln. Wer den Vorlauf auf den
/// angeforderten Ausschnitt begrenzt, lässt Blöcke weg, die in derselben
/// Kachel liegen — und die fehlen dann stillschweigend im Bild.
pub fn snap_to_tiles(rect: ScreenRect) -> ScreenRect {
    if rect.width == 0 || rect.height == 0 {
        return rect;
    }
    let tile = TILE as i32;
    let x = rect.x.div_euclid(tile) * tile;
    let y = rect.y.div_euclid(tile) * tile;
    let right = (rect.right() - 1).div_euclid(tile) * tile + tile;
    let bottom = (rect.bottom() - 1).div_euclid(tile) * tile + tile;
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

/// Bildbereich, in dem die ganze Welt liegen kann.
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
    let kante = REGION * CHUNK;
    let mut ganz: Option<ScreenRect> = None;
    for (rx, rz) in world.regions()? {
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
    /// Chunks, die gelesen wurden.
    pub chunks: usize,
}

/// Liest jeden Chunk einmal und sammelt beides ein: welche Blockstates
/// vorkommen und welche Kacheln überhaupt etwas zeigen.
///
/// `bounds` schränkt auf einen Bildausschnitt ein; ohne Angabe ist es die
/// ganze Welt.
///
/// Der Renderlauf liest die Chunks danach ein zweites Mal. Die Alternative
/// wäre eine Sprite-Tabelle hinter einer Sperre, und die stünde im
/// Renderpfad jedes Workers.
// ponytail: liest die Welt zweimal. Eine Blockstate-Liste neben der Welt
// spart den ersten Lauf, sobald er weh tut.
pub fn survey(
    world: &World,
    projection: Projection,
    y_range: (i32, i32),
    bounds: Option<ScreenRect>,
) -> Result<Survey> {
    // Auf ganze Kacheln runden, bevor irgendetwas ausgeschlossen wird:
    // gerendert wird die ganze Kachel, also muss auch der Vorlauf sie
    // ganz abdecken.
    let bounds = bounds.map(snap_to_tiles);
    let kante = REGION * CHUNK;
    let regions = world.regions()?;
    let teile: Vec<Survey> = regions
        .par_iter()
        .filter(|&&(rx, rz)| {
            let ganz = column_box(projection, rx * kante, rz * kante, y_range, kante);
            bounds.is_none_or(|b| overlaps(b, ganz))
        })
        .map(|&(rx, rz)| survey_region(world, projection, bounds, rx, rz))
        .collect::<Result<_>>()?;

    let mut tiles: BTreeSet<TileId> = BTreeSet::new();
    let mut survey = Survey::default();
    for teil in teile {
        tiles.extend(teil.tiles);
        survey.states.extend(teil.states);
        survey.chunks += teil.chunks;
    }
    survey.tiles = tiles.into_iter().collect();
    Ok(survey)
}

fn survey_region(
    world: &World,
    projection: Projection,
    bounds: Option<ScreenRect>,
    rx: i32,
    rz: i32,
) -> Result<Survey> {
    let mut survey = Survey::default();
    let Some(mut region) = world.region(rx, rz)? else {
        return Ok(survey);
    };

    let mut tiles = BTreeSet::new();
    for local_z in 0..REGION {
        for local_x in 0..REGION {
            let (cx, cz) = (rx * REGION + local_x, rz * REGION + local_z);
            let Some(chunk) = region.chunk(cx, cz)? else {
                continue;
            };
            survey.chunks += 1;

            // Nur Sections, in denen etwas steht. Die leeren ober- und
            // unterhalb des Geländes machen sonst jede Spalte so hoch wie
            // die ganze Welt.
            let mut belegt = chunk
                .sections()
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| s.y as i32 * CHUNK);
            let Some(unten) = belegt.next() else {
                continue;
            };
            let oben = belegt.next_back().unwrap_or(unten) + CHUNK;

            let rect = column_box(projection, cx * CHUNK, cz * CHUNK, (unten, oben), CHUNK);
            match bounds {
                // Erst ausschliessen, dann Paletten sammeln. Sonst
                // verlangt ein kleiner Ausschnitt die Assets für jeden
                // Block der Region, und ein einziger unbekannter Block
                // weit draussen bricht den ganzen Export ab.
                Some(bounds) if !overlaps(bounds, rect) => continue,
                Some(bounds) => tiles.extend(covering(clip(rect, bounds))),
                None => tiles.extend(covering(rect)),
            }

            for section in chunk.sections() {
                survey
                    .states
                    .extend(section.blocks().palette().iter().cloned());
            }
        }
    }

    survey.tiles = tiles.into_iter().collect();
    Ok(survey)
}

/// Bildrechteck, in dem eine Blockspalte der Kantenlänge `kante` landen
/// kann — die Reserve für überstehende Sprites eingerechnet.
fn column_box(
    projection: Projection,
    x: i32,
    z: i32,
    y_range: (i32, i32),
    kante: i32,
) -> ScreenRect {
    let mut min = (i32::MAX, i32::MAX);
    let mut max = (i32::MIN, i32::MIN);
    for &cx in &[x, x + kante] {
        for &cz in &[z, z + kante] {
            for &cy in &[y_range.0, y_range.1] {
                let (sx, sy) = projection.project_block([cx, cy, cz]);
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

fn overlaps(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

fn clip(rect: ScreenRect, bounds: ScreenRect) -> ScreenRect {
    let x = rect.x.max(bounds.x);
    let y = rect.y.max(bounds.y);
    ScreenRect {
        x,
        y,
        width: (rect.right().min(bounds.right()) - x).max(0) as u32,
        height: (rect.bottom().min(bounds.bottom()) - y).max(0) as u32,
    }
}

/// Kodiert ein Bild als verlustfreies WebP.
///
/// Verlustfrei und nicht verlustbehaftet: Minecraft-Texturen sind
/// Pixelkunst mit wenigen flachen Farben, die verlustfrei gut komprimiert.
/// Verlustbehaftet würde aus 16-Pixel-Texturen Matsch, und die
/// Kachelränder bekämen Artefakte, die man im Raster sieht.
pub fn encode_webp(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(out)
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
