//! Die Werte des Looks von Cinematic an einer Stelle, wie in 0058.
//! Siehe docs/renderer/cinematic.md, „Werte des Looks“.

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
    /// Stärke der Sonne.
    pub sonne: f32,
    /// Farbe der Sonne, linear.
    pub sonne_farbe: [f32; 3],
    /// Höhe der Sonne über dem Horizont, in Grad.
    pub sonne_hoehe: f32,
    /// Belichtung vor der Kurve.
    pub belichtung: f32,
    /// Bis hierher ist die Kurve eine Gerade.
    pub knie: f32,
    /// Ab hier ist die Kurve flach in 1.
    pub flach: f32,
}

/// Der Look aus 0058.
pub const LOOK: Look = Look {
    himmel: 3.0,
    himmel_anteil: 0.75,
    block: 1.5,
    sonne: 3.0,
    sonne_farbe: [1.0, 0.93, 0.83],
    sonne_hoehe: 48.47,
    belichtung: 0.25,
    knie: 0.8,
    flach: 1.2,
};

impl Look {
    /// Jeder Wert mit seinem Namen, in fester Reihenfolge, wie er im Code
    /// steht. Abgeleitete Werte wie die Richtung der Sonne aus Sinus und
    /// Kosinus fehlen: Deren letztes Bit kann je System abweichen.
    fn werte(&self) -> [(&'static str, &[f32]); 9] {
        [
            ("himmel", std::slice::from_ref(&self.himmel)),
            ("himmel_anteil", std::slice::from_ref(&self.himmel_anteil)),
            ("block", std::slice::from_ref(&self.block)),
            ("sonne", std::slice::from_ref(&self.sonne)),
            ("sonne_farbe", &self.sonne_farbe),
            ("sonne_hoehe", std::slice::from_ref(&self.sonne_hoehe)),
            ("belichtung", std::slice::from_ref(&self.belichtung)),
            ("knie", std::slice::from_ref(&self.knie)),
            ("flach", std::slice::from_ref(&self.flach)),
        ]
    }

    /// Der Fingerabdruck der Werte für `lookHash` in `map.json`: FNV-1a mit
    /// 64 Bit über jeden Namen, ein Nullbyte und die Bits jedes Werts in
    /// Little Endian, als 16 kleine Hexzeichen.
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
        format!("{hash:016x}")
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
        let hoch = self.sonne_hoehe.to_radians().sin().max(0.0);
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
        assert_eq!(LOOK.fingerabdruck(), "24417b93d2014610");
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
