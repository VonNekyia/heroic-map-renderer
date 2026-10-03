//! Der Typ der Dimension, die ein Lauf zeichnet, und was der Renderer von
//! ihm liest. Die Typen des Spiels stehen in `dimensionstypen.txt`, die
//! `Dimensionstypen.java` schreibt; Datenwurzeln liegen darüber.
//! Siehe docs/renderer/dimensionstypen.md.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::Value;

use super::blockstate::{boolean, field, float};
use super::colors::{Tint, color};
use super::model::Face;
use super::pack::{self, Pack};
use super::{parse_json, read_text};

/// Wie das Spiel die Seiten eines Blocks nach ihrer Richtung abschattiert,
/// `CardinalLighting.Type` in 26.2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardinalLight {
    #[default]
    Default,
    Nether,
}

impl CardinalLight {
    /// Die Helligkeit einer Seite, `CardinalLighting.DEFAULT` und
    /// `CardinalLighting.NETHER`.
    pub fn face(self, face: Face) -> f32 {
        match (self, face) {
            (CardinalLight::Default, Face::Down) => 0.5,
            (CardinalLight::Default, Face::Up) => 1.0,
            (CardinalLight::Nether, Face::Down | Face::Up) => 0.9,
            (_, Face::North | Face::South) => 0.8,
            (_, Face::West | Face::East) => 0.6,
        }
    }

    /// Die Richtungen des Lichts für Entity-Modelle vor dem Normieren, wie
    /// `Lighting.updateLevel` sie setzt: im Nether kommt das zweite von
    /// unten.
    pub fn entity_light(self) -> [[f32; 3]; 2] {
        match self {
            CardinalLight::Default => [[0.2, 1.0, -0.7], [-0.2, 1.0, 0.7]],
            CardinalLight::Nether => [[0.2, 1.0, -0.7], [-0.2, -1.0, 0.7]],
        }
    }
}

/// Was der Renderer vom Typ einer Dimension liest. Was der Typ nicht setzt,
/// hat die Vorgabe des Spiels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionType {
    /// `has_skylight`.
    pub has_skylight: bool,
    /// `cardinal_light`.
    pub cardinal_light: CardinalLight,
    /// `visual/ambient_light_color`.
    pub ambient_light_color: Tint,
    /// `visual/sky_light_factor`.
    pub sky_light_factor: f32,
    /// `visual/sky_light_color`.
    pub sky_light_color: Tint,
    /// `visual/block_light_tint`.
    pub block_light_tint: Tint,
    /// `visual/sky_color`, die Farbe des Himmels, wo ein Biom keine setzt.
    pub sky_color: Tint,
    /// `visual/fog_color`, die Farbe des Nebels, wo ein Biom keine setzt.
    pub fog_color: Tint,
    /// `visual/water_fog_color`, die Farbe des Nebels unter Wasser, wo ein
    /// Biom keine setzt.
    pub water_fog_color: Tint,
}

impl DimensionType {
    /// Der Typ `id` aus der Tabelle des Spiels, ohne die Datenwurzeln.
    pub fn des_spiels(id: &str) -> Option<DimensionType> {
        TABELLE.typen.get(id).copied()
    }

    /// Der Typ der Oberwelt aus der Tabelle des Spiels.
    pub fn oberwelt() -> DimensionType {
        DimensionType::des_spiels("minecraft:overworld").expect("in der Tabelle")
    }
}

/// Die Typen und Dimensionen aus den Datenwurzeln, und welche Dimension
/// der Lauf zeichnet.
#[derive(Default)]
pub(super) struct Dimensionen {
    /// Die Dimensionstypen je ID, spätere Wurzeln überschreiben frühere.
    typen: BTreeMap<String, DimensionType>,
    /// Je Dimension ihr Typ, als ID oder direkt in ihrer Definition.
    dimensionen: BTreeMap<String, Verweis>,
    /// Dateien, die der Codec ablehnt, je Pfad mit dem Grund.
    pub(super) kaputt: BTreeMap<String, String>,
    /// Attribute mit Modifikator, die der Renderer nicht rechnet: je Pfad
    /// das Attribut.
    pub(super) modifikatoren: BTreeMap<String, String>,
    /// Der Typ der Dimension, die der Lauf zeichnet, siehe
    /// [`Dimensionen::festlegen`].
    gewaehlt: Option<DimensionType>,
}

enum Verweis {
    Id(String),
    Direkt(DimensionType),
}

/// Die drei Dimensionen des Spiels haben ohne eigene Definition den Typ
/// gleichen Namens, wie in jeder Voreinstellung des Spiels.
const DES_SPIELS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];

impl Dimensionen {
    /// Liest `<dir>/<namespace>/dimension_type/**/*.json` und
    /// `<dir>/<namespace>/dimension/**/*.json`, aufgelistet wie die Biome
    /// ([`Pack`]). Liefert, wie viele Typen und Dimensionen es gelesen hat.
    pub(super) fn laden(&mut self, dir: &Path) -> Result<usize> {
        let pack = Pack::open(dir, &pack::DIMENSION)?;
        let mut gelesen = 0;
        for (name, pfad) in pack.files() {
            let Some((namespace, rest)) = name.split_once('/') else {
                continue;
            };
            let Some((art, id)) = rest
                .split_once('/')
                .and_then(|(art, id)| Some((art, id.strip_suffix(".json")?)))
            else {
                continue;
            };
            let id = format!("{namespace}:{id}");
            let json = read_text(pfad).and_then(|text| parse_json(&text, true));
            let mut modifikator = None;
            let ergebnis = json.and_then(|json| {
                if art == "dimension_type" {
                    self.typen.insert(id, typ(&json, &mut modifikator)?);
                } else {
                    self.dimensionen
                        .insert(id, dimension(&json, &mut modifikator)?);
                }
                Ok(())
            });
            match ergebnis {
                Ok(()) => gelesen += 1,
                Err(grund) => {
                    self.kaputt
                        .insert(pfad.display().to_string(), format!("{grund:#}"));
                }
            }
            if let Some(attribut) = modifikator {
                self.modifikatoren
                    .insert(pfad.display().to_string(), attribut);
            }
        }
        Ok(gelesen)
    }

    /// Legt fest, welche Dimension der Lauf zeichnet, `None` ohne
    /// Weltwurzel. Liefert eine Meldung, wenn dafür der Typ der Oberwelt
    /// gilt.
    pub(super) fn festlegen(&mut self, dimension: Option<&str>) -> Option<String> {
        let (typ, meldung) = self.aufloesen(dimension);
        self.gewaehlt = Some(typ);
        meldung
    }

    /// Der Typ der Dimension aus [`Dimensionen::festlegen`], ohne sie der
    /// der Oberwelt.
    pub(super) fn gewaehlt(&self) -> DimensionType {
        self.gewaehlt
            .unwrap_or_else(|| self.typ_zu("minecraft:overworld").expect("in der Tabelle"))
    }

    fn aufloesen(&self, dimension: Option<&str>) -> (DimensionType, Option<String>) {
        let oberwelt = self.typ_zu("minecraft:overworld").expect("in der Tabelle");
        let Some(dimension) = dimension else {
            return (
                oberwelt,
                Some("keine Weltwurzel, also keine Dimension: der Typ der Oberwelt".into()),
            );
        };
        // Wie `WorldDimensions.bake`: erst die Datenpakete.
        let typ_id = match self.dimensionen.get(dimension) {
            Some(Verweis::Direkt(typ)) => return (*typ, None),
            Some(Verweis::Id(id)) => id.clone(),
            None if DES_SPIELS.contains(&dimension) => dimension.to_string(),
            None => {
                return (
                    oberwelt,
                    Some(format!(
                        "{dimension} steht in keiner Datenwurzel: der Typ der Oberwelt"
                    )),
                );
            }
        };
        match self.typ_zu(&typ_id) {
            Some(typ) => (typ, None),
            None => (
                oberwelt,
                Some(format!(
                    "{dimension} hat den Typ {typ_id}, den weder das Spiel noch eine Datenwurzel kennt: der Typ der Oberwelt"
                )),
            ),
        }
    }

    /// Ein Typ aus den Datenwurzeln, sonst aus der Tabelle des Spiels.
    fn typ_zu(&self, id: &str) -> Option<DimensionType> {
        self.typen
            .get(id)
            .or_else(|| TABELLE.typen.get(id))
            .copied()
    }
}

/// Liest einen Dimensionstyp wie `DimensionType.DIRECT_CODEC` in 26.2, so
/// weit der Renderer ihn braucht: `has_skylight` ist Pflicht,
/// `cardinal_light` darf fehlen (`default`), ebenso `attributes`. Von den
/// Attributen zählen die aus [`ATTRIBUTE`]. Steht bei einem statt des
/// Werts ein Modifikator, bleibt die Vorgabe, und `modifikator` nennt es.
fn typ(json: &Value, modifikator: &mut Option<String>) -> Result<DimensionType> {
    ensure!(json.is_object(), "kein Objekt");
    let has_skylight = field(json, "has_skylight").ok_or_else(|| anyhow!("has_skylight fehlt"))?;
    let mut typ = DimensionType {
        has_skylight: boolean(has_skylight).context("has_skylight")?,
        cardinal_light: match field(json, "cardinal_light") {
            None => CardinalLight::Default,
            Some(wert) => cardinal_light(wert).context("cardinal_light")?,
        },
        ..TABELLE.vorgabe
    };
    if let Some(attribute) = field(json, "attributes") {
        let attribute = attribute
            .as_object()
            .ok_or_else(|| anyhow!("attributes ist kein Objekt"))?;
        for (id, wert) in attribute {
            let id = mit_namensraum(id);
            if wert.get("modifier").is_some() && ATTRIBUTE.contains(&id.as_str()) {
                *modifikator = Some(id);
                continue;
            }
            setze(&mut typ, &id, wert).with_context(|| id.clone())?;
        }
    }
    Ok(typ)
}

/// Die Attribute, die der Renderer liest: die vier der Lightmap und die
/// drei Farben des Himmels für Cinematic.
const ATTRIBUTE: [&str; 7] = [
    "minecraft:visual/ambient_light_color",
    "minecraft:visual/sky_light_factor",
    "minecraft:visual/sky_light_color",
    "minecraft:visual/block_light_tint",
    "minecraft:visual/sky_color",
    "minecraft:visual/fog_color",
    "minecraft:visual/water_fog_color",
];

/// Setzt eines der Attribute aus [`ATTRIBUTE`] auf seinen Wert, wie ihn
/// `EnvironmentAttribute.valueCodec` liest: Farben wie
/// `ExtraCodecs.STRING_RGB_COLOR`, `sky_light_factor` von 0 bis 1
/// (`AttributeRange.UNIT_FLOAT`). Andere Attribute lässt es aus.
fn setze(typ: &mut DimensionType, id: &str, wert: &Value) -> Result<()> {
    match id {
        "minecraft:visual/ambient_light_color" => typ.ambient_light_color = color(wert)?,
        "minecraft:visual/sky_light_color" => typ.sky_light_color = color(wert)?,
        "minecraft:visual/block_light_tint" => typ.block_light_tint = color(wert)?,
        "minecraft:visual/sky_color" => typ.sky_color = color(wert)?,
        "minecraft:visual/fog_color" => typ.fog_color = color(wert)?,
        "minecraft:visual/water_fog_color" => typ.water_fog_color = color(wert)?,
        "minecraft:visual/sky_light_factor" => {
            let faktor = float(wert)?;
            ensure!(
                (0.0..=1.0).contains(&faktor),
                "{faktor} liegt nicht in 0 bis 1"
            );
            typ.sky_light_factor = faktor;
        }
        _ => {}
    }
    Ok(())
}

/// `CardinalLighting.Type.CODEC`: `default` oder `nether`.
fn cardinal_light(json: &Value) -> Result<CardinalLight> {
    match json.as_str() {
        Some("default") => Ok(CardinalLight::Default),
        Some("nether") => Ok(CardinalLight::Nether),
        _ => bail!("{json} ist weder default noch nether"),
    }
}

/// Der Typ einer Dimension aus ihrer Definition (`LevelStem.CODEC`): `type`
/// ist die ID eines Typs oder der Typ selbst.
fn dimension(json: &Value, modifikator: &mut Option<String>) -> Result<Verweis> {
    ensure!(json.is_object(), "kein Objekt");
    match field(json, "type").ok_or_else(|| anyhow!("type fehlt"))? {
        Value::String(id) => Ok(Verweis::Id(mit_namensraum(id))),
        direkt => Ok(Verweis::Direkt(typ(direkt, modifikator).context("type")?)),
    }
}

/// Eine ID wie `Identifier.parse`: ohne Namensraum, oder mit leerem vor dem
/// Doppelpunkt, liegt sie unter `minecraft`.
pub(super) fn mit_namensraum(id: &str) -> String {
    match id.split_once(':') {
        Some((namensraum, _)) if !namensraum.is_empty() => id.to_string(),
        Some((_, pfad)) => format!("minecraft:{pfad}"),
        None => format!("minecraft:{id}"),
    }
}

/// `dimensionstypen.txt`, gelesen. Aufbau siehe `Dimensionstypen.java`.
struct Tabelle {
    /// Nur die Vorgaben der Attribute aus `EnvironmentAttributes`, dazu
    /// `cardinal_light` wie ohne Angabe.
    vorgabe: DimensionType,
    typen: BTreeMap<String, DimensionType>,
}

static TABELLE: LazyLock<Tabelle> = LazyLock::new(|| lesen(include_str!("dimensionstypen.txt")));

fn lesen(text: &str) -> Tabelle {
    let mut tabelle = Tabelle {
        vorgabe: DimensionType {
            has_skylight: true,
            cardinal_light: CardinalLight::Default,
            ambient_light_color: [0; 3],
            sky_light_factor: 0.0,
            sky_light_color: [0; 3],
            block_light_tint: [0; 3],
            sky_color: [0; 3],
            fog_color: [0; 3],
            water_fog_color: [0; 3],
        },
        typen: BTreeMap::new(),
    };
    for zeile in text.lines() {
        zeile_lesen(&mut tabelle, zeile)
            .unwrap_or_else(|grund| panic!("dimensionstypen.txt: {zeile}: {grund:#}"));
    }
    tabelle
}

/// Eine Zeile der Tabelle. Ihre Werte stehen, wie ein Datenpaket sie
/// schreibt: Farben als `#rrggbb`, sonst als JSON.
fn zeile_lesen(tabelle: &mut Tabelle, zeile: &str) -> Result<()> {
    let wert = |text: &str| -> Result<Value> {
        match text.starts_with('#') {
            true => Ok(Value::String(text.to_string())),
            false => serde_json::from_str(text).with_context(|| text.to_string()),
        }
    };
    match zeile.split(' ').collect::<Vec<_>>().as_slice() {
        ["vorgabe", id, text] => setze(&mut tabelle.vorgabe, id, &wert(text)?),
        ["typ", id, felder @ ..] => {
            let mut typ = tabelle.vorgabe;
            for feld in felder {
                let (name, text) = feld
                    .split_once('=')
                    .ok_or_else(|| anyhow!("{feld} ohne ="))?;
                match name {
                    "has_skylight" => typ.has_skylight = boolean(&wert(text)?)?,
                    "cardinal_light" => {
                        typ.cardinal_light = cardinal_light(&Value::String(text.to_string()))?;
                    }
                    _ => setze(&mut typ, name, &wert(text)?)?,
                }
            }
            tabelle.typen.insert(id.to_string(), typ);
            Ok(())
        }
        _ => bail!("unbekannte Zeile"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEISS: Tint = [255; 3];
    const GELB: Tint = [255, 216, 140];

    /// Die Tabelle wie in 26.2: vier Typen, belegt am Client per javap
    /// (`DimensionTypes.bootstrap`, `EnvironmentAttributes`) und an den
    /// JSON-Dateien unter `data/minecraft/dimension_type` im JAR.
    #[test]
    fn tabelle_wie_im_spiel() {
        let vorgabe = TABELLE.vorgabe;
        assert_eq!(
            (
                vorgabe.ambient_light_color,
                vorgabe.sky_light_factor,
                vorgabe.sky_light_color,
                vorgabe.block_light_tint
            ),
            ([0; 3], 1.0, WEISS, GELB)
        );
        assert_eq!(
            (
                vorgabe.sky_color,
                vorgabe.fog_color,
                vorgabe.water_fog_color
            ),
            ([0; 3], [0; 3], [0x05, 0x05, 0x33])
        );
        let typ = |id: &str| TABELLE.typen[id];
        assert_eq!(TABELLE.typen.len(), 4);
        assert_eq!(
            typ("minecraft:overworld"),
            DimensionType {
                has_skylight: true,
                cardinal_light: CardinalLight::Default,
                ambient_light_color: [10; 3],
                sky_light_factor: 1.0,
                sky_light_color: WEISS,
                block_light_tint: GELB,
                sky_color: [0x78, 0xa7, 0xff],
                fog_color: [0xc0, 0xd8, 0xff],
                water_fog_color: [0x05, 0x05, 0x33],
            }
        );
        assert_eq!(typ("minecraft:overworld_caves"), typ("minecraft:overworld"));
        assert_eq!(
            typ("minecraft:the_nether"),
            DimensionType {
                has_skylight: false,
                cardinal_light: CardinalLight::Nether,
                ambient_light_color: [0x30, 0x28, 0x21],
                sky_light_factor: 0.0,
                sky_light_color: [0x7a, 0x7a, 0xff],
                block_light_tint: GELB,
                sky_color: [0; 3],
                fog_color: [0; 3],
                water_fog_color: [0x05, 0x05, 0x33],
            }
        );
        assert_eq!(
            typ("minecraft:the_end"),
            DimensionType {
                has_skylight: true,
                cardinal_light: CardinalLight::Default,
                ambient_light_color: [0x3f, 0x47, 0x3f],
                sky_light_factor: 0.0,
                sky_light_color: [0xac, 0x60, 0xcd],
                block_light_tint: GELB,
                sky_color: [0; 3],
                fog_color: [0x18, 0x13, 0x18],
                water_fog_color: [0x05, 0x05, 0x33],
            }
        );
    }

    /// `CardinalLighting.DEFAULT` und `.NETHER` in 26.2.
    #[test]
    fn seiten_wie_cardinal_lighting() {
        use CardinalLight::{Default as Oberwelt, Nether};
        for (seite, oberwelt, nether) in [
            (Face::Down, 0.5, 0.9),
            (Face::Up, 1.0, 0.9),
            (Face::North, 0.8, 0.8),
            (Face::South, 0.8, 0.8),
            (Face::West, 0.6, 0.6),
            (Face::East, 0.6, 0.6),
        ] {
            assert_eq!(Oberwelt.face(seite), oberwelt, "{seite:?}");
            assert_eq!(Nether.face(seite), nether, "{seite:?}");
        }
    }

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    /// Wie `DimensionType.DIRECT_CODEC`: `has_skylight` ist Pflicht,
    /// `cardinal_light` ohne Angabe `default`, die Attribute ohne Angabe
    /// ihre Vorgabe. Farben wie `STRING_RGB_COLOR`, der Faktor von 0 bis 1.
    #[test]
    fn typ_wie_der_codec() {
        let lies = |text: &str| typ(&json(text), &mut None);
        let schlicht = lies(r#"{"has_skylight": false}"#).unwrap();
        assert_eq!(
            schlicht,
            DimensionType {
                has_skylight: false,
                ..TABELLE.vorgabe
            }
        );
        assert_eq!(schlicht.cardinal_light, CardinalLight::Default);
        let nether = lies(r#"{"has_skylight": true, "cardinal_light": "nether"}"#).unwrap();
        assert_eq!(nether.cardinal_light, CardinalLight::Nether);
        let bunt = lies(
            r##"{"has_skylight": true, "attributes": {
                "minecraft:visual/ambient_light_color": "#102030",
                "minecraft:visual/sky_light_color": 1056816,
                "minecraft:visual/block_light_tint": [0.5, 0.25, 1.0],
                "minecraft:visual/sky_light_factor": 0.25,
                "minecraft:visual/fog_color": "#ffffff",
                "minecraft:visual/cloud_color": "kein Wert, den der Renderer liest"
            }}"##,
        )
        .unwrap();
        assert_eq!(bunt.fog_color, [255; 3]);
        assert_eq!(bunt.ambient_light_color, [0x10, 0x20, 0x30]);
        assert_eq!(bunt.sky_light_color, [0x10, 0x20, 0x30]);
        assert_eq!(bunt.block_light_tint, [127, 63, 255]);
        assert_eq!(bunt.sky_light_factor, 0.25);
        for kaputt in [
            r#"{}"#,
            r#"{"has_skylight": 1}"#,
            r#"{"has_skylight": true, "cardinal_light": "hell"}"#,
            r#"{"has_skylight": true, "attributes": {"minecraft:visual/sky_light_factor": 1.5}}"#,
            r#"{"has_skylight": true, "attributes": {"minecraft:visual/sky_light_color": "rot"}}"#,
            r#"[]"#,
        ] {
            assert!(lies(kaputt).is_err(), "{kaputt}");
        }

        // Schlüssel ohne Namensraum liest das Spiel als `minecraft:…`
        // (`EnvironmentAttributes.CODEC` ist `byNameCodec`).
        let kurz = lies(
            r##"{"has_skylight": true, "attributes": {
                "visual/ambient_light_color": "#102030",
                ":visual/sky_light_factor": 0.5
            }}"##,
        )
        .unwrap();
        assert_eq!(
            (kurz.ambient_light_color, kurz.sky_light_factor),
            ([0x10, 0x20, 0x30], 0.5)
        );
        assert_eq!(mit_namensraum("beispiel:tief"), "beispiel:tief");

        let mut modifikator = None;
        let text = r##"{"has_skylight": true, "attributes": {
            "minecraft:visual/sky_light_color": {"modifier": "multiply", "argument": "#808080"}
        }}"##;
        let typ = typ(&json(text), &mut modifikator).unwrap();
        assert_eq!(typ.sky_light_color, WEISS, "Vorgabe statt Modifikator");
        assert_eq!(
            modifikator.as_deref(),
            Some("minecraft:visual/sky_light_color")
        );
    }

    fn schreibe(dir: &Path, pfad: &str, text: &str) {
        let pfad = dir.join(pfad);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, text).unwrap();
    }

    /// Welcher Typ zu einer Dimension gehört: der aus ihrer Definition in
    /// den Datenwurzeln, als ID oder direkt, auch für die drei des Spiels;
    /// die haben sonst den Typ gleichen Namens. Jede andere bekommt den der
    /// Oberwelt mit einer Meldung, ebenso ohne Weltwurzel.
    /// Typen aus den Datenwurzeln gehen denen des Spiels vor. Eine ID ohne
    /// Namensraum liegt wie im Spiel unter `minecraft`.
    #[test]
    fn dimension_findet_ihren_typ() {
        let wurzel = tempfile::tempdir().unwrap();
        let dir = wurzel.path();
        schreibe(
            dir,
            "beispiel/dimension_type/tief/dunkel.json",
            r##"{"has_skylight": false, "cardinal_light": "nether",
                "attributes": {"minecraft:visual/ambient_light_color": "#112233"}}"##,
        );
        schreibe(
            dir,
            "beispiel/dimension/hoehle.json",
            r#"{"type": "beispiel:tief/dunkel"}"#,
        );
        schreibe(
            dir,
            "beispiel/dimension/direkt.json",
            r#"{"type": {"has_skylight": true, "cardinal_light": "nether"}}"#,
        );
        schreibe(
            dir,
            "beispiel/dimension/fremd.json",
            r#"{"type": "beispiel:gibt_es_nicht"}"#,
        );
        schreibe(dir, "beispiel/dimension/ohne.json", r#"{"generator": {}}"#);
        schreibe(
            dir,
            "beispiel/dimension/kurz.json",
            r#"{"type": "the_end"}"#,
        );
        schreibe(
            dir,
            "beispiel/dimension_type/kaputt.json",
            r#"{"cardinal_light": "nether"}"#,
        );
        schreibe(
            dir,
            "minecraft/dimension_type/the_end.json",
            r#"{"has_skylight": false}"#,
        );
        schreibe(
            dir,
            "minecraft/dimension/the_nether.json",
            r#"{"type": "minecraft:the_end"}"#,
        );

        let mut dimensionen = Dimensionen::default();
        assert_eq!(dimensionen.laden(dir).unwrap(), 7);
        assert_eq!(dimensionen.kaputt.len(), 2, "{:?}", dimensionen.kaputt);

        let oberwelt = TABELLE.typen["minecraft:overworld"];
        assert_eq!(dimensionen.gewaehlt(), oberwelt, "ohne Festlegung");
        let mut typ = |dimension: Option<&str>| {
            let meldung = dimensionen.festlegen(dimension);
            (dimensionen.gewaehlt(), meldung.is_some())
        };
        let dunkel = typ(Some("beispiel:hoehle"));
        assert_eq!(dunkel.0.cardinal_light, CardinalLight::Nether);
        assert_eq!(dunkel.0.ambient_light_color, [0x11, 0x22, 0x33]);
        assert!(!dunkel.1);
        let direkt = typ(Some("beispiel:direkt"));
        assert_eq!(
            (direkt.0.has_skylight, direkt.0.cardinal_light, direkt.1),
            (true, CardinalLight::Nether, false)
        );
        assert_eq!(typ(Some("beispiel:fremd")), (oberwelt, true));
        assert_eq!(typ(Some("beispiel:unbekannt")), (oberwelt, true));
        assert_eq!(typ(None), (oberwelt, true), "ohne Weltwurzel");
        assert_eq!(
            typ(Some("minecraft:overworld")),
            (oberwelt, false),
            "ohne Definition der Typ gleichen Namens"
        );
        let ende = typ(Some("minecraft:the_end"));
        assert_eq!(
            typ(Some("minecraft:the_nether")),
            ende,
            "die Definition aus der Datenwurzel"
        );
        assert_eq!(
            (ende.0.has_skylight, ende.1),
            (false, false),
            "Typ aus der Datenwurzel"
        );
        assert_eq!(
            typ(Some("beispiel:kurz")),
            ende,
            "ohne Namensraum minecraft"
        );
    }
}
