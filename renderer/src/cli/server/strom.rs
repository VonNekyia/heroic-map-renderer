//! Der Download eines Baums als ein Strom: ein ZIP ohne Kompression mit
//! `map.json`, `manifest` und den Kacheln bis zur Stufe des Tokens, von grob
//! nach fein, so zeigt der Mod früh die ganze Welt; am Ende der leere
//! Eintrag [`ENDE`]. Je Eintrag nur ein lokaler Kopf mit Grösse und CRC-32,
//! ohne Data Descriptor und ohne zentrales Verzeichnis: Der Mod liest der
//! Reihe nach. Mehrere Threads lesen voraus, die Reihenfolge bleibt fest.
//! Siehe docs/benutzung/server.md, „Download als Strom“.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Instant;

use anyhow::{Context as _, Result};
use flate2::read::GzDecoder;
use hyper::body::{Body, Bytes, Frame};
use tokio::sync::mpsc;

use super::super::manifest::MANIFEST;

/// Der letzte, leere Eintrag: Nur mit ihm ist ein Strom vollständig. Ohne
/// zentrales Verzeichnis sähe ein Strom, der zwischen zwei Dateien abbricht,
/// sonst ganz aus.
pub(super) const ENDE: &str = "ende";

/// Bytes des lokalen Kopfs ohne den Namen.
const KOPF: u64 = 30;

/// Wie viele Stücke zwischen Leser und Verbindung warten dürfen; je Stück
/// eine Datei.
const PUFFER: usize = 16;

/// Threads, die je Strom voraus lesen. Kalte Kacheln kosten je Datei
/// Millisekunden, die nicht auf der Platte warten; mehrere zugleich teilen sie.
const LESER: usize = 8;

/// Wie viele Dateien höchstens gelesen, aber noch nicht gesendet sind; hält
/// den Speicher bei wenigen MB.
const FENSTER: usize = 64;

/// Eine Kachel als `z`, `x`, `y`.
pub(super) type Lage = (u32, i32, i32);

/// Eine Datei im Strom: ihr Name im ZIP und wo sie liegt.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Eintrag {
    pub name: String,
    pub pfad: PathBuf,
}

/// Was in den Strom eines Baums kommt, in fester Reihenfolge, und die
/// erwartete Grösse des ZIP: `map.json`, `manifest`, dann die Kacheln des
/// Manifests bis `stufe`, von grob nach fein, innerhalb einer Stufe wie im
/// Manifest. Mit `nach` nur die Kacheln nach dieser; steht sie nicht darin,
/// alle. Die Grösse der Kacheln ist die aus dem Manifest.
pub(super) fn plan(baum: &Path, stufe: u8, nach: Option<Lage>) -> Result<(Vec<Eintrag>, u64)> {
    let karte = baum.join("map.json");
    let manifest = baum.join(MANIFEST);
    let n_karte = std::fs::metadata(&karte)
        .with_context(|| format!("{} lesen", karte.display()))?
        .len();
    let roh = std::fs::read(&manifest).with_context(|| format!("{} lesen", manifest.display()))?;
    let mut kacheln = Vec::new();
    for zeile in BufReader::new(GzDecoder::new(&roh[..])).lines() {
        let zeile = zeile.with_context(|| format!("{} lesen", manifest.display()))?;
        if let Some((lage, n)) = kachel(&zeile)
            && lage.0 <= u32::from(stufe)
        {
            kacheln.push((lage, n));
        }
    }
    kacheln.sort_by_key(|&((z, ..), _)| z);
    if let Some(i) = nach.and_then(|nach| kacheln.iter().position(|&(lage, _)| lage == nach)) {
        kacheln.drain(..=i);
    }

    let groesse = |name: &str, n: u64| KOPF + name.len() as u64 + n;
    let mut erwartet =
        groesse("map.json", n_karte) + groesse(MANIFEST, roh.len() as u64) + groesse(ENDE, 0);
    let mut liste = vec![
        Eintrag {
            name: "map.json".into(),
            pfad: karte,
        },
        Eintrag {
            name: MANIFEST.into(),
            pfad: manifest,
        },
    ];
    for ((z, x, y), n) in kacheln {
        let name = format!("{z}/{x}/{y}.webp");
        erwartet += groesse(&name, n);
        liste.push(Eintrag {
            pfad: baum
                .join(z.to_string())
                .join(x.to_string())
                .join(format!("{y}.webp")),
            name,
        });
    }
    Ok((liste, erwartet))
}

/// `z/x/y` aus `?nach=`, ganze Zahlen ohne Vorzeichen bei `z`; `None` für
/// alles andere.
pub(super) fn lage(text: &str) -> Option<Lage> {
    let mut teile = text.split('/');
    let z = teile.next()?.parse().ok()?;
    let x = teile.next()?.parse().ok()?;
    let y = teile.next()?.parse().ok()?;
    teile.next().is_none().then_some((z, x, y))
}

/// Eine Zeile des Manifests, `z/x/y Grösse ETag`, als Zahlen; `None` für
/// alles andere. Der Name im ZIP entsteht neu aus den Zahlen.
fn kachel(zeile: &str) -> Option<(Lage, u64)> {
    let mut teile = zeile.split(' ');
    let lage = lage(teile.next()?)?;
    let n = teile.next()?.parse().ok()?;
    Some((lage, n))
}

/// Ein Eintrag des ZIP: der lokale Kopf, stored, ohne Extra-Feld, mit fester
/// Zeit 1980-01-01 00:00, dann der Inhalt. `None`, wenn der Inhalt nicht in
/// 32 Bit passt.
fn eintrag(name: &str, daten: &[u8]) -> Option<Vec<u8>> {
    let n = u32::try_from(daten.len()).ok()?;
    let mut aus = Vec::with_capacity(KOPF as usize + name.len() + daten.len());
    aus.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    aus.extend_from_slice(&10u16.to_le_bytes()); // Version 1.0 reicht für stored
    aus.extend_from_slice(&0u16.to_le_bytes()); // Flags
    aus.extend_from_slice(&0u16.to_le_bytes()); // stored
    aus.extend_from_slice(&0u16.to_le_bytes()); // Zeit
    aus.extend_from_slice(&0x0021u16.to_le_bytes()); // 1980-01-01
    aus.extend_from_slice(&crc32fast::hash(daten).to_le_bytes());
    aus.extend_from_slice(&n.to_le_bytes());
    aus.extend_from_slice(&n.to_le_bytes());
    aus.extend_from_slice(&(name.len() as u16).to_le_bytes());
    aus.extend_from_slice(&0u16.to_le_bytes());
    aus.extend_from_slice(name.as_bytes());
    aus.extend_from_slice(daten);
    Some(aus)
}

/// Wie ein Strom endete.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Ende {
    Fertig,
    /// Der Deckel des Tokens wäre überschritten.
    Deckel,
    /// Die Verbindung ist weg.
    Weg,
    Fehler(String),
}

/// Was ein Strom schickte.
#[derive(Debug)]
pub(super) struct Bericht {
    pub dateien: usize,
    pub bytes: u64,
    pub ende: Ende,
}

/// Ein Stück des Körpers für die Verbindung; ein Fehler bricht sie ab, ohne
/// das letzte Stück des chunked-Körpers.
pub(super) type Stueck = std::io::Result<Frame<Bytes>>;

/// Eine gelesene Datei: `None`, wenn es sie nicht mehr gibt.
type Gelesen = std::io::Result<Option<Vec<u8>>>;

/// Schreibt den Strom in `aus`. [`LESER`] Threads lesen die Dateien voraus,
/// höchstens [`FENSTER`] vor dem Schreiben; geschrieben wird in der
/// Reihenfolge von `eintraege`: buchen, dann Kopf und Inhalt als ein Stück.
/// Eine Datei, die inzwischen fehlt, fällt weg. Am Ende [`ENDE`]. Blockiert.
pub(super) fn schreibe(
    eintraege: &[Eintrag],
    aus: &mpsc::Sender<Stueck>,
    mut buche: impl FnMut(u64) -> bool,
) -> Bericht {
    let mut bericht = Bericht {
        dateien: 0,
        bytes: 0,
        ende: Ende::Fertig,
    };
    let naechste = AtomicUsize::new(0);
    // Je Marke darf ein Leser eine Datei lesen; das Schreiben gibt sie zurück.
    let (marke_zurueck, marken) = std::sync::mpsc::sync_channel::<()>(FENSTER);
    for _ in 0..FENSTER {
        let _ = marke_zurueck.send(());
    }
    let marken = Mutex::new(marken);
    let (fertig, gelesen) = std::sync::mpsc::channel::<(usize, Gelesen)>();
    std::thread::scope(|s| {
        for _ in 0..LESER.min(eintraege.len()) {
            let fertig = fertig.clone();
            let (marken, naechste) = (&marken, &naechste);
            s.spawn(move || {
                loop {
                    let marke = marken.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let i = naechste.fetch_add(1, Ordering::Relaxed);
                    if marke.is_err() || i >= eintraege.len() {
                        return;
                    }
                    if fertig.send((i, lies(&eintraege[i].pfad))).is_err() {
                        return;
                    }
                }
            });
        }
        drop(fertig);
        let mut warten = BTreeMap::new();
        for (i, e) in eintraege.iter().enumerate() {
            let daten = loop {
                if let Some(daten) = warten.remove(&i) {
                    break Some(daten);
                }
                match gelesen.recv() {
                    Ok((j, daten)) => {
                        warten.insert(j, daten);
                    }
                    Err(_) => break None,
                }
            };
            let stueck = match daten {
                Some(Ok(Some(daten))) => {
                    if buche(daten.len() as u64) {
                        eintrag(&e.name, &daten)
                            .ok_or_else(|| Ende::Fehler(format!("{} zu gross", e.name)))
                    } else {
                        Err(Ende::Deckel)
                    }
                }
                Some(Ok(None)) => {
                    let _ = marke_zurueck.send(());
                    continue;
                }
                Some(Err(fehler)) => Err(Ende::Fehler(format!("{} lesen: {fehler}", e.name))),
                None => Err(Ende::Fehler("Leser beendet".into())),
            };
            let stueck = match stueck {
                Ok(stueck) => stueck,
                Err(ende) => {
                    let _ = aus.blocking_send(Err(std::io::Error::other(format!("{ende:?}"))));
                    bericht.ende = ende;
                    break;
                }
            };
            bericht.bytes += stueck.len() as u64;
            if aus
                .blocking_send(Ok(Frame::data(Bytes::from(stueck))))
                .is_err()
            {
                bericht.ende = Ende::Weg;
                break;
            }
            bericht.dateien += 1;
            let _ = marke_zurueck.send(());
        }
        // Wartende Leser enden, sobald es keine Marken mehr gibt.
        drop(marke_zurueck);
        drop(gelesen);
    });
    if bericht.ende == Ende::Fertig {
        let ende = eintrag(ENDE, &[]).unwrap_or_default();
        if aus
            .blocking_send(Ok(Frame::data(Bytes::from(ende))))
            .is_err()
        {
            bericht.ende = Ende::Weg;
        } else {
            bericht.bytes += KOPF + ENDE.len() as u64;
        }
    }
    bericht
}

/// Eine Datei ganz; `None`, wenn es sie nicht gibt.
fn lies(pfad: &Path) -> Gelesen {
    match std::fs::read(pfad) {
        Ok(daten) => Ok(Some(daten)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Startet den Strom auf eigenen Threads, nicht im Pool von tokio: Mit
/// `--threads 1` hätte der nur einen zum Lesen, und ein langer Download
/// hielte die Webkarte an. `fertig` bekommt den Bericht und die Dauer.
pub(super) fn starte(
    eintraege: Vec<Eintrag>,
    buche: impl FnMut(u64) -> bool + Send + 'static,
    fertig: impl FnOnce(Bericht, std::time::Duration) + Send + 'static,
) -> std::io::Result<Kanal> {
    let (tx, rx) = mpsc::channel(PUFFER);
    std::thread::Builder::new()
        .name("download-strom".into())
        .spawn(move || {
            let beginn = Instant::now();
            let bericht = schreibe(&eintraege, &tx, buche);
            fertig(bericht, beginn.elapsed());
        })?;
    Ok(Kanal(rx))
}

/// Der Körper der Antwort: die Stücke des Schreibers, wie sie kommen.
pub(super) struct Kanal(mpsc::Receiver<Stueck>);

impl Body for Kanal {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        self.0.poll_recv(cx)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    /// Die Einträge eines Stroms wie ein Leser der Reihe nach: Name und
    /// Inhalt, CRC geprüft; `true`, wenn er mit [`ENDE`] schliesst und danach
    /// nichts mehr kommt.
    pub(crate) fn auspacken(zip: &[u8]) -> (Vec<(String, Vec<u8>)>, bool) {
        let mut aus = Vec::new();
        let mut i = 0;
        let zahl = |a: usize, n: usize| {
            zip[a..a + n]
                .iter()
                .rev()
                .fold(0u64, |s, &b| s << 8 | u64::from(b))
        };
        while i + KOPF as usize <= zip.len() {
            assert_eq!(zahl(i, 4), 0x0403_4b50, "Signatur bei {i}");
            assert_eq!(
                (zahl(i + 4, 2), zahl(i + 6, 2), zahl(i + 8, 2)),
                (10, 0, 0),
                "Version, Flags, stored"
            );
            assert_eq!(zahl(i + 28, 2), 0, "kein Extra-Feld");
            let (crc, n, m) = (
                zahl(i + 14, 4),
                zahl(i + 18, 4) as usize,
                zahl(i + 22, 4) as usize,
            );
            assert_eq!(n, m, "stored: beide Grössen gleich");
            let laenge = zahl(i + 26, 2) as usize;
            let name = String::from_utf8(zip[i + 30..i + 30 + laenge].to_vec()).unwrap();
            let inhalt = zip[i + 30 + laenge..i + 30 + laenge + n].to_vec();
            assert_eq!(u64::from(crc32fast::hash(&inhalt)), crc, "CRC von {name}");
            i += 30 + laenge + n;
            if name == ENDE {
                return (aus, i == zip.len());
            }
            aus.push((name, inhalt));
        }
        (aus, false)
    }

    /// Ein Baum mit `map.json`, Manifest in dieser Reihenfolge und den Kacheln.
    pub(crate) fn baum(dir: &Path, kacheln: &[(&str, &[u8])]) {
        std::fs::write(dir.join("map.json"), b"{\"maxZoom\":2}").unwrap();
        let mut gz = GzEncoder::new(Vec::new(), Compression::default());
        for (lage, inhalt) in kacheln {
            let pfad = dir.join(format!("{lage}.webp"));
            std::fs::create_dir_all(pfad.parent().unwrap()).unwrap();
            std::fs::write(&pfad, inhalt).unwrap();
            writeln!(gz, "{lage} {} \"e\"", inhalt.len()).unwrap();
        }
        std::fs::write(dir.join(MANIFEST), gz.finish().unwrap()).unwrap();
    }

    /// Schreibt den Strom wie die Verbindung ihn bekäme: die Bytes, der Grund
    /// eines Abbruchs und der Bericht.
    pub(crate) fn strom(
        eintraege: &[Eintrag],
        buche: impl FnMut(u64) -> bool + Send,
    ) -> (Vec<u8>, Option<String>, Bericht) {
        std::thread::scope(|s| {
            let (tx, mut rx) = mpsc::channel(PUFFER);
            let schreiber = s.spawn(move || schreibe(eintraege, &tx, buche));
            let (mut bytes, mut fehler) = (Vec::new(), None);
            while let Some(stueck) = rx.blocking_recv() {
                match stueck {
                    Ok(frame) => bytes.extend_from_slice(&frame.into_data().unwrap()),
                    Err(e) => fehler = Some(e.to_string()),
                }
            }
            (bytes, fehler, schreiber.join().unwrap())
        })
    }

    const KACHELN: [(&str, &[u8]); 5] = [
        ("2/0/0", b"zwei-a"),
        ("0/0/0", b"null"),
        ("1/0/1", b"eins-b"),
        ("1/0/0", b"eins-a"),
        ("2/-1/3", b"zwei-b"),
    ];

    fn namen(plan: &[Eintrag]) -> Vec<&str> {
        plan.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn von_grob_nach_fein_bis_zur_stufe() {
        let dir = tempfile::tempdir().unwrap();
        baum(dir.path(), &KACHELN);
        let (plan, erwartet) = super::plan(dir.path(), 1, None).unwrap();
        assert_eq!(
            namen(&plan),
            [
                "map.json",
                MANIFEST,
                "0/0/0.webp",
                "1/0/1.webp",
                "1/0/0.webp"
            ]
        );
        let (zip, fehler, bericht) = strom(&plan, |_| true);
        assert_eq!(
            (fehler, &bericht.ende, bericht.dateien),
            (None, &Ende::Fertig, 5)
        );
        assert_eq!(
            erwartet,
            zip.len() as u64,
            "X-Heroic-Bytes, solange nichts sich ändert"
        );
        assert_eq!(bericht.bytes, zip.len() as u64);
        let (eintraege, ganz) = auspacken(&zip);
        assert!(ganz, "endet mit {ENDE}");
        assert_eq!(eintraege[2], ("0/0/0.webp".to_string(), b"null".to_vec()));
        assert_eq!(
            eintraege[0].1,
            std::fs::read(dir.path().join("map.json")).unwrap()
        );
        assert_eq!(
            eintraege[1].1,
            std::fs::read(dir.path().join(MANIFEST)).unwrap(),
            "die gzip-Bytes"
        );
        for (name, _) in &eintraege {
            assert!(
                !name.contains("..") && !name.starts_with('/') && !name.contains('\\'),
                "{name}"
            );
        }
    }

    #[test]
    fn nach_setzt_hinter_der_kachel_fort() {
        let dir = tempfile::tempdir().unwrap();
        baum(dir.path(), &KACHELN);
        let (plan, erwartet) = super::plan(dir.path(), 2, Some((1, 0, 1))).unwrap();
        assert_eq!(
            namen(&plan),
            [
                "map.json",
                MANIFEST,
                "1/0/0.webp",
                "2/0/0.webp",
                "2/-1/3.webp"
            ]
        );
        let gebucht = std::sync::atomic::AtomicU64::new(0);
        let (zip, _, _) = strom(&plan, |n| {
            gebucht.fetch_add(n, Ordering::Relaxed);
            true
        });
        assert_eq!(erwartet, zip.len() as u64, "X-Heroic-Bytes nennt den Rest");
        let rest: u64 = [&b"eins-a"[..], b"zwei-a", b"zwei-b"]
            .iter()
            .map(|b| b.len() as u64)
            .sum();
        let vorn = (std::fs::metadata(dir.path().join("map.json"))
            .unwrap()
            .len()
            + std::fs::metadata(dir.path().join(MANIFEST)).unwrap().len())
            as u64;
        assert_eq!(gebucht.into_inner(), vorn + rest, "gebucht nur, was ging");
        let (unbekannt, _) = super::plan(dir.path(), 2, Some((7, 7, 7))).unwrap();
        assert_eq!(
            unbekannt.len(),
            7,
            "eine Kachel, die das Manifest nicht nennt: alle"
        );
        let (letzte, _) = super::plan(dir.path(), 2, Some((2, -1, 3))).unwrap();
        assert_eq!(
            namen(&letzte),
            ["map.json", MANIFEST],
            "nach der letzten nur die beiden"
        );
    }

    #[test]
    fn eine_geaenderte_oder_fehlende_datei_bleibt_gueltig() {
        let dir = tempfile::tempdir().unwrap();
        baum(dir.path(), &KACHELN);
        let (plan, _) = super::plan(dir.path(), 2, None).unwrap();
        std::fs::write(dir.path().join("1/0/1.webp"), b"inzwischen neu und laenger").unwrap();
        std::fs::remove_file(dir.path().join("2/0/0.webp")).unwrap();
        let (zip, fehler, bericht) = strom(&plan, |_| true);
        assert_eq!((fehler, bericht.ende), (None, Ende::Fertig));
        let (eintraege, ganz) = auspacken(&zip);
        assert!(ganz);
        let namen: Vec<&str> = eintraege.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            namen,
            [
                "map.json",
                MANIFEST,
                "0/0/0.webp",
                "1/0/1.webp",
                "1/0/0.webp",
                "2/-1/3.webp"
            ]
        );
        assert_eq!(
            eintraege[3].1, b"inzwischen neu und laenger",
            "Grösse und CRC der neuen Datei"
        );
    }

    #[test]
    fn am_deckel_bricht_er_ab_ohne_ende() {
        let dir = tempfile::tempdir().unwrap();
        baum(dir.path(), &KACHELN);
        let (plan, _) = super::plan(dir.path(), 2, None).unwrap();
        let mut frei = std::fs::metadata(dir.path().join("map.json"))
            .unwrap()
            .len()
            + std::fs::metadata(dir.path().join(MANIFEST)).unwrap().len()
            + 4;
        let (zip, fehler, bericht) = strom(&plan, move |n| {
            frei.checked_sub(n).map(|rest| frei = rest).is_some()
        });
        assert_eq!(bericht.ende, Ende::Deckel);
        assert!(fehler.is_some(), "die Verbindung bricht ab");
        let (eintraege, ganz) = auspacken(&zip);
        assert!(!ganz, "ohne {ENDE}");
        assert_eq!(
            eintraege.len(),
            3,
            "map.json, manifest und die erste Kachel"
        );
    }

    #[test]
    fn viele_dateien_in_fester_reihenfolge() {
        let dir = tempfile::tempdir().unwrap();
        let inhalte: Vec<(String, Vec<u8>)> = (0..500)
            .map(|i| {
                (
                    format!("3/{i}/0"),
                    format!("kachel {i}").repeat(i % 7 + 1).into_bytes(),
                )
            })
            .collect();
        let kacheln: Vec<(&str, &[u8])> = inhalte
            .iter()
            .map(|(l, b)| (l.as_str(), b.as_slice()))
            .collect();
        baum(dir.path(), &kacheln);
        let (plan, _) = super::plan(dir.path(), 3, None).unwrap();
        let (zip, fehler, _) = strom(&plan, |_| true);
        assert_eq!(fehler, None);
        let (eintraege, ganz) = auspacken(&zip);
        assert!(ganz);
        for (i, (name, inhalt)) in eintraege[2..].iter().enumerate() {
            assert_eq!(name, &format!("3/{i}/0.webp"));
            assert_eq!(inhalt, &inhalte[i].1);
        }
    }

    #[test]
    fn zeilen_des_manifests_und_nach() {
        assert_eq!(kachel("3/-2/5 1200 \"4b0-1\""), Some(((3, -2, 5), 1200)));
        for falsch in [
            "",
            "3/-2 12 \"e\"",
            "3/-2/5/1 12 \"e\"",
            "../1/2 12 \"e\"",
            "a/b/c 1 \"e\"",
            "3/1/2 x \"e\"",
            "-1/0/0 1 \"e\"",
        ] {
            assert_eq!(kachel(falsch), None, "{falsch}");
        }
        assert_eq!(lage("8/-120/77"), Some((8, -120, 77)));
        assert_eq!(lage("8/1/2.webp"), None);
    }
}
