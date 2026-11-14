//! Probe, nicht mergen: Entpacken der Chunkdaten und Packen der Höhen mit dem
//! Backend, das flate2 in diesem Build hat. Der Workflow baut zweimal: wie
//! heute mit miniz_oxide und mit `--features probe-zlib-rs`. fdeflate läuft
//! in beiden Builds gleich; an ihm ist zu sehen, wie weit die zwei
//! Programme auf demselben Rechner auseinanderliegen.
//!
//! Die Chunks sind die 20 echten Chunks der Testwelt aus den Fixtures, die
//! Höhen unter `tests/probe-hoehen` stammen aus Bäumen der Testwelt.

use std::hint::black_box;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::heights::{CELL, GROUND_CELL, Heights};

/// Die Datensätze einer Regionsdatei mit Verfahren 2 (zlib), noch gepackt.
fn datensaetze(datei: &[u8]) -> Vec<&[u8]> {
    (0..1024)
        .filter_map(|i| {
            let sektor = u32::from_be_bytes([0, datei[4 * i], datei[4 * i + 1], datei[4 * i + 2]]) as usize;
            if sektor < 2 {
                return None;
            }
            let kopf = sektor * 4096;
            let laenge = u32::from_be_bytes(datei[kopf..kopf + 4].try_into().unwrap()) as usize;
            (datei[kopf + 4] == 2).then(|| &datei[kopf + 5..kopf + 4 + laenge])
        })
        .collect()
}

/// Wie `decompress` in `world/region.rs` für Verfahren 2.
fn heute(daten: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(daten).read_to_end(&mut out).unwrap();
    out
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_entpacken() {
    let backend = if cfg!(feature = "probe-zlib-rs") { "zlib-rs" } else { "miniz_oxide" };
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let dateien = [
        std::fs::read(tests.join("fixtures/licht/region/r.-1.-1.mca")).unwrap(),
        std::fs::read(tests.join("fixtures/world/dimensions/minecraft/overworld/region/r.18.13.mca")).unwrap(),
    ];
    let gepackt: Vec<&[u8]> = dateien.iter().flat_map(|d| datensaetze(d)).collect();
    let roh: Vec<Vec<u8>> = gepackt.iter().map(|d| heute(d)).collect();
    for (d, r) in gepackt.iter().zip(&roh) {
        assert_eq!(&fdeflate::decompress_to_vec(d).unwrap(), r);
    }
    let runden = 300;
    let mut sekunden = [0.0f64; 2];
    for _ in 0..runden {
        let beginn = Instant::now();
        for d in &gepackt {
            black_box(heute(black_box(d)));
        }
        sekunden[0] += beginn.elapsed().as_secs_f64();
        let beginn = Instant::now();
        for d in &gepackt {
            black_box(fdeflate::decompress_to_vec(black_box(d)).unwrap());
        }
        sekunden[1] += beginn.elapsed().as_secs_f64();
    }
    let mb = runden as f64 * roh.iter().map(Vec::len).sum::<usize>() as f64 / 1e6;
    println!(
        "PROBE4 backend={backend} chunks={} gepackt={} roh={} flate2_mb_s={:.0} fdeflate_mb_s={:.0}",
        gepackt.len(),
        gepackt.iter().map(|d| d.len()).sum::<usize>(),
        roh.iter().map(Vec::len).sum::<usize>(),
        mb / sekunden[0],
        mb / sekunden[1],
    );

    // Die Höhen: packen wie `Heights::encode`, entpacken wie `Heights::decode`.
    for (ordner, cell) in [("heights", CELL), ("ground", GROUND_CELL)] {
        let mut pfade: Vec<_> =
            std::fs::read_dir(tests.join("probe-hoehen").join(ordner)).unwrap().map(|e| e.unwrap().path()).collect();
        pfade.sort();
        for pfad in pfade {
            let datei = std::fs::read(&pfad).unwrap();
            let hoehen = Heights::decode(&datei, cell).unwrap();
            let neu = hoehen.encode().unwrap();
            // Was dieses Backend packt, liest es als dieselben Werte zurück.
            assert_eq!(heute(&neu), heute(&datei));
            let runden = 40;
            let beginn = Instant::now();
            for _ in 0..runden {
                black_box(black_box(&hoehen).encode().unwrap());
            }
            let packen = beginn.elapsed().as_secs_f64();
            let beginn = Instant::now();
            for _ in 0..runden {
                black_box(Heights::decode(black_box(&neu), cell).unwrap());
            }
            let entpacken = beginn.elapsed().as_secs_f64();
            println!(
                "PROBE4 backend={backend} hoehen={ordner}/{} roh={} bytes_datei={} bytes={} ms_packen={:.3} ms_entpacken={:.3}",
                pfad.file_stem().unwrap().to_string_lossy(),
                heute(&datei).len(),
                datei.len(),
                neu.len(),
                1000.0 * packen / runden as f64,
                1000.0 * entpacken / runden as f64,
            );
        }
    }
}
