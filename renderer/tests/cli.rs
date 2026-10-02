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
use terranova_render::assets::{Assets, DimensionType};
use terranova_render::render::heights::{self, EMPTY, Heights};
use terranova_render::render::rasterizer::{Light, Lightmap};
use terranova_render::render::{
    BLEND_DEFAULT, BiomeTable, ChunkCache, Kamera, Projection, SpriteSet, TileId, encode_webp,
    pyramid, render_area, render_area_with, streifenbreite, survey,
};
use terranova_render::world::World;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base")
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

/// Ein Baum in einer eigenen Wurzel, wie `--tiles` ihn anlegt. Die Tests
/// rechnen mit [`Baum::path`], dem Ordner des Baums; `export` gibt
/// `--tiles` die Wurzel darüber und prüft, dass der Lauf genau diesen Ordner
/// beschreibt.
struct Baum {
    wurzel: TempDir,
    pfad: PathBuf,
}

impl Baum {
    fn path(&self) -> &Path {
        &self.pfad
    }

    fn wurzel(&self) -> &Path {
        self.wurzel.path()
    }
}

/// Der Baum `name`, etwa `2x1-se`, in einer neuen Wurzel.
fn neuer_baum(name: &str) -> Baum {
    let wurzel = tempdir();
    let pfad = wurzel.path().join(name);
    Baum { wurzel, pfad }
}

/// Der Ordner, den ein Lauf mit diesen Schaltern unter der Wurzel
/// beschreibt: `<kamera>-<richtung>` mit `x` statt `:`.
fn baum_name(extra: &[&str]) -> String {
    let wert = |schalter: &str| {
        extra
            .iter()
            .position(|&a| a == schalter)
            .map(|i| extra[i + 1].to_string())
    };
    // Gekürzt wie im Renderer: 8:6 schreibt nach 4x3-se.
    let kamera = wert("--camera").map_or(Kamera::ZWEI_ZU_EINS, |k| Kamera::parse(&k).unwrap());
    let richtung = wert("--direction")
        .unwrap_or_else(|| (if kamera.genordet() { "s" } else { "se" }).to_string());
    format!("{}-{richtung}", kamera.to_string().replace(':', "x"))
}

/// Ruft die Binärdatei auf.
fn cli(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .args(args)
        .output()
        .expect("terranova-render starten")
}

/// Kachelexport über die Binärdatei, genau mit diesen Schaltern, in den
/// Baum `out`: `--tiles` bekommt die Wurzel darüber.
fn export(welt: &Path, out: &Path, extra: &[&str]) -> Output {
    let wurzel = wurzel_von(out, extra);
    let mut args: Vec<&OsStr> = vec![
        OsStr::new("--world"),
        welt.as_ref(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--tiles"),
        wurzel.as_ref(),
    ];
    args.extend(extra.iter().map(OsStr::new));
    cli(&args)
}

/// Die Wurzel über dem Baum `out`; der Ordner muss zu Kamera und Richtung
/// in `extra` passen, sonst schriebe der Lauf woandershin.
fn wurzel_von<'a>(out: &'a Path, extra: &[&str]) -> &'a Path {
    let name = baum_name(extra);
    assert_eq!(
        out.file_name().and_then(|n| n.to_str()),
        Some(name.as_str()),
        "der Lauf schreibt nach {name}, der Test liest {}",
        out.display()
    );
    out.parent().expect("ein Baum liegt in einer Wurzel")
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

/// Alle Ausgabedateien mit Inhalt, `map.json` und die Höhen eingeschlossen.
fn schnappschuss(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    dateien(dir)
        .into_iter()
        .chain(std::iter::once("map.json".to_string()))
        .chain(hoehen(dir))
        .filter(|rel| dir.join(rel).is_file())
        .map(|rel| {
            let inhalt = std::fs::read(dir.join(&rel)).expect("Ausgabedatei lesen");
            (rel, inhalt)
        })
        .collect()
}

/// Wo die Höhen eines Baums liegen, relativ zu ihm: unter der Wurzel
/// `../heights`, in einem Baum der alten Ablage `heights`.
fn hoehen_ordner(dir: &Path) -> &'static str {
    if dir.join("heights").is_dir() {
        "heights"
    } else {
        "../heights"
    }
}

/// Die Dateien der Höhen als `../heights/<x>.<z>.bin`, in einem Baum der
/// alten Ablage `heights/<x>.<z>.bin`, sortiert.
fn hoehen(dir: &Path) -> Vec<String> {
    let ordner = hoehen_ordner(dir);
    let mut namen: Vec<String> = std::fs::read_dir(dir.join(ordner))
        .into_iter()
        .flatten()
        .flatten()
        .map(|eintrag| format!("{ordner}/{}", eintrag.file_name().to_string_lossy()))
        .collect();
    namen.sort();
    namen
}

/// Die Höhen der Region (rx, rz) des Baums.
fn hoehen_von(dir: &Path, rx: i32, rz: i32) -> Heights {
    let pfad = dir
        .join(hoehen_ordner(dir))
        .join(heights::path_of(rx, rz).trim_start_matches("heights/"));
    let daten = std::fs::read(&pfad).unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()));
    Heights::decode(&daten).unwrap()
}

/// Eine Kopie des Baums samt Höhen in einer neuen Wurzel.
fn kopie(dir: &Path) -> Baum {
    let ziel = neuer_baum(dir.file_name().unwrap().to_str().unwrap());
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

    let ganz = neuer_baum("2x1-se");
    let teil = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");

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
        let baum = neuer_baum("2x1-se");
        gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
        let vorher = dateien(baum.path());
        let ausgabe = tiles(neu.path(), baum.path(), &["--scale", scale, "--prune"]);
        let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
        // Angesagt wird vor der Basis: bis zum Ende bleibt Zeit für Strg+C.
        let ansage = text.find("Aufräumen:").expect("keine Ansage");
        assert!(ansage < text.find("Kacheln:").unwrap(), "{text}");
        let voll = neuer_baum("2x1-se");
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

    let baum = neuer_baum("2x1-se");
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
    let voll = neuer_baum("2x1-se");
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let baum = neuer_baum("2x1-se");
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
    let voll = neuer_baum("2x1-se");
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "12"]));
    let baum = neuer_baum("2x1-se");
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
    let baum = neuer_baum("2x1-se");
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
        let baum = neuer_baum("2x1-se");
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
    let baum = neuer_baum("2x1-se");
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
            let baum = neuer_baum("2x1-se");
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
/// zeigt aber schon ab dem Rendern nichts mehr: ein späterer Ausschnitt
/// nähme sonst ihren alten Inhalt in die Elternkachel. Nach einem Abbruch
/// ist deshalb jede leer gewordene Kachel entweder durchsichtig, weil schon
/// gerendert, oder unverändert alt, weil nicht erreicht. Der nächste Lauf
/// heilt beides. Der zweite Block bei (0, 4, 0) reicht in die Kacheln
/// (-1, -1) und (0, -1) der Basis und der Stufe darüber, der erste nicht,
/// auch wenn der Vorlauf sie nennt. Den Abbruch erzwingt ein Verzeichnis an
/// der Stelle einer Kachel des ersten Blocks zwei Stufen über der Basis.
/// Bei scale 16 sind beide Stufen darüber nativ, bei 12 verkleinert.
/// Auf einem Thread prüft der Test die strenge Form, alle durchsichtig:
/// Dort ist bis zum Abbruch alles darunter erreicht, denn die Basis rendert
/// (0, 0) zuletzt, und die vier Kacheln der gröbsten nativen Stufe liegen
/// in einem Band, das die feinere Stufe vor der gröberen rendert. Auf vier
/// Threads hat jeder sein eigenes Stück, bei scale 16 ein Band je Kachel.
/// Bricht einer ab, bevor ein anderer begonnen hat, fängt der nicht mehr an
/// (`verteile`), und dessen Kacheln bleiben unverändert alt.
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
        let voll = neuer_baum("2x1-se");
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        for threads in [1, 4] {
            let fall = format!("scale {scale}, {threads} Threads");
            let baum = neuer_baum("2x1-se");
            gelungen(&tiles(alt.path(), baum.path(), &["--scale", scale]));
            let z = max_zoom(baum.path());
            let stufen = [z, z - 1];
            let mut vorher = BTreeMap::new();
            for (stufe, tile) in stufen.iter().flat_map(|&s| leer_geworden.map(|t| (s, t))) {
                let pfad = kacheln(baum.path(), stufe)[&tile].clone();
                assert!(
                    bild(&pfad).pixels().any(|p| p.0[3] > 0),
                    "{fall}: {stufe} {tile:?}"
                );
                assert!(!kacheln(voll.path(), stufe).contains_key(&tile), "{fall}");
                vorher.insert((stufe, tile), (std::fs::read(&pfad).unwrap(), pfad));
            }
            let sperre = baum.path().join(format!("{}/0/0.webp", z - 2));
            std::fs::remove_file(&sperre).unwrap();
            std::fs::create_dir(&sperre).unwrap();
            let schalter = ["--native-levels", "9", "--scale", scale];
            let ausgabe = export_auf(threads, neu.path(), baum.path(), &schalter);
            assert!(!ausgabe.status.success(), "{fall}: kein Abbruch");
            for ((stufe, tile), (bytes, pfad)) in &vorher {
                let durchsichtig = bild(pfad).pixels().all(|p| p.0[3] == 0);
                let unberuehrt = threads > 1 && std::fs::read(pfad).unwrap() == *bytes;
                assert!(
                    durchsichtig || unberuehrt,
                    "{fall}: {stufe} {tile:?} zeigt noch den alten Inhalt"
                );
            }
            std::fs::remove_dir(&sperre).unwrap();
            gelungen(&tiles(neu.path(), baum.path(), &["--scale", scale]));
            assert_eq!(
                schnappschuss(baum.path()),
                schnappschuss(voll.path()),
                "{fall}"
            );
        }
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
    let voll = neuer_baum("2x1-se");
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));
    let baum = neuer_baum("2x1-se");
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
        let voll = neuer_baum("2x1-se");
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        let baum = neuer_baum("2x1-se");
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
    let voll = neuer_baum("2x1-se");
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let baum = neuer_baum("2x1-se");
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
        let voll = neuer_baum("2x1-se");
        gelungen(&tiles(neu.path(), voll.path(), &["--scale", scale]));
        let baum = neuer_baum("2x1-se");
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

    let out = neuer_baum("2x1-se");
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "16", "--center", "8", "8", "--size", "4"],
    ));

    // Gegenprobe: der Vollexport braucht das fehlende Asset sehr wohl, und
    // muss das auch sagen.
    let alles = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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

/// Was `--pyramid` aus der Basis dieses Baums, seiner `map.json` und
/// seinen Höhen von Grund auf baut.
fn von_grund_auf(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    von_der_platte(dir, max_zoom(dir))
}

/// Was `--pyramid` über den Stufen ab `ab` dieses Baums baut, samt seiner
/// `map.json` und seinen Höhen. Jede Stufe ist eine Minute jünger als die
/// darunter, so nimmt es native Stufen, wie sie sind.
fn von_der_platte(dir: &Path, ab: u32) -> BTreeMap<String, Vec<u8>> {
    let basis = max_zoom(dir);
    // Ein Baum mit eigener Wurzel: die Höhen liegen über ihm.
    let frisch = neuer_baum(dir.file_name().unwrap().to_str().unwrap());
    let damals = SystemTime::now() - Duration::from_secs(3600);
    for (rel, inhalt) in schnappschuss(dir) {
        let stufe = rel.split_once('/').and_then(|(z, _)| z.parse::<u32>().ok());
        if stufe.is_some_and(|z| z < ab) {
            continue;
        }
        let pfad = frisch.path().join(&rel);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(&pfad, inhalt).unwrap();
        if let Some(z) = stufe {
            setze_zeit(
                &pfad,
                damals + Duration::from_secs(60 * u64::from(basis - z)),
            );
        }
    }
    gelungen(&pyramide(frisch.path()));
    schnappschuss(frisch.path())
}

/// Wie [`export`], auf so vielen Threads.
fn export_auf(threads: usize, welt: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .arg("--world")
        .arg(welt)
        .arg("--assets")
        .arg(assets_ref())
        .arg("--tiles")
        .arg(wurzel_von(out, extra))
        .args(extra)
        .env("RAYON_NUM_THREADS", threads.to_string())
        .output()
        .expect("terranova-render starten")
}

/// Eine Welt mit so vielen Basiskacheln bei scale 32, dass ein Thread sie
/// in Streifen von vier Spalten rendert und die feinen Stufen der
/// Pyramide im Speicher entstehen.
fn weite_welt() -> TempDir {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (0..6).flat_map(|x| (0..6).map(move |z| (x, z))).collect();
    common::write_world(welt.path(), &chunks, gelaende);
    welt
}

/// So meldet ein Export, dass Eltern im Speicher entstanden sind.
const IM_SPEICHER: &str = "davon schon während des Renderns";

/// Die Basis dieser Welt bei scale 32 in der Reihenfolge, in der `rendere`
/// sie auf so vielen Threads verteilt: in Streifen so breit, wie
/// `breite_der_streifen` in `cli.rs` sie schneidet, jeder Zeile für Zeile.
fn reihe_wie_gerendert(welt: &Path, threads: usize) -> Vec<TileId> {
    let world = World::open(welt).unwrap();
    let mut reihe = survey(&world, Projection::new(32), (-64, 319), None)
        .unwrap()
        .tiles;
    let je_thread = (reihe.len() / threads) as f64;
    let stufen = (je_thread / 10.0).sqrt().log2().round().max(0.0) as u32;
    let breite = (1 << stufen).min(streifenbreite(32)) as i32;
    reihe.sort_by_key(|tile| (tile.x.div_euclid(breite), tile.y, tile.x));
    reihe
}

/// Die feinen Stufen entstehen im Speicher, während die Basis rendert, und
/// sind Byte für Byte, was `--pyramid` von der Platte baut: auf einem
/// Thread; auf dreien, deren Stücke Eltern zerschneiden; über der
/// gröbsten nativen Stufe; und für einen Ausschnitt in einem bestehenden
/// Baum, an dessen Rand die Eltern von der Platte kommen. Mit zwei
/// nativen Stufen ist die gröbste zu klein für Streifen, und nichts
/// entsteht im Speicher, auch nicht über der feineren.
#[test]
fn feine_stufen_im_speicher_wie_von_der_platte() {
    let welt = weite_welt();
    // Drei Threads teilen die Reihe in Drittel (`verteile`), und am ersten
    // Schnitt liegen Geschwister auf beiden Seiten. Das hängt nicht am
    // Stehlen. Bei zweien fiele er auf den Rand eines Streifens.
    let reihe = reihe_wie_gerendert(welt.path(), 3);
    let schnitt = reihe.len() / 3;
    assert_eq!(
        reihe[schnitt - 1].parent(),
        reihe[schnitt].parent(),
        "der erste Schnitt zwischen drei Threads trennt keine Geschwister"
    );
    let ganz = ["--scale", "32", "--native-levels", "0"];
    let nativ = ["--scale", "32", "--native-levels", "1"];
    let zwei = ["--scale", "32", "--native-levels", "2"];
    for (threads, args, stufen) in [(1, &ganz, 0), (3, &ganz, 0), (1, &nativ, 1), (1, &zwei, 2)] {
        let fall = format!("{threads} Threads, {args:?}");
        let out = neuer_baum("2x1-se");
        let ausgabe = export_auf(threads, welt.path(), out.path(), args);
        let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
        assert_eq!(
            meldung.contains(IM_SPEICHER),
            stufen < 2,
            "{fall}: {meldung}"
        );
        let ab = max_zoom(out.path()) - stufen;
        assert!(ab > 1, "{fall}: keine Pyramide zu prüfen");
        // Je Stufe zählt jede Kachel einmal, ob aus dem Speicher oder von
        // der Platte.
        for z in 0..ab {
            let zeile = format!(
                "Zoom {z:>2}:     {} Kacheln\n",
                kacheln(out.path(), z).len()
            );
            assert!(meldung.contains(&zeile), "{fall}: {zeile}{meldung}");
        }
        assert_eq!(
            schnappschuss(out.path()),
            von_der_platte(out.path(), ab),
            "{fall}"
        );
    }

    let out = neuer_baum("2x1-se");
    gelungen(&export_auf(1, welt.path(), out.path(), &ganz));
    let soll = schnappschuss(out.path());
    let ausschnitt = [&ganz[..], &["--center", "48", "48", "--size", "1536"]].concat();
    let ausgabe = export_auf(1, welt.path(), out.path(), &ausschnitt);
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(meldung.contains(IM_SPEICHER), "Ausschnitt: {meldung}");
    assert_eq!(schnappschuss(out.path()), soll, "Ausschnitt");
}

/// Die nativen Stufen laufen in Bändern und teilen Chunks und Licht über
/// die Stufen (`render_coarser`). Jede ihrer Kacheln ist trotzdem Byte für
/// Byte, was der Weg je Stufe zeichnet: ein Cache je scale ohne Vorrat, mit
/// der Tabelle des scale und dem Licht der Basis. Mit drei Stufen auf
/// einem Thread und mit Karte, mit zwei auf drei Threads und mit einer.
/// Die Welt ist ein Streifen aus der Szene aus `common::szene` entlang
/// einer Spalte der Kacheln, mit Licht, Wasser, Lava und zwei Biomen, dazu
/// Ackerboden, dessen Raster bei scale 4 kippt, siehe
/// `licht_unbekannter_bloecke_haengt_nicht_am_scale` in `tests/licht.rs`.
/// Bei scale 4 reicht sie für drei Bänder, bei 8 für mehr: Dann fällt auch,
/// was ein Band nicht mehr braucht, aus dem Vorrat. Eine einzelne Stufe
/// läuft ohne Bänder, und das Log nennt keine.
#[test]
fn native_stufen_wie_der_weg_je_stufe() {
    let welt = tempdir();
    let chunks: Vec<(i32, i32)> = (1..31)
        .flat_map(|x| (x - 1..=x + 1).map(move |z| (x, z)))
        .collect();
    let szene = |x: i32, y: i32, z: i32| common::szene(x.rem_euclid(32), y, z.rem_euclid(32));
    let biom = |cx: i32, _: i32| {
        Some(if cx.rem_euclid(2) == 0 {
            "minecraft:plains"
        } else {
            "minecraft:frozen"
        })
    };
    common::write_world_sections(welt.path(), &chunks, -1..=2, szene, biom);

    // Der Weg je Stufe, wie die Binärdatei ihn ohne Bänder ginge.
    let world = World::open(welt.path()).unwrap();
    let survey = survey(&world, Projection::new(32), (-64, 319), None).unwrap();
    let mut assets = Assets::open(vec![assets()]).unwrap();
    let basis = SpriteSet::build_in(&mut assets, &survey.states, Projection::new(32)).unwrap();
    let deckend = basis.licht_deckend(&survey.states);
    let biomes = BiomeTable::new(assets.colors()).with(BLEND_DEFAULT, world.seed().unwrap());
    let tabellen = BTreeMap::from([16, 8, 4].map(|scale| {
        let mut sprites = SpriteSet::build_mit_licht(
            &mut assets,
            &survey.states,
            Projection::new(scale),
            Some(deckend.clone()),
        )
        .unwrap();
        sprites.add_entities(&mut assets, &survey.entities).unwrap();
        sprites.set_biomes(biomes.clone());
        (scale, sprites)
    }));
    let mut caches: BTreeMap<u32, ChunkCache> = tabellen
        .iter()
        .map(|(scale, sprites)| (*scale, ChunkCache::new(&world, sprites)))
        .collect();
    let mut soll: BTreeMap<(u32, TileId), RgbaImage> = BTreeMap::new();

    let faelle = [
        (1, 32, 3, "off"),
        (3, 32, 3, "on"),
        (3, 32, 2, "off"),
        (1, 8, 1, "off"),
    ];
    for (threads, scale, stufen, gpu) in faelle {
        let fall = format!("{threads} Threads, scale {scale}, {stufen} Stufen, --gpu {gpu}");
        let out = neuer_baum("2x1-se");
        let args = [
            "--scale",
            &scale.to_string(),
            "--native-levels",
            &stufen.to_string(),
            "--gpu",
            gpu,
        ];
        let lauf = export_auf(threads, welt.path(), out.path(), &args);
        if gpu == "on" {
            if !lauf.status.success()
                && String::from_utf8_lossy(&lauf.stderr).contains("keine Grafikkarte gefunden")
            {
                common::ohne_gpu();
                continue;
            }
            assert_eq!(ganz_auf_der_karte(&lauf), stufen as usize, "{fall}");
        }
        gelungen(&lauf);
        // Bänder nennt das Log erst ab zwei Stufen.
        let ausgabe = String::from_utf8_lossy(&lauf.stdout);
        assert_eq!(
            ausgabe.contains("in Bändern aus"),
            stufen > 1,
            "{fall}:\n{ausgabe}"
        );
        let oben = max_zoom(out.path());
        // Drei Bänder aus vier Kacheln bei scale 4, mehr bei 8.
        let mindestens = [1, 13, 9][stufen as usize - 1];
        let grob = kacheln(out.path(), oben - stufen).len();
        assert!(
            grob >= mindestens,
            "{fall}: {grob} Kacheln auf der gröbsten Stufe"
        );
        for k in 1..=stufen {
            let s = scale >> k;
            let ist = kacheln(out.path(), oben - k);
            assert!(!ist.is_empty(), "{fall}: scale {s} ohne Kacheln");
            for (tile, pfad) in ist {
                let soll = soll.entry((s, tile)).or_insert_with(|| {
                    render_area_with(caches.get_mut(&s).unwrap(), tile.rect(), (-64, 319)).unwrap()
                });
                assert!(
                    bild(&pfad).as_raw() == soll.as_raw(),
                    "{fall}: scale {s}, {tile:?} ist nicht, was der Weg je Stufe zeichnet"
                );
            }
        }
    }
}

/// Bricht ein Export mitten in der Basis ab, steht über ihr keine
/// Elternkachel, der ein Kind fehlt: Was im Speicher entstand, ist schon
/// die Kachel des ganzen Laufs. Ein `--resume` danach baut denselben Baum
/// wie ein Lauf in einem Stück, die Eltern über neu gerenderten Kindern im
/// Speicher, die über stehen gebliebenen von der Platte. Den Abbruch
/// erzwingt ein Verzeichnis an der Stelle einer Basiskachel.
#[test]
fn abgebrochener_export_setzt_sich_fort_wie_in_einem_stueck() {
    let welt = weite_welt();
    let args = ["--scale", "32", "--native-levels", "0"];
    let ganz = neuer_baum("2x1-se");
    gelungen(&export_auf(1, welt.path(), ganz.path(), &args));
    let soll = schnappschuss(ganz.path());
    let basis = max_zoom(ganz.path());

    // In der Reihenfolge, in der ein Thread die Basis in Streifen von vier
    // Spalten rendert.
    let mut reihe: Vec<TileId> = kacheln(ganz.path(), basis).into_keys().collect();
    reihe.sort_by_key(|tile| (tile.x.div_euclid(4), tile.y, tile.x));
    let out = neuer_baum("2x1-se");
    let sperre = kachel_pfad(out.path(), basis, reihe[reihe.len() / 2]);
    std::fs::create_dir_all(&sperre).unwrap();
    let ausgabe = export_auf(1, welt.path(), out.path(), &args);
    assert!(!ausgabe.status.success(), "kein Abbruch");
    std::fs::remove_dir(&sperre).unwrap();

    let mut eltern = 0;
    for z in 0..basis {
        for (tile, pfad) in kacheln(out.path(), z) {
            let rel = format!("{z}/{}/{}.webp", tile.x, tile.y);
            assert_eq!(
                std::fs::read(&pfad).ok(),
                soll.get(&rel).cloned(),
                "{rel} ist nicht die Kachel des ganzen Laufs"
            );
            for kind in tile.children() {
                let rel = format!("{}/{}/{}.webp", z + 1, kind.x, kind.y);
                assert!(
                    !soll.contains_key(&rel) || out.path().join(&rel).is_file(),
                    "{rel} fehlt unter einer Elternkachel"
                );
            }
            eltern += 1;
        }
    }
    assert!(eltern > 0, "vor dem Abbruch entstand keine Elternkachel");

    // Alles ist alt, bis auf die letzte Basiskachel vor dem Abbruch: Nur
    // sie und die fehlenden rendert das Fortsetzen neu.
    let damals = SystemTime::now() - Duration::from_secs(3600);
    for z in 0..=basis {
        for pfad in kacheln(out.path(), z).values() {
            setze_zeit(pfad, damals);
        }
    }
    let zuletzt = reihe[..reihe.len() / 2]
        .iter()
        .rev()
        .map(|tile| kachel_pfad(out.path(), basis, *tile))
        .find(|pfad| pfad.is_file())
        .expect("eine Basiskachel vor dem Abbruch");
    setze_zeit(&zuletzt, damals + Duration::from_secs(600));
    let fortsetzen = [&args[..], &["--resume"]].concat();
    let ausgabe = export_auf(1, welt.path(), out.path(), &fortsetzen);
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(meldung.contains(IM_SPEICHER), "{meldung}");
    assert!(
        meldung.contains("vorhandene Kacheln übersprungen"),
        "{meldung}"
    );
    assert_eq!(schnappschuss(out.path()), soll);
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
    let baum = neuer_baum("2x1-se");
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

    let neu = neuer_baum("2x1-se");
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
    let zwoelf = neuer_baum("2x1-se");
    gelungen(&export(welt.path(), zwoelf.path(), &["--scale", "12"]));
    let karte = zwoelf.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info.as_object_mut().unwrap().remove("nativeLevels");
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    gelungen(&export(welt.path(), zwoelf.path(), &["--scale", "12"]));
    assert_eq!(native_in(zwoelf.path()), Some(0));
}

/// Der Radius der Mischung gehört zum Baum wie die nativen Stufen: Ein
/// Nachrendern ohne `--biome-blend` nimmt ihn aus `map.json` und ändert an
/// einer unveränderten Welt keine Datei, eines mit einem anderen bricht ab,
/// bevor es etwas schreibt, und `--pyramid` behält ihn. Ein neuer Baum
/// bekommt die Vorgabe 2. Einer aus einem älteren Stand ohne das Feld nimmt
/// den Schalter oder die Vorgabe, sagt es und trägt den Radius ein.
#[test]
fn mischung_gehoert_zum_baum() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let baum = neuer_baum("2x1-se");
    gelungen(&export(
        welt.path(),
        baum.path(),
        &["--scale", "16", "--biome-blend", "3"],
    ));
    assert_eq!(mischung_in(baum.path()), Some(3));
    let vorher = schnappschuss(baum.path());

    let ausschnitt = ["--scale", "16", "--center", "8", "8", "--size", "4"];
    gelungen(&export(welt.path(), baum.path(), &ausschnitt));
    assert!(
        schnappschuss(baum.path()) == vorher,
        "ohne Schalter anders gemischt"
    );

    let anders = [&ausschnitt[..], &["--biome-blend", "1"]].concat();
    let ausgabe = export(welt.path(), baum.path(), &anders);
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        !ausgabe.status.success() && meldung.contains("Mit --biome-blend 3 weiterrendern"),
        "{meldung}"
    );
    assert!(schnappschuss(baum.path()) == vorher);

    gelungen(&cli(&[OsStr::new("--pyramid"), baum.path().as_os_str()]));
    assert_eq!(mischung_in(baum.path()), Some(3), "--pyramid");

    let neu = neuer_baum("2x1-se");
    gelungen(&export(welt.path(), neu.path(), &["--scale", "16"]));
    assert_eq!(mischung_in(neu.path()), Some(2));

    let karte = neu.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info.as_object_mut().unwrap().remove("biomeBlend");
    std::fs::write(&karte, serde_json::to_string_pretty(&info).unwrap()).unwrap();
    let ausgabe = export(
        welt.path(),
        neu.path(),
        &["--scale", "16", "--biome-blend", "0"],
    );
    let meldung = String::from_utf8_lossy(&gelungen(&ausgabe).stdout);
    assert!(
        meldung.contains("nennt keinen Radius der Mischung"),
        "{meldung}"
    );
    assert_eq!(mischung_in(neu.path()), Some(0));
}

/// Der Radius der Mischung, wie `map.json` ihn nennt.
fn mischung_in(dir: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(dir.join("map.json")).expect("map.json lesen");
    let info: serde_json::Value = serde_json::from_str(&text).expect("map.json auswerten");
    info["biomeBlend"].as_u64()
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
    let out = neuer_baum("2x1-se");
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
        // Mit Karte steht dahinter noch " + GPU".
        format!("{} Kacheln nativ bei scale 4", nativ.len()),
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
        .arg(out.wurzel())
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
/// schon `trees.json`. Für einen Ordner, in dem schon anderes liegt, gibt es
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));

    let text = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    let info: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(info["tileSize"], 256);
    assert_eq!(info["minZoom"], 0);
    assert_eq!(info["scale"], 8);
    assert_eq!(info["tiles"], "{z}/{x}/{y}.webp");
    assert_eq!(info["camera"], "2:1");
    assert_eq!(info["direction"], "se");
    assert_eq!(
        info["projection"],
        serde_json::json!({"azimuth": "diagonal", "u": 4, "v": 2, "y": 4})
    );

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

/// Ein Export schreibt je Region die Höhen und nennt sie in `map.json`,
/// samt Zellgrösse und dem Höhenbereich, den der Renderer zeichnet. Je 4×4
/// Spalten zählt der obere Median der obersten Blöcke, die nicht Luft sind,
/// Wasser und Truhen also mit; eine Zelle ohne Block und ein Chunk, den es
/// nicht gibt, sind leer.
#[test]
fn export_schreibt_hoehen() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 0..=4, 8) => "minecraft:einfarbig",
        (8, 5..=9, 8) => "minecraft:water",
        (9, 7, 8) | (13, 7, 8) => "minecraft:chest",
        (0, 3..=6, 0) => "minecraft:water",
        _ => "minecraft:air",
    });
    let out = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));

    let text = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    let info: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(info["heights"], "../heights/{x}.{z}.bin");
    assert_eq!(info["heightsCell"], 4);
    assert_eq!(info["minY"], -64);
    assert_eq!(info["maxY"], 319);
    assert_eq!(hoehen(out.path()), ["../heights/0.0.bin"]);
    let hoehe = hoehen_von(out.path(), 0, 0);
    assert_eq!(hoehe.get(2, 2), 9, "Wasser 9 und Truhe 7, der obere Median");
    assert_eq!(hoehe.get(3, 2), 7, "nur eine Truhe");
    assert_eq!(hoehe.get(0, 0), 6, "nur Wasser");
    assert_eq!(hoehe.get(1, 1), EMPTY, "ohne Block");
    assert_eq!(hoehe.get(10, 10), EMPTY, "Chunk (2, 2) fehlt");
}

/// Der Export sagt, wie viele Chunks er übergeht, weil sie nicht fertig
/// erzeugt sind, und `--heights` ebenso. `--at` nennt ihren Status und dass
/// der Renderer sie nicht zeichnet, statt sie wie fehlende „nicht generiert“
/// zu nennen. `--scan` dekodiert beide, zählt den unfertigen und sammelt nur
/// die Blockstates des fertigen: Luft und `einfarbig`, nicht `mit_overlay`.
#[test]
fn unfertige_chunks_nennt_der_lauf() {
    let welt = tempdir();
    common::write_world_status(
        welt.path(),
        &[(0, 0), (1, 0)],
        0..=0,
        |x, y, _| match (x, y) {
            (_, 1..) => "minecraft:air",
            (..16, _) => "minecraft:einfarbig",
            _ => "minecraft:mit_overlay",
        },
        |cx, _| {
            if cx == 1 {
                "minecraft:carvers"
            } else {
                common::FULL
            }
        },
    );
    let out = neuer_baum("2x1-se");
    let export = tiles(welt.path(), out.path(), &["--scale", "16"]);
    let text = String::from_utf8_lossy(&gelungen(&export).stdout);
    assert!(
        text.contains("1 Chunks nicht fertig erzeugt, nicht gezeichnet"),
        "{text}"
    );
    let hoehen = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--heights"),
        out.path().as_os_str(),
    ]);
    let hoehen = String::from_utf8_lossy(&gelungen(&hoehen).stdout).into_owned();
    assert!(
        hoehen.contains("1 Chunks nicht fertig erzeugt, nicht gezeichnet"),
        "{hoehen}"
    );

    let at = |x: &str| {
        let mut args = vec![OsStr::new("--world"), welt.path().as_os_str()];
        args.extend(["--at", x, "0", "8"].map(OsStr::new));
        String::from_utf8_lossy(&gelungen(&cli(&args)).stdout).into_owned()
    };
    let unfertig = at("24");
    assert!(unfertig.contains("status=minecraft:carvers"), "{unfertig}");
    assert!(unfertig.contains("nicht fertig erzeugt"), "{unfertig}");
    let fertig = at("8");
    assert!(fertig.contains("status=minecraft:full"), "{fertig}");
    assert!(!fertig.contains("nicht fertig erzeugt"), "{fertig}");

    let scan = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--scan"),
    ]);
    let scan = String::from_utf8_lossy(&gelungen(&scan).stdout).into_owned();
    assert!(scan.contains("Scan:       2 Chunks"), "{scan}");
    assert!(scan.contains("davon 1 nicht fertig erzeugt"), "{scan}");
    assert!(scan.contains("2 verschiedene Blockstates"), "{scan}");
}

/// Ein Ausschnitt schreibt die Höhen der Chunks neu, die er liest: die im
/// schrägen Band seiner Kacheln, auch Chunk (4, 4), dessen Block weit unter
/// dem Ausschnitt landet. Chunk (20, 0) liegt ausserhalb des Bands und
/// behält seine. Zwischen den Läufen sind alle Blöcke höher gestiegen.
#[test]
fn ausschnitt_behaelt_die_hoehen_daneben() {
    let welt = |oben: i32| {
        let dir = tempdir();
        let chunks = [(0, 0), (4, 4), (20, 0)];
        common::write_world(dir.path(), &chunks, move |x, y, z| match (x, z) {
            (8, 8) if y == oben => "minecraft:einfarbig",
            (72, 72) | (328, 8) if y == oben => "minecraft:blauwuerfel",
            _ => "minecraft:air",
        });
        dir
    };
    let (alt, neu) = (welt(4), welt(9));
    let schalter = ["--scale", "16", "--native-levels", "0"];
    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(alt.path(), baum.path(), &schalter));
    assert_eq!(hoehen_von(baum.path(), 0, 0).get(82, 2), 4);

    let ausschnitt = [&schalter[..], &["--center", "8", "8", "--size", "4"]].concat();
    gelungen(&tiles(neu.path(), baum.path(), &ausschnitt));
    let hoehe = hoehen_von(baum.path(), 0, 0);
    assert_eq!(hoehe.get(2, 2), 9, "im Ausschnitt neu");
    assert_eq!(hoehe.get(18, 18), 9, "im Band neu");
    assert_eq!(hoehe.get(82, 2), 4, "ausserhalb wie vorher");
}

/// `--heights` schreibt in einen Baum ohne Höhen, etwa aus einem älteren
/// Stand, dieselben Höhen und Felder wie ein Export, ohne eine Kachel
/// anzufassen und ohne Assets. Wie ein Export nur in einen Baum dieser
/// Welt, und nur in einen Baum. Auch in einen Baum einer anderen Kamera:
/// Scale und Kamera kommen aus seiner `map.json`.
#[test]
fn heights_traegt_hoehen_nach() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    common::write_wurzel(welt.path(), 4_815_162_342);
    let nachtragen = |welt: &Path, dir: &Path| {
        cli(&[
            OsStr::new("--world"),
            welt.as_os_str(),
            OsStr::new("--heights"),
            dir.as_os_str(),
        ])
    };
    let ohne_hoehen = |dir: &Path| {
        std::fs::remove_dir_all(dir.join(hoehen_ordner(dir))).unwrap();
        let karte = dir.join("map.json");
        let mut info: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
        for feld in ["heights", "heightsCell", "minY", "maxY"] {
            info.as_object_mut().unwrap().remove(feld).expect(feld);
        }
        std::fs::write(&karte, serde_json::to_vec_pretty(&info).unwrap()).unwrap();
    };
    for kamera in ["top", "north-45"] {
        let anders = neuer_baum(&baum_name(&["--camera", kamera]));
        gelungen(&tiles(
            welt.path(),
            anders.path(),
            &["--scale", "16", "--camera", kamera],
        ));
        let soll = schnappschuss(anders.path());
        ohne_hoehen(anders.path());
        gelungen(&nachtragen(welt.path(), anders.path()));
        assert_eq!(schnappschuss(anders.path()), soll, "{kamera}");
    }

    let out = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));
    let soll = schnappschuss(out.path());
    ohne_hoehen(out.path());
    gelungen(&nachtragen(welt.path(), out.path()));
    assert_eq!(schnappschuss(out.path()), soll);

    // Auch mit `.` im Ordner des Baums landen die Höhen unter der Wurzel.
    ohne_hoehen(out.path());
    let ausgabe = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .current_dir(out.path())
        .arg("--world")
        .arg(welt.path())
        .args(["--heights", "."])
        .output()
        .expect("terranova-render starten");
    gelungen(&ausgabe);
    assert!(!out.path().join("heights").exists(), "Höhen im Baum");
    assert_eq!(schnappschuss(out.path()), soll, "mit .");

    // Ein Baum der alten Ablage, ohne Wurzel darüber, bekommt sie in sich
    // selbst.
    let alt = tempdir();
    let allein = alt.path().join("karte");
    for (rel, inhalt) in schnappschuss(out.path()) {
        if rel.starts_with("../") {
            continue;
        }
        let pfad = allein.join(rel);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(pfad, inhalt).unwrap();
    }
    let karte = allein.join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    for feld in ["heights", "heightsCell", "minY", "maxY"] {
        info.as_object_mut().unwrap().remove(feld).expect(feld);
    }
    std::fs::write(&karte, serde_json::to_vec_pretty(&info).unwrap()).unwrap();
    gelungen(&nachtragen(welt.path(), &allein));
    let info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    assert_eq!(info["heights"], "heights/{x}.{z}.bin");
    assert!(allein.join("heights/0.0.bin").is_file(), "Höhen fehlen");
    assert!(!alt.path().join("heights").exists(), "Höhen neben dem Baum");

    let fremd = tempdir();
    common::write_world(fremd.path(), &[(0, 0)], gelaende);
    common::write_wurzel(fremd.path(), 2_718_281_828);
    let ausgabe = nachtragen(fremd.path(), out.path());
    assert!(!ausgabe.status.success(), "die fremde Welt lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("anderen Welt"), "Meldung: {meldung}");
    assert_eq!(schnappschuss(out.path()), soll);

    let leer = tempdir();
    let ausgabe = nachtragen(welt.path(), leer.path());
    assert!(!ausgabe.status.success(), "ohne Baum lief es durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("nur einen Baum"), "Meldung: {meldung}");
    assert!(schnappschuss(leer.path()).is_empty(), "etwas geschrieben");
}

/// `--heights` braucht die Welt und schreibt nicht neben einem Export in
/// dasselbe Verzeichnis.
#[test]
fn heights_braucht_die_welt() {
    let out = neuer_baum("2x1-se");
    let ausgabe = cli(&[OsStr::new("--heights"), out.path().as_os_str()]);
    assert!(!ausgabe.status.success());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("--heights braucht --world"), "{meldung}");

    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let pfad = out.path().to_string_lossy().into_owned();
    let ausgabe = export(welt.path(), out.path(), &["--heights", &pfad]);
    assert!(
        !ausgabe.status.success(),
        "--heights neben --tiles lief durch"
    );
    assert!(schnappschuss(out.path()).is_empty(), "etwas geschrieben");

    // Scale, Kamera und Richtung kommen aus map.json; wer sie nennt, irrt
    // sich.
    for schalter in [
        ["--camera", "4:3"],
        ["--scale", "16"],
        ["--direction", "se"],
    ] {
        let ausgabe = cli(&[
            OsStr::new("--world"),
            welt.path().as_os_str(),
            OsStr::new("--heights"),
            out.path().as_os_str(),
            OsStr::new(schalter[0]),
            OsStr::new(schalter[1]),
        ]);
        assert!(!ausgabe.status.success(), "{schalter:?} lief durch");
        let meldung = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(
            meldung.contains("cannot be used with"),
            "{schalter:?}: {meldung}"
        );
    }
}

/// Ein Paar aus Kamera und scale ohne ganze Pixel bricht ab, bevor der Lauf
/// die Welt liest oder etwas schreibt, und nennt die Nachbarn.
#[test]
fn kamera_ohne_ganze_pixel_bricht_vor_der_welt_ab() {
    let leer = tempdir();
    let welt = leer.path().join("fehlt");
    let out = leer.path().join("out");
    let ausgabe = cli(&[
        OsStr::new("--world"),
        welt.as_os_str(),
        OsStr::new("--tiles"),
        out.as_os_str(),
        OsStr::new("--camera"),
        OsStr::new("5:3"),
    ]);
    assert!(!ausgabe.status.success());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains("5:3 geht bei scale 32 nicht (a = 9,6). Nächste gültige: 16:9 (a = 9)"),
        "{meldung}"
    );
    assert!(!out.exists(), "out angelegt");
}

/// Die Höhen einer Region, deren Datei fehlt, entfernt nur --prune, wie die
/// Kacheln ohne Chunk, angesagt vor der ersten Kachel und entfernt am Ende
/// des Laufs, und nur, wenn der Lauf die Region läse. Die übrigen Höhen
/// gleichen danach denen eines frischen Exports.
#[test]
fn prune_entfernt_die_hoehen_ohne_regionsdatei() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (-8, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0)], block);
    common::write_world(alt.path(), &[(-1, 0)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);
    let voll = neuer_baum("2x1-se");
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let baum = neuer_baum("2x1-se");
    let beide = ["../heights/-1.0.bin", "../heights/0.0.bin"];
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    assert_eq!(hoehen(baum.path()), beide);
    assert_eq!(hoehen_von(baum.path(), -1, 0).get(126, 2), 4);
    gelungen(&tiles(neu.path(), baum.path(), &["--scale", "16"]));
    assert_eq!(hoehen(baum.path()), beide, "ohne --prune entfernt");
    // Ein Ausschnitt weit rechts läse nichts aus der Region links und lässt
    // ihre Höhen stehen, auch mit --prune.
    let rechts = [
        "--scale", "16", "--center", "300", "8", "--size", "4", "--prune",
    ];
    let ausgabe = tiles(neu.path(), baum.path(), &rechts);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(!text.contains("Höhen ohne Regionsdatei"), "{text}");
    assert_eq!(hoehen(baum.path()), beide, "vom Ausschnitt entfernt");

    let ausgabe = tiles(neu.path(), baum.path(), &["--scale", "16", "--prune"]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    let ansage = text
        .find("Aufräumen:  Höhen ohne Regionsdatei: 1; sie verschwinden am Ende des Laufs")
        .unwrap_or_else(|| panic!("keine Ansage: {text}"));
    assert!(ansage < text.find("Kacheln:").unwrap(), "{text}");
    assert!(
        text.contains("Aufräumen:  Höhen ohne Regionsdatei entfernt: 1"),
        "{text}"
    );
    assert_eq!(hoehen(baum.path()), ["../heights/0.0.bin"]);
    assert_eq!(hoehen_von(baum.path(), 0, 0), hoehen_von(voll.path(), 0, 0));
}

/// Die Zoomnummer hängt an der Welt, nicht am Massstab des Laufs — aber
/// die Zahl der Stufen sehr wohl.
#[test]
fn zoomstufen_haengen_am_massstab() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);

    let fein = neuer_baum("2x1-se");
    let grob = neuer_baum("2x1-se");
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
        let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
        let baum = neuer_baum("2x1-se");
        gelungen(&tiles(alt.path(), baum.path(), &schalter));
        let ausschnitt = [&schalter[..], &["--center", "44", "8", "--size", "4"]].concat();
        gelungen(&tiles(neu.path(), baum.path(), &ausschnitt));
        let voll = neuer_baum("2x1-se");
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
    let baum = neuer_baum("2x1-se");
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
    let frisch = neuer_baum("2x1-se");
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

/// Kacheln, Höhen und `map.json` werden getauscht, nicht überschrieben: wer
/// eine Datei gerade liest, liest sie zu Ende, wie sie war, und ein Abbruch
/// mitten im Schreiben hinterlässt die alte. Der Test hält die Basis, die
/// Höhen und `map.json` offen, während ein zweiter Lauf eine veränderte,
/// grössere Welt schreibt. Daneben bleibt keine eigene Datei übrig.
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
    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let karte = baum.path().join("map.json");
    let region = baum.wurzel().join(heights::path_of(0, 0));
    let liste = baum.wurzel().join("trees.json");
    let offen: Vec<(PathBuf, Vec<u8>, std::fs::File)> = kacheln(baum.path(), max_zoom(baum.path()))
        .into_values()
        .chain([karte.clone(), region.clone(), liste])
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
        geaendert.contains(&karte) && geaendert.contains(&region) && geaendert.len() > 2,
        "map.json, die Höhen und eine Kachel hätten sich ändern müssen: {geaendert:?}"
    );

    let mut reste = Vec::new();
    let mut stapel = vec![baum.wurzel().to_path_buf()];
    while let Some(ordner) = stapel.pop() {
        for eintrag in std::fs::read_dir(&ordner).unwrap().flatten() {
            let name = eintrag.file_name().to_string_lossy().into_owned();
            let hoehen = ordner.ends_with("heights") && name.ends_with(".bin");
            let liste = ordner == baum.wurzel() && name == "trees.json";
            if eintrag.path().is_dir() {
                stapel.push(eintrag.path());
            } else if !name.ends_with(".webp") && name != "map.json" && !hoehen && !liste {
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
    let probe = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), probe.path(), &["--scale", "16"]));
    // Wo die letzte Kachel hin soll, steht ein Verzeichnis: sie zu
    // schreiben scheitert.
    let out = neuer_baum("2x1-se");
    let kachel = dateien(probe.path()).pop().expect("eine Kachel");
    std::fs::create_dir_all(out.path().join(&kachel)).unwrap();
    assert!(
        !tiles(welt.path(), out.path(), &["--scale", "16"])
            .status
            .success(),
        "{kachel} hätte sich nicht schreiben lassen dürfen"
    );
    // Wählen lässt sich der Baum schon nach dem abgebrochenen ersten Lauf.
    let liste = std::fs::read_to_string(out.wurzel().join("trees.json")).unwrap();
    assert!(liste.contains("\"2x1-se\""), "{liste}");
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
    let out = neuer_baum("2x1-se");
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

    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
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
    let out = neuer_baum("2x1-se");
    std::fs::create_dir_all(out.path()).unwrap();
    std::fs::write(
        out.path().join("map.json"),
        r#"{"tileSize":256,"scale":6,"minZoom":0,"maxZoom":3,"tiles":"{z}/{x}/{y}.webp","bounds":[0,0,256,256]}"#,
    )
    .unwrap();
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "8"]);
    assert!(!ausgabe.status.success());
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("neue Wurzel"), "Meldung: {meldung}");
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

    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(&nether, baum.path(), &["--scale", "16"]));
    let karte = std::fs::read_to_string(baum.path().join("map.json")).unwrap();
    kennung_in(&karte);
    let ausgabe = tiles(welt.path(), baum.path(), &["--scale", "16"]);
    assert!(
        !ausgabe.status.success(),
        "die Oberwelt kam in den Netherbaum"
    );

    let baum = neuer_baum("2x1-se");
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
    assert!(!ohne_wurzel.contains("neue Wurzel"), "{ohne_wurzel}");
    common::write_level_dat(kopie.path());
    let ohne_seed = meldung();
    assert!(ohne_seed.contains("nennt keinen Seed"), "{ohne_seed}");
    assert!(!ohne_seed.contains("Wurzel richten"), "{ohne_seed}");
}

/// Im Nether schattiert das Spiel die Oberseite mit 0,9 statt 1, die
/// Seiten wie in der Oberwelt: `cardinal_light` ist dort `nether`. Dazu
/// gibt es kein Himmelslicht, und die Lightmap liegt in seiner
/// Umgebungsfarbe. Dieselbe Szene im Nether ist deshalb überall dunkler:
/// an den Seiten im Verhältnis der Lightmap, oben bei 0,9 davon. Die
/// Ausgabe nennt die Dimension. Eine eigene Dimension findet ihren Typ in
/// einer Datenwurzel. Ohne Weltwurzel gilt die Oberwelt, mit einer Meldung.
#[test]
fn nether_schattiert_wie_im_spiel() {
    let welt = tempdir();
    let oberwelt = welt.path().join("dimensions/minecraft/overworld");
    let nether = welt.path().join("dimensions/minecraft/the_nether");
    let eigene = welt.path().join("dimensions/beispiel/tief");
    let ohne_wurzel = tempdir();
    let szene = |x, y, z| match (x, y, z) {
        (8, 0, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    for dir in [
        &oberwelt,
        &nether,
        &eigene,
        &ohne_wurzel.path().to_path_buf(),
    ] {
        common::write_world(dir, &[(0, 0)], szene);
    }
    common::write_wurzel(welt.path(), 1);
    // Eine Datenwurzel nur mit einer Dimension, ohne Biome.
    let daten = tempdir();
    std::fs::create_dir_all(daten.path().join("beispiel/dimension")).unwrap();
    std::fs::write(
        daten.path().join("beispiel/dimension/tief.json"),
        r#"{"type": "minecraft:the_nether"}"#,
    )
    .unwrap();
    let bild = |dir: &Path| {
        let png = tempdir();
        let pfad = png.path().join("bild.png");
        let ausgabe = gelungen(&cli(&[
            OsStr::new("--world"),
            dir.as_os_str(),
            OsStr::new("--assets"),
            assets_ref(),
            OsStr::new("--data"),
            daten.path().as_os_str(),
            OsStr::new("--render"),
            pfad.as_os_str(),
            OsStr::new("--center"),
            OsStr::new("8"),
            OsStr::new("8"),
            OsStr::new("--size"),
            OsStr::new("128"),
        ]))
        .stdout
        .clone();
        let bild = image::open(&pfad).unwrap().into_rgba8();
        (bild, String::from_utf8(ausgabe).unwrap())
    };
    let (hell, ausgabe) = bild(&oberwelt);
    assert!(
        ausgabe.contains("Dimension:  minecraft:overworld\n"),
        "{ausgabe}"
    );
    assert!(
        ausgabe.contains("0 Biome, 0 Bannermuster, 1 Dimensionen und Typen aus"),
        "{ausgabe}"
    );
    let (dunkel, ausgabe) = bild(&nether);
    assert!(
        ausgabe.contains("Dimension:  minecraft:the_nether\n"),
        "{ausgabe}"
    );
    let typ = DimensionType::des_spiels("minecraft:the_nether").unwrap();
    let seite = Lightmap::new(&typ).factors(Light::sky(0))[0] as f64 / 255.0;
    let (mut an_der_seite, mut oben) = (0, 0);
    for (h, d) in hell.pixels().zip(dunkel.pixels()) {
        assert_eq!(h[3], d[3], "dieselben Umrisse");
        assert!(
            (0..3).all(|c| d[c] < h[c] || h[c] == 0),
            "{h:?} im Nether nicht dunkler: {d:?}"
        );
        if h[3] == 255 && h[0] >= 60 {
            let verhaeltnis = d[0] as f64 / h[0] as f64;
            if (verhaeltnis - seite).abs() < 0.02 {
                an_der_seite += 1;
            } else if (verhaeltnis - 0.9 * seite).abs() < 0.02 {
                oben += 1;
            }
        }
    }
    assert!(
        an_der_seite > 0 && oben > 0,
        "{an_der_seite} an der Seite, {oben} oben"
    );
    let (tief, ausgabe) = bild(&eigene);
    assert!(ausgabe.contains("Dimension:  beispiel:tief\n"), "{ausgabe}");
    assert_eq!(tief.as_raw(), dunkel.as_raw(), "Typ aus der Datenwurzel");

    let (ohne, ausgabe) = bild(ohne_wurzel.path());
    assert!(ausgabe.contains("keine Weltwurzel"), "{ausgabe}");
    assert_eq!(
        ohne.as_raw(),
        hell.as_raw(),
        "ohne Weltwurzel wie die Oberwelt"
    );
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
    let baum = neuer_baum("2x1-se");
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
    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(&nether, baum.path(), &["--scale", "16"]));
    let karte = || std::fs::read_to_string(baum.path().join("map.json")).unwrap();
    let vorher = kennung_in(&karte());
    let ausgabe = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .current_dir(&nether)
        .args(["--world", ".", "--assets"])
        .arg(assets())
        .arg("--tiles")
        .arg(baum.wurzel())
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
    let salz = |wurzel: &Path| {
        let baum = wurzel.join("2x1-se");
        gelungen(&tiles(welt.path(), &baum, &["--scale", "16"]));
        let karte = std::fs::read_to_string(baum.join("map.json")).unwrap();
        std::fs::remove_dir_all(&baum).unwrap();
        let kennung = kennung_in(&karte);
        u64::from_str_radix(kennung.split('-').next().unwrap(), 16).unwrap()
    };
    let wurzel = tempdir();
    let salze = [
        salz(wurzel.path()),
        salz(wurzel.path()),
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
    let out = neuer_baum("2x1-se");
    let ausgabe = tiles(welt.path(), out.path(), &["--scale", "16", "--size", "0"]);
    assert!(!ausgabe.status.success(), "--size 0 lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("--size"), "Meldung: {meldung}");
    assert!(schnappschuss(out.path()).is_empty(), "etwas geschrieben");
}

/// Eine geflutete Truhe hat ein Blockmodell ohne Elemente, ihr Bild aus
/// dem Blockentity und ihr Wasser. `--block` nennt beides und meldet sie
/// nicht als Block ohne Modell; einen Block, der gar nichts zeichnet, schon.
/// Ohne Eigenschaften steht das Bild der Truhe nicht fest, leer ist sie
/// trotzdem nicht.
#[test]
fn geflutete_truhe_zeigt_blockentity_und_wasser() {
    let block = |block: &str| {
        let ausgabe = cli(&[
            OsStr::new("--assets"),
            assets_ref(),
            OsStr::new("--block"),
            OsStr::new(block),
        ]);
        String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned()
    };
    let text = block("chest[facing=north,type=single,waterlogged=true]");
    assert!(text.contains("Blockentity: "), "{text}");
    assert!(text.contains("minecraft:entity/chest/normal"), "{text}");
    assert!(text.contains("Flüssigkeit: Water"), "{text}");
    assert!(!text.contains("kein Modell"), "{text}");
    assert!(!text.contains("je nach Zustand"), "{text}");
    let text = block("nur_partikel");
    assert!(text.contains("kein Modell"), "{text}");
    assert!(!text.contains("Blockentity: "), "{text}");
    let text = block("chest");
    assert!(text.contains("je nach Zustand"), "{text}");
    assert!(!text.contains("kein Modell"), "{text}");
}

/// Fehlt eine Textur, sagt es der Lauf gleich nach der Sprite-Tabelle, vor
/// der ersten Kachel, und nennt sie am Ende. Der einfachen Truhe fehlt ihre
/// Textur in den Fixtures.
#[test]
fn fehlende_texturen_gleich_nach_der_sprite_tabelle() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:chest[facing=north,type=single,waterlogged=false]",
        (_, 3, _) => "minecraft:einfarbig",
        _ => "minecraft:air",
    });
    let out = neuer_baum("2x1-se");
    let text = String::from_utf8_lossy(&gelungen(&export(welt.path(), out.path(), &[])).stdout)
        .into_owned();
    let warnung = text.find("Texturen fehlen").expect(&text);
    assert!(warnung < text.find("Kacheln:").expect(&text), "{text}");
    assert!(
        text[warnung..].contains("minecraft:entity/chest/normal"),
        "{text}"
    );

    let bild = out.path().join("ausschnitt.png");
    let ausgabe = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--render"),
        bild.as_os_str(),
        OsStr::new("--size"),
        OsStr::new("64"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    let warnung = text.find("Texturen fehlen").expect(&text);
    assert!(warnung < text.find("Render:").expect(&text), "{text}");
}

/// `--scan` nennt dieselben Blöcke ohne Modell wie `--block`: den, der
/// nichts zeichnet, ja, die geflutete Truhe und Wasser nicht.
#[test]
fn scan_nennt_nur_bloecke_ohne_bild() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:chest[facing=north,type=single,waterlogged=true]",
        (9, 4, 8) => "minecraft:water",
        (10, 4, 8) => "minecraft:einfarbig",
        (11, 4, 8) => "minecraft:nur_partikel",
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
    assert_eq!(namen, ["minecraft:nur_partikel"], "{text}");
}

/// `--scan` zählt Banner mit Mustern und Krüge mit Scherben, und wie viele
/// davon samt Block verschieden sind, so viele Familien baut eine
/// Sprite-Tabelle höchstens dazu: zwei gleiche Banner zählen einmal,
/// derselbe Krug trocken und geflutet zweimal.
#[test]
fn scan_zaehlt_blockentities_mit_daten() {
    use fastnbt::Value;
    let lage = Value::Compound(std::collections::HashMap::from([
        (
            "pattern".to_string(),
            Value::String("minecraft:stripe_top".to_string()),
        ),
        ("color".to_string(), Value::String("red".to_string())),
    ]));
    let scherben = Value::List(vec![Value::String(
        "minecraft:angler_pottery_sherd".to_string(),
    )]);
    let welt = tempdir();
    common::write_world_entities(
        welt.path(),
        &[(0, 0)],
        |x, y, z| match (x, y, z) {
            (4 | 6, 1, 4) => "minecraft:white_banner[rotation=0]",
            (4, 1, 10) => "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=false]",
            (4, 1, 13) => "minecraft:decorated_pot[cracked=false,facing=north,waterlogged=true]",
            _ => "minecraft:air",
        },
        |_, _| {
            let banner = |x| {
                common::blockentity(
                    "minecraft:banner",
                    [x, 1, 4],
                    "patterns",
                    Value::List(vec![lage.clone()]),
                )
            };
            let krug =
                |z| common::blockentity("decorated_pot", [4, 1, z], "sherds", scherben.clone());
            vec![banner(4), banner(6), krug(10), krug(13)]
        },
    );
    let ausgabe = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--scan"),
    ]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    assert!(
        text.contains("2 Banner mit Mustern, 2 Krüge mit Scherben, 3 verschiedene samt Block"),
        "{text}"
    );
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
    let out = neuer_baum("2x1-se");
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

/// Zwei Kameras in einem Baum mischten sich still. Jede Kamera hat ihren
/// eigenen Ordner; trägt einer trotzdem einen Baum mit anderer Kamera, etwa
/// nach dem Umbenennen, bricht der Lauf ab, bevor er eine Kachel schreibt.
/// `map.json` nennt Kamera und Projektion der feinsten Stufe.
#[test]
fn andere_kamera_wird_abgelehnt() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = neuer_baum("4x3-se");
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "8", "--camera", "8:6"],
    ));
    let info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("map.json")).unwrap())
            .unwrap();
    assert_eq!(info["camera"], "4:3", "gekürzt");
    assert_eq!(
        info["projection"],
        serde_json::json!({"azimuth": "diagonal", "u": 4, "v": 3, "y": 4})
    );
    // Der Baum von 4:3 liegt jetzt, wo 2:1 schreibt.
    let falsch = out.path().with_file_name("2x1-se");
    std::fs::rename(out.path(), &falsch).unwrap();
    let vorher = schnappschuss(&falsch);

    // Ohne --camera: die Vorgabe ist 2:1. Die Meldung rät, den Ordner
    // zurückzubenennen; mit --camera 4:3 schriebe der Lauf nach 4x3-se.
    let ziel = out.path().display().to_string();
    let ausgabe = tiles(welt.path(), &falsch, &["--scale", "8"]);
    assert!(!ausgabe.status.success(), "2:1 hätte abbrechen müssen");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("Kamera 4:3"), "Meldung: {meldung}");
    assert!(
        meldung.contains(&format!("Den Ordner nach {ziel} umbenennen")),
        "Meldung: {meldung}"
    );
    assert!(!meldung.contains("--scale"), "Meldung: {meldung}");

    // Weicht auch der scale ab, nennt die Meldung ihn dazu.
    let ausgabe = tiles(welt.path(), &falsch, &["--scale", "16"]);
    assert!(!ausgabe.status.success(), "2:1 hätte abbrechen müssen");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains(&format!(
            "{ziel} umbenennen, dann mit --scale 8 weiterrendern"
        )),
        "Meldung: {meldung}"
    );
    assert_eq!(
        schnappschuss(&falsch),
        vorher,
        "der Baum hat sich verändert"
    );
}

/// Ein Baum aus einem älteren Stand nennt keine Kamera und zeigt 2:1: Ein
/// Lauf in 2:1 nimmt ihn auf und trägt sie ein, einer mit anderer Kamera
/// nicht, auch wenn der Baum in deren Ordner liegt.
#[test]
fn baum_ohne_kamera_zeigt_zwei_zu_eins() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let out = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));
    let karte = out.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    for feld in ["camera", "direction", "projection"] {
        info.as_object_mut().unwrap().remove(feld);
    }
    std::fs::write(&karte, serde_json::to_string(&info).unwrap()).unwrap();

    let fremd = kopie(out.path());
    let oben = fremd.path().with_file_name("top-se");
    std::fs::rename(fremd.path(), &oben).unwrap();
    let ausgabe = tiles(welt.path(), &oben, &["--scale", "8", "--camera", "top"]);
    assert!(!ausgabe.status.success(), "top hätte abbrechen müssen");
    assert!(String::from_utf8_lossy(&ausgabe.stderr).contains("Kamera 2:1"));

    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));
    let info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    assert_eq!(info["camera"], "2:1");
}

/// Genordet stehen `azimuth` `north` und `direction` `s` in `map.json`. Eine
/// andere Kamera schreibt ihren eigenen Baum daneben. Ein Baum mit einer
/// Richtung, die seine Kamera nicht kennt, bricht ab; fehlt sie, gilt die
/// Vorgabe der Kamera.
#[test]
fn genordeter_baum_mit_azimut_und_richtung() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let out = neuer_baum("north-45-s");
    let karte = out.path().join("map.json");
    let lies = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap()
    };
    let schreibe = |info: &serde_json::Value| {
        std::fs::write(&karte, serde_json::to_string(info).unwrap()).unwrap();
    };
    let genordet = ["--scale", "8", "--camera", "north-45"];
    gelungen(&tiles(welt.path(), out.path(), &genordet));
    let info = lies();
    assert_eq!(info["camera"], "north-45");
    assert_eq!(info["direction"], "s");
    assert_eq!(
        info["projection"],
        serde_json::json!({"azimuth": "north", "u": 8, "v": 8, "y": 8})
    );
    let vorher = schnappschuss(out.path());

    // Eine andere Kamera bekommt ihren eigenen Baum daneben.
    let oben = out.path().with_file_name("top-north-s");
    gelungen(&tiles(
        welt.path(),
        &oben,
        &["--scale", "8", "--camera", "top-north"],
    ));
    assert!(
        oben.join("map.json").is_file(),
        "top-north ohne eigenen Baum"
    );
    assert_eq!(
        schnappschuss(out.path()),
        vorher,
        "top-north hat den Baum verändert"
    );
    // Liegt der Baum aus north-45 im Ordner von top-north, bricht top-north
    // ab und rät zurück nach north-45-s.
    let vertauscht = kopie(out.path());
    let falsch = vertauscht.path().with_file_name("top-north-s");
    std::fs::rename(vertauscht.path(), &falsch).unwrap();
    let ausgabe = tiles(
        welt.path(),
        &falsch,
        &["--scale", "8", "--camera", "top-north"],
    );
    assert!(
        !ausgabe.status.success(),
        "top-north hätte abbrechen müssen"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("Kamera north-45"), "Meldung: {meldung}");
    assert!(
        meldung.contains(&vertauscht.path().display().to_string()),
        "Meldung: {meldung}"
    );

    let mut info = lies();
    info["direction"] = "se".into();
    schreibe(&info);
    let ausgabe = tiles(welt.path(), out.path(), &genordet);
    assert!(!ausgabe.status.success(), "se hätte abbrechen müssen");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains("north-45 schaut von einer Seite: s, w, n oder e"),
        "Meldung: {meldung}"
    );
    // Ohne `map.json`, die der Test selbst geändert hat.
    let ohne_karte = |mut baum: BTreeMap<String, Vec<u8>>| {
        baum.remove("map.json");
        baum
    };
    assert_eq!(
        ohne_karte(schnappschuss(out.path())),
        ohne_karte(vorher.clone()),
        "se hat den Baum verändert"
    );
    assert_eq!(lies(), info, "se hat map.json verändert");

    info.as_object_mut().unwrap().remove("direction");
    schreibe(&info);
    gelungen(&tiles(welt.path(), out.path(), &genordet));
    assert_eq!(lies()["direction"], "s");
    assert_eq!(
        schnappschuss(out.path()),
        vorher,
        "der Baum hat sich verändert"
    );
}

/// Ohne `--scale` rendern `top-north` und `north-45` bei 16, wo jedes Texel
/// einer Oberseite schon ein Pixel ist, alle anderen Kameras bei 32 (User,
/// 02.10.). Ein angegebener scale bleibt.
#[test]
fn ohne_scale_genordet_16_sonst_32() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    for (kamera, extra, scale) in [
        ("2:1", None, 32),
        ("top", None, 32),
        ("top-north", None, 16),
        ("north-45", None, 16),
        ("north-45", Some("8"), 8),
    ] {
        let mut args = vec!["--camera", kamera, "--size", "256"];
        if let Some(s) = extra {
            args.extend(["--scale", s]);
        }
        let out = neuer_baum(&baum_name(&args));
        gelungen(&tiles(welt.path(), out.path(), &args));
        let text = std::fs::read_to_string(out.path().join("map.json")).unwrap();
        let info: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(info["scale"], scale, "{kamera} mit {extra:?}");
    }
}

/// `--direction` passt zur Kamera: diagonal über eine Ecke, genordet von
/// einer Seite. Eine falsche Kombination bricht ab, bevor der Lauf die Welt
/// liest, und nennt die vier, die gehen. Jede passende geht, die Vorgabe
/// auch.
#[test]
fn richtung_wird_je_kamera_geprueft() {
    let leer = tempdir();
    let lauf = |kamera: &str, richtung: &str| {
        cli(&[
            OsStr::new("--world"),
            leer.path().join("fehlt").as_os_str(),
            OsStr::new("--camera"),
            OsStr::new(kamera),
            OsStr::new("--direction"),
            OsStr::new(richtung),
        ])
    };
    for (kamera, richtung, soll) in [
        (
            "north-45",
            "ne",
            "north-45 schaut von einer Seite: s, w, n oder e",
        ),
        (
            "top-north",
            "se",
            "top-north schaut von einer Seite: s, w, n oder e",
        ),
        ("8:5", "n", "8:5 schaut über eine Ecke: se, sw, nw oder ne"),
        ("top", "s", "top schaut über eine Ecke: se, sw, nw oder ne"),
    ] {
        let ausgabe = lauf(kamera, richtung);
        assert!(!ausgabe.status.success(), "{kamera} {richtung}");
        let meldung = String::from_utf8_lossy(&ausgabe.stderr);
        assert!(meldung.contains(soll), "{kamera} {richtung}: {meldung}");
        assert!(!String::from_utf8_lossy(&ausgabe.stdout).contains("Welt:"));
    }
    // Jede Kamera mit jeder ihrer vier Richtungen scheitert erst an der
    // fehlenden Welt.
    let diagonal = ["2:1", "8:5", "4:3", "1:1", "top"].map(|k| (k, ["se", "sw", "nw", "ne"]));
    let genordet = ["top-north", "north-45"].map(|k| (k, ["s", "w", "n", "e"]));
    for (kamera, richtung) in diagonal
        .into_iter()
        .chain(genordet)
        .flat_map(|(kamera, richtungen)| richtungen.map(|r| (kamera, r)))
    {
        let meldung = String::from_utf8_lossy(&lauf(kamera, richtung).stderr).into_owned();
        assert!(
            meldung.contains("kein region-Verzeichnis"),
            "{kamera} {richtung}: {meldung}"
        );
    }
}

/// Mit `--direction` entsteht ein eigener Baum unter derselben Wurzel, mit
/// der Richtung in `map.json` und `trees.json`. Ein Kasten aus einem Block
/// einer Farbe sieht aus `nw` und `sw` aus wie aus `se`, nur liegt er im
/// Blick woanders: Alle Bäume haben auf der feinsten Stufe dieselben Pixel.
/// Aus `sw` liegen die Seiten der Welt anders, je Seite gleich viele Pixel
/// wie aus `se`; ein falscher Drehsinn verlöre Kacheln. Der
/// Vorlauf findet die Kacheln also auch im Blick, über die Grenze zweier
/// Chunks hinweg. Der Kasten liegt weit weg vom Ursprung; dort deckte schon
/// der ungedrehte Kasten eines Chunks die Kacheln im Blick.
#[test]
fn richtung_ist_ein_eigener_baum() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(10, 10), (11, 10)], |x, y, z| {
        match (x, y, z) {
            (170..=181, 0..=2, 163..=166) => "minecraft:einfarbig",
            _ => "minecraft:air",
        }
    });
    let se = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), se.path(), &[]));
    let nw = se.wurzel().join("2x1-nw");
    gelungen(&tiles(welt.path(), &nw, &["--direction", "nw"]));
    let sw = se.wurzel().join("2x1-sw");
    gelungen(&tiles(welt.path(), &sw, &["--direction", "sw"]));
    let info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(nw.join("map.json")).unwrap()).unwrap();
    assert_eq!(info["direction"], "nw");
    let liste: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(se.wurzel().join("trees.json")).unwrap())
            .unwrap();
    let ordner: Vec<&str> = liste["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|baum| baum["path"].as_str().unwrap())
        .collect();
    assert_eq!(ordner, ["2x1-se", "2x1-nw", "2x1-sw"]);
    // Je Farbe, wie viele Pixel sie auf der feinsten Stufe hat.
    let farben = |dir: &Path| {
        let mut farben: BTreeMap<[u8; 4], usize> = BTreeMap::new();
        for pfad in kacheln(dir, max_zoom(dir)).values() {
            for pixel in bild(pfad).pixels().filter(|p| p.0[3] > 0) {
                *farben.entry(pixel.0).or_default() += 1;
            }
        }
        farben
    };
    let soll = farben(se.path());
    assert!(soll.len() >= 3, "drei Seiten, drei Farben: {soll:?}");
    assert_eq!(farben(&nw), soll, "aus nw");
    assert_eq!(farben(&sw), soll, "aus sw");
}

/// `--center 40 -20` legt den Punkt (40, 0, −20) der Welt in die Bildmitte,
/// aus jeder Richtung und bei jeder Art von Kamera. Um ihn liegen vier blaue
/// Blöcke mit der Oberseite in Höhe 0, in einem Boden einer anderen Farbe.
/// Die Pixel um die Mitte sind nur blau, wenn der Punkt genau dort liegt:
/// Einen Block daneben läge die Mitte auf einer Kante oder Ecke der vier.
/// Der Chunk an der Stelle im Blick fehlt; ungedreht bliebe die Mitte leer.
#[test]
fn center_in_der_welt_aus_jeder_richtung() {
    let welt = tempdir();
    let block = |x, y, z| match (x, y, z) {
        (39..=40, -1, -21..=-20) => "minecraft:blauwuerfel",
        (_, -1, _) => "minecraft:einfarbig",
        _ => "minecraft:air",
    };
    common::write_world_sections(welt.path(), &[(2, -2)], [-1], block, |_, _| None);
    let blau = image::Rgba([40, 60, 200, 255]);
    let diagonal = ["se", "sw", "nw", "ne"].map(|r| ("2:1", r));
    let genordet = ["s", "w", "n", "e"];
    for (kamera, richtung) in diagonal
        .into_iter()
        .chain([("top", "nw")])
        .chain(genordet.map(|r| ("top-north", r)))
        .chain(genordet.map(|r| ("north-45", r)))
    {
        let png = tempdir();
        let pfad = png.path().join("bild.png");
        gelungen(&cli(&[
            OsStr::new("--world"),
            welt.path().as_os_str(),
            OsStr::new("--assets"),
            assets_ref(),
            OsStr::new("--render"),
            pfad.as_os_str(),
            OsStr::new("--center"),
            OsStr::new("40"),
            OsStr::new("-20"),
            OsStr::new("--size"),
            OsStr::new("32"),
            OsStr::new("--camera"),
            OsStr::new(kamera),
            OsStr::new("--direction"),
            OsStr::new(richtung),
        ]));
        let bild = image::open(&pfad).unwrap().into_rgba8();
        // Die Mitte ist die Ecke zwischen den Pixeln 15 und 16.
        for (x, y) in [(14, 15), (17, 15), (14, 16), (17, 16)] {
            let p = *bild.get_pixel(x, y);
            assert_eq!(
                [p[2] > p[0], p[3] == 255],
                [true, true],
                "{kamera} {richtung}, Pixel ({x}, {y}): {p:?}, blau ist {blau:?}"
            );
        }
    }
}

/// `trees.json` unter der Wurzel nennt jeden Baum mit Ordner, Kamera,
/// Richtung und `look`, `2x1-se` zuerst, auch vor `1x1-se`, sonst nach
/// Ordner. Sie kommt aus der Platte: Ein gelöschter Baum fällt beim nächsten
/// Lauf heraus. Die Höhen liegen einmal unter der Wurzel, kein Baum hat
/// eigene.
#[test]
fn liste_der_baeume_unter_der_wurzel() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let oben = neuer_baum("top-north-s");
    let wurzel = oben.wurzel();
    let liste = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(wurzel.join("trees.json")).unwrap()).unwrap()
    };
    let ordner = || -> Vec<String> {
        liste()["trees"]
            .as_array()
            .unwrap()
            .iter()
            .map(|baum| baum["path"].as_str().unwrap().to_string())
            .collect()
    };
    gelungen(&tiles(
        welt.path(),
        oben.path(),
        &["--scale", "8", "--camera", "top-north"],
    ));
    assert_eq!(
        liste(),
        serde_json::json!({"trees": [
            {"path": "top-north-s", "camera": "top-north", "direction": "s", "look": "map"}
        ]})
    );
    let schraeg = wurzel.join("2x1-se");
    gelungen(&tiles(welt.path(), &schraeg, &["--scale", "8"]));
    let vier = wurzel.join("4x3-se");
    gelungen(&tiles(
        welt.path(),
        &vier,
        &["--scale", "8", "--camera", "4:3"],
    ));
    let eins = wurzel.join("1x1-se");
    gelungen(&tiles(
        welt.path(),
        &eins,
        &["--scale", "8", "--camera", "1:1"],
    ));
    assert_eq!(ordner(), ["2x1-se", "1x1-se", "4x3-se", "top-north-s"]);
    assert_eq!(liste()["trees"][2]["camera"], "4:3");
    assert!(wurzel.join("heights/0.0.bin").is_file(), "Höhen fehlen");
    for baum in ordner() {
        assert!(!wurzel.join(&baum).join("heights").exists(), "{baum}");
    }

    std::fs::remove_dir_all(&vier).unwrap();
    gelungen(&tiles(welt.path(), &schraeg, &["--scale", "8"]));
    assert_eq!(ordner(), ["2x1-se", "1x1-se", "top-north-s"]);
}

/// Eine Wurzel, eine Welt: Ihre Bäume teilen sich die Höhen. Ein Lauf einer
/// anderen Welt in dieselbe Wurzel bricht ab, bevor er etwas liest oder
/// schreibt, auch mit einer anderen Kamera und in einen neuen Ordner, und
/// ebenso `--heights` für einen Baum der anderen Welt darin. Ein Lauf
/// derselben Welt geht.
#[test]
fn eine_wurzel_eine_welt() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    common::write_wurzel(welt.path(), 4_815_162_342);
    let fremd = tempdir();
    common::write_world(fremd.path(), &[(0, 0), (2, 2)], gelaende);
    common::write_wurzel(fremd.path(), 2_718_281_828);
    let schraeg = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), schraeg.path(), &["--scale", "8"]));
    let wurzel = schraeg.wurzel();
    let hoehen = || schnappschuss(&wurzel.join("2x1-se"));
    let vorher = hoehen();

    let oben = wurzel.join("top-se");
    let ausgabe = tiles(
        fremd.path(),
        &oben,
        &["--scale", "8", "--camera", "top", "--prune"],
    );
    assert!(!ausgabe.status.success(), "die fremde Welt lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains("anderen Welt") && meldung.contains("neue Wurzel"),
        "Meldung: {meldung}"
    );
    assert!(!oben.exists(), "der fremde Baum ist angelegt");
    assert_eq!(
        hoehen(),
        vorher,
        "der Baum oder die Höhen haben sich verändert"
    );

    // Ebenso `--heights` für einen Baum der anderen Welt unter dieser Wurzel.
    let anderswo = neuer_baum("top-se");
    gelungen(&tiles(
        fremd.path(),
        anderswo.path(),
        &["--scale", "8", "--camera", "top"],
    ));
    std::fs::create_dir(&oben).unwrap();
    std::fs::copy(anderswo.path().join("map.json"), oben.join("map.json")).unwrap();
    let ausgabe = cli(&[
        OsStr::new("--world"),
        fremd.path().as_os_str(),
        OsStr::new("--heights"),
        oben.as_os_str(),
    ]);
    assert!(
        !ausgabe.status.success(),
        "--heights der fremden Welt lief durch"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(meldung.contains("anderen Welt"), "Meldung: {meldung}");
    assert_eq!(hoehen(), vorher, "--heights hat die Höhen verändert");
    std::fs::remove_dir_all(&oben).unwrap();

    gelungen(&tiles(
        welt.path(),
        &oben,
        &["--scale", "8", "--camera", "top"],
    ));
}

/// `--tiles` nimmt die Wurzel. Wer den Ordner eines Baums nennt, wie für
/// `--pyramid`, bekommt die Wurzel genannt; nichts ändert sich, und kein
/// Baum entsteht in ihm.
#[test]
fn baum_statt_wurzel_nennt_die_wurzel() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "8"]));
    let vorher = schnappschuss(baum.path());
    let args = [
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--tiles"),
        baum.path().as_os_str(),
        OsStr::new("--scale"),
        OsStr::new("8"),
    ];
    let ausgabe = cli(&args);
    assert!(!ausgabe.status.success(), "der Baum als Wurzel lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    let wurzel = std::path::absolute(baum.wurzel()).unwrap();
    assert!(
        meldung.contains(&format!("--tiles nimmt die Wurzel: {}", wurzel.display())),
        "Meldung: {meldung}"
    );
    assert!(!String::from_utf8_lossy(&ausgabe.stdout).contains("Welt:"));
    assert_eq!(schnappschuss(baum.path()), vorher);
    assert!(!baum.path().join("2x1-se").exists());
    assert!(!baum.path().join("trees.json").exists());

    // Jedes der beiden Zeichen reicht allein: ohne trees.json daneben die
    // Höhen unter ../, ohne Höhen die trees.json.
    let nennt_wurzel =
        || String::from_utf8_lossy(&cli(&args).stderr).contains("--tiles nimmt die Wurzel");
    let baeume = baum.wurzel().join("trees.json");
    let liste = std::fs::read(&baeume).unwrap();
    std::fs::remove_file(&baeume).unwrap();
    assert!(nennt_wurzel(), "nur die Höhen unter ../");
    std::fs::write(&baeume, liste).unwrap();
    let karte = baum.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&karte).unwrap()).unwrap();
    info.as_object_mut().unwrap().remove("heights").unwrap();
    std::fs::write(&karte, serde_json::to_vec_pretty(&info).unwrap()).unwrap();
    assert!(nennt_wurzel(), "nur trees.json");
}

/// Ein Nachbar mit kaputtem `map.json` lässt keinen Lauf scheitern, weder
/// am Anfang noch am Ende: Er fehlt in `trees.json`, und der Lauf sagt es.
#[test]
fn kaputter_nachbar_wird_uebergangen() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let baum = neuer_baum("2x1-se");
    let kaputt = baum.wurzel().join("kaputt");
    std::fs::create_dir_all(&kaputt).unwrap();
    std::fs::write(kaputt.join("map.json"), "{").unwrap();
    let unbekannt = baum.wurzel().join("8x5-se");
    std::fs::create_dir_all(&unbekannt).unwrap();
    std::fs::write(
        unbekannt.join("map.json"),
        r#"{"tileSize":256,"scale":8,"minZoom":0,"maxZoom":3,"tiles":"{z}/{x}/{y}.webp","bounds":[0,0,256,256],"camera":"8:5","direction":"n"}"#,
    )
    .unwrap();
    let ausgabe = tiles(welt.path(), baum.path(), &["--scale", "8"]);
    let text = String::from_utf8_lossy(&gelungen(&ausgabe).stdout).into_owned();
    for ordner in [&kaputt, &unbekannt] {
        assert!(
            text.contains(&format!("{} übergangen", ordner.display())),
            "{text}"
        );
    }
    let liste: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baum.wurzel().join("trees.json")).unwrap())
            .unwrap();
    assert_eq!(liste["trees"].as_array().unwrap().len(), 1, "{liste}");
    assert_eq!(liste["trees"][0]["path"], "2x1-se");
}

/// Steht im Ordner eines Baums eine andere gültige Richtung, etwa `sw` nach
/// dem Umbenennen, bricht ein Lauf aus `se` ab und rät, den Ordner nach
/// `2x1-sw` umzubenennen.
#[test]
fn andere_richtung_im_ordner_wird_abgelehnt() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let baum = neuer_baum("2x1-se");
    gelungen(&tiles(welt.path(), baum.path(), &["--scale", "8"]));
    let karte = baum.path().join("map.json");
    let mut info: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&karte).unwrap()).unwrap();
    info["direction"] = "sw".into();
    std::fs::write(&karte, serde_json::to_string(&info).unwrap()).unwrap();
    let vorher = schnappschuss(baum.path());
    let ausgabe = tiles(welt.path(), baum.path(), &["--scale", "8"]);
    assert!(!ausgabe.status.success(), "se hätte abbrechen müssen");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    let ziel = baum.path().with_file_name("2x1-sw");
    assert!(
        meldung.contains("Richtung sw")
            && meldung.contains(&format!("Den Ordner nach {} umbenennen", ziel.display())),
        "Meldung: {meldung}"
    );
    assert_eq!(schnappschuss(baum.path()), vorher);
}

/// Ein Baum der alten Ablage, `map.json` direkt unter `--tiles`, bricht ab,
/// bevor der Lauf die Welt liest und vor dem Hinweis zum Echtzeitschutz. Die
/// Meldung nennt den Ordner, in den er gehört, ohne `heights/`, die in der
/// Wurzel bleibt; nichts ändert sich, auch keine `trees.json`.
#[test]
fn alte_ablage_nennt_den_ordner() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    let alt = tempdir();
    let karte = r#"{"tileSize":256,"scale":8,"minZoom":0,"maxZoom":3,"tiles":"{z}/{x}/{y}.webp","bounds":[0,0,256,256],"camera":"north-45","direction":"s"}"#;
    std::fs::write(alt.path().join("map.json"), karte).unwrap();
    let ausgabe = cli(&[
        OsStr::new("--world"),
        welt.path().as_os_str(),
        OsStr::new("--assets"),
        assets_ref(),
        OsStr::new("--tiles"),
        alt.path().as_os_str(),
        OsStr::new("--camera"),
        OsStr::new("north-45"),
    ]);
    assert!(
        !ausgabe.status.success(),
        "die alte Ablage hätte abbrechen müssen"
    );
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    let ziel = alt.path().join("north-45-s");
    assert!(
        meldung.contains("alten Ablage")
            && meldung.contains(&format!("alles ausser heights/ nach {}", ziel.display()))
            && meldung.contains("heights/ bleibt in der Wurzel"),
        "Meldung: {meldung}"
    );
    let text = String::from_utf8_lossy(&ausgabe.stdout);
    for vorher in ["Welt:", "Defender:", "Assets:"] {
        assert!(!text.contains(vorher), "{vorher}: {text}");
    }
    assert_eq!(
        std::fs::read_to_string(alt.path().join("map.json")).unwrap(),
        karte
    );
    assert!(!alt.path().join("trees.json").exists());
    assert!(!ziel.exists());
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
    let wurzel = eltern.path().join("gibt-es-noch-nicht");
    let ziel = wurzel.join("2x1-se");
    gelungen(&tiles(
        welt.path(),
        &ziel,
        &["--scale", "16", "--size", "256"],
    ));

    assert!(ziel.join("map.json").is_file(), "map.json fehlt");
    assert!(wurzel.join("trees.json").is_file(), "trees.json fehlt");
    assert!(dateien(&ziel).is_empty(), "es dürfte keine Kachel geben");
}

/// `--gpu on` liefert dieselben Dateien wie `--gpu off`, Byte für Byte —
/// Kacheln, native Stufen, Pyramide, `map.json` —, und die Karte zeichnet
/// die Basis wie die nativen Stufen. Ebenso ein Ausschnitt und ein
/// Fortsetzen. Ohne Adapter (auch keinen Software-Adapter) wird
/// übersprungen und gesagt, ausser in der CI (`common::ohne_gpu`).
#[test]
fn gpu_liefert_dieselben_kacheln() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (1, 1)], gelaende);

    let cpu = neuer_baum("2x1-se");
    let gpu = neuer_baum("2x1-se");
    let aus = tiles(welt.path(), cpu.path(), &["--scale", "16", "--gpu", "off"]);
    let ausgabe = String::from_utf8_lossy(&gelungen(&aus).stdout);
    assert!(
        ausgabe
            .lines()
            .any(|zeile| zeile == "GPU:        aus (--gpu off)"),
        "--gpu off sagt nicht, dass die Karte aus ist:\n{ausgabe}"
    );
    let lauf = tiles(welt.path(), gpu.path(), &["--scale", "16", "--gpu", "on"]);
    if !lauf.status.success()
        && String::from_utf8_lossy(&lauf.stderr).contains("keine Grafikkarte gefunden")
    {
        common::ohne_gpu();
        return;
    }
    assert!(ganz_auf_der_karte(&lauf) > 0, "keine native Stufe");
    assert_eq!(schnappschuss(cpu.path()), schnappschuss(gpu.path()));

    // Fortsetzen nach einer verlorenen Basiskachel: die Karte zeichnet
    // sie und alles, was die zwei Minuten vor der jüngsten treffen.
    altern(gpu.path());
    let z = max_zoom(gpu.path());
    let (_, verloren) = kacheln(gpu.path(), z).pop_first().unwrap();
    std::fs::remove_file(&verloren).unwrap();
    ganz_auf_der_karte(&tiles(
        welt.path(),
        gpu.path(),
        &["--scale", "16", "--gpu", "on", "--resume"],
    ));
    assert_eq!(schnappschuss(cpu.path()), schnappschuss(gpu.path()));

    // Ein Ausschnitt.
    let (cpu, gpu) = (neuer_baum("2x1-se"), neuer_baum("2x1-se"));
    let ausschnitt = ["--scale", "16", "--size", "300", "--center", "8", "8"];
    gelungen(&tiles(
        welt.path(),
        cpu.path(),
        &[&ausschnitt[..], &["--gpu", "off"]].concat(),
    ));
    ganz_auf_der_karte(&tiles(
        welt.path(),
        gpu.path(),
        &[&ausschnitt[..], &["--gpu", "on"]].concat(),
    ));
    assert!(!dateien(gpu.path()).is_empty(), "der Ausschnitt ist leer");
    assert_eq!(schnappschuss(cpu.path()), schnappschuss(gpu.path()));
}

/// Die Karte hat in diesem Lauf alles gezeichnet: die Zeile der Basis und
/// die jeder nativen Stufe nennen sie ohne „für n von m“, und nichts fiel
/// auf die CPU zurück. Sonst fiele ein Fehler der Karte nicht auf, die CPU
/// zeichnet dieselben Bytes. Gibt die Zahl der nativen Stufen zurück.
fn ganz_auf_der_karte(lauf: &Output) -> usize {
    let ausgabe = String::from_utf8_lossy(&gelungen(lauf).stdout);
    let zeilen: Vec<&str> = ausgabe
        .lines()
        .filter(|zeile| zeile.starts_with("Kacheln:") || zeile.contains("nativ bei scale"))
        .collect();
    assert!(
        zeilen
            .first()
            .is_some_and(|zeile| zeile.starts_with("Kacheln:")),
        "keine Zeile der Basis:\n{ausgabe}"
    );
    for zeile in &zeilen {
        assert!(
            zeile.contains("+ GPU") && !zeile.contains("+ GPU für"),
            "nicht alles auf der Karte: {zeile}\n{ausgabe}"
        );
    }
    assert!(
        !ausgabe.contains("ab hier zeichnet die CPU"),
        "die Karte fiel aus, die CPU hat gezeichnet:\n{ausgabe}"
    );
    zeilen.len() - 1
}

/// Ein `WGPU_ADAPTER_NAME`, zu dem kein Adapter passt, ist keine Panik:
/// `--gpu auto` zeichnet auf der CPU und sagt warum, `--gpu on` bricht mit
/// derselben Meldung ab.
#[test]
fn unbekannter_adaptername_ist_keine_panik() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
    for (modus, gelingt) in [("auto", true), ("on", false)] {
        let out = tempdir();
        let lauf = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
            .arg("--world")
            .arg(welt.path())
            .arg("--assets")
            .arg(assets_ref())
            .arg("--tiles")
            .arg(out.path())
            .args(["--gpu", modus])
            .env("WGPU_ADAPTER_NAME", "Gibt es nicht 4711")
            .output()
            .expect("terranova-render starten");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&lauf.stdout),
            String::from_utf8_lossy(&lauf.stderr)
        );
        assert!(!text.contains("panicked"), "--gpu {modus}:\n{text}");
        assert!(
            text.contains("WGPU_ADAPTER_NAME=gibt es nicht 4711"),
            "--gpu {modus}:\n{text}"
        );
        assert_eq!(lauf.status.success(), gelingt, "--gpu {modus}:\n{text}");
        if gelingt {
            assert_eq!(text.matches("GPU:").count(), 1, "--gpu {modus}:\n{text}");
        }
    }
}

/// Scheitert die Karte, hier schon beim Anlegen des Zeichners an einer
/// Grenze von 1 kB (`TERRANOVA_GPU_GRENZE`), zeichnet die CPU alles, und
/// das Log sagt es genau einmal. Auf stderr steht keine Panik, obwohl wgpu
/// jeden solchen Fehler mit einer meldet. Die Kacheln sind dieselben wie
/// mit `--gpu off`.
#[test]
fn versagende_karte_steht_einmal_im_log() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (1, 1)], gelaende);
    let cpu = neuer_baum("2x1-se");
    gelungen(&tiles(
        welt.path(),
        cpu.path(),
        &["--scale", "16", "--gpu", "off"],
    ));
    let gpu = neuer_baum("2x1-se");
    let lauf = Command::new(env!("CARGO_BIN_EXE_terranova-render"))
        .arg("--world")
        .arg(welt.path())
        .arg("--assets")
        .arg(assets_ref())
        .arg("--tiles")
        .arg(gpu.wurzel())
        .args(["--native-levels", "9", "--scale", "16", "--gpu", "on"])
        .env("TERRANOVA_GPU_GRENZE", "1024")
        .output()
        .expect("terranova-render starten");
    let fehler = String::from_utf8_lossy(&lauf.stderr);
    if !lauf.status.success() && fehler.contains("keine Grafikkarte gefunden") {
        common::ohne_gpu();
        return;
    }
    let ausgabe = String::from_utf8_lossy(&gelungen(&lauf).stdout);
    assert_eq!(
        ausgabe.matches("ab hier zeichnet die CPU").count(),
        1,
        "{ausgabe}"
    );
    assert!(
        !fehler.contains("panicked"),
        "gefangene Panik im Log:\n{fehler}"
    );
    assert_eq!(schnappschuss(cpu.path()), schnappschuss(gpu.path()));
}

/// Unter Windows trägt das Binär ein Manifest mit dem Segment-Heap, siehe
/// build.rs. Ohne ihn holt sich libwebp den Speicher jeder Kachel frisch
/// vom Windows-Heap, und das Kodieren staut sich auf vielen Threads.
#[cfg(all(windows, target_env = "msvc"))]
#[test]
fn binaer_bekommt_den_segment_heap() {
    let binaer = std::fs::read(env!("CARGO_BIN_EXE_terranova-render")).expect("Binär lesen");
    let eintrag = br#"<heapType xmlns="http://schemas.microsoft.com/SMI/2020/WindowsSettings">SegmentHeap</heapType>"#;
    assert!(
        binaer
            .windows(eintrag.len())
            .any(|stelle| stelle == eintrag),
        "kein Segment-Heap im Manifest des Binärs"
    );
}
