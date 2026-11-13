//! Probe, nicht mergen: Bytes und Zeit des kompakten Packens, wenn libwebp die
//! Grösse des Farb-Caches nicht sucht, sondern fest bekommt oder nur bis zu
//! einer Grenze sucht. Die Variante liest die Kopie von libwebp aus
//! `HEROIC_PROBE_CACHE`; `PROBE_VARIANTE` ist nur ihr Name in der Ausgabe.
//! Die Kacheln unter `tests/probe-kacheln` sind echte Basiskacheln der
//! Testwelt, von oben mit Relief und im iso mit Cinematic. Sie liegen nicht
//! im Tag; wie sie entstehen, steht in
//! docs/messungen/2026-11-13-farbcache-fest-statt-suchen.md.

use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, decode_webp, encode_webp};
use image::RgbaImage;

fn kacheln(ordner: &Path) -> Vec<RgbaImage> {
    let mut pfade: Vec<_> = std::fs::read_dir(ordner).unwrap().map(|e| e.unwrap().path()).collect();
    pfade.sort();
    pfade
        .iter()
        .map(|p| decode_webp(&std::fs::read(p).unwrap(), (256, 256)).unwrap())
        .collect()
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_farbcache() {
    let variante = std::env::var("PROBE_VARIANTE").unwrap_or_else(|_| "heute".into());
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln");
    for baum in ["oben", "iso"] {
        let bilder = kacheln(&wurzel.join(baum));
        let runden = 4;
        let mut gepackt = Vec::new();
        let beginn = Instant::now();
        for runde in 0..runden {
            for bild in &bilder {
                let daten = encode_webp(bild, Packen::Kompakt).unwrap();
                if runde == 0 {
                    gepackt.push(daten);
                }
            }
        }
        let sekunden = beginn.elapsed().as_secs_f64();
        let bytes: usize = gepackt.iter().map(Vec::len).sum();
        // Die Pixel müssen bleiben, was sie waren.
        for (daten, bild) in gepackt.iter().zip(&bilder) {
            assert_eq!(decode_webp(daten, (256, 256)).unwrap().as_raw(), bild.as_raw());
        }
        // Zum Vergleich: dieselben Kacheln schnell gepackt, darauf wirkt die Variante nicht.
        let schnell: usize = bilder.iter().map(|b| encode_webp(b, Packen::Schnell).unwrap().len()).sum();
        println!(
            "PROBE variante={variante} baum={baum} kacheln={} bytes={bytes} schnell_bytes={schnell} ms_je_kachel={:.3}",
            bilder.len(),
            1000.0 * sekunden / (runden * bilder.len()) as f64,
        );
    }
}
