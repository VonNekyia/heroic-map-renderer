//! Prüft den CLI-Einstieg selbst, nicht nur die Bibliothek.
//!
//! Die Bibliothekstests setzen das Bildrechteck direkt. Der Weg dorthin
//! führt aber über `--center`, und der ist eine eigene Fehlerquelle.

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, SystemTime};

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{
    Projection, SpriteSet, TileId, encode_webp, pyramid, render_area, survey,
};
use terranova_render::world::World;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base")
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

/// Ruft die Binärdatei auf.
fn cli(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .args(args)
        .output()
        .expect("terranova-render starten")
}

/// Kachelexport über die Binärdatei, genau mit diesen Schaltern.
fn export(welt: &Path, out: &Path, extra: &[&str]) -> Output {
    let mut args: Vec<&OsStr> = vec![
        OsStr::new("--world"),
        welt.as_ref(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--tiles"),
        out.as_ref(),
    ];
    args.extend(extra.iter().map(OsStr::new));
    cli(&args)
}

/// Kachelexport über die Binärdatei. Ohne eigenes `--native-levels` mit
/// allen nativen Stufen, die der scale hergibt: bei 16 zwei, bei 12 keine.
/// So prüfen die Tests beide Wege, den nativen und das Verkleinern.
fn tiles(welt: &Path, out: &Path, extra: &[&str]) -> Output {
    if extra.contains(&"--native-levels") {
        return export(welt, out, extra);
    }
    let mut mit = vec!["--native-levels", "9"];
    mit.extend(extra);
    export(welt, out, &mit)
}

/// `assets()` als geliehener Pfad — die Binärdatei bekommt ihn mehrfach.
fn assets_ref() -> &'static OsStr {
    use std::sync::OnceLock;
    static PFAD: OnceLock<PathBuf> = OnceLock::new();
    PFAD.get_or_init(assets).as_os_str()
}

fn gelungen(out: &Output) -> &Output {
    assert!(
        out.status.success(),
        "Lauf fehlgeschlagen:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Alle geschriebenen Kacheln als `<z>/<x>/<y>.webp`, sortiert.
fn dateien(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stapel = vec![(dir.to_path_buf(), String::new())];
    while let Some((pfad, prefix)) = stapel.pop() {
        for eintrag in std::fs::read_dir(&pfad).into_iter().flatten().flatten() {
            let name = eintrag.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if eintrag.path().is_dir() {
                stapel.push((eintrag.path(), rel));
            } else if rel.ends_with(".webp") {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

/// Die feinste Zoomstufe, wie `map.json` sie nennt.
fn max_zoom(dir: &Path) -> u32 {
    let text = std::fs::read_to_string(dir.join("map.json")).expect("map.json lesen");
    let info: serde_json::Value = serde_json::from_str(&text).expect("map.json auswerten");
    info["maxZoom"].as_u64().expect("maxZoom") as u32
}

/// Alle Ausgabedateien mit Inhalt, `map.json` eingeschlossen.
fn schnappschuss(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    dateien(dir)
        .into_iter()
        .chain(std::iter::once("map.json".to_string()))
        .filter(|rel| dir.join(rel).is_file())
        .map(|rel| {
            let inhalt = std::fs::read(dir.join(&rel)).expect("Ausgabedatei lesen");
            (rel, inhalt)
        })
        .collect()
}

/// Eine Kopie des Baums in einem neuen Verzeichnis.
fn kopie(dir: &Path) -> TempDir {
    let ziel = tempdir();
    for (rel, inhalt) in schnappschuss(dir) {
        let pfad = ziel.path().join(rel);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    }
    ziel
}

/// Die Kacheln einer Zoomstufe.
fn kacheln(dir: &Path, z: u32) -> BTreeMap<TileId, PathBuf> {
    let mut out = BTreeMap::new();
    let vorsatz = format!("{z}/");
    for rel in dateien(dir) {
        let Some(rest) = rel.strip_prefix(&vorsatz) else {
            continue;
        };
        let (x, y) = rest.split_once('/').expect("<x>/<y>.webp");
        let tile = TileId {
            x: x.parse().expect("Kachel-x"),
            y: y.trim_end_matches(".webp").parse().expect("Kachel-y"),
        };
        out.insert(tile, dir.join(&rel));
    }
    out
}

fn bild(path: &Path) -> RgbaImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("{} lesen: {e}", path.display()))
        .into_rgba8()
}

/// Eine Treppenlandschaft über mehrere Chunks. Der entfernte Chunk
/// bekommt einen eigenen Block: fehlt dessen Sprite in der Tabelle,
/// verschwindet er lautlos statt einen Fehler zu werfen.
fn gelaende(x: i32, y: i32, z: i32) -> &'static str {
    let hoehe = 3 + x.rem_euclid(16) / 4 + z.rem_euclid(16) / 4;
    if y >= hoehe {
        "minecraft:air"
    } else if x >= 32 {
        "minecraft:stone"
    } else if y < 3 {
        "minecraft:einfarbig"
    } else {
        "minecraft:blauwuerfel"
    }
}

/// Baut eine kleine Szene um `anker` und rendert sie über die echte
/// Binärdatei. Zurück kommen die Pixel.
fn render(anker: i32) -> Vec<u8> {
    let dir = tempfile::tempdir().expect("Temporärverzeichnis");
    let chunk = anker >> 4;
    common::write_world(dir.path(), &[(chunk, chunk)], |x, y, z| {
        match (x - anker, y, z - anker) {
            (8, 4, 8) => "minecraft:einfarbig",
            (9, 4, 8) => "minecraft:blauwuerfel",
            (8, 4, 9) => "minecraft:stone",
            _ => "minecraft:air",
        }
    });

    let bild = dir.path().join("karte.png");
    let mitte = (anker + 8).to_string();
    let ausgabe = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .arg("--world")
        .arg(dir.path())
        .arg("--assets")
        .arg(assets())
        .arg("--render")
        .arg(&bild)
        .arg("--center")
        .arg(&mitte)
        .arg(&mitte)
        .args(["--size", "128", "--scale", "32"])
        .output()
        .expect("terranova-render starten");
    assert!(
        ausgabe.status.success(),
        "Renderlauf fehlgeschlagen:\n{}",
        String::from_utf8_lossy(&ausgabe.stderr)
    );

    image::open(&bild)
        .expect("gerendertes PNG lesen")
        .into_rgba8()
        .into_raw()
}

/// Dieselbe Szene weit draussen muss dasselbe Bild ergeben. `--center`
/// nimmt Weltkoordinaten entgegen; ab 2^24 kann f32 benachbarte Blöcke
/// nicht mehr unterscheiden, und die Bildmitte rutscht auf den Nachbarn.
///
/// Der Anker ist ungerade gewählt: jenseits von 2^24 liegen die
/// darstellbaren f32-Werte zwei auseinander, gerade Koordinaten kämen also
/// zufällig heil durch.
#[test]
fn center_verliert_weit_draussen_keine_praezision() {
    assert_eq!(render(0), render((1 << 24) + 1));
}

/// Eine Kachel aus einem begrenzten Export muss Byte für Byte der
/// gleichnamigen Kachel aus dem Vollexport gleichen.
///
/// Der Vorlauf bekommt einen Ausschnitt, ausgegeben werden aber immer ganze
/// Kacheln. Wer den Vorlauf nicht auf die Kachelgrenzen aufrundet, lässt
/// Chunks weg, die in derselben Kachel liegen — und deren Blöcke fehlen
/// dann im Bild, ohne dass ein Fehler auftaucht.
///
/// Der Aufbau ist mit Absicht so gewählt: Chunk (2, 2) liegt bei scale 16
/// rund 190 Pixel unter Chunk (0, 0) — weit ausserhalb des vier Pixel
/// breiten Ausschnitts, aber in derselben 256er Kachel. Und er besteht aus
/// einem Block, den der nahe Chunk nicht hat.
#[test]
fn ausschnitt_liefert_dieselben_kacheln_wie_der_vollexport() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);

    let ganz = tempdir();
    let teil = tempdir();
    gelungen(&tiles(welt.path(), ganz.path(), &["--scale", "16"]));
    gelungen(&tiles(
        welt.path(),
        teil.path(),
        &["--scale", "16", "--center", "4", "8", "--size", "4"],
    ));

    // Die Zoomnummern müssen übereinstimmen: sie hängen an der Welt, nicht
    // am Ausschnitt.
    let basis = max_zoom(ganz.path());
    assert_eq!(max_zoom(teil.path()), basis, "Ausschnitt nummeriert anders");

    let vorsatz = format!("{basis}/");
    let geschrieben: Vec<String> = dateien(teil.path())
        .into_iter()
        .filter(|rel| rel.starts_with(&vorsatz))
        .collect();
    assert!(
        !geschrieben.is_empty(),
        "der Ausschnitt schrieb keine Basis"
    );
    for rel in geschrieben {
        let aus_teil = std::fs::read(teil.path().join(&rel)).unwrap();
        let aus_ganz = std::fs::read(ganz.path().join(&rel))
            .unwrap_or_else(|_| panic!("{rel} fehlt im Vollexport"));
        assert_eq!(
            aus_teil, aus_ganz,
            "{rel} unterscheidet sich zwischen Ausschnitt und Vollexport"
        );
    }
}

/// Ein zweiter Lauf in dasselbe Verzeichnis darf keine Kachel stehen
/// lassen, die inzwischen leer ist.
#[test]
fn zweiter_lauf_raeumt_leer_gewordene_kacheln_weg() {
    let out = tempdir();

    let voll = tempdir();
    common::write_world(voll.path(), &[(0, 0)], |x, y, z| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    });
    gelungen(&tiles(voll.path(), out.path(), &["--scale", "16"]));
    assert!(
        !dateien(out.path()).is_empty(),
        "erster Lauf schrieb nichts"
    );

    // Derselbe Block, aber vollständig durchsichtig: die Kachel wird leer.
    let leer = tempdir();
    common::write_world(leer.path(), &[(0, 0)], |x, y, z| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:durchsichtig"
        } else {
            "minecraft:air"
        }
    });
    gelungen(&tiles(leer.path(), out.path(), &["--scale", "16"]));
    assert_eq!(
        dateien(out.path()),
        Vec::<String>::new(),
        "die alten Kacheln stehen noch da"
    );
}

/// Verschwindet ein Chunk aus der Welt, etwa weil ein Editor ihn
/// zurückgesetzt hat, sieht der Vorlauf ihn nicht mehr. Mit --prune müssen
/// seine alten Kacheln trotzdem weg, auf jeder Stufe: danach gleicht der
/// Baum einem frischen Export. Bei scale 16 mit nativen Stufen, bei 12
/// ohne; dort stapelt die Pyramide direkt auf der Basis, während die
/// veralteten Kacheln noch dastehen.
#[test]
fn verschwundener_chunk_verschwindet_auf_jeder_stufe() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (6, 6)], zwei_bloecke);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], zwei_bloecke);

    for scale in ["16", "12"] {
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
        let vorher = dateien(baum.path());
        let ausgabe = tiles(neu.path(), baum.path(), &["--scale", scale, "--prune"]);
        let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
        // Angesagt wird vor der Basis: bis zum Ende bleibt Zeit für Strg+C.
        let ansage = text.find("Aufräumen:").expect("keine Ansage");
        assert!(ansage < text.find("Kacheln:").unwrap(), "{text}");
        let voll = tempdir();
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));

        let soll = schnappschuss(voll.path());
        assert!(
            vorher.len() > soll.len(),
            "scale {scale}: der Chunk hatte keine eigenen Kacheln: {vorher:?}"
        );
        assert_eq!(schnappschuss(baum.path()), soll, "scale {scale}");
    }
}

fn zwei_bloecke(x: i32, y: i32, z: i32) -> &'static str {
    match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (104, 4, 104) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    }
}

/// Ohne --prune bleiben Kacheln stehen, die kein Chunk mehr berührt: einer
/// Teilkopie der Welt fehlt vieles, und der Lauf leerte sonst den Baum. Er
/// zählt sie, sagt, wie sie weggehen, und dass das nur mit der
/// vollständigen Welt geht: bei einer Teilkopie zerstörte der Schalter.
#[test]
fn ohne_prune_bleiben_kacheln_ohne_chunk_stehen() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (6, 6)], zwei_bloecke);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], zwei_bloecke);

    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let basis = kacheln(baum.path(), max_zoom(baum.path()));
    let ausgabe = tiles(neu.path(), baum.path(), &["--scale", "16"]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    assert!(
        text.contains(
            "2 von 4 Basiskacheln berührt kein Chunk dieser Welt mehr; sie bleiben stehen"
        ),
        "{text}"
    );
    assert!(
        text.contains("--prune entfernt sie, aber nur mit der vollständigen Welt"),
        "{text}"
    );
    assert_eq!(kacheln(baum.path(), max_zoom(baum.path())), basis);
}

/// Bricht ein Lauf mit --prune erst beim Entfernen ab, stehen Kacheln ohne
/// Chunk noch da: entfernt wird von der gröbsten Stufe bis zur Basis. Ihre
/// Eltern sind dann schon fort. Der nächste Lauf baut sie nach, auch einer
/// ohne --prune, und einer mit ihm räumt auf. Früher fand nach einem
/// solchen Abbruch kein Lauf mehr ihre Eltern.
///
/// Der Stein bei (200, 4, 8) liegt weit rechts vom ersten Block: seine
/// Eltern teilt er zwei Stufen lang mit niemandem. Er liegt auf der Grenze
/// zweier Basiskacheln. Den Abbruch erzwingt ein Verzeichnis an der Stelle
/// der ersten, die zweite steht danach ohne Eltern da.
#[test]
fn abgebrochenes_aufraeumen_heilt_im_naechsten_lauf() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let z = max_zoom(baum.path());
    let soll = kacheln(voll.path(), z);
    let stein: Vec<PathBuf> = kacheln(baum.path(), z)
        .into_iter()
        .filter(|(tile, _)| !soll.contains_key(tile))
        .map(|(_, pfad)| pfad)
        .collect();
    assert_eq!(stein.len(), 2, "{stein:?}");
    std::fs::remove_file(&stein[0]).unwrap();
    std::fs::create_dir(&stein[0]).unwrap();

    let ausgabe = tiles(neu.path(), baum.path(), &["--scale", "16", "--prune"]);
    assert!(
        !ausgabe.status.success(),
        "der Lauf hätte an {} scheitern müssen",
        stein[0].display()
    );
    std::fs::remove_dir(&stein[0]).unwrap();
    assert!(stein[1].is_file());
    assert_eq!(waisen(baum.path()), [format!("{z}/6/3")]);
    gelungen(&tiles(neu.path(), baum.path(), &["--scale", "16"]));
    assert_eq!(waisen(baum.path()), Vec::<String>::new());
    gelungen(&tiles(
        neu.path(),
        baum.path(),
        &["--scale", "16", "--prune"],
    ));
    assert_eq!(schnappschuss(baum.path()), schnappschuss(voll.path()));
}

/// Auch ein Ausschnitt baut fehlende Eltern nach, bei einer groben Kachel,
/// die er nur anschneidet. So steht der Baum nach einem Ausschnitt mit
/// --prune, der beim Aufräumen abbrach: die Elternkachel über dem alten
/// Block links ist fort, der Block noch da; der Test entfernt sie von Hand.
/// Der Ausschnitt reicht über den alten Block und den neuen rechts daneben,
/// die Kachel über dem alten beginnt links ausserhalb. Bei scale 12 ist
/// jede Stufe über der Basis verkleinert.
#[test]
fn ausschnitt_heilt_auch_angeschnittene_waisen() {
    let block = |x, y, z| match (x, y, z) {
        (85, 4, -60) => "minecraft:blauwuerfel",
        (115, 4, -85) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(5, -4), (7, -6)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(7, -6)], block);
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "12"]));
    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "12"]));
    let z = max_zoom(baum.path());
    let basis: Vec<TileId> = kacheln(baum.path(), z).into_keys().collect();
    assert_eq!(basis, [TileId { x: 3, y: 0 }, TileId { x: 4, y: 0 }]);
    std::fs::remove_file(baum.path().join(format!("{}/0/0.webp", z - 2))).unwrap();
    assert_eq!(waisen(baum.path()), [format!("{}/1/0", z - 1)]);

    let ausschnitt = ["--scale", "12", "--center", "107", "-64", "--size", "16"];
    gelungen(&tiles(neu.path(), baum.path(), &ausschnitt));
    assert_eq!(waisen(baum.path()), Vec::<String>::new());
    let mut mit = ausschnitt.to_vec();
    mit.push("--prune");
    gelungen(&tiles(neu.path(), baum.path(), &mit));
    assert_eq!(schnappschuss(baum.path()), schnappschuss(voll.path()));
}

/// Auch einer Waise auf einer nativen Stufe baut der nächste Lauf die
/// Elternkachel nach, aus der Welt. So steht der Baum, wenn ein Lauf mit
/// --prune beim Aufräumen zwischen zwei Stufen abbrach: entfernt wird von
/// der gröbsten an. Der Test entfernt die Kachel zwei Stufen über dem
/// Stein von Hand, den der Lauf danach nicht mehr in der Welt findet. Bei
/// scale 16 sind beide Stufen über der Basis nativ.
#[test]
fn waise_auf_nativer_stufe_bekommt_eltern() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let z = max_zoom(baum.path());
    std::fs::remove_file(baum.path().join(format!("{}/1/0.webp", z - 2))).unwrap();
    assert_eq!(
        waisen(baum.path()),
        [format!("{}/2/1", z - 1), format!("{}/3/1", z - 1)]
    );
    gelungen(&tiles(neu.path(), baum.path(), &["--scale", "16"]));
    assert_eq!(waisen(baum.path()), Vec::<String>::new());
}

/// Auch über einer Fläche ohne Chunk baut ein Lauf ohne --prune fehlende
/// Eltern nach. So steht der Baum, wenn ein Lauf mit --prune dort beim
/// Aufräumen abbrach; der Test entfernt die Stufe über dem Stein von Hand.
/// Früher brach der Lauf vorher mit „keine Kachel enthält etwas“ ab. Mit
/// nativen Stufen rundet der Ausschnitt auf ihr Raster auf und heilt beide
/// Waisen, ohne sie nur die in seiner eigenen Kachel.
#[test]
fn leerer_ausschnitt_heilt_waisen() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    for native in ["9", "0"] {
        let schalter = ["--scale", "16", "--native-levels", native];
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &schalter));
        let z = max_zoom(baum.path());
        for x in [2, 3] {
            std::fs::remove_file(baum.path().join(format!("{}/{x}/1.webp", z - 1))).unwrap();
        }
        assert_eq!(
            waisen(baum.path()),
            [format!("{z}/5/3"), format!("{z}/6/3")]
        );

        let ausschnitt = [&schalter[..], &["--center", "200", "8", "--size", "1"]].concat();
        let ausgabe = tiles(neu.path(), baum.path(), &ausschnitt);
        let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
        assert!(text.contains("sie bleiben stehen"), "{text}");
        let bleiben = match native {
            "0" => vec![format!("{z}/5/3")],
            _ => Vec::new(),
        };
        assert_eq!(waisen(baum.path()), bleiben, "--native-levels {native}");
    }
}

/// Ein Ausschnitt heilt nur Waisen, die er berührt; eine direkt daneben
/// bleibt, wie sie ist. Er liegt links vom Bildursprung, bei Pixel -1024
/// bis 0. Dort rundet `div_euclid` anders als eine Division, die zur Null
/// hin kürzt, und die nähme die Spalte rechts daneben mit. In ihr steht der
/// zweite Block, seiner Basiskachel fehlt die Elternkachel. Nähme der
/// Ausschnitt sie mit, renderte er die Elternkachel mit einer
/// Sprite-Tabelle ohne diesen Block, und sie zeigte nichts. Bei scale 16
/// mit nativen Stufen.
#[test]
fn ausschnitt_laesst_waisen_daneben_stehen() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 4), (2, 1)], |x, y, z| match (x, y, z) {
        (0, 4, 64) => "minecraft:einfarbig",
        (40, 4, 24) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    });
    let baum = tempdir();
    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "16"]));
    let z = max_zoom(baum.path());
    std::fs::remove_file(baum.path().join(format!("{}/0/0.webp", z - 1))).unwrap();
    assert_eq!(waisen(baum.path()), [format!("{z}/0/0")]);
    let vorher = schnappschuss(baum.path());

    let ausschnitt = ["--scale", "16", "--center", "0", "64", "--size", "16"];
    let ausgabe = tiles(welt.path(), baum.path(), &ausschnitt);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(!text.contains("berührt kein Chunk"), "{text}");
    assert_eq!(schnappschuss(baum.path()), vorher);
}

/// Ohne --prune bleiben Kacheln ohne Chunk stehen, und mit ihnen ihre
/// Eltern. Rendert eine native Elternkachel leer, weil der Rest der Welt
/// dort nichts mehr hat, bleibt sie durchsichtig stehen. Früher verschwand
/// sie, und die Kachel darunter stand ohne Eltern. Hier ist der Stein aus
/// Chunk (1, -1) fort, bei scale 16 in Basiskachel (1, -1), bei 32 in
/// (2, -1). Seine Elternkachel teilt er mit Vorlaufkacheln des ersten
/// Chunks, die leer rendern: bei 16 auf der ersten nativen Stufe, bei 32
/// auf der zweiten. Ohne native Stufen verkleinert der Lauf auch den Stein.
#[test]
fn ohne_prune_bleibt_keine_kachel_ohne_eltern() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (24, 12, -10) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0)], block);
    common::write_world(alt.path(), &[(1, -1)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    for (scale, stein) in [
        ("16", TileId { x: 1, y: -1 }),
        ("32", TileId { x: 2, y: -1 }),
    ] {
        for native in ["9", "0"] {
            let schalter = ["--scale", scale, "--native-levels", native];
            let fall = format!("scale {scale}, --native-levels {native}");
            let baum = tempdir();
            gelungen(&tiles(alt.path(), baum.path(), &schalter));
            let z = max_zoom(baum.path());
            assert!(kacheln(baum.path(), z).contains_key(&stein), "{fall}");
            gelungen(&tiles(neu.path(), baum.path(), &schalter));
            assert!(kacheln(baum.path(), z).contains_key(&stein), "{fall}");
            assert_eq!(waisen(baum.path()), Vec::<String>::new(), "{fall}");
        }
    }
}

/// Eine Kachel, die leer geworden ist, verschwindet erst am Ende des Laufs,
/// zeigt aber schon ab dem Rendern nichts mehr. Bricht der Lauf danach ab,
/// steht sie noch da, durchsichtig: ein späterer Ausschnitt nähme sonst
/// ihren alten Inhalt in die Elternkachel. Der zweite Block bei (0, 4, 0)
/// reicht in die Kacheln (-1, -1) und (0, -1) der Basis und der Stufe
/// darüber, der erste nicht, auch wenn der Vorlauf sie nennt. Den Abbruch
/// erzwingt ein Verzeichnis an der Stelle einer Kachel des ersten Blocks
/// zwei Stufen über der Basis; bis dahin sind die beiden fertig. Bei
/// scale 16 sind sie nativ, bei 12 verkleinert.
#[test]
fn leer_gewordene_kachel_zeigt_nach_abbruch_nichts() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (0, 4, 0) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    });
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    });
    let leer_geworden = [TileId { x: -1, y: -1 }, TileId { x: 0, y: -1 }];
    for scale in ["16", "12"] {
        let voll = tempdir();
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
        let z = max_zoom(baum.path());
        let stufen = [z, z - 1];
        for (stufe, tile) in stufen.iter().flat_map(|&s| leer_geworden.map(|t| (s, t))) {
            let alt = bild(&kacheln(baum.path(), stufe)[&tile]);
            assert!(
                alt.pixels().any(|p| p.0[3] > 0),
                "scale {scale}: {stufe} {tile:?}"
            );
            assert!(
                !kacheln(voll.path(), stufe).contains_key(&tile),
                "scale {scale}"
            );
        }
        let vorher: Vec<BTreeMap<TileId, PathBuf>> =
            stufen.iter().map(|&s| kacheln(baum.path(), s)).collect();
        let sperre = baum.path().join(format!("{}/0/0.webp", z - 2));
        std::fs::remove_file(&sperre).unwrap();
        std::fs::create_dir(&sperre).unwrap();
        let ausgabe = tiles(neu.path(), baum.path(), &["--scale", scale]);
        assert!(!ausgabe.status.success(), "scale {scale}: kein Abbruch");
        for (stufe, bestand) in stufen.iter().zip(&vorher) {
            for tile in &leer_geworden {
                assert!(
                    bild(&bestand[tile]).pixels().all(|p| p.0[3] == 0),
                    "scale {scale}: {stufe} {tile:?} zeigt noch den alten Inhalt"
                );
            }
        }
        std::fs::remove_dir(&sperre).unwrap();
        gelungen(&tiles(neu.path(), baum.path(), &["--scale", scale]));
        assert_eq!(
            schnappschuss(baum.path()),
            schnappschuss(voll.path()),
            "scale {scale}"
        );
    }
}

/// Die Ansage mit --prune steht vor der ersten Kachel: bricht der Lauf
/// schon beim Schreiben der Basis ab, ist sie gesagt. Dazu liegt ein
/// Verzeichnis an der Stelle einer Basiskachel, die der Lauf schreibt.
#[test]
fn ansage_kommt_vor_der_ersten_kachel() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (6, 6)], zwei_bloecke);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], zwei_bloecke);
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));
    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let z = max_zoom(baum.path());
    let soll = kacheln(voll.path(), z);
    let (_, pfad) = kacheln(baum.path(), z)
        .into_iter()
        .find(|(tile, _)| soll.contains_key(tile))
        .expect("der erste Block hat eine Basiskachel");
    std::fs::remove_file(&pfad).unwrap();
    std::fs::create_dir(&pfad).unwrap();
    let ausgabe = tiles(neu.path(), baum.path(), &["--scale", "16", "--prune"]);
    assert!(!ausgabe.status.success(), "kein Abbruch");
    let text = String::from_utf8_lossy(&ausgabe.stdout);
    assert!(
        text.contains("sie verschwinden am Ende des Laufs"),
        "{text}"
    );
    assert!(!text.contains("Kacheln:"), "{text}");
}

/// Kacheln, deren Elternkachel fehlt, als `<z>/<x>/<y>`.
fn waisen(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for z in 1..=max_zoom(dir) {
        let oben = kacheln(dir, z - 1);
        for tile in kacheln(dir, z).keys() {
            if !oben.contains_key(&tile.parent()) {
                out.push(format!("{z}/{}/{}", tile.x, tile.y));
            }
        }
    }
    out
}

/// Ein Lauf mit --prune läuft bis zum Ende der Pyramide wie einer ohne den
/// Schalter und räumt erst dann auf. Bricht er in ihr ab, steht jede Datei
/// noch da, der Baum ist derselbe wie nach dem Abbruch eines Laufs ohne
/// den Schalter, und ein Lauf ohne ihn ergibt danach denselben Baum wie
/// ohne den Abbruch davor, bis aufs Byte. Einer mit ihm heilt den Baum.
/// Früher wurden die Stufen über den Kacheln ohne Chunk vorher schon
/// durchsichtig, und kein Lauf ohne --prune stellte sie wieder her; noch
/// früher verschwanden sie sofort. Bei scale 16 mit nativen Stufen, bei 12
/// ohne. Der dritte Block liegt neben dem ersten: bei scale 12 teilen sich
/// ihre Basiskacheln eine Elternkachel, und die setzte die Pyramide früher
/// schon ohne ihn zusammen.
///
/// Den Abbruch erzwingt ein Verzeichnis an der Stelle einer Kachel der
/// Stufe 0, die die Pyramide neu schreibt: dort liegt der erste Block.
#[test]
fn abbruch_in_der_pyramide_entfernt_nichts() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) | (64, 4, 0) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0), (4, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);

    for scale in ["16", "12"] {
        let voll = tempdir();
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
        assert!(max_zoom(baum.path()) > 2, "scale {scale}: zu wenig Stufen");
        let vorher = dateien(baum.path());
        let ohne = kopie(baum.path());
        gelungen(&tiles(neu.path(), ohne.path(), &["--scale", scale]));

        let (_, pfad) = kacheln(baum.path(), 0)
            .into_iter()
            .find(|(tile, _)| kacheln(voll.path(), 0).contains_key(tile))
            .expect("der erste Block hat eine Kachel auf Stufe 0");
        std::fs::remove_file(&pfad).unwrap();
        std::fs::create_dir(&pfad).unwrap();
        let ohne_schalter = kopie(baum.path());
        std::fs::create_dir_all(
            ohne_schalter
                .path()
                .join(pfad.strip_prefix(baum.path()).unwrap()),
        )
        .unwrap();
        let ausgabe = tiles(neu.path(), ohne_schalter.path(), &["--scale", scale]);
        assert!(!ausgabe.status.success(), "scale {scale}: kein Abbruch");
        let ausgabe = tiles(neu.path(), baum.path(), &["--scale", scale, "--prune"]);
        assert!(!ausgabe.status.success(), "scale {scale}: kein Abbruch");
        let fehlt: Vec<&String> = vorher
            .iter()
            .filter(|rel| !baum.path().join(rel).is_file() && baum.path().join(rel) != pfad)
            .collect();
        assert!(fehlt.is_empty(), "scale {scale}: entfernt {fehlt:?}");
        // Auf Stufe 0 bricht der Lauf ab; was dort daneben schon
        // geschrieben ist, hängt an der Reihenfolge der Threads.
        let bis_zum_abbruch = |dir: &Path| {
            let mut stand = schnappschuss(dir);
            stand.retain(|rel, _| !rel.starts_with("0/"));
            stand
        };
        assert_eq!(
            bis_zum_abbruch(baum.path()),
            bis_zum_abbruch(ohne_schalter.path()),
            "scale {scale}: mit --prune anders als ohne"
        );

        std::fs::remove_dir(&pfad).unwrap();
        gelungen(&tiles(neu.path(), baum.path(), &["--scale", scale]));
        assert_eq!(waisen(baum.path()), Vec::<String>::new(), "scale {scale}");
        assert_eq!(
            schnappschuss(baum.path()),
            schnappschuss(ohne.path()),
            "scale {scale}: der Abbruch hat etwas verändert"
        );
        gelungen(&tiles(
            neu.path(),
            baum.path(),
            &["--scale", scale, "--prune"],
        ));
        assert_eq!(
            schnappschuss(baum.path()),
            schnappschuss(voll.path()),
            "scale {scale}"
        );
    }
}

/// Ein Ausschnitt mit --prune über einer ganz zurückgesetzten Fläche: der
/// Vorlauf findet dort nichts mehr, aufzuräumen gibt es trotzdem. Früher
/// brach der Lauf vorher mit „keine Kachel enthält etwas“ ab, und die
/// alten Kacheln blieben stehen.
#[test]
fn prune_raeumt_auch_ueber_leerer_flaeche_auf() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let ausschnitt = ["--scale", "16", "--center", "200", "8", "--size", "1"];
    let ohne = tiles(neu.path(), baum.path(), &ausschnitt);
    let text = String::from_utf8_lossy(&ohne.stderr);
    assert!(!ohne.status.success());
    assert!(text.contains("--prune entfernt sie"), "{text}");

    let mut mit = ausschnitt.to_vec();
    mit.push("--prune");
    gelungen(&tiles(neu.path(), baum.path(), &mit));
    assert_eq!(schnappschuss(baum.path()), schnappschuss(voll.path()));
    // Noch einmal: dort ist schon aufgeräumt, das ist kein falscher
    // Ausschnitt.
    gelungen(&tiles(neu.path(), baum.path(), &mit));
    assert_eq!(schnappschuss(baum.path()), schnappschuss(voll.path()));
}

/// Nach der Pyramide setzt ein Lauf mit --prune die Stufen über den
/// Kacheln ohne Chunk ohne sie neu zusammen, erst dann entfernt er. Bricht
/// er dazwischen ab, ist nichts entfernt, die Basis unverändert, und keine
/// Kachel ist durchsichtig geworden. Hier ergibt ein Lauf über die ganze
/// Welt ohne den Schalter danach denselben Baum wie ohne den Abbruch: unter
/// der Kachel auf Stufe 0 liegt noch ein Chunk, und die Pyramide setzt sie
/// neu zusammen. Einer mit dem Schalter räumt zu Ende. Der Ausschnitt liegt
/// über dem verschwundenen Chunk, sein Vorlauf findet nichts; die Kachel auf
/// Stufe 0 über beiden Blöcken schreibt dann nur `ohne_veraltete`, an ihrer
/// Stelle liegt ein Verzeichnis. Dass der Lauf erst nach der Pyramide
/// abbricht, zeigt ihre Zeile in der Ausgabe. Bei scale 16 mit nativen
/// Stufen, bei 12 ohne; dort wird die Kachel über dem verschwundenen Block
/// leer.
#[test]
fn abbruch_in_ohne_veraltete_entfernt_nichts() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (200, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (12, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    let sichtbar = |inhalt: &[u8]| {
        image::load_from_memory(inhalt)
            .unwrap()
            .into_rgba8()
            .pixels()
            .any(|p| p.0[3] > 0)
    };

    for scale in ["16", "12"] {
        let voll = tempdir();
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
        let vorher = schnappschuss(baum.path());
        let ohne = kopie(baum.path());
        gelungen(&tiles(neu.path(), ohne.path(), &["--scale", scale]));

        let pfad = kacheln(baum.path(), 0)
            .into_iter()
            .find(|(tile, pfad)| {
                kacheln(voll.path(), 0).get(tile).is_some_and(|soll| {
                    std::fs::read(soll).unwrap() != std::fs::read(pfad).unwrap()
                })
            })
            .map(|(_, pfad)| pfad)
            .expect("eine Kachel auf Stufe 0 über beiden Blöcken");
        std::fs::remove_file(&pfad).unwrap();
        std::fs::create_dir(&pfad).unwrap();
        let ausschnitt = [
            "--scale", scale, "--center", "200", "8", "--size", "1", "--prune",
        ];
        let ausgabe = tiles(neu.path(), baum.path(), &ausschnitt);
        assert!(!ausgabe.status.success(), "scale {scale}: kein Abbruch");
        assert!(
            String::from_utf8_lossy(&ausgabe.stdout).contains("Pyramide:"),
            "scale {scale}: Abbruch vor dem Ende der Pyramide"
        );

        let nachher = schnappschuss(baum.path());
        let basis = format!("{}/", max_zoom(baum.path()));
        for (rel, inhalt) in &vorher {
            if baum.path().join(rel) == pfad || !rel.ends_with(".webp") {
                continue;
            }
            let jetzt = nachher
                .get(rel)
                .unwrap_or_else(|| panic!("scale {scale}: {rel} entfernt"));
            if rel.starts_with(&basis) {
                assert_eq!(jetzt, inhalt, "scale {scale}: {rel} an der Basis verändert");
            }
            if sichtbar(inhalt) {
                assert!(sichtbar(jetzt), "scale {scale}: {rel} durchsichtig");
            }
        }

        std::fs::remove_dir(&pfad).unwrap();
        let ganz = kopie(baum.path());
        gelungen(&tiles(neu.path(), ganz.path(), &["--scale", scale]));
        assert_eq!(
            schnappschuss(ganz.path()),
            schnappschuss(ohne.path()),
            "scale {scale}: die ganze Welt ohne --prune"
        );
        gelungen(&tiles(neu.path(), baum.path(), &ausschnitt));
        assert_eq!(
            schnappschuss(baum.path()),
            schnappschuss(voll.path()),
            "scale {scale}: --prune räumt zu Ende"
        );
    }
}

/// Ohne --tiles gibt es nichts aufzuräumen und keine Stufe nativ zu
/// rendern; still übergangen hiesse ein Schalter etwas, das er nicht tut.
#[test]
fn prune_braucht_tiles() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], zwei_bloecke);
    for schalter in [&["--prune"][..], &["--native-levels", "1"]] {
        let mut args = vec![
            OsStr::new("--world"),
            welt.path().as_os_str(),
            OsStr::new("--scan"),
        ];
        args.extend(schalter.iter().map(OsStr::new));
        let ausgabe = cli(&args);
        assert!(!ausgabe.status.success(), "{schalter:?}");
        let text = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(text.contains("--tiles"), "{schalter:?}: {text}");
    }
}

/// Ein Ausschnitt darf nicht an einem Block scheitern, der weit ausserhalb
/// liegt und gar nicht gezeichnet wird.
#[test]
fn ausschnitt_braucht_keine_assets_fuer_ferne_bloecke() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (31, 31)], |x, y, z| {
        match (x, y, z) {
            (8, 4, 8) => "minecraft:einfarbig",
            (500, 4, 500) => "minecraft:gibt_es_nicht",
            _ => "minecraft:air",
        }
    });

    let out = tempdir();
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "16", "--center", "8", "8", "--size", "4"],
    ));

    // Gegenprobe: der Vollexport braucht das fehlende Asset sehr wohl, und
    // muss das auch sagen.
    let alles = tempdir();
    let ausgabe = tiles(welt.path(), alles.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "der Vollexport hätte am fehlenden Asset scheitern müssen"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("gibt_es_nicht"), "Meldung: {meldung}");
}

/// Jede gröbere Zoomstufe ist entweder nativ aus der Welt gerendert —
/// so viele Stufen, wie `--native-levels` verlangt, und nur solange ein
/// Block auf ganzen Pixeln liegt, also bis scale 4 — oder genau die
/// Verkleinerung ihrer vier Kinder. Und keine Kachel darf fehlen.
#[test]
fn pyramide_passt_auf_jeder_stufe_zu_ihren_kindern() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    // scale 8 mit einer nativen Stufe (4), dann Verkleinerungen — beide Wege.
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "8", "--native-levels", "1"],
    ));

    let basis = max_zoom(out.path());
    assert!(basis > 1, "kein Stapel zu prüfen");
    assert!(!kacheln(out.path(), basis).is_empty());

    let world = World::open(welt.path()).unwrap();
    let states = survey(&world, Projection::new(8), (0, 15), None)
        .unwrap()
        .states;
    let mut nativ = 0;
    let mut verkleinert = 0;

    for z in (0..basis).rev() {
        let eltern = kacheln(out.path(), z);
        let kinder = kacheln(out.path(), z + 1);
        assert!(!eltern.is_empty(), "Zoom {z} ist leer");

        // Nativ nur, solange ein Block auf ganzen Pixeln liegt: scale 4 ja,
        // scale 2 nicht mehr.
        let scale = 8 >> (basis - z);
        let sprites = (scale >= 4).then(|| {
            let mut assets = Assets::open(vec![assets()]).unwrap();
            SpriteSet::build_in(&mut assets, &states, Projection::new(scale)).unwrap()
        });

        for (parent, pfad) in &eltern {
            if let Some(sprites) = &sprites {
                let soll = render_area(&world, sprites, parent.rect(), (0, 15)).unwrap();
                assert_eq!(
                    bild(pfad).as_raw(),
                    soll.as_raw(),
                    "Zoom {z}, {parent:?} ist nicht nativ bei scale {scale} gerendert"
                );
                nativ += 1;
                continue;
            }
            let teile: Vec<(TileId, RgbaImage)> = parent
                .children()
                .into_iter()
                .filter(|kind| kinder.contains_key(kind))
                .map(|kind| (kind, bild(&kinder[&kind])))
                .collect();
            assert!(!teile.is_empty(), "Zoom {z}, {parent:?} ohne Kinder");
            assert_eq!(
                bild(pfad).as_raw(),
                pyramid::merge(*parent, &teile).as_raw(),
                "Zoom {z}, {parent:?} ist nicht die Verkleinerung seiner Kinder"
            );
            verkleinert += 1;
        }

        // Gegenrichtung: kein Kind ohne Elternkachel.
        for kind in kinder.keys() {
            assert!(
                eltern.contains_key(&kind.parent()),
                "{kind:?} auf Zoom {} hat keine Elternkachel",
                z + 1
            );
        }
    }
    assert!(
        nativ > 0 && verkleinert > 0,
        "{nativ} nativ, {verkleinert} verkleinert"
    );
}

/// `--pyramid` über dem Baum in diesem Verzeichnis, ohne Welt und Assets.
fn pyramide(dir: &Path) -> Output {
    cli(&[OsStr::new("--pyramid"), dir.as_ref()])
}

/// Setzt jede Kachel des Baums auf dieselbe Zeit eine Stunde zurück, als
/// wäre er lange vor dem nächsten `--pyramid` entstanden. Eine Kachel aus
/// den zwei Sekunden vor einem Aufruf baut der nächste noch einmal ein.
fn altern(dir: &Path) {
    let damals = SystemTime::now() - Duration::from_secs(3600);
    for z in 0..=max_zoom(dir) {
        for pfad in kacheln(dir, z).values() {
            setze_zeit(pfad, damals);
        }
    }
}

/// Was ein Stromausfall aus einer Kachel machen kann: in voller Länge, mit
/// gutem Kopf und Nullen in der zweiten Hälfte.
fn zerreisse(pfad: &Path) {
    let mut bytes = std::fs::read(pfad).unwrap();
    let haelfte = bytes.len() / 2;
    bytes[haelfte..].fill(0);
    std::fs::write(pfad, bytes).unwrap();
}

/// Wann die Datei zuletzt geschrieben wurde.
fn zeit_von(pfad: &Path) -> SystemTime {
    std::fs::metadata(pfad).unwrap().modified().unwrap()
}

fn setze_zeit(pfad: &Path, zeit: SystemTime) {
    let datei = std::fs::File::options().write(true).open(pfad).unwrap();
    datei.set_modified(zeit).unwrap();
}

fn kachel_pfad(dir: &Path, z: u32, tile: TileId) -> PathBuf {
    dir.join(format!("{z}/{}/{}.webp", tile.x, tile.y))
}

/// Schreibt eine Kachel mit diesem Bild, jetzt.
fn setze(dir: &Path, z: u32, tile: TileId, bild: &RgbaImage) {
    let pfad = kachel_pfad(dir, z, tile);
    std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
    std::fs::write(pfad, encode_webp(bild).unwrap()).unwrap();
}

/// Was `--pyramid` aus der Basis dieses Baums und seiner `map.json` von
/// Grund auf baut.
fn von_grund_auf(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let basis = format!("{}/", max_zoom(dir));
    let frisch = tempdir();
    for (rel, inhalt) in schnappschuss(dir) {
        if rel == "map.json" || rel.starts_with(&basis) {
            let pfad = frisch.path().join(rel);
            std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
            std::fs::write(pfad, inhalt).unwrap();
        }
    }
    gelungen(&pyramide(frisch.path()));
    schnappschuss(frisch.path())
}

/// `--pyramid` baut aus den Basiskacheln auf der Platte dieselben
/// Zoomstufen und dieselbe `map.json` wie ein Export ohne native Stufen,
/// ohne Welt und ohne Assets. Beim zweiten Mal baut es nichts mehr.
/// `map.json` bleibt liegen: Basisstufe, scale und das Salz der Kennung
/// stehen nur dort.
#[test]
fn pyramide_laesst_sich_aus_den_kacheln_nachbauen() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "8", "--native-levels", "0"],
    ));
    let soll = schnappschuss(out.path());
    let basis = max_zoom(out.path());
    assert!(basis > 0);

    for z in 0..basis {
        std::fs::remove_dir_all(out.path().join(z.to_string())).unwrap();
    }
    altern(out.path());
    gelungen(&pyramide(out.path()));
    assert_eq!(schnappschuss(out.path()), soll);

    let ausgabe = pyramide(out.path());
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    assert!(
        meldung.contains("Pyramide:   0 Kacheln neu, 0 entfernt"),
        "Meldung: {meldung}"
    );
    assert_eq!(schnappschuss(out.path()), soll);
}

/// `--pyramid` holt nach, was sich unter einer Kachel geändert hat, auf
/// jeder Stufe: eine neu geschriebene Basiskachel; eine Stufe, die ein
/// abgebrochener Aufruf schon neu geschrieben hat, ihre Eltern aber nicht
/// mehr; Kinder, die alle verschwunden sind. Das Ergebnis ist jedes Mal
/// dasselbe wie von Grund auf. Was der Aufruf schreibt, `map.json`
/// eingeschlossen, trägt eine Zeit vor seinem Beginn: ein Kind, das ein
/// Render währenddessen fertigstellt, ist danach jünger als seine
/// Elternkachel. Über einer unlesbaren Kachel versucht es jeder Aufruf
/// wieder.
#[test]
fn pyramide_holt_jede_aenderung_nach() {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (0..4)
        .flat_map(|x| (0..4).map(move |z| (x * 3, z * 3)))
        .collect();
    common::write_world(welt.path(), &chunks, gelaende);
    let out = tempdir();
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "8", "--native-levels", "0"],
    ));
    let basis = max_zoom(out.path());
    assert!(basis > 2, "zu wenig Stufen");
    altern(out.path());
    // Die Kachel über der Basis, die gleich ihre Kinder verliert, hat ein
    // Geschwister: dann bleibt ihre Elternkachel stehen und muss neu.
    let mitte = kacheln(out.path(), basis - 1);
    let oben = *mitte
        .keys()
        .find(|p| mitte.keys().filter(|q| q.parent() == p.parent()).count() > 1)
        .expect("keine Geschwister über der Basis");
    let unten = kacheln(out.path(), basis);
    let (&eine, _) = unten.iter().find(|(k, _)| k.parent() == oben).unwrap();
    let (&andere, _) = unten.iter().find(|(k, _)| k.parent() != oben).unwrap();
    let vorlage = bild(&unten[&andere]);

    let pruefe = |fall: &str| {
        let vorher = SystemTime::now();
        let ausgabe = pyramide(out.path());
        let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
        assert_eq!(
            schnappschuss(out.path()),
            von_grund_auf(out.path()),
            "{fall}: {meldung}"
        );
        for z in 0..basis {
            for (tile, pfad) in kacheln(out.path(), z) {
                assert!(
                    zeit_von(&pfad) < vorher,
                    "{fall}: Zoom {z}, {tile:?} trägt die Uhrzeit"
                );
            }
        }
        assert!(
            zeit_von(&out.path().join("map.json")) < vorher,
            "{fall}: map.json trägt die Uhrzeit"
        );
        meldung
    };

    // Eine Basiskachel bekommt den Inhalt einer anderen: neu sind genau
    // ihre Vorfahren, einer je Stufe.
    setze(out.path(), basis, eine, &vorlage);
    let meldung = pruefe("neue Basiskachel");
    assert!(
        meldung.contains(&format!("Pyramide:   {basis} Kacheln neu, 0 entfernt")),
        "{meldung}"
    );

    // Ein Aufruf brach nach der ersten Stufe ab: die zeigt schon die
    // geänderte Basiskachel, die Stufen darüber noch die alte.
    let mut blass = vorlage.clone();
    for pixel in blass.pixels_mut() {
        pixel.0[3] /= 2;
    }
    setze(out.path(), basis, eine, &blass);
    let kinder: Vec<(TileId, RgbaImage)> = kacheln(out.path(), basis)
        .into_iter()
        .filter(|(kind, _)| kind.parent() == oben)
        .map(|(kind, pfad)| (kind, bild(&pfad)))
        .collect();
    setze(out.path(), basis - 1, oben, &pyramid::merge(oben, &kinder));
    pruefe("abgebrochener Aufruf");

    // Alle Kinder einer Kachel verschwinden, und mit ihnen die Kachel.
    for (kind, pfad) in kacheln(out.path(), basis) {
        if kind.parent() == oben {
            std::fs::remove_file(pfad).unwrap();
        }
    }
    let meldung = pruefe("verschwundene Kinder");
    assert!(
        !kacheln(out.path(), basis - 1).contains_key(&oben),
        "{meldung}"
    );

    // Eine Kachel ist abgeschnitten, etwa von einem Absturz beim Schreiben:
    // der Aufruf lässt sie aus und sagt es, statt abzubrechen. Sie ist eine
    // Minute jünger als ihre Elternkachel, lange vor dem Aufruf.
    let kaputt = &unten[&andere];
    let geschrieben =
        zeit_von(&kachel_pfad(out.path(), basis - 1, andere.parent())) + Duration::from_secs(60);
    std::fs::write(kaputt, b"RIFF").unwrap();
    setze_zeit(kaputt, geschrieben);
    let meldung = pruefe("abgeschnittene Kachel");
    assert!(
        meldung.contains("1 Kacheln nicht lesbar, übergangen"),
        "{meldung}"
    );

    // Die Elternkachel trägt eine Zeit vor der Kachel. Kommt sie heil
    // zurück, mit derselben Zeit, holt der nächste Aufruf sie ein.
    std::fs::write(kaputt, encode_webp(&vorlage).unwrap()).unwrap();
    setze_zeit(kaputt, geschrieben);
    let meldung = pruefe("heile Kachel mit alter Zeit");
    assert!(!meldung.contains("nicht lesbar"), "{meldung}");
}

/// Eine Kachel oder `map.json` mit einer Zeit in der Zukunft stammt von
/// einer Uhr, die vorging, nicht von einem Render daneben. `--pyramid`
/// behandelt sie wie jede andere: Über einer geänderten Basiskachel baut es
/// eine solche Kachel direkt darüber neu, obwohl die Basiskachel älter ist
/// als ihre Zeit, und ebenso eine zwei Stufen höher; `map.json` bekommt die
/// richtigen Grenzen. Früher blieb all das stehen, auch als die Uhr es
/// eingeholt hatte.
/// Was wirklich fremd ist, prüft `cli::tests::fremd_nur_auf_nativen_stufen`
/// im Binär, dort lässt sich der Beginn von aussen setzen.
#[test]
fn zukunft_ist_nicht_fremd() {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (0..4)
        .flat_map(|x| (0..4).map(move |z| (x * 3, z * 3)))
        .collect();
    common::write_world(welt.path(), &chunks, gelaende);
    let out = tempdir();
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "8", "--native-levels", "0"],
    ));
    let basis = max_zoom(out.path());
    assert!(basis > 2, "zu wenig Stufen");
    altern(out.path());

    let unten = kacheln(out.path(), basis);
    let (&eine, _) = unten.iter().next().unwrap();
    setze(
        out.path(),
        basis,
        eine,
        &bild(unten.values().nth(1).unwrap()),
    );
    let oben = eine.parent().parent();
    let rot = RgbaImage::from_pixel(256, 256, image::Rgba([200, 0, 0, 255]));
    setze(out.path(), basis - 1, eine.parent(), &rot);
    setze(out.path(), basis - 2, oben, &rot);
    let karte = out.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info["bounds"] = serde_json::json!([0, 0, 256, 256]);
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    let spaeter = SystemTime::now() + Duration::from_secs(3600);
    setze_zeit(&kachel_pfad(out.path(), basis - 1, eine.parent()), spaeter);
    setze_zeit(&kachel_pfad(out.path(), basis - 2, oben), spaeter);
    setze_zeit(&karte, spaeter);

    let ausgabe = pyramide(out.path());
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert_eq!(
        schnappschuss(out.path()),
        von_grund_auf(out.path()),
        "{meldung}"
    );
}

/// `--pyramid` braucht nur das Verzeichnis, aber eines mit Baum. Ohne
/// `map.json`, oder wenn auf deren Basisstufe keine Kachel liegt, ändert
/// es nichts; ein vergessenes `--scale` kann es so gar nicht geben.
/// Jeden weiteren Schalter lehnt es ab, auch Welt und Assets, die es nur
/// laden würde.
#[test]
fn pyramide_braucht_einen_baum() {
    let leer = tempdir();
    let ausgabe = pyramide(leer.path());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        !ausgabe.status.success() && meldung.contains("map.json fehlt"),
        "{meldung}"
    );
    assert!(std::fs::read_dir(leer.path()).unwrap().next().is_none());

    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));
    let karte = out.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info["maxZoom"] = (max_zoom(out.path()) + 1).into();
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    let vorher = schnappschuss(out.path());
    let ausgabe = pyramide(out.path());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        !ausgabe.status.success() && meldung.contains("dort liegt aber keine Kachel"),
        "{meldung}"
    );
    assert_eq!(schnappschuss(out.path()), vorher);

    for schalter in [
        &["--size", "2048"][..],
        &["--center", "0", "0"],
        &["--scale", "16"],
        &["--native-levels", "1"],
        &["--tiles", "anderswo"],
        &["--prune"],
        &["--world", "anderswo"],
        &["--assets", "anderswo"],
        &["--data", "anderswo"],
    ] {
        let mut args = vec![OsStr::new("--pyramid"), out.path().as_os_str()];
        args.extend(schalter.iter().map(OsStr::new));
        let ausgabe = cli(&args);
        let meldung = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(
            !ausgabe.status.success() && meldung.contains("cannot be used with"),
            "{schalter:?}: {meldung}"
        );
    }
}

/// Die Zahl der nativen Stufen gehört zum Baum wie der scale, `map.json`
/// hält sie fest. Ein Nachrendern ohne `--native-levels` nimmt sie von
/// dort und ändert an einer unveränderten Welt keine Datei; eines mit
/// einer anderen Zahl bricht ab, bevor es etwas schreibt. Mehr, als der
/// scale hergibt, heisst alle. Ein neuer Baum rendert ohne den Schalter
/// keine Stufe nativ. Einer aus einem älteren Stand ohne das Feld braucht
/// den Schalter einmal, ohne ihn bricht der Lauf ab, bevor er etwas
/// schreibt; danach steht die Zahl in `map.json`.
#[test]
fn native_stufen_gehoeren_zum_baum() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let baum = tempdir();
    gelungen(&export(
        welt.path(),
        baum.path(),
        &["--scale", "16", "--native-levels", "2"],
    ));
    assert_eq!(native_in(baum.path()), Some(2));
    let vorher = schnappschuss(baum.path());

    let ausschnitt = ["--scale", "16", "--center", "8", "8", "--size", "4"];
    gelungen(&export(welt.path(), baum.path(), &ausschnitt));
    assert!(
        schnappschuss(baum.path()) == vorher,
        "ohne Schalter nicht mehr nativ"
    );

    let anders = [&ausschnitt[..], &["--native-levels", "1"]].concat();
    let ausgabe = export(welt.path(), baum.path(), &anders);
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        !ausgabe.status.success() && meldung.contains("Mit --native-levels 2 weiterrendern"),
        "{meldung}"
    );
    assert!(schnappschuss(baum.path()) == vorher);

    let alle = [&ausschnitt[..], &["--native-levels", "9"]].concat();
    gelungen(&export(welt.path(), baum.path(), &alle));
    assert!(schnappschuss(baum.path()) == vorher);

    let neu = tempdir();
    let ausgabe = export(welt.path(), neu.path(), &["--scale", "16"]);
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    assert!(!meldung.contains("nativ bei scale"), "{meldung}");
    assert_eq!(native_in(neu.path()), Some(0));

    let karte = neu.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info.as_object_mut().unwrap().remove("nativeLevels");
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    let vorher = schnappschuss(neu.path());
    let ausgabe = export(welt.path(), neu.path(), &["--scale", "16"]);
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        !ausgabe.status.success() && meldung.contains("nennt keine Zahl nativer Stufen"),
        "{meldung}"
    );
    assert!(schnappschuss(neu.path()) == vorher);
    gelungen(&export(
        welt.path(),
        neu.path(),
        &["--scale", "16", "--native-levels", "1"],
    ));
    assert_eq!(native_in(neu.path()), Some(1));
    gelungen(&export(welt.path(), neu.path(), &["--scale", "16"]));
    assert_eq!(native_in(neu.path()), Some(1));

    // Bei scale 12 gibt es keine native Stufe, also nichts zu fragen.
    let zwoelf = tempdir();
    gelungen(&export(welt.path(), zwoelf.path(), &["--scale", "12"]));
    let karte = zwoelf.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info.as_object_mut().unwrap().remove("nativeLevels");
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    gelungen(&export(welt.path(), zwoelf.path(), &["--scale", "12"]));
    assert_eq!(native_in(zwoelf.path()), Some(0));
}

/// Die Zahl der nativen Stufen, wie `map.json` sie nennt.
fn native_in(dir: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(dir.join("map.json")).expect("map.json lesen");
    let info: serde_json::Value = serde_json::from_str(&text).expect("map.json auswerten");
    info["nativeLevels"].as_u64()
}

/// `--resume` rendert auf der Basis nur, was fehlt: vorhandene Kacheln
/// bleiben unangetastet, gelöschte kommen wieder, ebenso die jüngste, die
/// ein Stromausfall zerrissen hat: mit gutem Kopf, in voller Länge und mit
/// Nullen in der zweiten Hälfte. Die nativen Stufen rendert es ganz neu,
/// denn dort kann `--pyramid` eine Kachel verkleinert haben, bevor die
/// Basis darunter fertig war: hier aus den Kindern ohne das gelöschte. Am
/// Ende steht Byte für Byte dasselbe da wie nach einem Lauf in einem Stück.
/// Rate und Grösse zählen nur, was der Lauf gerendert hat.
#[test]
fn resume_rendert_nur_was_fehlt() {
    let welt = tempdir();
    // Östlich vom Ursprung, damit Kacheln sich eine Elternkachel teilen: am
    // Ursprung trennt die Pyramide Spalte -1 von Spalte 0.
    let chunks: Vec<(i32, i32)> = (4..8).map(|x| (x, 0)).collect();
    common::write_world(welt.path(), &chunks, gelaende);
    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));
    let soll = schnappschuss(out.path());
    let z = max_zoom(out.path());
    let basis = kacheln(out.path(), z);
    let nativ = kacheln(out.path(), z - 1);
    // Eine Basiskachel mit Geschwistern: ohne sie zeigt die Elternkachel
    // noch etwas.
    let kind = *basis
        .keys()
        .find(|tile| {
            let eltern = tile.parent();
            basis.keys().filter(|t| t.parent() == eltern).count() > 1
        })
        .expect("Geschwister auf der Basis");
    let (weg, weg_nativ) = (basis[&kind].clone(), nativ[&kind.parent()].clone());
    let mut andere = basis
        .iter()
        .filter(|(t, _)| **t != kind)
        .map(|(_, p)| p.clone());
    let (bleibt, zerrissen) = (
        andere.next().unwrap(),
        andere.next().expect("drei Basiskacheln"),
    );
    let groesse = |pfad: &Path| std::fs::metadata(pfad).unwrap().len();
    let neu = groesse(&weg) + groesse(&zerrissen);
    for pfad in [&weg, &weg_nativ] {
        std::fs::remove_file(pfad).unwrap();
    }
    gelungen(&pyramide(out.path()));
    let verkleinert = std::fs::read(&weg_nativ).expect("--pyramid baut die native Kachel");
    let eltern = kind.parent();
    assert_ne!(
        verkleinert,
        soll[&format!("{}/{}/{}.webp", z - 1, eltern.x, eltern.y)]
    );
    // Der Lauf brach vor einer Stunde ab. Zuletzt schrieb er diese Kachel,
    // und der Strom fiel aus, bevor ihre zweite Hälfte auf der Platte stand.
    zerreisse(&zerrissen);
    let damals = SystemTime::now() - Duration::from_secs(3600);
    for pfad in basis.values().filter(|pfad| pfad.is_file()) {
        setze_zeit(pfad, damals);
    }
    setze_zeit(&zerrissen, damals + Duration::from_secs(600));
    let vorher = zeit_von(&bleibt);
    // Mit nativen Stufen baut der Lauf die ganze Pyramide darüber neu.
    let oben: Vec<PathBuf> = (0..z - 1)
        .flat_map(|stufe| kacheln(out.path(), stufe).into_values())
        .collect();
    assert!(!oben.is_empty(), "keine Pyramide über der nativen Stufe");
    for pfad in &oben {
        setze_zeit(pfad, damals);
    }

    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "8", "--resume"]);
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    for erwartet in [
        "Kacheln:    2 geschrieben".to_string(),
        format!("{} vorhandene Kacheln übersprungen", basis.len() - 2),
        format!("{:.0} kB je Kachel", neu as f64 / 2.0 / 1024.0),
        format!("{} Kacheln nativ bei scale 4,", nativ.len()),
    ] {
        assert!(
            meldung.contains(&erwartet),
            "{erwartet} fehlt in: {meldung}"
        );
    }
    assert!(weg.is_file(), "die gelöschte Basiskachel fehlt weiterhin");
    assert_eq!(
        zeit_von(&bleibt),
        vorher,
        "vorhandene Basiskachel neu gerendert"
    );
    for pfad in &oben {
        assert_ne!(
            zeit_von(pfad),
            damals,
            "{} nicht neu gebaut",
            pfad.display()
        );
    }
    assert_eq!(schnappschuss(out.path()), soll);
}

/// Ohne native Stufen baut `--resume` die ganze Pyramide neu, wie jeder
/// Lauf: auch über Basiskacheln, die es nicht neu rendert, und auch eine
/// Elternkachel, die ein Stromausfall zerrissen hat und dieselbe Zeit trägt
/// wie alle anderen. Am Ende steht derselbe Baum da wie nach einem Lauf in
/// einem Stück.
#[test]
fn resume_ohne_native_stufen_baut_die_pyramide_neu() {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (0..4)
        .flat_map(|x| (0..4).map(move |z| (x * 3, z * 3)))
        .collect();
    common::write_world(welt.path(), &chunks, gelaende);
    let out = tempdir();
    let args = ["--scale", "8", "--native-levels", "0"];
    gelungen(&tiles(welt.path(), out.path(), &args));
    let soll = schnappschuss(out.path());
    let basis = max_zoom(out.path());
    assert!(basis > 1, "keine Pyramide zu prüfen");

    let damals = SystemTime::now() - Duration::from_secs(3600);
    let unten = kacheln(out.path(), basis);
    for pfad in unten.values() {
        setze_zeit(pfad, damals);
    }
    let mut reihe = unten.values();
    let (weg, frisch, bleibt) = (
        reihe.next().unwrap(),
        reihe.next().unwrap(),
        reihe.next().expect("drei Basiskacheln"),
    );
    std::fs::remove_file(weg).unwrap();
    let zuletzt = damals + Duration::from_secs(600);
    setze_zeit(frisch, zuletzt);
    let oben: Vec<PathBuf> = (0..basis)
        .flat_map(|z| kacheln(out.path(), z).into_values())
        .collect();
    for pfad in &oben {
        setze_zeit(pfad, damals);
    }
    let zerrissen = &oben[oben.len() / 2];
    zerreisse(zerrissen);
    setze_zeit(zerrissen, damals);

    let fortsetzen = [&args[..], &["--resume"]].concat();
    gelungen(&tiles(welt.path(), out.path(), &fortsetzen));
    assert_eq!(
        zeit_von(bleibt),
        damals,
        "vorhandene Basiskachel neu gerendert"
    );
    assert_ne!(
        zeit_von(frisch),
        zuletzt,
        "frische Basiskachel nicht neu gerendert"
    );
    for pfad in &oben {
        assert_ne!(
            zeit_von(pfad),
            damals,
            "{} nicht neu gebaut",
            pfad.display()
        );
    }
    assert_eq!(schnappschuss(out.path()), soll);
}

/// Ein Fortsetzen, das selbst abbricht, lässt keine zerrissene Kachel
/// zurück. Es entfernt die frischen Kacheln, bevor es rendert, und das
/// nächste findet sie als fehlend. Stünden sie noch da, wäre dessen jüngste
/// Kachel eine des abgebrochenen, und die zerrissene läge weit vor ihren
/// zwei Minuten. Das erste Fortsetzen läuft auf einem Thread, damit die
/// zerrissene als letzte drankommt, und endet hart nach seiner ersten
/// Kachel. Kommt der Test erst später zum Zug, hat es sie womöglich schon
/// neu gerendert; zerrissen ist sie dann auch nicht mehr.
#[test]
fn abgebrochenes_fortsetzen_laesst_nichts_zerrissen() {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (0..4).map(|x| (x, 0)).collect();
    common::write_world(welt.path(), &chunks, gelaende);
    let out = tempdir();
    let args = ["--scale", "32", "--native-levels", "0"];
    gelungen(&tiles(welt.path(), out.path(), &args));
    let soll = schnappschuss(out.path());

    // In der Reihenfolge, in der ein Lauf die Basis rendert.
    let mut basis: Vec<(TileId, PathBuf)> = kacheln(out.path(), max_zoom(out.path()))
        .into_iter()
        .collect();
    basis.sort_by_key(|(tile, _)| (tile.x >> 4, tile.y >> 4, tile.x, tile.y));
    assert!(
        basis.len() >= 12,
        "{} Basiskacheln sind zu wenige",
        basis.len()
    );
    let damals = SystemTime::now() - Duration::from_secs(3600);
    for (_, pfad) in &basis {
        setze_zeit(pfad, damals);
    }
    let zerrissen = basis.last().unwrap().1.clone();
    zerreisse(&zerrissen);
    let kaputt = std::fs::read(&zerrissen).unwrap();
    setze_zeit(&zerrissen, damals + Duration::from_secs(600));
    let fehlen: Vec<PathBuf> = basis[..basis.len() / 2]
        .iter()
        .map(|(_, pfad)| pfad.clone())
        .collect();
    for pfad in &fehlen {
        std::fs::remove_file(pfad).unwrap();
    }

    let fortsetzen = [&args[..], &["--resume"]].concat();
    let mut erstes = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .arg("--world")
        .arg(welt.path())
        .arg("--assets")
        .arg(assets_ref())
        .arg("--tiles")
        .arg(out.path())
        .args(&fortsetzen)
        .env("RAYON_NUM_THREADS", "1")
        .stdout(Stdio::null())
        .spawn()
        .expect("terranova-render starten");
    loop {
        // Erst fragen, ob es geendet hat, dann nach Kacheln sehen: endete es
        // dazwischen, hat der Test seine Kacheln trotzdem gesehen.
        let geendet = erstes.try_wait().unwrap().is_some();
        if fehlen.iter().any(|pfad| pfad.exists()) {
            break;
        }
        assert!(
            !geendet,
            "das erste Fortsetzen endete, bevor es eine Kachel schrieb"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    erstes.kill().unwrap();
    erstes.wait().unwrap();
    assert!(
        !std::fs::read(&zerrissen).is_ok_and(|jetzt| jetzt == kaputt),
        "die zerrissene Kachel steht noch da"
    );

    gelungen(&tiles(welt.path(), out.path(), &fortsetzen));
    assert_eq!(schnappschuss(out.path()), soll);
}

/// Unter Windows nennt der erste Export in ein Verzeichnis die Befehle für
/// eine Ausnahme im Echtzeitschutz, für genau diesen Ordner und absolut,
/// auch wenn `--tiles` ihn relativ angibt; der zweite schweigt, dort steht
/// schon `map.json`. Für einen Ordner, in dem schon anderes liegt, gibt es
/// ihn nicht, und anderswo als unter Windows nie.
#[test]
fn hinweis_auf_den_echtzeitschutz_nur_beim_ersten_export() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let eltern = tempdir();
    let lauf = |ordner: &str| {
        let ausgabe = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
            .current_dir(eltern.path())
            .arg("--world")
            .arg(welt.path())
            .arg("--assets")
            .arg(assets())
            .args(["--tiles", ordner, "--scale", "8", "--native-levels", "0"])
            .output()
            .expect("terranova-render starten");
        String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned()
    };
    let (erster, zweiter) = (lauf("karte"), lauf("karte"));
    let ordner = eltern.path().join("karte");
    for befehl in ["Add-MpPreference", "Remove-MpPreference"] {
        let zeile = format!("{befehl} -ExclusionPath '{}'", ordner.display());
        assert_eq!(erster.contains(&zeile), cfg!(windows), "{zeile}: {erster}");
    }
    assert!(!zweiter.contains("-ExclusionPath"), "{zweiter}");

    std::fs::create_dir(eltern.path().join("voll")).unwrap();
    std::fs::write(eltern.path().join("voll").join("notizen.txt"), "x").unwrap();
    let voll = lauf("voll");
    assert!(!voll.contains("-ExclusionPath"), "{voll}");
}

/// Eine Ausnahme im Echtzeitschutz gibt es nur unter Windows; anderswo
/// bricht der Schalter ab, bevor ein Chunk gelesen ist.
#[cfg(not(windows))]
#[test]
fn defender_exclusion_nur_unter_windows() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let out = tempdir();
    let ausgabe = tiles(welt.path(), out.path(), &["--defender-exclusion"]);
    assert!(!ausgabe.status.success());
    let fehler = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(fehler.contains("nur unter Windows"), "{fehler}");
}

/// `map.json` muss beschreiben, was tatsächlich dasteht.
#[test]
fn map_json_beschreibt_die_kacheln() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));

    let text = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    let info: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(info["tileSize"], 256);
    assert_eq!(info["minZoom"], 0);
    assert_eq!(info["scale"], 8);
    assert_eq!(info["tiles"], "{z}/{x}/{y}.webp");

    let basis = info["maxZoom"].as_u64().unwrap() as u32;
    assert!(
        !kacheln(out.path(), basis).is_empty(),
        "auf maxZoom liegt nichts"
    );
    assert!(
        kacheln(out.path(), basis + 1).is_empty(),
        "unter maxZoom liegt noch eine Stufe"
    );

    // bounds muss jede Basiskachel umschliessen.
    let grenzen: Vec<i32> = info["bounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect();
    let tile = info["tileSize"].as_i64().unwrap() as i32;
    for kachel in kacheln(out.path(), basis).keys() {
        assert!(kachel.x * tile >= grenzen[0], "{kachel:?} links raus");
        assert!(kachel.y * tile >= grenzen[1], "{kachel:?} oben raus");
        assert!(
            (kachel.x + 1) * tile <= grenzen[2],
            "{kachel:?} rechts raus"
        );
        assert!((kachel.y + 1) * tile <= grenzen[3], "{kachel:?} unten raus");
    }
}

/// Die Zoomnummer hängt an der Welt, nicht am Massstab des Laufs — aber
/// die Zahl der Stufen sehr wohl.
#[test]
fn zoomstufen_haengen_am_massstab() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);

    let fein = tempdir();
    let grob = tempdir();
    gelungen(&tiles(welt.path(), fein.path(), &["--scale", "16"]));
    gelungen(&tiles(welt.path(), grob.path(), &["--scale", "4"]));

    assert!(
        max_zoom(fein.path()) > max_zoom(grob.path()),
        "feiner Massstab braucht mehr Stufen: {} gegen {}",
        max_zoom(fein.path()),
        max_zoom(grob.path())
    );
}

/// Ein Ausschnitt, in einen fertigen Kachelbaum nachgerendert, darf an
/// einer unveränderten Welt nichts ändern.
///
/// Die nativen Stufen rendern ihre Elternkacheln ganz aus der Welt, der
/// Export rundet den Ausschnitt deshalb auf ihr Raster auf. Der Stein in
/// Chunk (10, 4) liegt ausserhalb des angefragten Ausschnitts, aber in
/// derselben Elternkachel zwei Stufen darüber: rundete der Export nicht
/// auf, fehlte er in deren Sprite-Tabelle und würde dort zu Luft.
///
/// Chunk (20, 0) liegt auch ausserhalb der gerundeten Fläche. Weiter oben
/// in der Pyramide teilen sich seine Kacheln Eltern mit denen des
/// Ausschnitts: wer beim Neubauen nur die Kacheln dieses Laufs nimmt,
/// schreibt sie mit durchsichtigen Lücken zu, und `map.json` schrumpft auf
/// den Ausschnitt. Und der Lauf darf seine Kacheln nicht für verwaist
/// halten, nur weil der Vorlauf ihn nicht sieht, auch nicht mit --prune.
/// Mit nativen Stufen und ohne, dann mit einem Ausschnitt, der nicht
/// aufgerundet wird.
#[test]
fn nachrendern_in_einen_bestehenden_baum_aendert_nichts() {
    let welt = tempdir();
    common::write_world(
        welt.path(),
        &[(2, 0), (4, 0), (10, 4), (20, 0)],
        |x, y, z| match (x, y, z) {
            (44, 4, 8) => "minecraft:einfarbig",
            (76, 4, 8) => "minecraft:blauwuerfel",
            (165, 4, 65) => "minecraft:stone",
            (328, 4, 8) => "minecraft:einfarbig",
            _ => "minecraft:air",
        },
    );

    for native in ["9", "0"] {
        let schalter = ["--scale", "16", "--native-levels", native];
        let out = tempdir();
        gelungen(&tiles(welt.path(), out.path(), &schalter));
        let vorher = schnappschuss(out.path());
        assert!(
            vorher.len() > 3,
            "zu wenig zum Vergleichen: {:?}",
            vorher.keys().collect::<Vec<_>>()
        );

        // Dieselbe Welt, nur ein Ausschnitt um den ersten Block, in dasselbe
        // Verzeichnis.
        let ausschnitt = [
            &schalter[..],
            &["--center", "44", "8", "--size", "4", "--prune"],
        ]
        .concat();
        gelungen(&tiles(welt.path(), out.path(), &ausschnitt));

        let nachher = schnappschuss(out.path());
        assert_eq!(
            nachher.keys().collect::<Vec<_>>(),
            vorher.keys().collect::<Vec<_>>(),
            "--native-levels {native}: der Baum hat andere Dateien als vorher"
        );
        for (rel, alt) in &vorher {
            assert_eq!(
                &nachher[rel], alt,
                "--native-levels {native}: {rel} hat sich verändert"
            );
        }
    }
}

/// Ein Ausschnitt mit nativen Stufen braucht die Blöcke seiner ganzen
/// Elternfläche. Fehlt dort ein Asset, bricht der Lauf ab, bevor er eine
/// Kachel schreibt — nicht erst nach der Basis, mit alten gröberen Stufen
/// und in einem frischen Verzeichnis ohne Karte.
#[test]
fn unbekannter_block_in_der_elternflaeche_bricht_vor_dem_schreiben_ab() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (6, 6)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (100, 4, 100) => "minecraft:gibt_es_nicht",
        _ => "minecraft:air",
    });
    let out = tempdir();
    let ausgabe = tiles(
        welt.path(),
        out.path(),
        &["--scale", "16", "--center", "8", "8", "--size", "4"],
    );
    assert!(
        !ausgabe.status.success(),
        "der Block liegt in der Elternfläche und fehlt"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("gibt_es_nicht"), "Meldung: {meldung}");
    assert_eq!(
        dateien(out.path()),
        Vec::<String>::new(),
        "vor dem Fehler geschrieben"
    );
}

/// Nachgerendert wird ein Ausschnitt einer veränderten Welt. Die nativen
/// Stufen zeigen ganze Elternkacheln; damit alle Stufen denselben Stand
/// zeigen, reicht auch die Basis so weit. Danach gleicht der Baum einem
/// Vollexport der neuen Welt — sonst stünde ein Neubau neben dem
/// Ausschnitt nur auf den gröberen Stufen. Der neue Block liegt dafür
/// ausserhalb der Basiskachel des Ausschnitts, in seiner gerundeten
/// Fläche. Ohne native Stufen wird nicht gerundet; dann liegt er in der
/// Basiskachel, und auch jede gröbere Stufe muss ihn zeigen.
#[test]
fn nachrendern_zeigt_auf_allen_stufen_denselben_stand() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(2, 0), (4, 0)], |x, y, z| match (x, y, z) {
        (44, 4, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    });
    for (native, block) in [("9", (76, 4, 8)), ("0", (46, 4, 8))] {
        let neu = tempdir();
        common::write_world(neu.path(), &[(2, 0), (4, 0)], move |x, y, z| {
            match (x, y, z) {
                (44, 4, 8) => "minecraft:einfarbig",
                ort if ort == block => "minecraft:blauwuerfel",
                _ => "minecraft:air",
            }
        });

        let schalter = ["--scale", "16", "--native-levels", native];
        let baum = tempdir();
        gelungen(&tiles(alt.path(), baum.path(), &schalter));
        let ausschnitt = [&schalter[..], &["--center", "44", "8", "--size", "4"]].concat();
        gelungen(&tiles(neu.path(), baum.path(), &ausschnitt));
        let voll = tempdir();
        gelungen(&tiles(neu.path(), voll.path(), &schalter));

        let nachher = schnappschuss(baum.path());
        let soll = schnappschuss(voll.path());
        assert_eq!(
            nachher.keys().collect::<Vec<_>>(),
            soll.keys().collect::<Vec<_>>(),
            "--native-levels {native}"
        );
        for (rel, inhalt) in &soll {
            assert_eq!(
                &nachher[rel], inhalt,
                "--native-levels {native}: {rel} zeigt einen anderen Stand"
            );
        }
    }
}

/// Wächst die Welt über eine Zweierpotenz an Kacheln hinaus, behält ein
/// bestehender Baum seine Nummerierung, und der nächste Lauf rendert weiter
/// hinein, statt abzubrechen. Zoom 0 zeigt dann mehr als eine Kachel.
#[test]
fn gewachsene_welt_behaelt_die_nummerierung() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    });
    let baum = tempdir();
    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "16"]));
    let vorher = max_zoom(baum.path());

    // Eine neue Region weit draussen.
    common::write_world(welt.path(), &[(160, 0)], |x, y, z| {
        if (x, y, z) == (2568, 4, 8) {
            "minecraft:blauwuerfel"
        } else {
            "minecraft:air"
        }
    });
    let frisch = tempdir();
    gelungen(&tiles(welt.path(), frisch.path(), &["--scale", "16"]));
    let neu = max_zoom(frisch.path());
    assert!(neu > vorher, "die Welt ist nicht gewachsen: {neu}");

    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "16"]));
    assert_eq!(max_zoom(baum.path()), vorher, "Nummerierung verloren");
    // Die Basis ist dieselbe wie im frischen Baum, nur unter ihrer alten
    // Nummer.
    let basis = kacheln(baum.path(), vorher);
    let soll = kacheln(frisch.path(), neu);
    assert_eq!(
        basis.keys().collect::<Vec<_>>(),
        soll.keys().collect::<Vec<_>>()
    );
    for (tile, pfad) in &soll {
        assert_eq!(
            std::fs::read(&basis[tile]).unwrap(),
            std::fs::read(pfad).unwrap(),
            "{tile:?}"
        );
    }
    assert!(!kacheln(baum.path(), 0).is_empty(), "Zoom 0 fehlt");
}

/// Kacheln und `map.json` werden getauscht, nicht überschrieben: wer eine
/// Datei gerade liest, liest sie zu Ende, wie sie war, und ein Abbruch
/// mitten im Schreiben hinterlässt die alte. Der Test hält die Basis und
/// `map.json` offen, während ein zweiter Lauf eine veränderte, grössere
/// Welt schreibt. Daneben bleibt keine eigene Datei übrig.
#[test]
fn schreiben_tauscht_die_datei() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (2, 2)], gelaende);
    let neu = tempdir();
    common::write_world(
        neu.path(),
        &[(0, 0), (2, 2), (6, 0)],
        |x, y, z| match gelaende(x, y, z) {
            "minecraft:blauwuerfel" => "minecraft:einfarbig",
            block => block,
        },
    );
    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let karte = baum.path().join("map.json");
    let offen: Vec<(PathBuf, Vec<u8>, std::fs::File)> = kacheln(baum.path(), max_zoom(baum.path()))
        .into_values()
        .chain([karte.clone()])
        .map(|pfad| {
            let vorher = std::fs::read(&pfad).unwrap();
            let datei = std::fs::File::open(&pfad).unwrap();
            (pfad, vorher, datei)
        })
        .collect();

    gelungen(&tiles(neu.path(), baum.path(), &["--scale", "16"]));
    let mut geaendert = Vec::new();
    for (pfad, vorher, mut datei) in offen {
        let mut gelesen = Vec::new();
        datei.read_to_end(&mut gelesen).unwrap();
        assert!(gelesen == vorher, "{} überschrieben", pfad.display());
        if std::fs::read(&pfad).unwrap() != vorher {
            geaendert.push(pfad);
        }
    }
    assert!(
        geaendert.contains(&karte) && geaendert.len() > 1,
        "map.json und eine Kachel hätten sich ändern müssen: {geaendert:?}"
    );

    let mut reste = Vec::new();
    let mut stapel = vec![baum.path().to_path_buf()];
    while let Some(ordner) = stapel.pop() {
        for eintrag in std::fs::read_dir(ordner).unwrap().flatten() {
            if eintrag.path().is_dir() {
                stapel.push(eintrag.path());
            } else if !eintrag.file_name().to_string_lossy().ends_with(".webp")
                && eintrag.file_name() != "map.json"
            {
                reste.push(eintrag.path());
            }
        }
    }
    assert!(reste.is_empty(), "{reste:?}");
}

/// Bricht der erste Lauf beim Schreiben der Kacheln ab, steht trotzdem
/// schon im `map.json`, zu welchem scale der Baum gehört. Sonst mischte
/// der nächste Lauf mit anderem scale seine Kacheln unter die des
/// abgebrochenen.
#[test]
fn abgebrochener_lauf_hinterlaesst_map_json() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let probe = tempdir();
    gelungen(&tiles(welt.path(), probe.path(), &["--scale", "16"]));
    // Wo die letzte Kachel hin soll, steht ein Verzeichnis: sie zu
    // schreiben scheitert.
    let out = tempdir();
    let kachel = dateien(probe.path()).pop().expect("eine Kachel");
    std::fs::create_dir_all(out.path().join(&kachel)).unwrap();
    assert!(
        !tiles(welt.path(), out.path(), &["--scale", "16"])
            .status
            .success(),
        "{kachel} hätte sich nicht schreiben lassen dürfen"
    );
    let ausgabe = tiles(welt.path(), out.path(), &[]);
    assert!(!ausgabe.status.success());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("scale 16"), "Meldung: {meldung}");
}

/// Scheitert ein Lauf vor der ersten Kachel, etwa an einem fehlenden
/// Asset, legt er für das Verzeichnis nichts fest: der nächste darf einen
/// anderen scale nehmen.
#[test]
fn gescheiterter_lauf_legt_nichts_fest() {
    let kaputt = tempdir();
    common::write_world(kaputt.path(), &[(0, 0)], |x, y, z| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:gibt_es_nicht"
        } else {
            "minecraft:air"
        }
    });
    let out = tempdir();
    assert!(
        !tiles(kaputt.path(), out.path(), &["--scale", "16"])
            .status
            .success(),
        "der erste Lauf hätte am fehlenden Asset scheitern müssen"
    );
    assert!(!out.path().join("map.json").exists(), "map.json festgelegt");

    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));
    let text = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    assert!(text.contains(r#""scale": 8"#), "{text}");
}

/// Ein Baum gehört zu einer Welt. Eine andere mit demselben scale renderte
/// sonst still hinein: ihre Basis landete auf der Stufe der ersten, und wo
/// sie keine Chunks hat, blieben deren Kacheln stehen.
#[test]
fn fremde_welt_wird_abgelehnt() {
    let erste = tempdir();
    common::write_world(erste.path(), &[(0, 0), (2, 2)], gelaende);
    common::write_wurzel(erste.path(), 4_815_162_342);
    let zweite = tempdir();
    common::write_world(zweite.path(), &[(0, 0)], gelaende);
    common::write_wurzel(zweite.path(), 2_718_281_828);

    let out = tempdir();
    gelungen(&tiles(erste.path(), out.path(), &["--scale", "16"]));
    // `map.json` liegt öffentlich neben den Kacheln: den Seed selbst
    // verrät es nicht, nur seine Kennung.
    let karte = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    let kennung = kennung_in(&karte);
    let seed = 4_815_162_342_i64;
    assert!(!karte.contains(&seed.to_string()), "{karte}");
    assert!(!karte.contains(&format!("{seed:x}")), "{karte}");
    let vorher = schnappschuss(out.path());
    let ausgabe = tiles(zweite.path(), out.path(), &["--scale", "16"]);
    assert!(!ausgabe.status.success(), "die fremde Welt lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains(&format!(
            "anderen Welt oder Dimension: Kennung dort {kennung}"
        )),
        "Meldung: {meldung}"
    );
    assert_eq!(
        schnappschuss(out.path()),
        vorher,
        "der Baum hat sich verändert"
    );

    // Dieselbe Welt darf weiter.
    gelungen(&tiles(erste.path(), out.path(), &["--scale", "16"]));
}

/// Ein Baum eines älteren Stands trägt kein Feld `world`. Er gehört ab dem
/// nächsten Lauf zu dessen Welt, und danach ist er geschützt wie jeder
/// andere. Sonst müsste jeder bestehende Baum neu entstehen. Ein Baum einer
/// Welt ohne Kennung trägt dagegen `"world": null` und nimmt keine Welt
/// mit Kennung auf; früher sah er aus wie einer von früher, und jede Welt
/// kam hinein.
#[test]
fn alter_baum_ohne_kennung_wird_uebernommen() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    // Ohne level.dat hat die Welt keine Kennung.
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));
    let karte = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    assert!(karte.contains("\"world\": null"), "{karte}");

    common::write_wurzel(welt.path(), 4_815_162_342);
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "die Welt kam in einen fremden Baum"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("Welt ohne Kennung"), "{meldung}");

    // So sieht ein Baum eines älteren Stands aus. Eine Welt ohne Kennung
    // übernimmt ihn nicht, er bleibt für seine eigene.
    let mut alt: serde_json::Value = serde_json::from_str(&karte).unwrap();
    alt.as_object_mut().unwrap().remove("world");
    let alt = alt.to_string();
    std::fs::write(out.path().join("map.json"), &alt).unwrap();
    let ohne = tempdir();
    common::write_world(ohne.path(), &[(0, 0)], gelaende);
    let ausgabe = tiles(ohne.path(), out.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "die Welt ohne Kennung kam hinein"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("älteren Stand"), "{meldung}");
    let karte = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    assert_eq!(karte, alt, "map.json hat sich geändert");
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "16"]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    assert!(text.contains("nannte keine Welt"), "{text}");
    let karte = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    kennung_in(&karte);

    // Der nächste Lauf nimmt das Salz aus dem Baum und erkennt die Welt.
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));
    let danach = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    assert_eq!(danach, karte, "die Kennung hat sich geändert");

    let fremd = tempdir();
    common::write_world(fremd.path(), &[(0, 0)], gelaende);
    common::write_wurzel(fremd.path(), 2_718_281_828);
    assert!(
        !tiles(fremd.path(), out.path(), &["--scale", "16"])
            .status
            .success(),
        "nach dem Übernehmen ist der Baum geschützt"
    );
}

/// Ein Baum eines älteren Stands mit scale 6 lässt sich nicht fortsetzen:
/// `--scale 6` nimmt dieser Stand nicht mehr an. Die Meldung darf das
/// nicht raten.
#[test]
fn alter_scale_nennt_den_ausweg() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    common::write_wurzel(welt.path(), 4_815_162_342);
    let out = tempdir();
    std::fs::write(
        out.path().join("map.json"),
        r#"{"tileSize":256,"scale":6,"minZoom":0,"maxZoom":3,"tiles":"{z}/{x}/{y}.webp","bounds":[0,0,256,256]}"#,
    )
    .unwrap();
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "8"]);
    assert!(!ausgabe.status.success());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("neues Verzeichnis"), "Meldung: {meldung}");
    assert!(!meldung.contains("--scale 6"), "Meldung: {meldung}");
    // Der Baum ohne Kennung wäre übernommen worden. Gemeldet wird das erst
    // vor der ersten Kachel, und die kommt nie.
    let text = String::from_utf8_lossy(&ausgabe.stdout);
    assert!(!text.contains("keine Welt"), "{text}");
}

/// Alle Dimensionen einer Welt tragen denselben Seed. Die Kennung nimmt die
/// Dimension dazu: der Nether kommt nicht in den Baum der Oberwelt und die
/// Oberwelt nicht in seinen. Die Oberwelt, einmal über die Wurzel und
/// einmal über ihr Dimensionsverzeichnis, ist dieselbe Welt. Einer Kopie
/// ohne level.dat rät die Meldung zur Wurzel statt zu einem neuen Baum, und
/// zum Hochziehen, falls es `DIM-1` einer Welt vor 26.1 ist; einer mit
/// level.dat, aber ohne Seed, nicht noch einmal zur Wurzel.
#[test]
fn dimensionen_haben_eigene_kennungen() {
    let welt = tempdir();
    let oberwelt = welt.path().join("dimensions/minecraft/overworld");
    let nether = welt.path().join("dimensions/minecraft/the_nether");
    common::write_world(&oberwelt, &[(0, 0)], gelaende);
    common::write_world(&nether, &[(0, 0)], gelaende);
    common::write_wurzel(welt.path(), 4_815_162_342);

    let baum = tempdir();
    gelungen(&tiles(&nether, baum.path(), &["--scale", "16"]));
    let karte = std::fs::read_to_string(baum.path().join("map.json")).unwrap();
    kennung_in(&karte);
    let ausgabe = tiles(welt.path(), baum.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "die Oberwelt kam in den Netherbaum"
    );

    let baum = tempdir();
    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "16"]));
    let ausgabe = tiles(&nether, baum.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "der Nether kam in den Baum der Oberwelt"
    );
    gelungen(&tiles(&oberwelt, baum.path(), &["--scale", "16"]));

    let kopie = tempdir();
    common::write_world(kopie.path(), &[(0, 0)], gelaende);
    let meldung = || {
        let ausgabe = tiles(kopie.path(), baum.path(), &["--scale", "16"]);
        assert!(!ausgabe.status.success());
        String::from_utf8_lossy(&ausgabe.stderr).into_owned()
    };
    let ohne_wurzel = meldung();
    assert!(
        ohne_wurzel.contains("keine Weltwurzel mit level.dat"),
        "{ohne_wurzel}"
    );
    assert!(ohne_wurzel.contains("--forceUpgrade"), "{ohne_wurzel}");
    assert!(!ohne_wurzel.contains("neues Verzeichnis"), "{ohne_wurzel}");
    common::write_level_dat(kopie.path());
    let ohne_seed = meldung();
    assert!(ohne_seed.contains("nennt keinen Seed"), "{ohne_seed}");
    assert!(!ohne_seed.contains("Wurzel richten"), "{ohne_seed}");
}

/// Ohne Seed nennt die Ausgabe jeden Ort, an dem er gesucht wurde, von der
/// Datei der Dimension bis zu der der Paper-Oberwelt, und den Ausweg für
/// eine Welt vor 26.1. Ein neuer Baum entsteht trotzdem, mit
/// `"world": null`, und der Lauf sagt, warum.
#[test]
fn ohne_seed_nennt_jeden_ort() {
    let welt = tempdir();
    let nether = welt.path().join("dimensions/minecraft/the_nether");
    common::write_world(&nether, &[(0, 0)], gelaende);
    common::write_level_dat(welt.path());
    let baum = tempdir();
    let ausgabe = tiles(&nether, baum.path(), &["--scale", "16"]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    let orte = "weder in dimensions/minecraft/the_nether/data/minecraft/world_gen_settings.dat, \
                data/minecraft/world_gen_settings.dat \
                noch in dimensions/minecraft/overworld/data/minecraft/world_gen_settings.dat";
    assert!(text.contains(orte), "{text}");
    assert!(
        text.contains("mit Minecraft 26.2 und --forceUpgrade"),
        "{text}"
    );
    assert!(text.contains("\"world\": null"), "{text}");
    let karte = std::fs::read_to_string(baum.path().join("map.json")).unwrap();
    assert!(karte.contains("\"world\": null"), "{karte}");
}

/// Ein relativer Pfad führt zur selben Welt wie der absolute, auch `.` in
/// einer Dimension: der Baum nimmt den zweiten Lauf auf. Ohne den Weg zur
/// Wurzel hätte `.` keinen Namen und die Welt keine Kennung.
#[test]
fn punkt_als_welt_hat_dieselbe_kennung() {
    let welt = tempdir();
    let nether = welt.path().join("dimensions/minecraft/the_nether");
    common::write_world(&nether, &[(0, 0)], gelaende);
    common::write_wurzel(welt.path(), 42);
    let baum = tempdir();
    gelungen(&tiles(&nether, baum.path(), &["--scale", "16"]));
    let karte = || std::fs::read_to_string(baum.path().join("map.json")).unwrap();
    let vorher = kennung_in(&karte());
    let ausgabe = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .current_dir(&nether)
        .args(["--world", ".", "--assets"])
        .arg(assets())
        .arg("--tiles")
        .arg(baum.path())
        .args(["--scale", "16"])
        .output()
        .expect("terranova-render starten");
    gelungen(&ausgabe);
    assert_eq!(kennung_in(&karte()), vorher);
}

/// Jeder neue Baum zieht sein eigenes Salz. Mit einem festen liesse sich
/// eine Tabelle über alle Seeds einmal rechnen und gegen jeden Baum
/// halten. Zweimal dasselbe Verzeichnis und zwei andere: ein Salz aus dem
/// Zielpfad wäre beim ersten Paar gleich. Eines mit 8 Bit bliebe unter 256;
/// dass vier zufällige mit 64 Bit alle unter 2^56 liegen, ist 2^-32.
#[test]
fn zwei_baeume_bekommen_verschiedene_salze() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    common::write_wurzel(welt.path(), 4_815_162_342);
    let salz = |baum: &Path| {
        gelungen(&tiles(welt.path(), baum, &["--scale", "16"]));
        let karte = std::fs::read_to_string(baum.join("map.json")).unwrap();
        std::fs::remove_dir_all(baum).unwrap();
        std::fs::create_dir(baum).unwrap();
        let kennung = kennung_in(&karte);
        u64::from_str_radix(kennung.split('-').next().unwrap(), 16).unwrap()
    };
    let baum = tempdir();
    let salze = [
        salz(baum.path()),
        salz(baum.path()),
        salz(tempdir().path()),
        salz(tempdir().path()),
    ];
    for (i, a) in salze.iter().enumerate() {
        assert!(!salze[i + 1..].contains(a), "{salze:x?}");
    }
    assert!(salze.iter().any(|&s| s >= 1 << 56), "{salze:x?}");
}

/// Die Kennung aus `map.json`: Salz und Hash, je 16 Hexziffern.
fn kennung_in(karte: &str) -> String {
    let json: serde_json::Value = serde_json::from_str(karte).unwrap();
    let kennung = json["world"].as_str().expect("map.json nennt die Welt");
    let teile: Vec<&str> = kennung.split('-').collect();
    assert!(
        teile.len() == 2
            && teile
                .iter()
                .all(|t| t.len() == 16 && t.chars().all(|c| c.is_ascii_hexdigit())),
        "Kennung {kennung}"
    );
    kennung.to_string()
}

/// `--size 0` gäbe ein leeres Rechteck. Das rundet nicht auf das Raster
/// der nativen Stufen auf: der Vorlauf sähe nur den Mittelpunkt, und den
/// nativen Stufen fehlten die Blöcke ihrer Elternkacheln.
#[test]
fn size_null_wird_abgelehnt() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let out = tempdir();
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "16", "--size", "0"]);
    assert!(!ausgabe.status.success(), "--size 0 lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("--size"), "Meldung: {meldung}");
    assert!(schnappschuss(out.path()).is_empty(), "etwas geschrieben");
}

/// Eine geflutete Truhe bleibt eine Truhe, die Minecraft als Entity
/// zeichnet; auf der Karte steht dort nur ihr Wasser. `--block` muss das
/// sagen, auch wenn der Block Wasser enthält.
#[test]
fn geflutete_truhe_bleibt_ein_entity() {
    let ausgabe = cli(&[
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--block"),
        OsStr::new("chest[waterlogged=true]"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(text.contains("kein Modell"), "{text}");
    assert!(text.contains("Flüssigkeit: Water"), "{text}");
}

/// `--scan` nennt dieselben Blöcke ohne Modell wie `--block`: die
/// geflutete Truhe ja, Wasser nicht.
#[test]
fn scan_nennt_die_geflutete_truhe_aber_nicht_das_wasser() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:chest[waterlogged=true]",
        (9, 4, 8) => "minecraft:water",
        (10, 4, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    });
    let ausgabe = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--scan"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    let (_, liste) = text.split_once("Blöcke ohne Modell:").expect(&text);
    let namen: Vec<&str> = liste
        .lines()
        .skip(1)
        .map(str::trim)
        .take_while(|zeile| zeile.starts_with("minecraft:"))
        .collect();
    assert_eq!(namen, ["minecraft:chest"], "{text}");
}

/// Wasser hat kein Modell-JSON, der Renderer baut es im Code. `--block`
/// darf es deshalb nicht als modelllos melden.
#[test]
fn block_nennt_wasser_nicht_modelllos() {
    let ausgabe = cli(&[
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--block"),
        OsStr::new("water"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(!text.contains("kein Modell"), "{text}");
    assert!(text.contains("Flüssigkeit: Water"), "{text}");
}

/// Fragt ein Pack in multipart, was blocks.txt nicht kennt, sagt der Lauf
/// es, nennt die Datei und den Ausweg für neuere Assets.
#[test]
fn unbekannte_bedingung_nennt_blocks_txt() {
    let overlay = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-overlay");
    let ausgabe = cli(&[
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--assets"),
        overlay.as_os_str(),
        OsStr::new("--block"),
        OsStr::new("cobblestone_wall[north=true]"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(text.contains("minecraft:block/blauwuerfel"), "{text}");
    assert!(
        text.contains("was blocks.txt aus 26.2 nicht kennt"),
        "{text}"
    );
    assert!(
        text.contains("blocks.txt neu erzeugen und neu bauen"),
        "{text}"
    );
    assert!(
        text.contains("cobblestone_wall.json: Wert true für north"),
        "{text}"
    );
}

/// Ein vergessenes `--scale` darf einen bestehenden Baum nicht zerlegen:
/// die neuen Kacheln hätten auf denselben Stufen einen anderen Massstab als
/// die alten.
#[test]
fn anderer_scale_wird_abgelehnt() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));
    let vorher = schnappschuss(out.path());

    // Ohne --scale: der Standard ist 32.
    let ausgabe = tiles(
        welt.path(),
        out.path(),
        &["--center", "4", "8", "--size", "4"],
    );
    assert!(
        !ausgabe.status.success(),
        "der Lauf mit scale 32 hätte abbrechen müssen"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("scale 16"), "Meldung: {meldung}");
    assert_eq!(
        schnappschuss(out.path()),
        vorher,
        "der Baum hat sich verändert"
    );
}

/// Auch wenn nichts sichtbar ist, muss `map.json` geschrieben werden — und
/// dafür muss das Zielverzeichnis erst einmal entstehen.
#[test]
fn leeres_ergebnis_legt_das_ziel_trotzdem_an() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| {
        if (x, y, z) == (8, 4, 8) {
            "minecraft:durchsichtig"
        } else {
            "minecraft:air"
        }
    });

    let eltern = tempdir();
    let ziel = eltern.path().join("gibt-es-noch-nicht");
    gelungen(&tiles(
        welt.path(),
        &ziel,
        &["--scale", "16", "--size", "256"],
    ));

    assert!(ziel.join("map.json").is_file(), "map.json fehlt");
    assert!(dateien(&ziel).is_empty(), "es dürfte keine Kachel geben");
}

/// `--gpu on` liefert dieselben Dateien wie `--gpu off`, Byte für Byte —
/// Kacheln, Pyramide, `map.json`. Ohne Adapter (auch keinen
/// Software-Adapter) wird übersprungen und gesagt.
#[test]
fn gpu_liefert_dieselben_kacheln() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (1, 1)], gelaende);

    let cpu = tempdir();
    let gpu = tempdir();
    gelungen(&tiles(
        welt.path(),
        cpu.path(),
        &["--scale", "16", "--gpu", "off"],
    ));
    let lauf = tiles(welt.path(), gpu.path(), &["--scale", "16", "--gpu", "on"]);
    if !lauf.status.success()
        && String::from_utf8_lossy(&lauf.stderr).contains("keine Grafikkarte gefunden")
    {
        eprintln!("kein GPU-Adapter, auch kein Software-Adapter — Test übersprungen");
        return;
    }
    gelungen(&lauf);
    assert!(
        String::from_utf8_lossy(&lauf.stdout).contains("Threads + GPU"),
        "die Ausgabe nennt die GPU nicht"
    );
    assert_eq!(schnappschuss(cpu.path()), schnappschuss(gpu.path()));
}
