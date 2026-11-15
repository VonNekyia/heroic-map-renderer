//! `--compact-tree`: packt einen fertigen Baum kompakt nach, ohne Welt und
//! ohne Assets. Die Pixel bleiben, und jede Kachel behält ihre Zeit.
//! Siehe docs/entscheidungen/0093-nachverdichten.md.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::Path;
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result};
use heroic_map_renderer::render::{MapInfo, Packen, TILE, TileId, decode_webp, encode_webp};
use rayon::prelude::*;

use super::pixel::{self, Pixel};
use super::{
    OFFEN as GEBIET_OFFEN, STAND_NEU, aenderungszeit, fremd, lies_bestand, manifest,
    ordner_der_schicht, schreibe_info, sekunden, tausche, tile_path, vorhandene_mit_zeit,
};

/// Was mit einer Kachel geschah.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kachel {
    /// Neu gepackt, mit den Bytes vorher und nachher.
    Neu(usize, usize),
    /// Schon kompakt.
    Schon,
    /// Seit dem Beginn von jemand anders geschrieben, oder nicht mehr da.
    Fremd,
    /// Liess sich nicht dekodieren.
    Kaputt,
}

/// Packt jede Kachel des Baums in `dir` kompakt, von der Basis bis zur
/// gröbsten Stufe. Zuerst trägt `map.json` die Packung ein, damit jeder Lauf,
/// der jetzt beginnt, schon kompakt packt. Was ein anderer Lauf seit dem
/// Beginn schrieb, lässt es aus. Nach jedem Block von Kacheln legt es deren
/// Hashes ab; ein neuer Aufruf erkennt daran, was schon kompakt ist, und
/// kodiert es nicht noch einmal. Schicht und Rest des Reliefs sind je ein
/// Baum mit eigenen Hashes und eigenem Manifest; es packt sie danach ebenso.
/// Liegt der Stand oder das Gebiet eines abgebrochenen Laufs, legt es keine
/// Hashes ab: Ein Treffer hiesse, dass die Eltern der Kachel stimmen, und die
/// baut erst das nächste Update.
/// Siehe docs/entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md.
pub(super) fn verdichte(dir: &Path, mit_manifest: bool) -> Result<()> {
    let beginn = SystemTime::now();
    let gestartet = Instant::now();
    let alt = lies_bestand(dir)?.with_context(|| {
        format!(
            "{} fehlt: --compact-tree verdichtet nur einen Baum, den ein Export angelegt hat",
            dir.join("map.json").display()
        )
    })?;
    if alt.compact != Some(true) {
        let info = MapInfo {
            compact: Some(true),
            ..alt.clone()
        };
        schreibe_info(dir, &info, None)?;
    }
    let offen = [STAND_NEU, GEBIET_OFFEN]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|pfad| pfad.exists());
    if let Some(offen) = &offen {
        println!(
            "Hashes:     keine neuen, solange {} liegt; die Eltern baut das nächste Update",
            offen.display()
        );
    }
    let mut ordner = vec![dir.to_path_buf()];
    if alt.relief_tiles.is_some() {
        ordner.extend(ordner_der_schicht(dir)?);
    }
    let mut summe = BTreeMap::<&str, usize>::new();
    for (i, ordner) in ordner.iter().enumerate() {
        if i > 0 {
            println!("Schicht:    {}", ordner.display());
        }
        verdichte_ordner(
            ordner,
            alt.min_zoom..=alt.max_zoom,
            beginn,
            (mit_manifest, offen.is_none()),
            &mut summe,
        )?;
    }
    let zahl = |name| summe.get(name).copied().unwrap_or(0);
    println!(
        "Verdichtet: {} Kacheln, {:.1} MB statt {:.1} MB, {} schon kompakt, {} übergangen, {} nicht \
         lesbar, in {:.1} s",
        zahl("neu"),
        zahl("nachher") as f64 / 1_048_576.0,
        zahl("vorher") as f64 / 1_048_576.0,
        zahl("schon"),
        zahl("fremd"),
        zahl("kaputt"),
        sekunden(gestartet)
    );
    Ok(())
}

/// Packt die Stufen `stufen` eines Ordners kompakt, mit seinem Manifest und,
/// mit `merken`, seinen Hashes, und zählt in `summe`.
fn verdichte_ordner(
    dir: &Path,
    stufen: RangeInclusive<u32>,
    beginn: SystemTime,
    (mit_manifest, merken): (bool, bool),
    summe: &mut BTreeMap<&'static str, usize>,
) -> Result<()> {
    let manifest = manifest::Lauf::beginne(dir, mit_manifest)?;
    let hashes = Pixel::neu(dir, pixel::KODIERSTAND)?;
    for z in stufen.rev() {
        let mut je_block: BTreeMap<(i32, i32), Vec<(TileId, SystemTime)>> = BTreeMap::new();
        for (tile, zeit) in vorhandene_mit_zeit(dir, z)? {
            je_block
                .entry(pixel::block(tile))
                .or_default()
                .push((tile, zeit));
        }
        let stufe: Vec<Kachel> = je_block
            .into_par_iter()
            .map(|(block, kacheln)| -> Result<Vec<Kachel>> {
                let out = kacheln
                    .into_iter()
                    .map(|(tile, zeit)| {
                        verdichte_kachel(dir, z, tile, zeit, beginn, (&hashes, merken))
                    })
                    .collect::<Result<Vec<_>>>()?;
                hashes.schliesse_block(z, block)?;
                Ok(out)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let mut hier = BTreeMap::<&str, usize>::new();
        for kachel in &stufe {
            let (name, vorher, nachher) = match *kachel {
                Kachel::Neu(vorher, nachher) => ("neu", vorher, nachher),
                Kachel::Schon => ("schon", 0, 0),
                Kachel::Fremd => ("fremd", 0, 0),
                Kachel::Kaputt => ("kaputt", 0, 0),
            };
            *hier.entry(name).or_default() += 1;
            *summe.entry("vorher").or_default() += vorher;
            *summe.entry("nachher").or_default() += nachher;
        }
        for (name, n) in &hier {
            *summe.entry(name).or_default() += n;
        }
        let zahl = |name| hier.get(name).copied().unwrap_or(0);
        println!(
            "Zoom {z:>2}:     {} neu, {} schon kompakt, {} übergangen, {} nicht lesbar",
            zahl("neu"),
            zahl("schon"),
            zahl("fremd"),
            zahl("kaputt")
        );
    }
    hashes.schreibe()?;
    manifest.schliesse(None)
}

/// Packt eine Kachel kompakt, die beim Auflisten die Zeit `gelistet` hatte.
/// Die neue Datei bekommt dieselbe Zeit: `--pyramid` und `--resume`
/// vergleichen Zeiten, und die Pixel sind dieselben. Unmittelbar vor dem
/// Tausch prüft es noch einmal, ob die Kachel so dasteht. Ihren Hash merkt
/// es nur mit `merken`.
/// Siehe docs/benutzung/kacheln.md, „Nachverdichten“.
fn verdichte_kachel(
    dir: &Path,
    z: u32,
    tile: TileId,
    gelistet: SystemTime,
    beginn: SystemTime,
    (hashes, merken): (&Pixel, bool),
) -> Result<Kachel> {
    let pfad = tile_path(dir, z, tile);
    if fremd(Some(gelistet), beginn, SystemTime::now()) {
        return Ok(Kachel::Fremd);
    }
    let daten = match std::fs::read(&pfad) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Kachel::Fremd),
        gelesen => gelesen.with_context(|| format!("{} lesen", pfad.display()))?,
    };
    let Ok(bild) = decode_webp(&daten, (TILE, TILE)) else {
        return Ok(Kachel::Kaputt);
    };
    let hash = pixel::hash(&bild, Packen::Kompakt);
    if hashes.gleich(z, tile, &hash, &pfad).is_some() {
        return Ok(Kachel::Schon);
    }
    let kompakt = encode_webp(&bild, Packen::Kompakt)?;
    let ergebnis = if kompakt == daten {
        Kachel::Schon
    } else {
        if aenderungszeit(&pfad) != Some(gelistet) {
            return Ok(Kachel::Fremd);
        }
        tausche(&pfad, &kompakt, Some(gelistet), false)
            .with_context(|| format!("{} schreiben", pfad.display()))?;
        Kachel::Neu(daten.len(), kompakt.len())
    };
    if merken {
        hashes.merke(z, tile, &hash, &pfad)?;
    }
    Ok(ergebnis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use std::time::Duration;

    fn verlauf() -> RgbaImage {
        RgbaImage::from_fn(TILE, TILE, |x, y| {
            image::Rgba([x as u8, y as u8, ((x + y) / 2) as u8, 255])
        })
    }

    /// Eine Kachel wird kompakt, mit denselben Pixeln und ihrer alten Zeit.
    /// Eine, die nach dem Beginn geschrieben wurde, bleibt; eine kaputte
    /// auch. Ein zweiter Aufruf erkennt die kompakte am Hash.
    #[test]
    fn kachel_wird_kompakt_mit_ihrer_zeit() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        let tile = TileId { x: 3, y: -2 };
        let pfad = tile_path(dir, 4, tile);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        std::fs::write(&pfad, encode_webp(&verlauf(), Packen::Schnell).unwrap()).unwrap();
        let damals = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&pfad)
            .unwrap()
            .set_modified(damals)
            .unwrap();
        let hashes = Pixel::neu(dir, 1).unwrap();
        let beginn = SystemTime::now();

        let erst = verdichte_kachel(dir, 4, tile, damals, beginn, (&hashes, true)).unwrap();
        // Ein Verlauf packt ohne Vorhersage grösser, die Pixel bleiben.
        assert!(matches!(erst, Kachel::Neu(..)), "{erst:?}");
        let daten = std::fs::read(&pfad).unwrap();
        assert_eq!(daten, encode_webp(&verlauf(), Packen::Kompakt).unwrap());
        assert_eq!(aenderungszeit(&pfad), Some(damals));
        assert_eq!(
            verdichte_kachel(dir, 4, tile, damals, beginn, (&hashes, true)).unwrap(),
            Kachel::Schon
        );
        // Der Hash liegt nach dem Block auf der Platte; ein neuer Aufruf
        // erkennt die Kachel daran, ohne zu kodieren.
        hashes.schliesse_block(4, pixel::block(tile)).unwrap();
        let hash = pixel::hash(&verlauf(), Packen::Kompakt);
        assert!(
            Pixel::neu(dir, 1)
                .unwrap()
                .gleich(4, tile, &hash, &pfad)
                .is_some()
        );

        // Nach dem Beginn geschrieben, etwa von einem Export daneben.
        let jetzt = SystemTime::now();
        assert_eq!(
            verdichte_kachel(dir, 4, tile, jetzt, beginn, (&hashes, true)).unwrap(),
            Kachel::Fremd
        );
        // Zwischen Auflisten und Tausch geschrieben.
        std::fs::write(&pfad, encode_webp(&verlauf(), Packen::Schnell).unwrap()).unwrap();
        let spaeter = Pixel::neu(dir, 2).unwrap();
        assert_eq!(
            verdichte_kachel(dir, 4, tile, damals, beginn, (&spaeter, true)).unwrap(),
            Kachel::Fremd
        );
        std::fs::write(&pfad, b"RIFF").unwrap();
        assert_eq!(
            verdichte_kachel(
                dir,
                4,
                tile,
                aenderungszeit(&pfad).unwrap(),
                SystemTime::now(),
                (&spaeter, true)
            )
            .unwrap(),
            Kachel::Kaputt
        );
    }
}
