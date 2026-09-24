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

/// Wo die Server seit 26.1 den Seed ablegen: Vanilla an der Weltwurzel in
/// seinem `LevelResource.DATA`, Paper so in jeder Dimension unter
/// `dimensions/<namensraum>/<name>`.
const SEED_FILE: [&str; 3] = ["data", "minecraft", "world_gen_settings.dat"];

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
/// `dim-1` dieselben Regionen wie `DIM-1`, und `..` ist kein Name. Ergibt
/// der keine Welt, der angegebene, siehe [`locate_erst`].
fn locate(dir: &Path) -> Option<(PathBuf, String)> {
    locate_erst(
        std::fs::canonicalize(dir).ok().map(gewohnt),
        std::path::absolute(dir).ok(),
    )
}

/// Erst im kanonischen Pfad, dann im angegebenen, absolut gemacht, wie
/// cargo in `try_canonicalize`. Der kanonische folgt Links: liegt eine
/// Dimension über einen Link auf einer anderen Platte, führt er aus der
/// Welt hinaus. Und auf manchen Laufwerken gibt es ihn gar nicht, etwa auf
/// RAM-Disks, an denen `GetFinalPathNameByHandleW` scheitert.
fn locate_erst(
    kanonisch: Option<PathBuf>,
    angegeben: Option<PathBuf>,
) -> Option<(PathBuf, String)> {
    kanonisch
        .as_deref()
        .and_then(locate_in)
        .or_else(|| angegeben.as_deref().and_then(locate_in))
}

/// `canonicalize` stellt unter Windows `\\?\` voran. Ohne das Präfix steht
/// die Wurzel in Meldungen wie gewohnt da, sofern der Pfad dann dasselbe
/// meint: ein Name wie `aux` oder einer mit Punkt am Ende hiesse ohne es
/// etwas anderes, das prüft `absolute`.
fn gewohnt(pfad: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(rest) = pfad.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        let kurz = match rest.strip_prefix(r"UNC\") {
            Some(unc) => PathBuf::from(format!(r"\\{unc}")),
            None => PathBuf::from(rest),
        };
        if kurz.is_absolute() && std::path::absolute(&kurz).is_ok_and(|a| a == kurz) {
            return kurz;
        }
    }
    pfad
}

fn locate_in(dir: &Path) -> Option<(PathBuf, String)> {
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
    dimension()
        .or_else(|| is_root(dir).then(|| (dir.to_path_buf(), "minecraft:overworld".to_string())))
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

    /// Die Orte des Seeds relativ zur Weltwurzel, in der Reihenfolge, in der
    /// [`World::seed`] sucht: die Datei der Dimension selbst, die an der
    /// Wurzel, die der Paper-Oberwelt, `level.dat`.
    pub fn seed_files(&self) -> Vec<PathBuf> {
        let dimension = self.dimension().unwrap_or("minecraft:overworld");
        let (namespace, name) = dimension
            .split_once(':')
            .unwrap_or(("minecraft", dimension));
        let dimensionen = Path::new("dimensions");
        let mut out = vec![
            under(&under(dimensionen, &[namespace, name]), &SEED_FILE),
            under(Path::new(""), &SEED_FILE),
        ];
        let oberwelt = under(&under(dimensionen, &["minecraft", "overworld"]), &SEED_FILE);
        if oberwelt != out[0] {
            out.push(oberwelt);
        }
        out.push(PathBuf::from("level.dat"));
        out
    }

    /// Der Seed der Welt, mit der Dimension Grundlage ihrer Kennung im
    /// Kachelbaum. Seit 26.1 steht er in `world_gen_settings.dat` unter
    /// `data.seed`, davor in `level.dat` unter `Data.WorldGenSettings.seed`.
    ///
    /// Zuerst zählt die Datei der Dimension: Paper schreibt den Seed je
    /// Dimension, und eine Plugin-Welt hat oft einen eigenen. Sonst die an
    /// der Wurzel, wie Vanilla sie schreibt, oder die der Paper-Oberwelt,
    /// von beiden die jüngere, bei gleichem Alter die von Paper: unter
    /// Paper bleibt an der Wurzel eine ältere liegen, etwa aus der Zeit vor
    /// einer neu erzeugten Welt. Zuletzt `level.dat`.
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
        let orte: Vec<PathBuf> = self.seed_files().iter().map(|ort| root.join(ort)).collect();
        let [eigene, andere @ .., level] = orte.as_slice() else {
            unreachable!("mindestens die Dimension und level.dat");
        };
        let alter = |pfad: &Path| std::fs::metadata(pfad).and_then(|m| m.modified()).ok();
        let datei = match andere {
            _ if eigene.is_file() => Some(eigene),
            [wurzel, oberwelt] if wurzel.is_file() && oberwelt.is_file() => {
                Some(if alter(wurzel) > alter(oberwelt) {
                    wurzel
                } else {
                    oberwelt
                })
            }
            _ => andere.iter().find(|pfad| pfad.is_file()),
        };
        if let Some(datei) = datei {
            return Ok(Some(read_nbt::<GenSettings>(datei)?.data.seed));
        }
        let level = read_nbt::<Level>(level)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Scheitert `canonicalize`, oder ergibt der kanonische Pfad keine
    /// Welt, zählt der angegebene.
    #[test]
    fn ohne_kanonischen_pfad_der_angegebene() {
        let welt = tempfile::tempdir().unwrap();
        let dim = welt.path().join("DIM-1");
        std::fs::create_dir_all(&dim).unwrap();
        std::fs::write(welt.path().join("level.dat"), b"").unwrap();
        let nether = Some((
            welt.path().to_path_buf(),
            "minecraft:the_nether".to_string(),
        ));
        assert_eq!(locate_erst(None, Some(dim.clone())), nether);
        let draussen = tempfile::tempdir().unwrap();
        let aussen = Some(draussen.path().to_path_buf());
        assert_eq!(locate_erst(aussen, Some(dim.clone())), nether);
        assert_eq!(locate_erst(Some(dim), None), nether);
    }
}
