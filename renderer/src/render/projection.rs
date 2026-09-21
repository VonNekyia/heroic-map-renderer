/// Die feste isometrische Kamera.
///
/// ```text
/// screen_x = (x - z) * scale/2
/// screen_y = (x + z) * scale/4 - y * scale/2
/// ```
///
/// Damit belegt ein voller Würfel genau `scale` mal `scale` Pixel: die
/// Oberseite wird zur Raute von `scale` Breite und `scale/2` Höhe, die
/// Seitenflächen sind `scale/2` breit. Das ist die klassische 2:1-Isometrie
/// von Minecraft Overviewer und Dynmap.
///
/// Es gibt keine freie Kamera und keine Perspektive. Alle Faktoren stehen
/// hier und nirgendwo sonst.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// Pixelbreite eines Blocks.
    scale: u32,
}

impl Projection {
    /// 16 Pixel pro Block: Texturen erscheinen etwa halb so groß wie im
    /// Spiel, die Karte bleibt handlich. Siehe `--scale`.
    pub const DEFAULT_SCALE: u32 = 16;

    pub fn new(scale: u32) -> Projection {
        Projection {
            scale: scale.max(2),
        }
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Weltkoordinaten in Blockeinheiten auf Bildschirmpixel abbilden.
    pub fn project(&self, [x, y, z]: [f32; 3]) -> (f32, f32) {
        let half = self.scale as f32 / 2.0;
        let quarter = self.scale as f32 / 4.0;
        ((x - z) * half, (x + z) * quarter - y * half)
    }

    /// Tiefe entlang der Blickachse. Größer heißt näher an der Kamera.
    ///
    /// Die Blickrichtung ist (1, 1, 1): genau die Punkte, die sich um ein
    /// Vielfaches davon unterscheiden, landen auf demselben Pixel.
    pub fn depth([x, y, z]: [f32; 3]) -> f32 {
        x + y + z
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
        assert!(Projection::depth([1.0, 1.0, 1.0]) > Projection::depth([0.0, 0.0, 0.0]));
    }

    /// Occlusion verlangt, dass ein verdeckender Block nie tiefer liegt.
    #[test]
    fn hoehere_bloecke_sind_naeher() {
        assert!(Projection::depth([0.0, 1.0, 0.0]) > Projection::depth([0.0, 0.0, 0.0]));
        assert!(Projection::depth([1.0, 0.0, 0.0]) > Projection::depth([0.0, 0.0, 0.0]));
        assert!(Projection::depth([0.0, 0.0, 1.0]) > Projection::depth([0.0, 0.0, 0.0]));
    }

    #[test]
    fn scale_hat_eine_untergrenze() {
        assert_eq!(Projection::new(0).scale(), 2);
    }
}
