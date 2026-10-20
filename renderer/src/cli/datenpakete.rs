//! Die Datapacks der Welt, wie das Spiel sie wählt: aus `level.dat` und
//! `<weltwurzel>/datapacks/`, als Ordner oder Zip, mit den Overlays für das
//! Datenformat der Welt. Was der Renderer daraus liest, Biome, Bannermuster
//! und Dimensionen, legt er je Pack als eine Datenwurzel in den Cache.
//! Siehe docs/benutzung/assets.md, „Datapacks der Welt“.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail, ensure};
use heroic_map_renderer::world::{DATENVERSION_26_3, World};
use serde_json::Value;

use super::zip::Zip;

/// Was der Renderer unter `data/<namensraum>/` eines Packs liest.
const LISTEN: [&str; 4] = [
    "worldgen/biome/",
    "banner_pattern/",
    "dimension/",
    "dimension_type/",
];

/// Grenzen für das Zip eines Packs: die Datei selbst, jede gelesene Datei
/// darin und alle zusammen, entpackt, wie das Verzeichnis sie nennt.
/// Darüber übergeht der Renderer das Pack, statt den Speicher zu füllen.
const ZIP_HOECHSTENS: u64 = 256 << 20;
const JE_DATEI: usize = 16 << 20;
const ENTPACKT: usize = 256 << 20;

/// Je ID eines Packs der Welt sein Pfad und ob es ein Zip ist.
type Vorhanden = BTreeMap<String, (PathBuf, bool)>;

/// Ein Datapack der Welt in der Reihe des Spiels.
#[derive(Debug)]
pub(super) struct Datenpaket {
    /// Die ID wie im Spiel, `file/<name>`.
    pub id: String,
    pub zip: bool,
    /// Die Overlays, die für das Datenformat der Welt gelten, in der Reihe
    /// aus `pack.mcmeta`; das letzte gewinnt.
    pub overlays: Vec<String>,
    /// Die Datenwurzel im Cache; `None`, wenn das Pack nichts trägt, was der
    /// Renderer liest.
    pub wurzel: Option<PathBuf>,
}

/// Die Datapacks einer Welt: die gelesenen in der Reihe des Spiels und die
/// übergangenen mit Grund.
#[derive(Debug, Default)]
pub(super) struct Gefunden {
    pub pakete: Vec<Datenpaket>,
    pub uebergangen: Vec<(String, String)>,
}

/// Das Datenformat, gegen das das Spiel die Overlays prüft, nach der
/// Datenversion der Welt: `pack_version` in `version.json` von 26.2 und 26.3.
/// Eine unbekannte gilt wie die neueste.
/// Siehe docs/benutzung/assets.md, „Datapacks der Welt“.
fn datenformat(datenversion: Option<i32>) -> (i32, i32) {
    match datenversion {
        Some(v) if v < DATENVERSION_26_3 => (107, 1),
        _ => (121, 0),
    }
}

/// Die Datapacks der Welt, in der Reihe des Spiels, jedes mit seiner
/// Datenwurzel in `cache`. Ohne Weltwurzel oder ohne `datapacks/` keine.
/// Ein Pack, das sich nicht lesen lässt, übergeht es mit Grund; ohne Cache
/// jedes, das etwas trägt.
pub(super) fn der_welt(welt: &World, cache: Option<&Path>) -> Result<Gefunden> {
    let (Some(wurzel), Some((aktiv, aus))) = (welt.wurzel(), welt.datenpakete()?) else {
        return Ok(Gefunden::default());
    };
    let (vorhanden, mut uebergangen) = finde(&wurzel.join("datapacks"));
    let format = datenformat(welt.datenversion());
    let mut pakete = Vec::new();
    for id in waehle(&aktiv, &aus, &vorhanden) {
        let (pfad, zip) = &vorhanden[&id];
        match lies(&id, pfad, *zip, format, cache) {
            Ok(paket) => pakete.push(paket),
            Err(grund) => uebergangen.push((id, format!("{grund:#}"))),
        }
    }
    Ok(Gefunden {
        pakete,
        uebergangen,
    })
}

/// Die Packs in `ordner` wie `FolderRepositorySource.discoverPacks`, nach
/// ID: nur die oberste Ebene, ein Ordner mit der Datei `pack.mcmeta` oder
/// eine Datei auf `.zip`, ID `file/<name>`. Einem Link folgt das Spiel nur mit
/// `allowed_symlinks.txt`; der Renderer übergeht ihn mit Grund. Alles andere
/// lässt er still liegen.
fn finde(ordner: &Path) -> (Vorhanden, Vec<(String, String)>) {
    let mut vorhanden = BTreeMap::new();
    let mut uebergangen = Vec::new();
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return (vorhanden, uebergangen);
    };
    for eintrag in eintraege.flatten() {
        let pfad = eintrag.path();
        let Some(name) = pfad.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let id = format!("file/{name}");
        let Ok(art) = std::fs::symlink_metadata(&pfad) else {
            continue;
        };
        if art.file_type().is_symlink() {
            uebergangen.push((
                id,
                "ein Link; das Spiel folgt ihm nur mit allowed_symlinks.txt".into(),
            ));
        } else if art.is_dir() && pfad.join("pack.mcmeta").is_file() {
            vorhanden.insert(id, (pfad, false));
        } else if art.is_file() && name.ends_with(".zip") {
            vorhanden.insert(id, (pfad, true));
        }
    }
    (vorhanden, uebergangen)
}

/// Die Reihe wie `MinecraftServer.configurePackRepository`: die IDs aus
/// `aktiv`, die es gibt, in ihrer Reihe, dahinter jedes Pack der Welt, das
/// weder dort noch in `aus` steht, nach ID. Ein späteres überschreibt ein
/// früheres. IDs ohne Pack der Welt, etwa `vanilla` oder `paper`, fallen weg.
fn waehle(aktiv: &[String], aus: &[String], vorhanden: &Vorhanden) -> Vec<String> {
    let mut reihe: Vec<String> = Vec::new();
    for id in aktiv {
        if vorhanden.contains_key(id) && !reihe.contains(id) {
            reihe.push(id.clone());
        }
    }
    for id in vorhanden.keys() {
        if !aktiv.contains(id) && !aus.contains(id) {
            reihe.push(id.clone());
        }
    }
    reihe
}

/// Ein Format `[major, minor]` aus `pack.mcmeta`: eine Zahl gilt als
/// `major.0`, mit `oben` als `major.*`, wie `PackFormat.BOTTOM_CODEC` und
/// `TOP_CODEC`.
fn format_von(wert: &Value, oben: bool) -> Option<(i32, i32)> {
    let zahl = |w: &Value| w.as_i64().and_then(|n| i32::try_from(n).ok());
    match wert {
        Value::Array(teile) => match teile.as_slice() {
            [major] => Some((zahl(major)?, if oben { i32::MAX } else { 0 })),
            [major, minor] => Some((zahl(major)?, zahl(minor)?)),
            _ => None,
        },
        _ => Some((zahl(wert)?, if oben { i32::MAX } else { 0 })),
    }
}

/// Die Overlays aus `pack.mcmeta`, die für `format` gelten, in ihrer Reihe,
/// wie `OverlayMetadataSection.overlaysForVersion`. Ein Eintrag ganz ohne
/// Format zählt nicht; einer nur mit `formats`, der Angabe bis Format 81,
/// trifft die Formate von 26.2 und 26.3 nie. Ist die Angabe sonst kaputt,
/// lädt das Spiel das Pack nicht: dann ein Fehler.
fn overlays(meta: &Value, format: (i32, i32)) -> Result<Vec<String>> {
    let Some(angabe) = meta.get("overlays") else {
        return Ok(Vec::new());
    };
    let eintraege = angabe
        .get("entries")
        .and_then(Value::as_array)
        .context("overlays ohne Liste entries")?;
    let mut gelten = Vec::new();
    for eintrag in eintraege {
        let ordner = eintrag
            .get("directory")
            .and_then(Value::as_str)
            .filter(|o| {
                !o.is_empty()
                    && o.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
            .with_context(|| format!("Overlay ohne gültiges directory: {eintrag}"))?;
        let (min, max) = (eintrag.get("min_format"), eintrag.get("max_format"));
        match (min, max) {
            (Some(min), Some(max)) => {
                let (Some(min), Some(max)) = (format_von(min, false), format_von(max, true)) else {
                    bail!("Overlay {ordner}: min_format oder max_format kein Format");
                };
                if min <= format && format <= max {
                    gelten.push(ordner.to_string());
                }
            }
            (None, None) => {}
            _ => bail!("Overlay {ordner}: min_format und max_format nur zusammen"),
        }
    }
    Ok(gelten)
}

/// Liest ein Pack: `pack.mcmeta`, die Overlays für `format` und die Dateien,
/// die der Renderer liest, aus `data/` und den Overlays, das letzte Overlay
/// zuoberst wie `CompositePackResources`. Legt sie als Datenwurzel nach
/// [`ablegen`] in `cache`.
fn lies(
    id: &str,
    pfad: &Path,
    zip: bool,
    format: (i32, i32),
    cache: Option<&Path>,
) -> Result<Datenpaket> {
    let (overlays, dateien) = if zip {
        aus_zip(pfad, format)?
    } else {
        aus_ordner(pfad, format)?
    };
    let wurzel = match dateien.is_empty() {
        true => None,
        false => {
            let cache = cache.context("kein Ordner für den Cache bekannt; --cache-dir angeben")?;
            Some(ablegen(id, &overlays, &dateien, format, cache)?)
        }
    };
    Ok(Datenpaket {
        id: id.to_string(),
        zip,
        overlays,
        wurzel,
    })
}

/// Eine Datei, die der Renderer aus einem Pack liest: ihre Bytes und die
/// Zeit, die sie in der Datenwurzel trägt.
type Quelle = (Vec<u8>, SystemTime);

/// Ist `name` eine `.json` unter `<namensraum>/` in einer der [`LISTEN`]?
/// Nur sie liest das Spiel dort; eine Wurzel ganz ohne sie brächte
/// `Assets::load_data` zum Abbruch.
fn gelistet(name: &str) -> bool {
    name.ends_with(".json")
        && name
            .split_once('/')
            .is_some_and(|(ns, rest)| !ns.is_empty() && LISTEN.iter().any(|l| rest.starts_with(l)))
}

/// `pack.mcmeta` als JSON-Objekt; ohne Abschnitt `pack` mit `description`
/// lädt das Spiel das Pack nicht, auch nicht im Rückfall.
fn meta_von(text: &[u8]) -> Result<Value> {
    let meta: Value = serde_json::from_slice(text).context("pack.mcmeta kein JSON")?;
    ensure!(
        meta.get("pack")
            .and_then(|p| p.get("description"))
            .is_some(),
        "pack.mcmeta ohne pack.description"
    );
    Ok(meta)
}

fn aus_ordner(pfad: &Path, format: (i32, i32)) -> Result<(Vec<String>, BTreeMap<String, Quelle>)> {
    let meta = meta_von(&std::fs::read(pfad.join("pack.mcmeta")).context("pack.mcmeta lesen")?)?;
    let overlays = overlays(&meta, format)?;
    let mut dateien = BTreeMap::new();
    for schicht in std::iter::once("").chain(overlays.iter().map(String::as_str)) {
        let daten = match schicht {
            "" => pfad.join("data"),
            o => pfad.join(o).join("data"),
        };
        sammle(&daten, "", &mut dateien)?;
    }
    Ok((overlays, dateien))
}

/// Kann ein Ordner `rel` unter einer Datenwurzel noch zu einer der
/// [`LISTEN`] führen?
fn auf_dem_weg(rel: &str) -> bool {
    rel.split_once('/').is_none_or(|(_, rest)| {
        LISTEN
            .iter()
            .any(|l| l.starts_with(&format!("{rest}/")) || rest.starts_with(l))
    })
}

/// Jede Datei unter `ordner`, die [`gelistet`] ist, ohne Links, mit ihren
/// Bytes und ihrer Zeit; eine spätere Schicht überschreibt.
fn sammle(ordner: &Path, unter: &str, dateien: &mut BTreeMap<String, Quelle>) -> Result<()> {
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return Ok(());
    };
    for eintrag in eintraege.flatten() {
        let Some(name) = eintrag.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let rel = if unter.is_empty() {
            name
        } else {
            format!("{unter}/{name}")
        };
        let Ok(art) = std::fs::symlink_metadata(eintrag.path()) else {
            continue;
        };
        if art.is_dir() && auf_dem_weg(&rel) {
            sammle(&eintrag.path(), &rel, dateien)?;
        } else if art.is_file() && gelistet(&rel) {
            let bytes = std::fs::read(eintrag.path())
                .with_context(|| format!("{} lesen", eintrag.path().display()))?;
            dateien.insert(rel, (bytes, art.modified()?));
        }
    }
    Ok(())
}

/// Nennt das Verzeichnis für die Dateien, die `nimm` will, mehr als die
/// Grenzen erlauben, ist das ein Fehler.
fn in_grenzen(zip: &Zip, nimm: impl Fn(&str) -> bool) -> Result<()> {
    let (gross, summe) = zip.roh(nimm);
    ensure!(
        gross <= JE_DATEI && summe <= ENTPACKT,
        "entpackt zu gross: {} MiB in einer Datei, {} MiB zusammen; erlaubt sind {} und {}",
        gross >> 20,
        summe >> 20,
        JE_DATEI >> 20,
        ENTPACKT >> 20
    );
    Ok(())
}

fn aus_zip(pfad: &Path, format: (i32, i32)) -> Result<(Vec<String>, BTreeMap<String, Quelle>)> {
    let info = std::fs::metadata(pfad)?;
    ensure!(
        info.len() <= ZIP_HOECHSTENS,
        "Zip grösser als {} MiB",
        ZIP_HOECHSTENS >> 20
    );
    // Jede Datei trägt die Zeit des Zips: ein neues Zip, ein neuer Stand.
    let zeit = info.modified()?;
    let bytes = std::fs::read(pfad).context("lesen")?;
    let zip = Zip::lies(&bytes)?;
    let mut meta = None;
    let ist_meta = |n: &str| n == "pack.mcmeta";
    in_grenzen(&zip, ist_meta)?;
    zip.dateien(ist_meta, |_, b| {
        meta = Some(meta_von(b)?);
        Ok(())
    })?;
    let meta = meta.context("ohne pack.mcmeta")?;
    let overlays = overlays(&meta, format)?;
    let mut je_schicht: Vec<BTreeMap<String, Vec<u8>>> = vec![BTreeMap::new(); overlays.len() + 1];
    let vorsilben: Vec<String> = std::iter::once("data/".to_string())
        .chain(overlays.iter().map(|o| format!("{o}/data/")))
        .collect();
    let ist_gelistet = |n: &str| {
        vorsilben
            .iter()
            .any(|v| n.strip_prefix(v.as_str()).is_some_and(gelistet))
    };
    in_grenzen(&zip, ist_gelistet)?;
    zip.dateien(ist_gelistet, |name, b| {
        for (i, v) in vorsilben.iter().enumerate() {
            if let Some(rel) = name.strip_prefix(v.as_str()).filter(|r| gelistet(r)) {
                je_schicht[i].insert(rel.to_string(), b.to_vec());
            }
        }
        Ok(())
    })?;
    let mut dateien = BTreeMap::new();
    for schicht in je_schicht {
        for (rel, b) in schicht {
            dateien.insert(rel, (b, zeit));
        }
    }
    Ok((overlays, dateien))
}

/// Legt die Dateien eines Packs als Datenwurzel in `<cache>/datapacks/<kennung>`
/// ab, jede mit ihrer Zeit. Die Kennung hängt an ID, Datenformat, Overlays
/// und jeder Datei mit Länge und Zeit: Liegt sie schon da, gilt sie. Erst in
/// einen Ordner daneben, dann umbenannt; legt ein anderer Lauf sie zugleich
/// ab, gilt seine.
fn ablegen(
    id: &str,
    overlays: &[String],
    dateien: &BTreeMap<String, Quelle>,
    format: (i32, i32),
    cache: &Path,
) -> Result<PathBuf> {
    let mut fnv = 0xcbf2_9ce4_8422_2325u64;
    let mut nimm = |bytes: &[u8]| {
        for b in bytes {
            fnv = (fnv ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    nimm(id.as_bytes());
    nimm(&format.0.to_le_bytes());
    nimm(&format.1.to_le_bytes());
    for o in overlays {
        nimm(o.as_bytes());
        nimm(&[0]);
    }
    for (rel, (bytes, zeit)) in dateien {
        let nanos = zeit
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        nimm(rel.as_bytes());
        nimm(&(bytes.len() as u64).to_le_bytes());
        nimm(&nanos.to_le_bytes());
    }
    let ordner = cache.join("datapacks").join(format!("{fnv:016x}"));
    if ordner.is_dir() {
        return Ok(ordner);
    }
    let tmp = cache
        .join("datapacks")
        .join(format!("{fnv:016x}.{}.tmp", std::process::id()));
    let geschrieben = (|| -> Result<()> {
        for (rel, (bytes, zeit)) in dateien {
            let ziel = rel.split('/').fold(tmp.clone(), |p, t| p.join(t));
            std::fs::create_dir_all(ziel.parent().expect("unter tmp"))?;
            let mut datei = std::fs::File::create(&ziel)?;
            datei.write_all(bytes)?;
            datei.set_modified(*zeit)?;
        }
        Ok(())
    })();
    if let Err(e) = geschrieben {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e.context(format!("{} anlegen", tmp.display())));
    }
    if std::fs::rename(&tmp, &ordner).is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        ensure!(ordner.is_dir(), "{} anlegen", ordner.display());
    }
    Ok(ordner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 2026-06-16, ein DOS-Datum für die Zips der Tests.
    const DATUM: u16 = ((2026 - 1980) << 9) | (6 << 5) | 16;

    fn schreibe(wurzel: &Path, rel: &str, inhalt: &str) {
        let pfad = wurzel.join(rel);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    }

    /// Jede Datei unter `ordner` mit ihrem Inhalt, nach Pfad.
    fn inhalt(ordner: &Path) -> BTreeMap<String, String> {
        let mut aus = BTreeMap::new();
        let mut stapel = vec![(ordner.to_path_buf(), String::new())];
        while let Some((pfad, vor)) = stapel.pop() {
            for eintrag in std::fs::read_dir(&pfad).unwrap().flatten() {
                let rel = format!("{vor}{}", eintrag.file_name().to_str().unwrap());
                if eintrag.path().is_dir() {
                    stapel.push((eintrag.path(), format!("{rel}/")));
                } else {
                    aus.insert(rel, std::fs::read_to_string(eintrag.path()).unwrap());
                }
            }
        }
        aus
    }

    /// Erst `Enabled` in seiner Reihe, ohne IDs, die es in der Welt nicht
    /// gibt; dahinter jedes Pack, das in keiner Liste steht, nach ID;
    /// `Disabled` nie. Ein Pack ist ein Ordner mit `pack.mcmeta` oder eine
    /// Datei auf `.zip`, Gross und klein zählen.
    #[test]
    fn reihe_wie_das_spiel() {
        let welt = tempfile::tempdir().unwrap();
        let ordner = welt.path().join("datapacks");
        for pack in ["a", "b", "c"] {
            schreibe(&ordner, &format!("{pack}/pack.mcmeta"), "{}");
        }
        schreibe(&ordner, "ohne/data/x.json", "{}");
        schreibe(&ordner, "e.zip", "");
        schreibe(&ordner, "gross.ZIP", "");
        let (vorhanden, uebergangen) = finde(&ordner);
        assert!(uebergangen.is_empty());
        assert_eq!(
            vorhanden.keys().collect::<Vec<_>>(),
            ["file/a", "file/b", "file/c", "file/e.zip"]
        );
        assert!(vorhanden["file/e.zip"].1 && !vorhanden["file/a"].1);
        let ids = |liste: &[&str]| liste.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let aktiv = ids(&["vanilla", "file/e.zip", "paper", "file/a", "file/fehlt"]);
        assert_eq!(
            waehle(&aktiv, &ids(&["file/c"]), &vorhanden),
            ["file/e.zip", "file/a", "file/b"]
        );
    }

    /// Ein Overlay gilt, wenn das Format der Welt zwischen `min_format` und
    /// `max_format` liegt; eine Zahl heisst unten `major.0` und oben
    /// `major.*`. Die Reihe bleibt, ein Eintrag nur mit `formats` gilt nie.
    /// Eine kaputte Angabe ist ein Fehler.
    #[test]
    fn overlays_nach_format() {
        let meta = json!({"overlays": {"entries": [
            {"directory": "alt", "min_format": 80, "max_format": 100},
            {"directory": "genau_26.2", "min_format": [107, 1], "max_format": 107},
            {"directory": "zu_neu", "min_format": [107, 2], "max_format": [107, 9]},
            {"directory": "ab_26.3", "min_format": 121, "max_format": [121, 0]},
            {"directory": "beide", "min_format": [100], "max_format": 200},
            {"directory": "nur_formats", "formats": [1, 200]},
        ]}});
        assert_eq!(datenformat(Some(DATENVERSION_26_3 - 1)), (107, 1));
        assert_eq!(datenformat(None), (121, 0));
        let gelten = |version| overlays(&meta, datenformat(Some(version))).unwrap();
        assert_eq!(gelten(DATENVERSION_26_3 - 1), ["genau_26.2", "beide"]);
        assert_eq!(gelten(DATENVERSION_26_3), ["ab_26.3", "beide"]);
        for kaputt in [
            json!({"entries": {}}),
            json!({"entries": [{"directory": "a", "min_format": 1}]}),
            json!({"entries": [{"directory": "a/b", "min_format": 1, "max_format": 200}]}),
            json!({"entries": [{"directory": "a", "min_format": "1", "max_format": 200}]}),
        ] {
            assert!(
                overlays(&json!({ "overlays": kaputt }), (107, 1)).is_err(),
                "{kaputt}"
            );
        }
    }

    const META: &str = r#"{"pack": {"description": ""}, "overlays": {"entries": [
        {"directory": "neu", "min_format": 100, "max_format": 200},
        {"directory": "fremd", "min_format": 1, "max_format": 81}]}}"#;

    /// Die Dateien eines Packs, die der Renderer liest, unter `data/` und in
    /// den Overlays.
    const DATEIEN: [(&str, &str); 9] = [
        ("data/beispiel/worldgen/biome/rot.json", "basis"),
        ("neu/data/beispiel/worldgen/biome/rot.json", "neu"),
        ("fremd/data/beispiel/worldgen/biome/rot.json", "fremd"),
        ("fremd/data/beispiel/worldgen/biome/nur_fremd.json", "fremd"),
        ("data/beispiel/worldgen/biome/tief/blau.json", "blau"),
        ("data/beispiel/worldgen/biome/liesmich.txt", "text"),
        ("data/beispiel/banner_pattern/welle.json", "welle"),
        ("data/beispiel/function/f.mcfunction", "say 1"),
        ("data/beispiel/tags/worldgen/biome/t.json", "tag"),
    ];

    /// Aus Ordner und Zip dieselbe Datenwurzel: nur die Listen, das
    /// geltende Overlay über `data/`, das fremde nicht. Liegt sie schon im
    /// Cache, gilt sie; ihre Kennung folgt nur den Dateien, die der Renderer
    /// liest. Ein Pack ohne sie braucht keinen Cache.
    #[test]
    fn ordner_und_zip_als_datenwurzel() {
        let welt = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let ordner = welt.path().join("ordner");
        schreibe(&ordner, "pack.mcmeta", META);
        for (rel, text) in DATEIEN {
            schreibe(&ordner, rel, text);
        }
        let mut eintraege = vec![("pack.mcmeta", META.as_bytes(), true, DATUM)];
        eintraege.extend(DATEIEN.map(|(rel, text)| (rel, text.as_bytes(), false, DATUM)));
        let zip = welt.path().join("paket.zip");
        std::fs::write(&zip, super::super::zip::schreibe(&eintraege, 0)).unwrap();

        let soll = BTreeMap::from(
            [
                ("beispiel/worldgen/biome/rot.json", "neu"),
                ("beispiel/worldgen/biome/tief/blau.json", "blau"),
                ("beispiel/banner_pattern/welle.json", "welle"),
            ]
            .map(|(rel, text)| (rel.to_string(), text.to_string())),
        );
        let lies_mit = |pfad: &Path, zip, cache| lies("file/x", pfad, zip, (107, 1), cache);
        let aus_dem_ordner = lies_mit(&ordner, false, Some(cache.path())).unwrap();
        assert_eq!(aus_dem_ordner.overlays, ["neu"]);
        let wurzel = aus_dem_ordner.wurzel.unwrap();
        assert_eq!(inhalt(&wurzel), soll);
        let aus_dem_zip = lies_mit(&zip, true, Some(cache.path())).unwrap();
        assert_eq!(inhalt(&aus_dem_zip.wurzel.unwrap()), soll);

        schreibe(&ordner, "data/beispiel/function/f.mcfunction", "say 22");
        let wieder = lies_mit(&ordner, false, Some(cache.path())).unwrap();
        assert_eq!(
            wieder.wurzel.unwrap(),
            wurzel,
            "Funktion ändert die Kennung"
        );
        schreibe(
            &ordner,
            "neu/data/beispiel/worldgen/biome/rot.json",
            "neuer",
        );
        let neu = lies_mit(&ordner, false, Some(cache.path())).unwrap();
        assert_ne!(neu.wurzel.unwrap(), wurzel, "Biom ändert die Kennung nicht");
        assert!(
            format!("{:#}", lies_mit(&ordner, false, None).unwrap_err()).contains("--cache-dir")
        );

        let leer = welt.path().join("leer");
        schreibe(&leer, "pack.mcmeta", META);
        schreibe(&leer, "data/beispiel/function/f.mcfunction", "say 1");
        schreibe(&leer, "data/beispiel/worldgen/biome/liesmich.txt", "text");
        assert!(lies_mit(&leer, false, None).unwrap().wurzel.is_none());
        schreibe(&leer, "pack.mcmeta", r#"{"pack": {}}"#);
        assert!(lies_mit(&leer, false, None).is_err());
        std::fs::write(&zip, "kein Zip").unwrap();
        assert!(lies_mit(&zip, true, Some(cache.path())).is_err());
    }

    /// Ein Zip, das zu viel nennt, übergeht der Renderer, bevor er entpackt:
    /// über 16 MiB für eine gelesene Datei, über 256 MiB für alle, auch für
    /// `pack.mcmeta`, und ein Zip über 256 MiB.
    #[test]
    fn zu_grosse_zips() {
        let welt = tempfile::tempdir().unwrap();
        let pfad = welt.path().join("gross.zip");
        let namen: Vec<String> = (0..17)
            .map(|i| format!("data/beispiel/worldgen/biome/b{i}.json"))
            .collect();
        let mut eintraege = vec![("pack.mcmeta", META.as_bytes(), false, DATUM)];
        eintraege.extend(
            namen
                .iter()
                .map(|n| (n.as_str(), b"{}".as_slice(), false, DATUM)),
        );
        let zip = super::super::zip::schreibe(&eintraege, 0);
        let fehler = || {
            format!(
                "{:#}",
                lies("file/gross.zip", &pfad, true, (107, 1), None).unwrap_err()
            )
        };
        // Setzt im Verzeichnis die entpackte Grösse der Einträge.
        let nennt = |groessen: &[(usize, u32)]| {
            let mut zip = zip.clone();
            let koepfe: Vec<usize> = (0..zip.len() - 3)
                .filter(|&i| zip[i..i + 4] == *b"PK\x01\x02")
                .collect();
            for &(nummer, groesse) in groessen {
                let bei = koepfe[nummer] + 24;
                zip[bei..bei + 4].copy_from_slice(&groesse.to_le_bytes());
            }
            std::fs::write(&pfad, zip).unwrap();
            fehler()
        };
        assert!(nennt(&[(1, (16 << 20) + 1)]).contains("entpackt zu gross"));
        let alle: Vec<(usize, u32)> = (1..=17).map(|i| (i, 16 << 20)).collect();
        assert!(nennt(&alle).contains("entpackt zu gross"));
        assert!(nennt(&[(0, 0xF000_0000)]).contains("entpackt zu gross"));
        std::fs::File::create(&pfad)
            .unwrap()
            .set_len(ZIP_HOECHSTENS + 1)
            .unwrap();
        assert!(fehler().contains("Zip grösser als 256 MiB"));
    }
}
