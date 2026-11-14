//! Probe, nicht mergen: eine Leiter von Varianten des kompakten Packens,
//! jede Stufe auf der davor, an den Kacheln unter `tests/probe-kacheln2`.
//! Die Kopie von libwebp und `kodiere` lesen die Buchstaben in
//! `HEROIC_PROBE` bei jedem Packen; der Test setzt sie im Wechsel.
//!
//! | Buchstabe | Wirkung |
//! |---|---|
//! | `c` | Farb-Cache fest 10 Bit, keine Suche |
//! | `r` | RLE nicht als zweite Art von LZ77 |
//! | `a` | ohne `AnalyzeEntropy`, wo der Hinweis GRAPH ihr Ergebnis überschreibt |
//! | `q` | quality 51 statt 75 |
//! | `h` | 4 statt 16 Felder für Histogramme |
//! | `t` | ohne TraceBackwards |
//! | `l` | method 0 ohne räumliche Vorhersage, mit Farb-Cache 10 Bit |

use std::path::Path;
use std::time::Instant;

use heroic_map_renderer::render::{Packen, decode_webp, encode_webp};
use image::RgbaImage;

const VARIANTEN: [(&str, &str, Packen); 9] = [
    ("1-heute", "", Packen::Kompakt),
    ("2-cache10", "c", Packen::Kompakt),
    ("3-ohne-rle", "cr", Packen::Kompakt),
    ("4-ohne-analyse", "cra", Packen::Kompakt),
    ("5a-quality51", "craq", Packen::Kompakt),
    ("5b-histo4", "crah", Packen::Kompakt),
    ("5c-ohne-trace", "crat", Packen::Kompakt),
    ("6-method0-cache10", "l", Packen::Schnell),
    ("schnell-heute", "", Packen::Schnell),
];
const RUNDEN: usize = 3;

/// Was eine Variante über eine Menge von Kacheln ergibt; grösser, kleiner
/// und die schlechteste Kachel zählen gegen heute, gleich gegen die
/// Variante davor in der Liste.
#[derive(Clone, Copy, Default)]
struct Summe {
    kacheln: usize,
    bytes: usize,
    sekunden: f64,
    groesser: usize,
    kleiner: usize,
    gleich: usize,
    schlechteste: (f64, usize, usize),
}

impl Summe {
    fn dazu(&mut self, andere: &Summe) {
        self.kacheln += andere.kacheln;
        self.bytes += andere.bytes;
        self.sekunden += andere.sekunden;
        self.groesser += andere.groesser;
        self.kleiner += andere.kleiner;
        self.gleich += andere.gleich;
        if andere.schlechteste.0 > self.schlechteste.0 {
            self.schlechteste = andere.schlechteste;
        }
    }

    fn zeile(&self, gruppe: &str, variante: &str) {
        let (prozent, von, auf) = self.schlechteste;
        println!(
            "PROBE3 gruppe={gruppe} variante={variante} kacheln={} bytes={} ms={:.3} groesser={} kleiner={} \
             gleich_wie_davor={} schlechteste={prozent:+.2}% ({von} -> {auf} Byte)",
            self.kacheln,
            self.bytes,
            1000.0 * self.sekunden / (RUNDEN * self.kacheln) as f64,
            self.groesser,
            self.kleiner,
            self.gleich,
        );
    }
}

#[test]
#[ignore = "Probe für die CI, siehe .github/workflows/probe.yml"]
fn probe_leiter() {
    let wurzel = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probe-kacheln2");
    let mut gruppen: Vec<_> = std::fs::read_dir(&wurzel).unwrap().map(|e| e.unwrap().path()).collect();
    gruppen.sort();
    // Von oben und iso getrennt.
    let mut baeume = [[Summe::default(); VARIANTEN.len()]; 2];
    for gruppe in gruppen {
        let name = gruppe.file_name().unwrap().to_string_lossy().into_owned();
        let mut pfade: Vec<_> = std::fs::read_dir(&gruppe).unwrap().map(|e| e.unwrap().path()).collect();
        pfade.sort();
        let bilder: Vec<RgbaImage> = pfade
            .iter()
            .map(|p| decode_webp(&std::fs::read(p).unwrap(), (256, 256)).unwrap())
            .collect();
        let mut gepackt: Vec<Vec<Vec<u8>>> = vec![Vec::new(); VARIANTEN.len()];
        let mut sekunden = [0.0f64; VARIANTEN.len()];
        for runde in 0..RUNDEN {
            // Jede Runde beginnt bei einer anderen Variante.
            for k in 0..VARIANTEN.len() {
                let v = (k + 3 * runde) % VARIANTEN.len();
                let (_, buchstaben, packen) = VARIANTEN[v];
                // Der Test läuft allein in seinem Prozess; niemand liest die Umgebung daneben.
                unsafe { std::env::set_var("HEROIC_PROBE", buchstaben) };
                let beginn = Instant::now();
                let daten: Vec<Vec<u8>> = bilder.iter().map(|b| encode_webp(b, packen).unwrap()).collect();
                sekunden[v] += beginn.elapsed().as_secs_f64();
                if runde == 0 {
                    // Die Pixel müssen bleiben, was sie waren.
                    for (d, bild) in daten.iter().zip(&bilder) {
                        assert_eq!(decode_webp(d, (256, 256)).unwrap().as_raw(), bild.as_raw(), "{name} {buchstaben}");
                    }
                    gepackt[v] = daten;
                }
            }
        }
        for (v, (variante, _, _)) in VARIANTEN.iter().enumerate() {
            let mut summe = Summe { kacheln: bilder.len(), sekunden: sekunden[v], ..Summe::default() };
            for i in 0..bilder.len() {
                let (heute, jetzt) = (gepackt[0][i].len(), gepackt[v][i].len());
                summe.bytes += jetzt;
                summe.groesser += usize::from(jetzt > heute);
                summe.kleiner += usize::from(jetzt < heute);
                summe.gleich += usize::from(v > 0 && gepackt[v][i] == gepackt[v - 1][i]);
                let zuwachs = 100.0 * (jetzt as f64 / heute as f64 - 1.0);
                if zuwachs > summe.schlechteste.0 {
                    summe.schlechteste = (zuwachs, heute, jetzt);
                }
            }
            summe.zeile(&name, variante);
            baeume[usize::from(name.starts_with("iso"))][v].dazu(&summe);
        }
    }
    for (baum, summen) in ["oben-alle", "iso-alle"].iter().zip(&baeume) {
        for (summe, (variante, _, _)) in summen.iter().zip(&VARIANTEN) {
            summe.zeile(baum, variante);
        }
    }
}
