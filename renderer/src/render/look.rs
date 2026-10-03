//! Die Werte des Looks von Cinematic an einer Stelle, wie in 0058.
//! Siehe docs/renderer/cinematic.md, „Werte des Looks“.

use super::Kamera;

/// Die Werte, mit denen Cinematic zeichnet, benannt wie in 0058. Alle gehen
/// in den Fingerabdruck eines Baums ein, siehe [`Look::fingerabdruck`]: Wer
/// einen ändert, rendert die Bäume mit Cinematic neu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// Stärke des Himmelslichts.
    pub himmel: f32,
    /// Anteil der Farbe des Himmels am Himmelslicht, der Rest ist die des
    /// Nebels, linear gemischt.
    pub himmel_anteil: f32,
    /// Stärke des Blocklichts.
    pub block: f32,
    /// Stärke des Leuchtens leuchtender Blöcke, siehe [`Look::leuchtet`].
    pub leuchten: f32,
    /// Leuchten: bis zu dieser Helligkeit des hellsten Kanals, linear,
    /// leuchtet ein Texel nicht.
    pub leuchten_ab: f32,
    /// Leuchten: ab dieser Helligkeit leuchtet ein Texel ganz.
    pub leuchten_voll: f32,
    /// Stärke der Sonne.
    pub sonne: f32,
    /// Farbe der Sonne, linear.
    pub sonne_farbe: [f32; 3],
    /// Höhe der Sonne über dem Horizont, in Grad.
    pub sonne_hoehe: f32,
    /// Wie weit die Sonne waagrecht von links zur Kamera hin gedreht steht,
    /// in Grad.
    pub sonne_seite: f32,
    /// Wie weit ein Strahl zur Sonne reicht, in Blöcken entlang des Strahls.
    pub sonne_weite: f32,
    /// So viel Sonne lässt eine Bodenpflanze auf dem Strahl durch.
    pub pflanzen: f32,
    /// Wasser: F0 der Spiegelung nach Fresnel.
    pub wasser_spiegel: f32,
    /// Wasser: so viel seiner Deckkraft behält seine Textur.
    pub wasser_textur: f32,
    /// Wasser: Dichte, durch die die Farbe nach der Strecke bis zum Grund
    /// geteilt wird.
    pub wasser_dichte: f32,
    /// Wasser: Bis zu dieser Höhe der gespiegelten Richtung (y, Länge 1)
    /// spiegelt es nur den Nebel.
    pub wasser_horizont: f32,
    /// Wasser: Über so viel Höhe darüber geht der Nebel weich in den Himmel
    /// über.
    pub wasser_horizont_breite: f32,
    /// Wasser: Mit mindestens diesem Anteil am stärksten Kanal seiner Farbe
    /// zählt ein Kanal für die Dichte.
    pub wasser_anteil_min: f32,
    /// Wasser: so viel Dichte hat jeder Kanal dazu, auch der stärkste, vor
    /// der Teilung durch [`Look::wasser_dichte`].
    pub wasser_dichte_grund: f32,
    /// Wärme: so viel stärker wird der Weissabgleich höchstens, siehe
    /// [`Look::waerme`].
    pub waerme: f32,
    /// Wärme: ab dieser Temperatur des Bioms wird es wärmer.
    pub waerme_von: f32,
    /// Wärme: ab dieser Temperatur ist es ganz warm.
    pub waerme_bis: f32,
    /// Belichtung vor der Kurve.
    pub belichtung: f32,
    /// Bis hierher ist die Kurve eine Gerade.
    pub knie: f32,
    /// Ab hier ist die Kurve flach in 1.
    pub flach: f32,
    /// Stärke des Bloom aus dem Leuchten.
    pub bloom: f32,
    /// Breite des Bloom: σ der Gaussglocke in Blöcken, also in Pixeln mal
    /// scale.
    pub bloom_breite: f32,
}

/// Der Stand des Verfahrens, mit dem Cinematic zeichnet. Er geht in den
/// Fingerabdruck ein: Wer das Bild bei gleichen Werten ändert, erhöht ihn.
/// Siehe docs/benutzung/map-json.md, „Look“.
pub const VERFAHREN: u32 = 2;

/// Der Look aus 0058.
pub const LOOK: Look = Look {
    himmel: 3.0,
    himmel_anteil: 0.75,
    block: 1.5,
    leuchten: 2.0,
    leuchten_ab: 0.25,
    leuchten_voll: 0.75,
    sonne: 3.0,
    sonne_farbe: [1.0, 0.93, 0.83],
    sonne_hoehe: 48.47,
    sonne_seite: 8.75,
    sonne_weite: 128.0,
    pflanzen: 0.5,
    wasser_spiegel: 0.04,
    wasser_textur: 0.6,
    wasser_dichte: 8.0,
    wasser_horizont: -0.1,
    wasser_horizont_breite: 0.7,
    wasser_anteil_min: 0.02,
    wasser_dichte_grund: 0.35,
    waerme: 0.5,
    waerme_von: 0.5,
    waerme_bis: 1.0,
    belichtung: 0.25,
    knie: 0.8,
    flach: 1.2,
    bloom: 1.0,
    bloom_breite: 0.25,
};

impl Look {
    /// Jeder Wert mit seinem Namen, in fester Reihenfolge, wie er im Code
    /// steht. Abgeleitete Werte wie die Richtung der Sonne aus Sinus und
    /// Kosinus fehlen: Deren letztes Bit kann je System abweichen.
    fn werte(&self) -> [(&'static str, &[f32]); 27] {
        // Ganz zerlegt: Ein neues Feld kompiliert erst, wenn es hier steht.
        let Look {
            himmel,
            himmel_anteil,
            block,
            leuchten,
            leuchten_ab,
            leuchten_voll,
            sonne,
            sonne_farbe,
            sonne_hoehe,
            sonne_seite,
            sonne_weite,
            pflanzen,
            wasser_spiegel,
            wasser_textur,
            wasser_dichte,
            wasser_horizont,
            wasser_horizont_breite,
            wasser_anteil_min,
            wasser_dichte_grund,
            waerme,
            waerme_von,
            waerme_bis,
            belichtung,
            knie,
            flach,
            bloom,
            bloom_breite,
        } = self;
        [
            ("himmel", std::slice::from_ref(himmel)),
            ("himmel_anteil", std::slice::from_ref(himmel_anteil)),
            ("block", std::slice::from_ref(block)),
            ("leuchten", std::slice::from_ref(leuchten)),
            ("leuchten_ab", std::slice::from_ref(leuchten_ab)),
            ("leuchten_voll", std::slice::from_ref(leuchten_voll)),
            ("sonne", std::slice::from_ref(sonne)),
            ("sonne_farbe", sonne_farbe.as_slice()),
            ("sonne_hoehe", std::slice::from_ref(sonne_hoehe)),
            ("sonne_seite", std::slice::from_ref(sonne_seite)),
            ("sonne_weite", std::slice::from_ref(sonne_weite)),
            ("pflanzen", std::slice::from_ref(pflanzen)),
            ("wasser_spiegel", std::slice::from_ref(wasser_spiegel)),
            ("wasser_textur", std::slice::from_ref(wasser_textur)),
            ("wasser_dichte", std::slice::from_ref(wasser_dichte)),
            ("wasser_horizont", std::slice::from_ref(wasser_horizont)),
            (
                "wasser_horizont_breite",
                std::slice::from_ref(wasser_horizont_breite),
            ),
            ("wasser_anteil_min", std::slice::from_ref(wasser_anteil_min)),
            (
                "wasser_dichte_grund",
                std::slice::from_ref(wasser_dichte_grund),
            ),
            ("waerme", std::slice::from_ref(waerme)),
            ("waerme_von", std::slice::from_ref(waerme_von)),
            ("waerme_bis", std::slice::from_ref(waerme_bis)),
            ("belichtung", std::slice::from_ref(belichtung)),
            ("knie", std::slice::from_ref(knie)),
            ("flach", std::slice::from_ref(flach)),
            ("bloom", std::slice::from_ref(bloom)),
            ("bloom_breite", std::slice::from_ref(bloom_breite)),
        ]
    }

    /// Wie weit der Bloom bei `scale` reicht, siehe [`super::kino::Kino::bloom_radius`].
    pub fn bloom_radius(&self, scale: u32) -> usize {
        if self.bloom <= 0.0 {
            return 0;
        }
        let sigma = f64::from(self.bloom_breite) * f64::from(scale);
        let b = ((4.0 * sigma * sigma + 1.0).sqrt().round() as usize).max(1);
        (b | 1) / 2
    }

    /// Der Fingerabdruck der Werte für `lookHash` in `map.json`: FNV-1a mit
    /// 64 Bit über jeden Namen, ein Nullbyte und die Bits jedes Werts in
    /// Little Endian, dann ebenso `verfahren` mit [`VERFAHREN`] als u32, als
    /// 16 kleine Hexzeichen.
    /// Siehe docs/benutzung/map-json.md, „Look“.
    pub fn fingerabdruck(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut nimm = |bytes: &[u8]| {
            for &b in bytes {
                hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        for (name, werte) in self.werte() {
            nimm(name.as_bytes());
            nimm(&[0]);
            for wert in werte {
                nimm(&wert.to_bits().to_le_bytes());
            }
        }
        nimm(b"verfahren");
        nimm(&[0]);
        nimm(&VERFAHREN.to_le_bytes());
        format!("{hash:016x}")
    }

    /// Die Richtung zur Sonne im Blick, Länge 1: fest zur Kamera, um
    /// [`Look::sonne_hoehe`] über dem Horizont, waagrecht von links um
    /// [`Look::sonne_seite`] zur Kamera hin. Links und zur Kamera hin sind
    /// diagonal (−1, 0, 1)/√2 und (1, 0, 1)/√2, genordet (−1, 0, 0) und
    /// (0, 0, 1). In f64 und dann gerundet, wie [`Look::weissabgleich`].
    /// Siehe docs/renderer/cinematic.md, „Sonne“.
    pub fn sonne_im_blick(&self, kamera: Kamera) -> [f32; 3] {
        let r = std::f64::consts::FRAC_1_SQRT_2;
        let (rechts, zur_kamera) = if kamera.genordet() {
            ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
        } else {
            ([r, 0.0, -r], [r, 0.0, r])
        };
        let (h, w) = (
            f64::from(self.sonne_hoehe).to_radians(),
            f64::from(self.sonne_seite).to_radians(),
        );
        let s: [f64; 3] = std::array::from_fn(|k| {
            (-w.cos() * rechts[k] + w.sin() * zur_kamera[k]) * h.cos()
                + if k == 1 { h.sin() } else { 0.0 }
        });
        let laenge = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        s.map(|c| (c / laenge) as f32)
    }

    /// Die Kurve je Kanal: bis [`Look::knie`] eine Gerade, darüber eine
    /// Parabel, die bei [`Look::flach`] mit Steigung 0 in 1 übergeht.
    pub fn kurve(&self, x: f32) -> f32 {
        if x <= self.knie {
            return x.max(0.0);
        }
        let x = x.min(self.flach);
        x - (x - self.knie) * (x - self.knie) / (2.0 * (self.flach - self.knie))
    }

    /// Wie stark ein Texel der Farbe `farbe`, linear, leuchtet, mit der
    /// Stärke 1, wie im Prototyp aus #89: nach dem hellsten Kanal 0 bis
    /// [`Look::leuchten_ab`], dann weich (smoothstep) bis 1 bei
    /// [`Look::leuchten_voll`]. So leuchten nur die hellen Texel.
    /// Siehe docs/renderer/cinematic.md, „Leuchten“.
    pub fn leuchtet(&self, farbe: [f32; 3]) -> f32 {
        let m = farbe[0].max(farbe[1]).max(farbe[2]);
        let x = ((m - self.leuchten_ab) / (self.leuchten_voll - self.leuchten_ab)).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    /// Wie viel stärker der Weissabgleich in einem Biom der Temperatur `t`
    /// wirkt, wie in 0058: 1 bis [`Look::waerme_von`], dann gerade bis
    /// 1 + [`Look::waerme`] bei [`Look::waerme_bis`], darüber gleich.
    /// Siehe docs/renderer/cinematic.md, „Wärme“.
    pub fn waerme(&self, t: f32) -> f32 {
        let f = (t - self.waerme_von) / (self.waerme_bis - self.waerme_von);
        1.0 + self.waerme * f.clamp(0.0, 1.0)
    }

    /// Das Himmelslicht in den Farben `himmel` und `nebel`, linear, mit der
    /// Stärke 1.
    pub fn himmelslicht(&self, himmel: [f32; 3], nebel: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|c| nebel[c] + (himmel[c] - nebel[c]) * self.himmel_anteil)
    }

    /// Der Weissabgleich je Kanal, der eine weisse Fläche nach oben in Sonne
    /// und Himmel der Oberwelt farblos macht: ihre Helligkeit geteilt durch
    /// ihr Licht. `himmelslicht` ist das der Oberwelt aus
    /// [`Look::himmelslicht`].
    pub fn weissabgleich(&self, himmelslicht: [f32; 3]) -> [f32; 3] {
        // In f64 und dann gerundet: so hängt das Ergebnis nicht am letzten
        // Bit der libm des Systems, und die Goldbilder gelten überall.
        let hoch = (f64::from(self.sonne_hoehe).to_radians().sin() as f32).max(0.0);
        let e: [f32; 3] = std::array::from_fn(|c| {
            self.sonne_farbe[c] * self.sonne * hoch + himmelslicht[c] * self.himmel
        });
        let l = 0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2];
        e.map(|c| l / c.max(1e-6))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Der Fingerabdruck der Werte aus 0058, nachgerechnet mit FNV-1a in
    /// Python über dieselben Bytes. Ändert sich ein Wert oder die
    /// Reihenfolge versehentlich, fällt es hier auf; eine gewollte Änderung
    /// zieht den Test nach.
    #[test]
    fn fingerabdruck_der_werte_aus_0058() {
        assert_eq!(LOOK.fingerabdruck(), "7a37818630d6d4d3");
        let anders = Look {
            belichtung: 0.26,
            ..LOOK
        };
        assert_ne!(anders.fingerabdruck(), LOOK.fingerabdruck());
    }

    /// Bis zum Knie gerade, bei 1,2 flach in 1, dazwischen stetig und
    /// ohne Knick.
    #[test]
    fn kurve_wie_in_0058() {
        for x in [0.0, 0.3, 0.8] {
            assert_eq!(LOOK.kurve(x), x);
        }
        assert!((LOOK.kurve(1.2) - 1.0).abs() < 1e-6);
        assert_eq!(LOOK.kurve(5.0), LOOK.kurve(1.2));
        assert!((LOOK.kurve(1.0) - (1.0 - 0.04 / 0.8)).abs() < 1e-6);
        let steigung = |x: f32| (LOOK.kurve(x + 1e-3) - LOOK.kurve(x - 1e-3)) / 2e-3;
        assert!((steigung(0.8) - 1.0).abs() < 1e-2);
        assert!(steigung(1.199).abs() < 1e-2);
    }

    /// Die Sonne steht 48,47° hoch, kommt von links, 8,75° zur Kamera hin
    /// gedreht: diagonal von Nordwesten etwas mehr von Westen, genordet von
    /// Westen etwas von Süden.
    #[test]
    fn sonne_fest_zur_kamera() {
        for kamera in [
            Kamera::ZWEI_ZU_EINS,
            Kamera::Oben,
            Kamera::ObenNord,
            Kamera::Nord45,
        ] {
            let s = LOOK.sonne_im_blick(kamera);
            let laenge = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
            assert!((laenge - 1.0).abs() < 1e-6, "{kamera}");
            assert!((s[1].asin().to_degrees() - 48.47).abs() < 1e-3, "{kamera}");
            let waagrecht = (s[0] * s[0] + s[2] * s[2]).sqrt();
            let (links, zur_kamera) = if kamera.genordet() {
                (-s[0], s[2])
            } else {
                ((s[2] - s[0]) / 2f32.sqrt(), (s[0] + s[2]) / 2f32.sqrt())
            };
            assert!(
                (zur_kamera.atan2(links).to_degrees() - 8.75).abs() < 1e-3,
                "{kamera}"
            );
            assert!((links.hypot(zur_kamera) - waagrecht).abs() < 1e-6);
        }
    }

    /// Nur helle Texel leuchten: bis 0,25 im hellsten Kanal nicht, ab 0,75
    /// ganz, dazwischen weich; das Soll von smoothstep von Hand gerechnet.
    #[test]
    fn leuchten_nach_der_helligkeit() {
        for (farbe, soll) in [
            ([0.25, 0.1, 0.0], 0.0),
            ([0.0, 0.0, 0.1], 0.0),
            ([0.5, 0.2, 0.1], 0.5),
            ([0.1, 0.375, 0.2], 0.15625),
            ([0.3, 0.2, 0.75], 1.0),
            ([1.0; 3], 1.0),
        ] {
            assert!((LOOK.leuchtet(farbe) - soll).abs() < 1e-6, "{farbe:?}");
        }
    }

    /// Die Wärme nach 0058: bis 0,5 klar, ab 1,0 warm mit 1,5, Ebenen und
    /// Strände (0,8) mit 1,3, Wald (0,7) mit 1,2.
    #[test]
    fn waerme_nach_der_temperatur() {
        for (t, w) in [
            (-0.5, 1.0),
            (0.0, 1.0),
            (0.5, 1.0),
            (0.7, 1.2),
            (0.8, 1.3),
            (1.0, 1.5),
            (2.0, 1.5),
        ] {
            assert!((LOOK.waerme(t) - w).abs() < 1e-6, "{t}: {}", LOOK.waerme(t));
        }
    }

    /// Das Himmelslicht ist der Himmel zu `himmel_anteil`, der Nebel zum Rest.
    #[test]
    fn himmelslicht_mischt_himmel_und_nebel() {
        assert_eq!(LOOK.himmelslicht([1.0; 3], [0.0; 3]), [0.75; 3]);
        assert_eq!(LOOK.himmelslicht([0.0; 3], [1.0; 3]), [0.25; 3]);
    }

    /// Der Abgleich auf Sonne und Himmel der Oberwelt, je Kanal gegen eine
    /// Zahl, in Python aus 0058 gerechnet: Rot hebt er, Blau senkt er.
    #[test]
    fn weissabgleich_der_oberwelt() {
        let v = LOOK.weissabgleich([0.2726444, 0.4614934, 1.0]);
        let soll = [1.1379806, 1.0038583, 0.7167913];
        assert!((0..3).all(|c| (v[c] - soll[c]).abs() < 1e-5), "{v:?}");
    }

    /// Nach dem Abgleich hat eine weisse Fläche nach oben in Sonne und
    /// Himmel in jedem Kanal dieselbe Helligkeit.
    #[test]
    fn weissabgleich_macht_weiss_farblos() {
        let himmel = LOOK.himmelslicht([0.18, 0.39, 1.0], [0.53, 0.69, 1.0]);
        let v = LOOK.weissabgleich(himmel);
        let hoch = LOOK.sonne_hoehe.to_radians().sin();
        let e: [f32; 3] = std::array::from_fn(|c| {
            (LOOK.sonne_farbe[c] * LOOK.sonne * hoch + himmel[c] * LOOK.himmel) * v[c]
        });
        assert!(
            (e[0] - e[1]).abs() < 1e-4 && (e[1] - e[2]).abs() < 1e-4,
            "{e:?}"
        );
    }
}
