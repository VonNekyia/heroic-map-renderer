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
/// sind. Alles andere (die übrigen Heightmaps, Licht, structures) wird von
/// serde verworfen, von `block_entities` alles ausser [`Blockdaten`].
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
    #[serde(default)]
    block_entities: Vec<BlockEntityNbt>,
}

/// Ein Eintrag in `block_entities`, nur mit den Feldern, aus denen
/// [`Blockdaten`] werden. Die Lage liest das Spiel mit `getIntOr` und 0 als
/// Vorgabe (`BlockEntity.getPosFromTag`).
#[derive(Deserialize)]
struct BlockEntityNbt {
    #[serde(default)]
    id: String,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    z: i32,
    patterns: Option<fastnbt::Value>,
    sherds: Option<fastnbt::Value>,
}

/// Was ein Blockentity im Chunk über sein Bild sagt, soweit der Renderer es
/// zeichnet: die Muster eines Banners, die Scherben eines Krugs.
/// Siehe docs/renderer/blockentities.md, „Daten aus dem Chunk“.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Blockdaten {
    /// `patterns`: je Lage das Muster und der Name des Farbstoffs.
    Banner(Vec<(Muster, String)>),
    /// `sherds`: die Items hinten, links, rechts und vorne, höchstens vier.
    Krug(Vec<String>),
}

/// Das Muster einer Lage, wie `BannerPattern.CODEC` es liest: die ID eines
/// Musters der Registry oder ein Muster mit eigenem `asset_id`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Muster {
    Id(String),
    Asset(String),
}

impl BlockEntityNbt {
    /// Liest die Daten wie `BannerBlockEntity` und `DecoratedPotBlockEntity`
    /// in 26.2: Ein Eintrag, den der Codec ablehnt, fällt heraus, die
    /// übrigen rücken auf (`ListCodec`, `TagValueInput.read`). `None` ohne
    /// Daten, die das Bild ändern.
    fn daten(self) -> Option<Blockdaten> {
        // Eine Liste aus Werten verschiedener Art speichert das Spiel als
        // Liste von Compounds, jeden Wert unter dem leeren Namen, und packt
        // sie beim Lesen wieder aus (`ListTag.addAndUnwrap`).
        let liste = |wert| match wert {
            Some(fastnbt::Value::List(liste)) => liste
                .into_iter()
                .map(|wert| match wert {
                    fastnbt::Value::Compound(mut c) if c.len() == 1 && c.contains_key("") => {
                        c.remove("").expect("eben gefunden")
                    }
                    wert => wert,
                })
                .collect(),
            _ => Vec::new(),
        };
        // `Identifier.tryParse`: ohne Namensraum oder mit leerem `minecraft`.
        let id = match self.id.split_once(':') {
            None => self.id.as_str(),
            Some(("" | "minecraft", pfad)) => pfad,
            Some(_) => return None,
        };
        let daten = match id {
            "banner" => {
                Blockdaten::Banner(liste(self.patterns).into_iter().filter_map(lage).collect())
            }
            "decorated_pot" => Blockdaten::Krug(
                liste(self.sherds)
                    .into_iter()
                    .filter_map(|item| match item {
                        fastnbt::Value::String(item) => Some(item),
                        _ => None,
                    })
                    .take(4)
                    .collect(),
            ),
            _ => return None,
        };
        match &daten {
            Blockdaten::Banner(v) if v.is_empty() => None,
            Blockdaten::Krug(v) if v.is_empty() => None,
            _ => Some(daten),
        }
    }
}

/// Eine Lage aus `patterns`: `pattern` als ID oder als Muster mit
/// `asset_id` und `translation_key`, dazu `color`.
fn lage(wert: fastnbt::Value) -> Option<(Muster, String)> {
    use fastnbt::Value;
    let Value::Compound(mut lage) = wert else {
        return None;
    };
    let muster = match lage.remove("pattern")? {
        Value::String(id) => Muster::Id(id),
        Value::Compound(mut muster) => {
            match (muster.remove("asset_id")?, muster.get("translation_key")?) {
                (Value::String(asset), Value::String(_)) => Muster::Asset(asset),
                _ => return None,
            }
        }
        _ => return None,
    };
    let Value::String(farbe) = lage.remove("color")? else {
        return None;
    };
    Some((muster, farbe))
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
    /// Je Blockentity mit [`Blockdaten`] seine Weltkoordinate.
    blockentities: Vec<([i32; 3], Blockdaten)>,
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

        // Liegt ein Blockentity ausserhalb, rückt das Spiel es mit seiner
        // Lage im Chunk in diesen (`BlockEntity.getPosFromTag`).
        let (x0, z0) = (raw.x_pos * SECTION, raw.z_pos * SECTION);
        let blockentities = raw
            .block_entities
            .into_iter()
            .filter_map(|be| Some(([x0 + (be.x & 15), be.y, z0 + (be.z & 15)], be.daten()?)))
            .collect();

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
            blockentities,
        })
    }

    /// Die Blockentities, deren Daten das Bild ändern, mit Weltkoordinate.
    pub fn blockentities(&self) -> &[([i32; 3], Blockdaten)] {
        &self.blockentities
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
            blockentities: Vec::new(),
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

    /// `block_entities` wie im Spiel gelesen: Muster als ID oder mit
    /// `asset_id`; eine Lage ohne `translation_key`, ohne `color` oder mit
    /// einem Farbstoff, der kein Text ist, fällt heraus, die übrigen rücken
    /// auf. Beim Krug zählen die ersten vier Items, aus einer Liste
    /// verschiedener Werte ausgepackt. Ein Banner ohne Lagen,
    /// eine Truhe und ein Blockentity eines anderen Namensraums tragen
    /// nichts bei, eines ausserhalb des Chunks rückt mit seiner Lage im Chunk
    /// hinein.
    #[test]
    fn blockdaten_wie_im_spiel() {
        use fastnbt::Value;
        let text = |text: &str| Value::String(text.to_string());
        let compound = |felder: Vec<(&str, Value)>| {
            Value::Compound(
                felder
                    .into_iter()
                    .map(|(name, wert)| (name.to_string(), wert))
                    .collect(),
            )
        };
        let lage = |muster, farbe| compound(vec![("pattern", muster), ("color", farbe)]);
        let be = |id: &str, [x, y, z]: [i32; 3], feld: &str, wert| {
            compound(vec![
                ("id", text(id)),
                ("x", Value::Int(x)),
                ("y", Value::Int(y)),
                ("z", Value::Int(z)),
                (feld, wert),
            ])
        };
        let eine = || Value::List(vec![lage(text("cross"), text("red"))]);
        let lagen = Value::List(vec![
            lage(text("minecraft:stripe_top"), text("red")),
            lage(
                compound(vec![
                    ("asset_id", text("beispiel:welle")),
                    ("translation_key", text("beispiel.welle")),
                ]),
                text("blue"),
            ),
            lage(
                compound(vec![("asset_id", text("beispiel:ohne"))]),
                text("blue"),
            ),
            compound(vec![("pattern", text("cross"))]),
            lage(text("cross"), Value::Int(3)),
            lage(text("cross"), text("lime")),
        ]);
        // Werte verschiedener Art in einer Liste, wie das Spiel sie schreibt.
        let gehuellt = |wert| compound(vec![("", wert)]);
        let scherben = Value::List(vec![
            gehuellt(text("minecraft:brick")),
            gehuellt(Value::Int(1)),
            gehuellt(text("angler_pottery_sherd")),
            gehuellt(text("beispiel:scherbe")),
            gehuellt(text("minecraft:heart_pottery_sherd")),
            gehuellt(text("minecraft:skull_pottery_sherd")),
        ]);
        let nbt = compound(vec![
            ("DataVersion", Value::Int(4903)),
            ("xPos", Value::Int(2)),
            ("zPos", Value::Int(2)),
            ("Status", text("minecraft:full")),
            (
                "block_entities",
                Value::List(vec![
                    be("minecraft:banner", [33, 64, 34], "patterns", lagen),
                    be("decorated_pot", [40, 64, 47], "sherds", scherben),
                    be(
                        "minecraft:banner",
                        [34, 64, 34],
                        "patterns",
                        Value::List(vec![]),
                    ),
                    be(
                        "minecraft:chest",
                        [35, 64, 34],
                        "Items",
                        Value::List(vec![]),
                    ),
                    be("beispiel:banner", [36, 64, 34], "patterns", eine()),
                    be("minecraft:banner", [5, 70, -3], "patterns", eine()),
                ]),
            ),
        ]);
        let chunk = Chunk::decode(&fastnbt::to_bytes(&nbt).unwrap()).unwrap();
        let muster = |id: &str| Muster::Id(id.to_string());
        let items = |items: &[&str]| items.iter().map(|item| item.to_string()).collect();
        assert_eq!(
            chunk.blockentities(),
            [
                (
                    [33, 64, 34],
                    Blockdaten::Banner(vec![
                        (muster("minecraft:stripe_top"), "red".to_string()),
                        (
                            Muster::Asset("beispiel:welle".to_string()),
                            "blue".to_string()
                        ),
                        (muster("cross"), "lime".to_string()),
                    ])
                ),
                (
                    [40, 64, 47],
                    Blockdaten::Krug(items(&[
                        "minecraft:brick",
                        "angler_pottery_sherd",
                        "beispiel:scherbe",
                        "minecraft:heart_pottery_sherd",
                    ]))
                ),
                (
                    [37, 70, 45],
                    Blockdaten::Banner(vec![(muster("cross"), "red".to_string())])
                ),
            ]
        );
    }
}
