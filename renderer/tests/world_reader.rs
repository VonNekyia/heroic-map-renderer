//! Prüft den Decoder gegen eine echte, aus der Zielwelt extrahierte Region
//! (Paper 26.2, DataVersion 4903): 2×2 vollständig generierte Chunks,
//! 40 KB. Die Sollwerte stammen aus einem unabhängigen Python-Decoder.

use std::path::PathBuf;

use terranova_render::world::World;

/// Chunk (577, 416) der Fixture-Region deckt x 9232..9247, z 6656..6671 ab.
const CX: i32 = 577;
const CZ: i32 = 416;

fn world() -> World {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/world");
    World::open(&dir).expect("Fixture-Welt öffnen")
}

#[test]
fn findet_region_im_dimensions_layout() {
    let world = world();
    assert!(world.region_dir().ends_with("overworld/region"));
    assert_eq!(world.regions().unwrap(), vec![(18, 13)]);
}

#[test]
fn dekodiert_blockstates() {
    let chunk = world().chunk(CX, CZ).unwrap().expect("Chunk vorhanden");
    assert_eq!((chunk.x, chunk.z), (CX, CZ));
    assert_eq!(chunk.data_version, 4903);
    assert_eq!(chunk.status, "minecraft:full");

    let block = |x, y, z| chunk.block_at(x, y, z).map(ToString::to_string);
    assert_eq!(block(9232, -64, 6656).as_deref(), Some("minecraft:bedrock"));
    assert_eq!(
        block(9235, -60, 6663).as_deref(),
        Some("minecraft:deepslate[axis=y]")
    );
    assert_eq!(
        block(9240, 0, 6664).as_deref(),
        Some("minecraft:deepslate[axis=y]")
    );
    assert_eq!(block(9240, 62, 6664).as_deref(), Some("minecraft:stone"));
    assert_eq!(block(9247, 40, 6671).as_deref(), Some("minecraft:andesite"));
    assert_eq!(block(9240, 200, 6664).as_deref(), Some("minecraft:air"));
}

#[test]
fn findet_obersten_block_und_biom() {
    let chunk = world().chunk(CX, CZ).unwrap().unwrap();
    let (y, block) = chunk.highest_block(9240, 6664).expect("Spalte nicht leer");
    assert_eq!(y, 68);
    assert_eq!(block.to_string(), "minecraft:grass_block[snowy=false]");
    assert_eq!(block.name(), "minecraft:grass_block");
    assert_eq!(block.prop("snowy"), Some("false"));
    assert_eq!(chunk.biome_at(9240, 68, 6664), Some("minecraft:savanna"));
}

/// Minecraft legt unterhalb der Welthöhe eine Section an, die nur Lichtdaten
/// enthält (hier Y=-5). Die darf den Decoder weder abbrechen lassen noch die
/// Welthöhe verfälschen.
#[test]
fn ueberspringt_sections_ohne_block_states() {
    let chunk = world().chunk(CX, CZ).unwrap().unwrap();
    assert_eq!(chunk.y_min(), -64);
    assert_eq!(chunk.y_max(), 319);
    assert_eq!(chunk.sections().len(), 24);
    assert!(chunk.section(-5).is_none());
    assert!(chunk.section(-4).is_some());
}

#[test]
fn leere_sections_werden_erkannt() {
    let chunk = world().chunk(CX, CZ).unwrap().unwrap();
    // oberhalb des Terrains ist alles Luft
    assert!(chunk.section(15).unwrap().is_empty());
    assert!(!chunk.section(4).unwrap().is_empty());
}

#[test]
fn nicht_generierte_chunks_sind_none() {
    // Chunk in derselben Region, aber ausserhalb der 2×2 des Fixtures
    assert!(world().chunk(CX + 8, CZ + 8).unwrap().is_none());
    // Region existiert gar nicht
    assert!(world().chunk(0, 0).unwrap().is_none());
}

#[test]
fn fremde_koordinaten_liefern_none() {
    let chunk = world().chunk(CX, CZ).unwrap().unwrap();
    assert!(chunk.block_at(0, 64, 0).is_none());
    assert!(chunk.highest_block(0, 0).is_none());
}
