//! Gemeinsame Hilfen für Tests, die Weltdaten erzeugen.
//!
//! Eine echte Welt lässt sich nicht ins Repository legen, und aus einem
//! Ausschnitt einer echten Welt lassen sich einzelne Blöcke nicht gezielt
//! setzen. Beides braucht dieser Baukasten.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const SECTOR: usize = 4096;
/// Blöcke je Section-Kante.
pub const SECTION: i32 = 16;

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
}

pub fn palette(names: &[&str]) -> Vec<PaletteEntry> {
    names
        .iter()
        .map(|n| PaletteEntry {
            name: (*n).to_string(),
        })
        .collect()
}

/// Packt Indizes so, wie Minecraft es tut: keine Überlappung über
/// Long-Grenzen hinweg.
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

// -------------------------------------------------------------- Weltenbau

/// Schreibt eine wohlgeformte Welt mit einer Section (y 0..15) je Chunk.
///
/// `block` wird mit Weltkoordinaten aufgerufen und liefert den Blocknamen.
/// Alle Chunks müssen in Region (0, 0) liegen, also Chunkkoordinaten 0..31
/// haben — mehr braucht kein Test.
pub fn write_world(
    dir: &Path,
    chunks: &[(i32, i32)],
    block: impl Fn(i32, i32, i32) -> &'static str,
) -> PathBuf {
    let region_dir = dir.join("region");
    std::fs::create_dir_all(&region_dir).expect("region-Verzeichnis");

    let mut header = vec![0u8; 2 * SECTOR];
    let mut sectors = Vec::new();
    let mut next = 2u32;

    for &(cx, cz) in chunks {
        assert!(
            (0..32).contains(&cx) && (0..32).contains(&cz),
            "Chunk ({cx}, {cz}) liegt nicht in Region (0, 0)"
        );

        let payload = chunk_nbt(cx, cz, vec![section(cx, cz, &block)]);
        let mut record = Vec::new();
        record.extend_from_slice(&(payload.len() as u32 + 1).to_be_bytes());
        record.push(3); // unkomprimiert
        record.extend_from_slice(&payload);
        record.resize(record.len().next_multiple_of(SECTOR), 0);

        let index = (cx + cz * 32) as usize * 4;
        header[index..index + 3].copy_from_slice(&next.to_be_bytes()[1..]);
        header[index + 3] = (record.len() / SECTOR) as u8;
        header[SECTOR + index..SECTOR + index + 4].copy_from_slice(&1i32.to_be_bytes());
        next += (record.len() / SECTOR) as u32;
        sectors.extend_from_slice(&record);
    }

    header.extend_from_slice(&sectors);
    std::fs::write(region_dir.join("r.0.0.mca"), header).expect("Regionsdatei schreiben");
    dir.to_path_buf()
}

/// Baut die Section Y=0 eines Chunks aus der Blockfunktion.
fn section(cx: i32, cz: i32, block: &impl Fn(i32, i32, i32) -> &'static str) -> SectionNbt {
    let mut names: Vec<&'static str> = Vec::new();
    let mut index_of: HashMap<&'static str, usize> = HashMap::new();
    let mut indices = vec![0usize; 4096];

    for y in 0..SECTION {
        for z in 0..SECTION {
            for x in 0..SECTION {
                let name = block(cx * SECTION + x, y, cz * SECTION + z);
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
        y: 0,
        block_states: BlockStatesNbt {
            palette: palette(&names),
            data: (names.len() > 1).then(|| packed(&indices, bits)),
        },
    }
}
