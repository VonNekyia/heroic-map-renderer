pub mod chunk;
pub mod palette;
pub mod region;

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub use chunk::{Chunk, Section};
pub use palette::BlockState;
pub use region::{REGION, Region};

/// Beide Verzeichnislayouts, die in freier Wildbahn vorkommen: das klassische
/// `world/region` und das seit Minecraft 26.1 genutzte
/// `world/dimensions/minecraft/overworld/region`.
const REGION_DIRS: [&[&str]; 2] = [
    &["region"],
    &["dimensions", "minecraft", "overworld", "region"],
];

pub struct World {
    region_dir: PathBuf,
}

impl World {
    pub fn open(root: &Path) -> Result<World> {
        for candidate in REGION_DIRS {
            let dir = candidate
                .iter()
                .fold(root.to_path_buf(), |dir, part| dir.join(part));
            if dir.is_dir() {
                return Ok(World { region_dir: dir });
            }
        }
        bail!(
            "kein region-Verzeichnis unter {} (gesucht: {})",
            root.display(),
            REGION_DIRS.map(|c| c.join("/")).join(" oder ")
        )
    }

    pub fn region_dir(&self) -> &Path {
        &self.region_dir
    }

    /// Alle vorhandenen Regionen, aufsteigend sortiert.
    pub fn regions(&self) -> Result<Vec<(i32, i32)>> {
        let entries = std::fs::read_dir(&self.region_dir)
            .with_context(|| format!("{} lesen", self.region_dir.display()))?;
        let mut regions: Vec<(i32, i32)> = entries
            .filter_map(|e| region::coords_from_name(&e.ok()?.path()))
            .collect();
        regions.sort_unstable();
        Ok(regions)
    }

    pub fn region(&self, rx: i32, rz: i32) -> Result<Option<Region>> {
        let path = self.region_dir.join(format!("r.{rx}.{rz}.mca"));
        if !path.is_file() {
            return Ok(None);
        }
        Region::open(&path).map(Some)
    }

    /// Einzelnen Chunk laden. Öffnet die Regionsdatei jedes Mal neu — für
    /// CLI und Tests gedacht, nicht für den Renderpfad.
    pub fn chunk(&self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        let Some(mut region) = self.region(cx.div_euclid(REGION), cz.div_euclid(REGION))? else {
            return Ok(None);
        };
        region.chunk(cx, cz)
    }

    /// Der Seed der Welt, ihre Kennung im Kachelbaum. Seit 26.1 steht er
    /// neben den Regionen der Oberwelt in
    /// `data/minecraft/world_gen_settings.dat` unter `data.seed`, davor in
    /// `level.dat` unter `Data.WorldGenSettings.seed`. `None`, wenn keine
    /// der beiden Dateien da ist.
    pub fn seed(&self) -> Result<Option<i64>> {
        #[derive(Deserialize)]
        struct Seed {
            seed: i64,
        }
        #[derive(Deserialize)]
        struct GenSettings {
            data: Seed,
        }
        #[derive(Deserialize)]
        struct Level {
            #[serde(rename = "Data")]
            data: LevelData,
        }
        #[derive(Deserialize)]
        struct LevelData {
            #[serde(rename = "WorldGenSettings")]
            settings: Option<Seed>,
        }

        let dir = self.region_dir.parent().expect("region liegt in der Welt");
        let neu = dir.join("data/minecraft/world_gen_settings.dat");
        if neu.is_file() {
            return Ok(Some(read_nbt::<GenSettings>(&neu)?.data.seed));
        }
        let alt = dir.join("level.dat");
        if alt.is_file() {
            return Ok(read_nbt::<Level>(&alt)?.data.settings.map(|s| s.seed));
        }
        Ok(None)
    }
}

/// Eine gzip-gepackte NBT-Datei, wie `level.dat`.
fn read_nbt<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let mut nbt = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| flate2::read::GzDecoder::new(file).read_to_end(&mut nbt))
        .with_context(|| format!("{} lesen", path.display()))?;
    fastnbt::from_bytes(&nbt).with_context(|| format!("{} auswerten", path.display()))
}
