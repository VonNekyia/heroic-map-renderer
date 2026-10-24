//! Je Kachel ein Hash ihrer Pixel. Zeichnet ein Lauf eine Kachel mit
//! denselben Pixeln wieder, kodiert und schreibt er sie nicht. Ein Eintrag
//! gilt nur, solange die Datei so dasteht, wie ein Lauf mit demselben
//! Kodierstand sie hinterliess: mit derselben Grösse und derselben Zeit der
//! letzten Änderung. Schrieb sie jemand anders, kodiert der nächste Lauf sie
//! neu.
//! Siehe docs/benutzung/updates.md, „Gleiche Pixel“.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use heroic_map_renderer::render::stand::ALTE_BUILDS;
use heroic_map_renderer::render::{Packen, TileId};
use image::RgbaImage;
use rayon::prelude::*;

/// Der Ordner im Baum, je Stufe ein Unterordner mit einer Datei je Block.
pub(super) const ORDNER: &str = "pixel";
/// Wohin ein Lauf fertige Blöcke vor dem Ende schreibt. Nach der Pyramide
/// wandern sie nach [`ORDNER`]; was ein Abbruch hier liess, räumt der nächste
/// Lauf weg.
/// Siehe docs/benutzung/updates.md, „Gleiche Pixel“.
const NEU: &str = "pixel-neu";
const MAGIE: &[u8; 8] = b"HMRPIXEL";
const FASSUNG: u32 = 1;
/// Kantenlänge eines Blocks in Kacheln.
const KANTE: i32 = 32;
/// Wie der Renderer Kacheln kodiert: libwebp samt Patch und die Vorgaben in
/// `encode_webp` und `Packen`. Er steigt um eins mit jeder Änderung daran,
/// nach der dieselben Pixel andere Bytes ergeben. Was gezeichnet wird, zählt
/// nicht: Der Hash gilt den Pixeln.
/// Siehe docs/entscheidungen/0098-der-zeichenstand-statt-des-builds.md.
pub(super) const KODIERSTAND: u64 = 1;
/// Kopf: Magie, Fassung, Kodierstand, Zahl der Einträge.
const KOPF: usize = 8 + 4 + 8 + 4;
/// Bytes je Eintrag: Platz im Block, Hash, Grösse, Zeit in ns.
const EINTRAG: usize = 2 + 16 + 4 + 8;
/// Bytes je Platz im Speicher: Hash, Grösse, Zeit. Lauter Nullen heisst
/// leer, denn eine Kachel hat nie 0 Bytes.
const PLATZ: usize = EINTRAG - 2;

/// Die Pixel einer Kachel und ihre Datei, wie ein Lauf sie hinterliess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Eintrag {
    hash: [u8; 16],
    bytes: u32,
    zeit: u64,
}

impl Eintrag {
    fn packe(&self) -> [u8; PLATZ] {
        let mut out = [0; PLATZ];
        out[..16].copy_from_slice(&self.hash);
        out[16..20].copy_from_slice(&self.bytes.to_le_bytes());
        out[20..].copy_from_slice(&self.zeit.to_le_bytes());
        out
    }

    fn aus(platz: &[u8; PLATZ]) -> Option<Eintrag> {
        let bytes = u32::from_le_bytes(platz[16..20].try_into().expect("4 Bytes"));
        (bytes != 0).then(|| Eintrag {
            hash: platz[..16].try_into().expect("16 Bytes"),
            bytes,
            zeit: u64::from_le_bytes(platz[20..].try_into().expect("8 Bytes")),
        })
    }
}

/// Die Einträge eines Blocks als Feld, nach Platz `y · KANTE + x` im Block:
/// je Kachel [`PLATZ`] Bytes statt eines Eintrags in einer Tabelle.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Block {
    plaetze: Box<[[u8; PLATZ]]>,
    geaendert: bool,
}

impl Block {
    fn leer() -> Block {
        Block {
            plaetze: vec![[0; PLATZ]; (KANTE * KANTE) as usize].into_boxed_slice(),
            geaendert: false,
        }
    }

    fn eintrag(&self, platz: u16) -> Option<Eintrag> {
        Eintrag::aus(&self.plaetze[usize::from(platz)])
    }

    /// Setzt einen Eintrag; `true`, wenn er anders war.
    fn setze(&mut self, platz: u16, eintrag: Eintrag) -> bool {
        let neu = eintrag.packe();
        std::mem::replace(&mut self.plaetze[usize::from(platz)], neu) != neu
    }

    /// Die Einträge nach Platz.
    fn eintraege(&self) -> impl Iterator<Item = (u16, Eintrag)> + '_ {
        (0..self.plaetze.len() as u16).filter_map(|platz| Some((platz, self.eintrag(platz)?)))
    }
}

/// Block eines Baums: Stufe und Block-x, Block-y.
type BlockId = (u32, i32, i32);

/// Die Hashes eines Baums. Einen Block liest es erst, wenn ein Lauf eine
/// Kachel darin schreibt, und schreibt am Ende nur die geänderten.
pub(super) struct Pixel {
    ordner: PathBuf,
    neu: PathBuf,
    kodierstand: u64,
    bloecke: Mutex<HashMap<BlockId, Block>>,
    /// Je Block, wie viele Kacheln dieser Lauf darin noch ablegt, siehe
    /// [`Pixel::erwarte`].
    offen: Mutex<HashMap<BlockId, u32>>,
    gespart: AtomicUsize,
    frueh: AtomicUsize,
    /// Die Kacheln dieses Laufs mit einem Treffer, je Stufe, siehe
    /// [`Pixel::getroffen`].
    getroffen: Mutex<HashSet<(u32, TileId)>>,
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
    /// Die Hashes des Baums in `baum`, die ein Lauf mit diesem Kodierstand
    /// gelten lässt. Blöcke, die ein abgebrochener Lauf früh schrieb, nimmt
    /// es weg: Ihre Kacheln haben womöglich keine neuen Eltern.
    pub(super) fn neu(baum: &Path, kodierstand: u64) -> Result<Pixel> {
        let neu = baum.join(NEU);
        match std::fs::remove_dir_all(&neu) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("{} entfernen", neu.display()))?
            }
            _ => {}
        }
        Ok(Pixel {
            ordner: baum.join(ORDNER),
            neu,
            kodierstand,
            bloecke: Mutex::default(),
            offen: Mutex::default(),
            gespart: AtomicUsize::new(0),
            frueh: AtomicUsize::new(0),
            getroffen: Mutex::default(),
        })
    }

    fn pfad(ordner: &Path, (z, bx, by): BlockId) -> PathBuf {
        ordner.join(z.to_string()).join(format!("{bx}.{by}.bin"))
    }

    /// Wendet `f` auf den Block an und liest ihn vorher, falls nötig: zuerst
    /// einen, den dieser Lauf früh schrieb.
    fn mit_block<T>(&self, id: BlockId, f: impl FnOnce(&mut Block) -> T) -> T {
        let mut bloecke = self.bloecke.lock().unwrap_or_else(PoisonError::into_inner);
        let block = bloecke.entry(id).or_insert_with(|| {
            [&self.neu, &self.ordner]
                .into_iter()
                .find_map(|ordner| std::fs::read(Self::pfad(ordner, id)).ok())
                .and_then(|daten| aus_bytes(&daten, self.kodierstand))
                .unwrap_or_else(Block::leer)
        });
        f(block)
    }

    /// Diese Kacheln der Stufe `z` legt der Lauf gleich ab, jede genau einmal
    /// mit [`Pixel::fertig`]. Ein Block, dessen letzte Kachel fertig ist,
    /// geht dann früh nach [`NEU`] und aus dem Speicher.
    pub(super) fn erwarte(&self, z: u32, tiles: &[TileId]) {
        let mut offen = self.offen.lock().unwrap_or_else(PoisonError::into_inner);
        for tile in tiles {
            *offen.entry(block_von(z, *tile).0).or_default() += 1;
        }
    }

    /// Eine erwartete Kachel ist abgelegt, geschrieben, gleich geblieben
    /// oder leer.
    pub(super) fn fertig(&self, z: u32, tile: TileId) -> Result<()> {
        let id = block_von(z, tile).0;
        {
            let mut offen = self.offen.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(rest) = offen.get_mut(&id) else {
                return Ok(());
            };
            *rest -= 1;
            if *rest > 0 {
                return Ok(());
            }
            offen.remove(&id);
        }
        let block = self
            .bloecke
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
        if let Some(block) = block
            && block.geaendert
        {
            super::lege_ab(
                &Self::pfad(&self.neu, id),
                &als_bytes(self.kodierstand, &block),
                None,
            )?;
            self.frueh.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
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
        let eintrag = self.mit_block(id, |block| block.eintrag(platz))?;
        let gilt = eintrag.hash == *hash && datei(pfad) == Some((eintrag.bytes, eintrag.zeit));
        gilt.then(|| {
            self.gespart.fetch_add(1, Ordering::Relaxed);
            self.getroffen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert((z, tile));
            eintrag.bytes as usize
        })
    }

    /// Ob die Kachel in diesem Lauf einen Treffer hatte ([`Pixel::gleich`]):
    /// Dann hat der letzte fertige Lauf ihre Eltern aus genau dieser Datei
    /// gebaut, denn die Hashes schreibt ein Lauf erst nach der Pyramide. Eine
    /// Kachel mit gleichen Bytes ohne gültigen Eintrag, eine liegen gelassene
    /// und eine entfernte haben nie einen.
    /// Siehe docs/entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md.
    pub(super) fn getroffen(&self, z: u32, tile: TileId) -> bool {
        self.getroffen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&(z, tile))
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
            if block.setze(platz, neu) {
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
            let daten = als_bytes(self.kodierstand, &block);
            super::lege_ab(&Self::pfad(&self.ordner, id), &daten, None)?;
        }
        Ok(())
    }

    /// Schreibt jeden geänderten Block, nach der Pyramide: erst die übrigen
    /// nach [`NEU`], dann alles von dort nach [`ORDNER`]. Liefert, wie viele
    /// Kacheln der Lauf nicht kodiert hat, und wie viele Blöcke er schrieb.
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
            let daten = als_bytes(self.kodierstand, block);
            super::lege_ab(&Self::pfad(&self.neu, *id), &daten, None).map(drop)
        })?;
        verschiebe(&self.neu, &self.ordner)?;
        Ok((
            self.gespart.into_inner(),
            geaendert.len() + self.frueh.into_inner(),
        ))
    }
}

/// Bringt jede Blockdatei aus `neu` an ihren Platz in `ordner` und nimmt
/// `neu` weg.
fn verschiebe(neu: &Path, ordner: &Path) -> Result<()> {
    let stufen = match std::fs::read_dir(neu) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        stufen => stufen.with_context(|| format!("{} lesen", neu.display()))?,
    };
    for stufe in stufen {
        let stufe = stufe?;
        let ziel = ordner.join(stufe.file_name());
        std::fs::create_dir_all(&ziel).with_context(|| format!("{} anlegen", ziel.display()))?;
        for datei in std::fs::read_dir(stufe.path())? {
            let datei = datei?;
            let nach = ziel.join(datei.file_name());
            std::fs::rename(datei.path(), &nach).with_context(|| {
                format!("{} nach {} bringen", datei.path().display(), nach.display())
            })?;
        }
    }
    std::fs::remove_dir_all(neu).with_context(|| format!("{} entfernen", neu.display()))
}

/// Die Bytes einer Blockdatei, Little Endian, nach Platz geordnet.
fn als_bytes(kodierstand: u64, block: &Block) -> Vec<u8> {
    let plaetze: Vec<(u16, Eintrag)> = block.eintraege().collect();
    let mut out = Vec::with_capacity(KOPF + plaetze.len() * EINTRAG);
    out.extend_from_slice(MAGIE);
    out.extend_from_slice(&FASSUNG.to_le_bytes());
    out.extend_from_slice(&kodierstand.to_le_bytes());
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
/// Fassung oder einen anderen Kodierstand hat. Dann gilt der Block als leer,
/// und der Lauf kodiert seine Kacheln wie ohne Hash.
fn aus_bytes(daten: &[u8], kodierstand: u64) -> Option<Block> {
    let zahl = |von: usize, n: usize| daten.get(von..von + n);
    if zahl(0, 8)? != MAGIE
        || u32::from_le_bytes(zahl(8, 4)?.try_into().ok()?) != FASSUNG
        || kodierstand_im_kopf(u64::from_le_bytes(zahl(12, 8)?.try_into().ok()?)) != kodierstand
    {
        return None;
    }
    let anzahl = u32::from_le_bytes(zahl(20, 4)?.try_into().ok()?) as usize;
    if daten.len() != KOPF + anzahl * EINTRAG {
        return None;
    }
    let mut block = Block::leer();
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
        block.setze(platz, eintrag);
    }
    Some(block)
}

/// Der Kodierstand im Kopf einer Blockdatei. Bis v0.5.0 stand dort der
/// Fingerabdruck der ausführbaren Datei; die alten Builds kodieren wie
/// Kodierstand 1.
fn kodierstand_im_kopf(kopf: u64) -> u64 {
    match ALTE_BUILDS.contains(&kopf) {
        true => 1,
        false => kopf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bild(farbe: u8) -> RgbaImage {
        RgbaImage::from_pixel(4, 4, image::Rgba([farbe, 0, 0, 255]))
    }

    /// Ein Eintrag gilt für dieselben Pixel und dieselbe Datei, auch in
    /// einem späteren Lauf mit demselben Kodierstand. Andere Pixel, eine
    /// Datei, die jemand neu schrieb, eine fehlende Datei und ein anderer
    /// Kodierstand lassen den Lauf neu kodieren.
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

        let pixel = Pixel::neu(dir.path(), 7).unwrap();
        assert_eq!(pixel.gleich(3, tile, &rot, &kachel), None);
        pixel.merke(3, tile, &rot, &kachel).unwrap();
        assert_eq!(pixel.gleich(3, tile, &rot, &kachel), Some(3));
        assert_eq!(pixel.gleich(3, tile, &gruen, &kachel), None);
        assert_eq!(pixel.gleich(4, tile, &rot, &kachel), None);
        assert_eq!(pixel.schreibe().unwrap(), (1, 1));
        assert!(dir.path().join("pixel/3/-2.0.bin").is_file());

        let spaeter = Pixel::neu(dir.path(), 7).unwrap();
        assert_eq!(spaeter.gleich(3, tile, &rot, &kachel), Some(3));
        assert_eq!(spaeter.schreibe().unwrap(), (1, 0), "nichts geändert");
        let anderer = Pixel::neu(dir.path(), 8).unwrap();
        assert_eq!(anderer.gleich(3, tile, &rot, &kachel), None);

        // Dieselbe Grösse, neu geschrieben: die Zeit ist eine andere.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&kachel, b"neu").unwrap();
        assert_eq!(
            Pixel::neu(dir.path(), 7)
                .unwrap()
                .gleich(3, tile, &rot, &kachel),
            None
        );
        std::fs::remove_file(&kachel).unwrap();
        assert_eq!(
            Pixel::neu(dir.path(), 7)
                .unwrap()
                .gleich(3, tile, &rot, &kachel),
            None
        );
    }

    /// Ein Block eines alten Builds gilt als Kodierstand 1, nach dem nächsten
    /// nicht mehr.
    #[test]
    fn block_alter_builds_gilt_als_kodierstand_1() {
        let mut eintraege = Block::leer();
        eintraege.setze(
            3,
            Eintrag {
                hash: [1; 16],
                bytes: 2,
                zeit: 3,
            },
        );
        for &alt in ALTE_BUILDS {
            let daten = als_bytes(alt, &eintraege);
            assert_eq!(aus_bytes(&daten, 1), Some(eintraege.clone()));
            assert_eq!(aus_bytes(&daten, 2), None);
        }
    }

    /// Eine kaputte oder abgeschnittene Blockdatei gilt als leer und hält
    /// keinen Lauf auf; der nächste schreibt sie neu.
    #[test]
    fn kaputter_block_gilt_als_leer() {
        let mut eintraege = Block::leer();
        let e = Eintrag {
            hash: [9; 16],
            bytes: 12,
            zeit: 34,
        };
        eintraege.setze(1023, e);
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

    /// Ist die letzte erwartete Kachel eines Blocks fertig, geht er früh nach
    /// `pixel-neu` und aus dem Speicher, und der Lauf findet seine Einträge
    /// dort. `pixel` ändert sich erst mit `schreibe`, nach der Pyramide. Ein
    /// neuer Lauf nimmt ein liegen gebliebenes `pixel-neu` weg.
    /// Siehe docs/entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md.
    #[test]
    fn fertiger_block_geht_frueh_nach_pixel_neu() {
        let dir = tempfile::tempdir().unwrap();
        let kachel = dir.path().join("k.webp");
        std::fs::write(&kachel, b"alt").unwrap();
        let rot = hash(&bild(1), Packen::Schnell);
        let (a, b) = (TileId { x: 1, y: 1 }, TileId { x: 40, y: 1 });
        let frueh = dir.path().join("pixel-neu/3/0.0.bin");
        let fest = dir.path().join("pixel/3/0.0.bin");

        let pixel = Pixel::neu(dir.path(), 7).unwrap();
        pixel.erwarte(3, &[a, b]);
        pixel.merke(3, a, &rot, &kachel).unwrap();
        pixel.merke(3, b, &rot, &kachel).unwrap();
        pixel.fertig(3, a).unwrap();
        assert!(frueh.is_file() && !fest.exists());
        assert_eq!(
            pixel.bloecke.lock().unwrap().len(),
            1,
            "nur der Block von b"
        );
        assert_eq!(pixel.gleich(3, a, &rot, &kachel), Some(3));
        drop(pixel);

        let neu = Pixel::neu(dir.path(), 7).unwrap();
        assert!(!dir.path().join("pixel-neu").exists());
        assert_eq!(neu.gleich(3, a, &rot, &kachel), None);
        neu.erwarte(3, &[a]);
        neu.merke(3, a, &rot, &kachel).unwrap();
        neu.fertig(3, a).unwrap();
        assert_eq!(neu.schreibe().unwrap(), (0, 1));
        assert!(fest.is_file() && !dir.path().join("pixel-neu").exists());
        assert_eq!(
            Pixel::neu(dir.path(), 7)
                .unwrap()
                .gleich(3, a, &rot, &kachel),
            Some(3)
        );
    }
}
