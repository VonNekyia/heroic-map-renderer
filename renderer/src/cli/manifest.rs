//! Das Manifest eines Baums: je Kachel ihre Grösse und ihr ETag, für den
//! Download der Karte.
//! Siehe docs/plugin.md, „Manifest“.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use heroic_map_renderer::render::{TileId, pyramid};

use super::{entferne, je_kachel, tausche, tile_path};

/// Das Manifest neben `map.json`.
pub(super) const MANIFEST: &str = "manifest";

/// Je Lauf eine Marke, solange er Kacheln schreibt:
/// `manifest-offen-<pid>-<ns>`. Findet ein Lauf eine fremde vor, läuft der
/// andere noch, etwa ein `--pyramid` neben einem Export, oder er brach ab.
/// Dann kennt das Manifest vielleicht nicht jede Kachel, und er liest den
/// Baum ganz. Jeder Lauf entfernt seine eigene Marke und die eines
/// Prozesses, der beim Beginn schon nicht mehr lief.
pub(super) const OFFEN: &str = "manifest-offen-";

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

/// Ein Lauf, der Kacheln eines Baums schreibt, mit `--manifest`.
pub(super) struct Lauf<'a> {
    dir: &'a Path,
    /// Ob das Manifest beim Beginn schon nicht mehr zum Baum passen kann.
    ganz: bool,
    /// Die eigene Marke und die verwaisten von Prozessen, die beim Beginn
    /// nicht mehr liefen; nach dem Schreiben gehen sie weg.
    eigene: PathBuf,
    verwaist: Vec<PathBuf>,
}

impl<'a> Lauf<'a> {
    /// Vor der ersten Kachel. Ohne `--manifest` (`schreiben`) entfernt es ein
    /// altes Manifest, denn nach diesem Lauf stimmte es nicht mehr, und gibt
    /// keinen Lauf.
    pub(super) fn beginne(dir: &'a Path, schreiben: bool) -> Result<Option<Lauf<'a>>> {
        if !schreiben {
            entferne(&dir.join(MANIFEST))?;
            return Ok(None);
        }
        let fremde: Vec<(PathBuf, u32)> = std::fs::read_dir(dir)
            .with_context(|| format!("{} lesen", dir.display()))?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let pid = name.strip_prefix(OFFEN)?.split('-').next()?.parse().ok()?;
                Some((e.path(), pid))
            })
            .collect();
        let ganz = !fremde.is_empty() || !dir.join(MANIFEST).is_file();
        let verwaist = fremde
            .into_iter()
            .filter(|&(_, pid)| !laeuft(pid))
            .map(|(pfad, _)| pfad)
            .collect();
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let eigene = dir.join(format!("{OFFEN}{}-{ns}", std::process::id()));
        File::create(&eigene).with_context(|| format!("{} anlegen", eigene.display()))?;
        Ok(Some(Lauf {
            dir,
            ganz,
            eigene,
            verwaist,
        }))
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
        for marke in self.verwaist.iter().chain([&self.eigene]) {
            entferne(marke)?;
        }
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

/// Ob ein Prozess mit dieser Nummer läuft. Im Zweifel ja: Dann bleibt seine
/// Marke, und jeder Lauf liest ganz, bis sie weg ist.
#[cfg(windows)]
fn laeuft(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: Der Griff stammt aus OpenProcess und wird genau einmal
    // geschlossen; `code` lebt über den Aufruf.
    unsafe {
        let griff = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if griff.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let gelesen = GetExitCodeProcess(griff, &mut code) != 0;
        CloseHandle(griff);
        !gelesen || code == STILL_ACTIVE as u32
    }
}

#[cfg(unix)]
fn laeuft(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return true;
    };
    // SAFETY: Signal 0 prüft nur, ob es den Prozess gibt.
    unsafe { libc::kill(pid, 0) == 0 }
    || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(any(windows, unix)))]
fn laeuft(_: u32) -> bool {
    true
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
        // Grösse und Zeit aus dem Verzeichnis, im selben Durchgang: Unter
        // Windows kostet das nichts, unter Linux einen `statx` je Datei.
        // ponytail: hält die Zeilen einer Stufe zum Sortieren, für einen
        // Satz zum Download rund 7 MB; spaltenweise sortieren, wenn das stört.
        let mut stufe = Vec::new();
        je_kachel(dir, z, None, |tile, eintrag| {
            match eintrag.metadata() {
                Ok(meta) => stufe.push((tile, meta.len(), etag(&meta)?)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("{} lesen", eintrag.path().display()));
                }
            }
            Ok(())
        })?;
        stufe.sort_unstable_by_key(|(tile, ..)| *tile);
        for (tile, groesse, etag) in stufe {
            writeln!(aus, "{z}/{}/{} {groesse} {etag}", tile.x, tile.y)?;
            kacheln += 1;
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

    /// Die Nummer eines Prozesses, der eben endete.
    fn beendeter_prozess() -> u32 {
        let mut kind = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = kind.id();
        kind.wait().unwrap();
        assert!(!laeuft(pid), "{pid} läuft noch");
        pid
    }

    /// Dieser Prozess läuft, einer, der endete, nicht.
    #[test]
    fn laeuft_nur_wer_laeuft() {
        assert!(laeuft(std::process::id()));
        beendeter_prozess();
    }

    /// Grösse und Zeit aus dem Verzeichnis geben dasselbe ETag wie aus der
    /// offenen Datei, mit der der Server sendet, auch mit gesetzter Zeit.
    #[test]
    fn etag_aus_dem_verzeichnis_wie_aus_der_datei() {
        let dir = tempfile::tempdir().unwrap();
        let t = TileId { x: 0, y: 0 };
        std::fs::create_dir_all(dir.path().join("0/0")).unwrap();
        std::fs::write(tile_path(dir.path(), 0, t), b"frisch").unwrap();
        lege(
            dir.path(),
            0,
            TileId { x: 0, y: 1 },
            7,
            1_759_708_800_123_456_700,
        );
        let mut gesehen = 0;
        je_kachel(dir.path(), 0, None, |tile, eintrag| {
            let offen = File::open(tile_path(dir.path(), 0, tile)).unwrap();
            assert_eq!(
                etag(&eintrag.metadata().unwrap()).unwrap(),
                etag(&offen.metadata().unwrap()).unwrap()
            );
            gesehen += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(gesehen, 2);
    }

    /// Ein Lauf ohne Manifest, nach einem abgebrochenen oder neben einem
    /// laufenden liest den Baum ganz, auch wenn er nur wenig angefasst hat.
    /// Danach liegt seine Marke nicht mehr, und die eines laufenden bleibt.
    #[test]
    fn nach_einem_abbruch_ganz_gelesen() {
        let dir = tempfile::tempdir().unwrap();
        let t = TileId { x: 0, y: 0 };
        lege(dir.path(), 0, t, 10, 1_759_708_800_000_000_000);
        let lauf = || Lauf::beginne(dir.path(), true).unwrap().unwrap();
        let marken = || {
            let mut namen: Vec<String> = std::fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .map(|e| e.file_name().into_string().unwrap())
                .filter(|name| name.starts_with(OFFEN))
                .collect();
            namen.sort();
            namen
        };
        lauf().schliesse(Some(&BTreeSet::new())).unwrap();
        let pfad = dir.path().join(MANIFEST);
        let eins = entpackt(&std::fs::read(&pfad).unwrap());
        assert_eq!(eins.lines().count(), 1);
        assert!(marken().is_empty());

        // Ein Lauf schreibt eine Kachel und bricht ab, der nächste fasst
        // sie nicht an: Er liest ganz und räumt die verwaiste Marke weg.
        let abgebrochen = format!("{OFFEN}{}-1", beendeter_prozess());
        std::fs::write(dir.path().join(&abgebrochen), b"").unwrap();
        lege(dir.path(), 1, t, 10, 1_759_708_900_000_000_000);
        lauf().schliesse(Some(&BTreeSet::new())).unwrap();
        assert_eq!(
            entpackt(&std::fs::read(&pfad).unwrap()),
            entpackt(&ganz(dir.path()).unwrap().0)
        );
        assert!(marken().is_empty());

        // Neben einem, der noch läuft, liest er ganz und lässt dessen Marke.
        let laufend = format!("{OFFEN}{}-1", std::process::id());
        std::fs::write(dir.path().join(&laufend), b"").unwrap();
        lege(dir.path(), 2, t, 10, 1_759_709_000_000_000_000);
        lauf().schliesse(Some(&BTreeSet::new())).unwrap();
        assert_eq!(entpackt(&std::fs::read(&pfad).unwrap()).lines().count(), 3);
        assert_eq!(marken(), std::slice::from_ref(&laufend));
        std::fs::remove_file(dir.path().join(&laufend)).unwrap();

        // Ohne fremde Marke zieht er nur nach.
        lege(dir.path(), 3, t, 10, 1_759_709_100_000_000_000);
        lauf().schliesse(Some(&BTreeSet::new())).unwrap();
        assert_eq!(entpackt(&std::fs::read(&pfad).unwrap()).lines().count(), 3);

        // Ohne --manifest verschwindet es, und es gibt keinen Lauf.
        assert!(Lauf::beginne(dir.path(), false).unwrap().is_none());
        assert!(!pfad.exists());
        assert!(marken().is_empty());
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
