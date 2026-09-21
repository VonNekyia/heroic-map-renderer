//! Prüft den Metatile-Renderer an einer selbst gebauten Welt.
//!
//! Die Welt besteht aus Blöcken des Test-Assetbaums, ist also vollständig
//! kontrolliert und unabhängig von Mojang-Daten.

mod common;

use std::path::PathBuf;

use image::RgbaImage;
use tempfile::TempDir;
use terranova_render::assets::Assets;
use terranova_render::render::{Projection, ScreenRect, SpriteSet, render_area};
use terranova_render::world::{BlockState, World};

/// Die gebaute Welt reicht von y=0 bis y=15.
const Y_RANGE: (i32, i32) = (0, 15);

fn assets() -> Assets {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/assets-base");
    Assets::open(vec![base]).unwrap()
}

/// Eine Treppenlandschaft mit einem flachen Blatt obenauf.
fn gelaende(x: i32, y: i32, z: i32) -> &'static str {
    let hoehe = 3 + x.rem_euclid(16) / 4 + z.rem_euclid(16) / 4;
    if y < 3 {
        "minecraft:einfarbig"
    } else if y < hoehe {
        "minecraft:mit_overlay"
    } else if y == hoehe && (x + z).rem_euclid(5) == 0 {
        "minecraft:seerose"
    } else {
        "minecraft:air"
    }
}

/// Baut die Welt, sammelt ihre Blockstates und rendert den Ausschnitt.
fn render(dir: &TempDir, block: impl Fn(i32, i32, i32) -> &'static str, size: u32) -> RgbaImage {
    common::write_world(dir.path(), &[(0, 0), (1, 0), (0, 1), (1, 1)], block);
    let world = World::open(dir.path()).unwrap();

    let mut states = Vec::new();
    for cz in 0..2 {
        for cx in 0..2 {
            let chunk = world.chunk(cx, cz).unwrap().unwrap();
            for section in chunk.sections() {
                states.extend(section.blocks().palette().iter().cloned());
            }
        }
    }

    let projection = Projection::new(16);
    let sprites = SpriteSet::build(&mut assets(), &states, projection).unwrap();
    // Der Blockursprung liegt in der Bildmitte. Damit fällt Block
    // (8, 8, 8) genau dorthin — die Stelle, an der der Occlusion-Test
    // nachsieht.
    let rect = ScreenRect::centered(size, size);
    render_area(&world, &sprites, rect, Y_RANGE).unwrap()
}

fn tempdir() -> TempDir {
    tempfile::tempdir().expect("Temporärverzeichnis")
}

/// Goldbild: hält fest, wie der fertige Ausschnitt aussieht. Neu erzeugen
/// mit `UPDATE_GOLDEN=1 cargo test --test metatile`.
#[test]
fn goldbild_bleibt_gleich() {
    let bild = render(&tempdir(), gelaende, 128);
    let pfad = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/golden/metatile.png");

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        bild.save(&pfad).unwrap();
        return;
    }

    let gold = image::open(&pfad)
        .unwrap_or_else(|e| panic!("{} lesen: {e}", pfad.display()))
        .into_rgba8();
    assert_eq!(bild.dimensions(), gold.dimensions());

    let abweichend = bild
        .pixels()
        .zip(gold.pixels())
        .filter(|(a, b)| a != b)
        .count();
    if abweichend > 0 {
        // Neben dem Goldbild statt in %TEMP%: so kann CI das Bild als
        // Artefakt hochladen, wenn der Test fällt.
        let neu = pfad.with_file_name("metatile-ist.png");
        bild.save(&neu).ok();
        panic!(
            "{abweichend} von {} Pixeln weichen vom Goldbild ab. Aktuelles Bild: {}",
            bild.width() * bild.height(),
            neu.display()
        );
    }
}

/// Ein Block, dessen drei kamerazugewandte Nachbarn volle Blöcke sind, ist
/// nicht zu sehen — egal was er ist.
#[test]
fn eingeschlossene_bloecke_aendern_nichts() {
    let voll = |_x: i32, _y: i32, _z: i32| "minecraft:einfarbig";
    let mit_kern = |x: i32, y: i32, z: i32| {
        if (x, y, z) == (8, 8, 8) {
            "minecraft:stone"
        } else {
            "minecraft:einfarbig"
        }
    };

    let a = render(&tempdir(), voll, 96);
    let b = render(&tempdir(), mit_kern, 96);
    assert_eq!(
        a.as_raw(),
        b.as_raw(),
        "ein eingeschlossener Block darf nicht durchscheinen"
    );
}

/// Gegenprobe: an der Oberfläche macht derselbe Austausch sehr wohl einen
/// Unterschied. Sonst würde der Test oben auch bestehen, wenn gar nichts
/// gezeichnet wird.
#[test]
fn sichtbare_bloecke_aendern_das_bild() {
    let voll = |_x: i32, _y: i32, _z: i32| "minecraft:einfarbig";
    let mit_kuppe = |x: i32, y: i32, z: i32| {
        if y == 15 && (8..12).contains(&x) && (8..12).contains(&z) {
            "minecraft:stone"
        } else {
            "minecraft:einfarbig"
        }
    };

    let a = render(&tempdir(), voll, 96);
    let b = render(&tempdir(), mit_kuppe, 96);
    assert_ne!(a.as_raw(), b.as_raw());
}

/// Der Maleralgorithmus zeichnet von unten nach oben: was oben liegt,
/// übermalt was darunter liegt.
#[test]
fn hoehere_bloecke_uebermalen_tiefere() {
    let flach = |_x: i32, y: i32, _z: i32| {
        if y < 4 {
            "minecraft:einfarbig"
        } else {
            "minecraft:air"
        }
    };
    let mit_turm = |x: i32, y: i32, z: i32| {
        if y < 4 {
            "minecraft:einfarbig"
        } else if (x, z) == (16, 16) && y < 12 {
            "minecraft:stone"
        } else {
            "minecraft:air"
        }
    };

    let a = render(&tempdir(), flach, 128);
    let b = render(&tempdir(), mit_turm, 128);
    assert_ne!(a.as_raw(), b.as_raw(), "der Turm muss sichtbar sein");

    // Der Turm verdeckt Boden, der ohne ihn sichtbar wäre: die Zahl der
    // gedeckten Pixel darf dabei nicht sinken.
    let gedeckt = |bild: &RgbaImage| bild.pixels().filter(|p| p.0[3] > 0).count();
    assert!(gedeckt(&b) >= gedeckt(&a));
}

#[test]
fn leere_welt_ergibt_ein_leeres_bild() {
    let leer = |_x: i32, _y: i32, _z: i32| "minecraft:air";
    let bild = render(&tempdir(), leer, 64);
    assert!(bild.pixels().all(|p| p.0[3] == 0));
}

#[test]
fn blockstate_parsen_bleibt_kompatibel() {
    // Die Welt benutzt genau die Namen, die der Assetbaum kennt.
    assert!(BlockState::parse("minecraft:einfarbig").is_ok());
}
