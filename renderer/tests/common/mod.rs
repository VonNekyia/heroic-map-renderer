//! Gemeinsame Hilfen für die Tests: Weltdaten erzeugen und Links anlegen.
//!
//! Eine echte Welt lässt sich nicht ins Repository legen, und aus einem
//! Ausschnitt einer echten Welt lassen sich einzelne Blöcke nicht gezielt
//! setzen. Beides braucht dieser Baukasten.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use terranova_render::world::World;

pub const SECTOR: usize = 4096;
/// Blöcke je Section-Kante.
pub const SECTION: i32 = 16;

/// Legt `pfad` als Link auf das Verzeichnis `ziel` an: unter Windows eine
/// Junction, die jeder anlegen darf, sonst einen Symlink. `mklink` nähme
/// einen Schrägstrich im Pfad als Schalter, `absolute` setzt Backslashes.
pub fn link(ziel: &Path, pfad: &Path) {
    #[cfg(windows)]
    {
        let ausgabe = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(std::path::absolute(pfad).unwrap())
            .arg(std::path::absolute(ziel).unwrap())
            .output()
            .unwrap();
        assert!(
            ausgabe.status.success(),
            "{}",
            String::from_utf8_lossy(&ausgabe.stderr)
        );
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(ziel, pfad).unwrap();
}

// ---------------------------------------------------------------- NBT-Bau

#[derive(Serialize)]
pub struct ChunkNbt {
    #[serde(rename = "DataVersion")]
    pub data_version: i32,
    #[serde(rename = "xPos")]
    pub x_pos: i32,
    #[serde(rename = "zPos")]
    pub z_pos: i32,
    #[serde(rename = "Status")]
    pub status: String,
    pub sections: Vec<SectionNbt>,
}

#[derive(Serialize)]
pub struct SectionNbt {
    #[serde(rename = "Y")]
    pub y: i8,
    pub block_states: BlockStatesNbt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biomes: Option<BiomesNbt>,
}

/// Biome einer Section: ein Wert für alle 64 Zellen, deshalb ohne `data`.
#[derive(Serialize)]
pub struct BiomesNbt {
    pub palette: Vec<String>,
}

#[derive(Serialize)]
pub struct BlockStatesNbt {
    pub palette: Vec<PaletteEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<fastnbt::LongArray>,
}

#[derive(Serialize)]
pub struct PaletteEntry {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
}

/// Paletteneinträge aus Blocknamen, wahlweise mit Eigenschaften wie
/// `minecraft:oak_fence[north=true,waterlogged=true]`.
pub fn palette(names: &[&str]) -> Vec<PaletteEntry> {
    names
        .iter()
        .map(|full| {
            let (name, props) = match full.split_once('[') {
                Some((name, rest)) => (name, rest.trim_end_matches(']')),
                None => (*full, ""),
            };
            PaletteEntry {
                name: name.to_string(),
                properties: (!props.is_empty()).then(|| {
                    props
                        .split(',')
                        .filter_map(|kv| kv.split_once('='))
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect()
                }),
            }
        })
        .collect()
}

pub fn packed(entries: &[usize], bits: u32) -> fastnbt::LongArray {
    let per_long = 64 / bits as usize;
    let mut longs = vec![0i64; entries.len().div_ceil(per_long)];
    for (i, &v) in entries.iter().enumerate() {
        longs[i / per_long] |= ((v as u64) << ((i % per_long) * bits as usize)) as i64;
    }
    fastnbt::LongArray::new(longs)
}

pub fn chunk_nbt(cx: i32, cz: i32, sections: Vec<SectionNbt>) -> Vec<u8> {
    fastnbt::to_bytes(&ChunkNbt {
        data_version: 4903,
        x_pos: cx,
        z_pos: cz,
        status: "minecraft:full".to_string(),
        sections,
    })
    .expect("NBT serialisieren")
}

/// Wo der Seed liegt, seit 26.1. Die Datei trägt mehr, der Renderer liest
/// nur den Seed.
#[derive(Serialize)]
struct GenSettingsDat {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    data: SeedNbt,
}

#[derive(Serialize)]
struct SeedNbt {
    seed: i64,
}

fn write_gzip_nbt(path: &Path, value: &impl Serialize) {
    use std::io::Write;
    std::fs::create_dir_all(path.parent().unwrap()).expect("Verzeichnis anlegen");
    let mut gz = flate2::write::GzEncoder::new(
        std::fs::File::create(path).expect("NBT-Datei anlegen"),
        flate2::Compression::default(),
    );
    gz.write_all(&fastnbt::to_bytes(value).expect("NBT serialisieren"))
        .expect("NBT schreiben");
    gz.finish().expect("gzip abschliessen");
}

/// `level.dat`, die Marke der Weltwurzel. Der Renderer liest nichts
/// daraus: den Seed legt Minecraft seit 26.1 in `world_gen_settings.dat` ab.
pub fn write_level_dat(world: &Path) {
    std::fs::write(world.join("level.dat"), b"").expect("level.dat anlegen");
}

/// Eine Weltwurzel mit ihrem Seed, wie Vanilla sie schreibt.
pub fn write_wurzel(world: &Path, seed: i64) {
    write_level_dat(world);
    write_gen_settings(world, seed);
}

/// `world_gen_settings.dat` mit dem Seed in `<dir>/data/minecraft`. Vanilla
/// schreibt sie seit 26.1 in die Weltwurzel, Paper in jede Dimension.
pub fn write_gen_settings(dir: &Path, seed: i64) {
    let settings = GenSettingsDat {
        data_version: 4903,
        data: SeedNbt { seed },
    };
    write_gzip_nbt(
        &dir.join("data/minecraft/world_gen_settings.dat"),
        &settings,
    );
}

// -------------------------------------------------------------- Weltenbau

/// Schreibt eine wohlgeformte Welt mit einer Section (y 0..15) je Chunk.
///
/// `block` wird mit Weltkoordinaten aufgerufen und liefert den Blocknamen.
/// Alle Chunks müssen in dieselbe Region fallen — mehr braucht kein Test.
pub fn write_world(
    dir: &Path,
    chunks: &[(i32, i32)],
    block: impl Fn(i32, i32, i32) -> &'static str,
) -> PathBuf {
    write_world_in(dir, chunks, block, |_, _| None)
}

/// Wie `write_world`, dazu ein Biom je Chunk — oder keines, dann fehlt der
/// Eintrag. So eine Section liest Vanilla 26.2 als plains
/// (`SerializableChunkData.parse` nimmt dann `createForBiomes()`), der
/// Renderer genauso.
pub fn write_world_in(
    dir: &Path,
    chunks: &[(i32, i32)],
    block: impl Fn(i32, i32, i32) -> &'static str,
    biome: impl Fn(i32, i32) -> Option<&'static str>,
) -> PathBuf {
    write_region(dir, chunks, 0..=0, block, biome)
}

/// Wie `write_world_in`, aber mit diesen Sections je Chunk statt nur Y=0:
/// für Szenen über Section-Grenzen hinweg.
pub fn write_world_sections(
    dir: &Path,
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    block: impl Fn(i32, i32, i32) -> &'static str,
    biome: impl Fn(i32, i32) -> Option<&'static str>,
) -> PathBuf {
    write_region(dir, chunks, sections, block, biome)
}

fn write_region(
    dir: &Path,
    chunks: &[(i32, i32)],
    sections: std::ops::RangeInclusive<i8>,
    block: impl Fn(i32, i32, i32) -> &'static str,
    biome: impl Fn(i32, i32) -> Option<&'static str>,
) -> PathBuf {
    let region_dir = dir.join("region");
    std::fs::create_dir_all(&region_dir).expect("region-Verzeichnis");

    let (rx, rz) = (chunks[0].0 >> 5, chunks[0].1 >> 5);
    let mut header = vec![0u8; 2 * SECTOR];
    let mut sectors = Vec::new();
    let mut next = 2u32;

    for &(cx, cz) in chunks {
        assert!(
            (cx >> 5, cz >> 5) == (rx, rz),
            "Chunk ({cx}, {cz}) liegt nicht in Region ({rx}, {rz})"
        );

        let payload = chunk_nbt(
            cx,
            cz,
            sections
                .clone()
                .map(|sy| section(cx, cz, sy, &block, biome(cx, cz)))
                .collect(),
        );
        let mut record = Vec::new();
        record.extend_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
        record.push(3); // unkomprimiert
        record.extend_from_slice(&payload);
        record.resize(record.len().next_multiple_of(SECTOR), 0);

        let index = (cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize * 4;
        header[index..index + 3].copy_from_slice(&next.to_be_bytes()[1..]);
        header[index + 3] = (record.len() / SECTOR) as u8;
        header[SECTOR + index..SECTOR + index + 4].copy_from_slice(&1i32.to_be_bytes());
        next += (record.len() / SECTOR) as u32;
        sectors.extend_from_slice(&record);
    }

    header.extend_from_slice(&sectors);
    std::fs::write(region_dir.join(format!("r.{rx}.{rz}.mca")), header)
        .expect("Regionsdatei schreiben");
    dir.to_path_buf()
}

/// Baut die Section `sy` eines Chunks aus der Blockfunktion.
fn section(
    cx: i32,
    cz: i32,
    sy: i8,
    block: &impl Fn(i32, i32, i32) -> &'static str,
    biome: Option<&'static str>,
) -> SectionNbt {
    let mut names: Vec<&'static str> = Vec::new();
    let mut index_of: HashMap<&'static str, usize> = HashMap::new();
    let mut indices = vec![0usize; 4096];

    for y in 0..SECTION {
        for z in 0..SECTION {
            for x in 0..SECTION {
                let name = block(cx * SECTION + x, sy as i32 * SECTION + y, cz * SECTION + z);
                let next = names.len();
                let index = *index_of.entry(name).or_insert_with(|| {
                    names.push(name);
                    next
                });
                indices[(y * 256 + z * 16 + x) as usize] = index;
            }
        }
    }

    let bits = (64 - (names.len().max(2) as u64 - 1).leading_zeros()).max(4);
    SectionNbt {
        y: sy,
        block_states: BlockStatesNbt {
            palette: palette(&names),
            data: (names.len() > 1).then(|| packed(&indices, bits)),
        },
        biomes: biome.map(|name| BiomesNbt {
            palette: vec![name.to_string()],
        }),
    }
}

// ------------------------------------------------------------ Grafikkarte

/// Ohne Grafikkarte, auch ohne Software-Adapter, übergehen sich die
/// GPU-Tests und sagen es. In der CI steht `TERRANOVA_GPU_PFLICHT`: dort
/// ist ein fehlender Adapter ein Fehler, sonst bestünde jeder GPU-Test
/// still.
pub fn ohne_gpu() {
    assert!(
        std::env::var_os("TERRANOVA_GPU_PFLICHT").is_none(),
        "kein GPU-Adapter, aber TERRANOVA_GPU_PFLICHT ist gesetzt"
    );
    eprintln!("kein GPU-Adapter, auch kein Software-Adapter — Test übersprungen");
}

// ------------------------------------------------------ Szene mit allem

/// Höhenband der Szene aus [`szene`]: vier Sections, von y=-16 bis 47.
pub const SZENE_Y: (i32, i32) = (-16, 47);

/// Eine Szene mit allem, woran das Verdecken scheitern kann. Sie reicht
/// über vier Chunks in zwei Biomen und vier Sections, die unterste
/// einheitlich aus Stein, damit auch die Ränder zählen, an denen eine Maske
/// aus dem Nachbarchunk oder der Section darüber kommt, und die Fassungen
/// je Biom, die `in_biome` wählt: Gras an einer Ecke,
/// ein Becken über Chunk- und Section-Grenzen, mit einem Dach, unter dem
/// die Oberfläche tiefer liegt als der Boden des Dachs, und zwei
/// Wassertaschen unter Stein, die zu einer Seite an Stein grenzen und zur
/// anderen an Wasser mit Wasser darüber: ihre Oberfläche ragt in die Seite
/// zum Wasser hinein und scheint durch. Dazu Glas im Wasser, Lava in
/// Stufen und unter Lava, ein Lavasee mit Wänden nach +x und +z, eine
/// Lavatasche wie die Wassertaschen, zwei Lavasäulen, die zur einen Seite
/// über einer Stufe stehen und zur anderen an Stein grenzen, zwei weitere
/// so an den Rändern eines Chunks nach +x und +z, Lava mit Luft darüber
/// am oberen Rand einer Section, ein 15/16 hoher Block wie Ackerboden
/// neben Lava und gestapelt, Platten, Kuchen, eine Seerose, ein gefluteter
/// Zaun, eine Blasensäule, Säulen durch beide Section-Grenzen und Modelle,
/// die in Nachbarwürfel ragen, eines davon mit seinem oberen Teil in einem
/// verdeckten Würfel, dazu ein Block, der knapp über seinen Umriss ragt und
/// selbst verdeckt ist: beide zeichnen je Pixel neben ihrem Würfel, die kein
/// Nachbar deckt.
pub fn szene(x: i32, y: i32, z: i32) -> &'static str {
    match (x, y, z) {
        (0..=5, 2, 26..=31) | (26..=31, 2, 0..=2) => "minecraft:grass_block",
        (_, ..=2, _) => "minecraft:einfarbig",
        (27, 31, 21) | (27, 31..=32, 22) | (24, 31, 26) | (25, 31..=32, 26) => "minecraft:water",
        (27, 30..=32, 21..=22) | (28, 31, 21) => "minecraft:einfarbig",
        (24..=25, 30, 26) | (24, 32, 26) | (24, 31, 27) => "minecraft:einfarbig",
        (10..=13, 21, 10..=13) => "minecraft:einfarbig",
        (20, 3..=25, 8) => "minecraft:durchsichtig",
        (14, 12, 14) => "minecraft:oak_fence[waterlogged=true]",
        (18, 3..=20, 18) => "minecraft:bubble_column",
        (16, 3, 4) | (15, 5, 16) => "minecraft:ueberhang",
        (6..=25, 3..=21, 6..=25) => "minecraft:water",
        (8, 22, 20) => "minecraft:seerose",
        (2..=4, 3, 20..=23) | (3, 4, 21) => "minecraft:lava",
        (2, 4, 24) | (4, 3, 24) => "minecraft:lava[level=2]",
        (0..=3, 3..=5, 0..=3) | (29, 3, 1) | (29, 3..=4, 2) => "minecraft:lava",
        (1, 3..=4, 8) | (2, 3, 8) | (1, 3..=4, 12) | (1, 3, 13) => "minecraft:lava",
        (1, 3, 9) | (2, 3, 12) => "minecraft:einfarbig",
        (15, 8..=9, 29) | (16, 8, 29) | (29, 8..=9, 15) | (29, 8, 16) => "minecraft:lava",
        (15, 8, 30) | (30, 8, 15) => "minecraft:einfarbig",
        (2, 15, 29) => "minecraft:lava",
        (3, 15, 29) | (2, 15, 30) => "minecraft:einfarbig",
        (4, 3..=6, 0..=4) | (0..=3, 3..=6, 4) => "minecraft:einfarbig",
        (29, 4, 1) | (30, 3..=4, 1..=2) | (29, 5, 2) | (29, 3..=4, 3) => "minecraft:einfarbig",
        (5, 3, 20..=22) | (26, 3..=6, 3..=5) => "minecraft:ackerboden",
        (28, 3..=40, 28) | (27, 15..=17, 27) | (27, 31..=33, 26) => "minecraft:einfarbig",
        (17, 3, 28) | (16, 31, 2) => "minecraft:turm",
        (17, 32, 2) | (16, 32, 3) | (16, 33, 2) => "minecraft:einfarbig",
        (1, 3, 16) => "minecraft:rand",
        (2, 3, 16) | (1, 3, 17) => "minecraft:einfarbig",
        (1, 4, 16) => "minecraft:boden",
        (29, 3, 10) => "minecraft:obere_platte",
        (29, 3, 12) => "minecraft:untere_platte",
        (30, 3, 14) => "minecraft:kuchen",
        _ => "minecraft:air",
    }
}

/// Schreibt die Szene aus [`szene`] über vier Chunks: der bei x=0 in
/// plains, die anderen in frozen.
pub fn write_szene(dir: &Path) -> World {
    let chunks = [(0, 0), (1, 0), (0, 1), (1, 1)];
    let biom = |cx: i32, _: i32| {
        Some(if cx == 0 {
            "minecraft:plains"
        } else {
            "minecraft:frozen"
        })
    };
    write_world_sections(dir, &chunks, -1..=2, szene, biom);
    World::open(dir).unwrap()
}

/// Die Biomdaten der Tests, für `Assets::load_biomes`.
pub fn biomdaten() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data-base")
}
