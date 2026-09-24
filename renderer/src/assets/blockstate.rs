use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::{Number, Value};

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
        let json = super::parse_json(text, true)?;
        ensure!(json.is_object(), "kein Objekt");
        let variants = match field(&json, "variants") {
            None => Vec::new(),
            Some(Value::Object(entries)) => {
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
        let multipart = match field(&json, "multipart") {
            None => None,
            Some(Value::Array(cases)) => {
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
    /// solche Multipart-Bedingung dagegen wirft im Client von 26.2, dann
    /// hat der Block über alle Packs kein Modell. Ob die Assets zu 26.2
    /// gehören, weiss der Renderer aber nicht; in einer späteren Version
    /// gibt es die Eigenschaft oder den Wert vielleicht. Er vergleicht dort
    /// den Text, wie bei einem Block, den 26.2 nicht kennt, und gibt zurück,
    /// was die Definition nicht kennt.
    pub fn instantiate(&mut self, definition: &Definition) -> BTreeSet<String> {
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
        let mut unbekannt = BTreeSet::new();
        for case in self.multipart.iter_mut().flatten() {
            if let Some(when) = &mut case.when {
                when.instantiate(definition, &mut unbekannt);
            }
        }
        unbekannt
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
    /// trägt. Was die Definition nicht kennt, bleibt Text und kommt nach
    /// `unbekannt`.
    fn instantiate(&mut self, definition: &Definition, unbekannt: &mut BTreeSet<String>) {
        match self {
            Condition::Props(props) => {
                for (name, terms) in props {
                    let Some(prop) = definition.prop(name) else {
                        unbekannt.insert(format!("Eigenschaft {name}"));
                        continue;
                    };
                    for term in terms {
                        match definition.value(prop, &term.value) {
                            Some(value) => term.value = definition.props[prop].1[value].to_string(),
                            None => {
                                unbekannt.insert(format!("Wert {} für {name}", term.value));
                            }
                        }
                    }
                }
            }
            Condition::And(list) | Condition::Or(list) => {
                for condition in list {
                    condition.instantiate(definition, unbekannt);
                }
            }
        }
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
fn parse_apply(json: &Value) -> Result<Vec<ModelRef>> {
    let Value::Array(list) = json else {
        return Ok(vec![parse_model_ref(json)?]);
    };
    ensure!(!list.is_empty(), "leere Modellliste");
    let mut refs = Vec::with_capacity(list.len());
    let mut total = 0u64;
    for element in list {
        let weight = match field(element, "weight") {
            None => 1,
            Some(weight) => int(weight).context("weight")?,
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
fn parse_model_ref(json: &Value) -> Result<ModelRef> {
    ensure!(json.is_object(), "Modellverweis ist kein Objekt");
    let model = match field(json, "model") {
        Some(Value::String(id)) => identifier(id)?,
        Some(_) => bail!("model ist kein Text"),
        None => bail!("Modellverweis ohne model"),
    };
    let uvlock = match field(json, "uvlock") {
        None => false,
        Some(Value::Bool(uvlock)) => *uvlock,
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
fn quadrant(json: &Value, axis: &str) -> Result<i32> {
    let Some(value) = field(json, axis) else {
        return Ok(0);
    };
    let degrees = int(value).context(axis.to_string())?;
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
        "{text} ist kein gültiger Name"
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
fn parse_case(json: &Value) -> Result<Case> {
    ensure!(json.is_object(), "Multipart-Fall ist kein Objekt");
    let apply = field(json, "apply").ok_or_else(|| anyhow!("Multipart-Fall ohne apply"))?;
    Ok(Case {
        when: field(json, "when").map(parse_condition).transpose()?,
        apply: parse_apply(apply)?,
    })
}

/// `Condition.CODEC`: genau ein Schlüssel `OR` oder `AND` mit einer Liste,
/// sonst eine nichtleere Tabelle von Eigenschaften. Ein `OR` mit Text statt
/// Liste ist also eine Eigenschaft namens `OR`.
fn parse_condition(json: &Value) -> Result<Condition> {
    let Value::Object(entries) = json else {
        bail!("Bedingung ist kein Objekt");
    };
    if entries.len() == 1
        && let Some((op, Value::Array(list))) = entries.iter().next()
        && (op == "OR" || op == "AND")
    {
        let list = list.iter().map(parse_condition).collect::<Result<_>>()?;
        return Ok(if op == "OR" {
            Condition::Or(list)
        } else {
            Condition::And(list)
        });
    }
    ensure!(!entries.is_empty(), "leere Bedingung");
    Ok(Condition::Props(
        entries
            .iter()
            .map(|(name, value)| {
                let terms = parse_terms(value).with_context(|| format!("Bedingung {name}"))?;
                Ok((name.clone(), terms))
            })
            .collect::<Result<_>>()?,
    ))
}

/// `Terms.CODEC`: ein Text, alte Packs schreiben auch Zahlen und
/// Wahrheitswerte. Getrennt an `|`, ein `!` vorn verneint den Term, und
/// ein leerer Term wirft (`Empty term`).
fn parse_terms(json: &Value) -> Result<Vec<Term>> {
    let text = match json {
        Value::String(text) => text.clone(),
        Value::Bool(value) => value.to_string(),
        number => int(number)?.to_string(),
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

/// Ein Feld, wie DFU es liest: `null` zählt als fehlend. Das JSON selbst
/// liest serde_json wie Gson: Objekte in der Reihenfolge der Datei, ein
/// doppelter Schlüssel behält seinen ersten Platz und nimmt den letzten
/// Wert (`LinkedTreeMap.put`), Zahlen so, wie sie dastehen.
// ponytail: ein Objekt, dessen erster Schlüssel `$serde_json::private::Number`
// heisst, liest serde_json als Zahl. So schreibt kein Pack.
pub(super) fn field<'a>(json: &'a Value, name: &str) -> Option<&'a Value> {
    json.get(name).filter(|value| !value.is_null())
}

/// `Codec.INT`: nur eine JSON-Zahl, abgeschnitten wie `Number.intValue`.
pub(super) fn int(json: &Value) -> Result<i32> {
    match json {
        Value::Number(number) => int_value(number),
        _ => bail!("keine Zahl"),
    }
}

/// `Codec.FLOAT`: nur eine JSON-Zahl, gerundet wie `Float.parseFloat`, eine
/// zu grosse also unendlich.
pub(super) fn float(json: &Value) -> Result<f32> {
    match json {
        Value::Number(number) => number
            .as_str()
            .parse()
            .with_context(|| format!("Zahl {number}")),
        _ => bail!("keine Zahl"),
    }
}

/// `Codec.BOOL`: nur ein Wahrheitswert, wie `JsonOps.getBooleanValue`.
pub(super) fn boolean(json: &Value) -> Result<bool> {
    json.as_bool().ok_or_else(|| anyhow!("kein Wahrheitswert"))
}

/// Gsons `LazilyParsedNumber.intValue` aus der Zahl, wie sie in der Datei
/// steht: `Integer.parseInt`, dann `Long.parseLong`, sonst `BigDecimal`,
/// Richtung 0 abgeschnitten. Es zählen die unteren 32 Bit, 2^32 + 90 ist
/// also 90 und 90.5 auch. Über `NumberLimits` wirft eine Zahl mit einer
/// Skala ab 10000, und `BigDecimal` eine, deren Exponent kein `int` ist.
/// Länger als 1023 Zeichen ist keine, das prüft schon [`super::parse_json`].
pub(super) fn int_value(number: &Number) -> Result<i32> {
    let text = number.as_str();
    if let Ok(n) = text.parse::<i64>() {
        return Ok(n as i32);
    }
    let (mantissa, exponent) = text.split_once(['e', 'E']).unwrap_or((text, "0"));
    let (negative, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, mantissa),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let exponent: i32 = exponent
        .parse()
        .with_context(|| format!("Exponent von {text}"))?;
    let scale = fraction.len() as i64 - i64::from(exponent);
    ensure!(scale.abs() < 10_000, "Skala {scale} von {text}");
    let digits = format!("{whole}{fraction}");
    let kept = &digits[..digits.len().saturating_sub(scale.max(0) as usize)];
    let mut low = kept.bytes().fold(0u32, |low, digit| {
        low.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'))
    });
    // Ab 10^32 sind die unteren 32 Bit null.
    for _ in 0..(-scale).clamp(0, 32) {
        low = low.wrapping_mul(10);
    }
    let low = low as i32;
    Ok(if negative { low.wrapping_neg() } else { low })
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
        assert!(d.instantiate(definition).is_empty());
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
            r#"{"variants": {"": [{"model": "m", "weight": 1e10000}]}}"#.to_string(),
            format!(r#"{{"variants": {{"": [{{"model": "m", "weight": 1{}}}]}}}}"#, "0".repeat(1023)),
            r#"{"variants": {"": [{"model": "m", "weight": 1e400}]}}"#.to_string(),
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
            ("18446744073709551617", 1),
            ("1.99999999999999999999", 1),
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
        assert!(d.instantiate(definition).is_empty());
        for (age, n) in [(7, 1), (1, 1), (0, 0)] {
            let s = state(&format!("minecraft:wheat[age={age}]"));
            let refs = d.alternatives(&s, definition.index(&s)).unwrap();
            assert_eq!(refs[0].1.len(), n, "age={age}");
        }
    }

    /// Eine Multipart-Bedingung mit unbekannter Eigenschaft oder
    /// unbekanntem Wert bleibt Text, und `instantiate` nennt sie. So steht
    /// eine Mauer aus einem Pack vor 1.16 mit `"north": "true"` da, aber
    /// auch eine aus einer Version, die den Wert kennt. Der Client von 26.2
    /// wirft dort.
    #[test]
    fn unbekannte_bedingung_bleibt_text() {
        let wall = Definition::of("minecraft:cobblestone_wall").unwrap();
        for (when, unbekannt) in [
            (r#"{"north": "low|tall"}"#, ""),
            (r#"{"OR": [{"up": "true"}, {"north": "!none"}]}"#, ""),
            (r#"{"north": "true|low"}"#, "Wert true für north"),
            (
                r#"{"OR": [{"up": "true"}, {"nord": "low"}]}"#,
                "Eigenschaft nord",
            ),
            (r#"{"north": "!!low"}"#, "Wert !low für north"),
            (r#"{"OR": "x"}"#, "Eigenschaft OR"),
            (
                r#"{"nord": "low", "north": "true"}"#,
                "Eigenschaft nord, Wert true für north",
            ),
            (
                r#"{"AND": [{"up": "ja"}, {"up": "ja|nein"}]}"#,
                "Wert ja für up, Wert nein für up",
            ),
        ] {
            let mut d = def(&format!(
                r#"{{"multipart": [{{"when": {when}, "apply": {{"model": "m"}}}}]}}"#
            ));
            let gefunden: Vec<String> = d.instantiate(wall).into_iter().collect();
            assert_eq!(gefunden.join(", "), unbekannt, "{when}");
        }
        // Als Text passt die neue Mauer, eine aus 26.2 trifft keinen Fall.
        let mut d = def(r#"{"multipart": [{"when": {"north": "true"}, "apply": {"model": "m"}}]}"#);
        assert_eq!(d.instantiate(wall).len(), 1);
        let neu = state("minecraft:cobblestone_wall[north=true]");
        assert_eq!(
            d.alternatives(&neu, wall.index(&neu)).unwrap()[0].1.len(),
            1
        );
        let alt = state(
            "minecraft:cobblestone_wall[east=none,north=low,south=none,up=true,waterlogged=false,west=none]",
        );
        assert_eq!(
            d.alternatives(&alt, wall.index(&alt)).unwrap()[0].1.len(),
            0
        );
        // Unbekanntes gilt nicht als wahr: `nord` hat die Mauer nicht.
        let mut d = def(r#"{"multipart": [{"when": {"nord": "low"}, "apply": {"model": "m"}}]}"#);
        assert_eq!(d.instantiate(wall).len(), 1);
        assert_eq!(
            d.alternatives(&alt, wall.index(&alt)).unwrap()[0].1.len(),
            0
        );
        // Neben Unbekanntem liest der Renderer das Bekannte wie der Client,
        // `07` ist 7, davor und dahinter.
        let weizen = Definition::of("minecraft:wheat").unwrap();
        let reif = state("minecraft:wheat[age=7]");
        for oder in [
            r#"[{"age": "07"}, {"foo": "x"}]"#,
            r#"[{"foo": "x"}, {"age": "07"}]"#,
        ] {
            let mut d = def(&format!(
                r#"{{"multipart": [{{"when": {{"OR": {oder}}}, "apply": {{"model": "m"}}}}]}}"#
            ));
            let unbekannt: Vec<String> = d.instantiate(weizen).into_iter().collect();
            assert_eq!(unbekannt, ["Eigenschaft foo"], "{oder}");
            assert_eq!(
                d.alternatives(&reif, weizen.index(&reif)).unwrap()[0]
                    .1
                    .len(),
                1,
                "{oder}"
            );
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

    /// Die Tabelle aus Gson 2.14.0, gegengeprüft mit dem echten
    /// `LazilyParsedNumber`, dazu Zahlen, die f64 nicht fasst.
    #[test]
    fn int_value_wie_gson() {
        let zahl = |text: &str| int_value(&text.parse().unwrap());
        for (text, soll) in [
            ("1.0", 1),
            ("1.5", 1),
            ("1e2", 100),
            ("1.50E+1", 15),
            ("3000000000", -1294967296),
            ("-0.5", 0),
            ("-1.5", -1),
            ("2147483648", -2147483648),
            ("1e10", 1410065408),
            ("1e32", 0),
            ("1e9999", 0),
            ("18446744073709551617", 1),
            ("-18446744073709551617", -1),
            ("2.99999999999999999999", 2),
            ("123456789012e-3", 123456789),
        ] {
            assert_eq!(zahl(text).unwrap(), soll, "{text}");
        }
        assert!(zahl("1e10000").is_err());
        assert!(zahl("1e-10000").is_err());
        // Ein Exponent, der kein `int` ist, und einer, bei dem die Skala
        // in i128 überliefe: `BigDecimal` wirft, der Renderer auch.
        assert!(zahl("1e2147483648").is_err());
        assert!(zahl("1e-170141183460469231731687303715884105728").is_err());
        assert_eq!(zahl(&format!("1{}", "0".repeat(1_000))).unwrap(), 0);
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
