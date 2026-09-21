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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Drehung eines Elements um eine Achse, wie im Modell-JSON angegeben.
#[derive(Debug, Clone, Copy)]
pub struct Rotation {
    pub origin: [f32; 3],
    pub axis: Axis,
    pub angle: f32,
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
                rotation: element.rotation.and_then(|r| {
                    Some(Rotation {
                        origin: r.origin,
                        // Ohne Achse ist die Angabe bedeutungslos; solche
                        // Elemente kommen in Vanilla-Schildmodellen vor.
                        axis: match r.axis.as_deref()? {
                            "x" => Axis::X,
                            "y" => Axis::Y,
                            "z" => Axis::Z,
                            _ => return None,
                        },
                        angle: r.angle,
                        rescale: r.rescale,
                    })
                }),
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
struct RotationJson {
    #[serde(default)]
    origin: [f32; 3],
    axis: Option<String>,
    #[serde(default)]
    angle: f32,
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

    #[test]
    fn seitennamen() {
        assert_eq!(Face::parse("north"), Some(Face::North));
        assert_eq!(Face::parse("nordwest"), None);
        assert_eq!(Face::North.name(), "north");
    }
}
