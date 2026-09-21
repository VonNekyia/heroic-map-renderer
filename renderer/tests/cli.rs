//! Prüft den CLI-Einstieg selbst, nicht nur die Bibliothek.
//!
//! Die Bibliothekstests setzen das Bildrechteck direkt. Der Weg dorthin
//! führt aber über `--center`, und der ist eine eigene Fehlerquelle.

mod common;

use std::path::PathBuf;
use std::process::Command;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base")
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
