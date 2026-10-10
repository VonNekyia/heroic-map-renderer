//! Schreibt die Szene für die Insel im Banner des README und im Logo des
//! Plugins: eine schwebende Insel mit Baum, Teich und Laterne in einem
//! Chunk. Das Logo des Mods zeigt dieselbe Insel mit einer Fichte statt der
//! Eiche. Kein Test im engeren Sinn, deshalb `#[ignore]`. Aufruf und Weg
//! zum Bild: Skill `skills/doku-bilder-rendern/SKILL.md`.

mod common;

use std::path::Path;

#[test]
#[ignore]
fn logo_welt() {
    schreibe(|x, y, z| insel(x, y, z).unwrap_or_else(|| eiche(x, y, z)));
}

#[test]
#[ignore]
fn logo_welt_fichte() {
    schreibe(|x, y, z| insel(x, y, z).unwrap_or_else(|| fichte(x, y, z)));
}

fn schreibe(block: impl Fn(i32, i32, i32) -> &'static str) {
    let ziel = std::env::var("LOGO_WELT").expect("LOGO_WELT auf den Zielordner setzen");
    let dir = Path::new(&ziel);
    std::fs::create_dir_all(dir).unwrap();
    common::write_level_dat(dir);
    common::write_world_sections(dir, &[(0, 0)], 0..=1, block, |_, _| {
        Some("minecraft:plains")
    });
}

/// Die Insel ohne Baum: Mitte bei x = z = 8, oben 7 x 7 ohne Ecken, nach
/// unten spitz. `None` heisst: Hier entscheidet der Baum.
fn insel(x: i32, y: i32, z: i32) -> Option<&'static str> {
    let (dx, dz) = (x - 8, z - 8);
    let form =
        |r: i32| dx.abs() <= r && dz.abs() <= r && !(r > 0 && dx.abs() == r && dz.abs() == r);
    let teich = matches!((x, z), (6, 9) | (7, 9) | (7, 8));
    Some(match y {
        0 if form(0) => "minecraft:stone",
        1 if form(1) => "minecraft:stone",
        2 if form(2) && (x, z) == (10, 9) => "minecraft:iron_ore",
        2 if form(2) => "minecraft:stone",
        3 if form(3) && (x, z) == (7, 11) => "minecraft:coal_ore",
        3 if form(3) && (x, z) == (11, 7) => "minecraft:copper_ore",
        3 if form(3) => "minecraft:stone",
        4 if form(3) && teich => "minecraft:sand",
        4 if form(3) => "minecraft:dirt",
        5 if form(3) && teich => "minecraft:water[level=0]",
        5 if form(3) => "minecraft:grass_block[snowy=false]",
        6 => match (x, z) {
            (9, 10) => "minecraft:poppy",
            (10, 8) => "minecraft:dandelion",
            (6, 10) => "minecraft:oxeye_daisy",
            (8, 11) | (11, 9) | (9, 7) | (5, 8) => "minecraft:short_grass",
            (10, 10) => {
                "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]"
            }
            _ => return None,
        },
        7 if (x, z) == (10, 10) => "minecraft:lantern[hanging=false,waterlogged=false]",
        _ => return None,
    })
}

/// Die Eiche hinten, Stamm bei (6, 6..=9, 6): das Logo des Plugins.
fn eiche(x: i32, y: i32, z: i32) -> &'static str {
    let blatt = "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]";
    let (bx, bz) = (x - 6, z - 6);
    let krone =
        |r: i32| bx.abs() <= r && bz.abs() <= r && !(r > 1 && bx.abs() == r && bz.abs() == r);
    match y {
        6..=9 if (bx, bz) == (0, 0) => "minecraft:oak_log[axis=y]",
        8 | 9 if krone(2) => blatt,
        10 if krone(1) => blatt,
        11 if bx.abs() + bz.abs() <= 1 => blatt,
        _ => "minecraft:air",
    }
}

/// Die Fichte an derselben Stelle, Stamm bei (6, 6..=10, 6): das Logo des
/// Mods. Die Krone ein Kegel, der von unten nach oben schmaler wird, die
/// Spitze so hoch wie die Krone der Eiche: So passen beide Logos in
/// denselben Rahmen.
fn fichte(x: i32, y: i32, z: i32) -> &'static str {
    let blatt = "minecraft:spruce_leaves[distance=1,persistent=true,waterlogged=false]";
    let (bx, bz) = (x - 6, z - 6);
    let (ax, az) = (bx.abs(), bz.abs());
    match y {
        6..=10 if (bx, bz) == (0, 0) => "minecraft:spruce_log[axis=y]",
        // 5 x 5 ohne Ecken, eine Raute, 3 x 3, ein Kreuz, die Spitze.
        7 if ax <= 2 && az <= 2 && !(ax == 2 && az == 2) => blatt,
        8 if ax + az <= 2 => blatt,
        9 if ax <= 1 && az <= 1 => blatt,
        10 if ax + az <= 1 => blatt,
        11 if (bx, bz) == (0, 0) => blatt,
        _ => "minecraft:air",
    }
}
