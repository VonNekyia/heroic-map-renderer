//! Das Manifest eines Baums: je Kachel ihre Grösse und ihr ETag, für den
//! Download der Karte.
//! Siehe docs/plugin.md, „Manifest“.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use heroic_map_renderer::render::{TileId, pyramid};
use rayon::prelude::*;

use super::{entferne, je_kachel, tausche, tile_path};

/// Das Manifest neben `map.json`.
pub(super) const MANIFEST: &str = "manifest";

/// Liegt, solange ein Lauf Kacheln schreibt. Findet ein Lauf es vor, brach
/// der vorige ab, und das Manifest kennt vielleicht nicht jede Kachel: Er
/// liest den Baum dann ganz.
pub(super) const OFFEN: &str = "manifest-offen";

/// Das ETag einer Datei: Grösse und letzte Änderung in ns seit 1970, beide
/// hexadezimal, in Anführungszeichen. Dasselbe sendet der Server im Header.
pub(super) fn etag(meta: &Metadata) -> std::io::Result<String> {
    let ns = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    Ok(format!("\"{:x}-{ns:x}\"", meta.len()))
}

/// Die Kacheln, die ein Lauf über diesen Basiskacheln schreiben, liegen
/// lassen, leeren oder entfernen kann: sie selbst und ihre Eltern bis
/// Zoom 0, dazu die Eltern der Waisen, wie `build_pyramid` sie baut. Zur
/// Basis gehören auch die Kacheln ohne Chunk, deren Vorfahren
/// `ohne_veraltete` neu zusammensetzt.
pub(super) fn mit_eltern(
    max_zoom: u32,
    basis: &BTreeSet<TileId>,
    waisen: &BTreeMap<u32, BTreeSet<TileId>>,
) -> BTreeSet<(u32, TileId)> {
    let mut out = BTreeSet::new();
    let mut stufe = basis.clone();
    for z in (0..=max_zoom).rev() {
        out.extend(stufe.iter().map(|tile| (z, *tile)));
        stufe.extend(waisen.get(&z).into_iter().flatten());
        stufe = pyramid::parents(&stufe);
    }
    out
}

/// Ein Lauf, der Kacheln eines Baums schreibt.
pub(super) struct Lauf<'a> {
    dir: &'a Path,
    /// Ob das Manifest beim Beginn schon nicht mehr zum Baum passen kann.
    ganz: bool,
}

impl<'a> Lauf<'a> {
    /// Vor der ersten Kachel.
    pub(super) fn beginne(dir: &'a Path) -> Result<Lauf<'a>> {
        let offen = dir.join(OFFEN);
        let ganz = offen.exists() || !dir.join(MANIFEST).is_file();
        File::create(&offen).with_context(|| format!("{} anlegen", offen.display()))?;
        Ok(Lauf { dir, ganz })
    }

    /// Schreibt das Manifest neu, nach der letzten Kachel. `angefasst`: was
    /// der Lauf geschrieben, liegen gelassen oder entfernt haben kann, auch
    /// mehr; dann zieht es nur diese Kacheln im alten Manifest nach. Ohne
    /// liest es den ganzen Baum.
    pub(super) fn schliesse(self, angefasst: Option<&BTreeSet<(u32, TileId)>>) -> Result<()> {
        let started = Instant::now();
        let pfad = self.dir.join(MANIFEST);
        let nachgezogen = match angefasst {
            Some(angefasst) if !self.ganz => nachgezogen(self.dir, &pfad, angefasst)?,
            _ => None,
        };
        let (daten, kacheln) = match nachgezogen {
            Some(neu) => neu,
            None => ganz(self.dir)?,
        };
        tausche(&pfad, &daten, None, true)
            .with_context(|| format!("{} schreiben", pfad.display()))?;
        entferne(&self.dir.join(OFFEN))?;
        println!(
            "Manifest:   {kacheln} Kacheln, {:.1} MB gepackt, in {:.1} s",
            daten.len() as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        );
        Ok(())
    }
}

/// Schreibt die Zeile einer Kachel, falls sie dasteht; `true` dann.
fn zeile(aus: &mut impl Write, dir: &Path, z: u32, tile: TileId) -> Result<bool> {
    let pfad = tile_path(dir, z, tile);
    let meta = match std::fs::metadata(&pfad) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("{} lesen", pfad.display())),
    };
    let etag = etag(&meta).with_context(|| format!("{} lesen", pfad.display()))?;
    writeln!(aus, "{z}/{}/{} {} {etag}", tile.x, tile.y, meta.len())?;
    Ok(true)
}

/// Die Lage einer Zeile, `None`, wenn sie keiner aus [`zeile`] gleicht.
fn lage(zeile: &str) -> Option<(u32, TileId)> {
    let (lage, rest) = zeile.strip_suffix('\n')?.split_once(' ')?;
    rest.split_once(' ')?;
    let mut teile = lage.split('/');
    let z = teile.next()?.parse().ok()?;
    let x = teile.next()?.parse().ok()?;
    let y = teile.next()?.parse().ok()?;
    teile.next().is_none().then_some((z, TileId { x, y }))
}

/// Das alte Manifest mit neuen Zeilen für `angefasst`, gepackt, und die
/// Zahl der Kacheln. `None`, wenn es keins gibt oder es sich nicht lesen
/// lässt: Dann gilt es nicht.
fn nachgezogen(
    dir: &Path,
    pfad: &Path,
    angefasst: &BTreeSet<(u32, TileId)>,
) -> Result<Option<(Vec<u8>, usize)>> {
    let Ok(datei) = File::open(pfad) else {
        return Ok(None);
    };
    let mut alt = BufReader::new(GzDecoder::new(datei));
    let mut aus = GzEncoder::new(Vec::new(), Compression::default());
    let mut neu = angefasst.iter().peekable();
    let (mut kacheln, mut vorige) = (0, None);
    let mut text = String::new();
    loop {
        text.clear();
        match alt.read_line(&mut text) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => return Ok(None),
        }
        // Die Zeilen stehen aufsteigend; sonst ist es nicht von hier.
        let Some(hier) = lage(&text).filter(|hier| vorige < Some(*hier)) else {
            return Ok(None);
        };
        vorige = Some(hier);
        while let Some(&&(z, tile)) = neu.peek()
            && (z, tile) <= hier
        {
            neu.next();
            kacheln += usize::from(zeile(&mut aus, dir, z, tile)?);
        }
        // Eine angefasste Kachel steht schon neu da, oder sie fehlt jetzt.
        if !angefasst.contains(&hier) {
            aus.write_all(text.as_bytes())?;
            kacheln += 1;
        }
    }
    for &(z, tile) in neu {
        kacheln += usize::from(zeile(&mut aus, dir, z, tile)?);
    }
    Ok(Some((aus.finish()?, kacheln)))
}

/// Das Manifest aus dem ganzen Baum, gepackt, und die Zahl der Kacheln.
fn ganz(dir: &Path) -> Result<(Vec<u8>, usize)> {
    let mut stufen: Vec<u32> = match std::fs::read_dir(dir) {
        Ok(eintraege) => eintraege
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let z: u32 = name.parse().ok()?;
                (z.to_string() == name && e.path().is_dir()).then_some(z)
            })
            .collect(),
        Err(e) => return Err(e).with_context(|| format!("{} lesen", dir.display())),
    };
    stufen.sort_unstable();
    let mut aus = GzEncoder::new(Vec::new(), Compression::default());
    let mut kacheln = 0;
    for z in stufen {
        // ponytail: hält die Kacheln einer Stufe zum Sortieren, 8 Byte je
        // Kachel, an der grossen Welt rund 18 MB; spaltenweise sortieren,
        // wenn das stört.
        let mut stufe = Vec::new();
        je_kachel(dir, z, None, |tile, _| {
            stufe.push(tile);
            Ok(())
        })?;
        stufe.sort_unstable();
        // Je Kachel ein `metadata`, über die Threads verteilt; in Stücken,
        // damit nur die Zeilen eines Stücks im Speicher liegen.
        for stueck in stufe.chunks(1 << 16) {
            let zeilen = stueck
                .par_iter()
                .map(|&tile| {
                    let mut text = Vec::new();
                    Ok(zeile(&mut text, dir, z, tile)?.then_some(text))
                })
                .collect::<Result<Vec<_>>>()?;
            for text in zeilen.into_iter().flatten() {
                aus.write_all(&text)?;
                kacheln += 1;
            }
        }
    }
    Ok((aus.finish()?, kacheln))
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::time::Duration;

    use super::*;

    /// Eine Kachel mit `n` Bytes und dieser Zeit.
    fn lege(dir: &Path, z: u32, tile: TileId, n: usize, ns: u64) {
        let pfad = tile_path(dir, z, tile);
        std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        let datei = File::create(&pfad).unwrap();
        datei.set_len(n as u64).unwrap();
        datei
            .set_modified(UNIX_EPOCH + Duration::from_nanos(ns))
            .unwrap();
    }

    fn entpackt(daten: &[u8]) -> String {
        let mut text = String::new();
        GzDecoder::new(daten).read_to_string(&mut text).unwrap();
        text
    }

    fn vektor() -> serde_json::Value {
        let pfad = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/manifest.json");
        serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap()
    }

    /// Aus den Kacheln des Testvektors entsteht Zeichen für Zeichen sein
    /// Manifest: aufsteigend nach z, x, y als Zahlen, das ETag aus Grösse
    /// und Zeit. Die Summen je Stufe stimmen mit den Zeilen.
    #[test]
    fn manifest_wie_im_testvektor() {
        let vektor = vektor();
        let dir = tempfile::tempdir().unwrap();
        for k in vektor["kacheln"].as_array().unwrap() {
            let zahl = |name: &str| k[name].as_i64().unwrap();
            let tile = TileId {
                x: zahl("x") as i32,
                y: zahl("y") as i32,
            };
            lege(
                dir.path(),
                zahl("z") as u32,
                tile,
                zahl("bytes") as usize,
                k["mtime_ns"].as_u64().unwrap(),
            );
        }
        let (daten, kacheln) = ganz(dir.path()).unwrap();
        let text = entpackt(&daten);
        assert_eq!(text, vektor["manifest"].as_str().unwrap());
        assert_eq!(kacheln, text.lines().count());

        let mut stufen: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        for zeile in text.lines() {
            let mut teile = zeile.split(' ');
            let z = teile.next().unwrap().split('/').next().unwrap();
            let stufe = stufen.entry(z.parse().unwrap()).or_default();
            stufe.0 += 1;
            stufe.1 += teile.next().unwrap().parse::<u64>().unwrap();
        }
        let erwartet: BTreeMap<u64, (u64, u64)> = vektor["stufen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let zahl = |name: &str| s[name].as_u64().unwrap();
                (zahl("z"), (zahl("kacheln"), zahl("bytes")))
            })
            .collect();
        assert_eq!(stufen, erwartet);
    }

    /// Nachgezogen gleicht das Manifest einem, das den ganzen Baum liest:
    /// mit einer neu geschriebenen, einer entfernten und einer neuen Kachel,
    /// auch wenn `angefasst` mehr nennt. Ein Manifest, das sich nicht lesen
    /// lässt oder dessen Zeilen nicht aufsteigen, gilt nicht.
    #[test]
    fn nachgezogen_wie_ganz_gelesen() {
        let dir = tempfile::tempdir().unwrap();
        let t = |x, y| TileId { x, y };
        for (z, tile) in [(0, t(0, 0)), (1, t(0, 0)), (1, t(-1, 0)), (1, t(0, -1))] {
            lege(dir.path(), z, tile, 10, 1_759_708_800_000_000_000);
        }
        let pfad = dir.path().join(MANIFEST);
        std::fs::write(&pfad, ganz(dir.path()).unwrap().0).unwrap();

        lege(dir.path(), 1, t(0, 0), 20, 1_759_708_900_000_000_000);
        std::fs::remove_file(tile_path(dir.path(), 1, t(-1, 0))).unwrap();
        lege(dir.path(), 2, t(3, 3), 30, 1_759_708_900_000_000_000);
        let angefasst = BTreeSet::from([
            (0, t(0, 0)),
            (1, t(-1, 0)),
            (1, t(0, 0)),
            (1, t(5, 5)),
            (2, t(3, 3)),
        ]);
        let (daten, kacheln) = nachgezogen(dir.path(), &pfad, &angefasst).unwrap().unwrap();
        let (soll, soll_kacheln) = ganz(dir.path()).unwrap();
        assert_eq!(entpackt(&daten), entpackt(&soll));
        assert_eq!((kacheln, soll_kacheln), (4, 4));

        std::fs::write(&pfad, b"kein gzip").unwrap();
        assert!(
            nachgezogen(dir.path(), &pfad, &angefasst)
                .unwrap()
                .is_none()
        );
        let mut verkehrt = GzEncoder::new(Vec::new(), Compression::default());
        verkehrt
            .write_all(b"1/0/0 10 \"a-0\"\n0/0/0 10 \"a-0\"\n")
            .unwrap();
        std::fs::write(&pfad, verkehrt.finish().unwrap()).unwrap();
        assert!(
            nachgezogen(dir.path(), &pfad, &angefasst)
                .unwrap()
                .is_none()
        );
        std::fs::remove_file(&pfad).unwrap();
        assert!(
            nachgezogen(dir.path(), &pfad, &angefasst)
                .unwrap()
                .is_none()
        );
    }

    /// Ein Lauf ohne Manifest oder nach einem abgebrochenen liest den Baum
    /// ganz, auch wenn er nur wenig angefasst hat; danach liegt die Marke
    /// nicht mehr.
    #[test]
    fn nach_einem_abbruch_ganz_gelesen() {
        let dir = tempfile::tempdir().unwrap();
        let t = TileId { x: 0, y: 0 };
        lege(dir.path(), 0, t, 10, 1_759_708_800_000_000_000);
        Lauf::beginne(dir.path())
            .unwrap()
            .schliesse(Some(&BTreeSet::new()))
            .unwrap();
        let pfad = dir.path().join(MANIFEST);
        let eins = entpackt(&std::fs::read(&pfad).unwrap());
        assert_eq!(eins.lines().count(), 1);
        assert!(!dir.path().join(OFFEN).exists());

        // Ein Lauf schreibt eine Kachel und bricht ab, der nächste fasst
        // sie nicht an.
        Lauf::beginne(dir.path()).unwrap();
        lege(dir.path(), 1, t, 10, 1_759_708_900_000_000_000);
        Lauf::beginne(dir.path())
            .unwrap()
            .schliesse(Some(&BTreeSet::new()))
            .unwrap();
        assert_eq!(
            entpackt(&std::fs::read(&pfad).unwrap()),
            entpackt(&ganz(dir.path()).unwrap().0)
        );
        assert!(!dir.path().join(OFFEN).exists());
        // Ohne Abbruch zieht er nur nach.
        lege(dir.path(), 2, t, 10, 1_759_709_000_000_000_000);
        Lauf::beginne(dir.path())
            .unwrap()
            .schliesse(Some(&BTreeSet::new()))
            .unwrap();
        assert_eq!(entpackt(&std::fs::read(&pfad).unwrap()).lines().count(), 2);
    }

    /// Die Eltern reichen bis Zoom 0, die der Waisen ab ihrer Stufe.
    #[test]
    fn eltern_bis_zoom_null() {
        let t = |x, y| TileId { x, y };
        let waisen = BTreeMap::from([(2, BTreeSet::from([t(-3, 0)]))]);
        let alle = mit_eltern(3, &BTreeSet::from([t(5, 3)]), &waisen);
        let erwartet = BTreeSet::from([
            (3, t(5, 3)),
            (2, t(2, 1)),
            (1, t(1, 0)),
            (1, t(-2, 0)),
            (0, t(0, 0)),
            (0, t(-1, 0)),
        ]);
        assert_eq!(alle, erwartet);
    }
}
