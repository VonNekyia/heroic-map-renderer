use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use super::chunk::Chunk;

/// Chunks pro Regionskante.
pub const REGION: i32 = 32;

/// Eine `.mca`-Datei. `fastanvil` übernimmt nur das Containerformat
/// (Sektor-Tabelle, Kompression, ausgelagerte Chunks); der Inhalt wird von
/// [`Chunk::decode`] gelesen.
pub struct Region {
    pub x: i32,
    pub z: i32,
    inner: fastanvil::Region<File>,
}

impl Region {
    pub fn open(path: &Path) -> Result<Region> {
        let (x, z) = coords_from_name(path)
            .ok_or_else(|| anyhow!("kein Regionsdateiname: {}", path.display()))?;
        let file = File::open(path).with_context(|| format!("{} öffnen", path.display()))?;
        let inner = fastanvil::Region::from_stream(file)
            .map_err(|e| anyhow!("{} lesen: {e}", path.display()))?;
        Ok(Region { x, z, inner })
    }

    /// Chunk an **Welt**-Chunkkoordinaten. `None`, wenn er nicht generiert ist.
    pub fn chunk(&mut self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        let raw = self
            .inner
            .read_chunk(
                cx.rem_euclid(REGION) as usize,
                cz.rem_euclid(REGION) as usize,
            )
            .map_err(|e| {
                anyhow!(
                    "Chunk ({cx}, {cz}) aus r.{}.{}.mca lesen: {e}",
                    self.x,
                    self.z
                )
            })?;
        match raw {
            None => Ok(None),
            Some(raw) => Chunk::decode(&raw)
                .map(Some)
                .with_context(|| format!("Chunk ({cx}, {cz}) dekodieren")),
        }
    }
}

/// `r.18.-13.mca` -> `(18, -13)`.
pub fn coords_from_name(path: &Path) -> Option<(i32, i32)> {
    let mut parts = path.file_name()?.to_str()?.split('.');
    if parts.next()? != "r" {
        return None;
    }
    let x = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    if parts.next()? != "mca" || parts.next().is_some() {
        return None;
    }
    Some((x, z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn regionskoordinaten_aus_dateinamen() {
        let name = |s: &str| coords_from_name(&PathBuf::from(s));
        assert_eq!(name("r.18.13.mca"), Some((18, 13)));
        assert_eq!(name("r.-5.-13.mca"), Some((-5, -13)));
        assert_eq!(name("r.0.0.mca"), Some((0, 0)));
        assert_eq!(name("r.1.-14.mcc"), None);
        assert_eq!(name("level.dat"), None);
        assert_eq!(name("r.1.mca"), None);
        assert_eq!(name("r.1.2.3.mca"), None);
    }
}
