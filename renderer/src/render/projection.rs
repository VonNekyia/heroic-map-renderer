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

    /// Blockkoordinaten auf Bildschirmpixel abbilden.
    ///
    /// Rechnet in f64, anders als `project`. Minecraft erlaubt Koordinaten
    /// bis knapp 30 Millionen; ab 2^24 kann f32 benachbarte ganzzahlige
    /// Blöcke nicht mehr auseinanderhalten, und zwei Nachbarn landen auf
    /// demselben Pixel. Für Modellecken innerhalb eines Blocks reicht f32,
    /// für Weltkoordinaten nicht.
    pub fn project_block(&self, [x, y, z]: [i32; 3]) -> (f64, f64) {
        let half = self.scale as f64 / 2.0;
        let quarter = self.scale as f64 / 4.0;
        (
            (x as f64 - z as f64) * half,
            (x as f64 + z as f64) * quarter - y as f64 * half,
        )
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
