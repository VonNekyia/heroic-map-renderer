//! Prüft den CLI-Einstieg selbst, nicht nur die Bibliothek.
//!
//! Die Bibliothekstests setzen das Bildrechteck direkt. Der Weg dorthin
//! führt aber über `--center`, und der ist eine eigene Fehlerquelle.

mod common;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

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

/// Alle geschriebenen Kacheln als `<x>/<y>.webp`, sortiert.
fn dateien(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(spalten) = std::fs::read_dir(dir) else {
        return out;
    };
    for spalte in spalten.flatten() {
        let name = spalte.file_name().to_string_lossy().into_owned();
        for datei in std::fs::read_dir(spalte.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            out.push(format!("{name}/{}", datei.file_name().to_string_lossy()));
        }
    }
    out.sort();
    out
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

    let geschrieben = dateien(teil.path());
    assert!(!geschrieben.is_empty(), "der Ausschnitt schrieb nichts");
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
