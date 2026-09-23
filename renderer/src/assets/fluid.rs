//! Wasser und Lava, die es als Modell nicht gibt.
//!
//! `water.json` und `lava.json` nennen nur eine Partikeltextur; die
//! Geometrie baut Minecraft im Code (`LiquidBlockRenderer`). Ohne diesen
//! Nachbau bleiben Ozeane nackter Meeresboden — im Frontend war das der
//! auffälligste Fehlbestand.

use super::baker::{BakedModel, box_quads};

/// Welche Flüssigkeit ein Block enthält. Flächen zu einem Nachbarn mit
/// derselben Flüssigkeit entfallen beim Rendern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fluid {
    Water,
    Lava,
}
use super::{Assets, TextureId, split_id};
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

/// Hängt die Flüssigkeit an das gebackene Modell einer Blockstate.
///
/// Der Würfel endet bei der eigenen Höhe der Flüssigkeit, wie an einer
/// Oberfläche. Steht darüber dieselbe Flüssigkeit, reicht sie bis zur
/// Blockkante — diese Fassung baut `SpriteSet`, denn nur der Renderer
/// kennt den Nachbarn.
pub fn add(model: &mut BakedModel, state: &BlockState, assets: &mut Assets) {
    let Some((textur, level, tint, fluid)) = kind(state) else {
        return;
    };
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
    kind(state).map(|(_, _, _, fluid)| fluid)
}

/// Textur, Stufe, Färbung und Art der Flüssigkeit einer Blockstate.
fn kind(state: &BlockState) -> Option<(&'static str, u32, Option<u32>, Fluid)> {
    let water = |level| (WATER, level, Some(TINT_INDEX), Fluid::Water);
    Some(match split_id(state.name()).1 {
        "water" => water(level_of(state)),
        "lava" => ("block/lava_still", level_of(state), None, Fluid::Lava),
        // Eine Blasensäule ist Wasser mit Luftblasen; die Blasen sind ein
        // Partikeleffekt, das Wasser darunter ist ein voller Block.
        "bubble_column" => water(0),
        name if IMMER_IM_WASSER.contains(&name) => water(0),
        _ if state.prop("waterlogged") == Some("true") => water(0),
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
    let amount = if level == 0 || level >= 8 {
        8
    } else {
        8 - level
    };
    16.0 * amount as f32 / 9.0
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
}
