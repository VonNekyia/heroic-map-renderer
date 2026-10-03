pub mod biomzoom;
pub mod chunk;
pub mod palette;
pub mod region;

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub use chunk::{Abdruck, Blockdaten, Chunk, Muster, Section};
pub use palette::BlockState;
pub use region::{REGION, Region, Stempel, im_bereich};

/// Je Region die Stempel ihrer Chunks, siehe [`Region::stempel`].
pub type Stempelkarte = BTreeMap<(i32, i32), Vec<Option<Stempel>>>;

/// Wo unter `--world` die Regionen liegen: direkt darunter in einer
/// Dimension, `world/dimensions/<namensraum>/<name>/region`, und für die
/// Oberwelt unter der Wurzel in `dimensions/minecraft/overworld/region`,
/// wie seit Minecraft 26.1.
const REGION_DIRS: [&[&str]; 2] = [
    &["region"],
    &["dimensions", "minecraft", "overworld", "region"],
];

/// Wo die Server seit 26.1 den Seed ablegen: Vanilla an der Weltwurzel in
/// seinem `LevelResource.DATA`, Paper so in jeder Dimension unter
/// `dimensions/<namensraum>/<name>`.
const SEED_FILE: [&str; 3] = ["data", "minecraft", "world_gen_settings.dat"];

/// Wo die Weltgrenze liegt, seit 26.1 wie der Seed, siehe [`SEED_FILE`].
const GRENZE_FILE: [&str; 3] = ["data", "minecraft", "world_border.dat"];

/// Woher eine Dimension ihr Gelände hat: der Generator ihres Eintrags in
/// `world_gen_settings.dat`, `data.dimensions.<dimension>.generator`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Generator {
    /// `minecraft:noise` mit den Noise Settings dieser ID.
    Noise(String),
    /// `minecraft:noise` mit Noise Settings in der Datei, mit ihrem
    /// `sea_level`.
    NoiseMit(i32),
    /// `minecraft:flat`.
    Flat,
    /// `minecraft:debug`.
    Debug,
    /// Ein anderer, etwa aus einer Mod.
    Anderer(String),
}

/// Die Weltgrenze aus `world_border.dat`: Mitte und Kantenlänge in Blöcken.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Grenze {
    pub center_x: f64,
    pub center_z: f64,
    pub size: f64,
}

#[derive(Clone)]
pub struct World {
    /// Weltwurzel und Dimension, falls das Verzeichnis zu einer Welt
    /// gehört, siehe [`locate`].
    home: Option<(PathBuf, String)>,
    region_dir: PathBuf,
    /// Nur die Chunks darin gehören zur Welt, siehe [`World::mit_bereich`].
    bereich: Option<[i32; 4]>,
    /// `Data.DataVersion` aus `level.dat` der Weltwurzel.
    datenversion: Option<i32>,
}

/// Die Datenversion von 26.3. Eine Welt davor zeichnet die weiche
/// Beleuchtung wie 26.2, siehe [`World::ecke_wie_26_2`].
pub const DATENVERSION_26_3: i32 = 5023;

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
/// ihre Regionen unter `dimensions/minecraft/overworld` liegen. Die anderen
/// Dimensionen liegen darunter in `dimensions/<namensraum>/<name>`. Dieses
/// Layout geht vor, eine Kopie von `level.dat` in einer Dimension macht sie
/// nicht zur Oberwelt. Ohne `level.dat` darüber lässt sich die Welt nicht
/// erkennen, etwa bei einer Kopie ohne sie. `DIM-1` und `DIM1` gibt es nur
/// in Welten vor 26.1: ihre Regionen öffnet der Renderer noch, eine
/// Dimension erkennt er darin nicht, und sie bekommen keine Kennung.
///
/// Es zählt der Pfad, wie er auf der Platte steht: unter Windows öffnet
/// `DIMENSIONS/MINECRAFT/THE_NETHER` dieselben Regionen wie
/// `dimensions/minecraft/the_nether`, und `..` ist kein Name. Ergibt der
/// keine Welt, der angegebene, siehe [`locate_erst`].
/// Siehe docs/benutzung/welten.md, „Weltwurzel und Dimension“.
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
                let home = locate(root);
                let datenversion = match &home {
                    Some((wurzel, _)) => datenversion(&wurzel.join("level.dat"))?,
                    None => None,
                };
                return Ok(World {
                    home,
                    region_dir: dir,
                    bereich: None,
                    datenversion,
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

    /// Die Welt nur aus den Chunks in `bereich`, einem Rechteck aus Chunks
    /// `[x0, z0, x1, z1]`, halb offen; die übrigen fehlen wie nie erzeugte,
    /// für jeden, der liest. Siehe docs/benutzung/kacheln.md, „Ein Rechteck
    /// der Welt: `--area`“.
    pub fn mit_bereich(self, bereich: Option<[i32; 4]>) -> World {
        World { bereich, ..self }
    }

    /// Das Rechteck aus [`World::mit_bereich`].
    pub fn bereich(&self) -> Option<[i32; 4]> {
        self.bereich
    }

    /// `Data.DataVersion` aus `level.dat`; `None` ohne Weltwurzel oder ohne
    /// das Feld.
    pub fn datenversion(&self) -> Option<i32> {
        self.datenversion
    }

    /// Ob die weiche Beleuchtung die Sicht in der Ecke wie 26.2 fragt: in
    /// einer Welt vor [`DATENVERSION_26_3`]. Ohne Datenversion wie 26.3.
    /// Siehe docs/renderer/weiche-beleuchtung.md, „Welten aus 26.2“.
    pub fn ecke_wie_26_2(&self) -> bool {
        self.datenversion.is_some_and(|v| v < DATENVERSION_26_3)
    }

    /// Alle vorhandenen Regionen, aufsteigend sortiert; mit einem Bereich
    /// nur die, die ihn berühren.
    pub fn regions(&self) -> Result<Vec<(i32, i32)>> {
        let entries = std::fs::read_dir(&self.region_dir)
            .with_context(|| format!("{} lesen", self.region_dir.display()))?;
        let beruehrt = |&(rx, rz): &(i32, i32)| {
            self.bereich.is_none_or(|[x0, z0, x1, z1]| {
                rx * REGION < x1
                    && x0 < (rx + 1) * REGION
                    && rz * REGION < z1
                    && z0 < (rz + 1) * REGION
            })
        };
        let mut regions: Vec<(i32, i32)> = entries
            .filter_map(|e| region::coords_from_name(&e.ok()?.path()))
            .filter(beruehrt)
            .collect();
        regions.sort_unstable();
        Ok(regions)
    }

    /// Die Stempel aller Regionen, siehe [`Region::stempel`]. Eine Region,
    /// die zwischen Liste und Lesen verschwindet, fehlt.
    pub fn stempel(&self) -> Result<Stempelkarte> {
        use rayon::prelude::*;
        self.regions()?
            .into_par_iter()
            .map(|(rx, rz)| -> Result<_> {
                let Some(mut region) = self.region(rx, rz)? else {
                    return Ok(None);
                };
                Ok(Some(((rx, rz), region.stempel()?)))
            })
            .filter_map(Result::transpose)
            .collect()
    }

    pub fn region(&self, rx: i32, rz: i32) -> Result<Option<Region>> {
        let path = self.region_dir.join(format!("r.{rx}.{rz}.mca"));
        if !path.is_file() {
            return Ok(None);
        }
        Region::open(&path).map(|region| Some(region.mit_bereich(self.bereich)))
    }

    /// Einzelnen Chunk laden, wenn er fertig erzeugt ist, wie
    /// [`Region::chunk`]. Öffnet die Regionsdatei jedes Mal neu — für CLI
    /// und Tests gedacht, nicht für den Renderpfad.
    pub fn chunk(&self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        let Some(mut region) = self.region(cx.div_euclid(REGION), cz.div_euclid(REGION))? else {
            return Ok(None);
        };
        region.chunk(cx, cz)
    }

    /// Wie [`World::chunk`], aber in jedem Status, wie
    /// [`Region::stored_chunk`].
    pub fn stored_chunk(&self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        let Some(mut region) = self.region(cx.div_euclid(REGION), cz.div_euclid(REGION))? else {
            return Ok(None);
        };
        region.stored_chunk(cx, cz)
    }

    /// Die Dimension, etwa `minecraft:the_nether`, falls das Verzeichnis zu
    /// einer Welt gehört.
    pub fn dimension(&self) -> Option<&str> {
        self.home.as_ref().map(|(_, dimension)| dimension.as_str())
    }

    /// Die Orte des Seeds relativ zur Weltwurzel, in der Reihenfolge, in der
    /// [`World::seed`] sucht: die Datei der Dimension selbst, die an der
    /// Wurzel, die der Paper-Oberwelt.
    pub fn seed_files(&self) -> Vec<PathBuf> {
        self.orte(&SEED_FILE)
    }

    /// Die Orte einer Datei in `data/minecraft` relativ zur Weltwurzel, wie
    /// [`World::seed_files`].
    fn orte(&self, datei: &[&str]) -> Vec<PathBuf> {
        let dimension = self.dimension().unwrap_or("minecraft:overworld");
        let (namespace, name) = dimension
            .split_once(':')
            .unwrap_or(("minecraft", dimension));
        let dimensionen = Path::new("dimensions");
        let mut out = vec![
            under(&under(dimensionen, &[namespace, name]), datei),
            under(Path::new(""), datei),
        ];
        let oberwelt = under(&under(dimensionen, &["minecraft", "overworld"]), datei);
        if oberwelt != out[0] {
            out.push(oberwelt);
        }
        out
    }

    /// Welche der Orte aus [`World::orte`] gilt, wie beim Seed: zuerst die
    /// Datei der Dimension, sonst die an der Wurzel oder die der
    /// Paper-Oberwelt, von beiden die jüngere, bei gleichem Alter die von
    /// Paper. `None` ohne Welt oder ohne eine der Dateien.
    fn datei(&self, datei: &[&str]) -> Option<PathBuf> {
        let (root, _) = self.home.as_ref()?;
        let orte: Vec<PathBuf> = self.orte(datei).iter().map(|ort| root.join(ort)).collect();
        let [eigene, andere @ ..] = orte.as_slice() else {
            unreachable!("mindestens die Dimension");
        };
        let alter = |pfad: &Path| std::fs::metadata(pfad).and_then(|m| m.modified()).ok();
        match andere {
            _ if eigene.is_file() => Some(eigene),
            [wurzel, oberwelt] if wurzel.is_file() && oberwelt.is_file() => {
                Some(if alter(wurzel) > alter(oberwelt) {
                    wurzel
                } else {
                    oberwelt
                })
            }
            _ => andere.iter().find(|pfad| pfad.is_file()),
        }
        .cloned()
    }

    /// Der Generator der Dimension aus `world_gen_settings.dat`, gesucht wie
    /// der Seed; `None`, wenn die Datei fehlt oder die Dimension nicht
    /// nennt. Siehe docs/benutzung/welten.md, „Wasserspiegel“.
    pub fn generator(&self) -> Result<Option<Generator>> {
        #[derive(Deserialize)]
        struct Eintrag {
            generator: Roh,
        }
        #[derive(Deserialize)]
        struct Roh {
            #[serde(rename = "type")]
            art: String,
            settings: Option<fastnbt::Value>,
        }
        #[derive(Deserialize)]
        struct Daten {
            #[serde(default)]
            dimensions: std::collections::HashMap<String, Eintrag>,
        }
        #[derive(Deserialize)]
        struct GenSettings {
            data: Daten,
        }

        let Some(datei) = self.datei(&SEED_FILE) else {
            return Ok(None);
        };
        let dimension = self.dimension().unwrap_or("minecraft:overworld");
        let mut settings = read_nbt::<GenSettings>(&datei)?;
        let Some(eintrag) = settings.data.dimensions.remove(dimension) else {
            return Ok(None);
        };
        let Roh { art, settings } = eintrag.generator;
        Ok(Some(match (art.as_str(), settings) {
            ("minecraft:noise", Some(fastnbt::Value::String(id))) => Generator::Noise(id),
            ("minecraft:noise", Some(fastnbt::Value::Compound(werte))) => {
                match werte.get("sea_level") {
                    Some(fastnbt::Value::Int(meer)) => Generator::NoiseMit(*meer),
                    _ => Generator::Anderer(art),
                }
            }
            ("minecraft:flat", _) => Generator::Flat,
            ("minecraft:debug", _) => Generator::Debug,
            _ => Generator::Anderer(art),
        }))
    }

    /// Die Hülle der fertig erzeugten Chunks, `[x0, z0, x1, z1]` in Chunks,
    /// halb offen, mit einem Bereich nur darin; `None` ohne einen. So ist sie
    /// in jedem Lauf dieselbe, auch in einem Ausschnitt.
    /// Siehe docs/benutzung/map-json.md, „Die Welt“.
    pub fn huelle(&self) -> Result<Option<[i32; 4]>> {
        let mut da = Vec::new();
        for (rx, rz) in self.regions()? {
            if let Some(mut region) = self.region(rx, rz)? {
                da.extend(region.vorhanden()?);
            }
        }
        let Some(&(x, z)) = da.first() else {
            return Ok(None);
        };
        let mut b = da
            .iter()
            .fold([x, z, x + 1, z + 1], |[x0, z0, x1, z1], &(x, z)| {
                [x0.min(x), z0.min(z), x1.max(x + 1), z1.max(z + 1)]
            });
        // Jede Seite rückt nach innen, bis auf ihr ein fertig erzeugter Chunk
        // liegt. Der bleibt im Rechteck und auf seiner Seite, also reicht ein
        // Durchgang über die vier.
        for seite in 0..4 {
            loop {
                if b[0] >= b[2] || b[1] >= b[3] {
                    return Ok(None);
                }
                let linie = if seite < 2 { b[seite] } else { b[seite] - 1 };
                let mut traegt = false;
                for &(cx, cz) in &da {
                    let auf = if seite % 2 == 0 { cx } else { cz } == linie;
                    if auf && im_bereich(b, cx, cz) && self.chunk(cx, cz)?.is_some() {
                        traegt = true;
                        break;
                    }
                }
                if traegt {
                    break;
                }
                b[seite] += if seite < 2 { 1 } else { -1 };
            }
        }
        Ok(Some(b))
    }

    /// Die Weltgrenze der Dimension aus `world_border.dat`, gesucht wie der
    /// Seed; `None`, wenn die Datei fehlt.
    pub fn grenze(&self) -> Result<Option<Grenze>> {
        #[derive(Deserialize)]
        struct Datei {
            data: Grenze,
        }
        self.datei(&GRENZE_FILE)
            .map(|datei| Ok(read_nbt::<Datei>(&datei)?.data))
            .transpose()
    }

    /// Der Seed der Welt, mit der Dimension Grundlage ihrer Kennung im
    /// Kachelbaum. Er steht in `world_gen_settings.dat` unter `data.seed`;
    /// Welten vor 26.1 trugen ihn in `level.dat`, die liest der Renderer
    /// nicht.
    ///
    /// Zuerst zählt die Datei der Dimension, sonst die an der Wurzel oder die
    /// der Paper-Oberwelt, von beiden die jüngere, bei gleichem Alter die von
    /// Paper.
    /// Siehe docs/benutzung/welten.md, „Wo der Seed steht“.
    pub fn seed(&self) -> Result<Option<i64>> {
        #[derive(Deserialize)]
        struct Seed {
            seed: i64,
        }
        #[derive(Deserialize)]
        struct GenSettings {
            data: Seed,
        }

        self.datei(&SEED_FILE)
            .map(|datei| Ok(read_nbt::<GenSettings>(&datei)?.data.seed))
            .transpose()
    }
}

/// `Data.DataVersion` aus `level.dat`. Gelesen wird nur dieses Feld.
fn datenversion(level: &Path) -> Result<Option<i32>> {
    #[derive(Deserialize)]
    struct Daten {
        #[serde(rename = "DataVersion")]
        version: Option<i32>,
    }
    #[derive(Deserialize)]
    struct Level {
        #[serde(rename = "Data")]
        daten: Daten,
    }
    Ok(read_nbt::<Level>(level)?.daten.version)
}

/// Eine gzip-gepackte NBT-Datei, wie `world_gen_settings.dat`.
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
        let dim = welt.path().join("dimensions/minecraft/the_nether");
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
