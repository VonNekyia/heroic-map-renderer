use crate::assets::Face;

/// Die Kamera eines Laufs: diagonal schräg mit der Raute W:H oder von oben,
/// oder genordet von oben oder schräg von Süden.
///
/// ```text
/// screen_x = u * h
/// screen_y = v * a - y * b
///
/// diagonal:  u = x - z,  v = x + z
/// genordet:  u = x,      v = z
/// ```
///
/// Diagonal ist `h` scale/2. Schräg ist `a` = scale · H/(2W) und `b` =
/// scale/2, die Wände bleiben bei jeder Raute so hoch. Von oben ist `a` =
/// scale/2 und `b` = 0. 2:1 ist die Vorgabe: `a` = scale/4, ein voller
/// Würfel belegt dann genau `scale` mal `scale` Pixel. Genordet sind `h`
/// und `a` scale, `b` ist scale bei `north-45` und 0 bei `top-north`.
///
/// Es gibt keine freie Kamera und keine Perspektive. Alle Faktoren stehen
/// hier und nirgendwo sonst.
/// Siehe docs/renderer/kamera.md, „Projektion“.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// Pixelbreite eines Blocks.
    scale: u32,
    kamera: Kamera,
    /// Wo die Kamera steht. Alle Rechnungen dieser Datei gelten im Blick;
    /// in die Welt dreht [`Richtung`].
    richtung: Richtung,
}

/// Diagonal schräg mit einer Raute W:H oder von oben; genordet, Norden
/// oben, von oben oder schräg von Süden.
/// Siehe docs/renderer/kamera.md, „Kameras“.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kamera {
    Schraeg(Raute),
    Oben,
    /// `top-north`: von oben, Norden oben.
    ObenNord,
    /// `north-45`: von Süden, 45° hoch.
    Nord45,
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

    /// `W:H`, `top`, `top-north` oder `north-45`. W:H wird gekürzt und muss
    /// zwischen 2:1 und 1:1 liegen: flacher verdeckt das Gelände mehr,
    /// steiler erschiene die Oberseite höher als von oben.
    pub fn parse(text: &str) -> Result<Kamera, String> {
        match text {
            "top" => return Ok(Kamera::Oben),
            "top-north" => return Ok(Kamera::ObenNord),
            "north-45" => return Ok(Kamera::Nord45),
            _ => {}
        }
        let (w, h) = text
            .split_once(':')
            .and_then(|(w, h)| Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?)))
            .filter(|&(w, h)| w > 0 && h > 0)
            .ok_or_else(|| {
                format!(
                    "{text} ist keine Kamera: W:H mit ganzen Zahlen über 0, top, top-north \
                     oder north-45"
                )
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
    /// genau dann ganz und der scale gerade; von oben 2; genordet 1.
    /// Siehe docs/renderer/kamera.md, „Ganze Pixel“.
    pub fn schritt(self) -> u64 {
        match self {
            Kamera::Schraeg(Raute { breite, .. }) => 2 * u64::from(breite),
            Kamera::Oben => 2,
            Kamera::ObenNord | Kamera::Nord45 => 1,
        }
    }

    /// Genordet, mit u = x und v = z? Sonst diagonal, u = x − z und
    /// v = x + z.
    pub fn genordet(self) -> bool {
        matches!(self, Kamera::ObenNord | Kamera::Nord45)
    }

    /// Der scale ohne `--scale`: genordet 16, dort ist jedes Texel einer
    /// Oberseite schon ein Pixel; sonst [`Projection::DEFAULT_SCALE`].
    /// Siehe docs/renderer/kamera.md, „Genordet“.
    pub fn vorgabe_scale(self) -> u32 {
        if self.genordet() {
            16
        } else {
            Projection::DEFAULT_SCALE
        }
    }

    /// Der Azimut wie `projection.azimuth` in `map.json`: `diagonal` oder
    /// `north`.
    pub fn azimut(self) -> &'static str {
        if self.genordet() { "north" } else { "diagonal" }
    }

    /// `h` und `a` je scale als Bruch: diagonal h = 1/2 und a schräg
    /// H/(2W), von oben 1/2; genordet beide 1.
    fn h_a_je_scale(self) -> ((u64, u64), (u64, u64)) {
        match self {
            Kamera::Schraeg(Raute { breite, hoehe }) => {
                ((1, 2), (hoehe.into(), 2 * u64::from(breite)))
            }
            Kamera::Oben => ((1, 2), (1, 2)),
            Kamera::ObenNord | Kamera::Nord45 => ((1, 1), (1, 1)),
        }
    }
}

impl std::fmt::Display for Kamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kamera::Schraeg(Raute { breite, hoehe }) => write!(f, "{breite}:{hoehe}"),
            Kamera::Oben => write!(f, "top"),
            Kamera::ObenNord => write!(f, "top-north"),
            Kamera::Nord45 => write!(f, "north-45"),
        }
    }
}

/// Wo die Kamera steht, als Zahl der Vierteldrehungen von der Vorgabe aus,
/// im Uhrzeigersinn: diagonal `se`, `sw`, `nw`, `ne`, genordet `s`, `w`,
/// `n`, `e`. Der Renderer dreht dafür die Welt: Im Blick steht die Kamera
/// immer bei +x, +z wie aus der Vorgabe.
/// Siehe docs/renderer/kamera.md, „Richtungen“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Richtung(u8);

impl Richtung {
    const DIAGONAL: [&'static str; 4] = ["se", "sw", "nw", "ne"];
    const GENORDET: [&'static str; 4] = ["s", "w", "n", "e"];

    /// Die Richtung `text` für `kamera`: diagonal eine Ecke, genordet eine
    /// Seite.
    pub fn parse(text: &str, kamera: Kamera) -> Result<Richtung, String> {
        let namen = Richtung::namen(kamera);
        let liste = format!("{} oder {}", namen[..3].join(", "), namen[3]);
        match namen.iter().position(|&name| name == text) {
            Some(k) => Ok(Richtung(k as u8)),
            None if kamera.genordet() => Err(format!("{kamera} schaut von einer Seite: {liste}")),
            None => Err(format!("{kamera} schaut über eine Ecke: {liste}")),
        }
    }

    fn namen(kamera: Kamera) -> [&'static str; 4] {
        if kamera.genordet() {
            Richtung::GENORDET
        } else {
            Richtung::DIAGONAL
        }
    }

    /// Der Name wie `direction` in `map.json`.
    pub fn name(self, kamera: Kamera) -> &'static str {
        Richtung::namen(kamera)[self.0 as usize]
    }

    /// Vierteldrehungen von der Vorgabe aus, 0 bis 3.
    pub fn vierteldrehungen(self) -> u8 {
        self.0
    }

    /// Wo der Block `(x, z)` der Welt im Blick liegt. Dieselbe Formel gilt
    /// für Chunks und für die Zellen der Biome; eine Vierteldrehung ist
    /// `(x, z)` nach `(z, −x − 1)`.
    pub fn in_den_blick(self, [x, z]: [i32; 2]) -> [i32; 2] {
        match self.0 {
            0 => [x, z],
            1 => [z, -x - 1],
            2 => [-x - 1, -z - 1],
            _ => [-z - 1, x],
        }
    }

    /// Wo der Block `(x, z)` im Blick in der Welt liegt.
    pub fn in_die_welt(self, blick: [i32; 2]) -> [i32; 2] {
        self.zurueck().in_den_blick(blick)
    }

    /// Die Drehung zurück in die Welt.
    fn zurueck(self) -> Richtung {
        Richtung((4 - self.0) % 4)
    }

    /// Wohin ein Versatz der Welt im Blick zeigt, oder ein Punkt der Welt
    /// mit stetigen Koordinaten, etwa die Ecke eines Kastens: eine
    /// Vierteldrehung ist `(x, z)` nach `(z, −x)`.
    pub fn versatz_in_den_blick(self, [x, y, z]: [i32; 3]) -> [i32; 3] {
        match self.0 {
            0 => [x, y, z],
            1 => [z, y, -x],
            2 => [-x, y, -z],
            _ => [-z, y, x],
        }
    }

    /// Wohin ein Versatz im Blick in der Welt zeigt.
    pub fn versatz_in_die_welt(self, blick: [i32; 3]) -> [i32; 3] {
        self.zurueck().versatz_in_den_blick(blick)
    }

    /// Wohin die Normale einer Fläche der Welt im Blick zeigt, wie
    /// [`Richtung::versatz_in_den_blick`].
    pub fn normale_in_den_blick(self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        match self.0 {
            0 => [x, y, z],
            1 => [z, y, -x],
            2 => [-x, y, -z],
            _ => [-z, y, x],
        }
    }

    /// Wo ein Punkt eines Modells im Blick liegt, in Blockbreiten vom
    /// Ursprung seines Blocks: gedreht um die Mitte des Blocks.
    pub fn punkt_in_den_blick(self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        match self.0 {
            0 => [x, y, z],
            1 => [z, y, 1.0 - x],
            2 => [1.0 - x, y, 1.0 - z],
            _ => [1.0 - z, y, x],
        }
    }

    /// Welche Seite eine Seite der Welt im Blick ist.
    pub fn seite_in_den_blick(self, seite: Face) -> Face {
        const RUNDUM: [Face; 4] = [Face::North, Face::East, Face::South, Face::West];
        match RUNDUM.iter().position(|&s| s == seite) {
            // Im Blick dreht sich die Welt gegen den Uhrzeigersinn.
            Some(i) => RUNDUM[(i + 4 - self.0 as usize) % 4],
            None => seite,
        }
    }

    /// Welche Seite eine Seite im Blick in der Welt ist.
    pub fn seite_in_die_welt(self, seite: Face) -> Face {
        self.zurueck().seite_in_den_blick(seite)
    }
}

/// Der kleinste scale einer Kamera, siehe [`Projection::mit_kamera`].
fn kleinster_scale(kamera: Kamera) -> u32 {
    match kamera {
        Kamera::ObenNord => 1,
        _ => 2,
    }
}

fn ggt(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { ggt(b, a % b) }
}

impl Projection {
    /// 32 Pixel je Block: Diagonal ist eine Seitenfläche halb so breit wie
    /// der Würfel, erst so zeigt sie alle 16 Texel einer Textur. Bei 16 fiele
    /// jede zweite Texelspalte weg; dafür sind es viermal so viele
    /// Kacheln. Genordet zeigt schon scale 16 jedes Texel, dort ist das die
    /// Vorgabe ([`Kamera::vorgabe_scale`]). Siehe `--scale`.
    pub const DEFAULT_SCALE: u32 = 32;

    /// 2:1, die Vorgabe.
    pub fn new(scale: u32) -> Projection {
        Projection::mit_kamera(scale, Kamera::ZWEI_ZU_EINS)
    }

    /// Die Kamera in ihrer Vorgabe-Richtung, bei mindestens scale 2; nur
    /// `top-north` geht bis scale 1, die einfarbige Ansicht.
    /// Siehe docs/renderer/einfarbig.md.
    pub fn mit_kamera(scale: u32, kamera: Kamera) -> Projection {
        Projection {
            scale: scale.max(kleinster_scale(kamera)),
            kamera,
            richtung: Richtung::default(),
        }
    }

    /// Die einfarbige Ansicht von `--flat`: `top-north` bei scale 1, ein
    /// Pixel je Block.
    pub fn flach(&self) -> bool {
        self.kamera == Kamera::ObenNord && self.scale == 1
    }

    /// Dieselbe Projektion aus einer anderen Richtung.
    pub fn aus(self, richtung: Richtung) -> Projection {
        Projection { richtung, ..self }
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn kamera(&self) -> Kamera {
        self.kamera
    }

    pub fn richtung(&self) -> Richtung {
        self.richtung
    }

    /// Dieselbe Kamera aus derselben Richtung bei einem anderen scale, etwa
    /// für eine native Stufe.
    pub fn bei(&self, scale: u32) -> Projection {
        Projection {
            scale: scale.max(kleinster_scale(self.kamera)),
            ..*self
        }
    }

    /// Pixel je Schritt in `u`: diagonal scale/2, genordet scale.
    pub fn h(&self) -> f64 {
        let ((zaehler, nenner), _) = self.kamera.h_a_je_scale();
        (u64::from(self.scale) * zaehler) as f64 / nenner as f64
    }

    /// Pixel je Schritt in `v`.
    pub fn a(&self) -> f64 {
        let (_, (zaehler, nenner)) = self.kamera.h_a_je_scale();
        (u64::from(self.scale) * zaehler) as f64 / nenner as f64
    }

    /// Pixel je Block Höhe: diagonal schräg scale/2, `north-45` scale, von
    /// oben 0.
    pub fn b(&self) -> f64 {
        match self.kamera {
            Kamera::Schraeg(_) => self.scale as f64 / 2.0,
            Kamera::Nord45 => self.scale as f64,
            Kamera::Oben | Kamera::ObenNord => 0.0,
        }
    }

    /// Pixel, die die Oberseite eines Blocks bedeckt: die Fläche zwischen den
    /// Schritten nach +x und nach +z, diagonal die Raute, genordet das
    /// Quadrat.
    pub fn oberseite(&self) -> f64 {
        let o = self.project_block([0, 0, 0]);
        let x = self.project_block([1, 0, 0]);
        let z = self.project_block([0, 0, 1]);
        ((x.0 - o.0) * (z.1 - o.1) - (x.1 - o.1) * (z.0 - o.0)).abs()
    }

    /// Welche Nachbarn beim Verdecken ihren ganzen Umriss decken müssen,
    /// nach +x und nach +z: die, deren Umriss den eigenen überlappt.
    /// Diagonal schräg beide, genordet schräg nur der nach +z, von oben
    /// keiner. Den Boden deckt immer der Block darüber.
    /// Siehe docs/renderer/sprites-und-deckung.md, „Verdeckte Würfel“.
    pub fn verdeckende_seiten(&self) -> (bool, bool) {
        let schraeg = self.b() > 0.0;
        (schraeg && !self.kamera.genordet(), schraeg)
    }

    /// Die Bildachsen einer Blockspalte: diagonal `(x - z, x + z)`,
    /// genordet `(x, z)`.
    pub fn uv(&self, x: i32, z: i32) -> (i32, i32) {
        if self.kamera.genordet() {
            (x, z)
        } else {
            (x - z, x + z)
        }
    }

    /// Liegt jede Blockecke auf ganzen Pixeln? Genau dann, wenn der scale ein
    /// Vielfaches von [`Kamera::schritt`] ist. Diagonal heisst das: `a` ganz
    /// und der scale gerade, bei 2:1 ein Vielfaches von 4. Genordet geht
    /// jeder scale.
    /// Siehe docs/renderer/kamera.md, „Ganze Pixel“.
    pub fn ganze_pixel(&self) -> bool {
        u64::from(self.scale).is_multiple_of(self.kamera.schritt())
    }

    /// Die Blickachse, gekürzt auf ganze teilerfremde Zahlen: diagonal
    /// (b, 2a, b), 2:1 (1, 1, 1), 4:3 (2, 3, 2), 1:1 (1, 2, 1), von oben
    /// (0, 1, 0); genordet (0, a, b), `north-45` (0, 1, 1), `top-north`
    /// (0, 1, 0). Punkte, die sich um ein Vielfaches davon unterscheiden,
    /// landen auf demselben Pixel.
    pub fn achse(&self) -> [f32; 3] {
        let [x, y, z] = match self.kamera {
            // (scale/2, scale·H/W, scale/2) ∝ (W, 2H, W)
            Kamera::Schraeg(Raute { breite, hoehe }) => {
                [u64::from(breite), 2 * u64::from(hoehe), u64::from(breite)]
            }
            Kamera::Oben | Kamera::ObenNord => [0, 1, 0],
            Kamera::Nord45 => [0, 1, 1],
        };
        let g = ggt(ggt(x, y), z);
        [x / g, y / g, z / g].map(|c| c as f32)
    }

    /// Weltkoordinaten in Blockeinheiten auf Bildschirmpixel abbilden.
    pub fn project(&self, [x, y, z]: [f32; 3]) -> (f32, f32) {
        let (h, a, b) = (self.h() as f32, self.a() as f32, self.b() as f32);
        let (u, v) = if self.kamera.genordet() {
            (x, z)
        } else {
            (x - z, x + z)
        };
        (u * h, v * a - y * b)
    }

    /// Blockkoordinaten auf Bildschirmpixel abbilden, in f64 anders als
    /// `project`: Für Modellecken innerhalb eines Blocks reicht f32, für
    /// Weltkoordinaten nicht.
    /// Siehe docs/renderer/kamera.md, „Weltkoordinaten in f64“.
    pub fn project_block(&self, [x, y, z]: [i32; 3]) -> (f64, f64) {
        let (x, y, z) = (x as f64, y as f64, z as f64);
        let (u, v) = if self.kamera.genordet() {
            (x, z)
        } else {
            (x - z, x + z)
        };
        (u * self.h(), v * self.a() - y * self.b())
    }

    /// Tiefe entlang der Blickachse ([`Projection::achse`]). Größer heißt
    /// näher an der Kamera.
    pub fn depth(&self, [x, y, z]: [f32; 3]) -> f32 {
        let [ax, ay, az] = self.achse();
        x * ax + y * ay + z * az
    }

    /// Die Tiefe des Ursprungs von Block `block` wie [`Projection::depth`],
    /// in f64: genau auch weit draussen.
    pub fn depth_block(&self, block: [i32; 3]) -> f64 {
        let achse = self.achse();
        (0..3)
            .map(|k| f64::from(block[k]) * f64::from(achse[k]))
            .sum()
    }

    /// Der Punkt, den [`Projection::project`] auf `(x, y)` abbildet und der
    /// die Tiefe `tiefe` hat ([`Projection::depth`]), siehe
    /// [`Projection::umkehrung`].
    pub fn punkt(&self, xy: (f64, f64), tiefe: f64) -> [f64; 3] {
        self.umkehrung().punkt(xy, tiefe)
    }

    /// Die Umkehrung von [`Projection::project`] und [`Projection::depth`]
    /// zusammen: die inverse Matrix, die Adjunkte durch die Determinante.
    /// Einmal gerechnet für viele Punkte.
    pub fn umkehrung(&self) -> Umkehrung {
        let (h, a, b) = (self.h(), self.a(), self.b());
        let [ax, ay, az] = self.achse().map(f64::from);
        let m = if self.kamera.genordet() {
            [[h, 0.0, 0.0], [0.0, -b, a], [ax, ay, az]]
        } else {
            [[h, 0.0, -h], [a, -b, a], [ax, ay, az]]
        };
        // Mit zyklischen Indizes trägt der Kofaktor sein Vorzeichen schon.
        let ko = |i: usize, j: usize| {
            let (i1, i2, j1, j2) = ((i + 1) % 3, (i + 2) % 3, (j + 1) % 3, (j + 2) % 3);
            m[i1][j1] * m[i2][j2] - m[i1][j2] * m[i2][j1]
        };
        let det: f64 = (0..3).map(|j| m[0][j] * ko(0, j)).sum();
        Umkehrung(std::array::from_fn(|i| {
            std::array::from_fn(|j| ko(j, i) / det)
        }))
    }
}

/// Die Umkehrung von Projektion und Tiefe, siehe [`Projection::umkehrung`].
#[derive(Clone, Copy, Debug)]
pub struct Umkehrung([[f64; 3]; 3]);

impl Umkehrung {
    /// Der Punkt auf dem Bildpunkt `(x, y)` mit der Tiefe `tiefe`.
    pub fn punkt(&self, (x, y): (f64, f64), tiefe: f64) -> [f64; 3] {
        self.0.map(|z| z[0] * x + z[1] * y + z[2] * tiefe)
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
    fn punkt_kehrt_projektion_und_tiefe_um() {
        let kameras = [
            Kamera::ZWEI_ZU_EINS,
            Kamera::schraeg(4, 3).unwrap(),
            Kamera::schraeg(1, 1).unwrap(),
            Kamera::Oben,
            Kamera::ObenNord,
            Kamera::Nord45,
        ];
        for kamera in kameras {
            let p = Projection::mit_kamera(32, kamera);
            for q in [[0.25f32, 0.5, 0.75], [1.0, 0.0, 0.3], [0.9, 1.0, 0.1]] {
                let (x, y) = p.project(q);
                let r = p.punkt((x.into(), y.into()), p.depth(q).into());
                for k in 0..3 {
                    assert!(
                        (r[k] - f64::from(q[k])).abs() < 1e-5,
                        "{kamera}: {q:?} → {r:?}"
                    );
                }
            }
        }
    }

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
            ("top-north", 16, [0.0, 1.0, 0.0]),
            ("north-45", 16, [0.0, 1.0, 1.0]),
            ("top-north", 7, [0.0, 1.0, 0.0]),
            ("north-45", 7, [0.0, 1.0, 1.0]),
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
            for kamera in [Kamera::ObenNord, Kamera::Nord45] {
                assert!(Projection::mit_kamera(scale, kamera).ganze_pixel());
            }
        }
    }

    /// Genordet ist Norden oben und Osten rechts. Bei `north-45` sind
    /// Oberseite und Südwand je scale mal scale, bei `top-north` die
    /// Oberseite; Ost- und Westwand haben keine Breite.
    #[test]
    fn genordet_sind_oberseite_und_suedwand_quadrate() {
        // Die Oberseite: 2:1 bei 32 so gross wie genordet bei 16, die
        // übrigen Rauten im Verhältnis ihrer Höhe.
        let flaeche = |scale, kamera: &str| {
            Projection::mit_kamera(scale, Kamera::parse(kamera).unwrap()).oberseite()
        };
        assert_eq!(flaeche(32, "2:1"), 256.0);
        assert_eq!(flaeche(32, "8:5"), 320.0);
        assert_eq!(flaeche(32, "4:3"), 384.0);
        assert_eq!(flaeche(32, "1:1"), 512.0);
        assert_eq!(flaeche(32, "top"), 512.0);
        assert_eq!(flaeche(16, "top-north"), 256.0);
        assert_eq!(flaeche(16, "north-45"), 256.0);
        for (kamera, b) in [(Kamera::Nord45, 16.0), (Kamera::ObenNord, 0.0)] {
            let p = Projection::mit_kamera(16, kamera);
            let bild = |e: [i32; 3]| p.project_block(e);
            assert_eq!(bild([1, 0, 0]).0 - bild([0, 0, 0]).0, 16.0, "Osten rechts");
            assert_eq!(bild([0, 0, 1]).1 - bild([0, 0, 0]).1, 16.0, "Süden unten");
            assert_eq!(bild([0, 0, 0]).1 - bild([0, 1, 0]).1, b, "Höhe");
            assert_eq!(
                bild([0, 0, 1]).0,
                bild([0, 0, 0]).0,
                "keine Ost- und Westwand"
            );
            assert_eq!((p.h(), p.a(), p.b()), (16.0, 16.0, b));
            assert_eq!(p.uv(3, -5), (3, -5));
            assert_eq!(Kamera::parse(&kamera.to_string()), Ok(kamera));
            let vorgabe = Richtung::default().name(kamera);
            assert_eq!((vorgabe, kamera.azimut()), ("s", "north"));
        }
        let diagonal = Kamera::ZWEI_ZU_EINS;
        let vorgabe = Richtung::default().name(diagonal);
        assert_eq!((vorgabe, diagonal.azimut()), ("se", "diagonal"));
        assert_eq!(Projection::new(16).uv(3, -5), (8, -2));
    }

    /// Die Tabelle aus #68: wo der Block (x, z) der Welt im Blick liegt, je
    /// Richtung, mit den Namen diagonal und genordet. `in_die_welt` kehrt
    /// `in_den_blick` um, und die Formel für Blöcke gilt auch für Chunks:
    /// Der Chunk eines Blocks im Blick ist der gedrehte Chunk seines Blocks
    /// in der Welt.
    #[test]
    fn richtungen_drehen_die_welt_wie_die_tabelle() {
        let (x, z) = (5, -7);
        for (k, diagonal, genordet, blick) in [
            (0, "se", "s", [x, z]),
            (1, "sw", "w", [z, -x - 1]),
            (2, "nw", "n", [-x - 1, -z - 1]),
            (3, "ne", "e", [-z - 1, x]),
        ] {
            let schraeg = Richtung::parse(diagonal, Kamera::ZWEI_ZU_EINS).unwrap();
            assert_eq!(schraeg, Richtung::parse(genordet, Kamera::Nord45).unwrap());
            assert_eq!(schraeg.vierteldrehungen(), k);
            assert_eq!(schraeg.name(Kamera::Oben), diagonal);
            assert_eq!(schraeg.name(Kamera::ObenNord), genordet);
            assert_eq!(schraeg.in_den_blick([x, z]), blick, "{diagonal}");
            for px in -40..40 {
                for pz in [-33, -17, -16, -1, 0, 15, 16, 31] {
                    let welt = [px, pz];
                    let im_blick = schraeg.in_den_blick(welt);
                    assert_eq!(schraeg.in_die_welt(im_blick), welt, "{diagonal}");
                    assert_eq!(
                        im_blick.map(|c| c >> 4),
                        schraeg.in_den_blick(welt.map(|c| c >> 4)),
                        "{diagonal}: Chunk von {welt:?}"
                    );
                }
            }
        }
        assert_eq!(
            Richtung::default(),
            Richtung::parse("se", Kamera::ZWEI_ZU_EINS).unwrap()
        );
    }

    /// Eine Richtung, die nicht zur Kamera passt, nennt die vier, die gehen.
    #[test]
    fn falsche_richtung_nennt_die_vier() {
        let schraeg = Kamera::parse("8:5").unwrap();
        assert_eq!(
            Richtung::parse("n", schraeg),
            Err("8:5 schaut über eine Ecke: se, sw, nw oder ne".to_string())
        );
        assert_eq!(
            Richtung::parse("ne", Kamera::Nord45),
            Err("north-45 schaut von einer Seite: s, w, n oder e".to_string())
        );
        assert!(Richtung::parse("SE", Kamera::Oben).is_err());
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

    /// Jede Kamera ausser `top-north` bleibt bei mindestens scale 2, auch
    /// über [`Projection::bei`]; nur `top-north` geht bis 1.
    #[test]
    fn scale_hat_eine_untergrenze() {
        assert_eq!(Projection::new(0).scale(), 2);
        for kamera in ["2:1", "4:3", "1:1", "top", "north-45"] {
            let kamera = Kamera::parse(kamera).unwrap();
            for scale in [0, 1] {
                let p = Projection::mit_kamera(scale, kamera);
                assert_eq!(p.scale(), 2, "{kamera} bei {scale}");
                assert_eq!(Projection::mit_kamera(8, kamera).bei(scale).scale(), 2);
                assert!(!p.flach(), "{kamera}");
            }
        }
        let nord = Projection::mit_kamera(1, Kamera::ObenNord);
        assert_eq!(nord.scale(), 1);
        assert!(nord.flach());
        assert_eq!(Projection::mit_kamera(0, Kamera::ObenNord).scale(), 1);
        assert!(!Projection::mit_kamera(2, Kamera::ObenNord).flach());
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
