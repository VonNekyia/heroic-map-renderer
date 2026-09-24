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

/// Wo die Server den Seed ablegen, relativ zur Weltwurzel: Vanilla seit
/// 26.1 in seinem `LevelResource.DATA`, Paper bei jeder Dimension, gelesen
/// wird der der Oberwelt. In dieser Reihenfolge, danach `level.dat`.
const SEED_FILES: [&[&str]; 2] = [
    &["data", "minecraft", "world_gen_settings.dat"],
    &[
        "dimensions",
        "minecraft",
        "overworld",
        "data",
        "minecraft",
        "world_gen_settings.dat",
    ],
];

pub struct World {
    /// Weltwurzel und Dimension, falls das Verzeichnis zu einer Welt
    /// gehört, siehe [`locate`].
    home: Option<(PathBuf, String)>,
    region_dir: PathBuf,
}

/// Hängt Pfadkomponenten einzeln an. Ein Schrägstrich in `join` scheitert
/// unter Windows an Pfaden mit dem Präfix für lange Pfade.
fn under(root: &Path, parts: &[&str]) -> PathBuf {
    parts
        .iter()
        .fold(root.to_path_buf(), |dir, part| dir.join(part))
}

/// Weltwurzel und Dimension zu dem Verzeichnis, das `--world` nennt.
///
/// Die Wurzel, das Verzeichnis mit `level.dat`, ist die Oberwelt, auch wenn
/// ihre Regionen seit 26.1 unter `dimensions/minecraft/overworld` liegen.
/// Die anderen Dimensionen liegen darunter: seit 1.16 und in 26.x unter
/// `dimensions/<namensraum>/<name>`, Nether und End bis 1.21 als `DIM-1`
/// und `DIM1`, bei Bukkit in eigenen Welten wie `world_nether/DIM-1` mit
/// eigenem `level.dat`. Diese Layouts gehen vor, eine Kopie von `level.dat`
/// in einer Dimension macht sie nicht zur Oberwelt. Ohne `level.dat` darüber
/// lässt sich die Welt nicht erkennen, etwa bei einer Kopie ohne sie.
///
/// Es zählt der Pfad, wie er auf der Platte steht: unter Windows öffnet
/// `dim-1` dieselben Regionen wie `DIM-1`, und `..` ist kein Name.
fn locate(dir: &Path) -> Option<(PathBuf, String)> {
    let dir = std::fs::canonicalize(dir).ok()?;
    let is_root = |dir: &Path| dir.join("level.dat").is_file();
    let dimension = || {
        let name = dir.file_name()?.to_str()?;
        let parent = dir.parent()?;
        let legacy = match name {
            "DIM-1" => Some("minecraft:the_nether"),
            "DIM1" => Some("minecraft:the_end"),
            _ => None,
        };
        if let Some(dimension) = legacy {
            return is_root(parent).then(|| (parent.to_path_buf(), dimension.to_string()));
        }
        let namespace = parent.file_name()?.to_str()?;
        let dimensions = parent.parent()?;
        let root = dimensions.parent()?;
        (dimensions.file_name()? == "dimensions" && is_root(root))
            .then(|| (root.to_path_buf(), format!("{namespace}:{name}")))
    };
    dimension().or_else(|| is_root(&dir).then(|| (dir.clone(), "minecraft:overworld".to_string())))
}

impl World {
    pub fn open(root: &Path) -> Result<World> {
        for candidate in REGION_DIRS {
            let dir = under(root, candidate);
            if dir.is_dir() {
                return Ok(World {
                    home: locate(root),
                    region_dir: dir,
                });
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

    /// Die Dimension, etwa `minecraft:the_nether`, falls das Verzeichnis zu
    /// einer Welt gehört.
    pub fn dimension(&self) -> Option<&str> {
        self.home.as_ref().map(|(_, dimension)| dimension.as_str())
    }

    /// Der Seed der Welt, mit der Dimension Grundlage ihrer Kennung im
    /// Kachelbaum. Alle Dimensionen einer Welt tragen denselben, gelesen
    /// wird er an der Weltwurzel, siehe [`locate`]. Seit 26.1 steht er in
    /// `world_gen_settings.dat` unter `data.seed`, siehe `SEED_FILES`,
    /// davor in `level.dat` unter `Data.WorldGenSettings.seed`.
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

        let Some((root, _)) = &self.home else {
            return Ok(None);
        };
        for parts in SEED_FILES {
            let path = under(root, parts);
            if path.is_file() {
                return Ok(Some(read_nbt::<GenSettings>(&path)?.data.seed));
            }
        }
        let level = read_nbt::<Level>(&root.join("level.dat"))?;
        Ok(level.data.settings.map(|s| s.seed))
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
