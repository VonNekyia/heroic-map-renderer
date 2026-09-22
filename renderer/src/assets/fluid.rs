//! Wasser und Lava, die es als Modell nicht gibt.
//!
//! `water.json` und `lava.json` nennen nur eine Partikeltextur; die
//! Geometrie baut Minecraft im Code (`LiquidBlockRenderer`). Ohne diesen
//! Nachbau bleiben Ozeane nackter Meeresboden — im Frontend war das der
//! auffälligste Fehlbestand.

use super::baker::{BakedModel, box_quads};
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
pub fn add(model: &mut BakedModel, state: &BlockState, assets: &mut Assets) {
    let water = |level| (WATER, level, Some(TINT_INDEX));
    let (textur, level, tint) = match split_id(state.name()).1 {
        "water" => water(level_of(state)),
        "lava" => ("block/lava_still", level_of(state), None),
        // Eine Blasensäule ist Wasser mit Luftblasen; die Blasen sind ein
        // Partikeleffekt, das Wasser darunter ist ein voller Block.
        "bubble_column" => water(0),
        name if IMMER_IM_WASSER.contains(&name) => water(0),
        _ if state.prop("waterlogged") == Some("true") => water(0),
        _ => return,
    };

    let texture: TextureId = assets.texture(textur);
    model.quads.extend(box_quads(
        [0.0, 0.0, 0.0],
        [16.0, height(level), 16.0],
        texture,
        tint,
    ));
}

/// `level` einer Flüssigkeit, 0 für die Quelle.
fn level_of(state: &BlockState) -> u32 {
    state
        .prop("level")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Höhe der Flüssigkeitsoberfläche in Modellkoordinaten.
///
/// Minecraft rechnet `(8 - level) / 9` und lässt eine Quelle damit gut
/// einen Pixel unter der Blockkante enden. Das gilt hier nur für fliessende
/// Stufen: ohne Nachbarschaftswissen bekäme sonst jede Schicht eines Ozeans
/// eine Fuge, denn das Sprite kennt nur seine eigene Blockstate.
// ponytail: Quelle und fallendes Wasser auf volle Höhe. Erst nötig, wenn
// der Renderer Nachbarn kennt — dann die Oberfläche nur anheben, wenn
// darüber wieder Flüssigkeit steht.
fn height(level: u32) -> f32 {
    if level == 0 || level >= 8 {
        16.0
    } else {
        16.0 * (8 - level) as f32 / 9.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quelle_und_fallendes_wasser_fuellen_den_block() {
        assert_eq!(height(0), 16.0);
        assert_eq!(height(8), 16.0);
        assert_eq!(height(15), 16.0);
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
