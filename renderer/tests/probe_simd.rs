//! Probe, nicht mergen: Zeit und Bytes von libwebp mit allen SIMD-Stufen der
//! CPU, nur mit SSE2 und ohne SIMD. `PROBE_SIMD` wählt `alle`, `sse2` oder
//! `ohne`; je Aufruf ein Prozess, weil libwebp seine Tabellen einmal je
//! Prozess einrichtet. Die Kacheln sind 256 × 256 Pixel aus den Bildern der
//! Doku und den Goldbildern.

use std::ffi::c_int;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, encode_webp};
use image::RgbaImage;

type CpuInfo = unsafe extern "C" fn(c_int) -> c_int;

unsafe extern "C" {
    /// `VP8GetCPUInfo` aus `src/dsp/cpu.h`: fragt libwebp, was die CPU kann.
    static mut VP8GetCPUInfo: Option<CpuInfo>;
}

static ECHT: OnceLock<Option<CpuInfo>> = OnceLock::new();
const SSE2: c_int = 0;
const SSE4_1: c_int = 3;
const AVX2: c_int = 5;

fn echt(stufe: c_int) -> c_int {
    match ECHT.get().copied().flatten() {
        Some(frage) => unsafe { frage(stufe) },
        None => 0,
    }
}

unsafe extern "C" fn nur_sse2(stufe: c_int) -> c_int {
    if stufe == SSE2 { echt(stufe) } else { 0 }
}

unsafe extern "C" fn ohne(_: c_int) -> c_int {
    0
}

/// Alle ganzen Kacheln aus den Bildern unter `ordner`, nach Namen sortiert.
fn kacheln_aus(ordner: &Path, endung: &str, aus: &mut Vec<RgbaImage>) {
    let mut pfade: Vec<_> = std::fs::read_dir(ordner)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == endung))
        .collect();
    pfade.sort();
    for pfad in pfade {
        let Ok(bild) = image::open(&pfad) else {
            continue;
        };
        let bild = bild.to_rgba8();
        for y in 0..bild.height() / 256 {
            for x in 0..bild.width() / 256 {
                aus.push(image::imageops::crop_imm(&bild, x * 256, y * 256, 256, 256).to_image());
            }
        }
    }
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_simd() {
    let modus = std::env::var("PROBE_SIMD").unwrap_or_else(|_| "alle".into());
    // Vor dem ersten Kodieren: Danach hat libwebp seine Tabellen gewählt.
    unsafe {
        ECHT.set(VP8GetCPUInfo).ok();
        match modus.as_str() {
            "sse2" => VP8GetCPUInfo = Some(nur_sse2),
            "ohne" => VP8GetCPUInfo = Some(ohne),
            _ => {}
        }
    }
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut kacheln = Vec::new();
    kacheln_aus(&wurzel.join("../docs/bilder"), "webp", &mut kacheln);
    kacheln_aus(&wurzel.join("tests/fixtures/golden"), "png", &mut kacheln);
    kacheln.truncate(240);
    for (name, packen, runden) in [("schnell", Packen::Schnell, 12), ("kompakt", Packen::Kompakt, 4)] {
        let (mut bytes, mut hash) = (0usize, 0xcbf2_9ce4_8422_2325u64);
        let beginn = Instant::now();
        for runde in 0..runden {
            for kachel in &kacheln {
                let daten = encode_webp(kachel, packen).unwrap();
                if runde == 0 {
                    bytes += daten.len();
                    for b in daten {
                        hash = (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                    }
                }
            }
        }
        let sekunden = beginn.elapsed().as_secs_f64();
        println!(
            "PROBE modus={modus} cpu_sse2={} cpu_sse41={} cpu_avx2={} packen={name} kacheln={} runden={runden} \
             bytes={bytes} hash={hash:016x} sekunden={sekunden:.3} ms_je_kachel={:.3}",
            echt(SSE2),
            echt(SSE4_1),
            echt(AVX2),
            kacheln.len(),
            1000.0 * sekunden / (runden as usize * kacheln.len()) as f64,
        );
    }
}
