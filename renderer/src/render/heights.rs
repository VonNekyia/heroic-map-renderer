//! Höhen je Region für die Koordinatenanzeige im Frontend: je 4×4
//! Blockspalten der Median der obersten Blöcke, die nicht Luft sind. Sie
//! kommen aus der Heightmap `WORLD_SURFACE`, die das Spiel in jedem Chunk
//! speichert; der Vorlauf liest sie mit, einen eigenen Durchgang gibt es
//! nicht.
//! Siehe docs/benutzung/map-json.md, „Höhen“.

use std::io::{Read, Write};

use anyhow::{Result, ensure};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::world::{Chunk, REGION};

/// Blöcke je Kante einer Zelle.
pub const CELL: usize = 4;

/// Zellen je Kante einer Region.
pub const EDGE: usize = REGION as usize * 16 / CELL;

/// Zellen je Kante eines Chunks.
const JE_CHUNK: usize = 16 / CELL;

/// Eine Zelle ohne Block oder ohne Chunk.
pub const EMPTY: i16 = i16::MIN;

/// Pfadmuster der Dateien in `map.json`, relativ zu ihr.
pub const PATTERN: &str = "heights/{x}.{z}.bin";

/// Wo die Höhen der Region (rx, rz) liegen, relativ zu `map.json`.
pub fn path_of(rx: i32, rz: i32) -> String {
    PATTERN
        .replace("{x}", &rx.to_string())
        .replace("{z}", &rz.to_string())
}

/// Die Region zu einem Dateinamen aus [`PATTERN`], etwa `-1.3.bin`.
pub fn region_of(name: &str) -> Option<(i32, i32)> {
    let (x, z) = name.strip_suffix(".bin")?.split_once('.')?;
    Some((x.parse().ok()?, z.parse().ok()?))
}

/// Die Höhen einer Region, zeilenweise nach z: die Zelle mit den Spalten
/// (x, z) steht an ⌊(z − 512·rz)/4⌋·128 + ⌊(x − 512·rx)/4⌋.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heights(Vec<i16>);

impl Default for Heights {
    fn default() -> Self {
        Heights(vec![EMPTY; EDGE * EDGE])
    }
}

impl Heights {
    /// Die Höhe einer Zelle in Zellen der Region, 0 bis 127 je Achse.
    pub fn get(&self, x: usize, z: usize) -> i16 {
        self.0[z * EDGE + x]
    }

    /// Trägt die 16 Zellen eines Chunks ein: je Zelle der obere Median der
    /// Spalten mit Block, nach dem Sortieren also der Wert an der Stelle
    /// Anzahl/2; ohne Spalte mit Block [`EMPTY`].
    pub fn record(&mut self, chunk: &Chunk) {
        let oben = chunk.surface();
        let (x0, z0) = (
            chunk.x.rem_euclid(REGION) as usize * JE_CHUNK,
            chunk.z.rem_euclid(REGION) as usize * JE_CHUNK,
        );
        for zelle in 0..JE_CHUNK * JE_CHUNK {
            let (cx, cz) = (zelle % JE_CHUNK, zelle / JE_CHUNK);
            let mut werte: Vec<i32> = (0..CELL * CELL)
                .filter_map(|i| oben[(cz * CELL + i / CELL) * 16 + cx * CELL + i % CELL])
                .collect();
            werte.sort_unstable();
            self.0[(z0 + cz) * EDGE + x0 + cx] =
                werte.get(werte.len() / 2).map_or(EMPTY, |&y| y as i16);
        }
    }

    /// Übernimmt aus `old` die Chunks, die dieser Lauf nicht gelesen hat:
    /// `read` trägt je Chunkplatz der Region ein Flag, zeilenweise nach z.
    pub fn keep_unread(&mut self, old: &Heights, read: &[bool]) {
        let region = REGION as usize;
        for (platz, _) in read.iter().enumerate().filter(|(_, gelesen)| !**gelesen) {
            let (x0, z0) = ((platz % region) * JE_CHUNK, (platz / region) * JE_CHUNK);
            for z in z0..z0 + JE_CHUNK {
                let zeile = z * EDGE + x0..z * EDGE + x0 + JE_CHUNK;
                self.0[zeile.clone()].copy_from_slice(&old.0[zeile]);
            }
        }
    }

    /// Die Datei: ein zlib-Strom (RFC 1950), darin die Werte als i16
    /// little-endian.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let roh: Vec<u8> = self.0.iter().flat_map(|h| h.to_le_bytes()).collect();
        let mut packer = ZlibEncoder::new(Vec::new(), Compression::default());
        packer.write_all(&roh)?;
        Ok(packer.finish()?)
    }

    /// Liest eine Datei aus [`Heights::encode`].
    pub fn decode(daten: &[u8]) -> Result<Heights> {
        let mut roh = Vec::new();
        ZlibDecoder::new(daten).read_to_end(&mut roh)?;
        ensure!(
            roh.len() == EDGE * EDGE * 2,
            "{} Bytes statt {} für {EDGE} × {EDGE} Höhen",
            roh.len(),
            EDGE * EDGE * 2
        );
        let (paare, _) = roh.as_chunks::<2>();
        Ok(Heights(
            paare.iter().map(|&b| i16::from_le_bytes(b)).collect(),
        ))
    }
}

/// Die Höhen einer Region, wie der Vorlauf sie liest: je Chunkplatz,
/// zeilenweise nach z, ob er gelesen ist. Gelesen sind die Chunks, die der
/// Lauf liest, auch die ohne Block; einer, den es dort nicht gibt, ist
/// gelesen und leer.
#[derive(Debug)]
pub struct RegionHeights {
    pub x: i32,
    pub z: i32,
    pub heights: Heights,
    pub read: Vec<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datei_und_region() {
        assert_eq!(path_of(-1, 3), "heights/-1.3.bin");
        assert_eq!(region_of("-1.3.bin"), Some((-1, 3)));
        assert_eq!(region_of("map.json"), None);
        assert_eq!(region_of("1.2.bin.7.tmp"), None);
    }

    /// Kodieren und Lesen geben dieselben Höhen, little-endian und
    /// zeilenweise nach z, wie das Frontend sie liest.
    #[test]
    fn kodiert_wie_beschrieben() {
        let mut hoehen = Heights::default();
        hoehen.0[3 * EDGE + 5] = 300;
        hoehen.0[127 * EDGE + 127] = -64;
        let daten = hoehen.encode().unwrap();
        assert_eq!(Heights::decode(&daten).unwrap(), hoehen);

        let mut roh = Vec::new();
        ZlibDecoder::new(&daten[..]).read_to_end(&mut roh).unwrap();
        assert_eq!(roh.len(), 128 * 128 * 2);
        let bei = |i: usize| i16::from_le_bytes([roh[2 * i], roh[2 * i + 1]]);
        assert_eq!(bei(3 * EDGE + 5), 300);
        assert_eq!(hoehen.get(5, 3), 300);
        assert_eq!(bei(127 * EDGE + 127), -64);
        assert_eq!(bei(0), EMPTY);
        assert!(Heights::decode(&daten[..daten.len() / 2]).is_err());
    }

    /// Nicht gelesene Chunkplätze behalten die alten Werte, gelesene die
    /// neuen, auch wo die neuen leer sind.
    #[test]
    fn behaelt_nur_was_nicht_gelesen_ist() {
        let mut alt = Heights::default();
        alt.0.fill(7);
        let mut neu = Heights::default();
        neu.0[0] = 1;
        let mut gelesen = vec![true; (REGION * REGION) as usize];
        gelesen[1] = false;
        gelesen[REGION as usize * 31 + 31] = false;
        neu.keep_unread(&alt, &gelesen);
        assert_eq!(neu.get(0, 0), 1);
        assert_eq!(neu.get(3, 3), EMPTY);
        assert_eq!(neu.get(4, 0), 7);
        assert_eq!(neu.get(7, 3), 7);
        assert_eq!(neu.get(8, 0), EMPTY);
        assert_eq!(neu.get(127, 127), 7);
        assert_eq!(neu.get(124, 123), EMPTY);
    }
}
