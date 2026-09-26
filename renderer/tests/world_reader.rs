//! Prüft den Decoder gegen eine echte, aus der Zielwelt extrahierte Region
//! (Paper 26.2, DataVersion 4903): 2×2 vollständig generierte Chunks,
//! 40 KB. Die Sollwerte stammen aus einem unabhängigen Python-Decoder.

mod common;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

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
/// Seed liest eine Dimension zuerst aus ihrer eigenen Datei, wie Paper sie
/// schreibt, sonst an der Weltwurzel, dem Verzeichnis mit `level.dat`, aus
/// `data/minecraft`, wie Vanilla. Eine Dimension darunter findet ihre
/// Wurzel, eine Kopie ohne sie hat keine Kennung.
#[test]
fn findet_seed_und_dimension_in_jedem_layout() {
    let oberwelt = || Some("minecraft:overworld".to_string());
    let nether = || Some("minecraft:the_nether".to_string());

    // Vanilla 26: Seed an der Wurzel, Regionen je Dimension.
    let vanilla = tempfile::tempdir().unwrap();
    let dims = vanilla.path().join("dimensions/minecraft");
    std::fs::create_dir_all(dims.join("overworld/region")).unwrap();
    std::fs::create_dir_all(dims.join("the_nether/region")).unwrap();
    common::write_level_dat(vanilla.path());
    common::write_gen_settings(vanilla.path(), 7_331);
    assert_eq!(herkunft(vanilla.path()), (Some(7_331), oberwelt()));
    assert_eq!(herkunft(&dims.join("overworld")), (Some(7_331), oberwelt()));
    assert_eq!(herkunft(&dims.join("the_nether")), (Some(7_331), nether()));

    // Paper 26: Seed in jeder Dimension, jede liest ihren eigenen. Eine
    // Plugin-Welt hat oft einen anderen als die Oberwelt.
    let paper = tempfile::tempdir().unwrap();
    let dims = paper.path().join("dimensions/minecraft");
    let plugin = paper.path().join("dimensions/terralith/abgrund");
    std::fs::create_dir_all(dims.join("overworld/region")).unwrap();
    std::fs::create_dir_all(dims.join("the_nether/region")).unwrap();
    std::fs::create_dir_all(plugin.join("region")).unwrap();
    common::write_level_dat(paper.path());
    common::write_gen_settings(&dims.join("overworld"), -4_172_144_997_902_289_642);
    common::write_gen_settings(&dims.join("the_nether"), 99);
    common::write_gen_settings(&plugin, 1_234);
    let seed = Some(-4_172_144_997_902_289_642);
    assert_eq!(herkunft(paper.path()), (seed, oberwelt()));
    assert_eq!(herkunft(&dims.join("the_nether")), (Some(99), nether()));
    let abgrund = Some("terralith:abgrund".to_string());
    assert_eq!(herkunft(&plugin), (Some(1_234), abgrund));

    // Die Reihenfolge der Orte: die Datei der Dimension, dann von der an
    // der Wurzel und der der Paper-Oberwelt die jüngere, bei gleichem Alter
    // die von Paper. Unter Paper bleibt an der Wurzel eine ältere liegen.
    let alle = tempfile::tempdir().unwrap();
    let the_nether = alle.path().join("dimensions/minecraft/the_nether");
    let paper_ort = alle.path().join("dimensions/minecraft/overworld");
    std::fs::create_dir_all(paper_ort.join("region")).unwrap();
    std::fs::create_dir_all(the_nether.join("region")).unwrap();
    common::write_level_dat(alle.path());
    assert_eq!(herkunft(&the_nether), (None, nether()));
    common::write_gen_settings(alle.path(), 1);
    assert_eq!(herkunft(&the_nether).0, Some(1));
    common::write_gen_settings(&paper_ort, 2);
    let alter = |dir: &Path, sekunden: u64| {
        File::options()
            .write(true)
            .open(dir.join("data/minecraft/world_gen_settings.dat"))
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(sekunden))
            .unwrap();
    };
    alter(alle.path(), 2_000);
    alter(&paper_ort, 1_000);
    assert_eq!(
        herkunft(&the_nether).0,
        Some(1),
        "die an der Wurzel ist jünger"
    );
    assert_eq!(
        herkunft(alle.path()).0,
        Some(2),
        "die Oberwelt hat ihre eigene"
    );
    alter(&paper_ort, 2_000);
    assert_eq!(herkunft(&the_nether).0, Some(2), "gleich alt");
    alter(&paper_ort, 3_000);
    assert_eq!(herkunft(&the_nether).0, Some(2), "die von Paper ist jünger");
    common::write_gen_settings(&the_nether, 4);
    alter(&the_nether, 0);
    assert_eq!(herkunft(&the_nether).0, Some(4), "die eigene zuerst");

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
/// öffnet `DIMENSIONS/MINECRAFT/THE_NETHER` dieselben Regionen wie
/// `dimensions/minecraft/the_nether`. Eine Kopie von `level.dat` in einer
/// Dimension macht sie nicht zur Oberwelt, und nicht nur `minecraft` hat
/// Dimensionen.
#[test]
fn dimension_wie_auf_der_platte() {
    let welt = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let the_nether = welt.path().join("dimensions/minecraft/the_nether");
    let abgrund = welt.path().join("dimensions/terralith/abgrund");
    for dimension in [&the_nether, &abgrund] {
        std::fs::create_dir_all(dimension.join("region")).unwrap();
    }
    common::write_wurzel(welt.path(), 42);
    let nether = || (Some(42), Some("minecraft:the_nether".to_string()));
    assert_eq!(herkunft(&the_nether.join("region/..")), nether());
    assert_eq!(herkunft(&relativ(&the_nether)), nether());
    let terralith = (Some(42), Some("terralith:abgrund".to_string()));
    assert_eq!(herkunft(&abgrund), terralith);
    // Nur eine Platte, die Grossbuchstaben nicht unterscheidet, findet
    // diesen Pfad, also der Windows-Lauf.
    let anders = welt.path().join("DIMENSIONS/MINECRAFT/THE_NETHER");
    if anders.is_dir() {
        assert_eq!(herkunft(&anders), nether());
    }
    common::write_level_dat(&the_nether);
    assert_eq!(herkunft(&the_nether), nether());
}

/// Liegt eine Dimension hinter einem Link, etwa auf einer anderen Platte,
/// führt ihr kanonischer Pfad aus der Welt hinaus. Dann zählt der Pfad, wie
/// er angegeben ist. Unter Windows eine Junction, die jeder anlegen darf,
/// sonst ein Symlink.
#[test]
fn dimension_hinter_einem_link() {
    let welt = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let draussen = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    std::fs::create_dir_all(welt.path().join("dimensions/minecraft")).unwrap();
    std::fs::create_dir_all(draussen.path().join("nether/region")).unwrap();
    common::write_wurzel(welt.path(), 42);
    let the_nether = welt.path().join("dimensions/minecraft/the_nether");
    common::link(&draussen.path().join("nether"), &the_nether);
    let nether = (Some(42), Some("minecraft:the_nether".to_string()));
    assert_eq!(herkunft(&the_nether), nether);
}

/// Die Wurzel steht in Meldungen, wie man sie schreibt, ohne das Präfix
/// `\\?\`, das `canonicalize` unter Windows voranstellt.
#[test]
fn meldung_nennt_die_wurzel_ohne_praefix() {
    let welt = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let daten = welt.path().join("data/minecraft");
    std::fs::create_dir_all(welt.path().join("dimensions/minecraft/overworld/region")).unwrap();
    std::fs::create_dir_all(&daten).unwrap();
    common::write_level_dat(welt.path());
    std::fs::write(daten.join("world_gen_settings.dat"), b"kein gzip").unwrap();
    let fehler = World::open(welt.path()).unwrap().seed().unwrap_err();
    let text = format!("{fehler:#}");
    assert!(text.contains("world_gen_settings.dat"), "{text}");
    assert!(!text.contains(r"\\?\"), "{text}");
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
