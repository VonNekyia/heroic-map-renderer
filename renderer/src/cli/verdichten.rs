//! `--repack` und `--compact-tree`: packt einen fertigen Baum auf eine
//! andere Stufe um, ohne Welt und ohne Assets. Verlustfrei nach verlustfrei
//! bleiben die Pixel, und jede Kachel behält ihre Zeit. Mit zusammengelegten
//! Farben bekommt eine Kachel die Zeit des Umpackens, und die Eltern über
//! den nativen Stufen entstehen neu aus den Kindern.
//! Siehe docs/entscheidungen/0093-nachverdichten.md und
//! docs/entscheidungen/0146-umpacken.md.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use heroic_map_renderer::render::{
    MapInfo, Packen, TILE, TileId, decode_webp, encode_webp, lege_zusammen,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::pixel::{self, Pixel};
use super::{
    OFFEN as GEBIET_OFFEN, Qualitaet, STAND_NEU, abbruch, aenderungszeit, entferne, fremd, gebiete,
    lies_bestand, manifest, nativ_des_baums, ordner_der_schicht, pyramide_bauen,
    qualitaet_des_baums, schreibe_info, sekunden, stufen_des_baums, tausche, tile_path,
    vorhandene_mit_zeit,
};

/// Ein Umpacken mit zusammengelegten Farben, das noch nicht fertig ist. Solange
/// die Datei liegt, setzt ein neuer Aufruf mit demselben Ziel fort; jeder
/// volle Lauf räumt sie weg.
pub(super) const UMPACKEN_OFFEN: &str = "repack-offen.json";

/// Was in [`UMPACKEN_OFFEN`] steht.
#[derive(Serialize, Deserialize)]
struct Offen {
    /// Die Stufe des Baums vor dem Umpacken.
    quelle: String,
    ziel: String,
    /// Der Beginn in ns seit 1970: Was danach geschrieben wurde, ist umgepackt.
    beginn: u64,
}

/// Was mit einer Kachel geschah.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kachel {
    /// Neu gepackt, mit den Bytes vorher und nachher.
    Neu(usize, usize),
    /// Schon so gepackt.
    Schon,
    /// Seit dem Beginn von jemand anders geschrieben, oder nicht mehr da.
    Fremd,
    /// Liess sich nicht dekodieren.
    Kaputt,
}

/// Packt den Baum in `dir` auf die Stufe `ziel` um. Vor der ersten Kachel
/// trägt `map.json` die Stufe ein, damit jeder Lauf, der jetzt beginnt,
/// schon so packt. Je Kachel gilt die höchste Stufe aus `ziel` und den
/// gemerkten Gebieten.
///
/// - **Verlustfrei nach verlustfrei** packt es jede Stufe der Pyramide neu,
///   mit denselben Pixeln und der alten Zeit; was ein anderer Lauf seit dem
///   Beginn schrieb, lässt es aus. Nach jedem Block legt es die Hashes ab,
///   ausser wenn ein angefangener Stand oder ein offenes Gebiet liegt: Ein
///   Treffer hiesse, dass die Eltern der Kachel stimmen, und die baut erst
///   das nächste Update.
///   Siehe docs/entscheidungen/0124-pyramide-nur-ueber-neuen-kindern.md.
/// - **Mit zusammengelegten Farben** legt es [`UMPACKEN_OFFEN`] vor
///   `map.json` ab, packt die
///   Basis und die nativen Stufen aus ihren Pixeln, je Kachel mit der Zeit des
///   Umpackens, und baut am Ende jede Elternkachel darüber neu. Ob eine
///   Kachel schon umgepackt ist, sagt ihre Zeit gegen den Beginn, nicht ihr
///   Hash: Die Hashes der Blöcke in Arbeit verliert ein Abbruch.
///
/// Schicht und Rest des Reliefs sind je ein Baum mit eigenen Hashes und
/// eigenem Manifest; sie bleiben verlustfrei.
/// Siehe docs/entscheidungen/0146-umpacken.md.
pub(super) fn packe_um(dir: &Path, ziel: Qualitaet, mit_manifest: bool) -> Result<()> {
    let jetzt = SystemTime::now();
    let gestartet = Instant::now();
    let alt = lies_bestand(dir)?.with_context(|| {
        format!(
            "{} fehlt: --repack und --compact-tree packen nur einen Baum um, den ein Export \
             angelegt hat",
            dir.join("map.json").display()
        )
    })?;
    let karte = dir.join("map.json");
    let pfad_offen = dir.join(UMPACKEN_OFFEN);
    let offen: Option<Offen> = match std::fs::read(&pfad_offen) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        gelesen => Some(
            gelesen
                .map_err(anyhow::Error::from)
                .and_then(|d| serde_json::from_slice(&d).map_err(anyhow::Error::from))
                .with_context(|| format!("{} lesen", pfad_offen.display()))?,
        ),
    };
    // Nennt map.json schon das Ziel, steht die Quelle nur noch in der Datei
    // des Umpackens. Nennt es das Ziel noch nicht, endete dieser Aufruf
    // zwischen beiden Dateien oder ein voller Lauf vor dem Wegräumen der
    // Datei: Keine Kachel ist umgepackt, die Stufe nennt map.json, und es
    // beginnt neu, mit neuem Beginn.
    let (quelle, beginn, ablegen) = match &offen {
        Some(offen) => {
            ensure!(
                offen.ziel == ziel.name(),
                "{} liegt da: ein abgebrochenes Umpacken nach {}; --repack setzt es nur mit \
                 --quality {} fort, eine andere Stufe nimmt ein voller Lauf",
                pfad_offen.display(),
                offen.ziel,
                offen.ziel
            );
            match qualitaet_des_baums(dir, &alt)? {
                im_baum if im_baum == ziel => (
                    Qualitaet::aus_name(&offen.quelle)
                        .with_context(|| format!("{} nennt keine Stufe", pfad_offen.display()))?,
                    UNIX_EPOCH + Duration::from_nanos(offen.beginn),
                    false,
                ),
                im_baum => (im_baum, jetzt, true),
            }
        }
        None => (qualitaet_des_baums(dir, &alt)?, jetzt, true),
    };
    // Mit zusammengelegten Farben gälte der Hash den Pixeln vor der Vorstufe,
    // die es hier nicht mehr gibt.
    ensure!(
        quelle >= Qualitaet::MaximalCompact,
        "{} packt {}: --repack und --compact-tree packen nur einen Baum mit maximal oder \
         maximal_compact um; zurück führt nur ein voller Lauf mit --quality",
        karte.display(),
        quelle.name()
    );
    // Ein abgebrochener Lauf kann andere Gebiete haben; das nächste Update
    // oder --resume schliesst ihn ab.
    ensure!(
        !dir.join(gebiete::GEBIETE_NEU).exists(),
        "{} liegt da: ein Lauf ist abgebrochen; erst ein Update oder --resume schliesst ihn ab",
        dir.join(gebiete::GEBIETE_NEU).display()
    );
    let zusammen = ziel.packen().staerke().is_some();
    ensure!(
        !zusammen || !dir.join(STAND_NEU).exists(),
        "{} liegt da: ein Lauf ist abgebrochen; --repack {} legt erst nach dem nächsten Update \
         oder --resume Farben zusammen",
        dir.join(STAND_NEU).display(),
        ziel.name()
    );
    let tabelle = stufen_des_baums(dir, ziel.packen(), &alt)?;
    let info = MapInfo {
        compact: (ziel == Qualitaet::MaximalCompact).then_some(true),
        quality: match (&alt.quality, ziel) {
            (None, Qualitaet::MaximalCompact | Qualitaet::Maximal) => None,
            _ => Some(ziel.name().to_string()),
        },
        ..alt.clone()
    };
    // Erst die Datei des Umpackens, dann map.json: Endet der Aufruf
    // dazwischen, nennt der Baum noch die Quelle, und das Fortsetzen trägt das
    // Ziel nach.
    if zusammen && ablegen {
        let offen = Offen {
            quelle: quelle.name().to_string(),
            ziel: ziel.name().to_string(),
            beginn: u64::try_from(beginn.duration_since(UNIX_EPOCH)?.as_nanos())?,
        };
        tausche(&pfad_offen, &serde_json::to_vec(&offen)?, None, true)
            .with_context(|| format!("{} schreiben", pfad_offen.display()))?;
    }
    if (info.compact, &info.quality) != (alt.compact, &alt.quality) {
        schreibe_info(dir, &info, None)?;
    }
    let offen_lauf = [STAND_NEU, GEBIET_OFFEN]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|pfad| pfad.exists());
    if let Some(offen_lauf) = &offen_lauf {
        println!(
            "Hashes:     keine neuen, solange {} liegt; die Eltern baut {}",
            offen_lauf.display(),
            if zusammen {
                "am Ende dieser Aufruf"
            } else {
                "das nächste Update"
            }
        );
    }
    let merken = offen_lauf.is_none();
    let mut summe = BTreeMap::<&str, usize>::new();
    // Mit zusammengelegten Farben nur die Basis und die nativen Stufen; die
    // Eltern darüber baut danach die Pyramide.
    let nativ = nativ_des_baums(&alt).min(alt.max_zoom - alt.min_zoom);
    let stufen = match zusammen {
        true => alt.max_zoom - nativ..=alt.max_zoom,
        false => alt.min_zoom..=alt.max_zoom,
    };
    let packen_fuer = |z: u32, tile: TileId| {
        tabelle
            .as_ref()
            .and_then(|t| t.gehoben(z, tile))
            .map_or(ziel, |stufe| stufe.max(ziel))
            .packen()
    };
    verdichte_ordner(
        dir,
        stufen,
        beginn,
        (mit_manifest, merken),
        &packen_fuer,
        &mut summe,
    )?;
    if alt.relief_tiles.is_some() {
        // Die Schicht ist Daten für den Mod: in jeder Stufe verlustfrei.
        let schicht = match ziel {
            Qualitaet::Maximal => Packen::Schnell,
            _ => Packen::Kompakt,
        };
        for ordner in ordner_der_schicht(dir)? {
            println!("Schicht:    {}", ordner.display());
            verdichte_ordner(
                &ordner,
                alt.min_zoom..=alt.max_zoom,
                beginn,
                (mit_manifest, merken),
                &|_, _| schicht,
                &mut summe,
            )?;
        }
    }
    if zusammen {
        pyramide_bauen(
            dir,
            mit_manifest,
            SystemTime::now(),
            vorhandene_mit_zeit,
            true,
        )?;
        entferne(&pfad_offen)?;
    }
    let zahl = |name| summe.get(name).copied().unwrap_or(0);
    println!(
        "{}: {} Kacheln, {:.1} MB statt {:.1} MB, {} schon so, {} übergangen, {} nicht \
         lesbar, in {:.1} s",
        if zusammen { "Umgepackt" } else { "Verdichtet" },
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

/// Packt die Stufen `stufen` eines Ordners je Kachel so, wie `packen_fuer`
/// sagt, mit seinem Manifest und, mit `merken`, seinen Hashes, und zählt in
/// `summe`. Testhaken `HEROIC_ABBRUCH`: `umpacken-N` bricht nach N neu
/// gepackten Kacheln ab, mitten in einem Block.
/// Siehe docs/entwicklung/tests.md, „Testhaken“.
fn verdichte_ordner(
    dir: &Path,
    stufen: RangeInclusive<u32>,
    beginn: SystemTime,
    (mit_manifest, merken): (bool, bool),
    packen_fuer: &(dyn Fn(u32, TileId) -> Packen + Sync),
    summe: &mut BTreeMap<&'static str, usize>,
) -> Result<()> {
    let manifest = manifest::Lauf::beginne(dir, mit_manifest)?;
    let hashes = Pixel::neu(dir, pixel::KODIERSTAND)?;
    let abbruch_nach: Option<usize> = abbruch()
        .and_then(|a| a.strip_prefix("umpacken-").map(str::to_string))
        .and_then(|n| n.parse().ok());
    let neu_gepackt = AtomicUsize::new(0);
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
                        let packen = packen_fuer(z, tile);
                        let kachel = verdichte_kachel(
                            dir,
                            z,
                            tile,
                            zeit,
                            beginn,
                            packen,
                            (&hashes, merken),
                        )?;
                        if let Some(n) = abbruch_nach
                            && matches!(kachel, Kachel::Neu(..))
                            && neu_gepackt.fetch_add(1, Ordering::Relaxed) + 1 >= n
                        {
                            bail!("Abbruch nach {n} umgepackten Kacheln (HEROIC_ABBRUCH)");
                        }
                        Ok(kachel)
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
            "Zoom {z:>2}:     {} neu, {} schon so, {} übergangen, {} nicht lesbar",
            zahl("neu"),
            zahl("schon"),
            zahl("fremd"),
            zahl("kaputt")
        );
    }
    hashes.schreibe()?;
    manifest.schliesse(None)
}

/// Packt eine Kachel so, wie `packen` sagt, die beim Auflisten die Zeit
/// `gelistet` hatte. Unmittelbar vor dem Tausch prüft es noch einmal, ob die
/// Kachel so dasteht. Ihren Hash merkt es nur mit `merken`.
///
/// - **Verlustfrei** bekommt die neue Datei dieselbe Zeit: `--pyramid` und
///   `--resume` vergleichen Zeiten, und die Pixel sind dieselben.
/// - **Mit zusammengelegten Farben** packt es aus den Pixeln der Datei, die
///   verlustfrei ist, und die Kachel bekommt die Zeit des Umpackens: Daran
///   erkennt ein Fortsetzen, dass sie umgepackt ist.
///
/// Siehe docs/benutzung/kacheln.md, „Nachverdichten“.
fn verdichte_kachel(
    dir: &Path,
    z: u32,
    tile: TileId,
    gelistet: SystemTime,
    beginn: SystemTime,
    packen: Packen,
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
    // Der Hash gilt den Pixeln vor der Vorstufe, wie im Lauf.
    let hash = pixel::hash(&bild, packen);
    let zusammen = packen.staerke().is_some();
    if !zusammen && hashes.gleich(z, tile, &hash, &pfad).is_some() {
        return Ok(Kachel::Schon);
    }
    let neu = match packen.staerke() {
        Some(staerke) => {
            let mut danach = bild.clone();
            lege_zusammen(&mut danach, staerke);
            encode_webp(&danach, packen)?
        }
        None => encode_webp(&bild, packen)?,
    };
    let ergebnis = if neu == daten && !zusammen {
        Kachel::Schon
    } else {
        if aenderungszeit(&pfad) != Some(gelistet) {
            return Ok(Kachel::Fremd);
        }
        let zeit = (!zusammen).then_some(gelistet);
        tausche(&pfad, &neu, zeit, false)
            .with_context(|| format!("{} schreiben", pfad.display()))?;
        Kachel::Neu(daten.len(), neu.len())
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

        let erst = verdichte_kachel(
            dir,
            4,
            tile,
            damals,
            beginn,
            Packen::Kompakt,
            (&hashes, true),
        )
        .unwrap();
        // Ein Verlauf packt ohne Vorhersage grösser, die Pixel bleiben.
        assert!(matches!(erst, Kachel::Neu(..)), "{erst:?}");
        let daten = std::fs::read(&pfad).unwrap();
        assert_eq!(daten, encode_webp(&verlauf(), Packen::Kompakt).unwrap());
        assert_eq!(aenderungszeit(&pfad), Some(damals));
        assert_eq!(
            verdichte_kachel(
                dir,
                4,
                tile,
                damals,
                beginn,
                Packen::Kompakt,
                (&hashes, true)
            )
            .unwrap(),
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
            verdichte_kachel(
                dir,
                4,
                tile,
                jetzt,
                beginn,
                Packen::Kompakt,
                (&hashes, true)
            )
            .unwrap(),
            Kachel::Fremd
        );
        // Zwischen Auflisten und Tausch geschrieben.
        std::fs::write(&pfad, encode_webp(&verlauf(), Packen::Schnell).unwrap()).unwrap();
        let spaeter = Pixel::neu(dir, 2).unwrap();
        assert_eq!(
            verdichte_kachel(
                dir,
                4,
                tile,
                damals,
                beginn,
                Packen::Kompakt,
                (&spaeter, true)
            )
            .unwrap(),
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
                Packen::Kompakt,
                (&spaeter, true)
            )
            .unwrap(),
            Kachel::Kaputt
        );
    }
}
