//! Wasser und Lava, die es als Modell nicht gibt.
//!
//! `water.json` und `lava.json` nennen nur eine Partikeltextur; die
//! Geometrie baut Minecraft im Code (`LiquidBlockRenderer`). Ohne diesen
//! Nachbau bleiben Ozeane nackter Meeresboden — im Frontend war das der
//! auffälligste Fehlbestand.

use super::baker::{BakedModel, box_quads};

/// Welche Flüssigkeit ein Block enthält. Flächen zu einem Nachbarn mit
/// derselben Flüssigkeit entfallen beim Rendern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fluid {
    Water,
    Lava,
}
use super::{Assets, Face, TextureId, split_id};
use crate::world::BlockState;

/// Blöcke, die Wasser enthalten, ohne es in einer Eigenschaft zu führen.
///
/// Minecraft verdrahtet das ebenso fest (`LiquidBlockContainer`); aus den
/// Assets geht es nicht hervor.
const IMMER_IM_WASSER: [&str; 4] = ["kelp", "kelp_plant", "seagrass", "tall_seagrass"];

/// Färbungsindex der Wasserflächen.
///
/// Kein Modell-JSON vergibt diesen Index. Daran erkennt der Sprite-Bau,
/// dass hier die Wasserfarbe des Bioms gilt und nicht die Farbe des
/// Blocks, der das Wasser enthält.
pub const TINT_INDEX: u32 = u32::MAX;

const WATER: &str = "block/water_still";
const LAVA: &str = "block/lava_still";

/// Höhe in Neunteln der Blockhöhe, wenn dieselbe Flüssigkeit darüber
/// steht: bis zur Blockkante. Die Menge einer Oberfläche ist höchstens 8.
pub const FULL: u8 = 9;

/// Hängt die Flüssigkeit an das gebackene Modell einer Blockstate.
///
/// Der Würfel endet bei der eigenen Höhe der Flüssigkeit, wie an einer
/// Oberfläche. Steht darüber dieselbe Flüssigkeit, reicht sie bis zur
/// Blockkante — diese Fassung baut `SpriteSet`, denn nur der Renderer
/// kennt den Nachbarn.
pub fn add(model: &mut BakedModel, state: &BlockState, assets: &mut Assets) {
    let Some((level, fluid)) = kind(state) else {
        return;
    };
    let (textur, tint) = texture_of(fluid);
    let texture: TextureId = assets.texture(textur);
    model.quads.extend(box_quads(
        [0.0, 0.0, 0.0],
        [16.0, height(level), 16.0],
        texture,
        tint,
        Some(fluid),
    ));
}

/// Die Flüssigkeit einer Blockstate, falls sie eine enthält.
pub fn of(state: &BlockState) -> Option<Fluid> {
    kind(state).map(|(_, fluid)| fluid)
}

/// Ist der Block selbst die Flüssigkeit — Wasser, Lava, Blasensäule —
/// statt nur geflutet? Nur die haben ohne Modell trotzdem ein Bild; eine
/// geflutete Truhe bleibt eine Truhe, die Minecraft als Entity zeichnet.
pub fn is_block(state: &BlockState) -> bool {
    matches!(split_id(state.name()).1, "water" | "lava" | "bubble_column")
}

/// Art und Menge der Flüssigkeit in Neunteln der Blockhöhe
/// (`FlowingFluid.getAmount`: 8 für Quelle und Fall, sonst 8 minus Stufe).
/// Zwei Blockstates mit demselben Schlüssel bekommen denselben Würfel.
pub fn key(state: &BlockState) -> Option<(Fluid, u8)> {
    kind(state).map(|(level, fluid)| (fluid, amount(level)))
}

/// Ein Streifen der Seite `face` zwischen zwei Höhen in Neunteln: das
/// Stück der eigenen Seite, das über einem niedrigeren Nachbarn derselben
/// Flüssigkeit frei bleibt — am Fuss eines Wasserfalls, an einer Stufe
/// fliessenden Wassers. Vanilla hebt dort die Ecken der Oberfläche an;
/// hier bleibt sie eben, und der Streifen schliesst die Lücke.
pub fn strip(assets: &mut Assets, fluid: Fluid, face: Face, from: u8, to: u8) -> BakedModel {
    let (textur, tint) = texture_of(fluid);
    let texture = assets.texture(textur);
    let quads = box_quads(
        [0.0, 16.0 * from as f32 / 9.0, 0.0],
        [16.0, 16.0 * to as f32 / 9.0, 16.0],
        texture,
        tint,
        Some(fluid),
    )
    .filter(|q| q.fluid == Some((fluid, face)))
    .collect();
    BakedModel { quads }
}

/// Textur und Färbung einer Flüssigkeit. Lava bleibt ungefärbt.
fn texture_of(fluid: Fluid) -> (&'static str, Option<u32>) {
    match fluid {
        Fluid::Water => (WATER, Some(TINT_INDEX)),
        Fluid::Lava => (LAVA, None),
    }
}

/// Stufe und Art der Flüssigkeit einer Blockstate.
fn kind(state: &BlockState) -> Option<(u32, Fluid)> {
    Some(match split_id(state.name()).1 {
        "water" => (level_of(state), Fluid::Water),
        "lava" => (level_of(state), Fluid::Lava),
        // Eine Blasensäule ist Wasser mit Luftblasen; die Blasen sind ein
        // Partikeleffekt, das Wasser darunter ist ein voller Block.
        "bubble_column" => (0, Fluid::Water),
        name if IMMER_IM_WASSER.contains(&name) => (0, Fluid::Water),
        _ if state.prop("waterlogged") == Some("true") => (0, Fluid::Water),
        _ => return None,
    })
}

/// `level` einer Flüssigkeit, 0 für die Quelle.
fn level_of(state: &BlockState) -> u32 {
    state
        .prop("level")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Höhe der Flüssigkeitsoberfläche in Modellkoordinaten, wenn darüber
/// keine Flüssigkeit steht: `FlowingFluid.getOwnHeight`, die Menge durch
/// neun. Quelle und fallendes Wasser haben die Menge 8 und enden knapp zwei
/// Pixel unter der Blockkante — darüber ragen Stufen, Zaunpfosten und
/// obere Platten trocken heraus, wie im Spiel.
fn height(level: u32) -> f32 {
    16.0 * amount(level) as f32 / 9.0
}

/// `FlowingFluid.getAmount`: 8 für Quelle und Fall, sonst 8 minus Stufe.
fn amount(level: u32) -> u8 {
    if level == 0 || level >= 8 {
        8
    } else {
        8 - level as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quelle_und_fallendes_wasser_enden_bei_acht_neunteln() {
        for level in [0, 8, 15] {
            assert!((height(level) - 16.0 * 8.0 / 9.0).abs() < 1e-4, "{level}");
        }
    }

    /// Fliessendes Wasser wird mit jeder Stufe flacher.
    #[test]
    fn fliessendes_wasser_wird_flacher() {
        let hoehen: Vec<f32> = (1..8).map(height).collect();
        assert!(
            hoehen.windows(2).all(|p| p[0] > p[1]),
            "erwartet fallend, bekommen {hoehen:?}"
        );
        assert!((hoehen[0] - 16.0 * 7.0 / 9.0).abs() < 1e-4);
    }

    /// Nur Wasser, Lava und Blasensäule sind selbst die Flüssigkeit.
    #[test]
    fn geflutete_bloecke_sind_nicht_die_fluessigkeit() {
        let state = |text| BlockState::parse(text).unwrap();
        assert!(is_block(&state("minecraft:water[level=3]")));
        assert!(is_block(&state("minecraft:bubble_column")));
        assert!(!is_block(&state("minecraft:chest[waterlogged=true]")));
        assert!(of(&state("minecraft:chest[waterlogged=true]")).is_some());
        assert!(!is_block(&state("minecraft:stone")));
    }

    /// Ein Streifen ist genau eine Seitenfläche zwischen den beiden Höhen.
    #[test]
    fn streifen_liegt_zwischen_den_hoehen() {
        let base =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
        let mut assets = Assets::open(vec![base]).unwrap();
        let model = strip(&mut assets, Fluid::Water, Face::East, 7, FULL);
        assert_eq!(model.quads.len(), 1);
        let quad = &model.quads[0];
        assert_eq!(quad.fluid, Some((Fluid::Water, Face::East)));
        let hoehen: Vec<f32> = quad.corners.iter().map(|c| c[1]).collect();
        assert!(
            hoehen
                .iter()
                .all(|&y| (y - 7.0 / 9.0).abs() < 1e-6 || (y - 1.0).abs() < 1e-6),
            "{hoehen:?}"
        );
        assert!(
            quad.corners.iter().all(|c| (c[0] - 1.0).abs() < 1e-6),
            "Ostseite bei x = 1"
        );
    }
}
