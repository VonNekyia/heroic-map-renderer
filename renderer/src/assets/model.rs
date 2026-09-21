use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use super::texture::{TextureId, Textures};

/// Wie oft eine `#ref`-Kette weiterverfolgt wird, bevor ein Zyklus
/// angenommen wird.
const MAX_TEXTURE_REFS: usize = 8;

/// Die sechs Würfelseiten eines Modellelements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Face {
    pub fn parse(name: &str) -> Option<Face> {
        Some(match name {
            "down" => Face::Down,
            "up" => Face::Up,
            "north" => Face::North,
            "south" => Face::South,
            "west" => Face::West,
            "east" => Face::East,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Face::Down => "down",
            Face::Up => "up",
            Face::North => "north",
            Face::South => "south",
            Face::West => "west",
            Face::East => "east",
        }
    }
}

/// Drehung eines Elements, wie im Modell-JSON angegeben.
///
/// Minecraft kennt zwei Schreibweisen: die klassische mit `axis` und `angle`
/// und seit 1.21.11 eine mit `x`, `y` und `z` gleichzeitig. Beide landen hier
/// in `angles`; die klassische setzt genau einen Eintrag. Angewendet werden
/// die Winkel erst vom Baker in Schritt 3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotation {
    pub origin: [f32; 3],
    /// Winkel um X, Y und Z in Grad.
    pub angles: [f32; 3],
    pub rescale: bool,
}

#[derive(Debug, Clone)]
pub struct ElementFace {
    pub texture: TextureId,
    /// Aus `{"sprite": ..., "force_translucent": true}` in der Texturtabelle.
    /// Minecraft 26.x kennzeichnet damit Glas und Redstone; ausgewertet wird
    /// das Flag erst beim Transparenz-Schritt.
    pub force_translucent: bool,
    /// UV in Modellkoordinaten (0..16). Fehlt sie, leitet Minecraft sie aus
    /// der Elementgröße ab — das macht erst der Baker in Schritt 3.
    pub uv: Option<[f32; 4]>,
    pub cullface: Option<Face>,
    /// Drehung der Textur auf der Fläche, Vielfache von 90 Grad.
    pub rotation: u16,
    /// Index in die Färbung (Gras, Laub, Wasser).
    pub tint_index: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub rotation: Option<Rotation>,
    pub shade: bool,
    /// Höchstens sechs Einträge, nach Seite sortiert für stabile Ausgabe.
    pub faces: Vec<(Face, ElementFace)>,
}

/// Ein Modell mit aufgelöster `parent`-Kette und aufgelösten Texturen.
///
/// Ohne Elemente ist das Modell leer: Truhen, Banner und Schilder verweisen
/// auf `builtin/entity` und werden von Minecraft über Entity-Modelle
/// gezeichnet, die es in V1 nicht gibt.
#[derive(Debug)]
pub struct ResolvedModel {
    pub elements: Vec<Element>,
}

impl ResolvedModel {
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub(super) fn build(
        elements: Vec<ElementJson>,
        textures: &HashMap<String, TextureValue>,
        registry: &mut Textures,
        roots: &[PathBuf],
        model_id: &str,
    ) -> Result<ResolvedModel> {
        let mut out = Vec::with_capacity(elements.len());
        for element in elements {
            let mut faces: Vec<(Face, ElementFace)> = element
                .faces
                .into_iter()
                .filter_map(|(name, face)| {
                    let side = Face::parse(&name)?;
                    let (texture, force_translucent) =
                        match resolve_texture(&face.texture, textures) {
                            Some((path, translucent)) => (registry.load(roots, &path), translucent),
                            // Unauflösbare #ref: wie bei einer fehlenden
                            // Datei den Platzhalter nehmen, damit das Pack
                            // den Lauf nicht abbricht.
                            None => (
                                registry.load(roots, &format!("{model_id}#{}", face.texture)),
                                false,
                            ),
                        };
                    Some((
                        side,
                        ElementFace {
                            texture,
                            force_translucent,
                            uv: face.uv,
                            cullface: face.cullface.as_deref().and_then(Face::parse),
                            rotation: face.rotation % 360,
                            tint_index: face.tintindex.and_then(|t| u32::try_from(t).ok()),
                        },
                    ))
                })
                .collect();
            faces.sort_by_key(|(side, _)| *side);

            out.push(Element {
                from: element.from,
                to: element.to,
                rotation: element.rotation.map(Rotation::from),
                shade: element.shade,
                faces,
            });
        }
        Ok(ResolvedModel { elements: out })
    }
}

/// `#all` über die Texturtabelle auflösen, notfalls über mehrere Stufen.
///
/// Liefert den Sprite-Pfad und ob unterwegs ein `force_translucent` gesetzt war.
fn resolve_texture(
    reference: &str,
    textures: &HashMap<String, TextureValue>,
) -> Option<(String, bool)> {
    let mut current = reference.to_string();
    let mut translucent = false;
    for _ in 0..MAX_TEXTURE_REFS {
        let Some(name) = current.strip_prefix('#') else {
            return Some((current, translucent));
        };
        let value = textures.get(name)?;
        translucent |= value.force_translucent();
        current = value.sprite().to_string();
    }
    None
}

impl From<RotationJson> for Rotation {
    fn from(json: RotationJson) -> Rotation {
        // Die klassische Schreibweise hat Vorrang; nur wenn sie fehlt,
        // zählen x, y und z. Vanilla mischt beides innerhalb einer Datei,
        // etwa in block/template_hanging_sign_rot_3.
        let angles = match json.axis.as_deref() {
            Some("x") => [json.angle.unwrap_or(0.0), 0.0, 0.0],
            Some("y") => [0.0, json.angle.unwrap_or(0.0), 0.0],
            Some("z") => [0.0, 0.0, json.angle.unwrap_or(0.0)],
            _ => [
                json.x.unwrap_or(0.0),
                json.y.unwrap_or(0.0),
                json.z.unwrap_or(0.0),
            ],
        };
        Rotation {
            origin: json.origin,
            angles,
            rescale: json.rescale,
        }
    }
}

// ------------------------------------------------------------- JSON-Layout

/// Ein Textureintrag ist entweder ein Pfad oder seit Minecraft 26.x ein
/// Objekt mit zusätzlichen Angaben.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub(super) enum TextureValue {
    Sprite(String),
    Detailed {
        sprite: String,
        #[serde(default)]
        force_translucent: bool,
    },
}

impl TextureValue {
    fn sprite(&self) -> &str {
        match self {
            TextureValue::Sprite(s) => s,
            TextureValue::Detailed { sprite, .. } => sprite,
        }
    }

    fn force_translucent(&self) -> bool {
        match self {
            TextureValue::Sprite(_) => false,
            TextureValue::Detailed {
                force_translucent, ..
            } => *force_translucent,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct ModelJson {
    pub parent: Option<String>,
    #[serde(default)]
    pub textures: HashMap<String, TextureValue>,
    /// `None` heißt geerbt, `Some` überschreibt die Elemente des Elternteils.
    pub elements: Option<Vec<ElementJson>>,
}

#[derive(Deserialize)]
pub(super) struct ElementJson {
    from: [f32; 3],
    to: [f32; 3],
    #[serde(default)]
    faces: HashMap<String, FaceJson>,
    rotation: Option<RotationJson>,
    #[serde(default = "yes")]
    shade: bool,
}

#[derive(Deserialize)]
struct FaceJson {
    texture: String,
    uv: Option<[f32; 4]>,
    cullface: Option<String>,
    #[serde(default)]
    rotation: u16,
    tintindex: Option<i32>,
}

#[derive(Deserialize)]
pub(super) struct RotationJson {
    #[serde(default)]
    origin: [f32; 3],
    axis: Option<String>,
    angle: Option<f32>,
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    #[serde(default)]
    rescale: bool,
}

fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, TextureValue> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), TextureValue::Sprite(v.to_string())))
            .collect()
    }

    #[test]
    fn texturkette_wird_aufgeloest() {
        let textures = map(&[("side", "#all"), ("all", "minecraft:block/stone")]);
        assert_eq!(
            resolve_texture("#side", &textures),
            Some(("minecraft:block/stone".to_string(), false))
        );
        assert_eq!(
            resolve_texture("block/dirt", &textures),
            Some(("block/dirt".to_string(), false))
        );
    }

    #[test]
    fn force_translucent_ueberlebt_die_kette() {
        let mut textures = map(&[("all", "#glas")]);
        textures.insert(
            "glas".to_string(),
            TextureValue::Detailed {
                sprite: "minecraft:block/glass".to_string(),
                force_translucent: true,
            },
        );
        assert_eq!(
            resolve_texture("#all", &textures),
            Some(("minecraft:block/glass".to_string(), true))
        );
    }

    #[test]
    fn unaufloesbare_referenz_liefert_none() {
        assert_eq!(resolve_texture("#weg", &map(&[])), None);
        let zyklus = map(&[("a", "#b"), ("b", "#a")]);
        assert_eq!(resolve_texture("#a", &zyklus), None);
    }

    fn rotation(json: &str) -> Rotation {
        serde_json::from_str::<RotationJson>(json).unwrap().into()
    }

    #[test]
    fn klassische_rotation() {
        let r = rotation(r#"{"origin": [8, 0, 8], "axis": "y", "angle": -22.5}"#);
        assert_eq!(r.angles, [0.0, -22.5, 0.0]);
        assert_eq!(r.origin, [8.0, 0.0, 8.0]);
        assert!(!r.rescale);
    }

    /// Vanilla 26.2 nutzt das in block/template_hanging_sign_rot_3 und das
    /// TerraNova-Pack in bvb_template_sign_rot_3.
    #[test]
    fn mehrachsen_rotation() {
        let r = rotation(r#"{"x": 180, "y": -67.5, "z": -180, "origin": [8, 0, 8]}"#);
        assert_eq!(r.angles, [180.0, -67.5, -180.0]);
        assert_eq!(r.origin, [8.0, 0.0, 8.0]);
    }

    #[test]
    fn klassische_schreibweise_hat_vorrang() {
        let r = rotation(r#"{"axis": "y", "angle": 45, "x": 180, "z": 90}"#);
        assert_eq!(r.angles, [0.0, 45.0, 0.0]);
    }

    #[test]
    fn rotation_ohne_winkel_bleibt_erhalten() {
        // kommt in Vanilla-Schildmodellen vor: nur origin, keine Winkel
        let r = rotation(r#"{"origin": [8, 8, 8]}"#);
        assert_eq!(r.angles, [0.0, 0.0, 0.0]);
        assert_eq!(r.origin, [8.0, 8.0, 8.0]);
    }

    #[test]
    fn seitennamen() {
        assert_eq!(Face::parse("north"), Some(Face::North));
        assert_eq!(Face::parse("nordwest"), None);
        assert_eq!(Face::North.name(), "north");
    }
}
