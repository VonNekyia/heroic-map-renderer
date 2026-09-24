//! Färbung aus dem Biom: Gras, Laub und Wasser.
//!
//! Die Texturen dieser Blöcke sind grau. Minecraft multipliziert sie mit
//! einer Farbe, die vom Biom abhängt — aus einer Colormap, indiziert mit
//! Temperatur und Niederschlag, oder direkt aus der Biomdefinition. Welche
//! Blöcke das betrifft und woher ihre Farbe kommt, steht nicht in den
//! Assets, sondern im Code (`BlockColors`). Die Tabelle hier ist der
//! Nachbau davon, beschränkt auf das, was auf einer Karte Fläche macht.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use serde::Deserialize;

use super::{find_file, read_text, split_id};

/// Eine Färbung als RGB-Faktor.
pub type Tint = [u8; 3];

/// Die Farben für die färbbaren Flächen eines Sprites.
///
/// Ein Modell trägt höchstens eine eigene Färbung (`tintindex` ist in
/// Vanilla immer 0); Wasser in einem gefluteten Block kommt als zweite
/// dazu und hat immer die Wasserfarbe des Bioms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Tints {
    pub block: Option<Tint>,
    pub water: Option<Tint>,
}

/// Klima, mit dem ohne Biomdaten gerechnet wird: `plains`.
const DEFAULT_CLIMATE: (f32, f32) = (0.8, 0.4);
/// Farben von `plains`, wenn die Colormaps fehlen.
const DEFAULT_GRASS: Tint = [0x91, 0xBD, 0x59];
const DEFAULT_FOLIAGE: Tint = [0x77, 0xAB, 0x2F];
const DEFAULT_DRY_FOLIAGE: Tint = [0xA3, 0x75, 0x46];
/// `water_color`, das jede Vanilla-Biomdefinition trägt, wenn nichts
/// anderes dasteht.
const DEFAULT_WATER: Tint = [0x3F, 0x76, 0xE4];

/// Feste Farben aus `FoliageColor` und `LilyPadBlock`.
const SPRUCE: Tint = [0x61, 0x99, 0x61];
const BIRCH: Tint = [0x80, 0xA7, 0x55];
const LILY_PAD: Tint = [0x20, 0x80, 0x30];
/// Sumpfgras. Minecraft wählt je nach Rauschen zwischen zwei Grüntönen;
/// das hier ist der häufigere.
// ponytail: fester Ton statt Perlin-Rauschen je Position. Erst nötig, wenn
// jemand die Flecken im Sumpf vermisst.
const SWAMP_GRASS: Tint = [0x6A, 0x70, 0x39];

/// Woher die Farbe eines Blocks kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Grass,
    Foliage,
    DryFoliage,
    Water,
    Fixed(Tint),
}

/// Welche Blöcke gefärbt werden.
///
/// Alles andere mit `tintindex` bleibt ungefärbt: Kirsch- und
/// Blasseichenlaub tragen ihre Farbe in der Textur, Redstone und Ranken
/// färben nach Eigenschaften, und beides macht auf einer Karte keine
/// Fläche.
fn source_of(block: &str) -> Option<Source> {
    Some(match split_id(block).1 {
        "grass_block" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "potted_fern"
        | "bush" | "sugar_cane" => Source::Grass,
        "oak_leaves" | "jungle_leaves" | "acacia_leaves" | "dark_oak_leaves"
        | "mangrove_leaves" | "vine" => Source::Foliage,
        "leaf_litter" => Source::DryFoliage,
        "water_cauldron" => Source::Water,
        "spruce_leaves" => Source::Fixed(SPRUCE),
        "birch_leaves" => Source::Fixed(BIRCH),
        "lily_pad" => Source::Fixed(LILY_PAD),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    None,
    Swamp,
    DarkForest,
}

#[derive(Debug, Clone)]
struct Biome {
    temperature: f32,
    downfall: f32,
    water: Option<Tint>,
    grass: Option<Tint>,
    foliage: Option<Tint>,
    dry_foliage: Option<Tint>,
    modifier: Modifier,
}

/// Colormaps aus den Assets und Biome aus den Daten.
///
/// Beides ist optional: ohne Colormap gelten die Farben von `plains`, ohne
/// Biomdaten bekommt jeder Block die Farbe des Standardklimas.
#[derive(Default)]
pub struct Colors {
    grass: Option<RgbaImage>,
    foliage: Option<RgbaImage>,
    dry_foliage: Option<RgbaImage>,
    biomes: BTreeMap<String, Biome>,
}

impl Colors {
    /// Liest die Colormaps aus den Asset-Wurzeln; fehlende sind kein Fehler.
    pub fn load(roots: &[PathBuf]) -> Colors {
        let map = |name: &str| {
            find_file(
                roots,
                "minecraft",
                "textures",
                &format!("colormap/{name}"),
                "png",
            )
            .and_then(|(_, path)| image::open(path).ok())
            .map(|image| image.into_rgba8())
        };
        Colors {
            grass: map("grass"),
            foliage: map("foliage"),
            dry_foliage: map("dry_foliage"),
            biomes: BTreeMap::new(),
        }
    }

    /// Liest `<dir>/<namespace>/worldgen/biome/**/*.json` — das `data/` aus
    /// dem Client-JAR oder einem Datenpaket. Spätere Aufrufe überschreiben
    /// Biome gleichen Namens, wie gestapelte Datenpakete.
    pub fn load_biomes(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;
        for namespace in std::fs::read_dir(dir)
            .with_context(|| format!("{} lesen", dir.display()))?
            .flatten()
        {
            let root = namespace.path().join("worldgen").join("biome");
            let namespace = namespace.file_name().to_string_lossy().into_owned();

            // Datenpakete legen Biome auch in Unterordner, und der Pfad
            // gehört zur ID: `terralith:cave/underground_jungle` liegt
            // unter `biome/cave/underground_jungle.json`.
            let mut pending = vec![root.clone()];
            while let Some(current) = pending.pop() {
                let Ok(entries) = std::fs::read_dir(&current) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                        continue;
                    }
                    if path.extension().is_none_or(|e| e != "json") {
                        continue;
                    }
                    let text = read_text(&path)?;
                    let json: BiomeJson = serde_json::from_str(&text)
                        .with_context(|| format!("{} ist keine Biomdefinition", path.display()))?;
                    let id = path
                        .strip_prefix(&root)
                        .unwrap_or(&path)
                        .with_extension("")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    self.biomes.insert(format!("{namespace}:{id}"), json.into());
                    count += 1;
                }
            }
        }
        if count == 0 {
            bail!(
                "keine Biome unter {} — erwartet wird <dir>/minecraft/worldgen/biome/*.json",
                dir.display()
            );
        }
        Ok(count)
    }

    /// Wie viele Colormaps gefunden wurden, höchstens drei.
    pub fn maps(&self) -> usize {
        [&self.grass, &self.foliage, &self.dry_foliage]
            .iter()
            .filter(|m| m.is_some())
            .count()
    }

    /// Alle bekannten Biome, sortiert.
    pub fn biomes(&self) -> impl Iterator<Item = &str> {
        self.biomes.keys().map(String::as_str)
    }

    /// Die Farben eines Blocks in einem Biom. `None` als Biom — oder ein
    /// unbekanntes — ergibt das Standardklima.
    pub fn tints(&self, block: &str, biome: Option<&str>) -> Tints {
        let biome = biome.and_then(|name| self.biomes.get(name));
        Tints {
            block: source_of(block).map(|source| self.color(source, biome)),
            water: Some(self.color(Source::Water, biome)),
        }
    }

    fn color(&self, source: Source, biome: Option<&Biome>) -> Tint {
        let (temperature, downfall) =
            biome.map_or(DEFAULT_CLIMATE, |b| (b.temperature, b.downfall));
        let from_map = |map: &Option<RgbaImage>, fallback| {
            map.as_ref()
                .map_or(fallback, |map| lookup(map, temperature, downfall))
        };
        match source {
            Source::Fixed(tint) => tint,
            Source::Water => biome.and_then(|b| b.water).unwrap_or(DEFAULT_WATER),
            Source::Foliage => biome
                .and_then(|b| b.foliage)
                .unwrap_or_else(|| from_map(&self.foliage, DEFAULT_FOLIAGE)),
            Source::DryFoliage => biome
                .and_then(|b| b.dry_foliage)
                .unwrap_or_else(|| from_map(&self.dry_foliage, DEFAULT_DRY_FOLIAGE)),
            Source::Grass => {
                let grass = biome
                    .and_then(|b| b.grass)
                    .unwrap_or_else(|| from_map(&self.grass, DEFAULT_GRASS));
                match biome.map_or(Modifier::None, |b| b.modifier) {
                    Modifier::None => grass,
                    Modifier::Swamp => SWAMP_GRASS,
                    Modifier::DarkForest => dark_forest(grass),
                }
            }
        }
    }
}

/// Pixel der Colormap für ein Klima, wie `GrassColor.get`: Temperatur läuft
/// von rechts nach links, Niederschlag — mit der Temperatur gewichtet — von
/// unten nach oben.
///
/// Geklemmt wird in `float`, gerechnet in `double`, wie in
/// `Biome.getGrassColorFromTexture` und `ColorMapColorUtil.get`. In `f32`
/// landen acht Vanilla-Biome eine Zeile oder Spalte daneben, die Wiese
/// etwa in Zeile 153 statt 152.
fn lookup(map: &RgbaImage, temperature: f32, downfall: f32) -> Tint {
    let temperature = temperature.clamp(0.0, 1.0) as f64;
    let downfall = downfall.clamp(0.0, 1.0) as f64 * temperature;
    let x = ((1.0 - temperature) * 255.0) as u32;
    let y = ((1.0 - downfall) * 255.0) as u32;
    let (w, h) = map.dimensions();
    let p = map.get_pixel(x.min(w - 1), y.min(h - 1)).0;
    [p[0], p[1], p[2]]
}

/// `GrassColorModifier.DARK_FOREST`: `(color & 0xFEFEFE) + 0x28340A >> 1`.
///
/// Je Kanal gerechnet, und das ist dasselbe wie Minecrafts Rechnung im
/// gepackten Wert: die Maske macht jede Kanalsumme gerade, also wandert
/// beim Halbieren kein Bit über eine Kanalgrenze. Die Summe darf über 255
/// liegen — erst nach dem Halbieren passt sie wieder in ein Byte.
fn dark_forest(tint: Tint) -> Tint {
    let add = [0x28u16, 0x34, 0x0A];
    let mut out = [0u8; 3];
    for c in 0..3 {
        out[c] = (((tint[c] & 0xFE) as u16 + add[c]) >> 1) as u8;
    }
    out
}

#[derive(Deserialize)]
struct BiomeJson {
    temperature: f32,
    #[serde(default)]
    downfall: f32,
    #[serde(default)]
    effects: EffectsJson,
}

#[derive(Deserialize, Default)]
struct EffectsJson {
    water_color: Option<ColorJson>,
    grass_color: Option<ColorJson>,
    foliage_color: Option<ColorJson>,
    dry_foliage_color: Option<ColorJson>,
    grass_color_modifier: Option<String>,
}

/// Eine Farbe steht bis 1.21 als Zahl in der Datei, seit 26.x als `#rrggbb`.
#[derive(Deserialize)]
#[serde(untagged)]
enum ColorJson {
    Number(i64),
    Text(String),
}

impl ColorJson {
    fn rgb(&self) -> Option<Tint> {
        let n = match self {
            ColorJson::Number(n) => *n,
            ColorJson::Text(text) => i64::from_str_radix(text.strip_prefix('#')?, 16).ok()?,
        };
        Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
    }
}

impl From<BiomeJson> for Biome {
    fn from(json: BiomeJson) -> Biome {
        let e = json.effects;
        Biome {
            temperature: json.temperature,
            downfall: json.downfall,
            water: e.water_color.as_ref().and_then(ColorJson::rgb),
            grass: e.grass_color.as_ref().and_then(ColorJson::rgb),
            foliage: e.foliage_color.as_ref().and_then(ColorJson::rgb),
            dry_foliage: e.dry_foliage_color.as_ref().and_then(ColorJson::rgb),
            modifier: match e.grass_color_modifier.as_deref() {
                Some("swamp") => Modifier::Swamp,
                Some("dark_forest") => Modifier::DarkForest,
                _ => Modifier::None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farben_als_zahl_und_als_hex() {
        assert_eq!(ColorJson::Number(0x3F76E4).rgb(), Some([0x3F, 0x76, 0xE4]));
        assert_eq!(
            ColorJson::Text("#3f76e4".into()).rgb(),
            Some([0x3F, 0x76, 0xE4])
        );
        assert_eq!(ColorJson::Text("blau".into()).rgb(), None);
    }

    /// Die Formel aus `GrassColor.get`, an den bekannten Ecken der Colormap.
    #[test]
    fn colormap_wird_wie_in_minecraft_indiziert() {
        let map = RgbaImage::from_fn(256, 256, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        // heiss und trocken: rechts unten
        assert_eq!(lookup(&map, 1.0, 0.0), [0, 255, 0]);
        // heiss und nass: rechts oben
        assert_eq!(lookup(&map, 1.0, 1.0), [0, 0, 0]);
        // kalt: links; Niederschlag zählt mit der Temperatur gewichtet
        assert_eq!(lookup(&map, 0.0, 1.0), [255, 255, 0]);
        // plains
        assert_eq!(lookup(&map, 0.8, 0.4), [50, 173, 0]);
        // Wiese und Kirschhain: in f32 wäre es Zeile 153.
        assert_eq!(lookup(&map, 0.5, 0.8), [127, 152, 0]);
        // Taiga: Zeile 203; Steinstrand: Spalte 203.
        assert_eq!(lookup(&map, 0.25, 0.8)[1], 203);
        assert_eq!(lookup(&map, 0.2, 0.3)[0], 203);
    }

    #[test]
    fn dunkelwald_mischt_ins_braune() {
        // (0x90 + 0x28) / 2, (0xBC + 0x34) / 2, (0x58 + 0x0A) / 2
        assert_eq!(dark_forest([0x91, 0xBD, 0x59]), [0x5C, 0x78, 0x31]);
    }

    /// Regression: die Summe wurde vor dem Halbieren auf 255 gekappt. Bei
    /// hellen Grasfarben aus Datenpaketen kam so Grau statt Minecrafts
    /// `(0xFEFEFE + 0x28340A) >> 1`.
    #[test]
    fn dunkelwald_kappt_helle_farben_nicht() {
        assert_eq!(dark_forest([0xFF, 0xFF, 0xFF]), [147, 153, 132]);
    }

    #[test]
    fn ohne_daten_gelten_die_plains_farben() {
        let colors = Colors::default();
        assert_eq!(
            colors.tints("minecraft:grass_block", None),
            Tints {
                block: Some(DEFAULT_GRASS),
                water: Some(DEFAULT_WATER)
            }
        );
        assert_eq!(
            colors
                .tints("minecraft:stone", Some("minecraft:desert"))
                .block,
            None
        );
        assert_eq!(
            colors.tints("minecraft:spruce_leaves", None).block,
            Some(SPRUCE)
        );
        // `BlockColors` färbt den Busch mit Farn und Kurzgras zusammen.
        assert_eq!(
            colors.tints("minecraft:bush", None).block,
            Some(DEFAULT_GRASS)
        );
    }
}
