//! `--banners`: die Sprites der Banner zu den Entwürfen der Ebenen, je Satz
//! ohne und mit Krone, dazu `satz.json`, die Stempel und das Aufräumen von
//! `<out>/<modname>/banner/`.
//! Siehe docs/benutzung/ebenen.md, „Sprites“, und
//! docs/entscheidungen/0100-der-renderer-zeichnet-die-banner.md.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use heroic_map_renderer::assets::Assets;
use heroic_map_renderer::ebenen::{Ebene, ist_teil, kennung};
use heroic_map_renderer::render::banner::{self, Leinwand};
use heroic_map_renderer::render::{Kamera, Richtung};
use image::RgbaImage;
use serde_json::{Value, json};

use super::server::ist_baum;
use super::{BAEUME, tausche};

/// Der Satz, den jeder Baum von oben und der Mod nehmen: `north-45` aus `s`.
const OBEN: &str = "oben";
/// Fuss und Winkel eines Satzes.
const SATZ: &str = "satz.json";
/// Die Stempel eines Satzes, je Entwurf einer. Mit dem Punkt vorn liefert
/// der Server ihn nie aus.
const STEMPEL: &str = ".stempel";
/// Der Ordner der Sprites mit Krone in einem Satz.
const KRONE: &str = "krone";
/// Höchstens so gross ist die Datei einer Ebene.
/// Siehe docs/benutzung/ebenen.md, „Grenzen“.
const HOECHSTENS_DATEI: u64 = 4 << 20;

/// Ein Satz: der Name seines Ordners, Kamera und Richtung.
struct Satz {
    name: String,
    kamera: Kamera,
    richtung: Richtung,
}

/// Eine Ebene als `modname` und Name.
type Kennung = (String, String);

/// Zeichnet die Sprites der Ebenen in `dateien` nach `out` und räumt
/// `<out>/*/banner/` auf. Liefert die Meldung für die letzte Zeile:
/// `changed`, die Ebenen mit geänderten Sprites, und `failed`, die Ebenen,
/// die nicht gingen. Eine Ebene, die nicht geht, gilt als übergeben: Ihre
/// Sprites bleiben. Ist nicht einmal ihre Kennung zu lesen, räumt der
/// Aufruf gar nicht auf. Ein Fehler hier heisst, dass nichts ging.
pub(super) fn banners(
    assets: &mut Assets,
    dateien: &[PathBuf],
    out: &Path,
    tiles: Option<&Path>,
    abdruck: u64,
) -> Result<Value> {
    std::fs::create_dir_all(out).with_context(|| format!("{} anlegen", out.display()))?;
    let saetze = saetze(tiles)?;
    let mut ebenen: BTreeMap<Kennung, Ebene> = BTreeMap::new();
    let mut kaputt: BTreeSet<Kennung> = BTreeSet::new();
    let mut failed: BTreeSet<String> = BTreeSet::new();
    let mut aufraeumen = true;
    for datei in dateien {
        match lies_ebene(datei) {
            Ok(ebene) => {
                let id = (ebene.modname.clone(), ebene.name.clone());
                if kaputt.contains(&id) || ebenen.remove(&id).is_some() {
                    eprintln!("{}: {}:{} doppelt übergeben", datei.display(), id.0, id.1);
                    failed.insert(format!("{}:{}", id.0, id.1));
                    kaputt.insert(id);
                } else {
                    ebenen.insert(id, ebene);
                }
            }
            Err((id, grund)) => {
                eprintln!("{}: {grund:#}", datei.display());
                match id {
                    Some(id) => {
                        failed.insert(format!("{}:{}", id.0, id.1));
                        ebenen.remove(&id);
                        kaputt.insert(id);
                    }
                    None => {
                        failed.insert(datei.display().to_string());
                        aufraeumen = false;
                    }
                }
            }
        }
    }

    let mut changed: BTreeSet<String> = BTreeSet::new();
    for satz in &saetze {
        let leinwand = Leinwand::fuer(assets, satz.kamera, satz.richtung)?;
        let satz_json = serde_json::to_vec_pretty(&json!({
            "foot": [leinwand.fuss.0, leinwand.fuss.1],
            "angle": leinwand.winkel,
        }))?;
        for (id, ebene) in &ebenen {
            if kaputt.contains(id) {
                continue;
            }
            let ordner = out.join(&id.0).join("banner").join(&id.1).join(&satz.name);
            match zeichne_satz(assets, satz, &leinwand, &satz_json, ebene, &ordner, abdruck) {
                Ok(true) => {
                    changed.insert(format!("{}:{}", id.0, id.1));
                }
                Ok(false) => {}
                Err(grund) => {
                    eprintln!("{}:{}, Satz {}: {grund:#}", id.0, id.1, satz.name);
                    failed.insert(format!("{}:{}", id.0, id.1));
                    kaputt.insert(id.clone());
                }
            }
        }
    }
    if aufraeumen {
        let namen: BTreeSet<&str> = saetze.iter().map(|s| s.name.as_str()).collect();
        for id in raeume_auf(out, &ebenen, &kaputt, &namen)? {
            changed.insert(format!("{}:{}", id.0, id.1));
        }
    } else {
        eprintln!("eine Datei ohne lesbare Kennung: nichts aufgeräumt");
    }
    let changed: BTreeSet<&String> = changed.iter().filter(|id| !failed.contains(*id)).collect();
    Ok(json!({ "changed": changed, "failed": failed }))
}

/// Die Sätze: immer `oben`, dazu je Baum aus `trees.json` unter `tiles`,
/// der nicht von oben schaut, einer mit seinem `path`. Ohne `tiles` nur
/// `oben`, so für geheime Ebenen.
fn saetze(tiles: Option<&Path>) -> Result<Vec<Satz>> {
    let kamera = Kamera::Nord45;
    let richtung = Richtung::parse("s", kamera).map_err(anyhow::Error::msg)?;
    let mut saetze = vec![Satz {
        name: OBEN.to_string(),
        kamera,
        richtung,
    }];
    let Some(tiles) = tiles else {
        return Ok(saetze);
    };
    let pfad = tiles.join(BAEUME);
    let text = match std::fs::read_to_string(&pfad) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("{} fehlt: nur der Satz {OBEN}", pfad.display());
            return Ok(saetze);
        }
        Err(e) => return Err(e).with_context(|| format!("{} lesen", pfad.display())),
    };
    let json: Value =
        serde_json::from_str(&text).with_context(|| format!("{} lesen", pfad.display()))?;
    for baum in json["trees"].as_array().into_iter().flatten() {
        let text = |feld: &str| baum[feld].as_str().unwrap_or_default();
        let name = text("path");
        let gelesen = Kamera::parse(text("camera")).and_then(|kamera| {
            Richtung::parse(text("direction"), kamera).map(|richtung| (kamera, richtung))
        });
        match gelesen {
            _ if !ist_baum(name) || name == OBEN => {
                eprintln!("{}: Baum `{name}` übergangen", pfad.display());
            }
            Ok((Kamera::Oben | Kamera::ObenNord, _)) => {}
            Ok((kamera, richtung)) => saetze.push(Satz {
                name: name.to_string(),
                kamera,
                richtung,
            }),
            Err(grund) => eprintln!("{}: Baum `{name}` übergangen: {grund}", pfad.display()),
        }
    }
    Ok(saetze)
}

/// Liest `id` und `designs` einer Datei; geht das nicht, die Kennung, wenn
/// sie sich lesen lässt, und den Grund.
fn lies_ebene(datei: &Path) -> std::result::Result<Ebene, (Option<Kennung>, anyhow::Error)> {
    let text = (|| -> Result<String> {
        let groesse = std::fs::metadata(datei)?.len();
        ensure!(
            groesse <= HOECHSTENS_DATEI,
            "{groesse} Bytes, höchstens {HOECHSTENS_DATEI}"
        );
        Ok(std::fs::read_to_string(datei)?)
    })()
    .map_err(|e| (None, e))?;
    Ebene::lies(&text).map_err(|grund| {
        let json = serde_json::from_str::<Value>(&text).ok();
        let id = json
            .as_ref()
            .and_then(kennung)
            .map(|(modname, name)| (modname.to_string(), name.to_string()));
        (id, grund)
    })
}

/// Zeichnet die Sprites einer Ebene in einem Satz, deren Stempel nicht
/// passt oder deren Datei fehlt, dazu `satz.json` und die Stempel. Erst die
/// Sprites, dann der Stempel: Bricht der Lauf dazwischen ab, zeichnet der
/// nächste neu. Liefert, ob sich etwas geändert hat.
fn zeichne_satz(
    assets: &mut Assets,
    satz: &Satz,
    leinwand: &Leinwand,
    satz_json: &[u8],
    ebene: &Ebene,
    ordner: &Path,
    abdruck: u64,
) -> Result<bool> {
    if ebene.designs.is_empty() {
        return Ok(false);
    }
    let alt: BTreeMap<String, String> = std::fs::read(ordner.join(STEMPEL))
        .ok()
        .and_then(|roh| serde_json::from_slice(&roh).ok())
        .unwrap_or_default();
    let mut neu = BTreeMap::new();
    let mut geaendert = false;
    for (name, entwurf) in &ebene.designs {
        let stempel = banner::stempel(
            satz.kamera,
            satz.richtung,
            &entwurf.base,
            &entwurf.layers,
            abdruck,
        );
        let stempel = format!("{stempel:016x}");
        let pfade = [
            ordner.join(format!("{name}.png")),
            ordner.join(KRONE).join(format!("{name}.png")),
        ];
        if alt.get(name) != Some(&stempel) || !pfade.iter().all(|p| p.is_file()) {
            for (krone, pfad) in [false, true].into_iter().zip(&pfade) {
                let bild = banner::zeichne(
                    assets,
                    satz.kamera,
                    satz.richtung,
                    &entwurf.base,
                    &entwurf.layers,
                    krone,
                )?;
                if !krone && !bild.unbekannt.is_empty() {
                    let liste: Vec<&str> = bild.unbekannt.iter().map(String::as_str).collect();
                    eprintln!(
                        "{}:{}, Entwurf {name}: unbekannt und weggelassen: {}",
                        ebene.modname,
                        ebene.name,
                        liste.join(", ")
                    );
                }
                schreibe(pfad, &png(&leinwand.setze(&bild)?)?)?;
            }
            geaendert = true;
        }
        neu.insert(name.clone(), stempel);
    }
    let satz_pfad = ordner.join(SATZ);
    if std::fs::read(&satz_pfad).ok().as_deref() != Some(satz_json) {
        schreibe(&satz_pfad, satz_json)?;
        geaendert = true;
    }
    if neu != alt {
        schreibe(&ordner.join(STEMPEL), &serde_json::to_vec_pretty(&neu)?)?;
    }
    Ok(geaendert)
}

/// Das Bild als PNG.
fn png(bild: &RgbaImage) -> Result<Vec<u8>> {
    let mut roh = Cursor::new(Vec::new());
    bild.write_to(&mut roh, image::ImageFormat::Png)?;
    Ok(roh.into_inner())
}

/// Schreibt eine Datei, ohne dass jemand eine halbe sieht, siehe
/// [`tausche`].
fn schreibe(pfad: &Path, daten: &[u8]) -> Result<()> {
    if let Some(ordner) = pfad.parent() {
        std::fs::create_dir_all(ordner).with_context(|| format!("{} anlegen", ordner.display()))?;
    }
    tausche(pfad, daten, None, false).with_context(|| format!("{} schreiben", pfad.display()))
}

/// Löscht unter `<out>/*/banner/`, was zu keiner übergebenen Ebene, keinem
/// Entwurf und keinem Satz mehr gehört, dazu halbe Dateien. Was nicht so
/// heisst, wie `--banners` schreibt, bleibt, und mit ihm sein Ordner; eine
/// Ebene in `kaputt` bleibt ganz. Liefert die Ebenen, deren Sprites gingen.
fn raeume_auf(
    out: &Path,
    ebenen: &BTreeMap<Kennung, Ebene>,
    kaputt: &BTreeSet<Kennung>,
    saetze: &BTreeSet<&str>,
) -> Result<BTreeSet<Kennung>> {
    let mut weg = BTreeSet::new();
    for (modname, banner_ordner) in unterordner(out)? {
        let banner_ordner = banner_ordner.join("banner");
        if !ist_teil(&modname) || !banner_ordner.is_dir() {
            continue;
        }
        for (name, ebene_ordner) in unterordner(&banner_ordner)? {
            let id = (modname.clone(), name);
            if kaputt.contains(&id) {
                continue;
            }
            let entwuerfe = ebenen.get(&id).map(|ebene| &ebene.designs);
            for (satz, satz_ordner) in unterordner(&ebene_ordner)? {
                let gilt = entwuerfe.filter(|e| !e.is_empty() && saetze.contains(satz.as_str()));
                let behalten = |datei: &str| {
                    datei
                        .strip_suffix(".png")
                        .is_some_and(|name| gilt.is_some_and(|e| e.contains_key(name)))
                };
                let mut sprites_weg = raeume_ordner(&satz_ordner.join(KRONE), &behalten, false)?;
                sprites_weg |= raeume_ordner(&satz_ordner, &behalten, gilt.is_none())?;
                if sprites_weg {
                    weg.insert(id.clone());
                }
                let _ = std::fs::remove_dir(&satz_ordner);
            }
            let _ = std::fs::remove_dir(&ebene_ordner);
        }
        let _ = std::fs::remove_dir(&banner_ordner);
    }
    Ok(weg)
}

/// Löscht in `ordner` jedes Sprite, das `behalten` nicht nennt, und halbe
/// Dateien; mit `ganz` auch `satz.json`, den Stempel und einen leeren
/// Ordner `krone/`. Liefert, ob ein Sprite ging.
fn raeume_ordner(ordner: &Path, behalten: &dyn Fn(&str) -> bool, ganz: bool) -> Result<bool> {
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return Ok(false);
    };
    let mut sprite_weg = false;
    for eintrag in eintraege {
        let eintrag = eintrag.with_context(|| format!("{} lesen", ordner.display()))?;
        let pfad = eintrag.path();
        let Some(datei) = eintrag.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !pfad.is_file() {
            if ganz && datei == KRONE {
                let _ = std::fs::remove_dir(&pfad);
            }
            continue;
        }
        let sprite = datei.strip_suffix(".png").is_some_and(ist_teil);
        let loeschen = (sprite && !behalten(&datei))
            || datei.ends_with(".tmp")
            || (ganz && (datei == SATZ || datei == STEMPEL));
        if loeschen {
            std::fs::remove_file(&pfad).with_context(|| format!("{} löschen", pfad.display()))?;
            sprite_weg |= sprite;
        }
    }
    Ok(sprite_weg)
}

/// Die Unterordner von `ordner` mit ihrem Namen; fehlt er, keine.
fn unterordner(ordner: &Path) -> Result<Vec<(String, PathBuf)>> {
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return Ok(Vec::new());
    };
    let mut liste = Vec::new();
    for eintrag in eintraege {
        let eintrag = eintrag.with_context(|| format!("{} lesen", ordner.display()))?;
        if eintrag.path().is_dir()
            && let Some(name) = eintrag.file_name().to_str()
        {
            liste.push((name.to_string(), eintrag.path()));
        }
    }
    liste.sort();
    Ok(liste)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Assets {
        let fixture = |name: &str| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name)
        };
        let mut assets = Assets::open(vec![fixture("assets-base")]).unwrap();
        assets.load_banner_patterns(&fixture("data-base")).unwrap();
        assets
    }

    fn ebene(dir: &Path, datei: &str, text: &str) -> PathBuf {
        let pfad = dir.join(datei);
        std::fs::write(&pfad, text).unwrap();
        pfad
    }

    fn lauf(dateien: &[PathBuf], out: &Path, tiles: Option<&Path>) -> Value {
        banners(&mut assets(), dateien, out, tiles, 7).unwrap()
    }

    fn namen(meldung: &Value, feld: &str) -> Vec<String> {
        meldung[feld]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    const STAEDTE: &str = r#"{"id": "beispiel:staedte", "designs": {
        "nordreich": {"base": "white", "layers": [{"pattern": "minecraft:stripe_top", "color": "red"}]},
        "white": {"base": "white"}}}"#;

    /// Ein Satz je Baum, der nicht von oben schaut, unter seinem `path`,
    /// auch einer mit Cinematic; immer `oben`; ohne `tiles` nur `oben`.
    #[test]
    fn saetze_aus_den_baeumen() {
        let tiles = tempfile::tempdir().unwrap();
        std::fs::write(
            tiles.path().join(BAEUME),
            r#"{"trees": [
                {"path": "2x1-se", "camera": "2:1", "direction": "se", "look": "map"},
                {"path": "2x1-se-cinematic", "camera": "2:1", "direction": "se", "look": "cinematic"},
                {"path": "north-45-w", "camera": "north-45", "direction": "w", "look": "map"},
                {"path": "top-north-s", "camera": "top-north", "direction": "s", "look": "map"},
                {"path": "top-north-s-flat", "camera": "top-north", "direction": "s", "look": "flat"},
                {"path": "oben", "camera": "2:1", "direction": "se", "look": "map"},
                {"path": "../x", "camera": "2:1", "direction": "se", "look": "map"}]}"#,
        )
        .unwrap();
        let liste = saetze(Some(tiles.path())).unwrap();
        let namen: Vec<&str> = liste.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(namen, ["oben", "2x1-se", "2x1-se-cinematic", "north-45-w"]);
        assert_eq!(liste[0].kamera, Kamera::Nord45);
        assert_eq!(liste[0].richtung.name(Kamera::Nord45), "s");
        assert_eq!(saetze(None).unwrap().len(), 1);
    }

    /// Ein Aufruf schreibt je Satz und Entwurf beide Sprites auf der
    /// Leinwand des Satzes, `satz.json` und den Stempel; ein zweiter mit
    /// denselben Entwürfen ändert nichts. Ein Entwurf weniger nimmt seine
    /// Sprites, ein Aufruf ohne die Ebene alles bis auf fremde Dateien.
    #[test]
    fn schreiben_stempel_und_aufraeumen() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("layers");
        let datei = ebene(dir.path(), "staedte.json", STAEDTE);
        let meldung = lauf(std::slice::from_ref(&datei), &out, None);
        assert_eq!(namen(&meldung, "changed"), ["beispiel:staedte"]);
        assert!(namen(&meldung, "failed").is_empty());
        let satz = out.join("beispiel/banner/staedte/oben");
        let leinwand = Leinwand::fuer(
            &mut assets(),
            Kamera::Nord45,
            Richtung::parse("s", Kamera::Nord45).unwrap(),
        )
        .unwrap();
        for pfad in [
            "nordreich.png",
            "white.png",
            "krone/nordreich.png",
            "krone/white.png",
        ] {
            let bild = image::open(satz.join(pfad)).unwrap();
            assert_eq!((bild.width(), bild.height()), leinwand.groesse, "{pfad}");
        }
        let satz_json: Value =
            serde_json::from_slice(&std::fs::read(satz.join(SATZ)).unwrap()).unwrap();
        assert_eq!(
            satz_json,
            json!({"foot": [leinwand.fuss.0, leinwand.fuss.1], "angle": 0.0})
        );
        assert!(satz.join(STEMPEL).is_file());

        // Derselbe Aufruf noch einmal: nichts geändert, nichts neu geschrieben.
        let zeit = std::fs::metadata(satz.join("white.png"))
            .unwrap()
            .modified()
            .unwrap();
        let meldung = lauf(std::slice::from_ref(&datei), &out, None);
        assert!(namen(&meldung, "changed").is_empty());
        assert_eq!(
            std::fs::metadata(satz.join("white.png"))
                .unwrap()
                .modified()
                .unwrap(),
            zeit
        );

        // Ein anderer Abdruck der Assets zeichnet neu.
        let meldung = banners(&mut assets(), std::slice::from_ref(&datei), &out, None, 8).unwrap();
        assert_eq!(namen(&meldung, "changed"), ["beispiel:staedte"]);

        // Ein Entwurf weniger: seine Sprites gehen, eine fremde Datei bleibt.
        std::fs::write(satz.join("notiz.txt"), "bleibt").unwrap();
        let datei = ebene(
            dir.path(),
            "staedte.json",
            r#"{"id": "beispiel:staedte", "designs": {"white": {"base": "white"}}}"#,
        );
        let meldung = banners(&mut assets(), std::slice::from_ref(&datei), &out, None, 8).unwrap();
        assert_eq!(namen(&meldung, "changed"), ["beispiel:staedte"]);
        assert!(!satz.join("nordreich.png").exists() && !satz.join("krone/nordreich.png").exists());
        assert!(satz.join("white.png").is_file());

        // Ohne die Ebene: alles weg ausser der fremden Datei und ihrem Ordner.
        let meldung = lauf(&[], &out, None);
        assert_eq!(namen(&meldung, "changed"), ["beispiel:staedte"]);
        let mut rest: Vec<String> = walk(&out);
        rest.sort();
        assert_eq!(rest, ["beispiel/banner/staedte/oben/notiz.txt"]);
    }

    /// Zwei Ebenen, eine kaputt: Die gute wird gezeichnet, die alten Sprites
    /// der kaputten bleiben Byte für Byte, und sie steht unter `failed`.
    #[test]
    fn kaputte_ebene_haelt_die_anderen_nicht_an() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("layers");
        let alt = out.join("anderer/banner/wege/oben");
        std::fs::create_dir_all(alt.join(KRONE)).unwrap();
        for (pfad, inhalt) in [
            ("alt.png", "png"),
            ("krone/alt.png", "krone"),
            (SATZ, "{}"),
            (STEMPEL, "{}"),
        ] {
            std::fs::write(alt.join(pfad), inhalt).unwrap();
        }
        let lage = r#"{"pattern": "minecraft:stripe_top", "color": "red"}"#;
        let kaputt = format!(
            r#"{{"id": "anderer:wege", "designs": {{"alt": {{"base": "white", "layers": [{}]}}}}}}"#,
            [lage; 17].join(",")
        );
        let dateien = [
            ebene(dir.path(), "staedte.json", STAEDTE),
            ebene(dir.path(), "wege.json", &kaputt),
        ];
        let meldung = lauf(&dateien, &out, None);
        assert_eq!(namen(&meldung, "changed"), ["beispiel:staedte"]);
        assert_eq!(namen(&meldung, "failed"), ["anderer:wege"]);
        assert!(
            out.join("beispiel/banner/staedte/oben/nordreich.png")
                .is_file()
        );
        for (pfad, inhalt) in [
            ("alt.png", "png"),
            ("krone/alt.png", "krone"),
            (SATZ, "{}"),
            (STEMPEL, "{}"),
        ] {
            assert_eq!(
                std::fs::read_to_string(alt.join(pfad)).unwrap(),
                inhalt,
                "{pfad}"
            );
        }
    }

    /// Eine Datei ohne lesbare Kennung steht mit ihrem Pfad unter `failed`,
    /// und der Aufruf räumt nichts auf.
    #[test]
    fn ohne_kennung_kein_aufraeumen() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("layers");
        let alt = out.join("anderer/banner/wege/oben");
        std::fs::create_dir_all(&alt).unwrap();
        std::fs::write(alt.join("alt.png"), "png").unwrap();
        let datei = ebene(dir.path(), "kaputt.json", "kein JSON");
        let meldung = lauf(std::slice::from_ref(&datei), &out, None);
        assert_eq!(namen(&meldung, "failed"), [datei.display().to_string()]);
        assert!(alt.join("alt.png").is_file());
    }

    /// Jede Datei unter `wurzel`, relativ mit `/`.
    fn walk(wurzel: &Path) -> Vec<String> {
        let mut liste = Vec::new();
        let mut offen = vec![wurzel.to_path_buf()];
        while let Some(ordner) = offen.pop() {
            for eintrag in std::fs::read_dir(&ordner).unwrap() {
                let pfad = eintrag.unwrap().path();
                if pfad.is_dir() {
                    offen.push(pfad);
                } else {
                    let rel = pfad.strip_prefix(wurzel).unwrap();
                    liste.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        liste
    }
}
