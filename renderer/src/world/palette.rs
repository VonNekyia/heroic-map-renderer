use std::fmt;

/// Ein Minecraft-Blockstate aus der Chunk-Palette: Name plus Properties.
///
/// Properties werden beim Anlegen nach Schlüssel sortiert, damit `Display`,
/// `Eq` und `Hash` unabhängig von der Reihenfolge in der NBT-Datei sind.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BlockState {
    name: String,
    props: Vec<(String, String)>,
}

impl BlockState {
    pub fn new(name: impl Into<String>, mut props: Vec<(String, String)>) -> Self {
        props.sort();
        Self {
            name: name.into(),
            props,
        }
    }

    /// Liest die Schreibweise, die [`Display`](fmt::Display) erzeugt:
    /// `minecraft:oak_stairs[facing=east,half=bottom]`. Ohne Namensraum gilt
    /// `minecraft`.
    pub fn parse(text: &str) -> Result<BlockState, String> {
        let (name, props) = match text.split_once('[') {
            None => (text.trim(), ""),
            Some((name, rest)) => (
                name.trim(),
                rest.strip_suffix(']')
                    .ok_or_else(|| format!("'{text}': schließende Klammer fehlt"))?,
            ),
        };
        if name.is_empty() {
            return Err(format!("'{text}': kein Blockname"));
        }

        let props = props
            .split(',')
            .filter(|pair| !pair.trim().is_empty())
            .map(|pair| {
                pair.split_once('=')
                    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                    .ok_or_else(|| format!("'{pair}': erwartet name=wert"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let name = if name.contains(':') {
            name.to_string()
        } else {
            format!("minecraft:{name}")
        };
        Ok(BlockState::new(name, props))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn props(&self) -> &[(String, String)] {
        &self.props
    }

    pub fn is_air(&self) -> bool {
        matches!(
            self.name.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if self.props.is_empty() {
            return Ok(());
        }
        f.write_str("[")?;
        for (i, (k, v)) in self.props.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{k}={v}")?;
        }
        f.write_str("]")
    }
}

/// Palettenindizes, gepackt in i64-Werte.
///
/// Seit Minecraft 1.16 überlappt kein Eintrag eine Long-Grenze; die
/// überzähligen High-Bits jedes Longs bleiben ungenutzt.
#[derive(Debug)]
pub struct PackedIndices {
    data: Vec<i64>,
    bits: u32,
    per_long: usize,
}

impl PackedIndices {
    /// `min_bits` ist 4 für Blöcke und 1 für Biome (so schreibt Minecraft es).
    pub fn new(data: Vec<i64>, palette_len: usize, min_bits: u32) -> Self {
        let bits = bits_for(palette_len).max(min_bits);
        Self {
            data,
            bits,
            per_long: 64 / bits as usize,
        }
    }

    /// Anzahl Longs, die Minecraft für `entries` Einträge schreibt.
    pub fn expected_longs(&self, entries: usize) -> usize {
        entries.div_ceil(self.per_long)
    }

    /// Anzahl gespeicherter Longs (nicht Einträge).
    pub fn longs(&self) -> usize {
        self.data.len()
    }

    /// Erster Index, der über die Palette hinauszeigt.
    ///
    /// Der Scan entfällt, wenn die Bitbreite gar keinen zu großen Index
    /// darstellen kann — das ist bei jeder Palette mit Zweierpotenz-Größe der
    /// Fall und damit der häufigste Ausgang.
    pub fn first_index_beyond(&self, entries: usize, palette_len: usize) -> Option<usize> {
        if palette_len >= 1usize << self.bits {
            return None;
        }
        let mut found = None;
        self.for_each(entries, |_, index| {
            if found.is_none() && index >= palette_len {
                found = Some(index);
            }
        });
        found
    }

    /// Index an Position `i`. Liefert 0 statt zu panicken, falls die Datei
    /// zu wenige Longs enthält — ein kaputter Chunk soll keinen Renderlauf
    /// über hunderte Regionen abbrechen.
    pub fn get(&self, i: usize) -> usize {
        let Some(&long) = self.data.get(i / self.per_long) else {
            return 0;
        };
        let shift = (i % self.per_long) * self.bits as usize;
        ((long as u64 >> shift) & ((1u64 << self.bits) - 1)) as usize
    }

    /// Die ersten `entries` Indizes der Reihe nach, wie `get` sie liefern
    /// würde — aber ohne Division je Eintrag. Für alles, was eine ganze
    /// Section auf einmal durchgeht.
    pub fn for_each(&self, entries: usize, mut f: impl FnMut(usize, usize)) {
        let mask = (1u64 << self.bits) - 1;
        let mut i = 0;
        for &long in &self.data {
            let mut word = long as u64;
            for _ in 0..self.per_long {
                if i == entries {
                    return;
                }
                f(i, (word & mask) as usize);
                word >>= self.bits;
                i += 1;
            }
        }
        // Fehlende Longs zählen als Index 0, wie bei `get`.
        while i < entries {
            f(i, 0);
            i += 1;
        }
    }
}

/// Bits pro Eintrag für eine Palette dieser Größe: `ceil(log2(len))`, min. 1.
fn bits_for(palette_len: usize) -> u32 {
    64 - (palette_len.max(2) as u64 - 1).leading_zeros()
}

/// Palette plus optionale Indizes. Fehlen die Indizes, besteht der ganze
/// Container aus dem ersten Paletteneintrag.
#[derive(Debug)]
pub struct Paletted<T> {
    palette: Vec<T>,
    indices: Option<PackedIndices>,
}

impl<T> Paletted<T> {
    pub fn new(palette: Vec<T>, indices: Option<PackedIndices>) -> Self {
        Self { palette, indices }
    }

    pub fn palette(&self) -> &[T] {
        &self.palette
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        match &self.indices {
            None => self.palette.first(),
            Some(idx) => self.palette.get(idx.get(i)),
        }
    }

    /// Palettenindex an Position `i` — für Caches, die je Paletteneintrag
    /// statt je Block nachschlagen. Kann bei kaputten Daten über die
    /// Palette hinausgehen, wie bei `get`.
    pub fn index(&self, i: usize) -> usize {
        match &self.indices {
            None => 0,
            Some(idx) => idx.get(i),
        }
    }

    /// True, wenn der Container nur aus einem einzigen Wert besteht.
    pub fn is_uniform(&self) -> bool {
        self.indices.is_none() || self.palette.len() <= 1
    }

    /// Palettenindex je Position, der Reihe nach — siehe
    /// [`PackedIndices::for_each`].
    pub fn for_each_index(&self, entries: usize, mut f: impl FnMut(usize, usize)) {
        match &self.indices {
            None => (0..entries).for_each(|i| f(i, 0)),
            Some(idx) => idx.for_each(entries, f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `for_each` muss Eintrag für Eintrag dasselbe liefern wie `get` —
    /// auch bei einer Bitbreite, die 64 nicht teilt, und bei fehlenden
    /// Longs am Ende.
    #[test]
    fn for_each_liefert_dasselbe_wie_get() {
        let data: Vec<i64> = (1..=7u64)
            .map(|k| 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(k) as i64)
            .collect();
        // 20 Einträge: 5 Bits, 12 je Long, 84 Einträge in 7 Longs.
        let packed = PackedIndices::new(data, 20, 4);
        let mut seen = Vec::new();
        packed.for_each(100, |i, index| seen.push((i, index)));
        assert_eq!(seen.len(), 100);
        for (i, index) in seen {
            assert_eq!(index, packed.get(i), "Index {i}");
        }
    }

    #[test]
    fn bits_pro_eintrag() {
        assert_eq!(bits_for(1), 1);
        assert_eq!(bits_for(2), 1);
        assert_eq!(bits_for(3), 2);
        assert_eq!(bits_for(4), 2);
        assert_eq!(bits_for(5), 3);
        assert_eq!(bits_for(16), 4);
        assert_eq!(bits_for(17), 5);
        assert_eq!(bits_for(256), 8);
        assert_eq!(bits_for(257), 9);
    }

    /// Packt Werte so, wie Minecraft es tut, und liest sie zurück.
    fn pack(values: &[usize], bits: u32) -> Vec<i64> {
        let per_long = 64 / bits as usize;
        let mut out = vec![0i64; values.len().div_ceil(per_long)];
        for (i, &v) in values.iter().enumerate() {
            let shift = (i % per_long) * bits as usize;
            out[i / per_long] |= ((v as u64) << shift) as i64;
        }
        out
    }

    #[test]
    fn entpackt_ohne_long_ueberlappung() {
        // 5 Bit => 12 Einträge pro Long, 4 Bit High-Bits bleiben ungenutzt
        let values: Vec<usize> = (0..30).map(|i| i % 20).collect();
        let idx = PackedIndices::new(pack(&values, 5), 20, 4);
        assert_eq!(idx.bits, 5);
        assert_eq!(idx.per_long, 12);
        for (i, &want) in values.iter().enumerate() {
            assert_eq!(idx.get(i), want, "Eintrag {i}");
        }
    }

    #[test]
    fn mindestens_vier_bit_fuer_bloecke() {
        // Palette mit 3 Einträgen bräuchte 2 Bit, Minecraft schreibt trotzdem 4
        let values = vec![0usize, 1, 2, 1, 0, 2];
        let idx = PackedIndices::new(pack(&values, 4), 3, 4);
        assert_eq!(idx.bits, 4);
        for (i, &want) in values.iter().enumerate() {
            assert_eq!(idx.get(i), want);
        }
    }

    #[test]
    fn zu_kurze_daten_panicken_nicht() {
        let idx = PackedIndices::new(vec![0i64], 16, 4);
        assert_eq!(idx.get(0), 0);
        assert_eq!(idx.get(4095), 0);
    }

    #[test]
    fn blockstate_aus_text() {
        let p = |s| BlockState::parse(s).unwrap().to_string();
        assert_eq!(p("minecraft:stone"), "minecraft:stone");
        assert_eq!(p("stone"), "minecraft:stone");
        assert_eq!(
            p("minecraft:oak_stairs[half=bottom,facing=east]"),
            "minecraft:oak_stairs[facing=east,half=bottom]"
        );
        assert_eq!(p("terranova:x[a=1]"), "terranova:x[a=1]");
        // Leerzeichen und ein überzähliges Komma stören nicht
        assert_eq!(p("stone[ a = 1 ,]"), "minecraft:stone[a=1]");

        assert!(BlockState::parse("stone[a=1").is_err());
        assert!(BlockState::parse("stone[a]").is_err());
        assert!(BlockState::parse("[a=1]").is_err());
    }

    #[test]
    fn blockstate_ist_reihenfolgeunabhaengig() {
        let a = BlockState::new(
            "minecraft:oak_stairs",
            vec![
                ("half".into(), "bottom".into()),
                ("facing".into(), "east".into()),
            ],
        );
        let b = BlockState::new(
            "minecraft:oak_stairs",
            vec![
                ("facing".into(), "east".into()),
                ("half".into(), "bottom".into()),
            ],
        );
        assert_eq!(a, b);
        assert_eq!(
            a.to_string(),
            "minecraft:oak_stairs[facing=east,half=bottom]"
        );
        assert_eq!(a.prop("facing"), Some("east"));
        assert_eq!(a.prop("shape"), None);
    }
}
