pub mod chunk;
pub mod palette;
pub mod region;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub use chunk::{Chunk, Section};
pub use palette::BlockState;
pub use region::{REGION, Region};

/// Beide Verzeichnislayouts, die in freier Wildbahn vorkommen: das klassische
/// `world/region` und das ab 1.21 von Paper/Vanilla genutzte
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
}
