//! `--estimate`: Kacheln, Platz und Dauer eines Laufs, bevor er läuft.
//! Siehe docs/benutzung/kosten.md, „Schätzen: `--estimate`“.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use heroic_map_renderer::render::{
    Gebiet, Projection, Reach, ScreenRect, TILE, corner_tiles, pyramid, world_box,
};
use heroic_map_renderer::world::World;
use rayon::prelude::*;

use super::{
    Args, Y_RANGE, baum_name, lies_bestand, melde_json, mischung, native_stufen, rechteck, sekunden,
};

// Die Faktoren der Eichung, je `(unten, oben)`. Woher jeder kommt, steht in
// docs/benutzung/kosten.md, „Schätzen: `--estimate`“.

/// Kacheln je fertigem Chunk über `256 · oberseite / TILE²`; unter scale 8
/// nach oben weiter, dort runden die Ränder der Welt auf mehr Kacheln.
const KACHELN_JE_FLAECHE: (f64, f64) = (1.0, 1.2);
const KACHELN_JE_FLAECHE_UNTER_8: (f64, f64) = (1.0, 1.3);
/// Native Stufen und Pyramide zusammen gegen die Basis, in Bytes.
const DARUEBER_BYTES: (f64, f64) = (0.30, 0.47);
/// Bytes je Basiskachel gegen den Schnitt der Proben.
const BYTES_JE_KACHEL: (f64, f64) = (0.8, 1.0);
/// Der Vorlauf je Chunk gegen das Dekodieren der Stichprobe.
const VORLAUF_JE_DEKODIERTEM: (f64, f64) = (1.1, 1.6);
/// Die Pyramide gegen die Zeit der Basis.
const PYRAMIDE_ZEIT: (f64, f64) = (0.0, 0.03);
/// Die ganze Dauer.
const DAUER_SPANNE: (f64, f64) = (0.8, 1.5);
/// Um so viele Mitten rendert der Probelauf, über die Welt verteilt, je
/// einen kleinen und einen doppelt so breiten Ausschnitt.
const PROBEN: usize = 4;
/// So viele Basiskacheln je Thread hat der kleine Ausschnitt, der grosse
/// das Vierfache. Mit weniger dauert die Basis eines Ausschnitts bei vielen
/// Threads nur Zehntelsekunden, und kurze fremde Last verschiebt die Spanne.
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

/// Was der Lauf aus einem bestehenden Baum übernähme, für die Proben.
struct Einstellung {
    stufen: u32,
    blend: u8,
    /// Das Rechteck der Welt in Chunks, wie [`World::bereich`].
    bereich: Option<[i32; 4]>,
}

/// Ein Ordner, der beim Verlassen wegfällt, auch nach einem Fehler.
struct Wegwerf(PathBuf);

impl Drop for Wegwerf {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Schätzt den Lauf, den diese Schalter ohne `--estimate` unter `wurzel`
/// machten, und schreibt dort nichts.
pub(super) fn schaetze(
    world: &World,
    projection: Projection,
    args: &Args,
    bounds: Option<ScreenRect>,
    wurzel: &Path,
) -> Result<()> {
    let started = Instant::now();
    // Wie der Lauf: native Stufen, Mischung und Rechteck kommen aus einem
    // bestehenden Baum, und weicht der Aufruf davon ab, bricht es ab.
    let baum = wurzel.join(baum_name(projection, args.cinematic));
    let bestand = lies_bestand(&baum)?;
    let fest = rechteck(&baum, bestand.as_ref(), world.bereich())?;
    let world = &world.clone().mit_bereich(fest);
    let Some(welt) = world_box(world, projection, Y_RANGE)? else {
        println!("\nSchätzung:  keine Regionsdatei, nichts zu zeichnen");
        melde_json(serde_json::json!({"phase": "estimate", "chunks": 0}));
        return Ok(());
    };
    let max_zoom = match &bestand {
        Some(alt) => alt.max_zoom,
        None => pyramid::depth(&corner_tiles(welt)),
    };
    let einstellung = Einstellung {
        stufen: native_stufen(
            &baum,
            bestand.as_ref(),
            args.native_levels,
            projection,
            max_zoom,
        )?,
        blend: mischung(&baum, bestand.as_ref(), args.biome_blend)?,
        bereich: fest,
    };

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
    // am Rand der Welt; jeder k-te Chunk der nach Regionen geordneten Liste
    // trifft sie, die Ausschnitte des Probelaufs kaum.
    let schritt = n.div_ceil(STICHPROBE);
    let stichprobe: Vec<(i32, i32)> = chunks.iter().copied().step_by(schritt).collect();
    let dekodiert = Instant::now();
    let erzeugt = stichprobe
        .par_iter()
        .map(|&(cx, cz)| {
            Ok(world
                .stored_chunk(cx, cz)?
                .is_some_and(|c| c.is_generated()))
        })
        .collect::<Result<Vec<bool>>>()?;
    let je_chunk_dekodiert = dekodiert.elapsed().as_secs_f64() / stichprobe.len() as f64;
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

    // Kacheln je Chunk über die Fläche der Oberseite. Mit einem Fenster
    // zählen die Chunks jede Säule, die es berührt, auch unter ihm; dann
    // gelten höchstens seine Kacheln, gerundet wie im Lauf.
    let je_chunk = 256.0 * projection.oberseite() / f64::from(TILE * TILE);
    // Das Gebiet zählt Kacheln der gröbsten nativen Stufe, je 4^Stufen der Basis.
    let fenster = bounds.map(|rect| {
        let gebiet = Gebiet::rechteck(rect, einstellung.stufen);
        (gebiet.kacheln().len() << (2 * einstellung.stufen)) as f64
    });
    let faktor = if projection.scale() < 8 {
        KACHELN_JE_FLAECHE_UNTER_8
    } else {
        KACHELN_JE_FLAECHE
    };
    let flaeche = (
        n_fertig * je_chunk * faktor.0,
        n_fertig * je_chunk * faktor.1,
    );
    // Das Fenster ist die genaue obere Grenze; mehr plant kein Lauf.
    let kacheln = match fenster {
        Some(f) => (flaeche.0.min(f), f),
        None => flaeche,
    };

    // Jeder Ausschnitt des Probelaufs ist ein Quadrat aus ganzen Kacheln um
    // einen fertigen Chunk: Seine Kacheln sind voll wie im Innern der Welt,
    // nicht halb leer wie am Rand eines Rechtecks. Je Mitte einer mit Kante k
    // und einer mit 2k, siehe `basis_je_kachel_und_chunk`. Zusammen höchstens
    // ein Zwanzigstel der Kacheln der Welt: Je Mitte sind es 5 k².
    let threads = rayon::current_num_threads() as f64;
    let deckel = (kacheln.1 / (20.0 * 5.0 * PROBEN as f64)).sqrt();
    let kante = ((KACHELN_JE_THREAD * threads).sqrt().min(deckel).ceil() as u32).clamp(4, 32);
    // Neben --tiles, auf demselben Laufwerk: Dort schreibt der Lauf, mit
    // demselben Echtzeitschutz.
    let wurzel_ganz = std::path::absolute(wurzel).context("Pfad von --tiles")?;
    let neben = wurzel_ganz.parent().unwrap_or(&wurzel_ganz);
    let wegwerf = Wegwerf(neben.join(format!(".heroic-estimate-{}", std::process::id())));
    let proben = (0..2 * PROBEN)
        .map(|i| {
            let (cx, cz) = starts[(2 * (i / 2) + 1) * starts.len() / (2 * PROBEN)];
            let groesse = (1 + i as u32 % 2) * kante * TILE;
            let ausschnitt = ([cx * 16 + 8, cz * 16 + 8], groesse);
            let ordner = wegwerf.0.join(i.to_string());
            let probe = probe(args, &einstellung, projection, &ordner, ausschnitt);
            let _ = std::fs::remove_dir_all(&ordner);
            probe
        })
        .collect::<Result<Vec<Probe>>>()?;
    drop(wegwerf);
    let summe = |f: fn(&Probe) -> f64| proben.iter().map(f).sum::<f64>();
    // Ohne eine Kachel im Probelauf, etwa im End, gibt es nichts zu messen.
    let gemessen = summe(|p| p.kacheln) > 0.0;

    let basis_bytes = proben.iter().map(|p| p.basis_bytes.0).sum::<u64>();
    let geschrieben = proben.iter().map(|p| p.basis_bytes.1).sum::<u64>();
    let je_kachel = basis_bytes as f64 / geschrieben.max(1) as f64;
    let bytes = gemessen.then_some((
        kacheln.0 * je_kachel * BYTES_JE_KACHEL.0 * (1.0 + DARUEBER_BYTES.0),
        kacheln.1 * je_kachel * BYTES_JE_KACHEL.1 * (1.0 + DARUEBER_BYTES.1),
    ));
    // Jede Stufe darüber hat ein Viertel der Kacheln: zusammen ein Drittel.
    let dateien = (kacheln.0 * 4.0 / 3.0, kacheln.1 * 4.0 / 3.0);

    // Der Vorlauf liest jeden Chunk aus den Köpfen, auch die unfertigen, über
    // alle Regionen verteilt. Der eines Ausschnitts berührt nur wenige
    // Regionen und läuft kaum parallel; darum zählt die Stichprobe.
    let vorlauf = (
        n as f64 * je_chunk_dekodiert * VORLAUF_JE_DEKODIERTEM.0,
        n as f64 * je_chunk_dekodiert * VORLAUF_JE_DEKODIERTEM.1,
    );
    let basis_s = summe(|p| p.basis_s);
    let (je_kachel_s, je_chunk_s) = basis_je_kachel_und_chunk(&proben);
    let basis = (
        kacheln.0 * je_kachel_s + n_fertig * je_chunk_s,
        kacheln.1 * je_kachel_s + n_fertig * je_chunk_s,
    );
    let stufen = summe(|p| p.stufen_s) / basis_s.max(1e-9);
    let fest = proben
        .iter()
        .map(|p| (p.ganz_s - p.vorlauf_s - p.basis_s - p.stufen_s).max(0.0))
        .fold(0.0, f64::max);
    let dauer = |vorlauf: f64, basis: f64, pyramide: f64, faktor: f64| {
        (fest + vorlauf + basis * (1.0 + stufen + pyramide)) * faktor
    };
    let sekunden_spanne = gemessen.then(|| {
        (
            dauer(vorlauf.0, basis.0, PYRAMIDE_ZEIT.0, DAUER_SPANNE.0),
            dauer(vorlauf.1, basis.1, PYRAMIDE_ZEIT.1, DAUER_SPANNE.1),
        )
    });

    // Ein Lauf über einen bestehenden Baum überschreibt ihn: Frei sein muss
    // nur, was dazukommt.
    let gezaehlt = Instant::now();
    let (bestand_bytes, bestand_dateien) = if baum.is_dir() {
        bytes_unter(&baum)
    } else {
        (0, 0)
    };
    let gezaehlt = gezaehlt.elapsed().as_secs_f64();
    let frei = freier_platz(&baum);
    let reicht = frei
        .zip(bytes)
        .map(|(frei, (_, oben))| (frei + bestand_bytes) as f64 >= oben);

    println!(
        "            {:.0} % fertig erzeugt in {} Chunks der Stichprobe; Probelauf: {PROBEN} Ausschnitte zu {kante} x {kante} und {PROBEN} zu {} x {} Kacheln, zusammen in {:.1} s",
        fertig * 100.0,
        stichprobe.len(),
        2 * kante,
        2 * kante,
        started.elapsed().as_secs_f64()
    );
    println!(
        "            Basis {} bis {} Kacheln, {} native Stufen",
        rund(kacheln.0),
        rund(kacheln.1),
        einstellung.stufen
    );
    match bytes {
        Some((unten, oben)) => println!(
            "            Platz {} bis {}, {} bis {} Dateien",
            groesse(unten),
            groesse(oben),
            rund(dateien.0),
            rund(dateien.1)
        ),
        None => println!("            Platz unbekannt, der Probelauf zeichnete nichts"),
    }
    if bestand_dateien > 0 {
        println!(
            "            Bestand {} in {} Dateien, den der Lauf überschreibt, gezählt in {gezaehlt:.3} s",
            groesse(bestand_bytes as f64),
            rund(bestand_dateien as f64)
        );
    }
    match sekunden_spanne {
        Some((unten, oben)) => println!(
            "            Dauer {} bis {} mit {} Threads",
            zeit(unten),
            zeit(oben),
            threads
        ),
        None => println!("            Dauer unbekannt"),
    }
    match (frei, reicht) {
        (Some(frei), Some(true)) => println!("            Frei {}, reicht", groesse(frei as f64)),
        (Some(frei), Some(false)) => {
            println!("            Frei {}, reicht nicht", groesse(frei as f64))
        }
        (Some(frei), None) => println!("            Frei {}", groesse(frei as f64)),
        (None, _) => println!("            Frei: unbekannt"),
    }
    let spanne =
        |s: Option<(f64, f64)>| s.map(|(unten, oben)| [unten.round() as u64, oben.round() as u64]);
    melde_json(serde_json::json!({
        "phase": "estimate",
        "chunks": n,
        "finished": (fertig * 1000.0).round() / 1000.0,
        "levels": einstellung.stufen,
        "tiles": spanne(Some(kacheln)),
        "bytes": spanne(bytes),
        "files": spanne(Some(dateien)),
        "s": spanne(sekunden_spanne),
        "existing_bytes": bestand_bytes,
        "free_bytes": frei,
        "enough": reicht,
        "probe_s": sekunden(started),
    }));
    Ok(())
}

/// Die Zeit der Basis je Kachel und je gelesenem Chunk, kleinste Quadrate
/// über die Ausschnitte. Ein kleiner Ausschnitt liest mehr Chunks je Kachel
/// als die Welt: Unter ihm ragen Säulen hinein, die er dekodiert und
/// beleuchtet. Ein Ausschnitt doppelter Kante hat davon halb so viele je
/// Kachel; aus beiden trennt sich, was an der Kachel und was am Chunk hängt.
/// Fällt ein Anteil negativ aus, trägt der andere alles.
fn basis_je_kachel_und_chunk(proben: &[Probe]) -> (f64, f64) {
    let s = |f: &dyn Fn(&Probe) -> f64| proben.iter().map(f).sum::<f64>();
    let (tt, tc, cc) = (
        s(&|p| p.kacheln * p.kacheln),
        s(&|p| p.kacheln * p.chunks),
        s(&|p| p.chunks * p.chunks),
    );
    let (tb, cb) = (s(&|p| p.kacheln * p.basis_s), s(&|p| p.chunks * p.basis_s));
    let det = tt * cc - tc * tc;
    let (je_kachel, je_chunk) = if det > 0.0 {
        ((tb * cc - cb * tc) / det, (cb * tt - tb * tc) / det)
    } else {
        (-1.0, -1.0)
    };
    match (je_kachel >= 0.0, je_chunk >= 0.0) {
        (true, true) => (je_kachel, je_chunk),
        (false, true) if cc > 0.0 => (0.0, cb / cc),
        _ => (tb / tt.max(1e-9), 0.0),
    }
}

/// Ein Ausschnitt des Probelaufs: Mitte in Blöcken und Kantenlänge in
/// Pixeln, wie `--center` und `--size`.
type Ausschnitt = ([i32; 2], u32);

/// Rendert einen Ausschnitt mit denselben Schaltern in `wurzel` und liest
/// aus seinen JSON-Zeilen und Kacheln, was er gekostet hat.
fn probe(
    args: &Args,
    einstellung: &Einstellung,
    projection: Projection,
    wurzel: &Path,
    ausschnitt: Ausschnitt,
) -> Result<Probe> {
    let ausgabe = Command::new(std::env::current_exe().context("eigenes Binär finden")?)
        .args(probe_schalter(args, einstellung, wurzel, ausschnitt))
        .output()
        .context("Probelauf starten")?;
    // Ein fertiger Chunk ohne Block, etwa im End, gibt keine Kachel: Dann
    // zählt der Ausschnitt nichts.
    let fehler = String::from_utf8_lossy(&ausgabe.stderr);
    if !ausgabe.status.success() && fehler.contains("keine Kachel enthält etwas") {
        return Ok(Probe::default());
    }
    if !ausgabe.status.success() {
        bail!("Probelauf fehlgeschlagen: {}", fehler.trim());
    }
    let mut probe = lies_probe(&String::from_utf8_lossy(&ausgabe.stdout))?;
    probe.basis_bytes = basis_bytes(&wurzel.join(baum_name(projection, args.cinematic)))?;
    Ok(probe)
}

/// Was die JSON-Zeilen eines Probelaufs über ihn sagen. Die nativen Stufen
/// laufen in Bändern zugleich, mit einem Beginn: Ihre Phase dauert so lange
/// wie die längste Stufe, nicht die Summe.
fn lies_probe(ausgabe: &str) -> Result<Probe> {
    let mut probe = Probe::default();
    let zahl = |z: &serde_json::Value, feld: &str| z[feld].as_f64().unwrap_or(0.0);
    for zeile in ausgabe.lines().filter(|z| z.starts_with('{')) {
        let z: serde_json::Value = serde_json::from_str(zeile).context("JSON des Probelaufs")?;
        match z["phase"].as_str() {
            Some("prepass") if z.get("chunks").is_some() => {
                probe.chunks = zahl(&z, "chunks");
                probe.vorlauf_s = zahl(&z, "s");
            }
            Some("base") => {
                probe.kacheln = zahl(&z, "tiles");
                probe.basis_s = zahl(&z, "s");
            }
            Some("level") => probe.stufen_s = probe.stufen_s.max(zahl(&z, "s")),
            Some("done") => probe.ganz_s = zahl(&z, "s"),
            _ => {}
        }
    }
    Ok(probe)
}

/// Die Schalter des Probelaufs: dieselben wie für den Lauf, mit den Werten
/// eines bestehenden Baums, aber nur der Ausschnitt, in `wurzel` und mit
/// JSON.
fn probe_schalter(
    args: &Args,
    einstellung: &Einstellung,
    wurzel: &Path,
    ([x, z], groesse): Ausschnitt,
) -> Vec<OsString> {
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
    // Version und Cache des Client-Jars; die Zustimmung folgt unten. Ohne sie
    // bräche der Probelauf ohne `--assets` ab (#202).
    if let Some(version) = &args.client_version {
        paar("--client-version", version.into());
    }
    if let Some(cache) = &args.cache_dir {
        paar("--cache-dir", cache.into());
    }
    paar("--tiles", wurzel.into());
    paar("--camera", args.camera.to_string().into());
    if let Some(richtung) = &args.direction {
        paar("--direction", richtung.into());
    }
    if let Some(scale) = args.scale {
        paar("--scale", scale.to_string().into());
    }
    paar("--biome-blend", einstellung.blend.to_string().into());
    paar("--native-levels", einstellung.stufen.to_string().into());
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
    if args.download_client_jar {
        s.push("--download-client-jar".into());
    }
    // Das Rechteck in Chunks, `--area` nimmt zwei inklusive Ecken in Blöcken.
    if let Some([x0, z0, x1, z1]) = einstellung.bereich {
        s.push("--area".into());
        for c in [x0 * 16, z0 * 16, x1 * 16 - 1, z1 * 16 - 1] {
            s.push(c.to_string().into());
        }
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

/// Bytes und Zahl aller Dateien unter `ordner`, über die Threads verteilt.
/// Einem Link folgt es nicht.
fn bytes_unter(ordner: &Path) -> (u64, u64) {
    let Ok(eintraege) = std::fs::read_dir(ordner) else {
        return (0, 0);
    };
    let eintraege: Vec<_> = eintraege.flatten().collect();
    eintraege
        .par_iter()
        .map(|e| match e.file_type() {
            Ok(art) if art.is_dir() => bytes_unter(&e.path()),
            Ok(art) if art.is_file() => (e.metadata().map_or(0, |m| m.len()), 1),
            _ => (0, 0),
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
}

/// Freier Platz für den Benutzer unter `pfad` oder dem nächsten Ordner
/// darüber, den es gibt.
fn freier_platz(pfad: &Path) -> Option<u64> {
    let pfad = std::path::absolute(pfad).ok()?;
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
        // Unter Linux sind beide schon u64, unter macOS u32.
        #[allow(clippy::unnecessary_cast)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Aus Ausschnitten, deren Zeit genau `0,002 s` je Kachel und `0,001 s`
    /// je Chunk ist, kommen beide Anteile zurück. Mit nur einem Anteil trägt
    /// er alles, auch wenn der Fit den anderen negativ fände.
    #[test]
    fn basis_trennt_kachel_und_chunk() {
        let probe = |kacheln: f64, chunks: f64, je: (f64, f64)| Probe {
            kacheln,
            chunks,
            basis_s: kacheln * je.0 + chunks * je.1,
            ..Probe::default()
        };
        let beide = [
            (100.0, 260.0),
            (400.0, 720.0),
            (90.0, 250.0),
            (360.0, 700.0),
        ];
        let fit = |je: (f64, f64)| {
            let proben: Vec<Probe> = beide.iter().map(|&(t, c)| probe(t, c, je)).collect();
            basis_je_kachel_und_chunk(&proben)
        };
        let nahe =
            |(a, b): (f64, f64), (x, y): (f64, f64)| (a - x).abs() < 1e-9 && (b - y).abs() < 1e-9;
        assert!(nahe(fit((0.002, 0.001)), (0.002, 0.001)));
        assert!(nahe(fit((0.003, 0.0)), (0.003, 0.0)));
        assert!(nahe(fit((0.0, 0.002)), (0.0, 0.002)));
        // Mehr Zeit an den kleinen Ausschnitten, als Kachel und Chunk
        // erklären: Der Anteil je Kachel fiele negativ, der je Chunk trägt.
        let schief = [
            probe(100.0, 260.0, (0.0, 0.003)),
            probe(400.0, 720.0, (0.0, 0.001)),
        ];
        let (je_kachel, je_chunk) = basis_je_kachel_und_chunk(&schief);
        assert_eq!(je_kachel, 0.0);
        assert!(je_chunk > 0.0);
    }

    /// Drei native Stufen in Bändern enden fast zugleich: Ihre Phase zählt
    /// einmal, so lange wie die längste, nicht dreimal.
    #[test]
    fn stufen_zaehlen_als_eine_phase() {
        let ausgabe = r#"Vorlauf: Text
{"phase":"prepass","chunks":400,"tiles":100,"s":0.3}
{"phase":"base","tiles":100,"of":100,"rate":50.0,"eta_s":0,"s":2.0}
{"phase":"level","level":10,"tiles":25,"of":25,"rate":10.0,"eta_s":0,"s":2.5}
{"phase":"level","level":9,"tiles":9,"of":9,"rate":3.5,"eta_s":0,"s":2.6}
{"phase":"level","level":8,"tiles":4,"of":4,"rate":1.5,"eta_s":0,"s":2.7}
{"phase":"pyramid","level":7,"tiles":2}
{"phase":"done","tiles":100,"s":6.0}"#;
        let probe = lies_probe(ausgabe).unwrap();
        assert_eq!((probe.chunks, probe.kacheln), (400.0, 100.0));
        assert_eq!((probe.vorlauf_s, probe.basis_s), (0.3, 2.0));
        assert_eq!(probe.stufen_s, 2.7);
        assert_eq!(probe.ganz_s, 6.0);
    }

    /// Der Probelauf bekommt die Zustimmung zum Client-Jar, seine Version und
    /// den Cache, wenn der Lauf sie hat, und sonst nichts davon (#202).
    #[test]
    fn probelauf_mit_client_jar() {
        use clap::Parser;
        let einstellung = Einstellung {
            stufen: 0,
            blend: 2,
            bereich: None,
        };
        let schalter = |extra: &[&str]| -> Vec<String> {
            let args =
                Args::try_parse_from([&["x", "--world", "w", "--tiles", "t"], extra].concat())
                    .unwrap();
            probe_schalter(&args, &einstellung, Path::new("p"), ([0, 0], 512))
                .into_iter()
                .map(|s| s.into_string().unwrap())
                .collect()
        };
        let mit = schalter(&[
            "--download-client-jar",
            "--client-version",
            "26.3",
            "--cache-dir",
            "c",
        ]);
        assert!(mit.iter().any(|s| s == "--download-client-jar"), "{mit:?}");
        for paar in [["--client-version", "26.3"], ["--cache-dir", "c"]] {
            assert!(mit.windows(2).any(|w| w == paar), "{mit:?}");
        }
        let ohne = schalter(&["--assets", "a"]);
        for schalter in ["--download-client-jar", "--client-version", "--cache-dir"] {
            assert!(!ohne.iter().any(|s| s == schalter), "{ohne:?}");
        }
    }
}
