//! Ein Banner ohne Welt, als Sprite für die Ebenen: fester Massstab, ein
//! Pixel des Modells ist ein Pixel des Sprites, das Tuch im Blick nach
//! Süden, im vollen Himmelslicht; für eine Hauptstadt mit Krone.
//! Siehe docs/entscheidungen/0100-der-renderer-zeichnet-die-banner.md.

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use image::RgbaImage;

use crate::assets::baker::{BakedModel, box_quads};
use crate::assets::{Assets, TextureId, blockentity, models_of};
use crate::render::metatile::render_familie;
use crate::render::rasterizer::Light;
use crate::render::{Kamera, Projection, Richtung, SpriteSet};
use crate::world::{BlockState, Blockdaten, Muster};

/// Der Zeichenstand der Sprites der Banner, unabhängig von den Zeichenständen
/// der Bäume: Er steigt mit jeder Änderung an ihrem Bild, siehe
/// `bannerstand_folgt_den_goldbildern`.
/// Siehe docs/entscheidungen/0100-der-renderer-zeichnet-die-banner.md, „Stand und Stempel“.
pub const BANNERSTAND: u32 = 1;

/// Der scale, bei dem ein Pixel des Modells, ein Texel des Tuchs, ein Pixel
/// des Sprites ist. Das Spiel zeichnet Banner um 2/3 verkleinert
/// (`BannerRenderer.MODEL_SCALE`), ein Texel ist also 2/3 von 1/16 Block.
/// Schräg ist ein Block in der Breite (`h`) und in der Höhe (`b`) je
/// scale/2 Pixel, `north-45` je scale: schräg 48, `north-45` 24. Von oben gibt
/// es keins; Bäume von oben nehmen den Satz `oben` aus `north-45`.
pub fn massstab(kamera: Kamera) -> Option<u32> {
    match kamera {
        Kamera::Schraeg(_) => Some(48),
        Kamera::Nord45 => Some(24),
        Kamera::Oben | Kamera::ObenNord => None,
    }
}

/// Ein gezeichnetes Banner.
pub struct Bannerbild {
    pub bild: RgbaImage,
    /// Wo der Fuss der Stange liegt, in Pixeln des Bilds: die Mitte der
    /// Unterseite seines Blocks.
    pub fuss: (i32, i32),
    /// Um wie viel Grad die Unterkante des Tuchs nach rechts fällt.
    pub winkel: f64,
    /// Muster und Farbstoffe, die der Renderer nicht kennt; ihre Lagen fehlen.
    pub unbekannt: BTreeSet<String>,
}

/// Zeichnet ein Banner mit Grundfarbe `grund`, einem der 16 Farbstoffe, und
/// den Lagen `lagen` in `kamera` und `richtung`, mit `krone` auf dem
/// Querholz. Es steht in der Drehung `4 · Vierteldrehungen der Richtung`: so
/// zeigt das Tuch im Blick nach Süden, zur Kamera hin.
pub fn zeichne(
    assets: &mut Assets,
    kamera: Kamera,
    richtung: Richtung,
    grund: &str,
    lagen: &[(Muster, String)],
    krone: bool,
) -> Result<Bannerbild> {
    let scale = massstab(kamera).with_context(|| format!("{kamera} zeichnet kein Banner"))?;
    let projection = Projection::mit_kamera(scale, kamera).aus(richtung);
    let drehung = 4 * u32::from(richtung.vierteldrehungen());
    let state = BlockState::parse(&format!("minecraft:{grund}_banner[rotation={drehung}]"))
        .map_err(|e| anyhow!("{grund}: {e}"))?;
    let mut sprites = SpriteSet::build_in(assets, [&state], projection)?;
    let daten = Blockdaten::Banner(lagen.to_vec());
    let unbekannt = blockentity::unbekannt(&daten, assets).into_iter().collect();
    let mut models = models_of(assets, &state, Some(&daten))?;
    if krone {
        let textur = assets.eigene_textur("#krone", || {
            image::load_from_memory(KRONE_PNG)
                .expect("krone.png")
                .into_rgba8()
        });
        for (_, model) in &mut models {
            setze_krone(model, &state, textur)?;
        }
    }
    let familie = sprites
        .familie_aus(assets, &state, &models)
        .with_context(|| format!("{state} zeichnet nichts"))?;
    let (bild, (ox, oy)) = render_familie(&sprites, familie, Light { sky: 15, block: 0 })
        .with_context(|| format!("{state} zeichnet nichts"))?;
    // Die Mitte der Unterseite des Blocks und die Strecke eines Blocks quer
    // zum Blick, entlang x im Blick.
    let (h, a) = (projection.h(), projection.a());
    let (mx, my) = match kamera.genordet() {
        true => (0.5 * h, 0.5 * a),
        false => (0.0, a),
    };
    let quer = match kamera.genordet() {
        true => (h, 0.0),
        false => (h, a),
    };
    Ok(Bannerbild {
        bild,
        fuss: (ox + mx.round() as i32, oy + my.round() as i32),
        winkel: quer.1.atan2(quer.0).to_degrees(),
        unbekannt,
    })
}

/// Die Textur der Krone, 16 × 16, eigene Pixelkunst, deckend. Die Quelle
/// mit den Bereichen als Slices liegt in
/// `docs/bilder/quellen/krone/krone.aseprite`.
const KRONE_PNG: &[u8] = include_bytes!("krone.png");

/// Ein Bereich der Textur der Krone: x0, y0, x1, y1 in Texeln.
type Bereich = [f32; 4];

const REIF_AUSSEN_VORN: Bereich = [0.0, 0.0, 8.0, 3.0];
const REIF_AUSSEN_SEITE: Bereich = [8.0, 0.0, 14.0, 3.0];
const REIF_INNEN: Bereich = [0.0, 3.0, 8.0, 6.0];
const REIF_INNEN_SEITE: Bereich = [1.0, 3.0, 7.0, 6.0];
const REIF_INNEN_KANTE: Bereich = [0.0, 3.0, 1.0, 6.0];
const REIF_OBEN_VORN: Bereich = [8.0, 3.0, 16.0, 4.0];
const REIF_OBEN_SEITE: Bereich = [8.0, 4.0, 14.0, 5.0];
const ZACKE_OBEN: Bereich = [0.0, 7.0, 2.0, 8.0];
const ZACKE_VORN: Bereich = [0.0, 8.0, 2.0, 11.0];
const ZACKE_SEITE: Bereich = [2.0, 8.0, 3.0, 11.0];
/// Die Spalten am Rand von `REIF_AUSSEN_VORN`: Die Enden der Wand vorn und
/// hinten setzen ihre Aussenseite um die Ecke fort.
const REIF_ECKE_LINKS: Bereich = [0.0, 0.0, 1.0, 3.0];
const REIF_ECKE_RECHTS: Bereich = [7.0, 0.0, 8.0, 3.0];

/// Ein Quader der Krone in Pixeln des Modells, x nach rechts, y nach oben,
/// z nach vorn, zur Seite des Tuchs mit seinen Mustern; dazu je Seite ihr
/// Bereich, in der Reihenfolge von `box_quads`: unten, oben, hinten, vorn,
/// links, rechts. Die Unterseiten sieht keine Kamera.
type Quader = ([f32; 3], [f32; 3], [Bereich; 6]);

/// Die Krone: ein Reif von 8 × 3 × 8 aus vier Wänden, 1 dick, und je Seite
/// mittig eine Zacke von 2 × 3 × 1.
/// Siehe docs/renderer/blockentities.md, „Die Krone“.
const KRONE: [Quader; 8] = {
    let (a, i, k) = (REIF_AUSSEN_VORN, REIF_INNEN, REIF_INNEN_KANTE);
    let (s, is, o, os) = (
        REIF_AUSSEN_SEITE,
        REIF_INNEN_SEITE,
        REIF_OBEN_VORN,
        REIF_OBEN_SEITE,
    );
    let (l, r) = (REIF_ECKE_LINKS, REIF_ECKE_RECHTS);
    let (zo, zv, zs) = (ZACKE_OBEN, ZACKE_VORN, ZACKE_SEITE);
    [
        // Die Wände hinten und vorn, von hinten gesehen liegt r links.
        ([0.0, 0.0, 0.0], [8.0, 3.0, 1.0], [o, o, a, i, r, l]),
        ([0.0, 0.0, 7.0], [8.0, 3.0, 8.0], [o, o, i, a, l, r]),
        // Die Wände links und rechts zwischen ihnen.
        ([0.0, 0.0, 1.0], [1.0, 3.0, 7.0], [os, os, k, k, s, is]),
        ([7.0, 0.0, 1.0], [8.0, 3.0, 7.0], [os, os, k, k, is, s]),
        // Die Zacken hinten, vorn, links und rechts.
        ([3.0, 3.0, 0.0], [5.0, 6.0, 1.0], [zo, zo, zv, zv, zs, zs]),
        ([3.0, 3.0, 7.0], [5.0, 6.0, 8.0], [zo, zo, zv, zv, zs, zs]),
        ([0.0, 3.0, 3.0], [1.0, 6.0, 5.0], [zo, zo, zs, zs, zv, zv]),
        ([7.0, 3.0, 3.0], [8.0, 6.0, 5.0], [zo, zo, zs, zs, zv, zv]),
    ]
};

/// Setzt die Krone mittig auf das Querholz des Banners `state`, in dessen
/// Lage und Schicht: im selben Massstab und derselben Drehung wie das
/// Banner. Ein Bereich, der quer zu seiner Seite liegt, dreht mit.
fn setze_krone(model: &mut BakedModel, state: &BlockState, textur: TextureId) -> Result<()> {
    let (lage, oben, entity) =
        blockentity::erste_form(state).with_context(|| format!("{state} hat kein Querholz"))?;
    // Im Raum des Modells zeigt y nach unten und z nach hinten; die Mitte
    // der Krone liegt über der Mitte des Querholzes.
    let modell = |[x, y, z]: [f32; 3]| [x - 0.25, oben - y, 0.25 - z];
    let welt = |p: [f32; 3]| {
        lage.map(|zeile| zeile[0] * p[0] + zeile[1] * p[1] + zeile[2] * p[2] + zeile[3])
    };
    for (von, bis, bereiche) in KRONE {
        for (mut quad, bereich) in box_quads(von, bis, textur, None, None).zip(bereiche) {
            let spanne = |achse: usize| {
                let werte = quad.uvs.map(|uv| uv[achse]);
                let von = werte.iter().copied().fold(f32::INFINITY, f32::min);
                (von, werte.iter().copied().fold(f32::NEG_INFINITY, f32::max))
            };
            let ((u0, u1), (v0, v1)) = (spanne(0), spanne(1));
            let (breite, hoehe) = (16.0 * (u1 - u0), 16.0 * (v1 - v0));
            let quer = breite != hoehe
                && breite == bereich[3] - bereich[1]
                && hoehe == bereich[2] - bereich[0];
            quad.uvs = quad.uvs.map(|[u, v]| {
                let (fu, fv) = ((u - u0) / (u1 - u0), (v - v0) / (v1 - v0));
                let (fu, fv) = if quer { (fv, fu) } else { (fu, fv) };
                [
                    (bereich[0] + fu * (bereich[2] - bereich[0])) / 16.0,
                    (bereich[1] + fv * (bereich[3] - bereich[1])) / 16.0,
                ]
            });
            quad.corners = quad.corners.map(|ecke| welt(modell(ecke)));
            quad.entity = Some(entity);
            model.quads.push(quad);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::world::chunk::Fnv;

    /// Die Krone steht in jeder Drehung mittig auf dem Querholz: 8 Pixel des
    /// Modells breit und tief, 6 hoch, um 2/3 verkleinert wie das Banner,
    /// also 1/3 und 1/4 Block; ihr Boden auf der Oberseite des Querholzes,
    /// 2/3 · 44/16 Block über dem Boden des Blocks. Jede Seite eines Quaders
    /// zeigt aus ihm hinaus.
    #[test]
    fn krone_auf_dem_querholz() {
        for drehung in [0, 4, 8, 12] {
            let state =
                BlockState::parse(&format!("minecraft:white_banner[rotation={drehung}]")).unwrap();
            let mut model = BakedModel::default();
            setze_krone(&mut model, &state, TextureId(0)).unwrap();
            assert_eq!(model.quads.len(), 6 * KRONE.len());
            let ecken: Vec<[f32; 3]> = model.quads.iter().flat_map(|q| q.corners).collect();
            let rand = |achse: usize| {
                let werte = ecken.iter().map(|e| e[achse]);
                (
                    werte.clone().fold(f32::INFINITY, f32::min),
                    werte.fold(f32::NEG_INFINITY, f32::max),
                )
            };
            let nah =
                |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5;
            let (boden, halb) = (2.0 / 3.0 * 44.0 / 16.0, 1.0 / 6.0);
            assert!(
                nah(rand(0), (0.5 - halb, 0.5 + halb)),
                "{drehung}: x {:?}",
                rand(0)
            );
            assert!(
                nah(rand(2), (0.5 - halb, 0.5 + halb)),
                "{drehung}: z {:?}",
                rand(2)
            );
            assert!(
                nah(rand(1), (boden, boden + 0.25)),
                "{drehung}: y {:?}",
                rand(1)
            );
            for (i, seiten) in model.quads.chunks(6).enumerate() {
                let alle: Vec<[f32; 3]> = seiten.iter().flat_map(|q| q.corners).collect();
                let mitte = |q: &[[f32; 3]]| {
                    [0, 1, 2].map(|a| q.iter().map(|e| e[a]).sum::<f32>() / q.len() as f32)
                };
                let quader = mitte(&alle);
                for q in seiten {
                    let (n, m) = (q.normal(), mitte(&q.corners));
                    let aussen: f32 = (0..3).map(|a| n[a] * (m[a] - quader[a])).sum();
                    assert!(aussen > 0.0, "{drehung}: Quader {i}, Normale {n:?}");
                }
            }
        }
    }

    /// FNV-1a über die Goldbilder der Banner unter
    /// `tests/fixtures/golden-banner`, nach Dateinamen, je Bild der Name,
    /// Breite, Höhe und die Pixel.
    const GOLDBILDER: u64 = 0x16fc_1103_ab33_e0bc;

    /// Ändert sich ein Goldbild der Banner, zeichnet der Renderer die Sprites
    /// anders: `BANNERSTAND` steigt, nicht der Zeichenstand der Bäume.
    #[test]
    fn bannerstand_folgt_den_goldbildern() {
        let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden-banner");
        let mut namen: Vec<String> = std::fs::read_dir(&ordner)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".png") && !n.ends_with("-ist.png"))
            .collect();
        namen.sort_unstable();
        let mut fnv = Fnv::default();
        for name in &namen {
            let bild = image::open(ordner.join(name)).unwrap().into_rgba8();
            fnv.text(name.trim_end_matches(".png"));
            fnv.nimm(&bild.width().to_le_bytes());
            fnv.nimm(&bild.height().to_le_bytes());
            fnv.nimm(bild.as_raw());
        }
        assert_eq!(
            fnv.0,
            GOLDBILDER,
            "Die Goldbilder der Banner haben sich geändert. Zeichnet der Renderer \
             die Sprites anders, BANNERSTAND in renderer/src/render/banner.rs auf \
             {} heben. Kommt nur ein Goldbild dazu, bleibt er. In jedem Fall \
             GOLDBILDER dort auf {:#x} setzen.",
            BANNERSTAND + 1,
            fnv.0
        );
    }
}
