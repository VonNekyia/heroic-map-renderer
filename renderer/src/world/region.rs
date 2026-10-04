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
    /// Nur die Chunks darin liest die Region, siehe [`World::mit_bereich`].
    bereich: Option<[i32; 4]>,
}

/// Liegt der Chunk `(cx, cz)` im Rechteck aus Chunks `[x0, z0, x1, z1]`,
/// halb offen?
pub fn im_bereich([x0, z0, x1, z1]: [i32; 4], cx: i32, cz: i32) -> bool {
    (x0..x1).contains(&cx) && (z0..z1).contains(&cz)
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
            bereich: None,
        })
    }

    /// Die Region liest nur Chunks in `bereich`, die übrigen fehlen.
    pub fn mit_bereich(self, bereich: Option<[i32; 4]>) -> Region {
        Region { bereich, ..self }
    }

    /// Was der Kopf über jeden Chunk der Region sagt, nach z, dann x
    /// geordnet; `None` für einen leeren Tabelleneintrag und einen Chunk
    /// ausserhalb des Bereichs. Liest nur die zwei Sektoren des Kopfs.
    /// Siehe docs/benutzung/updates.md, „Was als geändert gilt“.
    pub fn stempel(&mut self) -> Result<Vec<Option<Stempel>>> {
        let mut kopf = Vec::with_capacity((HEADER_SECTORS * SECTOR) as usize);
        self.file.seek(SeekFrom::Start(0))?;
        (&mut self.file)
            .take(HEADER_SECTORS * SECTOR)
            .read_to_end(&mut kopf)
            .with_context(|| format!("Kopf von r.{}.{}.mca lesen", self.x, self.z))?;
        // Was am Kopf fehlt, etwa bei einer Datei, die der Server eben
        // anlegt, ist leer, wie im Spiel.
        kopf.resize((HEADER_SECTORS * SECTOR) as usize, 0);
        let wort = |i: usize| u32::from_be_bytes([kopf[i], kopf[i + 1], kopf[i + 2], kopf[i + 3]]);
        Ok((0..REGION * REGION)
            .map(|i| {
                let (cx, cz) = (self.x * REGION + i % REGION, self.z * REGION + i / REGION);
                let i = i as usize;
                let ort = wort(4 * i);
                (ort != 0 && self.bereich.is_none_or(|b| im_bereich(b, cx, cz))).then(|| Stempel {
                    zeit: wort(SECTOR as usize + 4 * i),
                    ort,
                })
            })
            .collect())
    }

    /// Chunk an **Welt**-Chunkkoordinaten, wenn er fertig erzeugt ist, siehe
    /// [`Chunk::is_generated`]. `None`, wenn er fehlt oder nicht fertig
    /// erzeugt ist. Darüber liest der Render; der Vorlauf, `--at` und
    /// `--scan` lesen mit [`Region::stored_chunk`] und fragen
    /// [`Chunk::is_generated`] selbst, weil sie die übrigen zählen oder
    /// nennen.
    pub fn chunk(&mut self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        Ok(self.stored_chunk(cx, cz)?.filter(Chunk::is_generated))
    }

    /// Chunk an **Welt**-Chunkkoordinaten in jedem Status, wie er in der
    /// Datei steht. `None` für einen leeren Tabelleneintrag und einen Chunk
    /// ausserhalb des Bereichs.
    ///
    /// Koordinaten aus einer anderen Region sind ein Fehler — ohne die Prüfung
    /// würde die Modulo-Umrechnung still den falschen Chunk liefern.
    ///
    /// Nennt der Chunk selbst eine andere Position, etwa aus einer von Hand
    /// kopierten Regionsdatei, steht er trotzdem an seinem Platz: so zeigt
    /// ihn das Spiel (`SerializableChunkData.read` in 26.2: "in the wrong
    /// location; relocating"), und so sehen ihn Vorlauf und Render.
    pub fn stored_chunk(&mut self, cx: i32, cz: i32) -> Result<Option<Chunk>> {
        if cx.div_euclid(REGION) != self.x || cz.div_euclid(REGION) != self.z {
            bail!(
                "Chunk ({cx}, {cz}) liegt nicht in r.{}.{}.mca",
                self.x,
                self.z
            );
        }
        if self.bereich.is_some_and(|b| !im_bereich(b, cx, cz)) {
            return Ok(None);
        }
        let Some(nbt) = self.chunk_nbt(cx, cz)? else {
            return Ok(None);
        };
        Chunk::decode(&nbt)
            .map(|mut chunk| {
                (chunk.x, chunk.z) = (cx, cz);
                Some(chunk)
            })
            .with_context(|| format!("Chunk ({cx}, {cz}) aus r.{}.{}.mca", self.x, self.z))
    }

    /// Die Chunks, die die Tabelle der Datei nennt, in **Welt**-Chunkkoordinaten,
    /// in jedem Status; mit einem Bereich nur die darin. Gelesen wird nur die
    /// Tabelle.
    pub fn vorhanden(&mut self) -> Result<Vec<(i32, i32)>> {
        Ok((0..REGION * REGION)
            .zip(self.stempel()?)
            .filter(|(_, stempel)| stempel.is_some())
            .map(|(i, _)| (self.x * REGION + i % REGION, self.z * REGION + i / REGION))
            .collect())
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

/// Was der Kopf einer Regionsdatei über einen Chunk sagt: wann das Spiel
/// ihn zuletzt geschrieben hat, in Sekunden (`RegionFile.write`), und den
/// Eintrag der Tabelle, Sektor und Länge. Schreibt das Spiel den Chunk neu,
/// ändert sich wenigstens die Zeit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stempel {
    pub zeit: u32,
    pub ort: u32,
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

    /// Der Kopf nennt je Chunk den Eintrag der Tabelle und die Zeit; ein
    /// leerer Eintrag ist kein Chunk, auch mit einer Zeit.
    #[test]
    fn stempel_aus_dem_kopf() {
        let dir = tempfile::tempdir().unwrap();
        let pfad = dir.path().join("r.-1.2.mca");
        let mut kopf = vec![0u8; 2 * SECTOR as usize];
        let setze = |kopf: &mut Vec<u8>, i: usize, ort: u32, zeit: u32| {
            kopf[4 * i..4 * i + 4].copy_from_slice(&ort.to_be_bytes());
            kopf[SECTOR as usize + 4 * i..SECTOR as usize + 4 * i + 4]
                .copy_from_slice(&zeit.to_be_bytes());
        };
        setze(&mut kopf, 0, (2 << 8) | 1, 1_700_000_000);
        setze(&mut kopf, 33, (3 << 8) | 2, 0);
        setze(&mut kopf, 1023, 0, 5);
        std::fs::write(&pfad, &kopf).unwrap();
        let stempel = Region::open(&pfad).unwrap().stempel().unwrap();
        assert_eq!(stempel.len(), 1024);
        assert_eq!(
            stempel[0],
            Some(Stempel {
                zeit: 1_700_000_000,
                ort: (2 << 8) | 1
            })
        );
        assert_eq!(
            stempel[33],
            Some(Stempel {
                zeit: 0,
                ort: (3 << 8) | 2
            }),
            "Zeit 0 schreiben manche Werkzeuge"
        );
        assert_eq!(stempel[1023], None, "leerer Eintrag");
        assert_eq!(stempel.iter().flatten().count(), 2);
        // Ein kurzer Kopf, etwa einer Datei, die der Server eben anlegt: Was
        // fehlt, ist leer.
        std::fs::write(&pfad, &kopf[..100]).unwrap();
        let kurz = Region::open(&pfad).unwrap().stempel().unwrap();
        assert_eq!(
            kurz[0],
            Some(Stempel {
                zeit: 0,
                ort: (2 << 8) | 1
            })
        );
        assert_eq!(kurz.iter().flatten().count(), 1);
        assert_eq!(
            Region::open(&pfad).unwrap().vorhanden().unwrap(),
            [(-32, 64)]
        );
        std::fs::write(&pfad, b"").unwrap();
        let leer = Region::open(&pfad).unwrap().stempel().unwrap();
        assert!(leer.iter().all(Option::is_none));
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
