//! `--estimate`: Kacheln, Platz und Dauer eines Laufs, bevor er läuft.
//! Siehe docs/benutzung/kosten.md, „Schätzen: `--estimate`“.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use heroic_map_renderer::render::{Projection, Reach, ScreenRect, TILE};
use heroic_map_renderer::world::World;
use rayon::prelude::*;

use super::{Args, Y_RANGE, baum_name, melde_json, sekunden};

/// Kacheln je fertigem Chunk über `256 · oberseite / TILE²`: an den
/// gemessenen Vollrendern 1,013 bis 1,167.
const KACHELN_JE_FLAECHE: (f64, f64) = (1.0, 1.2);
/// Was native Stufen und Pyramide zusammen gegen die Basis wiegen: an den
/// gemessenen Läufen 32 bis 45 %.
const DARUEBER_BYTES: (f64, f64) = (0.30, 0.47);
/// Die Pyramide gegen die Zeit der Basis: gemessen 0,2 bis 2,3 %.
const PYRAMIDE_ZEIT: (f64, f64) = (0.0, 0.03);
/// Um so viel schwankt die Dauer von Tag zu Tag und mit der Last.
const DAUER_SPANNE: (f64, f64) = (0.8, 1.3);
/// So viele Ausschnitte rendert der Probelauf, über die Welt verteilt.
const PROBEN: usize = 4;
/// So viele Basiskacheln je Thread hat jeder Ausschnitt, damit alle Threads
/// zu tun haben wie im Lauf.
const KACHELN_JE_THREAD: f64 = 16.0;
/// So viele Chunks aus den Köpfen dekodiert die Schätzung höchstens, um den
/// Anteil der fertig erzeugten zu zählen.
const STICHPROBE: usize = 2000;

/// Was ein Ausschnitt des Probelaufs gemessen hat, aus seinen JSON-Zeilen.
#[derive(Default, Debug)]
struct Probe {
    chunks: f64,
    vorlauf_s: f64,
    kacheln: f64,
    basis_s: f64,
    stufen_s: f64,
    ganz_s: f64,
    /// Bytes und Zahl der geschriebenen Basiskacheln.
    basis_bytes: (u64, u64),
}

/// Schätzt den Lauf, den diese Schalter ohne `--estimate` machten, und
/// schreibt nichts unter `dir`.
pub(super) fn schaetze(
    world: &World,
    projection: Projection,
    args: &Args,
    bounds: Option<ScreenRect>,
    dir: &Path,
) -> Result<()> {
    let started = Instant::now();
    let reach = Reach::new(projection, Y_RANGE, bounds);
    let mut chunks = Vec::new();
    for (rx, rz) in world.regions()? {
        if !reach.region(rx, rz) {
            continue;
        }
        if let Some(mut region) = world.region(rx, rz)? {
            let vorhanden = region.vorhanden()?;
            chunks.extend(
                vorhanden
                    .into_iter()
                    .filter(|&(cx, cz)| reach.chunk(cx, cz)),
            );
        }
    }
    let n = chunks.len();
    println!(
        "\nSchätzung:  {n} Chunks in den Köpfen der Regionen, in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    if n == 0 {
        println!("            nichts zu zeichnen");
        melde_json(serde_json::json!({"phase": "estimate", "chunks": 0}));
        return Ok(());
    }

    // Nicht fertig erzeugte Chunks zeichnet der Lauf nicht. Sie liegen meist
    // am Rand der Welt; eine gleichmässige Stichprobe trifft sie, die
    // Ausschnitte des Probelaufs kaum.
    let schritt = n.div_ceil(STICHPROBE);
    let stichprobe: Vec<(i32, i32)> = chunks.iter().copied().step_by(schritt).collect();
    let erzeugt = stichprobe
        .par_iter()
        .map(|&(cx, cz)| {
            Ok(world
                .stored_chunk(cx, cz)?
                .is_some_and(|c| c.is_generated()))
        })
        .collect::<Result<Vec<bool>>>()?;
    // Die Ausschnitte beginnen an fertigen Chunks: Einer nur aus unfertigen
    // hätte keine Kachel.
    let starts: Vec<(i32, i32)> = stichprobe
        .iter()
        .zip(&erzeugt)
        .filter_map(|(&chunk, &ja)| ja.then_some(chunk))
        .collect();
    let fertig = starts.len() as f64 / erzeugt.len() as f64;
    let n_fertig = n as f64 * fertig;
    if starts.is_empty() {
        println!("            kein Chunk der Stichprobe ist fertig erzeugt, nichts zu zeichnen");
        melde_json(serde_json::json!({"phase": "estimate", "chunks": n, "finished": 0.0}));
        return Ok(());
    }

    // Kacheln je Chunk über die Fläche der Oberseite. Jeder Ausschnitt des
    // Probelaufs ist ein Quadrat aus ganzen Kacheln um einen fertigen Chunk,
    // mit einigen Kacheln für jeden Thread: Seine Kacheln sind voll wie im
    // Innern der Welt, nicht halb leer wie am Rand eines Rechtecks.
    let je_chunk = 256.0 * projection.oberseite() / f64::from(TILE * TILE);
    let threads = rayon::current_num_threads() as f64;
    let kante = ((KACHELN_JE_THREAD * threads).sqrt().ceil() as u32).clamp(2, 64);
    let wurzel = std::env::temp_dir().join(format!("heroic-estimate-{}", std::process::id()));
    let proben = (0..PROBEN)
        .map(|i| {
            let (cx, cz) = starts[(2 * i + 1) * starts.len() / (2 * PROBEN)];
            let ausschnitt = ([cx * 16 + 8, cz * 16 + 8], kante * TILE);
            let probe = probe(args, projection, &wurzel.join(i.to_string()), ausschnitt);
            let _ = std::fs::remove_dir_all(wurzel.join(i.to_string()));
            probe
        })
        .collect::<Result<Vec<Probe>>>();
    let _ = std::fs::remove_dir_all(&wurzel);
    let proben = proben?;
    let summe = |f: fn(&Probe) -> f64| proben.iter().map(f).sum::<f64>();

    let kacheln = (
        n_fertig * je_chunk * KACHELN_JE_FLAECHE.0,
        n_fertig * je_chunk * KACHELN_JE_FLAECHE.1,
    );
    let basis_bytes = proben.iter().map(|p| p.basis_bytes.0).sum::<u64>();
    let geschrieben = proben.iter().map(|p| p.basis_bytes.1).sum::<u64>();
    let je_kachel = if geschrieben > 0 {
        basis_bytes as f64 / geschrieben as f64
    } else {
        0.0
    };
    let bytes = (
        kacheln.0 * je_kachel * (1.0 + DARUEBER_BYTES.0),
        kacheln.1 * je_kachel * (1.0 + DARUEBER_BYTES.1),
    );
    let dateien = kacheln.1 * 4.0 / 3.0;

    let vorlauf = n_fertig * summe(|p| p.vorlauf_s) / summe(|p| p.chunks).max(1.0);
    let basis_s = summe(|p| p.basis_s);
    let je_kachel_s = basis_s / summe(|p| p.kacheln).max(1.0);
    let basis = (kacheln.0 * je_kachel_s, kacheln.1 * je_kachel_s);
    let stufen = summe(|p| p.stufen_s) / basis_s.max(1e-9);
    let fest = proben
        .iter()
        .map(|p| (p.ganz_s - p.vorlauf_s - p.basis_s - p.stufen_s).max(0.0))
        .fold(0.0, f64::max);
    let dauer = |basis: f64, pyramide: f64, faktor: f64| {
        (fest + vorlauf + basis * (1.0 + stufen + pyramide)) * faktor
    };
    let sekunden_spanne = (
        dauer(basis.0, PYRAMIDE_ZEIT.0, DAUER_SPANNE.0),
        dauer(basis.1, PYRAMIDE_ZEIT.1, DAUER_SPANNE.1),
    );
    let frei = freier_platz(dir);
    let reicht = frei.map(|frei| frei as f64 >= bytes.1);

    println!(
        "            {:.0} % fertig erzeugt in {} Chunks der Stichprobe; Probelauf: {PROBEN} Ausschnitte zu {kante} x {kante} Kacheln, zusammen in {:.1} s",
        fertig * 100.0,
        stichprobe.len(),
        started.elapsed().as_secs_f64()
    );
    println!(
        "            Basis {} bis {} Kacheln",
        rund(kacheln.0),
        rund(kacheln.1)
    );
    println!(
        "            Platz {} bis {}, rund {} Dateien",
        groesse(bytes.0),
        groesse(bytes.1),
        rund(dateien)
    );
    println!(
        "            Dauer {} bis {} mit {} Threads",
        zeit(sekunden_spanne.0),
        zeit(sekunden_spanne.1),
        threads
    );
    match (frei, reicht) {
        (Some(frei), Some(true)) => println!("            Frei {}, reicht", groesse(frei as f64)),
        (Some(frei), _) => println!("            Frei {}, reicht nicht", groesse(frei as f64)),
        _ => println!("            Frei: unbekannt"),
    }
    melde_json(serde_json::json!({
        "phase": "estimate",
        "chunks": n,
        "finished": (fertig * 1000.0).round() / 1000.0,
        "tiles": [kacheln.0.round() as u64, kacheln.1.round() as u64],
        "bytes": [bytes.0.round() as u64, bytes.1.round() as u64],
        "files": dateien.round() as u64,
        "s": [sekunden_spanne.0.round() as u64, sekunden_spanne.1.round() as u64],
        "free_bytes": frei,
        "enough": reicht,
        "probe_s": sekunden(started),
    }));
    Ok(())
}

/// Ein Ausschnitt des Probelaufs: Mitte in Blöcken und Kantenlänge in
/// Pixeln, wie `--center` und `--size`.
type Ausschnitt = ([i32; 2], u32);

/// Rendert einen Ausschnitt mit denselben Schaltern in `wurzel` und liest
/// aus seinen JSON-Zeilen und Kacheln, was er gekostet hat.
fn probe(
    args: &Args,
    projection: Projection,
    wurzel: &Path,
    ausschnitt: Ausschnitt,
) -> Result<Probe> {
    let ausgabe = Command::new(std::env::current_exe().context("eigenes Binär finden")?)
        .args(probe_schalter(args, wurzel, ausschnitt))
        .output()
        .context("Probelauf starten")?;
    // Ein fertiger Chunk ohne Block, etwa im End, gibt keine Kachel: Dann
    // zählt der Ausschnitt nichts.
    let fehler = String::from_utf8_lossy(&ausgabe.stderr);
    if !ausgabe.status.success() && fehler.contains("keine Kachel enthält etwas") {
        return Ok(Probe::default());
    }
    if !ausgabe.status.success() {
        bail!(
            "Probelauf fehlgeschlagen: {}",
            String::from_utf8_lossy(&ausgabe.stderr).trim()
        );
    }
    let mut probe = Probe::default();
    let zahl = |z: &serde_json::Value, feld: &str| z[feld].as_f64().unwrap_or(0.0);
    for zeile in String::from_utf8_lossy(&ausgabe.stdout)
        .lines()
        .filter(|z| z.starts_with('{'))
    {
        let z: serde_json::Value = serde_json::from_str(zeile).context("JSON des Probelaufs")?;
        match z["phase"].as_str() {
            Some("prepass") if z.get("chunks").is_some() => {
                probe.chunks = zahl(&z, "chunks");
                probe.vorlauf_s = zahl(&z, "s");
            }
            Some("base") => {
                probe.kacheln = zahl(&z, "tiles");
                probe.basis_s = probe.kacheln / zahl(&z, "rate").max(1e-3);
            }
            // Je Stufe nur ihre letzte Zeile; die Zeilen der Stufen kommen
            // gemischt, also über die Zahl ihrer Kacheln.
            Some("level") if z["tiles"] == z["of"] => {
                probe.stufen_s += zahl(&z, "tiles") / zahl(&z, "rate").max(1e-3);
            }
            Some("done") => probe.ganz_s = zahl(&z, "s"),
            _ => {}
        }
    }
    probe.basis_bytes = basis_bytes(&wurzel.join(baum_name(projection, args.cinematic)))?;
    Ok(probe)
}

/// Die Schalter des Probelaufs: dieselben wie für den Lauf, aber nur der
/// Ausschnitt, in `wurzel` und mit JSON.
fn probe_schalter(args: &Args, wurzel: &Path, ([x, z], groesse): Ausschnitt) -> Vec<OsString> {
    let mut s: Vec<OsString> = Vec::new();
    let mut paar = |schalter: &str, wert: OsString| {
        s.push(schalter.into());
        s.push(wert);
    };
    if let Some(world) = &args.world {
        paar("--world", world.into());
    }
    for pfad in &args.assets {
        paar("--assets", pfad.into());
    }
    for pfad in &args.data {
        paar("--data", pfad.into());
    }
    paar("--tiles", wurzel.into());
    paar("--camera", args.camera.to_string().into());
    if let Some(richtung) = &args.direction {
        paar("--direction", richtung.into());
    }
    if let Some(scale) = args.scale {
        paar("--scale", scale.to_string().into());
    }
    if let Some(blend) = args.biome_blend {
        paar("--biome-blend", blend.to_string().into());
    }
    if let Some(stufen) = args.native_levels {
        paar("--native-levels", stufen.to_string().into());
    }
    let gpu = args
        .gpu
        .to_possible_value()
        .expect("jeder Wert hat einen Namen");
    paar("--gpu", gpu.get_name().into());
    if let Some(threads) = args.threads {
        paar("--threads", threads.to_string().into());
    }
    paar("--progress", "json".into());
    if args.cinematic {
        s.push("--cinematic".into());
    }
    if args.low_priority {
        s.push("--low-priority".into());
    }
    s.push("--center".into());
    s.push(x.to_string().into());
    s.push(z.to_string().into());
    s.push("--size".into());
    s.push(groesse.to_string().into());
    s
}

/// Bytes und Zahl der Basiskacheln eines Baums.
fn basis_bytes(baum: &Path) -> Result<(u64, u64)> {
    let karte: serde_json::Value = serde_json::from_slice(
        &std::fs::read(baum.join("map.json")).context("map.json des Probelaufs")?,
    )?;
    let basis = baum.join(karte["maxZoom"].as_u64().unwrap_or(0).to_string());
    let (mut bytes, mut anzahl) = (0, 0);
    for spalte in std::fs::read_dir(&basis).with_context(|| format!("{} lesen", basis.display()))? {
        for datei in std::fs::read_dir(spalte?.path())? {
            bytes += datei?.metadata()?.len();
            anzahl += 1;
        }
    }
    Ok((bytes, anzahl))
}

/// Freier Platz für den Benutzer unter `pfad` oder dem nächsten Ordner
/// darüber, den es gibt.
fn freier_platz(pfad: &Path) -> Option<u64> {
    let ordner = pfad.ancestors().find(|p| p.is_dir())?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let breit: Vec<u16> = ordner.as_os_str().encode_wide().chain([0]).collect();
        let mut frei = 0u64;
        // SAFETY: `breit` endet mit 0 und lebt über den Aufruf; die
        // übrigen Ausgaben dürfen fehlen.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                breit.as_ptr(),
                &mut frei,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(frei)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let pfad = std::ffi::CString::new(ordner.as_os_str().as_bytes()).ok()?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `pfad` endet mit 0; statvfs füllt `stat` bei Erfolg ganz.
        if unsafe { libc::statvfs(pfad.as_ptr(), stat.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: oben gefüllt.
        let stat = unsafe { stat.assume_init() };
        Some(stat.f_bavail as u64 * stat.f_frsize as u64)
    }
}

/// Eine grosse Zahl auf zwei bis drei gültige Stellen gerundet.
fn rund(x: f64) -> String {
    let stellen = 10f64.powi((x.max(1.0).log10().floor() as i32 - 2).max(0));
    format!("{:.0}", (x / stellen).round() * stellen)
}

/// Bytes dezimal, wie die Doku sie zählt.
fn groesse(bytes: f64) -> String {
    match bytes {
        b if b >= 1e9 => format!("{:.1} GB", b / 1e9),
        b if b >= 1e6 => format!("{:.0} MB", b / 1e6),
        b => format!("{:.0} kB", b / 1e3),
    }
}

fn zeit(s: f64) -> String {
    match s {
        s if s >= 5400.0 => format!("{:.1} h", s / 3600.0),
        s if s >= 90.0 => format!("{:.0} min", s / 60.0),
        s => format!("{s:.0} s"),
    }
}
