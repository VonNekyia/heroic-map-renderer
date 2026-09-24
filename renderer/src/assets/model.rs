use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::{Map, Value};

use super::blockstate::{field, identifier, int_value};
use super::texture::{TextureId, Textures};

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
    /// Index in die Färbung (Gras, Laub, Wasser). Gefärbt ist, wie im
    /// Client, jede Fläche mit einem `tintindex` ausser -1.
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
/// Ohne Elemente ist das Modell leer: Truhen, Banner und Schilder haben in
/// 26.2 nur eine Partikeltextur und werden von Minecraft über
/// Entity-Modelle gezeichnet, die es in V1 nicht gibt.
#[derive(Debug)]
pub struct ResolvedModel {
    pub elements: Vec<Element>,
}

impl ResolvedModel {
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Backt die Elemente wie `UnbakedCuboidGeometry.bake`: eine Seite ohne
    /// Fläche fällt weg, bevor der Client ihre Textur sucht, und jede andere
    /// nennt einen Slot der Texturtabelle.
    ///
    /// Ein Element oder eine Seite `null` und der Texturname `""` lassen
    /// sich lesen, aber nicht backen; das Modell ist dann kaputt.
    // ponytail: dem Client fehlt dann der ganze Zustand, bei Multipart jeder
    // Zustand des Blocks, hier nur dieser Verweis. So schreibt kein Pack.
    pub(super) fn build(
        elements: Vec<Option<ElementJson>>,
        textures: &HashMap<String, Slot>,
        registry: &mut Textures,
        roots: &[PathBuf],
        model_id: &str,
    ) -> Result<ResolvedModel> {
        let mut out = Vec::with_capacity(elements.len());
        for element in elements {
            let element = element.ok_or_else(|| anyhow!("ein Element ist null"))?;
            let [x, y, z] = [0, 1, 2].map(|axis| element.from[axis] != element.to[axis]);
            let mut faces = Vec::with_capacity(element.faces.len());
            for (side, face) in element.faces {
                let area = match side {
                    Face::West | Face::East => y && z,
                    Face::Down | Face::Up => x && z,
                    Face::North | Face::South => x && y,
                };
                if !area {
                    continue;
                }
                let face = face.ok_or_else(|| anyhow!("Seite {} ist null", side.name()))?;
                ensure!(
                    !face.texture.is_empty(),
                    "Seite {} ohne Texturname",
                    side.name()
                );
                let (texture, force_translucent) = match resolve_texture(&face.texture, textures) {
                    // `MaterialBaker.get`: dafür gibt es kein Bild.
                    Some(("minecraft:missingno", translucent)) => (Textures::MISSING, translucent),
                    Some((sprite, translucent)) => (registry.load(roots, sprite), translucent),
                    // Unauflösbarer Slot: wie bei einer fehlenden Datei den
                    // Platzhalter nehmen, damit das Pack den Lauf nicht
                    // abbricht.
                    None => (
                        registry.load(roots, &format!("{model_id}#{}", face.texture)),
                        false,
                    ),
                };
                faces.push((
                    side,
                    ElementFace {
                        texture,
                        force_translucent,
                        uv: face.uv,
                        cullface: face.cullface,
                        rotation: face.rotation,
                        tint_index: (face.tint_index != -1).then_some(face.tint_index as u32),
                    },
                ));
            }
            faces.sort_by_key(|(side, _)| *side);

            out.push(Element {
                from: element.from,
                to: element.to,
                rotation: element.rotation,
                shade: element.shade,
                faces,
            });
        }
        Ok(ResolvedModel { elements: out })
    }
}

/// Die Textur einer Seite wie `TextureSlots.getMaterial`: ein `#` vorn
/// fällt weg, der Rest ist immer der Name eines Slots, nie ein Pfad. Von
/// dort führen Verweise weiter, bis ein Material dasteht
/// (`TextureSlots$Resolver`); ein Zyklus oder ein Verweis ins Leere bleibt
/// ungelöst.
///
/// Liefert den Sprite-Namen und `force_translucent` des Materials.
fn resolve_texture<'a>(
    texture: &str,
    textures: &'a HashMap<String, Slot>,
) -> Option<(&'a str, bool)> {
    let mut name = texture.strip_prefix('#').unwrap_or(texture);
    // Ohne Zyklus kommt jeder Schritt auf einen anderen Slot.
    for _ in 0..textures.len() {
        match textures.get(name)? {
            Slot::Reference(target) => name = target,
            Slot::Material {
                sprite,
                force_translucent,
            } => return Some((sprite, *force_translucent)),
        }
    }
    None
}

// ------------------------------------------------------------ Modelldatei

/// Ein Eintrag der Texturtabelle, wie `TextureSlots.parseEntry` ihn liest.
#[derive(Debug, Clone)]
pub(super) enum Slot {
    /// `#name`: verweist auf einen anderen Slot, hier ohne `#`.
    Reference(String),
    /// Mit Namensraum, wie `Identifier` ihn liest.
    Material {
        sprite: String,
        force_translucent: bool,
    },
}

/// Eine Modelldatei, wie `CuboidModel$Deserializer` sie liest.
pub(super) struct ModelFile {
    /// Mit Namensraum; ein leerer `parent` heisst keiner.
    pub parent: Option<String>,
    pub textures: Vec<(String, Slot)>,
    /// `None`, wenn der Schlüssel fehlt: dann gelten die des Parents.
    /// Ein Element `null` lässt Gson durch.
    pub elements: Option<Vec<Option<ElementJson>>>,
}

pub(super) struct ElementJson {
    from: [f32; 3],
    to: [f32; 3],
    rotation: Option<Rotation>,
    shade: bool,
    /// Eine Seite `null` lässt Gson durch.
    faces: Vec<(Face, Option<FaceJson>)>,
}

struct FaceJson {
    texture: String,
    uv: Option<[f32; 4]>,
    cullface: Option<Face>,
    rotation: u16,
    tint_index: i32,
}

impl ModelFile {
    /// `MissingCuboidModel`, der Parent `builtin/missing` und alles, was an
    /// die Stelle eines fehlenden Parents tritt: ein voller Würfel, jede
    /// Seite mit dem Slot `missingno` und an ihrer eigenen Richtung gekappt.
    pub(super) fn missing() -> ModelFile {
        let faces = [
            Face::Down,
            Face::Up,
            Face::North,
            Face::South,
            Face::West,
            Face::East,
        ]
        .map(|side| {
            let face = FaceJson {
                texture: "missingno".to_string(),
                uv: Some([0.0, 0.0, 16.0, 16.0]),
                cullface: Some(side),
                rotation: 0,
                tint_index: -1,
            };
            (side, Some(face))
        });
        ModelFile {
            parent: None,
            textures: vec![
                (
                    "particle".to_string(),
                    Slot::Reference("missingno".to_string()),
                ),
                (
                    "missingno".to_string(),
                    Slot::Material {
                        sprite: "minecraft:missingno".to_string(),
                        force_translucent: false,
                    },
                ),
            ],
            elements: Some(vec![Some(ElementJson {
                from: [0.0; 3],
                to: [16.0; 3],
                rotation: None,
                shade: true,
                faces: faces.into(),
            })]),
        }
    }

    /// `CuboidModel$Deserializer`. Was er ablehnt, macht die Datei kaputt,
    /// auch in `ambientocclusion`, `display` und `gui_light`, die der
    /// Renderer sonst nicht braucht. Unbekannte Schlüssel übergeht er.
    pub(super) fn read(json: &Value) -> Result<ModelFile> {
        let model = object(json, "das Modell")?;
        let elements = match model.get("elements") {
            None => None,
            Some(list) => Some(
                array(list, "elements")?
                    .iter()
                    .enumerate()
                    .map(|(i, element)| {
                        parse_element(element).with_context(|| format!("Element {i}"))
                    })
                    .collect::<Result<_>>()?,
            ),
        };
        let parent = string_or(model, "parent", "")?;
        let textures = match model.get("textures") {
            None => Vec::new(),
            Some(table) => object(table, "textures")?
                .iter()
                .map(|(name, value)| {
                    let slot = parse_slot(value).with_context(|| format!("Textur {name}"))?;
                    Ok((name.clone(), slot))
                })
                .collect::<Result<_>>()?,
        };
        if let Some(value) = model.get("ambientocclusion") {
            boolean(value, "ambientocclusion")?;
        }
        if let Some(value) = model.get("display") {
            parse_display(value)?;
        }
        if let Some(value) = model.get("gui_light") {
            let light = string(value, "gui_light")?;
            ensure!(
                light == "front" || light == "side",
                "gui_light {light}, erlaubt sind front und side"
            );
        }
        let parent = (!parent.is_empty())
            .then(|| identifier(&parent))
            .transpose()?;
        Ok(ModelFile {
            parent,
            textures,
            elements,
        })
    }
}

/// `CuboidModelElement$Deserializer`.
fn parse_element(value: &Value) -> Result<Option<ElementJson>> {
    if value.is_null() {
        return Ok(None);
    }
    let element = object(value, "das Element")?;
    let position = |name: &str| -> Result<[f32; 3]> {
        let position = vector(required(element, name)?, name)?;
        ensure!(
            position.iter().all(|c| (-16.0..=32.0).contains(c)),
            "{name} {position:?} liegt nicht zwischen -16 und 32"
        );
        Ok(position)
    };
    let from = position("from")?;
    let to = position("to")?;
    let rotation = element.get("rotation").map(parse_rotation).transpose()?;
    let sides = object(required(element, "faces")?, "faces")?;
    ensure!(!sides.is_empty(), "ein Element ohne Seiten");
    let faces = sides
        .iter()
        .map(|(name, value)| {
            let side = Face::parse(name).ok_or_else(|| anyhow!("unbekannte Seite {name}"))?;
            let face = match value {
                Value::Null => None,
                value => Some(parse_face(value).with_context(|| format!("Seite {name}"))?),
            };
            Ok((side, face))
        })
        .collect::<Result<_>>()?;
    let shade = match element.get("shade") {
        None => true,
        Some(Value::Bool(shade)) => *shade,
        Some(_) => bail!("shade ist kein Wahrheitswert"),
    };
    if let Some(value) = element.get("light_emission") {
        let light = match value {
            Value::Number(number) => int_value(number)?,
            _ => bail!("light_emission ist keine Zahl"),
        };
        ensure!(
            (0..=15).contains(&light),
            "light_emission {light} liegt nicht zwischen 0 und 15"
        );
    }
    Ok(Some(ElementJson {
        from,
        to,
        rotation,
        shade,
        faces,
    }))
}

/// `CuboidFace$Deserializer`: eine unbekannte `cullface` heisst keine, die
/// Drehung ist modulo 360 eine von 0, 90, 180 und 270.
fn parse_face(value: &Value) -> Result<FaceJson> {
    let face = object(value, "die Seite")?;
    let cullface = Face::parse(&string_or(face, "cullface", "")?);
    let tint_index = int_or(face, "tintindex", -1)?;
    let texture = string(required(face, "texture")?, "texture")?;
    let uv = match face.get("uv") {
        None => None,
        Some(value) => {
            let uv = array(value, "uv")?;
            ensure!(uv.len() == 4, "uv hat {} statt 4 Werte", uv.len());
            Some([
                float(&uv[0], "uv")?,
                float(&uv[1], "uv")?,
                float(&uv[2], "uv")?,
                float(&uv[3], "uv")?,
            ])
        }
    };
    let degrees = int_or(face, "rotation", 0)?;
    let rotation = degrees.rem_euclid(360);
    ensure!(
        rotation % 90 == 0,
        "Drehung {degrees}, erlaubt sind 0, 90, 180 und 270"
    );
    Ok(FaceJson {
        texture,
        uv,
        cullface,
        rotation: rotation as u16,
        tint_index,
    })
}

/// `getRotation`: `origin` muss dastehen. `axis` und `angle` gehen vor `x`,
/// `y` und `z`, und die Achse gilt in jeder Schreibweise. Vanilla mischt
/// beides innerhalb einer Datei, etwa in block/template_hanging_sign_rot_3.
fn parse_rotation(value: &Value) -> Result<Rotation> {
    let rotation = object(value, "rotation")?;
    let origin = vector(required(rotation, "origin")?, "origin")?;
    let angles = if rotation.contains_key("axis") || rotation.contains_key("angle") {
        let axis = string(required(rotation, "axis")?, "axis")?.to_lowercase();
        let angle = float(required(rotation, "angle")?, "angle")?;
        match axis.as_str() {
            "x" => [angle, 0.0, 0.0],
            "y" => [0.0, angle, 0.0],
            "z" => [0.0, 0.0, angle],
            _ => bail!("unbekannte Achse {axis}"),
        }
    } else if ["x", "y", "z"]
        .iter()
        .any(|axis| rotation.contains_key(*axis))
    {
        [
            float_or(rotation, "x", 0.0)?,
            float_or(rotation, "y", 0.0)?,
            float_or(rotation, "z", 0.0)?,
        ]
    } else {
        bail!("rotation ohne axis und angle oder x, y und z");
    };
    let rescale = match rotation.get("rescale") {
        None => false,
        Some(value) => boolean(value, "rescale")?,
    };
    Ok(Rotation {
        origin,
        angles,
        rescale,
    })
}

/// `TextureSlots.parseEntry`: Text mit `#` vorn verweist auf einen Slot,
/// sonst gilt `Material.CODEC`, ein Name oder `{"sprite", "force_translucent"}`.
/// Das Objekt liest DFU, dort zählt `null` als fehlend.
fn parse_slot(value: &Value) -> Result<Slot> {
    match value {
        Value::String(name) => match name.strip_prefix('#') {
            Some(target) => Ok(Slot::Reference(target.to_string())),
            // `isTextureReference` liest das erste Zeichen.
            None if name.is_empty() => bail!("leerer Texturname"),
            None => Ok(Slot::Material {
                sprite: identifier(name)?,
                force_translucent: false,
            }),
        },
        Value::Object(_) => {
            let sprite = match field(value, "sprite") {
                Some(Value::String(name)) => identifier(name)?,
                Some(_) => bail!("sprite ist kein Text"),
                None => bail!("sprite fehlt"),
            };
            let force_translucent = match field(value, "force_translucent") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => bail!("force_translucent ist kein Wahrheitswert"),
            };
            Ok(Slot::Material {
                sprite,
                force_translucent,
            })
        }
        _ => bail!("weder Text noch Objekt"),
    }
}

/// `ItemTransforms$Deserializer`: je Ansicht ein Objekt, darin `rotation`,
/// `translation` und `scale` mit genau drei Zahlen. Eine Ansicht `null`
/// lässt Gson durch, eine unbekannte übergeht es.
fn parse_display(value: &Value) -> Result<()> {
    let display = object(value, "display")?;
    for view in [
        "thirdperson_righthand",
        "thirdperson_lefthand",
        "firstperson_righthand",
        "firstperson_lefthand",
        "head",
        "gui",
        "ground",
        "fixed",
        "on_shelf",
    ] {
        let Some(transform) = display.get(view).filter(|t| !t.is_null()) else {
            continue;
        };
        let transform = object(transform, view)?;
        for name in ["rotation", "translation", "scale"] {
            if let Some(value) = transform.get(name) {
                vector(value, name).with_context(|| format!("display {view}"))?;
            }
        }
    }
    Ok(())
}

// ------------------------------------------------ Felder wie `GsonHelper`

/// Die Felder einer Modelldatei liest Gson, nicht DFU: ein Feld mit `null`
/// ist da, und die Umwandlung scheitert dann.
fn required<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value> {
    object.get(name).ok_or_else(|| anyhow!("{name} fehlt"))
}

fn object<'a>(value: &'a Value, what: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| anyhow!("{what} ist kein Objekt"))
}

fn array<'a>(value: &'a Value, what: &str) -> Result<&'a [Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow!("{what} ist keine Liste"))
}

/// `convertToString`: jedes Primitiv, eine Zahl so, wie sie dasteht.
// ponytail: serde_json liest `-0` als `0` und `1E5` als `1e+5`. Das zählt
// nur bei Zahlen als Namen.
fn string(value: &Value, what: &str) -> Result<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => bail!("{what} ist kein Text"),
    }
}

fn string_or(object: &Map<String, Value>, name: &str, default: &str) -> Result<String> {
    object
        .get(name)
        .map_or_else(|| Ok(default.to_string()), |value| string(value, name))
}

/// `convertToInt`: nur eine Zahl, abgeschnitten wie `intValue`.
fn int_or(object: &Map<String, Value>, name: &str, default: i32) -> Result<i32> {
    match object.get(name) {
        None => Ok(default),
        Some(Value::Number(number)) => int_value(number).with_context(|| name.to_string()),
        Some(_) => bail!("{name} ist keine Zahl"),
    }
}

/// `convertToFloat`: nur eine Zahl, gerundet wie `Float.parseFloat`, eine
/// zu grosse also unendlich.
fn float(value: &Value, what: &str) -> Result<f32> {
    match value {
        Value::Number(number) => number
            .as_str()
            .parse()
            .with_context(|| format!("{what} {number}")),
        _ => bail!("{what} ist keine Zahl"),
    }
}

fn float_or(object: &Map<String, Value>, name: &str, default: f32) -> Result<f32> {
    object
        .get(name)
        .map_or(Ok(default), |value| float(value, name))
}

/// `convertToBoolean`: `JsonPrimitive.getAsBoolean`, Text also über
/// `Boolean.parseBoolean` und eine Zahl als falsch.
fn boolean(value: &Value, what: &str) -> Result<bool> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::String(text) => Ok(text.eq_ignore_ascii_case("true")),
        Value::Number(_) => Ok(false),
        _ => bail!("{what} ist kein Wahrheitswert"),
    }
}

/// `getVector3f`: genau drei Zahlen.
fn vector(value: &Value, what: &str) -> Result<[f32; 3]> {
    let list = array(value, what)?;
    ensure!(list.len() == 3, "{what} hat {} statt 3 Werte", list.len());
    Ok([
        float(&list[0], what)?,
        float(&list[1], what)?,
        float(&list[2], what)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, Slot> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), parse_slot(&Value::from(*v)).unwrap()))
            .collect()
    }

    /// Wie `TextureSlots.getMaterial`, gegengeprüft mit den echten Klassen
    /// aus 26.2: ein `#` vorn ist frei, ein zweites gehört zum Namen.
    #[test]
    fn flaechentextur_ist_immer_ein_slot() {
        let textures = map(&[
            ("side", "#all"),
            ("all", "block/stone"),
            ("", "block/leer"),
            ("h", "##x"),
            ("#x", "block/raute"),
        ]);
        let sprite = |name: &str| resolve_texture(name, &textures).map(|(s, _)| s.to_string());
        assert_eq!(sprite("#side").as_deref(), Some("minecraft:block/stone"));
        assert_eq!(sprite("side").as_deref(), Some("minecraft:block/stone"));
        assert_eq!(sprite("all").as_deref(), Some("minecraft:block/stone"));
        assert_eq!(sprite("##all"), None);
        assert_eq!(sprite("block/stone"), None);
        assert_eq!(sprite("#").as_deref(), Some("minecraft:block/leer"));
        assert_eq!(sprite("h").as_deref(), Some("minecraft:block/raute"));
        assert_eq!(sprite("#x"), None);
    }

    /// Der Client löst bis zum Fixpunkt auf, auch über mehr als sieben
    /// Schritte; ein Zyklus bleibt ungelöst.
    #[test]
    fn lange_ketten_und_zyklen() {
        let mut pairs: Vec<(String, String)> = (1..=12)
            .map(|i| (format!("s{i}"), format!("#s{}", i - 1)))
            .collect();
        pairs.push(("s0".into(), "block/ende".into()));
        pairs.push(("a".into(), "#b".into()));
        pairs.push(("b".into(), "#a".into()));
        pairs.push(("selbst".into(), "#selbst".into()));
        let pairs: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let textures = map(&pairs);
        assert_eq!(
            resolve_texture("#s12", &textures),
            Some(("minecraft:block/ende", false))
        );
        assert_eq!(resolve_texture("#a", &textures), None);
        assert_eq!(resolve_texture("#selbst", &textures), None);
        assert_eq!(resolve_texture("#weg", &textures), None);
    }

    #[test]
    fn force_translucent_ueberlebt_die_kette() {
        let mut textures = map(&[("all", "#glas")]);
        textures.insert(
            "glas".to_string(),
            parse_slot(&serde_json::json!({"sprite": "block/glass", "force_translucent": true}))
                .unwrap(),
        );
        assert_eq!(
            resolve_texture("#all", &textures),
            Some(("minecraft:block/glass", true))
        );
    }

    fn rotation(json: &str) -> Result<Rotation> {
        parse_rotation(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn klassische_rotation() {
        let r = rotation(r#"{"origin": [8, 0, 8], "axis": "y", "angle": -22.5}"#).unwrap();
        assert_eq!(r.angles, [0.0, -22.5, 0.0]);
        assert_eq!(r.origin, [8.0, 0.0, 8.0]);
        assert!(!r.rescale);
    }

    /// Vanilla 26.2 nutzt das in block/template_hanging_sign_rot_3 und das
    /// TerraNova-Pack in bvb_template_sign_rot_3.
    #[test]
    fn mehrachsen_rotation() {
        let r = rotation(r#"{"x": 180, "y": -67.5, "z": -180, "origin": [8, 0, 8]}"#).unwrap();
        assert_eq!(r.angles, [180.0, -67.5, -180.0]);
        assert_eq!(r.origin, [8.0, 0.0, 8.0]);
    }

    #[test]
    fn klassische_schreibweise_hat_vorrang() {
        let r = rotation(r#"{"origin": [8, 8, 8], "axis": "y", "angle": 45, "x": 180, "z": 90}"#)
            .unwrap();
        assert_eq!(r.angles, [0.0, 45.0, 0.0]);
    }

    /// Wie `CuboidModelElement$Deserializer`, gegengeprüft mit den echten
    /// Klassen aus 26.2: die Achse in jeder Schreibweise, `rescale` auch als
    /// Text, aber ohne `origin` oder ohne Winkel ist die Datei kaputt.
    #[test]
    fn rotation_wie_im_client() {
        let r = rotation(r#"{"origin": [8, 8, 8], "axis": "Y", "angle": 45, "rescale": "TRUE"}"#)
            .unwrap();
        assert_eq!(r.angles, [0.0, 45.0, 0.0]);
        assert!(r.rescale);
        for json in [
            r#"{"origin": [8, 8, 8]}"#,
            r#"{"axis": "y", "angle": 45}"#,
            r#"{"origin": [8, 8, 8], "angle": 45}"#,
            r#"{"origin": [8, 8, 8], "axis": "y"}"#,
            r#"{"origin": [8, 8, 8], "axis": "w", "angle": 45}"#,
            r#"{"origin": [8, 8], "x": 10}"#,
            r#"{"origin": [8, 8, 8], "x": "10"}"#,
            r#"{"origin": [8, 8, 8], "x": 10, "rescale": null}"#,
            "null",
        ] {
            assert!(rotation(json).is_err(), "{json}");
        }
    }

    fn element(json: &str) -> Result<Option<ElementJson>> {
        parse_element(&serde_json::from_str(json).unwrap())
    }

    /// Flächen wie `CuboidFace$Deserializer`: Zahlen wie `intValue` und
    /// `floorMod`, Texturnamen aus jedem Primitiv.
    #[test]
    fn seiten_wie_im_client() {
        let seite = |face: &str| {
            let json =
                format!(r#"{{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {{"up": {face}}}}}"#);
            element(&json).map(|e| e.unwrap().faces.into_iter().next().unwrap().1.unwrap())
        };
        for (drehung, soll) in [("-90", 270), ("90.0", 90), ("450", 90), ("4294967386", 90)] {
            let face = seite(&format!(r##"{{"texture": "#a", "rotation": {drehung}}}"##)).unwrap();
            assert_eq!(face.rotation, soll, "{drehung}");
        }
        for (index, soll) in [("0.0", 0), ("-5", -5), ("1e10", 1410065408)] {
            let face = seite(&format!(r##"{{"texture": "#a", "tintindex": {index}}}"##)).unwrap();
            assert_eq!(face.tint_index, soll, "{index}");
        }
        for (texture, soll) in [
            ("5", "5"),
            ("1.50", "1.50"),
            ("true", "true"),
            (r#""""#, ""),
        ] {
            let face = seite(&format!(r#"{{"texture": {texture}}}"#)).unwrap();
            assert_eq!(face.texture, soll);
        }
        for (cullface, soll) in [
            (r#""north""#, Some(Face::North)),
            (r#""NORTH""#, None),
            ("5", None),
        ] {
            let face = seite(&format!(r##"{{"texture": "#a", "cullface": {cullface}}}"##)).unwrap();
            assert_eq!(face.cullface, soll, "{cullface}");
        }
        let face = seite(r##"{"texture": "#a", "uv": [0, 0, 1e400, 16]}"##).unwrap();
        assert_eq!(face.uv, Some([0.0, 0.0, f32::INFINITY, 16.0]));
        for face in [
            r##"{"texture": "#a", "rotation": 45}"##,
            r##"{"texture": "#a", "rotation": "90"}"##,
            r##"{"texture": "#a", "rotation": null}"##,
            r##"{"texture": "#a", "tintindex": null}"##,
            r##"{"texture": "#a", "tintindex": true}"##,
            r##"{"texture": "#a", "tintindex": 1e10000}"##,
            r##"{"texture": "#a", "cullface": null}"##,
            r##"{"texture": "#a", "uv": [0, 0, 16]}"##,
            r##"{"texture": "#a", "uv": null}"##,
            r##"{"texture": "#a", "uv": ["0", 0, 16, 16]}"##,
            r#"{"texture": null}"#,
            "{}",
            r#""x""#,
        ] {
            assert!(seite(face).is_err(), "{face}");
        }
    }

    /// Elemente wie `CuboidModelElement$Deserializer`, gegengeprüft mit den
    /// echten Klassen aus 26.2.
    #[test]
    fn elemente_wie_im_client() {
        let seiten = r##""faces": {"up": {"texture": "#a"}}"##;
        let kiste = |extra: &str| {
            element(&format!(
                r#"{{"from": [0, 0, 0], "to": [16, 16, 16], {seiten}{extra}}}"#
            ))
        };
        assert!(!kiste(r#", "shade": false"#).unwrap().unwrap().shade);
        assert!(kiste(r#", "light_emission": 1.5"#).is_ok());
        assert!(element("null").unwrap().is_none());
        let null_seite =
            element(r#"{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"up": null}}"#).unwrap();
        assert!(null_seite.unwrap().faces[0].1.is_none());
        for json in [
            format!(r#"{{"from": [-17, 0, 0], "to": [16, 16, 16], {seiten}}}"#),
            format!(r#"{{"from": [0, 0, 0], "to": [32.0001, 16, 16], {seiten}}}"#),
            format!(r#"{{"from": [0, 0], "to": [16, 16, 16], {seiten}}}"#),
            format!(r#"{{"from": ["0", 0, 0], "to": [16, 16, 16], {seiten}}}"#),
            r#"{"from": [0, 0, 0], "to": [16, 16, 16]}"#.to_string(),
            r#"{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {}}"#.to_string(),
            r##"{"from": [0, 0, 0], "to": [16, 16, 16], "faces": {"North": {"texture": "#a"}}}"##
                .to_string(),
            r#""x""#.to_string(),
        ]
        .into_iter()
        .chain(
            [
                r#", "shade": "true""#,
                r#", "shade": null"#,
                r#", "light_emission": 16"#,
                r#", "light_emission": "3""#,
                r#", "rotation": null"#,
            ]
            .map(|extra| format!(r#"{{"from": [0, 0, 0], "to": [16, 16, 16], {seiten}{extra}}}"#)),
        ) {
            assert!(element(&json).is_err(), "{json}");
        }
    }

    /// Das Modell selbst wie `CuboidModel$Deserializer`, gegengeprüft mit den
    /// echten Klassen aus 26.2: auch Felder, die der Renderer nicht braucht,
    /// machen die Datei kaputt.
    #[test]
    fn modelldatei_wie_im_client() {
        let lies = |json: &str| ModelFile::read(&serde_json::from_str(json).unwrap());
        for json in [
            r#"{"ambientocclusion": "yes"}"#,
            r#"{"display": {"gui": null, "foo": 1}}"#,
            r#"{"gui_light": "front"}"#,
            r#"{"textures": {"a": {"sprite": "a:b", "force_translucent": null, "extra": [1]}}}"#,
            r##"{"textures": {"a": "#"}}"##,
            r#"{"parent": ""}"#,
        ] {
            assert!(lies(json).is_ok(), "{json}");
        }
        for json in [
            r#"{"ambientocclusion": null}"#,
            r#"{"display": {"gui": {"rotation": [1, 2]}}}"#,
            r#"{"display": {"gui": 5}}"#,
            r#"{"display": 5}"#,
            r#"{"display": {"gui": {"scale": null}}}"#,
            r#"{"gui_light": "FRONT"}"#,
            r#"{"gui_light": null}"#,
            r#"{"textures": null}"#,
            r#"{"textures": {"a": ""}}"#,
            r##"{"textures": {"a": {"sprite": "#x"}}}"##,
            r#"{"textures": {"a": "Block/X"}}"#,
            r#"{"textures": {"a": {"force_translucent": true}}}"#,
            r#"{"textures": {"a": ["a:b"]}}"#,
            r#"{"textures": {"a": 5}}"#,
            r#"{"elements": null}"#,
            r#"{"parent": null}"#,
            r#"{"parent": "Bad Name"}"#,
            "[]",
        ] {
            assert!(lies(json).is_err(), "{json}");
        }
        let m = lies(r#"{"parent": 5, "elements": []}"#).unwrap();
        assert_eq!(m.parent.as_deref(), Some("minecraft:5"));
        assert_eq!(m.elements.map(|e| e.len()), Some(0));
    }

    #[test]
    fn seitennamen() {
        assert_eq!(Face::parse("north"), Some(Face::North));
        assert_eq!(Face::parse("nordwest"), None);
        assert_eq!(Face::North.name(), "north");
    }
}
