//! Probe, nicht mergen: was near-lossless und verlustbehaftetes WebP (VP8)
//! an den Kacheln unter `tests/probe-kacheln2` brächten: Bytes, Zeit und wie
//! weit die Pixel abweichen. Nur Zahlen; docs/entscheidungen/0004 gilt.
//! Die Kacheln liegen nicht im Tag; wie sie entstehen, steht in
//! docs/messungen/2026-11-13-farbcache-fest-statt-suchen.md, zweiter Lauf.
//!
//! Die Abweichung zählt an sichtbaren Pixeln, also dort, wo das Original
//! Alpha über 0 hat; für Alpha selbst an allen Pixeln. PSNR über R, G und B
//! der sichtbaren Pixel, nicht mit Alpha gewichtet.

use std::ffi::c_int;
use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, decode_webp, encode_webp};
use image::RgbaImage;
use libwebp_sys as webp;

#[derive(Clone, Copy)]
enum Art {
    /// Wie `kodiere` mit `Packen::Kompakt`.
    Heute,
    /// Wie heute, dazu near-lossless mit dieser Stufe; 100 wäre aus.
    Nah(c_int),
    /// VP8 mit dieser quality und method 4, Alpha verlustfrei; mit oder ohne
    /// `use_sharp_yuv`.
    Vp8(f32, bool),
}

const VARIANTEN: [(&str, Art); 8] = [
    ("heute-kompakt", Art::Heute),
    ("near-lossless-80", Art::Nah(80)),
    ("near-lossless-60", Art::Nah(60)),
    ("near-lossless-40", Art::Nah(40)),
    ("vp8-q95", Art::Vp8(95.0, false)),
    ("vp8-q95-sharp", Art::Vp8(95.0, true)),
    ("vp8-q90", Art::Vp8(90.0, false)),
    ("vp8-q90-sharp", Art::Vp8(90.0, true)),
];
const RUNDEN: usize = 3;

unsafe extern "C" fn anhaengen(daten: *const u8, laenge: usize, bild: *const webp::WebPPicture) -> c_int {
    if laenge > 0 {
        // SAFETY: libwebp übergibt `laenge` lesbare Bytes, und `custom_ptr`
        // zeigt auf den `Vec<u8>` aus `packe`.
        unsafe {
            let out = &mut *(*bild).custom_ptr.cast::<Vec<u8>>();
            out.extend_from_slice(std::slice::from_raw_parts(daten, laenge));
        }
    }
    1
}

fn packe(image: &RgbaImage, art: Art) -> Vec<u8> {
    let mut config = webp::WebPConfig::new().unwrap();
    let mut bild = webp::WebPPicture::new().unwrap();
    config.thread_level = 0;
    match art {
        Art::Heute | Art::Nah(_) => {
            config.exact = 1;
            // SAFETY: `config` hat libwebp angelegt.
            assert!(unsafe { webp::WebPConfigLosslessPreset(&mut config, 1) } != 0);
            config.quality = 75.0;
            config.image_hint = webp::WebPImageHint::WEBP_HINT_GRAPH;
            if let Art::Nah(stufe) = art {
                config.near_lossless = stufe;
            }
        }
        Art::Vp8(quality, sharp) => {
            // Sonst die Vorgaben von libwebp: method 4, Alpha verlustfrei
            // gepackt (alpha_quality 100).
            config.quality = quality;
            config.use_sharp_yuv = c_int::from(sharp);
        }
    }
    bild.use_argb = 1;
    bild.width = image.width() as c_int;
    bild.height = image.height() as c_int;
    let mut out = Vec::new();
    bild.writer = Some(anhaengen);
    bild.custom_ptr = (&raw mut out).cast();
    // SAFETY: wie `kodiere` in render/tiles.rs.
    let gelungen = unsafe {
        webp::WebPPictureImportRGBA(&mut bild, image.as_raw().as_ptr(), 4 * bild.width) != 0
            && webp::WebPEncode(&config, &mut bild) != 0
    };
    let fehler = bild.error_code;
    // SAFETY: gibt frei, was der Import angelegt hat.
    unsafe { webp::WebPPictureFree(&mut bild) };
    assert!(gelungen, "libwebp kodiert die Kachel nicht: {fehler:?}");
    out
}

#[derive(Clone, Copy)]
struct Summe {
    kacheln: usize,
    bytes: usize,
    packen: f64,
    dekodieren: f64,
    sichtbar: u64,
    geaendert: u64,
    /// Grösste Abweichung je Kanal: R, G, B, A.
    groesste: [u8; 4],
    quadrate: u64,
    /// Kleinste PSNR einer einzelnen Kachel.
    schlechteste: f64,
}

fn psnr(quadrate: u64, sichtbar: u64) -> f64 {
    10.0 * (255.0f64 * 255.0 * 3.0 * sichtbar as f64 / quadrate as f64).log10()
}

impl Summe {
    const LEER: Summe = Summe {
        kacheln: 0,
        bytes: 0,
        packen: 0.0,
        dekodieren: 0.0,
        sichtbar: 0,
        geaendert: 0,
        groesste: [0; 4],
        quadrate: 0,
        schlechteste: f64::INFINITY,
    };

    fn dazu(&mut self, andere: &Summe) {
        self.kacheln += andere.kacheln;
        self.bytes += andere.bytes;
        self.packen += andere.packen;
        self.dekodieren += andere.dekodieren;
        self.sichtbar += andere.sichtbar;
        self.geaendert += andere.geaendert;
        self.quadrate += andere.quadrate;
        for k in 0..4 {
            self.groesste[k] = self.groesste[k].max(andere.groesste[k]);
        }
        self.schlechteste = self.schlechteste.min(andere.schlechteste);
    }

    /// Nimmt die Abweichung einer Kachel auf.
    fn vergleiche(&mut self, original: &RgbaImage, zurueck: &RgbaImage) {
        let (mut sichtbar, mut quadrate) = (0u64, 0u64);
        for (a, b) in original.pixels().zip(zurueck.pixels()) {
            self.groesste[3] = self.groesste[3].max(a[3].abs_diff(b[3]));
            if a[3] == 0 {
                continue;
            }
            sichtbar += 1;
            self.geaendert += u64::from(a != b);
            for k in 0..3 {
                let d = a[k].abs_diff(b[k]);
                self.groesste[k] = self.groesste[k].max(d);
                quadrate += u64::from(d) * u64::from(d);
            }
        }
        self.sichtbar += sichtbar;
        self.quadrate += quadrate;
        if sichtbar > 0 {
            self.schlechteste = self.schlechteste.min(psnr(quadrate, sichtbar));
        }
    }

    fn zeile(&self, gruppe: &str, variante: &str) {
        let je = 1000.0 / (RUNDEN * self.kacheln) as f64;
        let [r, g, b, a] = self.groesste;
        println!(
            "PROBE5 gruppe={gruppe} variante={variante} kacheln={} bytes={} ms={:.3} ms_dekodieren={:.3} sichtbar={} \
             geaendert_prozent={:.2} max_r={r} max_g={g} max_b={b} max_a={a} psnr={:.2} psnr_schlechteste_kachel={:.2}",
            self.kacheln,
            self.bytes,
            je * self.packen,
            je * self.dekodieren,
            self.sichtbar,
            100.0 * self.geaendert as f64 / self.sichtbar.max(1) as f64,
            psnr(self.quadrate, self.sichtbar),
            self.schlechteste,
        );
    }
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_verlust() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    let mut gruppen: Vec<_> = std::fs::read_dir(&wurzel).unwrap().map(|e| e.unwrap().path()).collect();
    gruppen.sort();
    // Von oben und iso getrennt.
    let mut baeume = [[Summe::LEER; VARIANTEN.len()]; 2];
    for gruppe in gruppen {
        let name = gruppe.file_name().unwrap().to_string_lossy().into_owned();
        let mut pfade: Vec<_> = std::fs::read_dir(&gruppe).unwrap().map(|e| e.unwrap().path()).collect();
        pfade.sort();
        let bilder: Vec<RgbaImage> = pfade
            .iter()
            .map(|p| decode_webp(&std::fs::read(p).unwrap(), (256, 256)).unwrap())
            .collect();
        // `packe` packt mit `Art::Heute` Byte für Byte wie der Renderer.
        for bild in &bilder {
            assert_eq!(packe(bild, Art::Heute), encode_webp(bild, Packen::Kompakt).unwrap(), "{name}");
        }
        let mut summen = [Summe { kacheln: bilder.len(), ..Summe::LEER }; VARIANTEN.len()];
        for runde in 0..RUNDEN {
            // Jede Runde beginnt bei einer anderen Variante.
            for k in 0..VARIANTEN.len() {
                let v = (k + 3 * runde) % VARIANTEN.len();
                let beginn = Instant::now();
                let daten: Vec<Vec<u8>> = bilder.iter().map(|b| packe(b, VARIANTEN[v].1)).collect();
                summen[v].packen += beginn.elapsed().as_secs_f64();
                let beginn = Instant::now();
                let zurueck: Vec<RgbaImage> = daten.iter().map(|d| decode_webp(d, (256, 256)).unwrap()).collect();
                summen[v].dekodieren += beginn.elapsed().as_secs_f64();
                if runde == 0 {
                    for ((d, bild), neu) in daten.iter().zip(&bilder).zip(&zurueck) {
                        summen[v].bytes += d.len();
                        summen[v].vergleiche(bild, neu);
                    }
                }
            }
        }
        for (summe, (variante, _)) in summen.iter().zip(&VARIANTEN) {
            summe.zeile(&name, variante);
        }
        for (ziel, summe) in baeume[usize::from(name.starts_with("iso"))].iter_mut().zip(&summen) {
            ziel.dazu(summe);
        }
    }
    for (baum, summen) in ["oben-alle", "iso-alle"].iter().zip(&baeume) {
        for (summe, (variante, _)) in summen.iter().zip(&VARIANTEN) {
            summe.zeile(baum, variante);
        }
    }
}
