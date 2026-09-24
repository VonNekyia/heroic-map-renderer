use anyhow::{Result, anyhow, bail};
use serde_json::Value;

use crate::world::BlockState;

/// Der Inhalt einer `blockstates/*.json`: eine Variantentabelle, eine Liste
/// von Multipart-Fällen oder beides. Wie im Client gilt zuerst die Tabelle,
/// und Multipart deckt jeden Zustand, den sie nicht nennt.
#[derive(Debug)]
pub struct BlockStateDef {
    variants: Vec<Variant>,
    multipart: Option<Vec<Case>>,
}

/// Ein Eintrag der Variantentabelle. `when` ist leer für den Schlüssel `""`,
/// der auf jede Blockstate passt.
#[derive(Debug)]
pub struct Variant {
    pub when: Vec<(String, String)>,
    pub apply: Vec<ModelRef>,
}

/// Verweis auf ein Modell samt Drehung aus der Blockstate-Datei.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelRef {
    pub model: String,
    pub x: i32,
    pub y: i32,
    /// Seit Minecraft 1.21.11 auch für Blockstate-Varianten erlaubt.
    pub z: i32,
    pub uvlock: bool,
    /// Gewicht in einer Variantenliste; ohne Angabe 1.
    pub weight: u32,
}

impl ModelRef {
    /// Der Verweis auf den Missing-Würfel, ungedreht.
    pub fn missing() -> ModelRef {
        ModelRef {
            model: super::MISSING_MODEL.to_string(),
            x: 0,
            y: 0,
            z: 0,
            uvlock: false,
            weight: 1,
        }
    }
}

#[derive(Debug)]
pub struct Case {
    pub when: Option<Condition>,
    pub apply: Vec<ModelRef>,
}

#[derive(Debug)]
pub enum Condition {
    /// Alle Paare müssen passen; der Wert darf `a|b` als Alternativen listen.
    Props(Vec<(String, Vec<Term>)>),
    And(Vec<Condition>),
    Or(Vec<Condition>),
}

/// Ein Wert in einer Multipart-Bedingung. Ein führendes `!` negiert ihn, wie
/// `KeyValueCondition.Term` in Minecraft.
#[derive(Debug, PartialEq, Eq)]
pub struct Term {
    value: String,
    negated: bool,
}

impl Term {
    fn parse(text: &str) -> Term {
        match text.strip_prefix('!') {
            Some(rest) => Term {
                value: rest.to_string(),
                negated: true,
            },
            None => Term {
                value: text.to_string(),
                negated: false,
            },
        }
    }

    fn matches(&self, value: &str) -> bool {
        (self.value == value) != self.negated
    }
}

impl BlockStateDef {
    /// Liest eine Definition so streng wie der Client: eine leere
    /// Variantentabelle, eine leere Liste und ein Gewicht unter 1 lehnt er
    /// ab, wie eine Datei mit keinem von beiden.
    pub fn parse(json: &Value) -> Result<BlockStateDef> {
        let mut variants = Vec::new();
        if let Some(value) = json.get("variants") {
            let object = value
                .as_object()
                .ok_or_else(|| anyhow!("variants ist kein Objekt"))?;
            if object.is_empty() {
                bail!("variants ist leer");
            }
            for (key, value) in object {
                variants.push(Variant {
                    when: parse_variant_key(key)?,
                    apply: parse_apply(value)?,
                });
            }
        }

        let multipart = match json.get("multipart") {
            Some(value) => {
                let list = value
                    .as_array()
                    .ok_or_else(|| anyhow!("multipart ist keine Liste"))?;
                let mut cases = Vec::with_capacity(list.len());
                for case in list {
                    let apply = case
                        .get("apply")
                        .ok_or_else(|| anyhow!("Multipart-Fall ohne apply"))?;
                    cases.push(Case {
                        when: case.get("when").map(parse_condition).transpose()?,
                        apply: parse_apply(apply)?,
                    });
                }
                Some(cases)
            }
            None => None,
        };

        if variants.is_empty() && multipart.is_none() {
            bail!("weder variants noch multipart");
        }
        Ok(BlockStateDef {
            variants,
            multipart,
        })
    }

    /// Die Modelle der ersten Alternative — für Sprite-Raster und
    /// Diagnose, wo es auf eine feste Wahl ankommt.
    pub fn select(&self, state: &BlockState) -> Vec<ModelRef> {
        self.alternatives(state)
            .and_then(|alternativen| alternativen.into_iter().next())
            .map(|(_, refs)| refs)
            .unwrap_or_default()
    }

    /// Alle Alternativen mit ihrem Gewicht, oder `None`, wenn die Datei den
    /// Zustand nicht kennt: keine Variante passt, und Multipart gibt es
    /// nicht. Dann gilt die Datei eines tieferen Packs.
    ///
    /// Eine Variantenliste ist Vanillas Zufall: Sand, Stein und Erde
    /// liegen in vier Drehungen vor, und welche ein Block bekommt, würfelt
    /// seine Position. Bei `multipart` gilt je Fall der erste Eintrag —
    /// Listen haben dort nur Bambus, Chorus und Feuer. Multipart darf leer
    /// ausgehen: trifft keine Bedingung zu, hat der Zustand keine Geometrie.
    // ponytail: multipart ohne Zufall. Erst nötig, wenn jemand die
    // Bambus-Varianten vermisst.
    pub fn alternatives(&self, state: &BlockState) -> Option<Vec<(u32, Vec<ModelRef>)>> {
        let variant = self.variants.iter().find(|variant| {
            variant
                .when
                .iter()
                .all(|(name, value)| state.prop(name) == Some(value.as_str()))
        });
        if let Some(variant) = variant {
            return Some(
                variant
                    .apply
                    .iter()
                    .map(|r| (r.weight, vec![r.clone()]))
                    .collect(),
            );
        }
        let cases = self.multipart.as_ref()?;
        Some(vec![(
            1,
            cases
                .iter()
                .filter(|case| case.when.as_ref().is_none_or(|c| c.matches(state)))
                .filter_map(|case| case.apply.first().cloned())
                .collect(),
        )])
    }
}

impl Condition {
    fn matches(&self, state: &BlockState) -> bool {
        match self {
            Condition::Props(props) => props.iter().all(|(name, terms)| {
                state
                    .prop(name)
                    .is_some_and(|value| terms.iter().any(|term| term.matches(value)))
            }),
            Condition::And(list) => list.iter().all(|c| c.matches(state)),
            Condition::Or(list) => list.iter().any(|c| c.matches(state)),
        }
    }
}

/// `"facing=east,half=bottom"` -> Paare. Der leere Schlüssel passt immer.
fn parse_variant_key(key: &str) -> Result<Vec<(String, String)>> {
    if key.is_empty() {
        return Ok(Vec::new());
    }
    key.split(',')
        .map(|pair| {
            pair.split_once('=')
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .ok_or_else(|| anyhow!("Variantenschlüssel ohne '=': {pair}"))
        })
        .collect()
}

fn parse_apply(value: &Value) -> Result<Vec<ModelRef>> {
    match value {
        Value::Array(list) if list.is_empty() => bail!("leere Modellliste"),
        Value::Array(list) => list.iter().map(parse_model_ref).collect(),
        object => Ok(vec![parse_model_ref(object)?]),
    }
}

fn parse_model_ref(value: &Value) -> Result<ModelRef> {
    let weight = match value.get("weight") {
        None => 1,
        Some(weight) => weight
            .as_u64()
            .filter(|&weight| weight >= 1)
            .and_then(|weight| u32::try_from(weight).ok())
            .ok_or_else(|| anyhow!("Gewicht {weight} ist keine positive Zahl"))?,
    };
    Ok(ModelRef {
        weight,
        model: value
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Modellverweis ohne model"))?
            .to_string(),
        x: value.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
        y: value.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
        z: value.get("z").and_then(Value::as_i64).unwrap_or(0) as i32,
        uvlock: value
            .get("uvlock")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_condition(value: &Value) -> Result<Condition> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("when ist kein Objekt"))?;

    for (key, combine) in [
        ("OR", Condition::Or as fn(Vec<Condition>) -> Condition),
        ("AND", Condition::And),
    ] {
        if let Some(list) = object.get(key) {
            let list = list
                .as_array()
                .ok_or_else(|| anyhow!("{key} ist keine Liste"))?;
            return Ok(combine(
                list.iter().map(parse_condition).collect::<Result<_>>()?,
            ));
        }
    }

    Ok(Condition::Props(
        object
            .iter()
            .map(|(name, value)| {
                let value = value_as_string(value)
                    .ok_or_else(|| anyhow!("Bedingung {name} hat keinen einfachen Wert"))?;
                Ok((
                    name.clone(),
                    value.split('|').map(Term::parse).collect::<Vec<_>>(),
                ))
            })
            .collect::<Result<_>>()?,
    ))
}

/// Blockstate-Werte sind Strings, manche Packs schreiben aber `true` oder `3`.
fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str) -> BlockState {
        BlockState::parse(text).unwrap()
    }

    fn def(json: &str) -> BlockStateDef {
        BlockStateDef::parse(&serde_json::from_str(json).unwrap()).unwrap()
    }

    #[test]
    fn variante_ohne_schluessel_passt_immer() {
        let d = def(r#"{"variants": {"": {"model": "block/stone"}}}"#);
        let models = d.select(&state("minecraft:stone"));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model, "block/stone");
    }

    #[test]
    fn gewichtete_liste_nimmt_den_ersten() {
        let d = def(r#"{"variants": {"": [
                {"model": "block/stone"},
                {"model": "block/stone_mirrored", "y": 180}
            ]}}"#);
        let models = d.select(&state("minecraft:stone"));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model, "block/stone");
        assert_eq!(models[0].y, 0);
    }

    #[test]
    fn variante_waehlt_nach_properties() {
        let d = def(r#"{"variants": {
                "facing=east,half=bottom": {"model": "block/stairs", "y": 270, "uvlock": true},
                "facing=west,half=bottom": {"model": "block/stairs", "y": 90}
            }}"#);
        let models = d.select(&state(
            "minecraft:oak_stairs[facing=east,half=bottom,shape=straight]",
        ));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].y, 270);
        assert!(models[0].uvlock);

        // Eine Property, die in keinem Schlüssel steht, stört nicht; eine
        // fehlende Übereinstimmung schon.
        assert!(
            d.select(&state("minecraft:oak_stairs[facing=north,half=bottom]"))
                .is_empty()
        );
    }

    #[test]
    fn multipart_sammelt_alle_treffer() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/post"}},
                {"apply": {"model": "block/side"}, "when": {"north": "true"}},
                {"apply": {"model": "block/side", "y": 90}, "when": {"east": "true"}}
            ]}"#);
        let models = d.select(&state(
            "minecraft:oak_fence[east=true,north=true,south=false]",
        ));
        assert_eq!(models.len(), 3);
        let models = d.select(&state("minecraft:oak_fence[east=false,north=false]"));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model, "block/post");
    }

    #[test]
    fn bedingungen_mit_oder_und_alternativen() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/dot"}, "when": {"OR": [
                    {"north": "none", "east": "none"},
                    {"north": "side|up"}
                ]}}
            ]}"#);
        assert_eq!(
            d.select(&state("minecraft:redstone_wire[east=none,north=none]"))
                .len(),
            1
        );
        assert_eq!(
            d.select(&state("minecraft:redstone_wire[east=side,north=up]"))
                .len(),
            1
        );
        assert_eq!(
            d.select(&state("minecraft:redstone_wire[east=side,north=none]"))
                .len(),
            0
        );
    }

    #[test]
    fn bedingungen_mit_und() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/x"}, "when": {"AND": [
                    {"up": "true"},
                    {"down": "false"}
                ]}}
            ]}"#);
        assert_eq!(
            d.select(&state("minecraft:vine[up=true,down=false]")).len(),
            1
        );
        assert_eq!(
            d.select(&state("minecraft:vine[up=true,down=true]")).len(),
            0
        );
    }

    #[test]
    fn nicht_string_werte_werden_akzeptiert() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"lit": true}}]}"#);
        assert_eq!(d.select(&state("minecraft:x[lit=true]")).len(), 1);
        assert_eq!(d.select(&state("minecraft:x[lit=false]")).len(), 0);
    }

    /// Minecraft behandelt ein führendes `!` als Negation
    /// (`KeyValueCondition.Term`). Ohne das wird `!north` wörtlich
    /// verglichen und passt auf keinen gültigen Wert.
    #[test]
    fn negierte_bedingungen() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"facing": "!north"}}]}"#);
        assert_eq!(d.select(&state("minecraft:x[facing=east]")).len(), 1);
        assert_eq!(d.select(&state("minecraft:x[facing=north]")).len(), 0);
    }

    #[test]
    fn negierte_bedingungen_mit_alternativen() {
        let d =
            def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"facing": "!north|east"}}]}"#);
        // Erst an `|` trennen, dann jeden Term auf `!` prüfen; es genügt,
        // wenn einer passt.
        assert_eq!(d.select(&state("minecraft:x[facing=east]")).len(), 1);
        assert_eq!(d.select(&state("minecraft:x[facing=south]")).len(), 1);
        // north scheitert an beiden Termen
        assert_eq!(d.select(&state("minecraft:x[facing=north]")).len(), 0);
    }

    /// Trifft in einem Multipart keine Bedingung zu, hat der Zustand keine
    /// Geometrie — das ist kein Fehler, und der Zustand gilt als gedeckt.
    #[test]
    fn multipart_darf_leer_ausgehen() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"powered": "true"}}]}"#);
        let leer = d.alternatives(&state("minecraft:x[powered=false]"));
        assert_eq!(leer, Some(vec![(1, Vec::new())]));
    }

    /// Eine Variantentabelle ohne Treffer kennt den Zustand nicht; dann
    /// gilt ein tieferes Pack.
    #[test]
    fn variantentabelle_ohne_treffer_kennt_den_zustand_nicht() {
        let d = def(r#"{"variants": {"lit=true": {"model": "m"}}}"#);
        assert!(d.alternatives(&state("minecraft:x[lit=false]")).is_none());
    }

    /// Beides in einer Datei: zuerst die Tabelle, Multipart für den Rest,
    /// wie `BlockStateModelDispatcher.instantiate` mit `putIfAbsent`.
    #[test]
    fn variants_vor_multipart() {
        let d = def(r#"{"variants": {"lit=true": {"model": "a"}},
                "multipart": [{"apply": {"model": "b"}}]}"#);
        assert_eq!(d.select(&state("minecraft:x[lit=true]"))[0].model, "a");
        assert_eq!(d.select(&state("minecraft:x[lit=false]"))[0].model, "b");
    }

    /// Was der Client ablehnt, lehnt auch der Renderer ab: dann gilt die
    /// Datei eines tieferen Packs.
    #[test]
    fn leere_listen_und_gewicht_null_sind_fehler() {
        for json in [
            r#"{"variants": {}}"#,
            r#"{"variants": {"": []}}"#,
            r#"{"variants": {"": [{"model": "m", "weight": 0}]}}"#,
        ] {
            let json = serde_json::from_str(json).unwrap();
            assert!(BlockStateDef::parse(&json).is_err(), "{json}");
        }
    }

    /// Seit Minecraft 1.21.11 dürfen Modellverweise auch um Z gedreht sein.
    #[test]
    fn z_drehung_wird_gelesen() {
        let d = def(r#"{"variants": {"": {"model": "m", "x": 90, "y": 180, "z": 270}}}"#);
        let m = &d.select(&state("minecraft:x"))[0];
        assert_eq!((m.x, m.y, m.z), (90, 180, 270));

        let d = def(r#"{"variants": {"": {"model": "m"}}}"#);
        assert_eq!(d.select(&state("minecraft:x"))[0].z, 0);
    }

    #[test]
    fn datei_ohne_variants_und_multipart_ist_fehler() {
        assert!(BlockStateDef::parse(&serde_json::json!({"foo": 1})).is_err());
    }
}
