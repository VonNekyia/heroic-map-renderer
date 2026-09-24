use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::world::BlockState;

/// Der Inhalt einer `blockstates/*.json`: eine Variantentabelle, eine Liste
/// von Multipart-Fällen oder beides. Wie im Client gilt zuerst die Tabelle,
/// und Multipart deckt jeden Zustand, den sie nicht nennt.
#[derive(Debug)]
pub struct BlockStateDef {
    /// In der Reihenfolge der Datei.
    variants: Vec<Variant>,
    multipart: Option<Vec<Case>>,
    /// Nach [`BlockStateDef::instantiate`] die Variante jedes Zustands,
    /// nach seinem Platz in `getPossibleStates`.
    owners: Option<Vec<Option<usize>>>,
}

/// Ein Eintrag der Variantentabelle.
#[derive(Debug)]
struct Variant {
    /// Die Teile des Schlüssels in der Reihenfolge der Datei. `None`, wenn
    /// ein Name ohne `=` dasteht: den Eintrag verwirft der Client.
    key: Option<Vec<(String, String)>>,
    apply: Vec<ModelRef>,
}

/// Verweis auf ein Modell samt Drehung aus der Blockstate-Datei.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelRef {
    /// Mit Namensraum, wie `Identifier` ihn liest.
    pub model: String,
    /// Drehungen in Grad, 0, 90, 180 oder 270.
    pub x: i32,
    pub y: i32,
    /// Seit Minecraft 1.21.11 auch für Blockstate-Varianten erlaubt.
    pub z: i32,
    pub uvlock: bool,
    /// Gewicht in einer Liste; ein einzelnes Objekt hat 1.
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
    fn matches(&self, value: &str) -> bool {
        (self.value == value) != self.negated
    }
}

/// Die Eigenschaften eines Blocks in 26.2, nach Namen sortiert wie in
/// `StateDefinition`, die Werte in der Reihenfolge von `getPossibleValues`.
/// Daraus folgt die Reihenfolge der Zustände: die erste Eigenschaft zählt
/// am meisten, die letzte wechselt am schnellsten.
#[derive(Debug)]
pub struct Definition {
    props: Vec<(&'static str, Vec<&'static str>)>,
}

/// Alle Blöcke von 26.2 aus dem Datengenerator (`reports/blocks.json`), je
/// Zeile ein Block mit seinen Eigenschaften. Neu erzeugen: README,
/// „Blockstates wie im Client“.
static BLOCKS: LazyLock<HashMap<&'static str, Definition>> = LazyLock::new(|| {
    include_str!("blocks.txt")
        .lines()
        .map(|line| {
            let mut parts = line.split(' ');
            let name = parts.next().unwrap_or_default();
            let mut props: Vec<_> = parts
                .filter_map(|part| part.split_once('='))
                .map(|(prop, values)| (prop, values.split(',').collect()))
                .collect();
            props.sort_unstable_by_key(|&(prop, _)| prop);
            (name, Definition { props })
        })
        .collect()
});

impl Definition {
    /// Die Definition eines Blocks, wenn 26.2 ihn kennt.
    pub fn of(block: &str) -> Option<&'static Definition> {
        BLOCKS.get(block.strip_prefix("minecraft:")?)
    }

    fn states(&self) -> usize {
        self.props.iter().map(|(_, values)| values.len()).product()
    }

    /// Der Platz eines Zustands in `getPossibleStates`, oder `None`, wenn
    /// es ihn in 26.2 nicht gibt.
    pub fn index(&self, state: &BlockState) -> Option<usize> {
        if state.props().len() != self.props.len() {
            return None;
        }
        self.props.iter().zip(state.props()).try_fold(
            0,
            |index, ((prop, values), (name, value))| {
                (prop == name).then_some(())?;
                Some(index * values.len() + values.iter().position(|v| v == value)?)
            },
        )
    }

    /// Der Wert, den ein Zustand für eine Eigenschaft hat, aus seinem Platz.
    fn digit(&self, index: usize, prop: usize) -> usize {
        let stride: usize = self.props[prop + 1..]
            .iter()
            .map(|(_, v)| v.len())
            .product();
        index / stride % self.props[prop].1.len()
    }

    fn prop(&self, name: &str) -> Option<usize> {
        self.props.iter().position(|(prop, _)| *prop == name)
    }

    /// `Property.getValue`: der Name genau, bei `IntegerProperty` die Zahl
    /// über `Integer.parseInt`, `01` und `+1` sind also 1. Nur dort sind
    /// die Namen Zahlen, per javap und Report an 26.2 geprüft.
    // ponytail: parseInt nimmt auch andere Unicode-Ziffern, hier nur ASCII.
    fn value(&self, prop: usize, text: &str) -> Option<usize> {
        let values = &self.props[prop].1;
        let text = match values[0].parse::<i32>() {
            Ok(_) => text.parse::<i32>().ok()?.to_string(),
            Err(_) => text.to_string(),
        };
        values.iter().position(|value| *value == text)
    }

    /// `VariantSelector.predicate`: jede Eigenschaft und jeden Wert
    /// nachschlagen, bei doppelten gilt die letzte. `None` bei einer
    /// unbekannten Eigenschaft oder einem unbekannten Wert.
    fn resolve(&self, key: &[(String, String)]) -> Option<Vec<(usize, usize)>> {
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (name, value) in key {
            let prop = self.prop(name)?;
            let value = self.value(prop, value)?;
            out.retain(|&(p, _)| p != prop);
            out.push((prop, value));
        }
        Some(out)
    }
}

impl BlockStateDef {
    /// Liest eine Datei so streng wie der Client: nach dem ersten Dokument
    /// darf nichts mehr kommen (`StrictJsonParser`), und was der Codec
    /// ablehnt, macht die ganze Datei kaputt. `null` zählt als fehlend.
    pub fn read(text: &str) -> Result<BlockStateDef> {
        let json: Json = serde_json::from_str(text).context("kein gültiges JSON")?;
        ensure!(matches!(json, Json::Object(_)), "kein Objekt");
        let variants = match json.field("variants") {
            None => Vec::new(),
            Some(Json::Object(entries)) => {
                ensure!(!entries.is_empty(), "variants ist leer");
                entries
                    .iter()
                    .map(|(key, value)| {
                        Ok(Variant {
                            key: parse_variant_key(key),
                            apply: parse_apply(value)
                                .with_context(|| format!("Variante \"{key}\""))?,
                        })
                    })
                    .collect::<Result<_>>()?
            }
            Some(_) => bail!("variants ist kein Objekt"),
        };
        let multipart = match json.field("multipart") {
            None => None,
            Some(Json::Array(cases)) => {
                ensure!(!cases.is_empty(), "multipart ist leer");
                Some(cases.iter().map(parse_case).collect::<Result<_>>()?)
            }
            Some(_) => bail!("multipart ist keine Liste"),
        };
        ensure!(
            !variants.is_empty() || multipart.is_some(),
            "weder variants noch multipart"
        );
        Ok(BlockStateDef {
            variants,
            multipart,
            owners: None,
        })
    }

    /// `BlockStateModelDispatcher.instantiate` gegen die Definition aus
    /// 26.2. Einen Variantenschlüssel mit unbekannter Eigenschaft oder
    /// unbekanntem Wert verwirft der Client, nur diesen Eintrag. Überlappen
    /// sich zwei, bekommt der erste gemeinsame Zustand den späteren, und
    /// der Rest des späteren fällt weg (`Overlapping definition`). Eine
    /// solche Multipart-Bedingung dagegen wirft: dann hat der Block über
    /// alle Packs kein Modell.
    pub fn instantiate(&mut self, definition: &Definition) -> Result<()> {
        let mut owners = vec![None; definition.states()];
        for (entry, variant) in self.variants.iter().enumerate() {
            let Some(key) = variant.key.as_deref().and_then(|k| definition.resolve(k)) else {
                continue;
            };
            for (state, owner) in owners.iter_mut().enumerate() {
                let matches = key.iter().all(|&(p, v)| definition.digit(state, p) == v);
                if matches && owner.replace(entry).is_some() {
                    break;
                }
            }
        }
        self.owners = Some(owners);
        for case in self.multipart.iter_mut().flatten() {
            if let Some(when) = &mut case.when {
                when.instantiate(definition)?;
            }
        }
        Ok(())
    }

    /// Alle Alternativen mit ihrem Gewicht, oder `None`, wenn die Datei den
    /// Zustand nicht kennt: keine Variante trägt ihn, und Multipart gibt es
    /// nicht. Dann gilt die Datei eines tieferen Packs. `index` ist der
    /// Platz des Zustands in 26.2; ohne ihn, bei Blöcken und Zuständen, die
    /// 26.2 nicht kennt, gilt der erste passende Schlüssel.
    ///
    /// Eine Variantenliste ist Vanillas Zufall: Sand, Stein und Erde
    /// liegen in vier Drehungen vor, und welche ein Block bekommt, würfelt
    /// seine Position. Bei `multipart` gilt je Fall der erste Eintrag —
    /// Listen haben dort nur Bambus, Chorus und Feuer. Multipart darf leer
    /// ausgehen: trifft keine Bedingung zu, hat der Zustand keine Geometrie.
    // ponytail: multipart ohne Zufall. Erst nötig, wenn jemand die
    // Bambus-Varianten vermisst.
    pub fn alternatives(
        &self,
        state: &BlockState,
        index: Option<usize>,
    ) -> Option<Vec<(u32, Vec<ModelRef>)>> {
        let variant = match (&self.owners, index) {
            (Some(owners), Some(index)) => owners[index],
            _ => self.variants.iter().position(|variant| {
                variant
                    .key
                    .as_deref()
                    .is_some_and(|key| matches_key(key, state))
            }),
        };
        if let Some(variant) = variant {
            return Some(
                self.variants[variant]
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

    /// `Condition.instantiate`: jede Eigenschaft und jeden Wert gegen die
    /// Definition prüfen und die Werte so schreiben, wie der Zustand sie
    /// trägt.
    fn instantiate(&mut self, definition: &Definition) -> Result<()> {
        match self {
            Condition::Props(props) => {
                for (name, terms) in props {
                    let prop = definition
                        .prop(name)
                        .ok_or_else(|| anyhow!("unbekannte Eigenschaft {name}"))?;
                    for term in terms {
                        let value = definition
                            .value(prop, &term.value)
                            .ok_or_else(|| anyhow!("unbekannter Wert {} für {name}", term.value))?;
                        term.value = definition.props[prop].1[value].to_string();
                    }
                }
            }
            Condition::And(list) | Condition::Or(list) => {
                for condition in list {
                    condition.instantiate(definition)?;
                }
            }
        }
        Ok(())
    }
}

/// Passt ein Schlüssel, ohne dass die Definition bekannt ist? Nennt er
/// eine Eigenschaft zweimal, gilt wie im Client die letzte.
fn matches_key(key: &[(String, String)], state: &BlockState) -> bool {
    key.iter().enumerate().all(|(i, (name, value))| {
        key[i + 1..].iter().any(|(later, _)| later == name) || state.prop(name) == Some(value)
    })
}

/// `VariantSelector`: an `,` trennen, jeden Teil am ersten `=`. Ein leerer
/// Name zählt nicht, `""`, `","` und `"=north"` passen also auf alles. Ein
/// Name ohne `=` ist eine unbekannte Eigenschaft, dann `None`.
fn parse_variant_key(key: &str) -> Option<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for part in key.split(',') {
        match part.split_once('=') {
            Some(("", _)) => {}
            Some((name, value)) => pairs.push((name.to_string(), value.to_string())),
            None if part.is_empty() => {}
            None => return None,
        }
    }
    Some(pairs)
}

/// `BlockStateModel.Unbaked.CODEC`: eine nichtleere Liste mit Gewichten,
/// sonst ein einzelnes Objekt. Dessen `weight` liest der Client nicht.
fn parse_apply(json: &Json) -> Result<Vec<ModelRef>> {
    let Json::Array(list) = json else {
        return Ok(vec![parse_model_ref(json)?]);
    };
    ensure!(!list.is_empty(), "leere Modellliste");
    let mut refs = Vec::with_capacity(list.len());
    let mut total = 0u64;
    for element in list {
        let weight = match element.field("weight") {
            None => 1,
            Some(weight) => weight.int().context("weight")?,
        };
        ensure!(weight >= 1, "Gewicht {weight} ist nicht positiv");
        total += weight as u64;
        refs.push(ModelRef {
            weight: weight as u32,
            ..parse_model_ref(element)?
        });
    }
    // `WeightedRandom.getTotalWeight` wirft darüber; `nextInt` hinge
    // sonst an einer negativen Schranke.
    ensure!(
        total <= i32::MAX as u64,
        "Summe der Gewichte {total} über 2147483647"
    );
    Ok(refs)
}

/// `Variant.MAP_CODEC`: ein Modell, die Drehungen und `uvlock`.
fn parse_model_ref(json: &Json) -> Result<ModelRef> {
    ensure!(
        matches!(json, Json::Object(_)),
        "Modellverweis ist kein Objekt"
    );
    let model = match json.field("model") {
        Some(Json::String(id)) => identifier(id)?,
        Some(_) => bail!("model ist kein Text"),
        None => bail!("Modellverweis ohne model"),
    };
    let uvlock = match json.field("uvlock") {
        None => false,
        Some(Json::Bool(uvlock)) => *uvlock,
        Some(_) => bail!("uvlock ist kein Wahrheitswert"),
    };
    Ok(ModelRef {
        model,
        x: quadrant(json, "x")?,
        y: quadrant(json, "y")?,
        z: quadrant(json, "z")?,
        uvlock,
        weight: 1,
    })
}

/// `Quadrant.CODEC`: eine Zahl, modulo 360 eine von 0, 90, 180 und 270.
/// -90 ist also 270, 45 ein Fehler.
fn quadrant(json: &Json, axis: &str) -> Result<i32> {
    let Some(value) = json.field(axis) else {
        return Ok(0);
    };
    let degrees = value.int().context(axis.to_string())?;
    let quadrant = degrees.rem_euclid(360);
    ensure!(
        quadrant % 90 == 0,
        "Drehung {degrees} um {axis}, erlaubt sind 0, 90, 180 und 270"
    );
    Ok(quadrant)
}

/// `Identifier.CODEC`: ohne Namensraum, auch mit `:` vorn, gilt
/// `minecraft`. Ein Grossbuchstabe macht die Datei kaputt.
pub(super) fn identifier(text: &str) -> Result<String> {
    let (namespace, path) = super::split_id(text);
    ensure!(
        is_identifier(namespace, path),
        "{text} ist kein gültiger Modellname"
    );
    Ok(format!("{namespace}:{path}"))
}

/// `Identifier.isValidNamespace` und `isValidPath`: `a-z0-9_.-`, der
/// Namensraum aber nicht `..`, im Pfad dazu `/`.
pub(super) fn is_identifier(namespace: &str, path: &str) -> bool {
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b);
    namespace != ".." && namespace.bytes().all(valid) && path.bytes().all(|b| valid(b) || b == b'/')
}

/// `Selector.CODEC`: `when` darf fehlen, `apply` nicht.
fn parse_case(json: &Json) -> Result<Case> {
    ensure!(
        matches!(json, Json::Object(_)),
        "Multipart-Fall ist kein Objekt"
    );
    let apply = json
        .field("apply")
        .ok_or_else(|| anyhow!("Multipart-Fall ohne apply"))?;
    Ok(Case {
        when: json.field("when").map(parse_condition).transpose()?,
        apply: parse_apply(apply)?,
    })
}

/// `Condition.CODEC`: genau ein Schlüssel `OR` oder `AND` mit einer Liste,
/// sonst eine nichtleere Tabelle von Eigenschaften. Ein `OR` mit Text statt
/// Liste ist also eine Eigenschaft namens `OR`.
fn parse_condition(json: &Json) -> Result<Condition> {
    let Json::Object(entries) = json else {
        bail!("Bedingung ist kein Objekt");
    };
    match entries.as_slice() {
        [(op, Json::Array(list))] if op == "OR" || op == "AND" => {
            let list = list.iter().map(parse_condition).collect::<Result<_>>()?;
            Ok(if op == "OR" {
                Condition::Or(list)
            } else {
                Condition::And(list)
            })
        }
        [] => bail!("leere Bedingung"),
        _ => Ok(Condition::Props(
            entries
                .iter()
                .map(|(name, value)| {
                    let terms = parse_terms(value).with_context(|| format!("Bedingung {name}"))?;
                    Ok((name.clone(), terms))
                })
                .collect::<Result<_>>()?,
        )),
    }
}

/// `Terms.CODEC`: ein Text, alte Packs schreiben auch Zahlen und
/// Wahrheitswerte. Getrennt an `|`, ein `!` vorn verneint den Term, und
/// ein leerer Term wirft (`Empty term`).
fn parse_terms(json: &Json) -> Result<Vec<Term>> {
    let text = match json {
        Json::String(text) => text.clone(),
        Json::Bool(value) => value.to_string(),
        number => number.int()?.to_string(),
    };
    text.split('|')
        .map(|part| {
            let (value, negated) = match part.strip_prefix('!') {
                Some(rest) => (rest, true),
                None => (part, false),
            };
            ensure!(!value.is_empty(), "leerer Term in \"{text}\"");
            Ok(Term {
                value: value.to_string(),
                negated,
            })
        })
        .collect()
}

/// JSON, wie Gson es liest: Objekte in der Reihenfolge der Datei, ein
/// doppelter Schlüssel behält seinen ersten Platz und nimmt den letzten
/// Wert (`LinkedTreeMap.put`). Die Reihenfolge zählt bei überlappenden
/// Variantenschlüsseln, und `serde_json::Value` sortiert sie.
#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Ein Feld, wie DFU es liest: `null` zählt als fehlend.
    fn field(&self, name: &str) -> Option<&Json> {
        let Json::Object(entries) = self else {
            return None;
        };
        entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .filter(|value| !matches!(value, Json::Null))
    }

    /// `Codec.INT`: nur eine JSON-Zahl, abgeschnitten wie
    /// `Number.intValue`. 90.5 ist also 90, und 2^32 + 90 auch.
    // ponytail: Kommazahlen ab 2^127 sättigen statt abzuschneiden; so
    // schreibt kein Pack.
    fn int(&self) -> Result<i32> {
        match *self {
            Json::Int(n) => Ok(n as i32),
            Json::Float(f) => Ok(f as i128 as i32),
            _ => bail!("keine Zahl"),
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Int(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Int(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Json, E> {
        Ok(Json::Float(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut list = Vec::new();
        while let Some(item) = seq.next_element()? {
            list.push(item);
        }
        Ok(Json::Array(list))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut entries: Vec<(String, Json)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key, value)),
            }
        }
        Ok(Json::Object(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str) -> BlockState {
        BlockState::parse(text).unwrap()
    }

    fn def(json: &str) -> BlockStateDef {
        BlockStateDef::read(json).unwrap()
    }

    /// Die Modelle der ersten Alternative, ohne Definition.
    fn models(d: &BlockStateDef, text: &str) -> Vec<ModelRef> {
        d.alternatives(&state(text), None)
            .and_then(|alternativen| alternativen.into_iter().next())
            .map(|(_, refs)| refs)
            .unwrap_or_default()
    }

    /// Das Modell, das der Client einem Zustand gibt, gegen die Definition
    /// aus 26.2; `None`, wenn keine Variante ihn trägt.
    fn exact(json: &str, text: &str) -> Option<String> {
        let mut d = def(json);
        let s = state(text);
        let definition = Definition::of(s.name()).unwrap();
        d.instantiate(definition).unwrap();
        let index = definition.index(&s).expect("Zustand gibt es in 26.2");
        Some(d.alternatives(&s, Some(index))?[0].1[0].model.clone())
    }

    #[test]
    fn variante_ohne_schluessel_passt_immer() {
        let d = def(r#"{"variants": {"": {"model": "block/stone"}}}"#);
        let stein = models(&d, "minecraft:stone");
        assert_eq!(stein.len(), 1);
        assert_eq!(stein[0].model, "minecraft:block/stone");
    }

    #[test]
    fn gewichtete_liste_nimmt_den_ersten() {
        let d = def(r#"{"variants": {"": [
                {"model": "block/stone"},
                {"model": "block/stone_mirrored", "y": 180}
            ]}}"#);
        let stein = models(&d, "minecraft:stone");
        assert_eq!(stein.len(), 1);
        assert_eq!(stein[0].model, "minecraft:block/stone");
        assert_eq!(stein[0].y, 0);
    }

    #[test]
    fn variante_waehlt_nach_properties() {
        let d = def(r#"{"variants": {
                "facing=east,half=bottom": {"model": "block/stairs", "y": 270, "uvlock": true},
                "facing=west,half=bottom": {"model": "block/stairs", "y": 90}
            }}"#);
        let osten = models(
            &d,
            "minecraft:oak_stairs[facing=east,half=bottom,shape=straight]",
        );
        assert_eq!(osten.len(), 1);
        assert_eq!(osten[0].y, 270);
        assert!(osten[0].uvlock);

        // Eine Property, die in keinem Schlüssel steht, stört nicht; eine
        // fehlende Übereinstimmung schon.
        assert!(models(&d, "minecraft:oak_stairs[facing=north,half=bottom]").is_empty());
    }

    #[test]
    fn multipart_sammelt_alle_treffer() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/post"}},
                {"apply": {"model": "block/side"}, "when": {"north": "true"}},
                {"apply": {"model": "block/side", "y": 90}, "when": {"east": "true"}}
            ]}"#);
        let alle = models(&d, "minecraft:oak_fence[east=true,north=true,south=false]");
        assert_eq!(alle.len(), 3);
        let pfosten = models(&d, "minecraft:oak_fence[east=false,north=false]");
        assert_eq!(pfosten.len(), 1);
        assert_eq!(pfosten[0].model, "minecraft:block/post");
    }

    #[test]
    fn bedingungen_mit_oder_und_alternativen() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/dot"}, "when": {"OR": [
                    {"north": "none", "east": "none"},
                    {"north": "side|up"}
                ]}}
            ]}"#);
        let n = |s: &str| models(&d, s).len();
        assert_eq!(n("minecraft:redstone_wire[east=none,north=none]"), 1);
        assert_eq!(n("minecraft:redstone_wire[east=side,north=up]"), 1);
        assert_eq!(n("minecraft:redstone_wire[east=side,north=none]"), 0);
    }

    #[test]
    fn bedingungen_mit_und() {
        let d = def(r#"{"multipart": [
                {"apply": {"model": "block/x"}, "when": {"AND": [
                    {"up": "true"},
                    {"down": "false"}
                ]}}
            ]}"#);
        assert_eq!(models(&d, "minecraft:vine[up=true,down=false]").len(), 1);
        assert_eq!(models(&d, "minecraft:vine[up=true,down=true]").len(), 0);
    }

    /// Alte Packs schreiben Zahlen und Wahrheitswerte; der Client liest sie
    /// über `Codec.INT` und `Codec.BOOL`, 1.5 also als 1.
    #[test]
    fn nicht_string_werte_werden_akzeptiert() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"lit": true}}]}"#);
        assert_eq!(models(&d, "minecraft:x[lit=true]").len(), 1);
        assert_eq!(models(&d, "minecraft:x[lit=false]").len(), 0);
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"age": 1.5}}]}"#);
        assert_eq!(models(&d, "minecraft:x[age=1]").len(), 1);
    }

    /// Minecraft behandelt ein führendes `!` als Negation
    /// (`KeyValueCondition.Term`). Ohne das wird `!north` wörtlich
    /// verglichen und passt auf keinen gültigen Wert.
    #[test]
    fn negierte_bedingungen() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"facing": "!north"}}]}"#);
        assert_eq!(models(&d, "minecraft:x[facing=east]").len(), 1);
        assert_eq!(models(&d, "minecraft:x[facing=north]").len(), 0);
    }

    #[test]
    fn negierte_bedingungen_mit_alternativen() {
        let d =
            def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"facing": "!north|east"}}]}"#);
        // Erst an `|` trennen, dann jeden Term auf `!` prüfen; es genügt,
        // wenn einer passt.
        assert_eq!(models(&d, "minecraft:x[facing=east]").len(), 1);
        assert_eq!(models(&d, "minecraft:x[facing=south]").len(), 1);
        // north scheitert an beiden Termen
        assert_eq!(models(&d, "minecraft:x[facing=north]").len(), 0);
    }

    /// Trifft in einem Multipart keine Bedingung zu, hat der Zustand keine
    /// Geometrie — das ist kein Fehler, und der Zustand gilt als gedeckt.
    #[test]
    fn multipart_darf_leer_ausgehen() {
        let d = def(r#"{"multipart": [{"apply": {"model": "m"}, "when": {"powered": "true"}}]}"#);
        let leer = d.alternatives(&state("minecraft:x[powered=false]"), None);
        assert_eq!(leer, Some(vec![(1, Vec::new())]));
    }

    /// Eine Variantentabelle ohne Treffer kennt den Zustand nicht; dann
    /// gilt ein tieferes Pack.
    #[test]
    fn variantentabelle_ohne_treffer_kennt_den_zustand_nicht() {
        let d = def(r#"{"variants": {"lit=true": {"model": "m"}}}"#);
        assert!(
            d.alternatives(&state("minecraft:x[lit=false]"), None)
                .is_none()
        );
    }

    /// Beides in einer Datei: zuerst die Tabelle, Multipart für den Rest,
    /// wie `BlockStateModelDispatcher.instantiate` mit `putIfAbsent`.
    #[test]
    fn variants_vor_multipart() {
        let d = def(r#"{"variants": {"lit=true": {"model": "a"}},
                "multipart": [{"apply": {"model": "b"}}]}"#);
        assert_eq!(models(&d, "minecraft:x[lit=true]")[0].model, "minecraft:a");
        assert_eq!(models(&d, "minecraft:x[lit=false]")[0].model, "minecraft:b");
    }

    /// Was der Codec des Clients ablehnt, macht die Datei kaputt; dann gilt
    /// die eines tieferen Packs. Belegt per javap an 26.2 samt DFU und Gson.
    #[test]
    fn was_der_client_ablehnt_macht_die_datei_kaputt() {
        let m = r#"{"model": "m"}"#;
        for json in [
            r#"{"variants": {}}"#.to_string(),
            r#"{"multipart": []}"#.to_string(),
            r#"{"variants": {"": []}}"#.to_string(),
            r#"{"variants": {"": null}}"#.to_string(),
            r#"{"multipart": [null]}"#.to_string(),
            r#"{"multipart": [{"when": {"a": "b"}}]}"#.to_string(),
            r#"{"variants": 5}"#.to_string(),
            r#"{}"#.to_string(),
            r#"[]"#.to_string(),
            r#"null"#.to_string(),
            format!(r#"{{"variants": {{"": {m}}}}} x"#),
            // Gewichte nur an Listenelementen, dort mindestens 1
            r#"{"variants": {"": [{"model": "m", "weight": 0}]}}"#.to_string(),
            r#"{"variants": {"": [{"model": "m", "weight": -1}]}}"#.to_string(),
            r#"{"variants": {"": [{"model": "m", "weight": "2"}]}}"#.to_string(),
            r#"{"variants": {"": [{"model": "m", "weight": true}]}}"#.to_string(),
            r#"{"variants": {"": [{"model": "a", "weight": 2}, {"model": "b", "weight": 2147483647}]}}"#.to_string(),
            r#"{"variants": {"": [{"model": "m", "weight": 2147483648}]}}"#.to_string(),
            r#"{"multipart": [{"apply": [{"model": "m", "weight": 0}]}]}"#.to_string(),
            // Modellname nach `Identifier`
            r#"{"variants": {"": {"model": "block/Stone"}}}"#.to_string(),
            r#"{"variants": {"": {"model": "a:b:c"}}}"#.to_string(),
            r#"{"variants": {"": {"model": "..:block/x"}}}"#.to_string(),
            r#"{"variants": {"": {"model": 5}}}"#.to_string(),
            r#"{"variants": {"": {"model": null}}}"#.to_string(),
            // Drehungen nur in Vielfachen von 90, als Zahl
            r#"{"variants": {"": {"model": "m", "x": 45}}}"#.to_string(),
            r#"{"variants": {"": {"model": "m", "y": "90"}}}"#.to_string(),
            r#"{"variants": {"": {"model": "m", "z": true}}}"#.to_string(),
            r#"{"variants": {"": {"model": "m", "uvlock": "true"}}}"#.to_string(),
            r#"{"variants": {"": {"model": "m", "uvlock": 1}}}"#.to_string(),
            // Bedingungen
            format!(r#"{{"multipart": [{{"when": {{}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"OR": [], "facing": "north"}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"OR": [], "AND": []}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"or": []}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"OR": [{{}}]}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"north": null}}, "apply": {m}}}]}}"#),
            format!(r#"{{"multipart": [{{"when": {{"north": []}}, "apply": {m}}}]}}"#),
        ]
        .into_iter()
        .chain(["", "a||b", "|a", "a|", "!"].map(|term| {
            format!(r#"{{"multipart": [{{"when": {{"north": "{term}"}}, "apply": {m}}}]}}"#)
        })) {
            assert!(BlockStateDef::read(&json).is_err(), "{json}");
        }
    }

    /// Umgekehrt nimmt der Client manches hin, was nach Fehler aussieht:
    /// `null` zählt als fehlend, ein einzelnes Objekt liest kein Gewicht,
    /// und Zahlen schneidet `intValue` ab.
    #[test]
    fn was_der_client_hinnimmt() {
        let d =
            def(r#"{"variants": null, "multipart": [{"when": null, "apply": {"model": "m"}}]}"#);
        assert_eq!(models(&d, "minecraft:x").len(), 1);

        for weight in ["0", "-1", "\"x\"", "1.5", "null"] {
            let d = def(&format!(
                r#"{{"variants": {{"": {{"model": "m", "weight": {weight}}}}}}}"#
            ));
            assert_eq!(models(&d, "minecraft:x")[0].weight, 1, "{weight}");
        }
        for (weight, soll) in [
            ("null", 1),
            ("1.5", 1),
            ("4294967297", 1),
            ("2147483647", 2147483647),
        ] {
            let d = def(&format!(
                r#"{{"variants": {{"": [{{"model": "m", "weight": {weight}}}]}}}}"#
            ));
            assert_eq!(models(&d, "minecraft:x")[0].weight, soll, "{weight}");
        }
        for model in ["block/x", ":block/x", "minecraft:block/x"] {
            let d = def(&format!(
                r#"{{"variants": {{"": {{"model": "{model}"}}}}}}"#
            ));
            assert_eq!(models(&d, "minecraft:x")[0].model, "minecraft:block/x");
        }
        for (winkel, soll) in [
            ("-90", 270),
            ("450", 90),
            ("90.5", 90),
            ("4294967386", 90),
            ("null", 0),
        ] {
            let d = def(&format!(
                r#"{{"variants": {{"": {{"model": "m", "x": {winkel}, "uvlock": null}}}}}}"#
            ));
            let r = &models(&d, "minecraft:x")[0];
            assert_eq!((r.x, r.uvlock), (soll, false), "{winkel}");
        }
    }

    /// `VariantSelector` trimmt nichts und überspringt leere Namen; ein
    /// Name ohne `=` verwirft den Eintrag, eine doppelte Eigenschaft
    /// nimmt den letzten Wert.
    #[test]
    fn schluessel_wie_im_client() {
        let d = def(r#"{"variants": {"facing=north,": {"model": "a"}, "facing": {"model": "b"}}}"#);
        assert_eq!(
            models(&d, "minecraft:x[facing=north]")[0].model,
            "minecraft:a"
        );
        assert!(models(&d, "minecraft:x[facing=south]").is_empty());
        for key in ["", ",", "=north"] {
            let d = def(&format!(r#"{{"variants": {{"{key}": {{"model": "a"}}}}}}"#));
            assert_eq!(models(&d, "minecraft:x[facing=south]").len(), 1, "{key:?}");
        }
        let d = def(r#"{"variants": {"facing=north,facing=south": {"model": "a"}}}"#);
        assert_eq!(models(&d, "minecraft:x[facing=south]").len(), 1);
        assert!(models(&d, "minecraft:x[facing=north]").is_empty());
    }

    /// Nur `OR` und `AND` mit einer Liste verbinden; `OR []` ist nie
    /// wahr, `AND []` immer. `OR` mit Text ist eine Eigenschaft.
    #[test]
    fn kombinierte_bedingungen_wie_im_client() {
        let fall = |when: &str| {
            let d = def(&format!(
                r#"{{"multipart": [{{"when": {when}, "apply": {{"model": "m"}}}}]}}"#
            ));
            models(&d, "minecraft:x[OR=x]").len()
        };
        assert_eq!(fall(r#"{"OR": []}"#), 0);
        assert_eq!(fall(r#"{"AND": []}"#), 1);
        assert_eq!(fall(r#"{"OR": "x"}"#), 1);
    }

    /// Hat eine Datei doppelte Schlüssel, gilt wie bei Gson der letzte Wert
    /// auf dem Platz des ersten.
    #[test]
    fn doppelte_schluessel_wie_gson() {
        let d = def(
            r#"{"variants": {"": {"model": "a"}}, "variants": {"lit=true": {"model": "b"}, "": {"model": "c"}, "lit=true": {"model": "d"}}}"#,
        );
        assert_eq!(d.variants.len(), 2);
        assert_eq!(d.variants[0].apply[0].model, "minecraft:d");
        assert_eq!(d.variants[1].apply[0].model, "minecraft:c");
    }

    /// Überlappen sich zwei Schlüssel, geht der Client die Einträge in der
    /// Reihenfolge der Datei durch und die Zustände in der von
    /// `getPossibleStates`. Den ersten gemeinsamen Zustand überschreibt der
    /// spätere Eintrag und bricht dort ab. Heuballen: x, y, z.
    #[test]
    fn ueberlappende_schluessel_wie_im_client() {
        let erst_y = r#"{"variants": {"axis=y": {"model": "a"}, "": {"model": "b"}}}"#;
        assert_eq!(
            exact(erst_y, "minecraft:hay_block[axis=x]").as_deref(),
            Some("minecraft:b")
        );
        assert_eq!(
            exact(erst_y, "minecraft:hay_block[axis=y]").as_deref(),
            Some("minecraft:b")
        );
        assert_eq!(exact(erst_y, "minecraft:hay_block[axis=z]"), None);

        let erst_alle = r#"{"variants": {"": {"model": "b"}, "axis=y": {"model": "a"}}}"#;
        assert_eq!(
            exact(erst_alle, "minecraft:hay_block[axis=x]").as_deref(),
            Some("minecraft:b")
        );
        assert_eq!(
            exact(erst_alle, "minecraft:hay_block[axis=y]").as_deref(),
            Some("minecraft:a")
        );
        assert_eq!(
            exact(erst_alle, "minecraft:hay_block[axis=z]").as_deref(),
            Some("minecraft:b")
        );
    }

    /// Gegen die Definition: Zahlen liest der Client mit `parseInt`, einen
    /// unbekannten Wert oder eine unbekannte Eigenschaft verwirft er mit
    /// dem Eintrag. Weizen hat age 0 bis 7.
    #[test]
    fn schluessel_gegen_die_definition() {
        let json = r#"{"variants": {"age=07": {"model": "a"}, "age=+1": {"model": "b"}, "age=8": {"model": "c"}, "foo=bar": {"model": "d"}, "age=2,foo=bar": {"model": "e"}, "age=5,age=3": {"model": "f"}}}"#;
        let wheat = |age: u8| exact(json, &format!("minecraft:wheat[age={age}]"));
        assert_eq!(wheat(7).as_deref(), Some("minecraft:a"));
        assert_eq!(wheat(1).as_deref(), Some("minecraft:b"));
        assert_eq!(wheat(2), None);
        assert_eq!(wheat(0), None);
        assert_eq!(wheat(3).as_deref(), Some("minecraft:f"));
        assert_eq!(wheat(5), None);
        // Ohne Definition vergleicht der Renderer den Text.
        assert!(models(&def(json), "minecraft:x[age=7]").is_empty());

        let when = r#"{"multipart": [{"when": {"age": "07|+1"}, "apply": {"model": "a"}}]}"#;
        let mut d = def(when);
        let definition = Definition::of("minecraft:wheat").unwrap();
        d.instantiate(definition).unwrap();
        for (age, n) in [(7, 1), (1, 1), (0, 0)] {
            let s = state(&format!("minecraft:wheat[age={age}]"));
            let refs = d.alternatives(&s, definition.index(&s)).unwrap();
            assert_eq!(refs[0].1.len(), n, "age={age}");
        }
    }

    /// Eine Multipart-Bedingung mit unbekannter Eigenschaft oder
    /// unbekanntem Wert wirft beim Instanziieren, etwa eine Mauer aus
    /// einem Pack vor 1.16 mit `"north": "true"`.
    #[test]
    fn unbekannte_bedingung_wirft() {
        let wall = Definition::of("minecraft:cobblestone_wall").unwrap();
        for (when, ok) in [
            (r#"{"north": "low|tall"}"#, true),
            (r#"{"OR": [{"up": "true"}, {"north": "!none"}]}"#, true),
            (r#"{"north": "true"}"#, false),
            (r#"{"OR": [{"up": "true"}, {"nord": "low"}]}"#, false),
            (r#"{"north": "!!low"}"#, false),
            (r#"{"OR": "x"}"#, false),
        ] {
            let mut d = def(&format!(
                r#"{{"multipart": [{{"when": {when}, "apply": {{"model": "m"}}}}]}}"#
            ));
            assert_eq!(d.instantiate(wall).is_ok(), ok, "{when}");
        }
    }

    /// Die Tabelle stimmt mit dem Report von 26.2 überein: 1196 Blöcke,
    /// Eichentreppen mit 80 Zuständen, der erste nach Namen sortiert.
    #[test]
    fn blocktabelle_aus_26_2() {
        assert_eq!(BLOCKS.len(), 1196);
        let stairs = Definition::of("minecraft:oak_stairs").unwrap();
        assert_eq!(stairs.states(), 80);
        let erster =
            state("minecraft:oak_stairs[facing=north,half=top,shape=straight,waterlogged=true]");
        assert_eq!(stairs.index(&erster), Some(0));
        let s =
            state("minecraft:oak_stairs[facing=south,half=top,shape=straight,waterlogged=false]");
        assert_eq!(stairs.index(&s), Some(21));
        assert_eq!(stairs.digit(21, 0), 1);
        assert!(
            stairs
                .index(&state(
                    "minecraft:oak_stairs[facing=up,half=top,shape=straight,waterlogged=true]"
                ))
                .is_none()
        );
        assert!(
            stairs
                .index(&state("minecraft:oak_stairs[facing=north]"))
                .is_none()
        );
        // Im Report stehen die Eigenschaften der Truhe als type, facing,
        // waterlogged; die Zustände zählen sie nach Namen sortiert.
        let chest = Definition::of("minecraft:chest").unwrap();
        let s = state("minecraft:chest[facing=south,type=left,waterlogged=false]");
        assert_eq!(chest.index(&s), Some(9));
        assert!(Definition::of("minecraft:einfarbig").is_none());
        assert!(Definition::of("terranova:stone").is_none());
    }

    /// Seit Minecraft 1.21.11 dürfen Modellverweise auch um Z gedreht sein.
    #[test]
    fn z_drehung_wird_gelesen() {
        let d = def(r#"{"variants": {"": {"model": "m", "x": 90, "y": 180, "z": 270}}}"#);
        let m = &models(&d, "minecraft:x")[0];
        assert_eq!((m.x, m.y, m.z), (90, 180, 270));

        let d = def(r#"{"variants": {"": {"model": "m"}}}"#);
        assert_eq!(models(&d, "minecraft:x")[0].z, 0);
    }
}
