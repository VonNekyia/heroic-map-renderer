use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;

use super::palette::{BlockState, PackedIndices, Paletted};

/// Blöcke pro Section-Kante.
pub const SECTION: i32 = 16;
/// Blöcke pro Section.
const BLOCKS_PER_SECTION: usize = 4096;
/// Biome werden in 4×4×4-Zellen gespeichert: 64 pro Section.
const BIOMES_PER_SECTION: usize = 64;

/// Das rohe NBT-Layout eines Chunks — nur die Felder, die zum Rendern nötig
/// sind. Alles andere (Heightmaps, block_entities, Licht, structures) wird
/// von serde verworfen.
#[derive(Deserialize)]
struct ChunkNbt {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(default)]
    sections: Vec<SectionNbt>,
}

#[derive(Deserialize)]
struct SectionNbt {
    #[serde(rename = "Y")]
    y: i8,
    /// Fehlt bei den Licht-Padding-Sections ober- und unterhalb der Welthöhe.
    block_states: Option<PalettedNbt<PaletteEntry>>,
    biomes: Option<PalettedNbt<String>>,
}

#[derive(Deserialize)]
struct PalettedNbt<T> {
    palette: Vec<T>,
    /// Fehlt, wenn die ganze Section aus einem einzigen Wert besteht.
    data: Option<fastnbt::LongArray>,
}

#[derive(Deserialize)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties")]
    properties: Option<HashMap<String, String>>,
}

/// Eine 16×16×16-Section eines Chunks.
#[derive(Debug)]
pub struct Section {
    pub y: i8,
    blocks: Paletted<BlockState>,
    biomes: Paletted<String>,
}

impl Section {
    /// Blockstate an lokaler Position (0..16 je Achse).
    pub fn block(&self, x: i32, y: i32, z: i32) -> Option<&BlockState> {
        self.blocks
            .get(((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize)
    }

    /// Biom an lokaler Position (0..16 je Achse), aufgelöst auf 4×4×4-Zellen.
    pub fn biome(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        self.biomes
            .get((((y & 15) / 4) * 16 + ((z & 15) / 4) * 4 + (x & 15) / 4) as usize)
            .map(String::as_str)
    }

    pub fn blocks(&self) -> &Paletted<BlockState> {
        &self.blocks
    }

    /// True, wenn die Section komplett aus Luft besteht — der billigste
    /// Filter, den der Renderer hat.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_uniform() && self.blocks.palette().first().is_none_or(BlockState::is_air)
    }
}

/// Ein dekodierter Chunk: nur die Daten, die zum Rendern nötig sind.
#[derive(Debug)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub data_version: i32,
    pub status: String,
    /// Aufsteigend nach `y` sortiert.
    sections: Vec<Section>,
}

impl Chunk {
    /// Dekodiert einen Chunk aus unkomprimiertem NBT.
    pub fn decode(nbt: &[u8]) -> Result<Chunk> {
        let raw: ChunkNbt = fastnbt::from_bytes(nbt).context("Chunk-NBT lesen")?;

        let mut sections = Vec::with_capacity(raw.sections.len());
        for section in raw.sections {
            if let Some(section) = decode_section(section)? {
                sections.push(section);
            }
        }
        sections.sort_by_key(|s| s.y);

        Ok(Chunk {
            x: raw.x_pos,
            z: raw.z_pos,
            data_version: raw.data_version,
            status: raw.status.unwrap_or_default(),
            sections,
        })
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn section(&self, section_y: i8) -> Option<&Section> {
        self.sections
            .binary_search_by_key(&section_y, |s| s.y)
            .ok()
            .map(|i| &self.sections[i])
    }

    /// Unterste Blockkoordinate, die dieser Chunk abdeckt.
    pub fn y_min(&self) -> i32 {
        self.sections.first().map_or(0, |s| s.y as i32 * SECTION)
    }

    /// Oberste Blockkoordinate (inklusive).
    pub fn y_max(&self) -> i32 {
        self.sections
            .last()
            .map_or(0, |s| s.y as i32 * SECTION + SECTION - 1)
    }

    /// Blockstate an einer **Welt**koordinate. `None`, wenn die Koordinate
    /// nicht in diesem Chunk liegt oder ausserhalb der Welthöhe.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Option<&BlockState> {
        self.section_for(x, y, z)?.block(x, y, z)
    }

    /// Biom an einer Weltkoordinate.
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        self.section_for(x, y, z)?.biome(x, y, z)
    }

    /// Oberster Block der Spalte, der nicht Luft ist.
    pub fn highest_block(&self, x: i32, z: i32) -> Option<(i32, &BlockState)> {
        if !self.contains_column(x, z) {
            return None;
        }
        for section in self.sections.iter().rev() {
            if section.is_empty() {
                continue;
            }
            for local_y in (0..SECTION).rev() {
                // `None` wäre ein unauflösbarer Palettenindex. Der Decoder
                // lässt so etwas nicht durch; falls doch, darf eine einzelne
                // kaputte Position nicht die ganze Spalte als leer melden.
                let Some(block) = section.block(x, local_y, z) else {
                    continue;
                };
                if !block.is_air() {
                    return Some((section.y as i32 * SECTION + local_y, block));
                }
            }
        }
        None
    }

    fn contains_column(&self, x: i32, z: i32) -> bool {
        x >> 4 == self.x && z >> 4 == self.z
    }

    fn section_for(&self, x: i32, y: i32, z: i32) -> Option<&Section> {
        if !self.contains_column(x, z) {
            return None;
        }
        self.section(i8::try_from(y >> 4).ok()?)
    }
}

/// `None` für Sections ohne `block_states` — Minecraft legt ober- und
/// unterhalb der Welthöhe Sections an, die nur Lichtdaten enthalten.
fn decode_section(nbt: SectionNbt) -> Result<Option<Section>> {
    let y = nbt.y;
    let Some(block_states) = nbt.block_states else {
        return Ok(None);
    };

    let palette = block_states
        .palette
        .into_iter()
        .map(|entry| {
            BlockState::new(
                entry.name,
                entry.properties.unwrap_or_default().into_iter().collect(),
            )
        })
        .collect();

    let blocks = paletted(
        palette,
        block_states.data,
        BLOCKS_PER_SECTION,
        4,
        y,
        "block_states",
    )?;

    let biomes = match nbt.biomes {
        None => Paletted::new(Vec::new(), None),
        Some(biomes) => paletted(
            biomes.palette,
            biomes.data,
            BIOMES_PER_SECTION,
            1,
            y,
            "biomes",
        )?,
    };

    Ok(Some(Section { y, blocks, biomes }))
}

fn paletted<T>(
    palette: Vec<T>,
    data: Option<fastnbt::LongArray>,
    entries: usize,
    min_bits: u32,
    y: i8,
    what: &str,
) -> Result<Paletted<T>> {
    if palette.is_empty() {
        bail!("Section {y}: {what} hat eine leere Palette");
    }

    let Some(data) = data else {
        // Minecraft lässt `data` nur weg, wenn die ganze Section aus einem
        // einzigen Wert besteht. Fehlt es bei größerer Palette, sind die
        // Indizes verloren — die Section still mit dem ersten Eintrag zu
        // füllen würde falsches Terrain erzeugen statt einen Fehler.
        if palette.len() > 1 {
            bail!(
                "Section {y}: {what} hat {} Paletteneinträge, aber keine Indexdaten",
                palette.len()
            );
        }
        return Ok(Paletted::new(palette, None));
    };

    let indices = PackedIndices::new(data.into_inner(), palette.len(), min_bits);
    let expected = indices.expected_longs(entries);
    if indices.longs() != expected {
        bail!(
            "Section {y}: {what} hat {} Longs, erwartet {expected} für {} Paletteneinträge",
            indices.longs(),
            palette.len()
        );
    }
    if let Some(index) = indices.first_index_beyond(entries, palette.len()) {
        bail!(
            "Section {y}: {what} verweist auf Paletten-Index {index}, die Palette hat nur {} Einträge",
            palette.len()
        );
    }
    Ok(Paletted::new(palette, Some(indices)))
}
