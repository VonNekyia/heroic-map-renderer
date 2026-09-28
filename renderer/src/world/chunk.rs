use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;

use super::palette::{BlockState, PackedIndices, Paletted};

/// Blöcke pro Section-Kante.
pub const SECTION: i32 = 16;
/// Blöcke pro Section.
const BLOCKS_PER_SECTION: usize = 4096;
/// Biome werden in 4×4×4-Zellen gespeichert: 64 pro Section.
const BIOMES_PER_SECTION: usize = 64;

/// Das rohe NBT-Layout eines Chunks — nur die Felder, die zum Rendern nötig
/// sind. Alles andere (die übrigen Heightmaps, block_entities, Licht,
/// structures) wird von serde verworfen.
#[derive(Deserialize)]
struct ChunkNbt {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
    /// Die unterste Section der Welt, nicht des Chunks.
    #[serde(rename = "yPos")]
    y_pos: Option<i32>,
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "Heightmaps", default)]
    heightmaps: HeightmapsNbt,
    #[serde(default)]
    sections: Vec<SectionNbt>,
}

#[derive(Deserialize, Default)]
struct HeightmapsNbt {
    #[serde(rename = "WORLD_SURFACE")]
    world_surface: Option<fastnbt::LongArray>,
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
    /// Sortiert, wie `BlockState` sie will.
    #[serde(rename = "Properties", default)]
    properties: BTreeMap<String, String>,
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
        self.blocks.get(Section::offset(x, y, z))
    }

    /// Palettenindex an lokaler Position.
    pub fn slot(&self, x: i32, y: i32, z: i32) -> usize {
        self.blocks.index(Section::offset(x, y, z))
    }

    fn offset(x: i32, y: i32, z: i32) -> usize {
        ((y & 15) * 256 + (z & 15) * 16 + (x & 15)) as usize
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

    pub fn biomes(&self) -> &Paletted<String> {
        &self.biomes
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
    y_pos: Option<i32>,
    /// `WORLD_SURFACE`, wie sie im Chunk steht, siehe [`Chunk::surface`].
    world_surface: Option<Vec<i64>>,
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
            y_pos: raw.y_pos,
            world_surface: raw
                .heightmaps
                .world_surface
                .map(fastnbt::LongArray::into_inner),
        })
    }

    /// Je Spalte, zeilenweise nach z, das y des obersten Blocks, der nicht
    /// Luft ist, oder `None` ohne Block. Aus der Heightmap `WORLD_SURFACE`,
    /// die das Spiel ab dem Status `carvers` speichert; fehlt sie oder passt
    /// sie nicht zum Chunk, aus den Blöcken wie [`Chunk::highest_block`].
    /// Siehe docs/benutzung/map-json.md, „Höhen“.
    pub fn surface(&self) -> [Option<i32>; 256] {
        self.stored_surface().unwrap_or_else(|| {
            std::array::from_fn(|i| {
                let (x, z) = ((i % 16) as i32, (i / 16) as i32);
                self.highest_block(self.x * SECTION + x, self.z * SECTION + z)
                    .map(|(y, _)| y)
            })
        })
    }

    /// `WORLD_SURFACE` wie im Spiel (`Heightmap`, `SimpleBitStorage` in
    /// 26.2): je Long so viele Werte, wie ganz hineinpassen, mit so vielen
    /// Bits, wie die Höhe der Welt plus eins braucht. Der Wert ist
    /// y + 1 − minY, 0 heisst kein Block.
    fn stored_surface(&self) -> Option<[Option<i32>; 256]> {
        let longs = self.world_surface.as_deref()?;
        let min_y = self.y_pos? * SECTION;
        let hoehe = u32::try_from(self.sections.last()?.y as i32 * SECTION + SECTION - min_y)
            .ok()
            .filter(|&h| h > 0)?;
        let bits = u32::BITS - hoehe.leading_zeros();
        let je_long = (u64::BITS / bits) as usize;
        if 256usize.div_ceil(je_long) != longs.len() {
            return None;
        }
        let maske = (1u64 << bits) - 1;
        Some(std::array::from_fn(|i| {
            let wert = (longs[i / je_long] as u64 >> ((i % je_long) as u32 * bits)) & maske;
            (wert > 0).then(|| wert as i32 - 1 + min_y)
        }))
    }

    /// Ob seine Blöcke feststehen: ab dem Status `minecraft:light`, also
    /// bei `light`, `spawn` und `full`. Erst dann haben auch die Nachbarn
    /// alles gesetzt, was von ihnen in ihn hineinreicht. Davor fehlen ihm
    /// Bäume, Seen und Schnee, oder er ist noch ganz Luft. Den Namen liest
    /// das Spiel als Identifier, `full` ist dort `minecraft:full`.
    /// Siehe docs/benutzung/welten.md, „Nicht fertig erzeugte Chunks“.
    pub fn is_generated(&self) -> bool {
        let status = self
            .status
            .strip_prefix("minecraft:")
            .unwrap_or(&self.status);
        matches!(status, "light" | "spawn" | "full")
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn section(&self, section_y: i8) -> Option<&Section> {
        self.section_index(section_y).map(|i| &self.sections[i])
    }

    /// Position der Section in [`Chunk::sections`].
    pub fn section_index(&self, section_y: i8) -> Option<usize> {
        self.sections.binary_search_by_key(&section_y, |s| s.y).ok()
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

    /// Position der Section in [`Chunk::sections`] und Palettenindex eines
    /// Blocks. Damit schlägt ein Renderlauf je Paletteneintrag einmal nach
    /// statt je Block.
    pub fn slot(&self, x: i32, y: i32, z: i32) -> Option<(usize, usize)> {
        if !self.contains_column(x, z) {
            return None;
        }
        let position = self
            .sections
            .binary_search_by_key(&i8::try_from(y >> 4).ok()?, |s| s.y)
            .ok()?;
        Some((position, self.sections[position].slot(x, y, z)))
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
        .map(|entry| BlockState::new(entry.name, entry.properties.into_iter().collect()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    /// Die Heightmap der echten Region aus 26.2 steht in jedem ihrer vier
    /// Chunks und nennt je Spalte den obersten Block, der nicht Luft ist, wie
    /// die Blöcke selbst.
    #[test]
    fn heightmap_wie_die_bloecke() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/world");
        let world = World::open(&dir).unwrap();
        for (cx, cz) in [(577, 416), (578, 416), (577, 417), (578, 417)] {
            let chunk = world.chunk(cx, cz).unwrap().expect("Chunk");
            let gespeichert = chunk.stored_surface().expect("Heightmap");
            for (i, y) in gespeichert.into_iter().enumerate() {
                let (x, z) = (cx * 16 + (i % 16) as i32, cz * 16 + (i / 16) as i32);
                assert_eq!(y, chunk.highest_block(x, z).map(|(y, _)| y), "({x}, {z})");
            }
        }
    }

    /// Steht die Heightmap im Chunk, gilt sie, auch wo die Blöcke etwas
    /// anderes sagen: 9 Bit je Spalte bei 384 Blöcken Höhe, 7 Werte je Long,
    /// y + 1 − minY. Passt ihre Länge nicht, gelten die Blöcke.
    #[test]
    fn heightmap_geht_vor() {
        let luft = || Section {
            y: 19,
            blocks: Paletted::new(vec![BlockState::new("minecraft:air", Vec::new())], None),
            biomes: Paletted::new(Vec::new(), None),
        };
        let mut longs = vec![0i64; 37];
        longs[0] = 20 + 1 + 64;
        longs[1] = 1 << 9;
        let chunk = |longs: Vec<i64>| Chunk {
            x: 0,
            z: 0,
            data_version: 4903,
            status: "minecraft:full".to_string(),
            sections: vec![luft()],
            y_pos: Some(-4),
            world_surface: Some(longs),
        };
        let oben = chunk(longs.clone()).surface();
        assert_eq!(oben[0], Some(20));
        assert_eq!(oben[8], Some(-64), "zweites Long, zweiter Wert");
        assert_eq!(oben.iter().flatten().count(), 2);
        longs.pop();
        assert!(
            chunk(longs).surface().iter().all(Option::is_none),
            "aus den Blöcken"
        );
    }
}
