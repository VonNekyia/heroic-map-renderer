//! Je Kachel ein Hash ihrer Pixel. Zeichnet ein Lauf eine Kachel mit
//! denselben Pixeln wieder, kodiert und schreibt er sie nicht. Ein Eintrag
//! gilt nur, solange die Datei so dasteht, wie ein Lauf desselben Renderers
//! sie hinterliess: mit derselben Grösse und derselben Zeit der letzten
//! Änderung. Schrieb sie jemand anders, kodiert der nächste Lauf sie neu.
//! Siehe docs/benutzung/updates.md, „Gleiche Pixel“.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use heroic_map_renderer::render::{Packen, TileId};
use image::RgbaImage;
use rayon::prelude::*;

/// Der Ordner im Baum, je Stufe ein Unterordner mit einer Datei je Block.
pub(super) const ORDNER: &str = "pixel";
const MAGIE: &[u8; 8] = b"HMRPIXEL";
const FASSUNG: u32 = 1;
/// Kantenlänge eines Blocks in Kacheln.
const KANTE: i32 = 32;
/// Kopf: Magie, Fassung, Fingerabdruck des Renderers, Zahl der Einträge.
const KOPF: usize = 8 + 4 + 8 + 4;
/// Bytes je Eintrag: Platz im Block, Hash, Grösse, Zeit in ns.
const EINTRAG: usize = 2 + 16 + 4 + 8;

/// Die Pixel einer Kachel und ihre Datei, wie ein Lauf sie hinterliess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Eintrag {
    hash: [u8; 16],
    bytes: u32,
    zeit: u64,
}

/// Die Einträge eines Blocks, nach Platz `y · KANTE + x` im Block.
#[derive(Default)]
struct Block {
    eintraege: HashMap<u16, Eintrag>,
    geaendert: bool,
}

/// Block eines Baums: Stufe und Block-x, Block-y.
type BlockId = (u32, i32, i32);

/// Die Hashes eines Baums. Einen Block liest es erst, wenn ein Lauf eine
/// Kachel darin schreibt, und schreibt am Ende nur die geänderten.
pub(super) struct Pixel {
    ordner: PathBuf,
    renderer: u64,
    bloecke: Mutex<HashMap<BlockId, Block>>,
    gespart: AtomicUsize,
}

/// SHA-256 über die Packung und die RGBA-Bytes, die ersten 16 Bytes: Kompakt
/// gepackt hat dasselbe Bild andere Bytes.
pub(super) fn hash(bild: &RgbaImage, packen: Packen) -> [u8; 16] {
    let mut sha = ring::digest::Context::new(&ring::digest::SHA256);
    sha.update(&[packen as u8]);
    sha.update(bild.as_raw());
    let voll = sha.finish();
    voll.as_ref()[..16]
        .try_into()
        .expect("SHA-256 hat 32 Bytes")
}

/// Grösse und Zeit der letzten Änderung in ns, `None` ohne Datei.
fn datei(pfad: &Path) -> Option<(u32, u64)> {
    let meta = std::fs::metadata(pfad).ok()?;
    let zeit = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((
        u32::try_from(meta.len()).ok()?,
        u64::try_from(zeit.as_nanos()).ok()?,
    ))
}

/// In welchem Block eine Kachel liegt, `(bx, by)` je Stufe.
pub(super) fn block(tile: TileId) -> (i32, i32) {
    (tile.x.div_euclid(KANTE), tile.y.div_euclid(KANTE))
}

fn block_von(z: u32, tile: TileId) -> (BlockId, u16) {
    let platz = tile.y.rem_euclid(KANTE) * KANTE + tile.x.rem_euclid(KANTE);
    (
        (z, tile.x.div_euclid(KANTE), tile.y.div_euclid(KANTE)),
        platz as u16,
    )
}

impl Pixel {
    /// Die Hashes des Baums in `baum`, die ein Renderer mit diesem
    /// Fingerabdruck gelten lässt.
    pub(super) fn neu(baum: &Path, renderer: u64) -> Pixel {
        Pixel {
            ordner: baum.join(ORDNER),
            renderer,
            bloecke: Mutex::default(),
            gespart: AtomicUsize::new(0),
        }
    }

    fn pfad(ordner: &Path, (z, bx, by): BlockId) -> PathBuf {
        ordner.join(z.to_string()).join(format!("{bx}.{by}.bin"))
    }

    /// Wendet `f` auf den Block an und liest ihn vorher, falls nötig.
    fn mit_block<T>(&self, id: BlockId, f: impl FnOnce(&mut Block) -> T) -> T {
        let mut bloecke = self.bloecke.lock().unwrap_or_else(PoisonError::into_inner);
        let block = bloecke.entry(id).or_insert_with(|| Block {
            eintraege: std::fs::read(Self::pfad(&self.ordner, id))
                .ok()
                .and_then(|daten| aus_bytes(&daten, self.renderer))
                .unwrap_or_default(),
            geaendert: false,
        });
        f(block)
    }

    /// Die Grösse der Kachel in `pfad`, wenn sie schon diese Pixel zeigt:
    /// Ihr Eintrag hat diesen Hash, und die Datei hat noch seine Grösse und
    /// Zeit.
    pub(super) fn gleich(
        &self,
        z: u32,
        tile: TileId,
        hash: &[u8; 16],
        pfad: &Path,
    ) -> Option<usize> {
        let (id, platz) = block_von(z, tile);
        let eintrag = self.mit_block(id, |block| block.eintraege.get(&platz).copied())?;
        let gilt = eintrag.hash == *hash && datei(pfad) == Some((eintrag.bytes, eintrag.zeit));
        gilt.then(|| {
            self.gespart.fetch_add(1, Ordering::Relaxed);
            eintrag.bytes as usize
        })
    }

    /// Merkt sich die Pixel einer Kachel, die eben geschrieben wurde oder
    /// mit denselben Bytes liegen blieb.
    pub(super) fn merke(&self, z: u32, tile: TileId, hash: &[u8; 16], pfad: &Path) -> Result<()> {
        let (bytes, zeit) = datei(pfad)
            .with_context(|| format!("{} nach dem Schreiben nicht da", pfad.display()))?;
        let neu = Eintrag {
            hash: *hash,
            bytes,
            zeit,
        };
        let (id, platz) = block_von(z, tile);
        self.mit_block(id, |block| {
            if block.eintraege.insert(platz, neu) != Some(neu) {
                block.geaendert = true;
            }
        });
        Ok(())
    }

    /// Schreibt diesen Block, falls er sich geändert hat, und gibt ihn aus
    /// dem Speicher frei: für einen Lauf, der danach keine Kachel darin mehr
    /// ablegt. Ein Abbruch verliert so nur die Blöcke in Arbeit.
    pub(super) fn schliesse_block(&self, z: u32, (bx, by): (i32, i32)) -> Result<()> {
        let id = (z, bx, by);
        let mut bloecke = self.bloecke.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(block) = bloecke.remove(&id)
            && block.geaendert
        {
            let daten = als_bytes(self.renderer, &block.eintraege);
            super::lege_ab(&Self::pfad(&self.ordner, id), &daten, None)?;
        }
        Ok(())
    }

    /// Schreibt jeden geänderten Block. Liefert, wie viele Kacheln der
    /// Lauf nicht kodiert hat, und wie viele Blöcke er schrieb.
    pub(super) fn schreibe(self) -> Result<(usize, usize)> {
        let bloecke = self
            .bloecke
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        let geaendert: Vec<(BlockId, Block)> = bloecke
            .into_iter()
            .filter(|(_, block)| block.geaendert)
            .collect();
        geaendert.par_iter().try_for_each(|(id, block)| {
            let daten = als_bytes(self.renderer, &block.eintraege);
            super::lege_ab(&Self::pfad(&self.ordner, *id), &daten, None).map(drop)
        })?;
        Ok((self.gespart.into_inner(), geaendert.len()))
    }
}

/// Die Bytes einer Blockdatei, Little Endian, nach Platz geordnet.
fn als_bytes(renderer: u64, eintraege: &HashMap<u16, Eintrag>) -> Vec<u8> {
    let mut plaetze: Vec<(&u16, &Eintrag)> = eintraege.iter().collect();
    plaetze.sort_unstable_by_key(|(platz, _)| **platz);
    let mut out = Vec::with_capacity(KOPF + plaetze.len() * EINTRAG);
    out.extend_from_slice(MAGIE);
    out.extend_from_slice(&FASSUNG.to_le_bytes());
    out.extend_from_slice(&renderer.to_le_bytes());
    out.extend_from_slice(&(plaetze.len() as u32).to_le_bytes());
    for (platz, e) in plaetze {
        out.extend_from_slice(&platz.to_le_bytes());
        out.extend_from_slice(&e.hash);
        out.extend_from_slice(&e.bytes.to_le_bytes());
        out.extend_from_slice(&e.zeit.to_le_bytes());
    }
    out
}

/// Die Einträge einer Blockdatei; `None`, wenn sie kaputt ist, eine andere
/// Fassung hat oder von einem anderen Renderer stammt. Dann gilt der Block
/// als leer, und der Lauf kodiert seine Kacheln wie ohne Hash.
fn aus_bytes(daten: &[u8], renderer: u64) -> Option<HashMap<u16, Eintrag>> {
    let zahl = |von: usize, n: usize| daten.get(von..von + n);
    if zahl(0, 8)? != MAGIE
        || u32::from_le_bytes(zahl(8, 4)?.try_into().ok()?) != FASSUNG
        || u64::from_le_bytes(zahl(12, 8)?.try_into().ok()?) != renderer
    {
        return None;
    }
    let anzahl = u32::from_le_bytes(zahl(20, 4)?.try_into().ok()?) as usize;
    if daten.len() != KOPF + anzahl * EINTRAG {
        return None;
    }
    let mut eintraege = HashMap::with_capacity(anzahl);
    for e in daten[KOPF..].as_chunks::<EINTRAG>().0 {
        let platz = u16::from_le_bytes([e[0], e[1]]);
        if i32::from(platz) >= KANTE * KANTE {
            return None;
        }
        let eintrag = Eintrag {
            hash: e[2..18].try_into().ok()?,
            bytes: u32::from_le_bytes(e[18..22].try_into().ok()?),
            zeit: u64::from_le_bytes(e[22..30].try_into().ok()?),
        };
        eintraege.insert(platz, eintrag);
    }
    Some(eintraege)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bild(farbe: u8) -> RgbaImage {
        RgbaImage::from_pixel(4, 4, image::Rgba([farbe, 0, 0, 255]))
    }

    /// Ein Eintrag gilt für dieselben Pixel und dieselbe Datei, auch in
    /// einem späteren Lauf desselben Renderers. Andere Pixel, eine Datei,
    /// die jemand neu schrieb, eine fehlende Datei und ein anderer Renderer
    /// lassen den Lauf neu kodieren.
    #[test]
    fn eintrag_gilt_nur_fuer_dieselbe_datei() {
        let dir = tempfile::tempdir().unwrap();
        let tile = TileId { x: -33, y: 5 };
        let kachel = dir.path().join("k.webp");
        std::fs::write(&kachel, b"alt").unwrap();
        let (rot, gruen) = (
            hash(&bild(1), Packen::Schnell),
            hash(&bild(2), Packen::Schnell),
        );
        assert_ne!(rot, gruen);
        assert_ne!(rot, hash(&bild(1), Packen::Kompakt));

        let pixel = Pixel::neu(dir.path(), 7);
        assert_eq!(pixel.gleich(3, tile, &rot, &kachel), None);
        pixel.merke(3, tile, &rot, &kachel).unwrap();
        assert_eq!(pixel.gleich(3, tile, &rot, &kachel), Some(3));
        assert_eq!(pixel.gleich(3, tile, &gruen, &kachel), None);
        assert_eq!(pixel.gleich(4, tile, &rot, &kachel), None);
        assert_eq!(pixel.schreibe().unwrap(), (1, 1));
        assert!(dir.path().join("pixel/3/-2.0.bin").is_file());

        let spaeter = Pixel::neu(dir.path(), 7);
        assert_eq!(spaeter.gleich(3, tile, &rot, &kachel), Some(3));
        assert_eq!(spaeter.schreibe().unwrap(), (1, 0), "nichts geändert");
        let anderer = Pixel::neu(dir.path(), 8);
        assert_eq!(anderer.gleich(3, tile, &rot, &kachel), None);

        // Dieselbe Grösse, neu geschrieben: die Zeit ist eine andere.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&kachel, b"neu").unwrap();
        assert_eq!(
            Pixel::neu(dir.path(), 7).gleich(3, tile, &rot, &kachel),
            None
        );
        std::fs::remove_file(&kachel).unwrap();
        assert_eq!(
            Pixel::neu(dir.path(), 7).gleich(3, tile, &rot, &kachel),
            None
        );
    }

    /// Eine kaputte oder abgeschnittene Blockdatei gilt als leer und hält
    /// keinen Lauf auf; der nächste schreibt sie neu.
    #[test]
    fn kaputter_block_gilt_als_leer() {
        let mut eintraege = HashMap::new();
        let e = Eintrag {
            hash: [9; 16],
            bytes: 12,
            zeit: 34,
        };
        eintraege.insert(1023, e);
        let daten = als_bytes(5, &eintraege);
        assert_eq!(aus_bytes(&daten, 5), Some(eintraege.clone()));
        assert_eq!(aus_bytes(&daten[..daten.len() - 1], 5), None);
        assert_eq!(aus_bytes(&daten, 6), None);
        let mut falsch = daten.clone();
        falsch[KOPF..KOPF + 2].copy_from_slice(&1024u16.to_le_bytes());
        assert_eq!(aus_bytes(&falsch, 5), None);
        assert_eq!(aus_bytes(b"HMRPIXEL", 5), None);
        assert_eq!(aus_bytes(b"", 5), None);
    }
}
