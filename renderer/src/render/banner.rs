//! Ein Banner ohne Welt, als Sprite für die Ebenen: fester Massstab, ein
//! Pixel des Modells ist ein Pixel des Sprites, das Tuch im Blick nach
//! Süden, im vollen Himmelslicht.
//! Siehe docs/entscheidungen/0100-der-renderer-zeichnet-die-banner.md.

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use image::RgbaImage;

use crate::assets::Assets;
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
/// den Lagen `lagen` in `kamera` und `richtung`. Es steht in der Drehung
/// `4 · Vierteldrehungen der Richtung`: so zeigt das Tuch im Blick nach
/// Süden, zur Kamera hin.
pub fn zeichne(
    assets: &mut Assets,
    kamera: Kamera,
    richtung: Richtung,
    grund: &str,
    lagen: &[(Muster, String)],
) -> Result<Bannerbild> {
    let scale = massstab(kamera).with_context(|| format!("{kamera} zeichnet kein Banner"))?;
    let projection = Projection::mit_kamera(scale, kamera).aus(richtung);
    let drehung = 4 * u32::from(richtung.vierteldrehungen());
    let state = BlockState::parse(&format!("minecraft:{grund}_banner[rotation={drehung}]"))
        .map_err(|e| anyhow!("{grund}: {e}"))?;
    let mut sprites = SpriteSet::build_in(assets, [&state], projection)?;
    let daten = Blockdaten::Banner(lagen.to_vec());
    let unbekannt = sprites.add_entities(assets, [&(state.clone(), daten.clone())])?;
    let basis = sprites
        .family_index(&state)
        .with_context(|| format!("{state} hat kein Modell"))?;
    let familie = sprites.variante(basis, &daten).unwrap_or(basis);
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::world::chunk::Fnv;

    /// FNV-1a über die Goldbilder der Banner unter
    /// `tests/fixtures/golden-banner`, nach Dateinamen, je Bild der Name,
    /// Breite, Höhe und die Pixel.
    const GOLDBILDER: u64 = 0x717c_e3db_81da_0e39;

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
