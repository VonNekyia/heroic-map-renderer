//! Schreibt die Szene für die Insel im Banner des README: eine schwebende
//! Insel mit Baum, Teich und Laterne in einem Chunk. Kein Test im engeren
//! Sinn, deshalb `#[ignore]`. Aufruf und Weg zum Bild: Skill
//! `skills/doku-bilder-rendern/SKILL.md`.

mod common;

use std::path::Path;

#[test]
#[ignore]
fn logo_welt() {
    let ziel = std::env::var("LOGO_WELT").expect("LOGO_WELT auf den Zielordner setzen");
    let dir = Path::new(&ziel);
    std::fs::create_dir_all(dir).unwrap();
    common::write_level_dat(dir);
    common::write_world_sections(dir, &[(0, 0)], 0..=1, insel, |_, _| {
        Some("minecraft:plains")
    });
}

/// Mitte bei x = z = 8. Oben 7 x 7 ohne Ecken, nach unten spitz.
fn insel(x: i32, y: i32, z: i32) -> &'static str {
    let (dx, dz) = (x - 8, z - 8);
    let form =
        |r: i32| dx.abs() <= r && dz.abs() <= r && !(r > 0 && dx.abs() == r && dz.abs() == r);
    let blatt = "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]";
    let teich = matches!((x, z), (6, 9) | (7, 9) | (7, 8));
    // Baum hinten: Stamm bei (6, 6..=9, 6).
    let (bx, bz) = (x - 6, z - 6);
    let krone =
        |r: i32| bx.abs() <= r && bz.abs() <= r && !(r > 1 && bx.abs() == r && bz.abs() == r);
    match y {
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
        6 | 7 if (x, z) == (6, 6) => "minecraft:oak_log[axis=y]",
        6 => match (x, z) {
            (9, 10) => "minecraft:poppy",
            (10, 8) => "minecraft:dandelion",
            (6, 10) => "minecraft:oxeye_daisy",
            (8, 11) | (11, 9) | (9, 7) | (5, 8) => "minecraft:short_grass",
            (10, 10) => {
                "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]"
            }
            _ => "minecraft:air",
        },
        7 if (x, z) == (10, 10) => "minecraft:lantern[hanging=false,waterlogged=false]",
        8 | 9 if (bx, bz) == (0, 0) => "minecraft:oak_log[axis=y]",
        8 | 9 if krone(2) => blatt,
        10 if krone(1) => blatt,
        11 if bx.abs() + bz.abs() <= 1 => blatt,
        _ => "minecraft:air",
    }
}
