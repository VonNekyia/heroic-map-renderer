//! Probe, nicht mergen: der Farb-Cache fest auf 10 Bit gegen die Suche von
//! heute, je Gruppe von Kacheln einzeln. Die Kopie von libwebp liest
//! `HEROIC_PROBE_CACHE` bei jedem Packen; der Test setzt die Variable im
//! Wechsel. Die Kacheln unter `tests/probe-kacheln2` sind echte Kacheln der
//! Testwelt: Wasser, Wüste, Wald und der Rand der Welt, von oben und im iso,
//! dazu Kacheln der Pyramide. Sie liegen nicht im Tag; wie sie entstehen,
//! steht in docs/messungen/2026-11-13-farbcache-fest-statt-suchen.md.

use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, decode_webp, encode_webp};
use image::RgbaImage;

fn fest(an: bool) {
    // Der Test läuft allein in seinem Prozess; niemand liest die Umgebung daneben.
    unsafe {
        if an {
            std::env::set_var("HEROIC_PROBE_CACHE", "f10");
        } else {
            std::env::remove_var("HEROIC_PROBE_CACHE");
        }
    }
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_farbcache2() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    let mut gruppen: Vec<_> = std::fs::read_dir(&wurzel).unwrap().map(|e| e.unwrap().path()).collect();
    gruppen.sort();
    for gruppe in gruppen {
        let mut pfade: Vec<_> = std::fs::read_dir(&gruppe).unwrap().map(|e| e.unwrap().path()).collect();
        pfade.sort();
        let bilder: Vec<RgbaImage> = pfade
            .iter()
            .map(|p| decode_webp(&std::fs::read(p).unwrap(), (256, 256)).unwrap())
            .collect();
        let mut bytes = [vec![0usize; bilder.len()], vec![0usize; bilder.len()]];
        let mut sekunden = [0.0f64; 2];
        let runden = 3;
        for runde in 0..runden {
            for (variante, an) in [false, true].into_iter().enumerate() {
                fest(an);
                let beginn = Instant::now();
                let mut gepackt = Vec::with_capacity(bilder.len());
                for bild in &bilder {
                    gepackt.push(encode_webp(bild, Packen::Kompakt).unwrap());
                }
                sekunden[variante] += beginn.elapsed().as_secs_f64();
                if runde == 0 {
                    for (i, (daten, bild)) in gepackt.iter().zip(&bilder).enumerate() {
                        bytes[variante][i] = daten.len();
                        // Die Pixel müssen bleiben, was sie waren.
                        assert_eq!(decode_webp(daten, (256, 256)).unwrap().as_raw(), bild.as_raw());
                    }
                }
            }
        }
        fest(false);
        let summe = |v: usize| bytes[v].iter().sum::<usize>();
        let zuwachs = |i: usize| 100.0 * (bytes[1][i] as f64 / bytes[0][i] as f64 - 1.0);
        let schlechteste = (0..bilder.len()).max_by(|&a, &b| zuwachs(a).total_cmp(&zuwachs(b))).unwrap();
        let groesser = (0..bilder.len()).filter(|&i| bytes[1][i] > bytes[0][i]).count();
        let kleiner = (0..bilder.len()).filter(|&i| bytes[1][i] < bytes[0][i]).count();
        let je = |v: usize| 1000.0 * sekunden[v] / (runden * bilder.len()) as f64;
        println!(
            "PROBE2 gruppe={} kacheln={} bytes_heute={} bytes_fest10={} bytes_prozent={:+.3} ms_heute={:.3} ms_fest10={:.3} \
             zeit_prozent={:+.1} groesser={groesser} kleiner={kleiner} schlechteste={:+.2}% ({} -> {} Byte)",
            gruppe.file_name().unwrap().to_string_lossy(),
            bilder.len(),
            summe(0),
            summe(1),
            100.0 * (summe(1) as f64 / summe(0) as f64 - 1.0),
            je(0),
            je(1),
            100.0 * (je(1) / je(0) - 1.0),
            zuwachs(schlechteste),
            bytes[0][schlechteste],
            bytes[1][schlechteste],
        );
    }
}
