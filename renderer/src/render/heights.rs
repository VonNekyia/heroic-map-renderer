//! Höhen je Region für die Koordinatenanzeige im Frontend: je Blockspalte
//! das y des obersten Blocks, den der Renderer zeichnet, ohne Blöcke, die
//! nur Flüssigkeit sind. So zielt auch das Spiel: `LocalPlayer.pick` ruft
//! `Entity.pick(…, false)` auf, und das sucht mit `ClipContext.Fluid.NONE`
//! durch Wasser hindurch.
//! Siehe docs/benutzung/map-json.md, „Höhen“.

use std::io::{Read, Write};

use anyhow::{Result, ensure};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use rayon::prelude::*;

use super::sprites::SpriteSet;
use super::tiles::{Content, Reach};
use crate::world::{BlockState, Chunk, REGION, World};

/// Blöcke je Kante einer Region.
pub const EDGE: usize = REGION as usize * 16;

/// Eine Spalte ohne gezeichneten Block oder ohne Chunk.
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

/// Ob ein Block in die Höhe zählt: er hat ein Sprite und ist nicht nur
/// Flüssigkeit. Luft, Licht, Blöcke ohne Geometrie wie Truhen und solche,
/// deren Flächen alle von der Kamera weg zeigen, wie Feuer, haben keines;
/// eine geflutete Truhe ist nur ihr Wasser, ein gefluteter Zaun zählt mit
/// seinem Pfosten.
pub fn counts(sprites: &SpriteSet, state: &BlockState) -> bool {
    sprites.family_of(state).is_some_and(|f| !f.pure_fluid)
}

/// Die Höhen einer Region, zeilenweise nach z: die Spalte (x, z) steht an
/// (z − 512·rz)·512 + (x − 512·rx).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heights(Vec<i16>);

impl Default for Heights {
    fn default() -> Self {
        Heights(vec![EMPTY; EDGE * EDGE])
    }
}

impl Heights {
    /// Die Höhe einer Spalte in Koordinaten der Region, 0 bis 511 je Achse.
    pub fn get(&self, x: usize, z: usize) -> i16 {
        self.0[z * EDGE + x]
    }

    /// Trägt die 256 Spalten eines Chunks ein: das y des obersten Blocks in
    /// `y_range`, der [`counts`], sonst [`EMPTY`]. Gesucht wird von der
    /// obersten Section abwärts; eine Section, deren Palette keinen solchen
    /// Block hat, wird nicht Block für Block angesehen.
    pub fn record(&mut self, chunk: &Chunk, y_range: (i32, i32), sprites: &SpriteSet) {
        let mut spalten = [EMPTY; 256];
        let mut offen = spalten.len();
        for section in chunk.sections().iter().rev() {
            let unten = section.y as i32 * 16;
            if offen == 0 || unten + 15 < y_range.0 {
                break;
            }
            if unten > y_range.1 || section.is_empty() {
                continue;
            }
            let zaehlt: Vec<bool> = section
                .blocks()
                .palette()
                .iter()
                .map(|state| counts(sprites, state))
                .collect();
            if !zaehlt.contains(&true) {
                continue;
            }
            let hoechste = (y_range.1 - unten).min(15);
            let tiefste = (y_range.0 - unten).max(0);
            for (i, spalte) in spalten.iter_mut().enumerate() {
                if *spalte != EMPTY {
                    continue;
                }
                let (x, z) = ((i % 16) as i32, (i / 16) as i32);
                // Ein Index hinter der Palette zeichnet nichts, wie beim
                // Rendern.
                if let Some(y) = (tiefste..=hoechste)
                    .rev()
                    .find(|&y| zaehlt.get(section.slot(x, y, z)) == Some(&true))
                {
                    *spalte = (unten + y) as i16;
                    offen -= 1;
                }
            }
        }
        let (x0, z0) = (
            chunk.x.rem_euclid(REGION) as usize * 16,
            chunk.z.rem_euclid(REGION) as usize * 16,
        );
        for (z, zeile) in spalten.as_chunks::<16>().0.iter().enumerate() {
            let start = (z0 + z) * EDGE + x0;
            self.0[start..start + 16].copy_from_slice(zeile);
        }
    }

    /// Übernimmt aus `old` die Chunks, die dieser Lauf nicht gelesen hat:
    /// `read` trägt je Chunkplatz der Region ein Flag, zeilenweise nach z.
    pub fn keep_unread(&mut self, old: &Heights, read: &[bool]) {
        let region = REGION as usize;
        for (platz, _) in read.iter().enumerate().filter(|(_, gelesen)| !**gelesen) {
            let (x0, z0) = ((platz % region) * 16, (platz / region) * 16);
            for z in z0..z0 + 16 {
                let zeile = z * EDGE + x0..z * EDGE + x0 + 16;
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

/// Liest die Chunks, die `reach` nennt, und gibt `write` je Region ihre
/// Höhen samt Flag je Chunkplatz, ob er gelesen ist, zeilenweise nach z.
/// Gelesen sind die Chunks, deren Blöcke im Ausschnitt landen können, und
/// die ohne Block; einer, den es nicht gibt, ist gelesen und leer. Zurück
/// kommen die Zahl der Regionen und die Bytes, die `write` meldet.
///
/// Welcher Block zählt, weiss erst die Sprite-Tabelle, und die entsteht
/// aus dem Vorlauf. Deshalb liest dieser Durchgang die Chunks noch einmal.
// ponytail: ein Durchgang mehr durch die Welt, so lang wie der Vorlauf,
// gut 1 % eines Exports. Sparen liesse er sich auf zwei Wegen:
// - Der Vorlauf sammelt je Spalte Kandidaten, bevor die Sprite-Tabelle
//   steht. Dann stünde neben `counts` eine zweite Regel dafür, welche
//   Blöcke sicher nicht zählen, und Spalten, deren Kandidat doch nicht
//   zählt, bräuchten wieder einen Durchgang.
// - Die Basis nimmt die Höhen aus den Chunks in ihrem Cache. Dann kämen
//   sie erst mit den Kacheln statt vor der ersten, jede Region bräuchte
//   eine Buchführung über die Threads, und `--resume` und `--heights`
//   bräuchten diesen Durchgang trotzdem: sie rendern Chunks nicht, deren
//   Höhen sie schreiben.
// Siehe docs/messungen/2026-09-28-hoehen.md, „Schluss“.
pub fn read_heights(
    world: &World,
    reach: Reach,
    y_range: (i32, i32),
    sprites: &SpriteSet,
    write: &(impl Fn(i32, i32, Heights, &[bool]) -> Result<usize> + Sync),
) -> Result<(usize, usize)> {
    let regions: Vec<(i32, i32)> = world
        .regions()?
        .into_iter()
        .filter(|&(rx, rz)| reach.region(rx, rz))
        .collect();
    let geschrieben = regions
        .par_iter()
        .map(|&(rx, rz)| -> Result<Option<usize>> {
            let Some(mut region) = world.region(rx, rz)? else {
                return Ok(None);
            };
            let mut hoehen = Heights::default();
            let mut gelesen = vec![false; (REGION * REGION) as usize];
            for (platz, gelesen) in gelesen.iter_mut().enumerate() {
                let platz = platz as i32;
                let (cx, cz) = (rx * REGION + platz % REGION, rz * REGION + platz / REGION);
                if !reach.chunk(cx, cz) {
                    continue;
                }
                let chunk = region.chunk(cx, cz)?;
                // Landet nichts davon im Ausschnitt, kennt die Sprite-Tabelle
                // seine Blöcke nicht, und seine Kacheln bleiben, wie sie sind.
                // Dann bleiben auch seine Höhen.
                if let Some(chunk) = &chunk
                    && matches!(reach.content(chunk), Content::Outside)
                {
                    continue;
                }
                *gelesen = true;
                if let Some(chunk) = chunk {
                    hoehen.record(&chunk, y_range, sprites);
                }
            }
            if !gelesen.contains(&true) {
                return Ok(None);
            }
            write(rx, rz, hoehen, &gelesen).map(Some)
        })
        .collect::<Result<Vec<_>>>()?;
    let geschrieben: Vec<usize> = geschrieben.into_iter().flatten().collect();
    Ok((geschrieben.len(), geschrieben.iter().sum()))
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
        hoehen.0[511 * EDGE + 511] = -64;
        let daten = hoehen.encode().unwrap();
        assert_eq!(Heights::decode(&daten).unwrap(), hoehen);

        let mut roh = Vec::new();
        ZlibDecoder::new(&daten[..]).read_to_end(&mut roh).unwrap();
        let bei = |i: usize| i16::from_le_bytes([roh[2 * i], roh[2 * i + 1]]);
        assert_eq!(bei(3 * EDGE + 5), 300);
        assert_eq!(hoehen.get(5, 3), 300);
        assert_eq!(bei(511 * EDGE + 511), -64);
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
        assert_eq!(neu.get(15, 15), EMPTY);
        assert_eq!(neu.get(16, 0), 7);
        assert_eq!(neu.get(31, 15), 7);
        assert_eq!(neu.get(32, 0), EMPTY);
        assert_eq!(neu.get(511, 511), 7);
        assert_eq!(neu.get(496, 495), EMPTY);
    }
}
