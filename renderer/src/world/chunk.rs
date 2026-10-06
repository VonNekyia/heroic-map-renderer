use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

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
    #[serde(default, deserialize_with = "eintraege")]
    block_entities: Vec<BlockEntityNbt>,
    /// Was Plugins am Chunk ablegen (PersistentDataContainer). Gelesen wird
    /// nur [`LAUBFARBEN`].
    #[serde(rename = "ChunkBukkitValues")]
    bukkit: Option<fastnbt::Value>,
}

/// Der Schlüssel für eigene Laubfarben in `ChunkBukkitValues`.
/// Siehe docs/benutzung/laubfarben.md.
pub const LAUBFARBEN: &str = "heroicmap:leaf_colors";

/// Bit 24 einer Laubfarbe: Das Spiel nimmt eine hellere Blatttextur.
pub const LAUB_HELL: u32 = 1 << 24;

/// Die Laubfarben eines Chunks nach Fassung 1 des Vertrags: je Stelle im
/// Chunk ihre Farbe, sortiert, bei doppelter Stelle die letzte. `Err` nennt,
/// warum die Bytes dem Vertrag nicht folgen.
/// Siehe docs/benutzung/laubfarben.md, „Format“.
fn laubfarben(bytes: &[u8]) -> Result<Vec<([i32; 3], u32)>, String> {
    let Some((&fassung, mut rest)) = bytes.split_first() else {
        return Err("leer".into());
    };
    if fassung != 1 {
        return Err(format!("Fassung {fassung}"));
    }
    let mut zahl = || -> Result<i32, String> {
        let (kopf, weiter) = rest.split_first_chunk::<4>().ok_or("zu kurz")?;
        rest = weiter;
        Ok(i32::from_be_bytes(*kopf))
    };
    let mut je_stelle = BTreeMap::new();
    for _ in 0..zahl()? {
        let farbe = zahl()? as u32;
        if farbe >> 25 != 0 {
            return Err(format!("Farbe {farbe:#x}"));
        }
        let anzahl = zahl()?;
        if anzahl < 0 {
            return Err(format!("{anzahl} Stellen"));
        }
        for _ in 0..anzahl {
            let stelle = zahl()?;
            if stelle >> 20 != 0 {
                return Err(format!("Stelle {stelle:#x}"));
            }
            let lage = [stelle & 15, (stelle >> 8) - 2048, (stelle >> 4) & 15];
            je_stelle.insert(lage, farbe);
        }
    }
    if !rest.is_empty() {
        return Err(format!("{} Bytes nach dem Ende", rest.len()));
    }
    Ok(je_stelle.into_iter().collect())
}

/// Ein Eintrag in `block_entities`, nur mit den Feldern, aus denen
/// [`Blockdaten`] werden. Jedes darf fehlen oder von anderer Art sein: Das
/// Spiel liest sie mit `getStringOr` und `getIntOr`.
#[derive(Deserialize)]
struct BlockEntityNbt {
    id: Option<fastnbt::Value>,
    x: Option<fastnbt::Value>,
    y: Option<fastnbt::Value>,
    z: Option<fastnbt::Value>,
    patterns: Option<fastnbt::Value>,
    sherds: Option<fastnbt::Value>,
}

/// Liest `block_entities` wie das Spiel (`getList`, `ListTag.compoundStream`
/// in `SerializableChunkData.parse`): Ist es keine Liste, gibt es keine
/// Einträge, und was in der Liste kein Compound ist, fällt weg.
fn eintraege<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<BlockEntityNbt>, D::Error> {
    use serde::de::{Error, IgnoredAny, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    struct Liste;
    impl<'de> Visitor<'de> for Liste {
        type Value = Vec<BlockEntityNbt>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("block_entities")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut liste: A) -> Result<Self::Value, A::Error> {
            let mut eintraege = Vec::new();
            while let Some(Eintrag(eintrag)) = liste.next_element()? {
                eintraege.extend(eintrag);
            }
            Ok(eintraege)
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok(Vec::new())
        }
        fn visit_i64<E: Error>(self, _: i64) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
        fn visit_f64<E: Error>(self, _: f64) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
        fn visit_str<E: Error>(self, _: &str) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }
    }

    /// Ein Element der Liste: ein Compound, sonst nichts.
    struct Eintrag(Option<BlockEntityNbt>);
    impl<'de> Deserialize<'de> for Eintrag {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Eintrag, D::Error> {
            d.deserialize_any(Element)
        }
    }
    struct Element;
    impl<'de> Visitor<'de> for Element {
        type Value = Eintrag;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("ein Blockentity")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Eintrag, A::Error> {
            let map = serde::de::value::MapAccessDeserializer::new(map);
            BlockEntityNbt::deserialize(map).map(|be| Eintrag(Some(be)))
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut liste: A) -> Result<Eintrag, A::Error> {
            while liste.next_element::<IgnoredAny>()?.is_some() {}
            Ok(Eintrag(None))
        }
        fn visit_i64<E: Error>(self, _: i64) -> Result<Eintrag, E> {
            Ok(Eintrag(None))
        }
        fn visit_f64<E: Error>(self, _: f64) -> Result<Eintrag, E> {
            Ok(Eintrag(None))
        }
        fn visit_str<E: Error>(self, _: &str) -> Result<Eintrag, E> {
            Ok(Eintrag(None))
        }
    }

    d.deserialize_any(Liste)
}

/// Eine Zahl wie `CompoundTag.getIntOr` mit 0 als Vorgabe: jede Zahl über
/// `intValue`, Long mit seinen unteren 32 Bit, Float und Double abgerundet
/// (`Mth.floor`), sonst 0.
fn zahl(wert: &Option<fastnbt::Value>) -> i32 {
    use fastnbt::Value;
    match *wert {
        Some(Value::Byte(v)) => v.into(),
        Some(Value::Short(v)) => v.into(),
        Some(Value::Int(v)) => v,
        Some(Value::Long(v)) => v as i32,
        Some(Value::Float(v)) => f64::from(v).floor() as i32,
        Some(Value::Double(v)) => v.floor() as i32,
        _ => 0,
    }
}

/// Was ein Blockentity im Chunk über sein Bild sagt, soweit der Renderer es
/// zeichnet: die Muster eines Banners, die Scherben eines Krugs.
/// Siehe docs/renderer/blockentities.md, „Daten aus dem Chunk“.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Blockdaten {
    /// `patterns`: je Lage das Muster und der Name des Farbstoffs.
    Banner(Vec<(Muster, String)>),
    /// `sherds`: die Items hinten, links, rechts und vorne, höchstens vier;
    /// eine leere Seite als leerer Text.
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
    /// Die Lage im Chunk wie `BlockEntity.getPosFromTag`: `getIntOr` mit 0,
    /// x und z auf den Chunk bezogen. Liegt ein Blockentity ausserhalb,
    /// rückt es so mit seiner Lage im Chunk in diesen.
    fn stelle(&self) -> [i32; 3] {
        [zahl(&self.x) & 15, zahl(&self.y), zahl(&self.z) & 15]
    }

    /// Welches der beiden Blockentities mit Daten es ist, nach seiner
    /// Kennung wie `Identifier.bySeparator`: ohne Namensraum oder mit
    /// leerem gilt `minecraft`. Eine Kennung, die kein Text ist, liest
    /// `getStringOr` als leer; das Spiel überspringt den Eintrag.
    fn art(&self) -> Option<Art> {
        let Some(fastnbt::Value::String(id)) = &self.id else {
            return None;
        };
        let pfad = match id.split_once(':') {
            None => id.as_str(),
            Some(("" | "minecraft", pfad)) => pfad,
            Some(_) => return None,
        };
        match pfad {
            "banner" => Some(Art::Banner),
            "decorated_pot" => Some(Art::Krug),
            _ => None,
        }
    }

    /// Liest die Daten wie `BannerBlockEntity` und `DecoratedPotBlockEntity`
    /// in 26.2: Ein Eintrag, den der Codec ablehnt, fällt heraus, die
    /// übrigen rücken auf (`ListCodec`, `TagValueInput.read`). Die Scherben
    /// eines Krugs aus 26.3 stehen als Objekt, siehe [`seiten_ab_26_3`].
    /// `None` ohne Daten, die das Bild ändern.
    fn daten(self, art: Art) -> Option<Blockdaten> {
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
        let daten = match art {
            Art::Banner => {
                Blockdaten::Banner(liste(self.patterns).into_iter().filter_map(lage).collect())
            }
            Art::Krug => Blockdaten::Krug(match self.sherds {
                Some(fastnbt::Value::Compound(seiten)) => seiten_ab_26_3(seiten),
                sherds => liste(sherds)
                    .into_iter()
                    .filter_map(|item| match item {
                        fastnbt::Value::String(item) => Some(item),
                        _ => None,
                    })
                    .take(4)
                    .collect(),
            }),
        };
        match &daten {
            Blockdaten::Banner(v) if v.is_empty() => None,
            Blockdaten::Krug(v) if v.is_empty() => None,
            _ => Some(daten),
        }
    }
}

/// Die Seiten eines Krugs ab 26.3 (`PotDecorations.CODEC`, nach
/// `PotDecorationsBlockEntityUnflatteningFix`): `back`, `left`, `right` und
/// `front`, je optional ein `ItemStackTemplate`, ein Item-Name oder ein
/// Compound mit `id`. Eine Seite, die fehlt oder sich nicht lesen lässt, ist
/// leer, die übrigen bleiben: `TagValueInput.read` nimmt das Teilergebnis.
/// Ein `count` ausserhalb von 1 bis 99 behält das Item, ebenso als
/// Teilergebnis. Leere Seiten am Ende fallen weg, so gleicht ein Krug dem
/// aus 26.2.
/// Siehe docs/renderer/blockentities.md, „Daten aus dem Chunk“.
fn seiten_ab_26_3(mut seiten: HashMap<String, fastnbt::Value>) -> Vec<String> {
    use fastnbt::Value;
    let mut items: Vec<String> = ["back", "left", "right", "front"]
        .into_iter()
        .map(|seite| match seiten.remove(seite) {
            Some(Value::String(item)) => item,
            Some(Value::Compound(mut vorlage)) => match vorlage.remove("id") {
                Some(Value::String(item)) => item,
                _ => String::new(),
            },
            _ => String::new(),
        })
        .collect();
    while items.last().is_some_and(String::is_empty) {
        items.pop();
    }
    items
}

/// Die beiden Blockentities, deren Daten der Renderer liest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Art {
    Banner,
    Krug,
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

/// Ein Eintrag der Palette. Ab 26.3 heissen die Felder `id` und
/// `properties` (`BlockStateFieldNamesFix`, DataVersion 5006); Chunks, die
/// der Server noch nicht neu gespeichert hat, behalten die alten Namen.
#[derive(Deserialize)]
struct PaletteEntry {
    #[serde(rename = "Name", alias = "id")]
    name: String,
    /// Sortiert, wie `BlockState` sie will.
    #[serde(rename = "Properties", alias = "properties", default)]
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
    /// Je Blockentity mit [`Blockdaten`] seine Lage im Chunk, siehe
    /// [`Chunk::blockentities`].
    blockentities: Vec<([i32; 3], Blockdaten)>,
    /// Eigene Laubfarben je Lage im Chunk, sortiert, siehe
    /// [`Chunk::laubfarben`].
    laubfarben: Vec<([i32; 3], u32)>,
    /// Warum die Laubfarben des Chunks nicht dem Vertrag folgen; dann gilt
    /// keine.
    laubfarben_fehler: Option<String>,
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

        // Nennt `block_entities` eine Stelle mehrmals, gilt wie in einem
        // fertigen Chunk der letzte Eintrag, der zum Block passt:
        // `postLoadChunk` legt jedes Blockentity mit `setBlockEntity` ab, und
        // eines, das nicht zum Block passt, gibt `loadStatic` gar nicht erst
        // zurück. Welche Art zum Block passt, entscheidet später das Bild des
        // Blocks (`blockentity::aendert`); hier gilt der letzte je Art.
        let mut je_stelle = BTreeMap::new();
        for be in raw.block_entities {
            if let Some(art) = be.art() {
                je_stelle.insert((be.stelle(), art), be.daten(art));
            }
        }
        let blockentities = je_stelle
            .into_iter()
            .filter_map(|((stelle, _), daten)| Some((stelle, daten?)))
            .collect();

        // Ein anderer Typ unter dem Schlüssel oder Bytes gegen den Vertrag
        // sind ein Fehler des Schreibers: Der Chunk zeichnet dann ohne.
        let eintrag = match raw.bukkit {
            Some(fastnbt::Value::Compound(mut werte)) => werte.remove(LAUBFARBEN),
            _ => None,
        };
        let (laubfarben, laubfarben_fehler) = match eintrag {
            None => (Vec::new(), None),
            Some(fastnbt::Value::ByteArray(bytes)) => {
                let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
                match laubfarben(&bytes) {
                    Ok(farben) => (farben, None),
                    Err(grund) => (Vec::new(), Some(grund)),
                }
            }
            Some(_) => (Vec::new(), Some("kein Byte-Array".into())),
        };

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
            laubfarben,
            laubfarben_fehler,
        })
    }

    /// Die eigenen Laubfarben mit Weltkoordinate, sortiert nach Lage im
    /// Chunk, wie [`Chunk::blockentities`]. Bit 0 bis 23 sind RGB, Bit 24
    /// ist [`LAUB_HELL`].
    pub fn laubfarben(&self) -> impl Iterator<Item = ([i32; 3], u32)> + '_ {
        let ursprung = self.x.checked_mul(SECTION).zip(self.z.checked_mul(SECTION));
        self.laubfarben
            .iter()
            .filter_map(move |&([x, y, z], farbe)| {
                let (x0, z0) = ursprung?;
                Some(([x0 + x, y, z0 + z], farbe))
            })
    }

    /// Warum die Laubfarben des Chunks nicht galten, `None`, wenn es keine
    /// gab oder sie dem Vertrag folgten.
    pub fn laubfarben_fehler(&self) -> Option<&str> {
        self.laubfarben_fehler.as_deref()
    }

    /// Die Blockentities, deren Daten das Bild ändern, mit Weltkoordinate.
    /// Die bildet erst die Lage des Chunks, die er am Ende hat: Einen Chunk,
    /// der an der falschen Stelle der Regionsdatei steht, legt
    /// [`super::Region::stored_chunk`] an seinen Platz, und seine
    /// Blockentities kommen mit. Eine Lage ausserhalb der Zahlen, die ein
    /// Block haben kann, gibt keine.
    pub fn blockentities(&self) -> impl Iterator<Item = ([i32; 3], &Blockdaten)> {
        let ursprung = self.x.checked_mul(SECTION).zip(self.z.checked_mul(SECTION));
        self.blockentities
            .iter()
            .filter_map(move |&([x, y, z], ref daten)| {
                let (x0, z0) = ursprung?;
                Some(([x0 + x, y, z0 + z], daten))
            })
    }

    /// Je Spalte, zeilenweise nach z, das y des obersten Blocks, der nicht
    /// Luft ist, oder `None` ohne Block. Aus der Heightmap `WORLD_SURFACE`,
    /// die das Spiel ab dem Status `carvers` speichert, ab 26.3 `terrain`;
    /// fehlt sie oder passt sie nicht zum Chunk, aus den Blöcken wie
    /// [`Chunk::highest_block`].
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

    /// Der Fingerabdruck dessen, was der Renderer aus dem Chunk zeichnet,
    /// für Updates: FNV-1a mit 64 Bit über die Blöcke und Biome jeder
    /// Section, als Läufe gleicher Werte der Reihe nach, und über die
    /// [`Blockdaten`]. Wie die Palette geordnet ist und wie ihre Felder
    /// heissen, ändert ihn nicht, ein Chunk aus 26.2 und derselbe aus 26.3
    /// haben denselben. Nur für einen fertig erzeugten Chunk: Einen
    /// unfertigen zeichnet der Renderer nicht, [`Inhalt::von`] fängt ihn ab.
    /// Siehe docs/benutzung/updates.md, „Was als geändert gilt“.
    ///
    /// [`Inhalt::von`]: crate::render::stand::Inhalt::von
    pub fn abdruck(&self) -> Abdruck {
        let mut fnv = Fnv::default();
        let mut oben = None;
        for section in &self.sections {
            fnv.nimm(&section.y.to_le_bytes());
            let bloecke: Vec<u64> = section
                .blocks
                .palette()
                .iter()
                .map(wert_von_block)
                .collect();
            fnv.laeufe(&section.blocks, BLOCKS_PER_SECTION, &bloecke);
            let biome: Vec<u64> = section
                .biomes
                .palette()
                .iter()
                .map(|biom| Fnv::von(&[biom.as_bytes()]))
                .collect();
            fnv.laeufe(&section.biomes, BIOMES_PER_SECTION, &biome);
            if !section.is_empty() {
                let luft: Vec<bool> = section
                    .blocks
                    .palette()
                    .iter()
                    .map(BlockState::is_air)
                    .collect();
                let mut hoechster = None;
                section
                    .blocks
                    .for_each_index(BLOCKS_PER_SECTION, |i, index| {
                        if !luft.get(index).copied().unwrap_or(false) {
                            hoechster = Some(i);
                        }
                    });
                if let Some(i) = hoechster {
                    oben = Some(section.y as i32 * SECTION + (i / 256) as i32);
                }
            }
        }
        for ([x, y, z], daten) in &self.blockentities {
            fnv.nimm(&[*x as u8, *z as u8]);
            fnv.nimm(&y.to_le_bytes());
            match daten {
                Blockdaten::Banner(lagen) => {
                    fnv.nimm(&[1]);
                    for (muster, farbe) in lagen {
                        let (art, name) = match muster {
                            Muster::Id(id) => (1, id),
                            Muster::Asset(asset) => (2, asset),
                        };
                        fnv.nimm(&[art]);
                        fnv.text(name);
                        fnv.text(farbe);
                    }
                }
                Blockdaten::Krug(seiten) => {
                    fnv.nimm(&[2]);
                    for seite in seiten {
                        fnv.text(seite);
                    }
                }
            }
            fnv.nimm(&[0xff]);
        }
        // Nur wenn es welche gibt: Ein Chunk ohne behält seinen Abdruck.
        if !self.laubfarben.is_empty() {
            fnv.nimm(&[3]);
            for ([x, y, z], farbe) in &self.laubfarben {
                fnv.nimm(&[*x as u8, *z as u8]);
                fnv.nimm(&y.to_le_bytes());
                fnv.nimm(&farbe.to_le_bytes());
            }
        }
        Abdruck { hash: fnv.0, oben }
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

/// Was ein Update über einen Chunk weiss, siehe [`Chunk::abdruck`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abdruck {
    /// Der Fingerabdruck seiner Blöcke, Biome und [`Blockdaten`].
    pub hash: u64,
    /// Das y seines höchsten Blocks, der nicht Luft ist; `None` ohne einen.
    pub oben: Option<i32>,
}

/// FNV-1a mit 64 Bit, wie der Fingerabdruck des Looks.
pub(crate) struct Fnv(pub(crate) u64);

impl Default for Fnv {
    fn default() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    pub(crate) fn nimm(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    /// Ein Text mit einem Nullbyte dahinter.
    pub(crate) fn text(&mut self, text: &str) {
        self.nimm(text.as_bytes());
        self.nimm(&[0]);
    }

    /// Der Fingerabdruck dieser Texte allein.
    fn von(texte: &[&[u8]]) -> u64 {
        let mut fnv = Fnv::default();
        for text in texte {
            fnv.nimm(text);
            fnv.nimm(&[0]);
        }
        fnv.0
    }

    /// Die Werte eines Containers als Läufe: je Lauf der Wert seines
    /// Paletteneintrags aus `werte` und seine Länge. So hängt der
    /// Fingerabdruck nur an den Werten der Reihe nach, nicht an der Palette.
    /// Ein Index ausserhalb der Palette zählt als eigener Wert.
    fn laeufe<T>(&mut self, container: &Paletted<T>, eintraege: usize, werte: &[u64]) {
        let wert = |index: usize| werte.get(index).copied().unwrap_or(u64::MAX);
        let mut lauf: Option<(u64, u32)> = None;
        container.for_each_index(eintraege, |_, index| {
            let w = wert(index);
            match &mut lauf {
                Some((bisher, n)) if *bisher == w => *n += 1,
                _ => {
                    if let Some((bisher, n)) = lauf.replace((w, 1)) {
                        self.nimm(&bisher.to_le_bytes());
                        self.nimm(&n.to_le_bytes());
                    }
                }
            }
        });
        if let Some((bisher, n)) = lauf {
            self.nimm(&bisher.to_le_bytes());
            self.nimm(&n.to_le_bytes());
        }
    }
}

/// Der Wert eines Blockstates für [`Fnv::laeufe`]: Name und Eigenschaften,
/// die sortiert sind.
fn wert_von_block(state: &BlockState) -> u64 {
    let mut fnv = Fnv::default();
    fnv.text(state.name());
    for (k, v) in state.props() {
        fnv.text(k);
        fnv.text(v);
    }
    fnv.0
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
            laubfarben: Vec::new(),
            laubfarben_fehler: None,
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
            gelesen(&chunk),
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
                    [37, 70, 45],
                    Blockdaten::Banner(vec![(muster("cross"), "red".to_string())])
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
            ]
        );
    }

    /// Die Scherben eines Krugs aus 26.3 als Objekt, wie
    /// `PotDecorations.CODEC` sie liest: je Seite ein Item-Name oder ein
    /// Compound mit `id`, eine fehlende Seite leer. Was sich nicht lesen
    /// lässt, eine Zahl oder ein Compound ohne `id`, ist leer, die übrigen
    /// bleiben; ein `count` ausserhalb von 1 bis 99 behält das Item. Ohne
    /// eine Seite mit Item trägt der Krug nichts bei.
    #[test]
    fn krug_ab_26_3() {
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
        let krug = |x: i32, seiten| {
            compound(vec![
                ("id", text("minecraft:decorated_pot")),
                ("x", Value::Int(x)),
                ("y", Value::Int(64)),
                ("z", Value::Int(0)),
                ("sherds", seiten),
            ])
        };
        let nbt = nbt_mit(Value::List(vec![
            krug(
                0,
                compound(vec![
                    ("back", text("minecraft:brick")),
                    (
                        "left",
                        compound(vec![
                            ("id", text("angler_pottery_sherd")),
                            ("count", Value::Int(1)),
                        ]),
                    ),
                    ("front", text("minecraft:skull_pottery_sherd")),
                ]),
            ),
            krug(
                1,
                compound(vec![
                    ("right", Value::Int(1)),
                    ("front", compound(vec![("count", Value::Int(1))])),
                ]),
            ),
            krug(
                2,
                compound(vec![
                    ("back", Value::Int(3)),
                    (
                        "left",
                        compound(vec![
                            ("id", text("minecraft:heart_pottery_sherd")),
                            ("count", Value::Int(0)),
                        ]),
                    ),
                ]),
            ),
        ]));
        let chunk = Chunk::decode(&nbt).unwrap();
        let items = |items: &[&str]| items.iter().map(|item| item.to_string()).collect();
        assert_eq!(
            gelesen(&chunk),
            [
                (
                    [0, 64, 0],
                    Blockdaten::Krug(items(&[
                        "minecraft:brick",
                        "angler_pottery_sherd",
                        "",
                        "minecraft:skull_pottery_sherd",
                    ]))
                ),
                (
                    [2, 64, 0],
                    Blockdaten::Krug(items(&["", "minecraft:heart_pottery_sherd"]))
                ),
            ]
        );
    }

    /// Die Palette ab 26.3 mit `id` und `properties`, daneben eine Section
    /// mit den alten Namen, wie in einer Welt, die erst zum Teil neu
    /// gespeichert ist: Beide lesen dieselben Blöcke.
    #[test]
    fn palette_ab_26_3() {
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
        let section = |y: i8, name: &str, properties: &str| {
            let eintrag = compound(vec![
                (name, text("minecraft:oak_log")),
                (properties, compound(vec![("axis", text("x"))])),
            ]);
            compound(vec![
                ("Y", Value::Byte(y)),
                (
                    "block_states",
                    compound(vec![("palette", Value::List(vec![eintrag]))]),
                ),
                (
                    "biomes",
                    compound(vec![(
                        "palette",
                        Value::List(vec![text("minecraft:plains")]),
                    )]),
                ),
            ])
        };
        let nbt = compound(vec![
            ("DataVersion", Value::Int(5023)),
            ("xPos", Value::Int(0)),
            ("zPos", Value::Int(0)),
            ("Status", text("minecraft:full")),
            (
                "sections",
                Value::List(vec![
                    section(0, "id", "properties"),
                    section(1, "Name", "Properties"),
                ]),
            ),
        ]);
        let chunk = Chunk::decode(&fastnbt::to_bytes(&nbt).unwrap()).unwrap();
        let stamm = BlockState::new("minecraft:oak_log", vec![("axis".into(), "x".into())]);
        for y in [0, 16] {
            assert_eq!(chunk.block_at(0, y, 0), Some(&stamm), "y = {y}");
        }
    }

    /// Die Blockentities eines Chunks mit Weltkoordinate, nach Stelle.
    /// Eine Section für [`chunk_aus`]: Palette mit Namen und Eigenschaften,
    /// Indizes oder keine, ein Biom.
    struct Probe<'a> {
        y: i8,
        palette: Vec<(&'a str, Vec<(&'a str, &'a str)>)>,
        indizes: Option<Vec<usize>>,
        biom: &'a str,
    }

    /// Ein Chunk aus diesen Sections, mit den Feldnamen der Palette aus
    /// 26.3 oder aus 26.2, und diesen `block_entities`.
    fn chunk_aus(
        status: &str,
        sections: &[Probe],
        ab_26_3: bool,
        entities: Vec<fastnbt::Value>,
    ) -> Chunk {
        use fastnbt::Value;
        let text = |t: &str| Value::String(t.to_string());
        let compound = |felder: Vec<(&str, Value)>| {
            Value::Compound(
                felder
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
            )
        };
        let (name, eigenschaften) = if ab_26_3 {
            ("id", "properties")
        } else {
            ("Name", "Properties")
        };
        let sections = sections
            .iter()
            .map(|s| {
                let palette = s
                    .palette
                    .iter()
                    .map(|(n, props)| {
                        let mut felder = vec![(name, text(n))];
                        if !props.is_empty() {
                            felder.push((
                                eigenschaften,
                                compound(props.iter().map(|(k, v)| (*k, text(v))).collect()),
                            ));
                        }
                        compound(felder)
                    })
                    .collect();
                let mut blocks = vec![("palette", Value::List(palette))];
                if let Some(indizes) = &s.indizes {
                    let bits = (usize::BITS - (s.palette.len().max(2) - 1).leading_zeros()).max(4);
                    let je_long = 64 / bits as usize;
                    let mut longs = vec![0i64; 4096usize.div_ceil(je_long)];
                    for (i, &v) in indizes.iter().enumerate() {
                        longs[i / je_long] |=
                            ((v as u64) << ((i % je_long) * bits as usize)) as i64;
                    }
                    blocks.push(("data", Value::LongArray(fastnbt::LongArray::new(longs))));
                }
                compound(vec![
                    ("Y", Value::Byte(s.y)),
                    ("block_states", compound(blocks)),
                    (
                        "biomes",
                        compound(vec![("palette", Value::List(vec![text(s.biom)]))]),
                    ),
                ])
            })
            .collect();
        let nbt = fastnbt::to_bytes(&compound(vec![
            ("DataVersion", Value::Int(4903)),
            ("xPos", Value::Int(0)),
            ("zPos", Value::Int(0)),
            ("Status", text(status)),
            ("sections", Value::List(sections)),
            ("block_entities", Value::List(entities)),
        ]))
        .unwrap();
        Chunk::decode(&nbt).unwrap()
    }

    /// Der Fingerabdruck hängt an den Blöcken der Reihe nach, an den Biomen
    /// und an den Blockdaten, nicht an der Ordnung der Palette, an ihren
    /// Feldnamen oder daran, ob eine einheitliche Section Indizes hat.
    /// `oben` ist der höchste Block, der nicht Luft ist. Ein Chunk, der
    /// nicht fertig erzeugt ist, hat immer denselben ohne `oben`.
    #[test]
    fn abdruck_haengt_an_den_bloecken_nicht_an_der_palette() {
        let stein = ("minecraft:stone", vec![]);
        let luft = ("minecraft:air", vec![]);
        let treppe = |seite| {
            (
                "minecraft:oak_stairs",
                vec![("half", "bottom"), ("facing", seite)],
            )
        };
        // Stein bei y = 5, x = 3, z = 2, sonst Luft; darüber eine Treppe.
        let stelle = 5 * 256 + 2 * 16 + 3;
        let indizes = |ein: usize, aus: usize, wo: usize| {
            let mut v = vec![aus; 4096];
            v[wo] = ein;
            v
        };
        let unten = |palette: Vec<_>, ein, aus| Probe {
            y: 0,
            palette,
            indizes: Some(indizes(ein, aus, stelle)),
            biom: "minecraft:plains",
        };
        let oben = |seite, biom| Probe {
            y: 1,
            palette: vec![luft.clone(), treppe(seite)],
            indizes: Some(indizes(1, 0, 3 * 256)),
            biom,
        };
        let chunk = |sections: &[Probe], ab_26_3| {
            chunk_aus("minecraft:full", sections, ab_26_3, Vec::new())
        };
        let a = chunk(
            &[
                unten(vec![luft.clone(), stein.clone()], 1, 0),
                oben("east", "minecraft:plains"),
            ],
            false,
        );
        let b = chunk(
            &[
                unten(vec![stein.clone(), luft.clone()], 0, 1),
                oben("east", "minecraft:plains"),
            ],
            true,
        );
        assert_eq!(
            a.abdruck(),
            b.abdruck(),
            "Palette umgestellt, Feldnamen aus 26.3"
        );
        assert_eq!(a.abdruck().oben, Some(19), "die Treppe bei 16 + 3");

        let anders = [
            chunk(
                &[
                    unten(vec![luft.clone(), stein.clone()], 1, 0),
                    oben("west", "minecraft:plains"),
                ],
                false,
            ),
            chunk(
                &[
                    unten(vec![luft.clone(), stein.clone()], 1, 0),
                    oben("east", "minecraft:desert"),
                ],
                false,
            ),
            chunk(
                &[
                    unten(vec![luft.clone(), stein.clone()], 0, 1),
                    oben("east", "minecraft:plains"),
                ],
                false,
            ),
            chunk(&[oben("east", "minecraft:plains")], false),
            chunk(
                &[
                    Probe {
                        indizes: Some(indizes(1, 0, stelle + 1)),
                        ..unten(vec![luft.clone(), stein.clone()], 1, 0)
                    },
                    oben("east", "minecraft:plains"),
                ],
                false,
            ),
            // Dieselbe Folge der Werte, der letzte Lauf gleich lang: Nur die
            // Längen davor unterscheiden.
            chunk(
                &[
                    Probe {
                        indizes: Some({
                            let mut v = indizes(1, 0, stelle);
                            v[stelle - 1] = 1;
                            v
                        }),
                        ..unten(vec![luft.clone(), stein.clone()], 1, 0)
                    },
                    oben("east", "minecraft:plains"),
                ],
                false,
            ),
        ];
        for (i, c) in anders.iter().enumerate() {
            assert_ne!(c.abdruck().hash, a.abdruck().hash, "Fall {i}");
        }

        let einheitlich = |indizes| Probe {
            y: 0,
            palette: if indizes {
                vec![stein.clone(), luft.clone()]
            } else {
                vec![stein.clone()]
            },
            indizes: indizes.then(|| vec![0; 4096]),
            biom: "minecraft:plains",
        };
        let ohne = chunk(&[einheitlich(false)], false).abdruck();
        assert_eq!(
            ohne,
            chunk(&[einheitlich(true)], false).abdruck(),
            "einheitlich mit Indizes"
        );
        assert_eq!(ohne.oben, Some(15));

        let mit_banner = chunk_aus(
            "minecraft:full",
            &[
                unten(vec![luft.clone(), stein.clone()], 1, 0),
                oben("east", "minecraft:plains"),
            ],
            false,
            vec![banner(
                fastnbt::Value::String("minecraft:banner".to_string()),
                [("x", None), ("y", None), ("z", None)],
            )],
        );
        assert_ne!(
            mit_banner.abdruck().hash,
            a.abdruck().hash,
            "Banner mit Muster"
        );
    }

    /// Bytes nach Fassung 1 des Vertrags: je Gruppe Farbe und Lagen im Chunk.
    fn laub_bytes(gruppen: &[(u32, &[[i32; 3]])]) -> Vec<u8> {
        let mut bytes = vec![1];
        bytes.extend((gruppen.len() as i32).to_be_bytes());
        for (farbe, lagen) in gruppen {
            bytes.extend(farbe.to_be_bytes());
            bytes.extend((lagen.len() as i32).to_be_bytes());
            for [x, y, z] in *lagen {
                bytes.extend((x | z << 4 | (y + 2048) << 8).to_be_bytes());
            }
        }
        bytes
    }

    /// Ein Chunk mit diesem Wert unter `ChunkBukkitValues`.
    fn mit_bukkit(wert: fastnbt::Value) -> Chunk {
        use fastnbt::Value;
        let nbt = fastnbt::to_bytes(&Value::Compound(
            [
                ("DataVersion", Value::Int(4903)),
                ("xPos", Value::Int(2)),
                ("zPos", Value::Int(-1)),
                ("Status", Value::String("minecraft:full".to_string())),
                ("ChunkBukkitValues", wert),
            ]
            .into_iter()
            .map(|(name, wert)| (name.to_string(), wert))
            .collect(),
        ))
        .unwrap();
        Chunk::decode(&nbt).unwrap()
    }

    fn laub(bytes: Vec<u8>) -> Chunk {
        use fastnbt::Value;
        let array = fastnbt::ByteArray::new(bytes.into_iter().map(|b| b as i8).collect());
        mit_bukkit(Value::Compound(
            [(LAUBFARBEN.to_string(), Value::ByteArray(array))].into(),
        ))
    }

    /// Fassung 1: Gruppen aus Farbe und Lagen, Big Endian, Lage
    /// `x | z<<4 | (y+2048)<<8`, auch unter y = 0. Steht eine Lage doppelt,
    /// gilt die letzte. Die Lagen kommen in Weltkoordinaten.
    #[test]
    fn laubfarben_nach_fassung_1() {
        let rot = 0xff_0000;
        let hell = LAUB_HELL | 0x00_ff00;
        let chunk = laub(laub_bytes(&[
            (rot, &[[1, 70, 2], [15, -64, 15]]),
            (hell, &[[1, 70, 2], [0, 2031, 0]]),
        ]));
        assert_eq!(chunk.laubfarben_fehler(), None);
        let farben: Vec<_> = chunk.laubfarben().collect();
        assert_eq!(
            farben,
            vec![
                ([32, 2031, -16], hell),
                ([33, 70, -14], hell),
                ([47, -64, -1], rot)
            ]
        );
    }

    /// Was dem Vertrag nicht folgt, gilt ganz nicht, und der Grund steht
    /// da; der Chunk selbst liest sich weiter. Ohne Schlüssel gibt es weder
    /// Farben noch Grund.
    #[test]
    fn ungueltige_laubfarben_gelten_nicht() {
        use fastnbt::Value;
        let gut = laub_bytes(&[(0x123456, &[[0, 0, 0]])]);
        let mut fassung = gut.clone();
        fassung[0] = 2;
        let mut farbe = gut.clone();
        farbe[5] = 0x02;
        let mut lage = gut.clone();
        let ende = lage.len();
        lage[ende - 3] = 0x10;
        let falle: [(Vec<u8>, &str); 6] = [
            (Vec::new(), "leer"),
            (fassung, "Fassung 2"),
            (gut[..gut.len() - 1].to_vec(), "zu kurz"),
            ([gut.clone(), vec![0]].concat(), "1 Bytes nach dem Ende"),
            (farbe, "Farbe"),
            (lage, "Stelle"),
        ];
        for (bytes, grund) in falle {
            let chunk = laub(bytes);
            assert_eq!(chunk.laubfarben().count(), 0, "{grund}");
            let fehler = chunk.laubfarben_fehler().unwrap_or_default();
            assert!(fehler.starts_with(grund), "{fehler} statt {grund}");
        }
        let int = mit_bukkit(Value::Compound(
            [(LAUBFARBEN.to_string(), Value::Int(7))].into(),
        ));
        assert_eq!(int.laubfarben_fehler(), Some("kein Byte-Array"));
        let fremd = mit_bukkit(Value::Compound(
            [("anderes:plugin".to_string(), Value::Int(7))].into(),
        ));
        assert_eq!(
            (fremd.laubfarben().count(), fremd.laubfarben_fehler()),
            (0, None)
        );
        let kein_compound = mit_bukkit(Value::Int(1));
        assert_eq!(kein_compound.laubfarben_fehler(), None);
    }

    /// Der Abdruck ändert sich mit jeder Farbe und Lage; ohne Schlüssel
    /// bleibt er, wie er ohne die Felder war.
    #[test]
    fn abdruck_haengt_an_den_laubfarben() {
        use fastnbt::Value;
        let leer = mit_bukkit(Value::Compound(Default::default())).abdruck();
        let fremd = mit_bukkit(Value::Compound(
            [("anderes:plugin".to_string(), Value::Int(7))].into(),
        ))
        .abdruck();
        assert_eq!(leer, fremd);
        let rot = laub(laub_bytes(&[(0xff_0000, &[[1, 70, 2]])])).abdruck();
        let gruen = laub(laub_bytes(&[(0x00_ff00, &[[1, 70, 2]])])).abdruck();
        let woanders = laub(laub_bytes(&[(0xff_0000, &[[1, 71, 2]])])).abdruck();
        assert_ne!(leer, rot);
        assert_ne!(rot, gruen);
        assert_ne!(rot, woanders);
    }

    fn gelesen(chunk: &Chunk) -> Vec<([i32; 3], Blockdaten)> {
        chunk
            .blockentities()
            .map(|(stelle, daten)| (stelle, daten.clone()))
            .collect()
    }

    fn nbt_mit(block_entities: fastnbt::Value) -> Vec<u8> {
        use fastnbt::Value;
        fastnbt::to_bytes(&Value::Compound(
            [
                ("DataVersion", Value::Int(4903)),
                ("xPos", Value::Int(0)),
                ("zPos", Value::Int(0)),
                ("Status", Value::String("minecraft:full".to_string())),
                ("block_entities", block_entities),
            ]
            .into_iter()
            .map(|(name, wert)| (name.to_string(), wert))
            .collect(),
        ))
        .unwrap()
    }

    /// Ein Banner mit einer Lage `cross` in Rot, mit Kennung und Lage, wie
    /// sie im Eintrag stehen.
    fn banner(id: fastnbt::Value, lage: [(&str, Option<fastnbt::Value>); 3]) -> fastnbt::Value {
        use fastnbt::Value;
        let text = |text: &str| Value::String(text.to_string());
        let lagen = Value::List(vec![Value::Compound(
            [("pattern", text("cross")), ("color", text("red"))]
                .into_iter()
                .map(|(name, wert)| (name.to_string(), wert))
                .collect(),
        )]);
        let mut felder: std::collections::HashMap<String, Value> =
            [("id".to_string(), id), ("patterns".to_string(), lagen)].into();
        for (name, wert) in lage {
            if let Some(wert) = wert {
                felder.insert(name.to_string(), wert);
            }
        }
        Value::Compound(felder)
    }

    /// Kennung und Lage wie `getStringOr` und `getIntOr` mit 0: ein leerer
    /// Namensraum gilt als `minecraft`, eine fehlende Lage als 0, jede Zahl
    /// über `intValue` (Long mit den unteren 32 Bit, Float und Double
    /// abgerundet), ein Text als 0. Eine Kennung, die kein Text ist, lässt das
    /// Spiel aus.
    #[test]
    fn kennung_und_lage_wie_im_spiel() {
        use fastnbt::Value;
        let text = |text: &str| Value::String(text.to_string());
        let rot = || Blockdaten::Banner(vec![(Muster::Id("cross".to_string()), "red".to_string())]);
        let nbt = nbt_mit(Value::List(vec![
            banner(
                text(":banner"),
                [
                    ("x", Some(Value::Int(1))),
                    ("y", Some(Value::Int(64))),
                    ("z", Some(Value::Int(1))),
                ],
            ),
            banner(
                text("minecraft:banner"),
                [
                    ("x", None),
                    ("y", Some(Value::Int(65))),
                    ("z", Some(Value::Int(2))),
                ],
            ),
            banner(
                text("banner"),
                [
                    ("x", Some(Value::Float(-0.5))),
                    ("y", Some(Value::Long((1 << 32) + 66))),
                    ("z", Some(text("3"))),
                ],
            ),
            banner(
                text("banner"),
                [
                    ("x", Some(Value::Double(6.9))),
                    ("y", Some(Value::Byte(67))),
                    ("z", Some(Value::Short(4))),
                ],
            ),
            banner(
                Value::Int(3),
                [
                    ("x", Some(Value::Int(8))),
                    ("y", Some(Value::Int(64))),
                    ("z", Some(Value::Int(8))),
                ],
            ),
        ]));
        let chunk = Chunk::decode(&nbt).unwrap();
        assert_eq!(
            gelesen(&chunk),
            [
                ([0, 65, 2], rot()),
                ([1, 64, 1], rot()),
                ([6, 67, 4], rot()),
                ([15, 66, 0], rot()),
            ]
        );
    }

    /// `block_entities` wie `ListTag.compoundStream`: Ist es keine Liste,
    /// gibt es keine Einträge, und was in der Liste kein Compound ist,
    /// fällt weg. Der Chunk selbst liest sich in jedem Fall.
    #[test]
    fn keine_liste_wie_im_spiel() {
        use fastnbt::Value;
        for (was, wert) in [
            ("ein Compound", Value::Compound(Default::default())),
            ("eine Zahl", Value::Int(7)),
            ("ein Text", Value::String("banner".to_string())),
            (
                "eine Liste aus Zahlen",
                Value::List(vec![Value::Int(1), Value::Int(2)]),
            ),
            (
                "eine Liste aus Listen",
                Value::List(vec![Value::List(vec![Value::Int(1)])]),
            ),
            (
                "eine Liste aus Texten",
                Value::List(vec![Value::String("x".to_string())]),
            ),
            (
                "eine Liste aus Arrays",
                Value::List(vec![Value::IntArray(fastnbt::IntArray::new(vec![1]))]),
            ),
        ] {
            let chunk = Chunk::decode(&nbt_mit(wert)).unwrap_or_else(|e| panic!("{was}: {e:#}"));
            assert!(gelesen(&chunk).is_empty(), "{was}");
        }
    }

    /// Nennt `block_entities` eine Stelle mehrmals, gilt wie in einem
    /// fertigen Chunk (`postLoadChunk`, `setBlockEntity`) je Art der letzte
    /// Eintrag, auch einer ohne Daten. Eine Truhe an der Stelle eines Banners
    /// verdrängt ihn nicht: Sie passt nicht zum Block, und `loadStatic` gibt
    /// sie gar nicht erst zurück. Banner und Krug an derselben Stelle
    /// bleiben beide; welcher zum Block passt, entscheidet dessen Bild.
    #[test]
    fn letzter_eintrag_je_stelle() {
        use fastnbt::Value;
        let text = |text: &str| Value::String(text.to_string());
        let lage = |[x, y, z]: [i32; 3]| {
            [
                ("x", Some(Value::Int(x))),
                ("y", Some(Value::Int(y))),
                ("z", Some(Value::Int(z))),
            ]
        };
        let mit_farbe = |farbe: &str| {
            let Value::Compound(mut be) = banner(text("minecraft:banner"), lage([1, 64, 1])) else {
                unreachable!()
            };
            be.insert(
                "patterns".to_string(),
                Value::List(vec![Value::Compound(
                    [("pattern", text("cross")), ("color", text(farbe))]
                        .into_iter()
                        .map(|(name, wert)| (name.to_string(), wert))
                        .collect(),
                )]),
            );
            Value::Compound(be)
        };
        let ohne_daten = |stelle| {
            let Value::Compound(mut be) = banner(text("minecraft:banner"), lage(stelle)) else {
                unreachable!()
            };
            be.insert("patterns".to_string(), Value::List(vec![]));
            Value::Compound(be)
        };
        let truhe = Value::Compound(
            [
                ("id".to_string(), text("minecraft:chest")),
                ("x".to_string(), Value::Int(1)),
                ("y".to_string(), Value::Int(64)),
                ("z".to_string(), Value::Int(1)),
            ]
            .into(),
        );
        let krug = Value::Compound(
            [
                ("id".to_string(), text("minecraft:decorated_pot")),
                ("x".to_string(), Value::Int(3)),
                ("y".to_string(), Value::Int(64)),
                ("z".to_string(), Value::Int(3)),
                (
                    "sherds".to_string(),
                    Value::List(vec![text("minecraft:angler_pottery_sherd")]),
                ),
            ]
            .into(),
        );
        let nbt = nbt_mit(Value::List(vec![
            mit_farbe("red"),
            truhe,
            mit_farbe("blue"),
            banner(text("minecraft:banner"), lage([2, 64, 2])),
            ohne_daten([2, 64, 2]),
            krug,
            banner(text("minecraft:banner"), lage([3, 64, 3])),
        ]));
        let chunk = Chunk::decode(&nbt).unwrap();
        let farbe = |farbe: &str| {
            Blockdaten::Banner(vec![(Muster::Id("cross".to_string()), farbe.to_string())])
        };
        assert_eq!(
            gelesen(&chunk),
            [
                ([1, 64, 1], farbe("blue")),
                ([3, 64, 3], farbe("red")),
                (
                    [3, 64, 3],
                    Blockdaten::Krug(vec!["minecraft:angler_pottery_sherd".to_string()])
                ),
            ]
        );
    }
}
