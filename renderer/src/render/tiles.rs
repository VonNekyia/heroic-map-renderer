//! Die Bildebene in Kacheln zerlegen und herausfinden, welche davon etwas
//! zeigen.

use std::collections::{BTreeSet, HashSet};

use std::ffi::c_int;
use std::sync::Once;

use anyhow::{Result, anyhow, ensure};
use image::RgbaImage;
use libwebp_sys as webp;
use rayon::prelude::*;

use crate::world::{BlockState, Blockdaten, Chunk, REGION, World};

use super::heights::{Heights, RegionHeights};
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
}

/// Welche Chunks der Vorlauf liest: die, deren Spalte über die ganze
/// Welthöhe den Ausschnitt berührt; ohne Ausschnitt alle.
#[derive(Clone, Copy)]
pub struct Reach {
    projection: Projection,
    hoehe: (i32, i32),
    bounds: Option<ScreenRect>,
}

impl Reach {
    pub fn new(projection: Projection, y_range: (i32, i32), bounds: Option<ScreenRect>) -> Reach {
        Reach {
            projection,
            // Bis zur Oberkante des obersten Blocks, wie der genaue Kasten
            // je Chunk im Vorlauf.
            hoehe: (y_range.0, y_range.1 + 1),
            // Auf ganze Kacheln runden, bevor irgendetwas ausgeschlossen
            // wird: gerendert wird die ganze Kachel, also muss auch der
            // Vorlauf sie ganz abdecken.
            bounds: bounds.map(snap_to_tiles),
        }
    }

    /// Ob der Lauf Chunks der Region (rx, rz) liest.
    pub fn region(&self, rx: i32, rz: i32) -> bool {
        let kante = REGION * CHUNK;
        self.column(rx * kante, rz * kante, kante)
    }

    /// Ob der Lauf den Chunk (cx, cz) liest.
    pub fn chunk(&self, cx: i32, cz: i32) -> bool {
        self.column(cx * CHUNK, cz * CHUNK, CHUNK)
    }

    fn column(&self, x: i32, z: i32, kante: i32) -> bool {
        self.bounds
            .is_none_or(|b| overlaps(b, column_box(self.projection, x, z, self.hoehe, kante)))
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
        match self.bounds {
            Some(bounds) if !overlaps(bounds, rect) => Content::Outside,
            Some(bounds) => Content::Inside(clip(rect, bounds)),
            None => Content::Inside(rect),
        }
    }
}

/// Was ein Chunk im Ausschnitt eines Laufs zeigt, siehe [`Reach::content`].
enum Content {
    /// Keine Section, in der etwas steht.
    Empty,
    /// Seine Blöcke landen ausserhalb des Ausschnitts.
    Outside,
    /// Seine Blöcke können in diesem Rechteck landen, auf den Ausschnitt
    /// beschnitten, samt der Reserve für überstehende Sprites.
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
    let reach = Reach::new(projection, y_range, bounds);
    let regions = world.regions()?;
    let teile: Vec<Survey> = regions
        .par_iter()
        .filter(|&&(rx, rz)| reach.region(rx, rz))
        .map(|&(rx, rz)| survey_region(world, reach, rx, rz))
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
    }
    survey.tiles = tiles.into_iter().collect();
    Ok(survey)
}

fn survey_region(world: &World, reach: Reach, rx: i32, rz: i32) -> Result<Survey> {
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
            // wird gar nicht erst dekodiert.
            if !reach.chunk(cx, cz) {
                continue;
            }
            gelesen[(local_z * REGION + local_x) as usize] = true;
            let Some(chunk) = region.stored_chunk(cx, cz)? else {
                continue;
            };
            if !chunk.is_generated() {
                survey.unfinished += 1;
                continue;
            }
            survey.chunks += 1;
            // Die Höhen hängen nicht an der Sprite-Tabelle: auch ein Chunk,
            // dessen Blöcke ausserhalb landen, bekommt seine.
            hoehen.record(&chunk);

            // Erst ausschliessen, dann Paletten sammeln. Sonst verlangt ein
            // kleiner Ausschnitt die Assets für jeden Block der Region, und
            // ein einziger unbekannter Block weit draussen bricht den ganzen
            // Export ab.
            let Content::Inside(rect) = reach.content(&chunk) else {
                continue;
            };
            tiles.extend(covering(rect));

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
    let mut min = (i32::MAX, i32::MAX);
    let mut max = (i32::MIN, i32::MIN);
    let richtung = projection.richtung();
    for &cx in &[x, x + kante] {
        for &cz in &[z, z + kante] {
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
