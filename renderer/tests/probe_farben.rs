//! Probe, nicht mergen: eigene Vorstufen, die die Farben einer Kachel
//! runden, bevor libwebp sie kompakt verlustfrei packt, gegen near-lossless
//! und verlustbehaftet. Alpha bleibt in jeder Vorstufe Bit für Bit, ein
//! Kanal mit 255 bleibt 255, und keiner wird neu 255.
//!
//! `probe_farben` misst an den Kacheln unter `tests/probe-kacheln2` Bytes,
//! Zeit und Abweichung; `probe_farben_pyramide` baut über den feinsten
//! Stufen die Pyramide aus den dekodierten Kindern.
//!
//! Die Abweichung der Farbe zählt an sichtbaren Pixeln, also dort, wo das
//! Original Alpha über 0 hat; die von Alpha an allen Pixeln. PSNR über R, G
//! und B, nicht mit Alpha gewichtet.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_int;
use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, TileId, Verkleinern, decode_webp, encode_webp, merge};
use image::{Rgba, RgbaImage};
use libwebp_sys as webp;

#[derive(Clone, Copy)]
enum Art {
    /// Wie `kodiere` mit `Packen::Kompakt`.
    Heute,
    /// Wie heute, dazu near-lossless von libwebp mit dieser Stufe.
    Nah(c_int),
    /// Die Farbe wie near-lossless, Alpha Bit für Bit wie im Bild:
    /// near-lossless packen, dekodieren, Alpha zurücksetzen, kompakt packen.
    /// Als Zeit zählt das erste Packen.
    NahAlpha(c_int),
    /// VP8 mit dieser quality und method 4, Alpha verlustfrei.
    Vp8(f32),
    /// Jeder Kanal auf die Mitte seiner Zelle in einem festen Gitter mit
    /// Schritt 2 d + 1: höchstens d daneben.
    Gitter(u8),
    /// Das Gitter mit dem kleinsten d bis zu diesem, mit dem die Kachel in
    /// 256 Farben passt; passt keins, das grösste.
    GitterPalette(u8),
    /// Ähnliche Farben der Kachel zusammengelegt, die häufigste zuerst,
    /// keine mehr als d je Kanal daneben, bis höchstens so viele Farben
    /// bleiben. Passt es nicht: near-lossless mit derselben Grenze.
    Zusammen(u8, usize),
    /// Jedes Pixel übernimmt den linken oder oberen Nachbarn der Ausgabe,
    /// wenn der höchstens d je Kanal neben seinem Original liegt.
    Nachbar(u8),
    /// Wie `Nachbar`; ohne passenden Nachbarn die Mitte der Gitterzelle.
    NachbarGitter(u8),
    /// Median-Schnitt über die deckenden Pixel auf so viele Farben, ohne
    /// Grenze für die Abweichung und ohne Rücksicht auf 255.
    Median(usize),
}

unsafe extern "C" fn anhaengen(daten: *const u8, laenge: usize, bild: *const webp::WebPPicture) -> c_int {
    if laenge > 0 {
        // SAFETY: libwebp übergibt `laenge` lesbare Bytes, und `custom_ptr`
        // zeigt auf den `Vec<u8>` aus `kodiere`.
        unsafe {
            let out = &mut *(*bild).custom_ptr.cast::<Vec<u8>>();
            out.extend_from_slice(std::slice::from_raw_parts(daten, laenge));
        }
    }
    1
}

/// Packt mit libwebp: kompakt, mit near-lossless oder als VP8.
fn kodiere(image: &RgbaImage, nah: c_int, vp8: Option<f32>) -> Vec<u8> {
    let mut config = webp::WebPConfig::new().unwrap();
    let mut bild = webp::WebPPicture::new().unwrap();
    config.thread_level = 0;
    match vp8 {
        // Sonst die Vorgaben von libwebp: method 4, Alpha verlustfrei.
        Some(quality) => config.quality = quality,
        None => {
            config.exact = 1;
            // SAFETY: `config` hat libwebp angelegt.
            assert!(unsafe { webp::WebPConfigLosslessPreset(&mut config, 1) } != 0);
            config.quality = 75.0;
            config.image_hint = webp::WebPImageHint::WEBP_HINT_GRAPH;
            config.near_lossless = nah;
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

/// Die Mitte der Gitterzelle; 255 bleibt 255, und nichts wird 255.
fn gitter_wert(v: u8, d: u8) -> u8 {
    if v == 255 {
        return 255;
    }
    let schritt = 2 * u16::from(d) + 1;
    ((u16::from(v) / schritt) * schritt + u16::from(d)).min(254) as u8
}

fn gitter(image: &RgbaImage, d: u8) -> RgbaImage {
    let tabelle: [u8; 256] = std::array::from_fn(|v| gitter_wert(v as u8, d));
    let mut out = image.clone();
    for p in out.pixels_mut().filter(|p| p[3] > 0) {
        for k in 0..3 {
            p[k] = tabelle[usize::from(p[k])];
        }
    }
    out
}

/// Ob die Kachel in eine Palette von libwebp passt.
fn passt_in_palette(image: &RgbaImage) -> bool {
    let mut farben = HashSet::new();
    image.pixels().all(|p| {
        farben.insert(p.0);
        farben.len() <= 256
    })
}

/// Ob `b` für `a` stehen darf: dasselbe Alpha, je Kanal höchstens d
/// daneben, und 255 genau dort, wo `a` 255 hat.
fn passt(a: &[u8; 4], b: &[u8; 4], d: u8) -> bool {
    a[3] == b[3] && (0..3).all(|k| a[k].abs_diff(b[k]) <= d && (a[k] == 255) == (b[k] == 255))
}

fn zusammenlegen(image: &RgbaImage, d: u8, hoechstens: usize) -> Option<RgbaImage> {
    let mut zahl: HashMap<[u8; 4], u32> = HashMap::new();
    for p in image.pixels() {
        *zahl.entry(p.0).or_default() += 1;
    }
    if zahl.len() <= hoechstens {
        return Some(image.clone());
    }
    let mut farben: Vec<([u8; 4], u32)> = zahl.into_iter().collect();
    farben.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut vertreter: Vec<[u8; 4]> = Vec::new();
    let mut ziel: HashMap<[u8; 4], [u8; 4]> = HashMap::with_capacity(farben.len());
    for (farbe, _) in &farben {
        // Durchsichtige Pixel bleiben, wie sie sind.
        let gefunden = if farbe[3] == 0 { None } else { vertreter.iter().find(|v| passt(farbe, v, d)) };
        let v = match gefunden {
            Some(v) => *v,
            None if vertreter.len() == hoechstens => return None,
            None => {
                vertreter.push(*farbe);
                *farbe
            }
        };
        ziel.insert(*farbe, v);
    }
    let mut out = image.clone();
    for p in out.pixels_mut() {
        p.0 = ziel[&p.0];
    }
    Some(out)
}

fn nachbar(image: &RgbaImage, d: u8, mit_gitter: bool) -> RgbaImage {
    let mut out = image.clone();
    for y in 0..image.height() {
        for x in 0..image.width() {
            let original = image.get_pixel(x, y).0;
            if original[3] == 0 {
                continue;
            }
            let links = (x > 0).then(|| out.get_pixel(x - 1, y).0);
            let oben = (y > 0).then(|| out.get_pixel(x, y - 1).0);
            let neu = [links, oben].into_iter().flatten().find(|n| passt(&original, n, d)).unwrap_or_else(|| {
                if mit_gitter {
                    let [r, g, b, a] = original;
                    [gitter_wert(r, d), gitter_wert(g, d), gitter_wert(b, d), a]
                } else {
                    original
                }
            });
            out.put_pixel(x, y, Rgba(neu));
        }
    }
    out
}

fn median_schnitt(image: &RgbaImage, hoechstens: usize) -> RgbaImage {
    let mut zahl: HashMap<[u8; 3], u64> = HashMap::new();
    for p in image.pixels().filter(|p| p[3] == 255) {
        *zahl.entry([p[0], p[1], p[2]]).or_default() += 1;
    }
    if zahl.len() <= hoechstens {
        return image.clone();
    }
    let mut alle: Vec<([u8; 3], u64)> = zahl.into_iter().collect();
    alle.sort_unstable();
    let spanne = |kiste: &[([u8; 3], u64)], k: usize| {
        let (min, max) = kiste.iter().fold((255, 0), |(lo, hi), (f, _)| (lo.min(f[k]), hi.max(f[k])));
        max - min
    };
    let mut kisten = vec![alle];
    while kisten.len() < hoechstens {
        // Die Kiste, deren längste Seite mal Pixelzahl am grössten ist.
        let gewicht = |kiste: &Vec<([u8; 3], u64)>| {
            let laengste = (0..3).map(|k| spanne(kiste, k)).max().unwrap();
            u64::from(laengste) * kiste.iter().map(|(_, n)| n).sum::<u64>()
        };
        let (i, groesstes) = kisten.iter().map(gewicht).enumerate().max_by_key(|(_, g)| *g).unwrap();
        if groesstes == 0 {
            break;
        }
        let mut kiste = kisten.swap_remove(i);
        let kanal = (0..3).max_by_key(|k| spanne(&kiste, *k)).unwrap();
        kiste.sort_unstable_by_key(|(f, _)| (f[kanal], *f));
        let haelfte = kiste.iter().map(|(_, n)| n).sum::<u64>() / 2;
        let mut bisher = 0;
        let schnitt = kiste.iter().position(|(_, n)| {
            bisher += n;
            bisher > haelfte
        });
        let schnitt = schnitt.unwrap().clamp(1, kiste.len() - 1);
        let rest = kiste.split_off(schnitt);
        kisten.push(kiste);
        kisten.push(rest);
    }
    let mut ziel: HashMap<[u8; 3], [u8; 3]> = HashMap::new();
    for kiste in &kisten {
        let pixel = kiste.iter().map(|(_, n)| n).sum::<u64>();
        let mittel: [u8; 3] =
            std::array::from_fn(|k| ((kiste.iter().map(|(f, n)| u64::from(f[k]) * n).sum::<u64>() + pixel / 2) / pixel) as u8);
        for (farbe, _) in kiste {
            ziel.insert(*farbe, mittel);
        }
    }
    let mut out = image.clone();
    for p in out.pixels_mut().filter(|p| p[3] == 255) {
        let [r, g, b] = ziel[&[p[0], p[1], p[2]]];
        p.0 = [r, g, b, 255];
    }
    out
}

struct Gepackt {
    daten: Vec<u8>,
    /// Vorstufe und Packen zusammen.
    sekunden: f64,
    /// Davon die Vorstufe.
    vorstufe: f64,
    palette: bool,
    /// Die Vorstufe passte nicht, near-lossless sprang ein.
    rueckfall: bool,
}

fn packe(image: &RgbaImage, art: Art) -> Gepackt {
    let beginn = Instant::now();
    // Ohne Vorstufe; mit near-lossless rundet libwebp nur Kacheln ohne Palette.
    let einfach = |nah: c_int, vp8: Option<f32>| {
        let daten = kodiere(image, nah, vp8);
        let sekunden = beginn.elapsed().as_secs_f64();
        Gepackt { daten, sekunden, vorstufe: 0.0, palette: vp8.is_none() && passt_in_palette(image), rueckfall: false }
    };
    let vorstufe = match art {
        Art::Heute => return einfach(100, None),
        Art::Nah(stufe) => return einfach(stufe, None),
        Art::Vp8(quality) => return einfach(100, Some(quality)),
        Art::NahAlpha(stufe) => {
            let erstes = kodiere(image, stufe, None);
            let sekunden = beginn.elapsed().as_secs_f64();
            let mut gerundet = decode_webp(&erstes, image.dimensions()).unwrap();
            for (ziel, quelle) in gerundet.pixels_mut().zip(image.pixels()) {
                ziel[3] = quelle[3];
            }
            let daten = kodiere(&gerundet, 100, None);
            return Gepackt { daten, sekunden, vorstufe: 0.0, palette: passt_in_palette(&gerundet), rueckfall: false };
        }
        Art::Gitter(d) => Some(gitter(image, d)),
        Art::GitterPalette(bis) => {
            if passt_in_palette(image) {
                Some(image.clone())
            } else {
                (1..=bis).map(|d| gitter(image, d)).find(|bild| passt_in_palette(bild)).or_else(|| Some(gitter(image, bis)))
            }
        }
        Art::Zusammen(d, hoechstens) => zusammenlegen(image, d, hoechstens),
        Art::Nachbar(d) => Some(nachbar(image, d, false)),
        Art::NachbarGitter(d) => Some(nachbar(image, d, true)),
        Art::Median(hoechstens) => Some(median_schnitt(image, hoechstens)),
    };
    let dauer = beginn.elapsed().as_secs_f64();
    match vorstufe {
        Some(bild) => {
            let daten = kodiere(&bild, 100, None);
            let sekunden = beginn.elapsed().as_secs_f64();
            Gepackt { daten, sekunden, vorstufe: dauer, palette: passt_in_palette(&bild), rueckfall: false }
        }
        None => {
            let Art::Zusammen(d, _) = art else { unreachable!() };
            let stufe = match d {
                2 => 60,
                4 => 40,
                8 => 20,
                _ => panic!("keine Stufe von near-lossless für d = {d}"),
            };
            let zurueck = packe(image, Art::NahAlpha(stufe));
            Gepackt { sekunden: dauer + zurueck.sekunden, vorstufe: dauer, rueckfall: true, ..zurueck }
        }
    }
}

fn psnr(quadrate: u64, sichtbar: u64) -> f64 {
    10.0 * (255.0f64 * 255.0 * 3.0 * sichtbar as f64 / quadrate as f64).log10()
}

#[derive(Default)]
struct Zahl {
    kacheln: usize,
    bytes: usize,
    packen: f64,
    vorstufe: f64,
    dekodieren: f64,
    palette: usize,
    rueckfall: usize,
    sichtbar: u64,
    geaendert: u64,
    quadrate: u64,
    /// Grösste Abweichung je Kanal: R, G, B, A.
    groesste: [u8; 4],
    alpha_anders: u64,
    /// Kanäle, die 255 waren und es nicht mehr sind, und solche, die es neu sind.
    weg255: u64,
    neu255: u64,
    /// Kleinste PSNR einer Kachel; `None`, solange keine abweicht.
    schlechteste: Option<f64>,
}

impl Zahl {
    /// Packt die Kachel, dekodiert sie und vergleicht mit `original`. Gibt
    /// zurück, was der Betrachter sieht.
    fn nimm(&mut self, bild: &RgbaImage, original: &RgbaImage, art: Art) -> RgbaImage {
        let gepackt = packe(bild, art);
        let beginn = Instant::now();
        let zurueck = decode_webp(&gepackt.daten, bild.dimensions()).unwrap();
        self.dekodieren += beginn.elapsed().as_secs_f64();
        self.kacheln += 1;
        self.bytes += gepackt.daten.len();
        self.packen += gepackt.sekunden;
        self.vorstufe += gepackt.vorstufe;
        self.palette += usize::from(gepackt.palette);
        self.rueckfall += usize::from(gepackt.rueckfall);
        let (mut sichtbar, mut quadrate) = (0u64, 0u64);
        for (a, b) in original.pixels().zip(zurueck.pixels()) {
            self.groesste[3] = self.groesste[3].max(a[3].abs_diff(b[3]));
            self.alpha_anders += u64::from(a[3] != b[3]);
            if a[3] == 0 {
                continue;
            }
            sichtbar += 1;
            self.geaendert += u64::from(a.0[..3] != b.0[..3]);
            for k in 0..3 {
                let d = a[k].abs_diff(b[k]);
                self.groesste[k] = self.groesste[k].max(d);
                quadrate += u64::from(d) * u64::from(d);
                self.weg255 += u64::from(a[k] == 255 && b[k] != 255);
                self.neu255 += u64::from(a[k] != 255 && b[k] == 255);
            }
        }
        self.sichtbar += sichtbar;
        self.quadrate += quadrate;
        if quadrate > 0 {
            let wert = psnr(quadrate, sichtbar);
            self.schlechteste = Some(self.schlechteste.map_or(wert, |s| s.min(wert)));
        }
        zurueck
    }

    fn zeile(&self, kopf: &str) {
        let [r, g, b, a] = self.groesste;
        println!(
            "{kopf} kacheln={} bytes={} ms_packen={:.2} ms_vorstufe={:.2} ms_dekodieren={:.2} palette={} rueckfall={} \
             sichtbar={} geaendert={} quadrate={} max_r={r} max_g={g} max_b={b} max_a={a} alpha_anders={} weg255={} \
             neu255={} psnr={:.2} psnr_schlechteste_kachel={:.2}",
            self.kacheln,
            self.bytes,
            1000.0 * self.packen,
            1000.0 * self.vorstufe,
            1000.0 * self.dekodieren,
            self.palette,
            self.rueckfall,
            self.sichtbar,
            self.geaendert,
            self.quadrate,
            self.alpha_anders,
            self.weg255,
            self.neu255,
            psnr(self.quadrate, self.sichtbar),
            self.schlechteste.unwrap_or(f64::INFINITY),
        );
    }
}

const VARIANTEN: [(&str, Art); 27] = [
    ("heute", Art::Heute),
    ("nl60", Art::Nah(60)),
    ("nl40", Art::Nah(40)),
    ("nl20", Art::Nah(20)),
    ("nl60a", Art::NahAlpha(60)),
    ("nl40a", Art::NahAlpha(40)),
    ("q95", Art::Vp8(95.0)),
    ("q90", Art::Vp8(90.0)),
    ("gitter2", Art::Gitter(2)),
    ("gitter4", Art::Gitter(4)),
    ("gitter8", Art::Gitter(8)),
    ("gitterpalette4", Art::GitterPalette(4)),
    ("gitterpalette8", Art::GitterPalette(8)),
    ("zusammen2-256", Art::Zusammen(2, 256)),
    ("zusammen4-256", Art::Zusammen(4, 256)),
    ("zusammen4-128", Art::Zusammen(4, 128)),
    ("zusammen4-64", Art::Zusammen(4, 64)),
    ("zusammen8-256", Art::Zusammen(8, 256)),
    ("zusammen8-64", Art::Zusammen(8, 64)),
    ("nachbar2", Art::Nachbar(2)),
    ("nachbar4", Art::Nachbar(4)),
    ("nachbar8", Art::Nachbar(8)),
    ("nachbargitter4", Art::NachbarGitter(4)),
    ("median256", Art::Median(256)),
    ("median128", Art::Median(128)),
    ("median64", Art::Median(64)),
    ("median16", Art::Median(16)),
];

fn lies(pfad: &Path) -> RgbaImage {
    decode_webp(&std::fs::read(pfad).unwrap(), (256, 256)).unwrap()
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_farben() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    let mut gruppen: Vec<_> = std::fs::read_dir(&wurzel).unwrap().map(|e| e.unwrap().path()).collect();
    gruppen.sort();
    for gruppe in gruppen {
        let name = gruppe.file_name().unwrap().to_string_lossy().into_owned();
        let mut pfade: Vec<_> = std::fs::read_dir(&gruppe).unwrap().map(|e| e.unwrap().path()).collect();
        pfade.sort();
        let bilder: Vec<RgbaImage> = pfade.iter().map(|p| lies(p)).collect();
        // `kodiere` packt ohne near-lossless Byte für Byte wie der Renderer.
        assert_eq!(kodiere(&bilder[0], 100, None), encode_webp(&bilder[0], Packen::Kompakt).unwrap(), "{name}");
        // Wie viele Farben die Kacheln haben.
        let mut farben: Vec<usize> = bilder.iter().map(|b| b.pixels().map(|p| p.0).collect::<HashSet<_>>().len()).collect();
        farben.sort_unstable();
        println!(
            "PROBE7F gruppe={name} kacheln={} median={} groesste={} bis256={}",
            farben.len(),
            farben[farben.len() / 2],
            farben[farben.len() - 1],
            farben.iter().filter(|n| **n <= 256).count(),
        );
        for (variante, art) in VARIANTEN {
            let mut zahl = Zahl::default();
            for bild in &bilder {
                zahl.nimm(bild, bild, art);
            }
            zahl.zeile(&format!("PROBE7 gruppe={name} variante={variante}"));
        }
    }
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

/// Jede Stufe packt wie die Basis, die Eltern entstehen aus den dekodierten
/// Kindern.
const LAEUFE: [(&str, Art); 10] = [
    ("heute", Art::Heute),
    ("nl60a", Art::NahAlpha(60)),
    ("nl40a", Art::NahAlpha(40)),
    ("gitter2", Art::Gitter(2)),
    ("gitter4", Art::Gitter(4)),
    ("gitterpalette4", Art::GitterPalette(4)),
    ("zusammen2-256", Art::Zusammen(2, 256)),
    ("zusammen4-256", Art::Zusammen(4, 256)),
    ("nachbar4", Art::Nachbar(4)),
    ("nachbargitter4", Art::NachbarGitter(4)),
];

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_farben_pyramide() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    // Feinste Stufe, zugleich die Zahl der Stufen über ihr, und wie der Baum verkleinert.
    for (baum, fein, verkleinern) in [("oben", 8u32, Verkleinern::Pixel), ("iso", 10, Verkleinern::Mitteln)] {
        for gruppe in ["wald", "wasser", "wueste", "rand"] {
            let mut bezug: Vec<Stufe> = vec![
                std::fs::read_dir(wurzel.join(format!("{baum}-{gruppe}")))
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .map(|pfad| {
                        let name = pfad.file_stem().unwrap().to_string_lossy().into_owned();
                        let xy: Vec<i32> = name.split('_').map(|t| t.parse().unwrap()).collect();
                        (TileId { x: xy[0], y: xy[1] }, lies(&pfad))
                    })
                    .collect(),
            ];
            for tiefe in 1..=fein {
                bezug.push(eltern(&bezug[tiefe as usize - 1], verkleinern, tiefe));
            }
            for (lauf, art) in LAEUFE {
                let mut sicht = Stufe::new();
                for tiefe in 0..=fein {
                    let stufe = &bezug[tiefe as usize];
                    let bilder = if tiefe == 0 { stufe.clone() } else { eltern(&sicht, verkleinern, tiefe) };
                    assert!(bilder.keys().eq(stufe.keys()));
                    let mut zahl = Zahl::default();
                    sicht = bilder.iter().map(|(id, bild)| (*id, zahl.nimm(bild, &stufe[id], art))).collect();
                    zahl.zeile(&format!("PROBE8 baum={baum} gruppe={gruppe} lauf={lauf} stufe={tiefe}"));
                }
            }
        }
    }
}
