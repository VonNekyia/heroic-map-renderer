//! Höhen je Region für die Koordinatenanzeige im Frontend: je 4×4
//! Blockspalten der Median der obersten Blöcke, die nicht Luft sind. Sie
//! kommen aus der Heightmap `WORLD_SURFACE`, die das Spiel in jedem Chunk
//! speichert; der Vorlauf liest sie mit, einen eigenen Durchgang gibt es
//! nicht. Ebenso der Boden ohne Laub aus `MOTION_BLOCKING_NO_LEAVES`, je
//! Block, für Formen auf dem Gelände.
//! Siehe docs/benutzung/map-json.md, „Höhen“.

use std::io::{Read, Write};

use anyhow::{Result, ensure};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::world::{Chunk, REGION};

/// Blöcke je Kante einer Zelle der Höhen.
pub const CELL: usize = 4;

/// Blöcke je Kante einer Zelle des Bodens ohne Laub: je Block.
/// Siehe docs/entscheidungen/0103-boden-ohne-laub.md.
pub const GROUND_CELL: usize = 1;

/// Eine Zelle ohne Block oder ohne Chunk.
pub const EMPTY: i16 = i16::MIN;

/// Pfadmuster der Dateien relativ zu dem Ordner, in dem `heights/` liegt:
/// in einem Baum der alten Ablage dieser selbst.
pub const PATTERN: &str = "heights/{x}.{z}.bin";

/// Das Muster in `map.json` eines Baums unter der Wurzel von `--tiles`:
/// Alle Bäume einer Welt teilen die Höhen dort.
/// Siehe docs/benutzung/map-json.md, „Höhen“.
pub const PATTERN_WURZEL: &str = "../heights/{x}.{z}.bin";

/// Pfadmuster des Bodens ohne Laub, wie [`PATTERN`].
pub const PATTERN_BODEN: &str = "ground/{x}.{z}.bin";

/// Das Muster des Bodens in `map.json` eines Baums unter der Wurzel, wie
/// [`PATTERN_WURZEL`].
pub const PATTERN_BODEN_WURZEL: &str = "../ground/{x}.{z}.bin";

/// Wo die Höhen der Region (rx, rz) liegen, relativ zu dem Ordner mit
/// `heights/`.
pub fn path_of(rx: i32, rz: i32) -> String {
    PATTERN
        .replace("{x}", &rx.to_string())
        .replace("{z}", &rz.to_string())
}

/// Wo der Boden der Region (rx, rz) liegt, neben [`path_of`].
pub fn ground_path_of(rx: i32, rz: i32) -> String {
    PATTERN_BODEN
        .replace("{x}", &rx.to_string())
        .replace("{z}", &rz.to_string())
}

/// Die Region zu einem Dateinamen aus [`PATTERN`], etwa `-1.3.bin`.
pub fn region_of(name: &str) -> Option<(i32, i32)> {
    let (x, z) = name.strip_suffix(".bin")?.split_once('.')?;
    Some((x.parse().ok()?, z.parse().ok()?))
}

/// Die Höhen einer Region in Zellen aus `cell` × `cell` Spalten,
/// zeilenweise nach z: die Zelle mit den Spalten (x, z) steht an
/// ⌊(z − 512·rz)/cell⌋·edge + ⌊(x − 512·rx)/cell⌋, edge = 512/cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heights {
    cell: usize,
    werte: Vec<i16>,
}

impl Heights {
    /// Leer, mit `cell` Blöcken je Kante einer Zelle, einem Teiler von 16.
    pub fn new(cell: usize) -> Self {
        assert!(
            cell > 0 && 16 % cell == 0,
            "Zelle {cell} teilt keinen Chunk"
        );
        let edge = REGION as usize * 16 / cell;
        Heights {
            cell,
            werte: vec![EMPTY; edge * edge],
        }
    }

    /// Zellen je Kante der Region.
    pub fn edge(&self) -> usize {
        REGION as usize * 16 / self.cell
    }

    /// Die Höhe einer Zelle in Zellen der Region, 0 bis `edge` − 1 je Achse.
    pub fn get(&self, x: usize, z: usize) -> i16 {
        self.werte[z * self.edge() + x]
    }

    /// Trägt die Zellen eines Chunks ein: je Zelle der obere Median der
    /// Spalten mit Block, nach dem Sortieren also der Wert an der Stelle
    /// Anzahl/2; ohne Spalte mit Block [`EMPTY`]. Bei `cell` 1 die Spalte.
    pub fn record(&mut self, chunk: &Chunk) {
        self.trage_ein(chunk, chunk.surface());
    }

    /// Wie [`Heights::record`], mit dem Boden ohne Laub, [`Chunk::ground`].
    pub fn record_ground(&mut self, chunk: &Chunk) {
        self.trage_ein(chunk, chunk.ground());
    }

    fn trage_ein(&mut self, chunk: &Chunk, oben: [Option<i32>; 256]) {
        let (cell, edge) = (self.cell, self.edge());
        let je_chunk = 16 / cell;
        let (x0, z0) = (
            chunk.x.rem_euclid(REGION) as usize * je_chunk,
            chunk.z.rem_euclid(REGION) as usize * je_chunk,
        );
        let mut werte = Vec::with_capacity(cell * cell);
        for zelle in 0..je_chunk * je_chunk {
            let (cx, cz) = (zelle % je_chunk, zelle / je_chunk);
            werte.clear();
            werte.extend(
                (0..cell * cell)
                    .filter_map(|i| oben[(cz * cell + i / cell) * 16 + cx * cell + i % cell]),
            );
            werte.sort_unstable();
            self.werte[(z0 + cz) * edge + x0 + cx] =
                werte.get(werte.len() / 2).map_or(EMPTY, |&y| y as i16);
        }
    }

    /// Setzt jede Zelle mit Wert auf den oberen Median der 3 × 3 Zellen um
    /// sie, so fallen Stämme auf flachem Boden weg. Leere Zellen und solche
    /// ausserhalb der Region zählen nicht mit; eine leere bleibt leer.
    /// Siehe docs/entscheidungen/0103-boden-ohne-laub.md, „Entscheidung“.
    pub fn median_3x3(&mut self) {
        let edge = self.edge();
        // Die Zeilen über und in der, die gerade entsteht, noch ohne Median;
        // die darunter steht so noch in `werte`.
        let mut ueber = vec![EMPTY; edge];
        let mut mitte = vec![EMPTY; edge];
        let mut fenster = Vec::with_capacity(9);
        for z in 0..edge {
            mitte.copy_from_slice(&self.werte[z * edge..(z + 1) * edge]);
            for x in (0..edge).filter(|&x| mitte[x] != EMPTY) {
                fenster.clear();
                for nx in x.saturating_sub(1)..(x + 2).min(edge) {
                    if z > 0 {
                        fenster.push(ueber[nx]);
                    }
                    fenster.push(mitte[nx]);
                    if z + 1 < edge {
                        fenster.push(self.werte[(z + 1) * edge + nx]);
                    }
                }
                fenster.retain(|&y| y != EMPTY);
                fenster.sort_unstable();
                self.werte[z * edge + x] = fenster[fenster.len() / 2];
            }
            std::mem::swap(&mut ueber, &mut mitte);
        }
    }

    /// Übernimmt aus `old` die Chunks, die dieser Lauf nicht gelesen hat:
    /// `read` trägt je Chunkplatz der Region ein Flag, zeilenweise nach z.
    /// `old` hat dieselbe Zelle.
    pub fn keep_unread(&mut self, old: &Heights, read: &[bool]) {
        let (region, edge, je_chunk) = (REGION as usize, self.edge(), 16 / self.cell);
        for (platz, _) in read.iter().enumerate().filter(|(_, gelesen)| !**gelesen) {
            let (x0, z0) = ((platz % region) * je_chunk, (platz / region) * je_chunk);
            for z in z0..z0 + je_chunk {
                let zeile = z * edge + x0..z * edge + x0 + je_chunk;
                self.werte[zeile.clone()].copy_from_slice(&old.werte[zeile]);
            }
        }
    }

    /// Die Datei: ein zlib-Strom (RFC 1950), darin die Werte als i16
    /// little-endian.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut packer = ZlibEncoder::new(Vec::new(), Compression::default());
        // In Stücken: Ein Puffer für alles hielte je Block 512 KiB je Thread.
        let mut stueck = [0u8; 8192];
        for teil in self.werte.chunks(stueck.len() / 2) {
            for (ziel, h) in stueck.as_chunks_mut::<2>().0.iter_mut().zip(teil) {
                *ziel = h.to_le_bytes();
            }
            packer.write_all(&stueck[..teil.len() * 2])?;
        }
        Ok(packer.finish()?)
    }

    /// Liest eine Datei aus [`Heights::encode`] mit Zellen aus `cell` ×
    /// `cell` Spalten.
    pub fn decode(daten: &[u8], cell: usize) -> Result<Heights> {
        let mut leer = Heights::new(cell);
        let edge = leer.edge();
        let mut roh = Vec::new();
        ZlibDecoder::new(daten).read_to_end(&mut roh)?;
        ensure!(
            roh.len() == edge * edge * 2,
            "{} Bytes statt {} für {edge} × {edge} Höhen",
            roh.len(),
            edge * edge * 2
        );
        let (paare, _) = roh.as_chunks::<2>();
        leer.werte = paare.iter().map(|&b| i16::from_le_bytes(b)).collect();
        Ok(leer)
    }
}

/// Die Höhen einer Region, wie der Vorlauf sie liest, schon gepackt wie in
/// der Datei: So hält er je Region nur die gepackten Bytes, bis sie
/// geschrieben sind. `read` trägt je Chunkplatz, zeilenweise nach z, ob er
/// gelesen ist. Gelesen sind die Chunks, die der Lauf liest, auch die ohne
/// Block; einer, den es dort nicht gibt, ist gelesen und leer.
/// Siehe docs/entscheidungen/0103-boden-ohne-laub.md, „Kosten nach Regel 26“.
#[derive(Debug)]
pub struct RegionHeights {
    pub x: i32,
    pub z: i32,
    /// [`Heights::encode`] der Höhen, Zelle [`CELL`].
    pub heights: Vec<u8>,
    /// Der Boden ohne Laub für dieselben Chunks, Zelle [`GROUND_CELL`].
    pub ground: Vec<u8>,
    pub read: Vec<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datei_und_region() {
        assert_eq!(path_of(-1, 3), "heights/-1.3.bin");
        assert_eq!(ground_path_of(-1, 3), "ground/-1.3.bin");
        assert_eq!(region_of("-1.3.bin"), Some((-1, 3)));
        assert_eq!(region_of("map.json"), None);
        assert_eq!(region_of("1.2.bin.7.tmp"), None);
    }

    /// Kodieren und Lesen geben dieselben Höhen, little-endian und
    /// zeilenweise nach z, wie das Frontend sie liest.
    #[test]
    fn kodiert_wie_beschrieben() {
        for (cell, edge) in [(CELL, 128), (GROUND_CELL, 512)] {
            let mut hoehen = Heights::new(cell);
            assert_eq!(hoehen.edge(), edge);
            hoehen.werte[3 * edge + 5] = 300;
            hoehen.werte[(edge - 1) * edge + edge - 1] = -64;
            let daten = hoehen.encode().unwrap();
            assert_eq!(Heights::decode(&daten, cell).unwrap(), hoehen);

            let mut roh = Vec::new();
            ZlibDecoder::new(&daten[..]).read_to_end(&mut roh).unwrap();
            assert_eq!(roh.len(), edge * edge * 2);
            let bei = |i: usize| i16::from_le_bytes([roh[2 * i], roh[2 * i + 1]]);
            assert_eq!(bei(3 * edge + 5), 300);
            assert_eq!(hoehen.get(5, 3), 300);
            assert_eq!(bei((edge - 1) * edge + edge - 1), -64);
            assert_eq!(bei(0), EMPTY);
            assert!(Heights::decode(&daten[..daten.len() / 2], cell).is_err());
        }
        let vier = Heights::new(CELL).encode().unwrap();
        assert!(Heights::decode(&vier, GROUND_CELL).is_err(), "andere Zelle");
    }

    /// Der Median 3 × 3: Ein Stamm von 1 × 1 und einer von 2 × 2 auf flachem
    /// Boden fallen weg, eine Klippe bleibt an ihrer Kante. Eine leere Zelle
    /// zählt nicht und bleibt leer; am Rand der Region zählt, was da ist.
    #[test]
    fn median_nimmt_staemme_und_laesst_klippen() {
        let mut boden = Heights::new(GROUND_CELL);
        let edge = boden.edge();
        for z in 0..32 {
            for x in 0..32 {
                boden.werte[z * edge + x] = if x >= 20 { 80 } else { 60 };
            }
        }
        let mut setze = |x: usize, z: usize, y: i16| boden.werte[z * edge + x] = y;
        setze(5, 5, 69);
        for (x, z) in [(10, 10), (11, 10), (10, 11), (11, 11)] {
            setze(x, z, 66);
        }
        setze(0, 25, 69);
        setze(3, 15, EMPTY);
        boden.median_3x3();

        for z in 0..32 {
            for x in 0..32 {
                let erwartet = match (x, z) {
                    (3, 15) => EMPTY,
                    _ if x >= 20 => 80,
                    _ => 60,
                };
                assert_eq!(boden.get(x, z), erwartet, "({x}, {z})");
            }
        }
        assert_eq!(boden.get(32, 0), EMPTY, "ausserhalb der Fläche bleibt leer");
    }

    /// Nicht gelesene Chunkplätze behalten die alten Werte, gelesene die
    /// neuen, auch wo die neuen leer sind; je Zelle 4 Spalten wie je Block.
    #[test]
    fn behaelt_nur_was_nicht_gelesen_ist() {
        for (cell, je_chunk) in [(CELL, 4), (GROUND_CELL, 16)] {
            let mut alt = Heights::new(cell);
            alt.werte.fill(7);
            let mut neu = Heights::new(cell);
            neu.werte[0] = 1;
            let mut gelesen = vec![true; (REGION * REGION) as usize];
            gelesen[1] = false;
            gelesen[REGION as usize * 31 + 31] = false;
            neu.keep_unread(&alt, &gelesen);
            let ende = neu.edge() - 1;
            assert_eq!(neu.get(0, 0), 1);
            assert_eq!(neu.get(je_chunk - 1, je_chunk - 1), EMPTY);
            assert_eq!(neu.get(je_chunk, 0), 7);
            assert_eq!(neu.get(2 * je_chunk - 1, je_chunk - 1), 7);
            assert_eq!(neu.get(2 * je_chunk, 0), EMPTY);
            assert_eq!(neu.get(ende, ende), 7);
            assert_eq!(neu.get(ende + 1 - je_chunk, ende - je_chunk), EMPTY);
        }
    }
}
