//! Probe, nicht mergen: was mit einer Pyramide geschieht, deren Eltern aus
//! den dekodierten Kindern entstehen, wenn die Kacheln near-lossless oder
//! verlustbehaftet (VP8) gepackt sind. Je Stufe über der Basis: Bytes, wie
//! weit die Pixel von der verlustfreien Pyramide abweichen, und die Zeit
//! für Packen und Dekodieren.
//!
//! Die Basis sind die vollständigen feinsten Stufen von vier Ausschnitten
//! der Testwelt unter `tests/probe-kacheln2`, je von oben und iso. Die
//! Abweichung der Farbe zählt an Pixeln, die in der verlustfreien Pyramide
//! Alpha über 0 haben, die von Alpha an allen Pixeln. PSNR über R, G und B,
//! nicht mit Alpha gewichtet; dazu vormultipliziert, Farbe mal Alpha, an
//! allen Pixeln.

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, TileId, Verkleinern, decode_webp, encode_webp, merge};
use image::RgbaImage;
use libwebp_sys as webp;

#[derive(Clone, Copy)]
enum Art {
    /// Wie `kodiere` mit `Packen::Kompakt`.
    Heute,
    /// Wie heute, dazu near-lossless mit dieser Stufe.
    Nah(c_int),
    /// Die Farbe gerundet wie near-lossless mit dieser Stufe, Alpha Bit für
    /// Bit wie im Bild: near-lossless packen, dekodieren, Alpha
    /// zurücksetzen, kompakt verlustfrei packen. Die Zeit sagt darum nichts.
    NahAlpha(c_int),
    /// VP8 mit dieser quality und method 4, Alpha verlustfrei.
    Vp8(f32),
}

/// Name, Packung der Basis, Packung der Eltern, ob die Eltern aus den
/// dekodierten Kindern entstehen, und die Schalter in `HEROIC_PROBE` für
/// die Kopie von libwebp: „cra“ ist Farb-Cache fest, ohne RLE, ohne Analyse
/// der Entropie. Ohne dekodierte Kinder packt jede Stufe das Bild der
/// verlustfreien Pyramide: der Verlust einer Stufe allein.
const LAEUFE: [(&str, Art, Art, bool, &str); 16] = [
    ("verlustfrei", Art::Heute, Art::Heute, false, ""),
    ("verlustfrei-s4", Art::Heute, Art::Heute, false, "cra"),
    ("direkt-q90", Art::Vp8(90.0), Art::Vp8(90.0), false, ""),
    ("direkt-nl60", Art::Nah(60), Art::Nah(60), false, ""),
    ("direkt-nl40", Art::Nah(40), Art::Nah(40), false, ""),
    ("direkt-nl60a", Art::NahAlpha(60), Art::NahAlpha(60), false, ""),
    ("direkt-nl40a", Art::NahAlpha(40), Art::NahAlpha(40), false, ""),
    ("q90-q90", Art::Vp8(90.0), Art::Vp8(90.0), true, ""),
    ("q90-nl60", Art::Vp8(90.0), Art::Nah(60), true, ""),
    ("q90-verlustfrei", Art::Vp8(90.0), Art::Heute, true, ""),
    ("nl40-nl40", Art::Nah(40), Art::Nah(40), true, ""),
    ("nl60-nl60", Art::Nah(60), Art::Nah(60), true, ""),
    ("nl40-nl40-s4", Art::Nah(40), Art::Nah(40), true, "cra"),
    ("nl60-nl60-s4", Art::Nah(60), Art::Nah(60), true, "cra"),
    ("nl40a-nl40a", Art::NahAlpha(40), Art::NahAlpha(40), true, ""),
    ("nl60a-nl60a", Art::NahAlpha(60), Art::NahAlpha(60), true, ""),
];

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
    if let Art::NahAlpha(stufe) = art {
        let mut gerundet = decode_webp(&packe(image, Art::Nah(stufe)), image.dimensions()).unwrap();
        for (ziel, quelle) in gerundet.pixels_mut().zip(image.pixels()) {
            ziel[3] = quelle[3];
        }
        return packe(&gerundet, Art::Heute);
    }
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
        // Sonst die Vorgaben von libwebp: method 4, Alpha verlustfrei
        // gepackt (alpha_quality 100).
        Art::Vp8(quality) => config.quality = quality,
        Art::NahAlpha(_) => unreachable!(),
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

type Stufe = BTreeMap<TileId, RgbaImage>;

/// Die Stufe über `kinder`, `tiefe` Stufen über der Basis, wie die Pyramide
/// sie von der Platte baut.
fn eltern(kinder: &Stufe, art: Verkleinern, tiefe: u32) -> Stufe {
    let mut teile: BTreeMap<TileId, Vec<(TileId, RgbaImage)>> = BTreeMap::new();
    for (id, bild) in kinder {
        teile.entry(id.parent()).or_default().push((*id, bild.clone()));
    }
    teile.into_iter().map(|(id, teile)| (id, merge(id, &teile, art, tiefe))).collect()
}

fn psnr(quadrate: u64, sichtbar: u64) -> f64 {
    10.0 * (255.0f64 * 255.0 * 3.0 * sichtbar as f64 / quadrate as f64).log10()
}

#[derive(Default)]
struct Zahl {
    bytes: usize,
    sichtbar: u64,
    quadrate: u64,
    max_rgb: u8,
    max_a: u8,
    /// Grösste Abweichung der Farbe an Pixeln, die im Original ganz decken.
    max_rgb_deckend: u8,
    /// Vormultipliziert, also Farbe mal Alpha: an allen Pixeln.
    quadrate_vm: u64,
    max_vm: u8,
    /// Pixel, deren Alpha abweicht.
    alpha_anders: u64,
    /// Pixel, die sichtbar wurden oder verschwanden.
    gekippt: u64,
    /// Kleinste PSNR einer Kachel; `None`, solange keine abweicht.
    schlechteste: Option<f64>,
    packen: f64,
    dekodieren: f64,
}

/// Packt jede Kachel aus `bilder`, dekodiert sie wieder und vergleicht mit
/// der verlustfreien Stufe `bezug`. Gibt zurück, was der Betrachter sieht.
fn packe_stufe(bilder: &Stufe, bezug: &Stufe, art: Art, zahl: &mut Zahl) -> Stufe {
    assert!(bilder.keys().eq(bezug.keys()), "andere Kacheln als die verlustfreie Stufe");
    let mut sicht = Stufe::new();
    for ((id, bild), original) in bilder.iter().zip(bezug.values()) {
        let beginn = Instant::now();
        let daten = packe(bild, art);
        zahl.packen += beginn.elapsed().as_secs_f64();
        let beginn = Instant::now();
        let zurueck = decode_webp(&daten, (256, 256)).unwrap();
        zahl.dekodieren += beginn.elapsed().as_secs_f64();
        zahl.bytes += daten.len();
        let (mut sichtbar, mut quadrate) = (0u64, 0u64);
        for (a, b) in original.pixels().zip(zurueck.pixels()) {
            zahl.max_a = zahl.max_a.max(a[3].abs_diff(b[3]));
            zahl.alpha_anders += u64::from(a[3] != b[3]);
            zahl.gekippt += u64::from((a[3] == 0) != (b[3] == 0));
            for k in 0..3 {
                let vm = |p: &image::Rgba<u8>| (u32::from(p[k]) * u32::from(p[3]) + 127) / 255;
                let d = vm(a).abs_diff(vm(b));
                zahl.max_vm = zahl.max_vm.max(d as u8);
                zahl.quadrate_vm += u64::from(d * d);
            }
            if a[3] == 0 {
                continue;
            }
            sichtbar += 1;
            for k in 0..3 {
                let d = a[k].abs_diff(b[k]);
                zahl.max_rgb = zahl.max_rgb.max(d);
                if a[3] == 255 {
                    zahl.max_rgb_deckend = zahl.max_rgb_deckend.max(d);
                }
                quadrate += u64::from(d) * u64::from(d);
            }
        }
        zahl.sichtbar += sichtbar;
        zahl.quadrate += quadrate;
        if quadrate > 0 {
            let wert = psnr(quadrate, sichtbar);
            zahl.schlechteste = Some(zahl.schlechteste.map_or(wert, |s| s.min(wert)));
        }
        sicht.insert(*id, zurueck);
    }
    sicht
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_pyramide() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    let lies = |pfad: &Path| decode_webp(&std::fs::read(pfad).unwrap(), (256, 256)).unwrap();
    let zahlen = |teile: &[&str]| -> Vec<i32> { teile.iter().map(|t| t.parse().unwrap()).collect() };
    // Feinste Stufe, zugleich die Zahl der Stufen über ihr, und wie der Baum verkleinert.
    for (baum, fein, verkleinern) in [("oben", 8u32, Verkleinern::Pixel), ("iso", 10, Verkleinern::Mitteln)] {
        for gruppe in ["wald", "wasser", "wueste", "rand"] {
            let mut bezug: Vec<Stufe> = vec![
                std::fs::read_dir(wurzel.join(format!("{baum}-{gruppe}")))
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .map(|pfad| {
                        let name = pfad.file_stem().unwrap().to_string_lossy().into_owned();
                        let xy = zahlen(&name.split('_').collect::<Vec<_>>());
                        (TileId { x: xy[0], y: xy[1] }, lies(&pfad))
                    })
                    .collect(),
            ];
            for tiefe in 1..=fein {
                bezug.push(eltern(&bezug[tiefe as usize - 1], verkleinern, tiefe));
            }
            // Gegenprobe: Die Stufen, die der Renderer über diesen Kacheln
            // gebaut hat, gleichen den hier gebauten Pixel für Pixel.
            let mut geprueft = 0;
            for pfad in std::fs::read_dir(wurzel.join(format!("{baum}-pyramide"))).unwrap().map(|e| e.unwrap().path()) {
                let name = pfad.file_stem().unwrap().to_string_lossy().into_owned();
                let Some(rest) = name.strip_prefix(&format!("{gruppe}_z")) else { continue };
                let zxy = zahlen(&rest.split('_').collect::<Vec<_>>());
                let id = TileId { x: zxy[1], y: zxy[2] };
                assert!(bezug[(fein as i32 - zxy[0]) as usize].get(&id) == Some(&lies(&pfad)), "{baum} {name}");
                geprueft += 1;
            }
            assert!(geprueft > 0, "{baum} {gruppe}: keine Stufe des Renderers zum Vergleich");
            // `packe` packt mit `Art::Heute` Byte für Byte wie der Renderer.
            unsafe { std::env::set_var("HEROIC_PROBE", "") };
            let erste = bezug[0].values().next().unwrap();
            assert_eq!(packe(erste, Art::Heute), encode_webp(erste, Packen::Kompakt).unwrap());

            for (lauf, basis, oben, dekodiert, schalter) in LAEUFE {
                // Der Test läuft allein in seinem Prozess; niemand liest die Umgebung daneben.
                unsafe { std::env::set_var("HEROIC_PROBE", schalter) };
                let mut sicht = Stufe::new();
                for tiefe in 0..=fein {
                    let mut zahl = Zahl::default();
                    let stufe = &bezug[tiefe as usize];
                    let art = if tiefe == 0 { basis } else { oben };
                    sicht = if dekodiert && tiefe > 0 {
                        packe_stufe(&eltern(&sicht, verkleinern, tiefe), stufe, art, &mut zahl)
                    } else {
                        packe_stufe(stufe, stufe, art, &mut zahl)
                    };
                    println!(
                        "PROBE6 baum={baum} gruppe={gruppe} lauf={lauf} stufe={tiefe} kacheln={} bytes={} sichtbar={} \
                         quadrate={} max_rgb={} max_a={} alpha_anders={} gekippt={} psnr={:.2} psnr_schlechteste_kachel={:.2} \
                         ms_packen={:.2} ms_dekodieren={:.2} max_rgb_deckend={} quadrate_vm={} max_vm={}",
                        stufe.len(),
                        zahl.bytes,
                        zahl.sichtbar,
                        zahl.quadrate,
                        zahl.max_rgb,
                        zahl.max_a,
                        zahl.alpha_anders,
                        zahl.gekippt,
                        psnr(zahl.quadrate, zahl.sichtbar),
                        zahl.schlechteste.unwrap_or(f64::INFINITY),
                        1000.0 * zahl.packen,
                        1000.0 * zahl.dekodieren,
                        zahl.max_rgb_deckend,
                        zahl.quadrate_vm,
                        zahl.max_vm,
                    );
                }
            }
        }
    }
}
