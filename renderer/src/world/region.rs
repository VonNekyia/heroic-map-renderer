use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use super::chunk::Chunk;

/// Chunks pro Regionskante.
pub const REGION: i32 = 32;
/// Regionsdateien sind in 4-KiB-Sektoren organisiert.
const SECTOR: u64 = 4096;
/// Zwei Sektoren Kopf: Chunk-Tabelle und Zeitstempel-Tabelle.
const HEADER_SECTORS: u64 = 2;
/// Kopf eines Chunk-Datensatzes: 4 Byte Länge, 1 Byte Kompressionsverfahren.
const ENTRY_HEADER: u64 = 5;
/// Bit im Kompressionsbyte: die Chunkdaten liegen in einer eigenen
/// `c.<x>.<z>.mcc`-Datei, weil sie nicht in 255 Sektoren passen.
const EXTERNAL: u8 = 0x80;

/// Eine `.mca`-Datei.
///
/// Das Containerformat ist klein genug, um es direkt zu lesen: Sektor-Tabelle,
/// Längenfeld, Kompressionsbyte, optional ausgelagerte Chunkdaten. Der Inhalt
/// wird von [`Chunk::decode`] gelesen.
pub struct Region {
    pub x: i32,
    pub z: i32,
    file: File,
    /// Verzeichnis der Regionsdatei — dort liegen auch die `.mcc`-Dateien.
    dir: PathBuf,
    file_len: u64,
}

impl Region {
    pub fn open(path: &Path) -> Result<Region> {
        let (x, z) = coords_from_name(path)
            .ok_or_else(|| anyhow!("kein Regionsdateiname: {}", path.display()))?;
        let file = File::open(path).with_context(|| format!("{} öffnen", path.display()))?;
        let file_len = file
            .metadata()
            .with_context(|| format!("{} prüfen", path.display()))?
            .len();
        Ok(Region {
            x,
            z,
            file,
            dir: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            file_len,
        })
    }

    /// Chunk an **Welt**-Chunkkoordinaten. `None`, wenn er nicht generiert ist.
    ///
    /// Koordinaten aus einer anderen Region sind ein Fehler — ohne die Prüfung
    /// würde die Modulo-Umrechnung still den falschen Chunk liefern.
    pub fn chunk(&mut self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        if cx.div_euclid(REGION) != self.x || cz.div_euclid(REGION) != self.z {
            bail!(
                "Chunk ({cx}, {cz}) liegt nicht in r.{}.{}.mca",
                self.x,
                self.z
            );
        }
        let Some(nbt) = self.chunk_nbt(cx, cz)? else {
            return Ok(None);
        };
        Chunk::decode(&nbt)
            .map(Some)
            .with_context(|| format!("Chunk ({cx}, {cz}) aus r.{}.{}.mca", self.x, self.z))
    }

    /// Unkomprimiertes Chunk-NBT, oder `None` für einen leeren Tabelleneintrag.
    fn chunk_nbt(&mut self, cx: i32, cz: i32) -> Result<Option<Vec<u8>>> {
        let Some((offset, sectors)) = self.entry(cx.rem_euclid(REGION), cz.rem_euclid(REGION))?
        else {
            return Ok(None);
        };

        let mut head = [0u8; ENTRY_HEADER as usize];
        self.file.seek(SeekFrom::Start(offset * SECTOR))?;
        self.file
            .read_exact(&mut head)
            .with_context(|| format!("Chunk ({cx}, {cz}): Datensatzkopf lesen"))?;

        // Das Längenfeld zählt das Kompressionsbyte mit, ist also nie 0.
        let declared = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
        let scheme = head[4];
        let Some(payload_len) = declared.checked_sub(1) else {
            bail!("Chunk ({cx}, {cz}): Längenfeld ist 0");
        };

        let data = if scheme & EXTERNAL != 0 {
            // Ausgelagerte Chunks stehen vollständig in der .mcc-Datei; das
            // Längenfeld in der Region beschreibt sie nicht.
            let path = self.dir.join(format!("c.{cx}.{cz}.mcc"));
            std::fs::read(&path).with_context(|| {
                format!(
                    "Chunk ({cx}, {cz}) ist ausgelagert, {} nicht lesbar",
                    path.display()
                )
            })?
        } else {
            // Die Sektorzahl ist eine Allokationsangabe: der letzte Datensatz
            // einer Datei wird nicht auf die volle Sektorgrenze aufgefüllt.
            // Begrenzend ist deshalb, was von beidem kleiner ist — sonst
            // fallen genau diese ungepolsterten Chunks aus der Karte.
            let allocated = sectors * SECTOR - ENTRY_HEADER;
            let in_file = self.file_len - offset * SECTOR - ENTRY_HEADER;
            if payload_len > allocated.min(in_file) {
                bail!(
                    "Chunk ({cx}, {cz}): Längenfeld {payload_len} überschreitet die belegten \
                     {sectors} Sektoren ({allocated} Byte) oder das Dateiende ({in_file} Byte)"
                );
            }
            let mut data = vec![0u8; payload_len as usize];
            self.file
                .read_exact(&mut data)
                .with_context(|| format!("Chunk ({cx}, {cz}): {payload_len} Bytes lesen"))?;
            data
        };

        decompress(scheme & !EXTERNAL, &data)
            .map(Some)
            .with_context(|| format!("Chunk ({cx}, {cz}) entpacken"))
    }

    /// Tabelleneintrag: Sektor-Offset und Sektor-Anzahl, beide geprüft.
    fn entry(&mut self, lx: i32, lz: i32) -> Result<Option<(u64, u64)>> {
        self.file
            .seek(SeekFrom::Start(4 * (lx + lz * REGION) as u64))?;
        let mut buf = [0u8; 4];
        self.file.read_exact(&mut buf)?;

        let offset = u64::from(u32::from_be_bytes([0, buf[0], buf[1], buf[2]]));
        let sectors = u64::from(buf[3]);
        if offset == 0 && sectors == 0 {
            return Ok(None);
        }
        if offset < HEADER_SECTORS || sectors == 0 {
            bail!("lokaler Chunk ({lx}, {lz}): Tabelleneintrag zeigt in den Dateikopf");
        }
        // Geprüft wird nur, dass der Datensatzkopf in der Datei liegt. Ob die
        // volle Sektorzahl physisch vorhanden ist, sagt nichts aus: der letzte
        // Datensatz einer Regionsdatei ist regelmäßig kürzer.
        if offset * SECTOR + ENTRY_HEADER > self.file_len {
            bail!("lokaler Chunk ({lx}, {lz}): Tabelleneintrag zeigt hinter das Dateiende");
        }
        Ok(Some((offset, sectors)))
    }
}

fn decompress(scheme: u8, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    match scheme {
        1 => {
            flate2::read::GzDecoder::new(data).read_to_end(&mut out)?;
        }
        2 => {
            flate2::read::ZlibDecoder::new(data).read_to_end(&mut out)?;
        }
        3 => out.extend_from_slice(data),
        4 => {
            lz4_java_wrc::Lz4BlockInput::new(data).read_to_end(&mut out)?;
        }
        other => bail!("unbekanntes Kompressionsverfahren {other}"),
    }
    Ok(out)
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

    #[test]
    fn unbekanntes_verfahren_ist_fehler() {
        assert!(decompress(7, b"egal").is_err());
        // das externe Bit muss der Aufrufer vorher entfernen
        assert!(decompress(EXTERNAL | 2, b"egal").is_err());
    }

    #[test]
    fn unkomprimiert_wird_durchgereicht() {
        assert_eq!(decompress(3, b"rohdaten").unwrap(), b"rohdaten");
    }
}
