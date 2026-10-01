/// Die Kamera eines Laufs: schräg mit der Raute W:H oder von oben.
///
/// ```text
/// screen_x = (x - z) * h
/// screen_y = (x + z) * a - y * b
/// ```
///
/// `h` ist immer scale/2. Schräg ist `a` = scale · H/(2W) und `b` =
/// scale/2, die Wände bleiben bei jeder Raute so hoch. Von oben ist `a` =
/// scale/2 und `b` = 0. 2:1 ist die Vorgabe: `a` = scale/4, ein voller
/// Würfel belegt dann genau `scale` mal `scale` Pixel.
///
/// Es gibt keine freie Kamera und keine Perspektive. Alle Faktoren stehen
/// hier und nirgendwo sonst.
/// Siehe docs/renderer/kamera.md, „Projektion“.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// Pixelbreite eines Blocks.
    scale: u32,
    kamera: Kamera,
}

/// Schräg mit einer Raute W:H oder von oben.
/// Siehe docs/renderer/kamera.md, „Kameras“.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kamera {
    Schraeg(Raute),
    Oben,
}

/// Die Raute W:H einer schrägen Kamera, gekürzt und zwischen 2:1 und 1:1:
/// Sie entsteht nur über [`Kamera::schraeg`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raute {
    breite: u32,
    hoehe: u32,
}

impl Kamera {
    /// Die Vorgabe.
    pub const ZWEI_ZU_EINS: Kamera = Kamera::Schraeg(Raute {
        breite: 2,
        hoehe: 1,
    });

    /// `W:H` oder `top`. W:H wird gekürzt und muss zwischen 2:1 und 1:1
    /// liegen: flacher verdeckt das Gelände mehr, steiler erschiene die
    /// Oberseite höher als von oben.
    pub fn parse(text: &str) -> Result<Kamera, String> {
        if text == "top" {
            return Ok(Kamera::Oben);
        }
        let (w, h) = text
            .split_once(':')
            .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))
            .filter(|&(w, h)| w > 0 && h > 0)
            .ok_or_else(|| {
                format!("{text} ist keine Kamera: W:H mit ganzen Zahlen über 0 oder top")
            })?;
        Kamera::schraeg(w, h)
    }

    /// Schräg mit der Raute `w`:`h`, gekürzt; zwischen 2:1 und 1:1.
    pub fn schraeg(w: u32, h: u32) -> Result<Kamera, String> {
        if u64::from(w) > 2 * u64::from(h) {
            return Err(format!("{w}:{h} ist flacher als 2:1"));
        }
        if h > w {
            return Err(format!("{w}:{h} ist steiler als 1:1"));
        }
        let g = ggt(w.into(), h.into()) as u32;
        Ok(Kamera::Schraeg(Raute {
            breite: w / g,
            hoehe: h / g,
        }))
    }

    /// Der Schritt im scale, in dem jede Blockecke auf ganzen Pixeln liegt:
    /// schräg 2W, denn a = scale · H/(2W) mit teilerfremden W und H ist
    /// genau dann ganz und der scale gerade; von oben 2.
    /// Siehe docs/renderer/kamera.md, „Ganze Pixel“.
    pub fn schritt(self) -> u64 {
        match self {
            Kamera::Schraeg(Raute { breite, .. }) => 2 * u64::from(breite),
            Kamera::Oben => 2,
        }
    }

    /// `a` je scale als Bruch: schräg H/(2W), von oben 1/2.
    fn a_je_scale(self) -> (u64, u64) {
        match self {
            Kamera::Schraeg(Raute { breite, hoehe }) => (hoehe.into(), 2 * u64::from(breite)),
            Kamera::Oben => (1, 2),
        }
    }
}

impl std::fmt::Display for Kamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kamera::Schraeg(Raute { breite, hoehe }) => write!(f, "{breite}:{hoehe}"),
            Kamera::Oben => write!(f, "top"),
        }
    }
}

fn ggt(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { ggt(b, a % b) }
}

impl Projection {
    /// 32 Pixel je Block: eine Seitenfläche ist halb so breit wie der
    /// Würfel, erst so zeigt sie alle 16 Texel einer Textur. Bei 16 fiele
    /// jede zweite Texelspalte weg; dafür sind es viermal so viele
    /// Kacheln. Siehe `--scale`.
    pub const DEFAULT_SCALE: u32 = 32;

    /// 2:1, die Vorgabe.
    pub fn new(scale: u32) -> Projection {
        Projection::mit_kamera(scale, Kamera::ZWEI_ZU_EINS)
    }

    pub fn mit_kamera(scale: u32, kamera: Kamera) -> Projection {
        Projection {
            scale: scale.max(2),
            kamera,
        }
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn kamera(&self) -> Kamera {
        self.kamera
    }

    /// Dieselbe Kamera bei einem anderen scale, etwa für eine native Stufe.
    pub fn bei(&self, scale: u32) -> Projection {
        Projection::mit_kamera(scale, self.kamera)
    }

    /// Pixel je Schritt in `u = x - z`: scale/2.
    pub fn h(&self) -> f64 {
        self.scale as f64 / 2.0
    }

    /// Pixel je Schritt in `v = x + z`.
    pub fn a(&self) -> f64 {
        let (zaehler, nenner) = self.kamera.a_je_scale();
        (u64::from(self.scale) * zaehler) as f64 / nenner as f64
    }

    /// Pixel je Block Höhe: schräg scale/2, von oben 0.
    pub fn b(&self) -> f64 {
        match self.kamera {
            Kamera::Schraeg(_) => self.scale as f64 / 2.0,
            Kamera::Oben => 0.0,
        }
    }

    /// Liegt jede Blockecke auf ganzen Pixeln? Genau dann, wenn `a` ganz und
    /// der scale gerade ist, also ein Vielfaches von [`Kamera::schritt`];
    /// bei 2:1 heisst das: ein Vielfaches von 4.
    /// Siehe docs/renderer/kamera.md, „Ganze Pixel“.
    pub fn ganze_pixel(&self) -> bool {
        u64::from(self.scale).is_multiple_of(self.kamera.schritt())
    }

    /// Die Blickachse (b, 2a, b), gekürzt auf ganze teilerfremde Zahlen:
    /// 2:1 (1, 1, 1), 4:3 (2, 3, 2), 1:1 (1, 2, 1), von oben (0, 1, 0).
    /// Punkte, die sich um ein Vielfaches davon unterscheiden, landen auf
    /// demselben Pixel.
    pub fn achse(&self) -> [f32; 3] {
        let (seite, mitte) = match self.kamera {
            // (scale/2, scale·H/W, scale/2) ∝ (W, 2H, W)
            Kamera::Schraeg(Raute { breite, hoehe }) => (u64::from(breite), 2 * u64::from(hoehe)),
            Kamera::Oben => (0, 1),
        };
        let g = ggt(seite, mitte);
        [seite / g, mitte / g, seite / g].map(|c| c as f32)
    }

    /// Weltkoordinaten in Blockeinheiten auf Bildschirmpixel abbilden.
    pub fn project(&self, [x, y, z]: [f32; 3]) -> (f32, f32) {
        let (h, a, b) = (self.h() as f32, self.a() as f32, self.b() as f32);
        ((x - z) * h, (x + z) * a - y * b)
    }

    /// Blockkoordinaten auf Bildschirmpixel abbilden, in f64 anders als
    /// `project`: Für Modellecken innerhalb eines Blocks reicht f32, für
    /// Weltkoordinaten nicht.
    /// Siehe docs/renderer/kamera.md, „Weltkoordinaten in f64“.
    pub fn project_block(&self, [x, y, z]: [i32; 3]) -> (f64, f64) {
        (
            (x as f64 - z as f64) * self.h(),
            (x as f64 + z as f64) * self.a() - y as f64 * self.b(),
        )
    }

    /// Tiefe entlang der Blickachse ([`Projection::achse`]). Größer heißt
    /// näher an der Kamera.
    pub fn depth(&self, [x, y, z]: [f32; 3]) -> f32 {
        let [ax, ay, az] = self.achse();
        x * ax + y * ay + z * az
    }
}

impl Default for Projection {
    fn default() -> Self {
        Projection::new(Projection::DEFAULT_SCALE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wuerfel_belegt_genau_scale_mal_scale() {
        let p = Projection::new(16);
        let ecken = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ];
        let punkte: Vec<(f32, f32)> = ecken.iter().map(|&e| p.project(e)).collect();
        let (xs, ys): (Vec<f32>, Vec<f32>) = punkte.into_iter().unzip();
        let breite = xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min);
        let hoehe = ys.iter().cloned().fold(f32::MIN, f32::max)
            - ys.iter().cloned().fold(f32::MAX, f32::min);
        assert_eq!(breite, 16.0);
        assert_eq!(hoehe, 16.0);
    }

    #[test]
    fn oberseite_ist_eine_raute_im_verhaeltnis_zwei_zu_eins() {
        let p = Projection::new(32);
        let a = p.project([0.0, 1.0, 0.0]);
        let b = p.project([1.0, 1.0, 0.0]);
        let c = p.project([1.0, 1.0, 1.0]);
        let d = p.project([0.0, 1.0, 1.0]);
        assert_eq!(b.0 - d.0, 32.0, "Breite der Raute");
        assert_eq!(c.1 - a.1, 16.0, "Höhe der Raute");
    }

    /// Punkte auf der Blickachse fallen auf dasselbe Pixel.
    #[test]
    fn blickachse_ist_eins_eins_eins() {
        let p = Projection::new(16);
        let vorne = p.project([1.0, 1.0, 1.0]);
        let hinten = p.project([0.0, 0.0, 0.0]);
        assert_eq!(vorne, hinten);
        assert!(p.depth([1.0, 1.0, 1.0]) > p.depth([0.0, 0.0, 0.0]));
    }

    /// Occlusion verlangt, dass ein verdeckender Block nie tiefer liegt.
    #[test]
    fn hoehere_bloecke_sind_naeher() {
        let p = Projection::new(16);
        assert!(p.depth([0.0, 1.0, 0.0]) > p.depth([0.0, 0.0, 0.0]));
        assert!(p.depth([1.0, 0.0, 0.0]) > p.depth([0.0, 0.0, 0.0]));
        assert!(p.depth([0.0, 0.0, 1.0]) > p.depth([0.0, 0.0, 0.0]));
    }

    /// Je Kamera: Punkte entlang ihrer Achse landen auf demselben Pixel, die
    /// Tiefe wächst zur Kamera und fällt zu keinem der Nachbarn, die einen
    /// Block verdecken können. Jede Ecke liegt auf ganzen Pixeln.
    #[test]
    fn achse_jeder_kamera_steht_im_bild_still() {
        let kameras = [
            ("2:1", 32, [1.0, 1.0, 1.0]),
            ("16:9", 32, [8.0, 9.0, 8.0]),
            ("8:5", 32, [4.0, 5.0, 4.0]),
            ("4:3", 32, [2.0, 3.0, 2.0]),
            ("1:1", 32, [1.0, 2.0, 1.0]),
            ("top", 32, [0.0, 1.0, 0.0]),
            ("5:3", 30, [5.0, 6.0, 5.0]),
        ];
        for (kamera, scale, achse) in kameras {
            let p = Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap());
            assert_eq!(p.achse(), achse, "{kamera}");
            assert_eq!(p.project([0.0; 3]), p.project(achse), "{kamera}");
            assert!(p.depth(achse) > p.depth([0.0; 3]), "{kamera}");
            for nachbar in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                assert!(
                    p.depth(nachbar) >= p.depth([0.0; 3]),
                    "{kamera}, {nachbar:?}"
                );
            }
            assert!(p.ganze_pixel(), "{kamera} bei {scale}");
            for block in [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-7, 300, 13]] {
                let (x, y) = p.project_block(block);
                assert!(x.fract() == 0.0 && y.fract() == 0.0, "{kamera}, {block:?}");
            }
        }
    }

    /// 2:1 auf ganzen Pixeln genau bei Vielfachen von 4, von oben bei
    /// geradem scale.
    #[test]
    fn ganze_pixel_wie_die_alte_regel() {
        for scale in 2..=64 {
            assert_eq!(
                Projection::new(scale).ganze_pixel(),
                scale % 4 == 0,
                "{scale}"
            );
            let oben = Projection::mit_kamera(scale, Kamera::Oben);
            assert_eq!(oben.ganze_pixel(), scale % 2 == 0, "top bei {scale}");
        }
    }

    /// Der Schritt 2W gibt dieselben scales wie die Regel selbst: `a` ganz
    /// und der scale gerade, für jede gekürzte Raute bis 32:32.
    #[test]
    fn schritt_gleicht_der_regel() {
        for w in 1..=32u32 {
            for h in w.div_ceil(2)..=w {
                let kamera = Kamera::schraeg(w, h).unwrap();
                let Kamera::Schraeg(Raute { breite, hoehe }) = kamera else {
                    unreachable!()
                };
                for scale in 2..=256u32 {
                    let p = Projection::mit_kamera(scale, kamera);
                    let regel = scale % 2 == 0 && (scale * hoehe) % (2 * breite) == 0;
                    assert_eq!(p.ganze_pixel(), regel, "{kamera} bei {scale}");
                }
            }
        }
    }

    /// Grosse Rauten laufen nicht über: Die Regel lehnt sie ab, statt mit
    /// a = 0 weiterzurechnen.
    #[test]
    fn grosse_rauten_laufen_nicht_ueber() {
        let k = Kamera::parse("134217729:134217728").unwrap();
        let p = Projection::mit_kamera(32, k);
        assert!(!p.ganze_pixel());
        assert!(
            (p.a() - 16.0).abs() < 1e-6 && p.a() != 16.0,
            "a = {}",
            p.a()
        );
        assert_eq!(p.achse(), [134217729.0, 268435456.0, 134217729.0]);
        assert_eq!(
            Kamera::parse("4294967295:1"),
            Err("4294967295:1 ist flacher als 2:1".to_string())
        );
        assert_eq!(Kamera::parse("4294967295:4294967295"), Kamera::parse("1:1"));
    }

    #[test]
    fn scale_hat_eine_untergrenze() {
        assert_eq!(Projection::new(0).scale(), 2);
    }

    /// Jenseits von 2^24 unterscheidet f32 benachbarte Blöcke nicht mehr.
    /// project_block muss es trotzdem tun.
    #[test]
    fn weit_entfernte_nachbarn_fallen_nicht_zusammen() {
        let p = Projection::new(32);
        let weit = 1 << 24;
        let a = p.project_block([weit, 4, weit]);
        let b = p.project_block([weit + 1, 4, weit]);
        assert_ne!(a, b);
        assert_eq!(b.0 - a.0, 16.0);

        // Gegenprobe: genau das geht mit f32 schief.
        let f32_a = p.project([weit as f32, 4.0, weit as f32]);
        let f32_b = p.project([(weit + 1) as f32, 4.0, weit as f32]);
        assert_eq!(f32_a, f32_b, "f32 kann das hier nicht mehr");
    }

    /// Verschiebung um einen festen Vektor verschiebt das Bild genau
    /// mit — auch weit draussen.
    #[test]
    fn projektion_ist_verschiebungstreu() {
        let p = Projection::new(16);
        for anker in [0, 1 << 20, 1 << 24, 29_999_984] {
            let a = p.project_block([anker, 70, anker]);
            let b = p.project_block([anker + 3, 70, anker - 5]);
            assert_eq!((b.0 - a.0, b.1 - a.1), (64.0, -8.0), "bei {anker}");
        }
    }
}
