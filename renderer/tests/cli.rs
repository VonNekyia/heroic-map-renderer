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
use terranova_render::assets::Assets;
use terranova_render::render::{Projection, SpriteSet, TileId, pyramid, render_area, survey};
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

/// Verschwindet ein Chunk aus der Welt, etwa weil ein Editor ihn
/// zurückgesetzt hat, sieht der Vorlauf ihn nicht mehr. Seine alten
/// Kacheln müssen trotzdem weg, auf jeder Stufe: danach gleicht der Baum
/// einem frischen Export.
#[test]
fn verschwundener_chunk_verschwindet_auf_jeder_stufe() {
    let block = |x, y, z| match (x, y, z) {
        (8, 4, 8) => "minecraft:einfarbig",
        (104, 4, 104) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    };
    let alt = tempdir();
    common::write_world(alt.path(), &[(0, 0), (6, 6)], block);
    let neu = tempdir();
    common::write_world(neu.path(), &[(0, 0)], block);

    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    let vorher = dateien(baum.path());
    gelungen(&tiles(neu.path(), baum.path(), &["--scale", "16"]));
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let soll = schnappschuss(voll.path());
    assert!(
        vorher.len() > soll.len(),
        "der Chunk hatte keine eigenen Kacheln: {vorher:?}"
    );
    assert_eq!(schnappschuss(baum.path()), soll);
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
/// solange ein Block auf ganzen Pixeln liegt, also bis scale 4 — oder
/// genau die Verkleinerung ihrer vier Kinder. Und keine Kachel darf fehlen.
#[test]
fn pyramide_passt_auf_jeder_stufe_zu_ihren_kindern() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0), (2, 2)], gelaende);
    let out = tempdir();
    // scale 8: eine native Stufe (4), dann Verkleinerungen — beide Wege.
    gelungen(&tiles(welt.path(), out.path(), &["--scale", "8"]));

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
/// halten, nur weil der Vorlauf ihn nicht sieht.
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
/// Ausschnitt nur auf den gröberen Stufen.
#[test]
fn nachrendern_zeigt_auf_allen_stufen_denselben_stand() {
    let alt = tempdir();
    common::write_world(alt.path(), &[(2, 0), (4, 0)], |x, y, z| match (x, y, z) {
        (44, 4, 8) => "minecraft:einfarbig",
        _ => "minecraft:air",
    });
    let neu = tempdir();
    common::write_world(neu.path(), &[(2, 0), (4, 0)], |x, y, z| match (x, y, z) {
        (44, 4, 8) => "minecraft:einfarbig",
        (76, 4, 8) => "minecraft:blauwuerfel",
        _ => "minecraft:air",
    });

    let baum = tempdir();
    gelungen(&tiles(alt.path(), baum.path(), &["--scale", "16"]));
    gelungen(&tiles(
        neu.path(),
        baum.path(),
        &["--scale", "16", "--center", "44", "8", "--size", "4"],
    ));
    let voll = tempdir();
    gelungen(&tiles(neu.path(), voll.path(), &["--scale", "16"]));

    let nachher = schnappschuss(baum.path());
    let soll = schnappschuss(voll.path());
    assert_eq!(
        nachher.keys().collect::<Vec<_>>(),
        soll.keys().collect::<Vec<_>>()
    );
    for (rel, inhalt) in &soll {
        assert_eq!(&nachher[rel], inhalt, "{rel} zeigt einen anderen Stand");
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
    common::write_level_dat(erste.path(), 4_815_162_342);
    let zweite = tempdir();
    common::write_world(zweite.path(), &[(0, 0)], gelaende);
    common::write_level_dat(zweite.path(), 2_718_281_828);

    let out = tempdir();
    gelungen(&tiles(erste.path(), out.path(), &["--scale", "16"]));
    // `map.json` liegt öffentlich neben den Kacheln: den Seed selbst
    // verrät es nicht, nur seine Kennung.
    let karte = std::fs::read_to_string(out.path().join("map.json")).unwrap();
    assert!(karte.contains(r#""world": "f39bc820d10860f2""#), "{karte}");
    assert!(!karte.contains("4815162342"), "{karte}");
    let vorher = schnappschuss(out.path());
    let ausgabe = tiles(zweite.path(), out.path(), &["--scale", "16"]);
    assert!(!ausgabe.status.success(), "die fremde Welt lief durch");
    let meldung = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        meldung.contains("anderen Welt: Kennung dort f39bc820d10860f2"),
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

/// Ein Baum eines älteren Stands mit scale 6 lässt sich nicht fortsetzen:
/// `--scale 6` nimmt dieser Stand nicht mehr an. Die Meldung darf das
/// nicht raten.
#[test]
fn alter_scale_nennt_den_ausweg() {
    let welt = tempdir();
    common::write_world(welt.path(), &[(0, 0)], gelaende);
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
