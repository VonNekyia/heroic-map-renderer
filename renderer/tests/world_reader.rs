//! Prüft den Decoder gegen eine echte, aus der Zielwelt extrahierte Region
//! (Paper 26.2, DataVersion 4903): 2×2 vollständig generierte Chunks,
//! 40 KB. Die Sollwerte stammen aus einem unabhängigen Python-Decoder.

mod common;

use std::path::{Path, PathBuf};

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

/// Seed und Dimension einer Welt, wie `World` sie sieht.
fn herkunft(dir: &Path) -> (Option<i64>, Option<String>) {
    let world = World::open(dir).unwrap();
    (world.seed().unwrap(), world.dimension().map(str::to_string))
}

/// Seed und Dimension sind die Grundlage der Kennung im Kachelbaum. Den
/// Seed tragen alle Dimensionen einer Welt gleich, gelesen wird er an der
/// Weltwurzel, dem Verzeichnis mit `level.dat`: seit 26.1 bei Vanilla aus
/// `data/minecraft`, bei Paper aus der Oberwelt, bis 1.21 aus `level.dat`,
/// in dieser Reihenfolge. Eine Dimension darunter findet ihre Wurzel, eine
/// Kopie ohne sie hat keine Kennung.
#[test]
fn findet_seed_und_dimension_in_jedem_layout() {
    let oberwelt = || Some("minecraft:overworld".to_string());
    let nether = || Some("minecraft:the_nether".to_string());

    // Vanilla 26: Seed an der Wurzel, Regionen je Dimension.
    let vanilla = tempfile::tempdir().unwrap();
    let dims = vanilla.path().join("dimensions/minecraft");
    std::fs::create_dir_all(dims.join("overworld/region")).unwrap();
    std::fs::create_dir_all(dims.join("the_nether/region")).unwrap();
    common::write_level_dat_ohne_seed(vanilla.path());
    common::write_gen_settings(vanilla.path(), 7_331);
    assert_eq!(herkunft(vanilla.path()), (Some(7_331), oberwelt()));
    assert_eq!(herkunft(&dims.join("overworld")), (Some(7_331), oberwelt()));
    assert_eq!(herkunft(&dims.join("the_nether")), (Some(7_331), nether()));

    // Paper 26: Seed in jeder Dimension, gelesen wird der der Oberwelt.
    let paper = tempfile::tempdir().unwrap();
    let dims = paper.path().join("dimensions/minecraft");
    std::fs::create_dir_all(dims.join("overworld/region")).unwrap();
    std::fs::create_dir_all(dims.join("the_nether/region")).unwrap();
    common::write_level_dat_ohne_seed(paper.path());
    common::write_gen_settings(&dims.join("overworld"), -4_172_144_997_902_289_642);
    common::write_gen_settings(&dims.join("the_nether"), -4_172_144_997_902_289_642);
    let seed = Some(-4_172_144_997_902_289_642);
    assert_eq!(herkunft(paper.path()), (seed, oberwelt()));
    assert_eq!(herkunft(&dims.join("the_nether")), (seed, nether()));

    // Bis 1.21: Seed in level.dat, Nether und End als DIM-1 und DIM1, bei
    // Bukkit in einer eigenen Welt.
    let alt = tempfile::tempdir().unwrap();
    for sub in ["region", "DIM-1/region", "DIM1/region"] {
        std::fs::create_dir_all(alt.path().join(sub)).unwrap();
    }
    common::write_level_dat(alt.path(), 42);
    assert_eq!(herkunft(alt.path()), (Some(42), oberwelt()));
    assert_eq!(herkunft(&alt.path().join("DIM-1")), (Some(42), nether()));
    let ende = Some("minecraft:the_end".to_string());
    assert_eq!(herkunft(&alt.path().join("DIM1")), (Some(42), ende));
    let bukkit = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(bukkit.path().join("DIM-1/region")).unwrap();
    common::write_level_dat(bukkit.path(), 42);
    assert_eq!(herkunft(&bukkit.path().join("DIM-1")), (Some(42), nether()));

    // Die Reihenfolge der Orte: Vanilla vor Paper vor level.dat.
    let alle = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(alle.path().join("region")).unwrap();
    let paper_ort = alle.path().join("dimensions/minecraft/overworld");
    common::write_level_dat(alle.path(), 3);
    assert_eq!(herkunft(alle.path()).0, Some(3));
    common::write_gen_settings(&paper_ort, 2);
    assert_eq!(herkunft(alle.path()).0, Some(2));
    common::write_gen_settings(alle.path(), 1);
    assert_eq!(herkunft(alle.path()).0, Some(1));

    // Eine Kopie einer Dimension ohne ihre Wurzel.
    let kopie = tempfile::tempdir().unwrap();
    let the_nether = kopie.path().join("the_nether");
    std::fs::create_dir_all(the_nether.join("region")).unwrap();
    common::write_gen_settings(&the_nether, 7_331);
    assert_eq!(herkunft(&the_nether), (None, None));

    // Die Fixture hat nur Regionen.
    assert_eq!(world().seed().unwrap(), None);
}

/// Die Dimension steht so da, wie sie auf der Platte heisst: `..` ist kein
/// Name, ein relativer Pfad führt zur selben Welt, und unter Windows
/// öffnet `dim-1` dieselben Regionen wie `DIM-1`. Eine Kopie von
/// `level.dat` in einer Dimension macht sie nicht zur Oberwelt, und nicht
/// nur `minecraft` hat Dimensionen.
#[test]
fn dimension_wie_auf_der_platte() {
    let welt = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    for sub in [
        "region",
        "DIM-1/region",
        "dimensions/minecraft/the_nether/region",
        "dimensions/terralith/abgrund/region",
    ] {
        std::fs::create_dir_all(welt.path().join(sub)).unwrap();
    }
    common::write_level_dat(welt.path(), 42);
    let nether = || (Some(42), Some("minecraft:the_nether".to_string()));
    assert_eq!(herkunft(&welt.path().join("DIM-1/region/..")), nether());
    assert_eq!(herkunft(&relativ(&welt.path().join("DIM-1"))), nether());
    let abgrund = welt.path().join("dimensions/terralith/abgrund");
    let terralith = (Some(42), Some("terralith:abgrund".to_string()));
    assert_eq!(herkunft(&abgrund), terralith);
    // Nur eine Platte, die Grossbuchstaben nicht unterscheidet, findet
    // diese Pfade, also der Windows-Lauf.
    for anders in ["dim-1", "DIMENSIONS/MINECRAFT/THE_NETHER"] {
        let pfad = welt.path().join(anders);
        if pfad.is_dir() {
            assert_eq!(herkunft(&pfad), nether(), "{anders}");
        }
    }
    let the_nether = welt.path().join("dimensions/minecraft/the_nether");
    common::write_level_dat(&the_nether, 7);
    assert_eq!(herkunft(&the_nether), nether());
}

/// Der Weg vom Arbeitsverzeichnis zu `ziel`, damit ein Test einen
/// relativen Pfad übergibt. Beide liegen unter dem Target-Verzeichnis
/// derselben Platte.
fn relativ(ziel: &Path) -> PathBuf {
    let hier = std::env::current_dir().unwrap();
    let gleich = hier
        .components()
        .zip(ziel.components())
        .take_while(|(a, b)| a == b)
        .count();
    let mut weg = PathBuf::new();
    for _ in gleich..hier.components().count() {
        weg.push("..");
    }
    weg.extend(ziel.components().skip(gleich));
    assert!(weg.is_relative(), "{}", weg.display());
    weg
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
