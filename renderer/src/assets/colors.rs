//! Färbung aus dem Biom: Gras, Laub und Wasser.
//!
//! Die Texturen dieser Blöcke sind grau. Minecraft multipliziert sie mit
//! einer Farbe, die vom Biom abhängt — aus einer Colormap, indiziert mit
//! Temperatur und Niederschlag, oder direkt aus der Biomdefinition. Welche
//! Blöcke das betrifft und woher ihre Farbe kommt, steht nicht in den
//! Assets, sondern im Code (`BlockColors`). Die Tabelle hier ist der
//! Nachbau davon, beschränkt auf das, was auf einer Karte Fläche macht.
//! Siehe docs/renderer/biomfarben.md, „Welche Blöcke“.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};
use image::RgbaImage;
use serde_json::Value;

use super::blockstate::{boolean, field, float, int_value};
use super::noise;
use super::pack::{self, Pack};
use super::{parse_json, read_text, split_id};

/// Eine Färbung als RGB-Faktor.
pub type Tint = [u8; 3];

/// Die Farben für die färbbaren Flächen eines Sprites.
///
/// Jede Fläche mit `tintindex` trägt die Farbe des Blocks, gleich mit
/// welchem Index. In Vanilla tragen nur Blütenteppich und Wildblumen einen
/// anderen als 0: ihre Stiele Lage 1, die das Spiel mit Gras färbt; Lage 0
/// liesse es ungefärbt, ihre Modelle haben aber keine. Wasser in einem
/// gefluteten Block kommt als zweite Farbe dazu und hat immer die
/// Wasserfarbe des Bioms.
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
/// Die beiden Grün des Sumpfs aus `GrassColorModifier.SWAMP`: das dunkle,
/// wo das Rauschen unter -0,1 liegt, sonst das helle.
/// Siehe docs/renderer/biomfarben.md, „Sumpfgras“.
const SWAMP_DARK: Tint = [0x4C, 0x76, 0x3C];
const SWAMP_LIGHT: Tint = [0x6A, 0x70, 0x39];

/// Die vier `ColorResolver` aus `BiomeColors`: welche Farbe des Bioms eine
/// gefärbte Fläche trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolver {
    Grass,
    Foliage,
    DryFoliage,
    Water,
}

/// Woher die Farbe eines Blocks kommt: aus dem Biom, gemischt beim
/// Zeichnen, oder fest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Biome(Resolver),
    Fixed(Tint),
}

/// Welche Blöcke gefärbt werden. Alles andere mit `tintindex` bleibt
/// ungefärbt.
/// Siehe docs/renderer/biomfarben.md, „Welche Blöcke“.
pub fn source_of(block: &str) -> Option<Source> {
    use Resolver::*;
    Some(match split_id(block).1 {
        "grass_block" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "potted_fern"
        | "bush" | "sugar_cane" | "pink_petals" | "wildflowers" => Source::Biome(Grass),
        "oak_leaves" | "jungle_leaves" | "acacia_leaves" | "dark_oak_leaves"
        | "mangrove_leaves" | "vine" => Source::Biome(Foliage),
        "leaf_litter" => Source::Biome(DryFoliage),
        "water_cauldron" => Source::Biome(Water),
        "spruce_leaves" => Source::Fixed(SPRUCE),
        "birch_leaves" => Source::Fixed(BIRCH),
        "lily_pad" => Source::Fixed(LILY_PAD),
        _ => return None,
    })
}

/// Ob ein Block seine Farbe am Block darunter nimmt: die obere Hälfte von
/// hohem Gras und grossem Farn, wie `BlockTintSources.doubleTallGrass`.
/// Siehe docs/renderer/biomfarben.md, „Welche Blöcke“.
pub fn tinted_below(block: &str, half: Option<&str>) -> bool {
    half == Some("upper") && matches!(split_id(block).1, "tall_grass" | "large_fern")
}

/// Die Farben eines Bioms, wie die vier [`Resolver`] sie liefern, ohne
/// Mischung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiomeColors {
    grass: Tint,
    foliage: Tint,
    dry_foliage: Tint,
    water: Tint,
    /// `grass_color_modifier: swamp`: das Gras nach dem Rauschen an der
    /// Stelle.
    swamp: bool,
}

impl BiomeColors {
    /// Die Farbe an der Blockspalte `(x, z)`; nur Sumpfgras hängt an ihr.
    pub fn get(&self, resolver: Resolver, x: i32, z: i32) -> Tint {
        match resolver {
            Resolver::Grass if self.swamp => {
                if noise::biome_info(x, z) < -0.1 {
                    SWAMP_DARK
                } else {
                    SWAMP_LIGHT
                }
            }
            Resolver::Grass => self.grass,
            Resolver::Foliage => self.foliage,
            Resolver::DryFoliage => self.dry_foliage,
            Resolver::Water => self.water,
        }
    }
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
    broken_biomes: BTreeMap<String, String>,
    unreadable: BTreeMap<String, String>,
}

impl Colors {
    /// Liest die Colormaps aus den Asset-Wurzeln; fehlende sind kein Fehler.
    /// Der Client öffnet sie direkt, statt sie aufzulisten
    /// (`LegacyStuffWrapper.getPixels`), aus dem obersten Pack, das sie hat.
    pub fn load(packs: &[Pack]) -> Colors {
        let map = |name: &str| {
            packs
                .iter()
                .rev()
                .find_map(|pack| {
                    pack.resource("minecraft", &format!("textures/colormap/{name}.png"))
                })
                .and_then(|path| image::open(path).ok())
                .map(|image| image.into_rgba8())
        };
        Colors {
            grass: map("grass"),
            foliage: map("foliage"),
            dry_foliage: map("dry_foliage"),
            ..Colors::default()
        }
    }

    /// Liest `<dir>/<namespace>/worldgen/biome/**/*.json` — das `data/` aus
    /// dem Client-JAR oder einem Datenpaket —, aufgelistet wie im Client
    /// ([`Pack`]). Der Pfad gehört zur ID: `beispiel:hoehle/tropfstein` liegt
    /// unter `biome/hoehle/tropfstein.json`. Spätere Aufrufe überschreiben
    /// Biome gleichen Namens, wie gestapelte Datenpakete. Ein Biom, das der
    /// Codec ablehnt ([`biome`]), übergeht der Renderer und nennt es in
    /// [`Colors::broken_biomes`]. Liefert, wie viele Biome es gelesen hat.
    /// Siehe docs/renderer/biomfarben.md, „Biome lesen“.
    pub fn load_biomes(&mut self, dir: &Path) -> Result<usize> {
        let pack = Pack::open(dir, &pack::BIOME)?;
        let mut dateien = 0;
        let mut count = 0;
        for (name, pfad) in pack.files() {
            let Some((namespace, rest)) = name.split_once('/') else {
                continue;
            };
            let Some(id) = rest
                .strip_prefix("worldgen/biome/")
                .and_then(|id| id.strip_suffix(".json"))
            else {
                continue;
            };
            dateien += 1;
            let biom = read_text(pfad).and_then(|text| biome(&parse_json(&text, true)?));
            match biom {
                Ok(biom) => {
                    self.biomes.insert(format!("{namespace}:{id}"), biom);
                    count += 1;
                }
                Err(grund) => {
                    self.broken_biomes
                        .insert(pfad.display().to_string(), format!("{grund:#}"));
                }
            }
        }
        if dateien == 0 {
            let unlesbar: String = pack
                .unreadable()
                .iter()
                .map(|(pfad, grund)| format!("; {pfad} nicht lesbar: {grund}"))
                .collect();
            bail!(
                "keine Biome unter {} — erwartet wird <dir>/minecraft/worldgen/biome/*.json{unlesbar}",
                dir.display()
            );
        }
        self.unreadable.extend(pack.unreadable().clone());
        Ok(count)
    }

    /// Biomdateien, die der Codec ablehnt, je Pfad mit dem Grund.
    pub fn broken_biomes(&self) -> &BTreeMap<String, String> {
        &self.broken_biomes
    }

    /// Anfänge von Listen in den Datenwurzeln, die sich nicht lesen
    /// liessen ([`Pack::unreadable`]).
    pub fn unreadable(&self) -> &BTreeMap<String, String> {
        &self.unreadable
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

    /// Die Farben eines Blocks in einem Biom, ungemischt, Sumpfgras an der
    /// Spalte x = z = 0. `None` als Biom — oder ein unbekanntes — ergibt
    /// das Standardklima.
    pub fn tints(&self, block: &str, biome: Option<&str>) -> Tints {
        let colors = self.biome_colors(biome);
        Tints {
            block: source_of(block).map(|source| match source {
                Source::Fixed(tint) => tint,
                Source::Biome(resolver) => colors.get(resolver, 0, 0),
            }),
            water: Some(colors.water),
        }
    }

    /// Die Farben eines Bioms; `None` — oder ein unbekanntes — ergibt das
    /// Standardklima.
    pub fn biome_colors(&self, biome: Option<&str>) -> BiomeColors {
        let biome = biome.and_then(|name| self.biomes.get(name));
        let (temperature, downfall) =
            biome.map_or(DEFAULT_CLIMATE, |b| (b.temperature, b.downfall));
        let from_map = |map: &Option<RgbaImage>, fallback| {
            map.as_ref()
                .map_or(fallback, |map| lookup(map, temperature, downfall))
        };
        let grass = biome
            .and_then(|b| b.grass)
            .unwrap_or_else(|| from_map(&self.grass, DEFAULT_GRASS));
        let modifier = biome.map_or(Modifier::None, |b| b.modifier);
        BiomeColors {
            grass: match modifier {
                Modifier::DarkForest => dark_forest(grass),
                _ => grass,
            },
            foliage: biome
                .and_then(|b| b.foliage)
                .unwrap_or_else(|| from_map(&self.foliage, DEFAULT_FOLIAGE)),
            dry_foliage: biome
                .and_then(|b| b.dry_foliage)
                .unwrap_or_else(|| from_map(&self.dry_foliage, DEFAULT_DRY_FOLIAGE)),
            water: biome.and_then(|b| b.water).unwrap_or(DEFAULT_WATER),
            swamp: modifier == Modifier::Swamp,
        }
    }
}

/// Pixel der Colormap für ein Klima, wie `GrassColor.get`: Temperatur läuft
/// von rechts nach links, Niederschlag — mit der Temperatur gewichtet — von
/// unten nach oben. Geklemmt wird in `float`, gerechnet in `double`, wie in
/// `Biome.getGrassColorFromTexture` und `ColorMapColorUtil.get`; in `f32`
/// landen manche Biome eine Zeile daneben.
/// Siehe docs/renderer/biomfarben.md, „Gras, Laub und Wasser“.
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

/// Ein Biom, soweit der Renderer es braucht, gelesen wie
/// `Biome.DIRECT_CODEC`: `ClimateSettings` ganz, aus `effects` die Farben
/// und `grass_color_modifier`. Pflicht sind `has_precipitation`,
/// `temperature`, `downfall`, `effects` und darin `water_color`; `null`
/// zählt wie im Codec als fehlend.
fn biome(json: &Value) -> Result<Biome> {
    ensure!(json.is_object(), "kein Objekt");
    let pflicht = |json: &Value, name: &str| {
        field(json, name)
            .cloned()
            .ok_or_else(|| anyhow!("{name} fehlt"))
    };
    boolean(&pflicht(json, "has_precipitation")?).context("has_precipitation")?;
    let temperature = float(&pflicht(json, "temperature")?).context("temperature")?;
    if let Some(wert) = field(json, "temperature_modifier") {
        name_aus(wert, &["none", "frozen"]).context("temperature_modifier")?;
    }
    let downfall = float(&pflicht(json, "downfall")?).context("downfall")?;
    let effects = pflicht(json, "effects")?;
    ensure!(effects.is_object(), "effects ist kein Objekt");
    let farbe = |name: &str| {
        field(&effects, name)
            .map(|wert| color(wert).with_context(|| name.to_string()))
            .transpose()
    };
    let water = color(&pflicht(&effects, "water_color")?).context("water_color")?;
    let modifier = match field(&effects, "grass_color_modifier") {
        None => Modifier::None,
        Some(wert) => match name_aus(wert, &["none", "dark_forest", "swamp"])
            .context("grass_color_modifier")?
        {
            "dark_forest" => Modifier::DarkForest,
            "swamp" => Modifier::Swamp,
            _ => Modifier::None,
        },
    };
    Ok(Biome {
        temperature,
        downfall,
        water: Some(water),
        grass: farbe("grass_color")?,
        foliage: farbe("foliage_color")?,
        dry_foliage: farbe("dry_foliage_color")?,
        modifier,
    })
}

/// Ein Name aus `StringRepresentable`: nur Text, und nur einer der Werte.
fn name_aus<'a>(json: &'a Value, namen: &[&str]) -> Result<&'a str> {
    json.as_str()
        .filter(|name| namen.contains(name))
        .ok_or_else(|| anyhow!("{json} ist keiner von {}", namen.join(", ")))
}

/// `ExtraCodecs.STRING_RGB_COLOR`: `#rrggbb` mit genau sechs Hexziffern,
/// sonst eine ganze Zahl (`Codec.INT`, abgeschnitten wie `intValue`), sonst
/// drei Kommazahlen (`VECTOR3F`), je Kanal `Mth.floor(x * 255)`, in `float`
/// gerechnet und auf acht Bit gekappt wie `ARGB.color`. Es zählen die
/// unteren 24 Bit.
pub(super) fn color(json: &Value) -> Result<Tint> {
    let rgb = match json {
        Value::String(text) => {
            let hex = text
                .strip_prefix('#')
                .filter(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| anyhow!("{text} ist kein #rrggbb"))?;
            u32::from_str_radix(hex, 16)?
        }
        Value::Number(number) => int_value(number)? as u32,
        Value::Array(liste) => {
            ensure!(liste.len() == 3, "{} statt 3 Werte", liste.len());
            let kanal = |wert: &Value| -> Result<u32> {
                Ok((f64::from(float(wert)? * 255.0).floor() as i32 & 0xFF) as u32)
            };
            kanal(&liste[0])? << 16 | kanal(&liste[1])? << 8 | kanal(&liste[2])?
        }
        _ => bail!("{json} ist keine Farbe"),
    };
    Ok([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wie `STRING_RGB_COLOR` in 26.2: Text, ganze Zahl oder drei
    /// Kommazahlen, belegt per javap samt DFU 10.0.21.
    #[test]
    fn farben_wie_der_codec() {
        let farbe = |json: &str| color(&serde_json::from_str(json).unwrap());
        for (json, soll) in [
            ("4159204", [0x3F, 0x76, 0xE4]),
            ("4159204.9", [0x3F, 0x76, 0xE4]),
            ("-12618012", [0x3F, 0x76, 0xE4]),
            (r##""#3f76e4""##, [0x3F, 0x76, 0xE4]),
            (r##""#3F76E4""##, [0x3F, 0x76, 0xE4]),
            ("[0.2, 0.4, 0.8]", [51, 102, 204]),
            ("[1, 2.0, -0.5]", [255, 254, 128]),
        ] {
            assert_eq!(farbe(json).unwrap(), soll, "{json}");
        }
        for json in [
            r##""#3f76e""##,
            r##""#+3f76e""##,
            r##""3f76e4""##,
            r#""blau""#,
            "[1, 2]",
            r#"[1, 2, "3"]"#,
            "true",
            "{}",
        ] {
            assert!(farbe(json).is_err(), "{json}");
        }
    }

    /// Ein Biom liest der Renderer wie der Codec: was fehlt oder nicht
    /// passt, macht es kaputt. Ein doppelter Schlüssel nimmt wie in Gson
    /// den letzten Wert, `null` zählt als fehlend.
    #[test]
    fn biom_wie_der_codec() {
        let lies = |json: &str| biome(&parse_json(json, true).unwrap());
        let gut = r##"{"has_precipitation": true, "temperature": 0.5, "temperature": 0.9, "downfall": 0.4, "effects": {"water_color": [0.2, 0.4, 0.8], "grass_color": "#91bd59", "foliage_color": null, "grass_color_modifier": "swamp"}}"##;
        let biom = lies(gut).unwrap();
        assert_eq!(biom.temperature, 0.9);
        assert_eq!(biom.water, Some([51, 102, 204]));
        assert_eq!(biom.grass, Some([0x91, 0xBD, 0x59]));
        assert_eq!(biom.foliage, None);
        assert_eq!(biom.modifier, Modifier::Swamp);
        for json in [
            r#"{"temperature": 0.5, "downfall": 0.4, "effects": {"water_color": 1}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "effects": {"water_color": 1}}"#,
            r#"{"has_precipitation": true, "temperature": "0.5", "downfall": 0.4, "effects": {"water_color": 1}}"#,
            r#"{"has_precipitation": 1, "temperature": 0.5, "downfall": 0.4, "effects": {"water_color": 1}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "downfall": 0.4}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "downfall": 0.4, "effects": {}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "downfall": 0.4, "effects": {"water_color": null}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "downfall": 0.4, "effects": {"water_color": 1, "grass_color": "gruen"}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "downfall": 0.4, "effects": {"water_color": 1, "grass_color_modifier": "wald"}}"#,
            r#"{"has_precipitation": true, "temperature": 0.5, "temperature_modifier": "warm", "downfall": 0.4, "effects": {"water_color": 1}}"#,
        ] {
            assert!(lies(json).is_err(), "{json}");
        }
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

    /// Im Sumpf wählt das Rauschen je Spalte eines der zwei Grün, wie
    /// `GrassColorModifier.SWAMP`: an den beiden Stellen, die im Spiel der
    /// Grenze -0,1 am nächsten liegen, einmal knapp darunter, einmal knapp
    /// darüber, siehe `noise::tests`. Laub und Wasser hängen nicht an der
    /// Stelle, und ohne Modifikator auch das Gras nicht.
    #[test]
    fn sumpfgras_nach_dem_rauschen() {
        let sumpf = BiomeColors {
            grass: DEFAULT_GRASS,
            foliage: DEFAULT_FOLIAGE,
            dry_foliage: DEFAULT_DRY_FOLIAGE,
            water: DEFAULT_WATER,
            swamp: true,
        };
        assert_eq!(sumpf.get(Resolver::Grass, 416, -988), SWAMP_DARK);
        assert_eq!(sumpf.get(Resolver::Grass, 404, 737), SWAMP_LIGHT);
        assert_eq!(sumpf.get(Resolver::Foliage, 416, -988), DEFAULT_FOLIAGE);
        assert_eq!(sumpf.get(Resolver::Water, 416, -988), DEFAULT_WATER);
        let ebene = BiomeColors {
            swamp: false,
            ..sumpf
        };
        assert_eq!(ebene.get(Resolver::Grass, 416, -988), DEFAULT_GRASS);
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

    /// Die Färbung aus `BlockColors.createDefault` in 26.2, soweit sie
    /// Fläche macht: Blütenteppich und Wildblumen färben ihre Stiele mit
    /// Gras, die obere Hälfte von hohem Gras und grossem Farn nimmt die Farbe
    /// am Block darunter, die von anderen Doppelpflanzen nicht.
    #[test]
    fn farbquellen_wie_blockcolors() {
        use Resolver::*;
        for (block, soll) in [
            ("grass_block", Some(Source::Biome(Grass))),
            ("short_grass", Some(Source::Biome(Grass))),
            ("tall_grass", Some(Source::Biome(Grass))),
            ("large_fern", Some(Source::Biome(Grass))),
            ("pink_petals", Some(Source::Biome(Grass))),
            ("wildflowers", Some(Source::Biome(Grass))),
            ("sugar_cane", Some(Source::Biome(Grass))),
            ("vine", Some(Source::Biome(Foliage))),
            ("leaf_litter", Some(Source::Biome(DryFoliage))),
            ("water_cauldron", Some(Source::Biome(Water))),
            ("birch_leaves", Some(Source::Fixed(BIRCH))),
            ("cherry_leaves", None),
            ("sunflower", None),
        ] {
            assert_eq!(source_of(&format!("minecraft:{block}")), soll, "{block}");
        }
        assert!(tinted_below("minecraft:tall_grass", Some("upper")));
        assert!(tinted_below("minecraft:large_fern", Some("upper")));
        assert!(!tinted_below("minecraft:tall_grass", Some("lower")));
        assert!(!tinted_below("minecraft:sunflower", Some("upper")));
    }
}
