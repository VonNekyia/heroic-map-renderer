//! Die Farbe des Bioms je Block, gemischt wie im Client.
//! Siehe docs/renderer/biomfarben.md, „Übergänge zwischen Biomen“.

use std::collections::HashMap;

use anyhow::Result;

use crate::assets::colors::{BiomeColors, Colors, Resolver, Tint};
use crate::world::biomzoom::{obfuscate_seed, zoom};

/// Wie weit der Client mischt, wenn niemand es ändert:
/// `Options.biomeBlendRadius`.
pub const BLEND_DEFAULT: u8 = 2;
/// Der grösste Radius, den der Client anbietet.
pub const BLEND_MAX: u8 = 7;

/// Die Farben aller Biome je Nummer, der Radius der Mischung und der Seed,
/// mit dem der Zoom würfelt.
#[derive(Clone)]
pub struct BiomeTable {
    colors: Vec<BiomeColors>,
    index: HashMap<String, u16>,
    /// Das Biom für fehlende Chunks und Sections ohne Biome, und für
    /// Biome ohne Definition: plains, wie im Client.
    plains: u16,
    radius: u8,
    /// Der Seed aus [`obfuscate_seed`]; ohne ihn bleibt es beim Raster von
    /// 4×4×4 Blöcken.
    zoom_seed: Option<i64>,
}

impl BiomeTable {
    /// Die Tabelle zu den Biomen aus `colors`, mit Radius 2 und ohne Seed.
    pub fn new(colors: &Colors) -> BiomeTable {
        let namen: Vec<&str> = colors.biomes().collect();
        let mut farben: Vec<BiomeColors> = namen
            .iter()
            .map(|&n| colors.biome_colors(Some(n)))
            .collect();
        let index: HashMap<String, u16> = namen
            .iter()
            .enumerate()
            .map(|(i, &n)| (n.to_string(), i as u16))
            .collect();
        let plains = match index.get("minecraft:plains") {
            Some(&i) => i,
            None => {
                farben.push(colors.biome_colors(None));
                (farben.len() - 1) as u16
            }
        };
        BiomeTable {
            colors: farben,
            index,
            plains,
            radius: BLEND_DEFAULT,
            zoom_seed: None,
        }
    }

    /// Mit diesem Radius der Mischung, höchstens [`BLEND_MAX`], und dem Seed
    /// der Welt, falls sie einen hat.
    pub fn with(mut self, radius: u8, seed: Option<i64>) -> BiomeTable {
        self.radius = radius.min(BLEND_MAX);
        self.zoom_seed = seed.map(obfuscate_seed);
        self
    }

    /// Die Nummer eines Bioms; ein Biom ohne Definition wird plains.
    pub fn id(&self, name: &str) -> u16 {
        self.index.get(name).copied().unwrap_or(self.plains)
    }

    pub fn plains(&self) -> u16 {
        self.plains
    }

    pub fn radius(&self) -> u8 {
        self.radius
    }

    /// Die Viertelposition, deren Biom der Block trägt: mit Seed wie
    /// `BiomeManager.getBiome`, ohne die Zelle, in der er liegt.
    /// Siehe docs/renderer/biomfarben.md, „Biom je Block“.
    pub fn quart(&self, block: [i32; 3]) -> [i32; 3] {
        match self.zoom_seed {
            Some(seed) => zoom(seed, block),
            None => block.map(|c| c >> 2),
        }
    }

    /// Die Farbe von `resolver` im Biom `biome` an der Spalte `(x, z)`.
    pub fn color(&self, biome: u16, resolver: Resolver, x: i32, z: i32) -> Tint {
        self.colors[biome as usize].get(resolver, x, z)
    }

    /// `ClientLevel.calculateBlockTint`: das Mittel der Farben über die
    /// Blöcke im Quadrat mit dem Radius um den Block, auf seiner Höhe, je
    /// Kanal ganzzahlig geteilt. Mit Radius 0 die Farbe seines eigenen
    /// Bioms. `biome_of` liefert das Biom eines Blocks nach [`quart`].
    /// Siehe docs/renderer/biomfarben.md, „Übergänge zwischen Biomen“.
    ///
    /// [`quart`]: BiomeTable::quart
    pub fn blend(
        &self,
        resolver: Resolver,
        [x, y, z]: [i32; 3],
        mut biome_of: impl FnMut([i32; 3]) -> Result<u16>,
    ) -> Result<Tint> {
        let r = self.radius as i32;
        if r == 0 {
            return Ok(self.color(biome_of([x, y, z])?, resolver, x, z));
        }
        let mut summe = [0u32; 3];
        for cz in z - r..=z + r {
            for cx in x - r..=x + r {
                let farbe = self.color(biome_of([cx, y, cz])?, resolver, cx, cz);
                for (s, c) in summe.iter_mut().zip(farbe) {
                    *s += c as u32;
                }
            }
        }
        let n = ((2 * r + 1) * (2 * r + 1)) as u32;
        Ok(summe.map(|s| (s / n) as u8))
    }
}
