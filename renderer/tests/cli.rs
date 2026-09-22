//! Prüft den CLI-Einstieg selbst, nicht nur die Bibliothek.
//!
//! Die Bibliothekstests setzen das Bildrechteck direkt. Der Weg dorthin
//! führt aber über `--center`, und der ist eine eigene Fehlerquelle.

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::render::{TileId, pyramid};

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

/// Kachelexport über die Binärdatei.
fn tiles(welt: &Path, out: &Path, extra: &[&str]) -> Output {
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

/// Jede gröbere Zoomstufe muss genau die Verkleinerung ihrer vier Kinder
/// sein — und keine Kachel darf fehlen.
#[test]
fn pyramide_passt_auf_jeder_stufe_zu_ihren_kindern() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));

    let basis = max_zoom(out.path());
    assert!(basis > 0, "kein Stapel zu prüfen");
    assert!(!kacheln(out.path(), basis).is_empty());

    for z in (0..basis).rev() {
        let eltern = kacheln(out.path(), z);
        let kinder = kacheln(out.path(), z + 1);
        assert!(!eltern.is_empty(), "Zoom {z} ist leer");

        for (parent, pfad) in &eltern {
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
/// Die Elternkacheln am Rand des Ausschnitts haben Geschwister ausserhalb.
/// Wer beim Neubauen nur die Kacheln dieses Laufs berücksichtigt, schreibt
/// sie mit durchsichtigen Lücken zu — und `map.json` schrumpft auf den
/// Ausschnitt zusammen.
#[test]
fn nachrendern_in_einen_bestehenden_baum_aendert_nichts() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(2, 0), (4, 0)], |x, y, z| match (x, y, z) {
        (44, 4, 8) => "minecraft:einfarbig",
        (76, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    });

    let out = tempdir();
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "16"]));
    let vorher = schnappschuss(out.path());
    assert!(
        vorher.len() > 3,
        "zu wenig zum Vergleichen: {:?}",
        vorher.keys().collect::<Vec<_>>()
    );

    // Dieselbe Welt, nur ein Ausschnitt um den ersten Block, in dasselbe
    // Verzeichnis.
    gelungen(&tiles(
        welt.path(),
        out.path(),
        &["--scale", "16", "--center", "44", "8", "--size", "4"],
    ));

    let nachher = schnappschuss(out.path());
    assert_eq!(
        nachher.keys().collect::<Vec<_>>(),
        vorher.keys().collect::<Vec<_>>(),
        "der Baum hat andere Dateien als vorher"
    );
    for (rel, alt) in &vorher {
        assert_eq!(&nachher[rel], alt, "{rel} hat sich verändert");
    }
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
