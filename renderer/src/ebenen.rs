//! Was der Renderer aus den Dateien der Ebenen liest: die Kennung und die
//! Entwürfe der Banner, für `--banners`. Alles andere einer Ebene zeichnen
//! die Ansichten selbst.
//! Siehe docs/benutzung/ebenen.md, „Entwürfe“, und
//! docs/entscheidungen/0100-der-renderer-zeichnet-die-banner.md.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

use crate::world::Muster;

/// Die 16 Farbstoffe des Spiels, wie `DyeColor` sie nennt.
pub const FARBSTOFFE: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// Höchstens so viele Entwürfe je Ebene, wie Bilder.
pub const HOECHSTENS_ENTWUERFE: usize = 200;

/// Höchstens so viele Lagen je Entwurf, wie das Spiel zeichnet.
pub const HOECHSTENS_LAGEN: usize = 16;

/// Ein Teil der Kennung einer Ebene, der Name eines Bilds ohne Endung oder
/// eines Entwurfs: 1 bis 64 Zeichen aus `a`–`z`, `0`–`9`, `_`, `-` und `.`,
/// kein Punkt vorn, wo das Plugin halbe Dateien schreibt, keiner hinten, den
/// Windows streicht, und kein Gerät von Windows.
/// Siehe docs/benutzung/ebenen.md, „Kennung“.
pub fn ist_teil(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !geraet(name)
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
        })
}

/// Ob Windows unter diesem Namen ein Gerät öffnet, in jedem Ordner und mit
/// jeder Endung: `con`, `prn`, `aux`, `nul`, `com0` bis `com9`, `lpt0` bis
/// `lpt9`.
pub fn geraet(teil: &str) -> bool {
    let stamm = teil.split('.').next().unwrap_or(teil).to_ascii_lowercase();
    match stamm.as_bytes() {
        b"con" | b"prn" | b"aux" | b"nul" => true,
        [b'c', b'o', b'm', d] | [b'l', b'p', b't', d] => d.is_ascii_digit(),
        _ => false,
    }
}

/// `modname` und Name aus der `id` des JSON einer Ebene, wenn sie der Regel
/// für eine Kennung folgt; auch dann, wenn sich ihre Entwürfe nicht lesen
/// lassen.
pub fn kennung(json: &Value) -> Option<(&str, &str)> {
    json.get("id")?
        .as_str()?
        .split_once(':')
        .filter(|(m, n)| ist_teil(m) && ist_teil(n))
}

/// Ein Entwurf eines Banners: Grundfarbe und Lagen wie im Spiel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entwurf {
    /// Einer der [`FARBSTOFFE`].
    pub base: String,
    /// Je Lage das Muster und sein Farbstoff, von unten nach oben.
    pub layers: Vec<(Muster, String)>,
}

/// Was der Renderer aus der Datei einer Ebene liest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ebene {
    /// Der `modname` vor dem `:` der Kennung.
    pub modname: String,
    /// Der Teil der Kennung nach dem `:`.
    pub name: String,
    /// Die Entwürfe nach ihrem Namen.
    pub designs: BTreeMap<String, Entwurf>,
}

impl Ebene {
    /// Liest `id` und `designs` aus dem JSON einer Ebene; Objekte und alle
    /// übrigen Felder dürfen fehlen und zählen nicht. Was gegen die Grenzen
    /// verstösst, ist ein Fehler: eine Kennung oder ein Name ausserhalb der
    /// Regel, mehr als [`HOECHSTENS_ENTWUERFE`] Entwürfe, mehr als
    /// [`HOECHSTENS_LAGEN`] Lagen, ein Farbstoff, den es nicht gibt, ein
    /// Muster nicht in der Form `namespace:pfad`. Ein Muster, das der Renderer
    /// nicht kennt, bleibt; das Bild lässt es weg und meldet es.
    pub fn lies(text: &str) -> Result<Ebene> {
        let json: Value = serde_json::from_str(text).context("kein JSON")?;
        let id = json
            .get("id")
            .and_then(Value::as_str)
            .context("ohne `id`")?;
        let (modname, name) =
            kennung(&json).with_context(|| format!("`{id}` ist keine Kennung `modname:ebene`"))?;
        let mut designs = BTreeMap::new();
        if let Some(liste) = json.get("designs") {
            let liste = liste
                .as_object()
                .with_context(|| format!("{id}: `designs` ist kein Objekt"))?;
            ensure!(
                liste.len() <= HOECHSTENS_ENTWUERFE,
                "{id}: {} Entwürfe, höchstens {HOECHSTENS_ENTWUERFE}",
                liste.len()
            );
            for (entwurf, wert) in liste {
                ensure!(
                    ist_teil(entwurf),
                    "{id}: `{entwurf}` ist kein Name eines Entwurfs"
                );
                let gelesen =
                    lies_entwurf(wert).with_context(|| format!("{id}, Entwurf {entwurf}"))?;
                designs.insert(entwurf.clone(), gelesen);
            }
        }
        Ok(Ebene {
            modname: modname.to_string(),
            name: name.to_string(),
            designs,
        })
    }
}

fn lies_entwurf(wert: &Value) -> Result<Entwurf> {
    let farbstoff = |wert: Option<&Value>, feld: &str| -> Result<String> {
        let farbe = wert
            .and_then(Value::as_str)
            .with_context(|| format!("ohne `{feld}`"))?;
        ensure!(FARBSTOFFE.contains(&farbe), "`{farbe}` ist kein Farbstoff");
        Ok(farbe.to_string())
    };
    let base = farbstoff(wert.get("base"), "base")?;
    let mut layers = Vec::new();
    if let Some(lagen) = wert.get("layers") {
        let Some(lagen) = lagen.as_array() else {
            bail!("`layers` ist keine Liste");
        };
        ensure!(
            lagen.len() <= HOECHSTENS_LAGEN,
            "{} Lagen, höchstens {HOECHSTENS_LAGEN}",
            lagen.len()
        );
        for (i, lage) in lagen.iter().enumerate() {
            let muster = lage
                .get("pattern")
                .and_then(Value::as_str)
                .with_context(|| format!("Lage {i} ohne `pattern`"))?;
            let form = muster
                .split_once(':')
                .is_some_and(|(ns, pfad)| !ns.is_empty() && !pfad.is_empty());
            ensure!(form, "Lage {i}: `{muster}` ist nicht `namespace:pfad`");
            let farbe =
                farbstoff(lage.get("color"), "color").with_context(|| format!("Lage {i}"))?;
            layers.push((Muster::Id(muster.to_string()), farbe));
        }
    }
    Ok(Entwurf { base, layers })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ebene(designs: &str) -> Result<Ebene> {
        Ebene::lies(&format!(
            r#"{{"id": "beispiel:staedte", "name": {{"de": "Städte"}}, "designs": {designs}, "objects": []}}"#
        ))
    }

    /// Ein Entwurf wie im Beispiel von `ebenen.md`; Objekte und Namen zählen
    /// nicht, ein unbekanntes Muster bleibt.
    #[test]
    fn entwurf_wie_im_format() {
        let e = ebene(
            r#"{"nordreich": {"base": "white", "layers": [
                {"pattern": "minecraft:stripe_bottom", "color": "red"},
                {"pattern": "minecraft:gibt_es_nicht", "color": "light_blue"}]},
              "white": {"base": "white"}}"#,
        )
        .unwrap();
        assert_eq!(
            (e.modname.as_str(), e.name.as_str()),
            ("beispiel", "staedte")
        );
        assert_eq!(e.designs.len(), 2);
        let nordreich = &e.designs["nordreich"];
        assert_eq!(nordreich.base, "white");
        assert_eq!(
            nordreich.layers,
            vec![
                (Muster::Id("minecraft:stripe_bottom".into()), "red".into()),
                (
                    Muster::Id("minecraft:gibt_es_nicht".into()),
                    "light_blue".into()
                ),
            ]
        );
        assert!(e.designs["white"].layers.is_empty());
        // Ohne `designs` keine Entwürfe, ohne Fehler.
        let leer = Ebene::lies(r#"{"id": "beispiel:wege"}"#).unwrap();
        assert!(leer.designs.is_empty());
    }

    /// Eine UUID als Name eines Entwurfs, wie Nations sie schickt.
    #[test]
    fn uuid_als_name() {
        let e = ebene(r#"{"0f8fad5b-d9cb-469f-a165-70867728950e": {"base": "black"}}"#).unwrap();
        assert!(
            e.designs
                .contains_key("0f8fad5b-d9cb-469f-a165-70867728950e")
        );
    }

    /// Jede Grenze aus dem Format ist ein Fehler, kein stilles Abschneiden.
    #[test]
    fn entwurf_ausserhalb_der_grenzen() {
        let lage = r#"{"pattern": "minecraft:stripe_top", "color": "red"}"#;
        let siebzehn = format!(
            r#"{{"x": {{"base": "white", "layers": [{}]}}}}"#,
            [lage; 17].join(",")
        );
        let sechzehn = format!(
            r#"{{"x": {{"base": "white", "layers": [{}]}}}}"#,
            [lage; 16].join(",")
        );
        assert!(ebene(&sechzehn).is_ok());
        let zweihunderteins: Vec<String> = (0..201)
            .map(|i| format!(r#""e{i}": {{"base": "white"}}"#))
            .collect();
        let zweihundert: Vec<String> = (0..200)
            .map(|i| format!(r#""e{i}": {{"base": "white"}}"#))
            .collect();
        assert!(ebene(&format!("{{{}}}", zweihundert.join(","))).is_ok());
        for falsch in [
            siebzehn,
            format!("{{{}}}", zweihunderteins.join(",")),
            r#"{"x": {"base": "rosa"}}"#.to_string(),
            r#"{"x": {"base": "white", "layers": [{"pattern": "minecraft:stripe_top", "color": "rosa"}]}}"#.to_string(),
            r#"{"x": {"base": "white", "layers": [{"pattern": "stripe_top", "color": "red"}]}}"#.to_string(),
            r#"{"x": {"layers": []}}"#.to_string(),
            r#"{"Gross": {"base": "white"}}"#.to_string(),
            r#"{"con": {"base": "white"}}"#.to_string(),
            r#"{".x": {"base": "white"}}"#.to_string(),
            r#"[]"#.to_string(),
        ] {
            assert!(ebene(&falsch).is_err(), "{falsch}");
        }
        assert!(Ebene::lies(r#"{"id": "staedte"}"#).is_err());
        assert!(Ebene::lies(r#"{"id": "Beispiel:staedte"}"#).is_err());
        // Die Kennung bleibt lesbar, auch wenn die Entwürfe es nicht sind.
        let kaputt: Value =
            serde_json::from_str(r#"{"id": "beispiel:staedte", "designs": []}"#).unwrap();
        assert_eq!(kennung(&kaputt), Some(("beispiel", "staedte")));
        let ohne: Value = serde_json::from_str(r#"{"id": "Beispiel:staedte"}"#).unwrap();
        assert_eq!(kennung(&ohne), None);
    }

    /// Die Regel für einen Teil der Kennung.
    #[test]
    fn teil_der_kennung() {
        for gut in [
            "staedte",
            "burg_16",
            "a.b",
            "0f8fad5b-d9cb",
            &"a".repeat(64),
        ] {
            assert!(ist_teil(gut), "{gut}");
        }
        for schlecht in [
            "",
            ".x",
            "x.",
            "A",
            "a b",
            "con",
            "com1.png",
            "lpt9",
            &"a".repeat(65),
        ] {
            assert!(!ist_teil(schlecht), "{schlecht}");
        }
    }
}
