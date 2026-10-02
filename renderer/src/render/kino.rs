//! Was Cinematic zum Zeichnen braucht: das Licht des Spiels in HDR, je
//! Stufe der Lightmap und je Biom, und der Ton aus Weissabgleich,
//! Belichtung und Kurve.
//! Siehe docs/renderer/cinematic.md.

use crate::assets::DimensionType;
use crate::assets::colors::Tint;

use super::Kamera;
use super::look::Look;
use super::pyramid::{LINEAR, to_srgb};
use super::rasterizer::{BLOCK_FACTOR, Geometrie};
use super::tint::BiomeTable;

/// Die Werte des Looks, umgerechnet für eine Dimension und ihre Biome.
pub struct Kino {
    look: Look,
    /// Je Stufe des Himmelslichts sein Licht ohne die Farbe des Himmels:
    /// `getBrightness` mal `SkyFactor` und `SkyLightColor` der Dimension, mal
    /// [`Look::himmel`].
    himmel_stufen: [[f32; 3]; 16],
    /// Je Stufe des Blocklichts sein Licht: `getBrightness` mal
    /// `BlockFactor` in der Farbe zwischen `BlockLightTint` und Weiss wie in
    /// `lightmap.fsh`, mal [`Look::block`].
    block_stufen: [[f32; 3]; 16],
    /// Himmel, Nebel und Nebel unter Wasser des Dimensionstyps, für Biome
    /// ohne eigene Farbe.
    vorgabe: [Tint; 3],
    /// Je Biom die Farben seines Himmels.
    himmel: Vec<Himmelsfarben>,
    /// Weissabgleich mal Belichtung, je Kanal.
    ton: [f32; 3],
    /// Die Richtung zur Sonne im Blick, siehe [`Look::sonne_im_blick`].
    sonne: [f32; 3],
    /// Das Licht der Sonne auf einer Fläche, die genau zu ihr zeigt: ihre
    /// Farbe mal ihrer Stärke; 0, wo der Dimensionstyp kein Himmelslicht
    /// zeigt (`sky_light_factor` 0).
    sonne_licht: [f32; 3],
}

/// Die Farben des Himmels an einem Block, linear, mit der Stärke 1.
/// Siehe docs/renderer/cinematic.md, „Farbe des Himmels“.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Himmelsfarben {
    /// Das Himmelslicht auf einer Fläche, aus Himmel und Nebel nach
    /// [`Look::himmelslicht`].
    pub licht: [f32; 3],
    /// `sky_color` und `fog_color`, die das Wasser spiegelt.
    pub himmel: [f32; 3],
    pub nebel: [f32; 3],
    /// `water_fog_color`, in der Wasser nach der Strecke färbt.
    pub wassernebel: [f32; 3],
}

impl Himmelsfarben {
    /// Jede Farbe mit `f` verrechnet, zum Mischen über Blöcke.
    pub fn je_farbe(self, other: Himmelsfarben, f: impl Fn(f32, f32) -> f32) -> Himmelsfarben {
        let paar = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|c| f(a[c], b[c]));
        Himmelsfarben {
            licht: paar(self.licht, other.licht),
            himmel: paar(self.himmel, other.himmel),
            nebel: paar(self.nebel, other.nebel),
            wassernebel: paar(self.wassernebel, other.wassernebel),
        }
    }
}

/// Eine Farbe in linearem Licht.
fn linear(farbe: Tint) -> [f32; 3] {
    farbe.map(|c| LINEAR[c as usize])
}

/// `getBrightness` in `lightmap.fsh`: die Helligkeit einer Stufe von 0 bis
/// 15, wie [`super::rasterizer::brightness_rgb`] sie rechnet.
fn helligkeit(stufe: usize) -> f32 {
    let l = stufe as f32 / 15.0;
    l / (4.0 - 3.0 * l)
}

impl Kino {
    /// Der Look `look` in der Dimension vom Typ `typ`, mit den Farben der
    /// Biome aus `biomes`, aus der Kamera `kamera`.
    pub fn new(look: Look, typ: &DimensionType, biomes: &BiomeTable, kamera: Kamera) -> Kino {
        let himmel_farbe = linear(typ.sky_light_color);
        let tint = linear(typ.block_light_tint);
        let himmel_stufen = std::array::from_fn(|s| {
            let k = helligkeit(s) * typ.sky_light_factor * look.himmel;
            himmel_farbe.map(|c| c * k)
        });
        let block_stufen = std::array::from_fn(|b| {
            let l = b as f32 / 15.0;
            let mix = 0.9 * (2.0 * l - 1.0) * (2.0 * l - 1.0);
            let k = helligkeit(b) * BLOCK_FACTOR * look.block;
            tint.map(|t| (t + (1.0 - t) * mix) * k)
        });
        // Abgeglichen wird immer auf Sonne und Himmel der Oberwelt, siehe
        // `Look::weissabgleich`.
        let oberwelt = DimensionType::oberwelt();
        let weiss = look.weissabgleich(
            look.himmelslicht(linear(oberwelt.sky_color), linear(oberwelt.fog_color)),
        );
        let mut kino = Kino {
            look,
            himmel_stufen,
            block_stufen,
            vorgabe: [typ.sky_color, typ.fog_color, typ.water_fog_color],
            himmel: Vec::new(),
            ton: weiss.map(|v| v * look.belichtung),
            sonne: look.sonne_im_blick(kamera),
            sonne_licht: match typ.sky_light_factor > 0.0 {
                true => look.sonne_farbe.map(|c| c * look.sonne),
                false => [0.0; 3],
            },
        };
        kino.mit_biomen(biomes);
        kino
    }

    /// Nimmt die Farben der Biome aus `biomes`; was ein Biom nicht setzt,
    /// kommt vom Dimensionstyp.
    pub fn mit_biomen(&mut self, biomes: &BiomeTable) {
        let [himmel, nebel, wassernebel] = self.vorgabe;
        self.himmel = biomes
            .himmel()
            .map(|h| {
                let (himmel, nebel) = (
                    linear(h.himmel.unwrap_or(himmel)),
                    linear(h.nebel.unwrap_or(nebel)),
                );
                Himmelsfarben {
                    licht: self.look.himmelslicht(himmel, nebel),
                    himmel,
                    nebel,
                    wassernebel: linear(h.wassernebel.unwrap_or(wassernebel)),
                }
            })
            .collect();
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    /// Die Farben des Himmels im Biom `biome`.
    pub fn himmel(&self, biome: u16) -> Himmelsfarben {
        self.himmel[biome as usize]
    }

    /// Was eine Wasserfläche mit der Normale `n` spiegelt, gesehen in
    /// Richtung `blick` (vom Auge in die Szene, beide Länge 1): ihr Anteil
    /// nach Fresnel (Schlick, F0 [`Look::wasser_spiegel`]) und der Himmel in
    /// der gespiegelten Richtung, zum Horizont hin in der Farbe des Nebels,
    /// wie im Prototyp aus #89.
    /// Siehe docs/renderer/cinematic.md, „Wasser“.
    pub fn spiegel(&self, farben: &Himmelsfarben, blick: [f32; 3], n: [f32; 3]) -> (f32, [f32; 3]) {
        let dn = blick[0] * n[0] + blick[1] * n[1] + blick[2] * n[2];
        let f0 = self.look.wasser_spiegel;
        let anteil = f0 + (1.0 - f0) * (1.0 + dn).clamp(0.0, 1.0).powi(5);
        let hoch = blick[1] - 2.0 * dn * n[1];
        let h = ((hoch + 0.1) / 0.7).clamp(0.0, 1.0);
        let h = h * h * (3.0 - 2.0 * h);
        let himmel =
            std::array::from_fn(|c| farben.nebel[c] + (farben.himmel[c] - farben.nebel[c]) * h);
        (anteil, himmel)
    }

    /// Wie dicht Wasser der Farbe `w`, linear, je Kanal ist, je Block
    /// Strecke: Kanäle, die die Farbe schwächer trägt, dämpft es stärker,
    /// geteilt durch [`Look::wasser_dichte`], wie im Prototyp aus #89.
    pub fn wasser_dichte(&self, w: [f32; 3]) -> [f32; 3] {
        let m = w.iter().fold(1e-4f32, |a, &c| a.max(c));
        w.map(|c| (-(c / m).max(0.02).ln() + 0.35) / self.look.wasser_dichte)
    }

    /// Das Licht an einem Pixel in HDR je Kanal: `himmel` die Farbe des
    /// Himmelslichts am Block, `sky` und `block` die Stufen in Sechzehnteln,
    /// `schatten` der Schatten der weichen Beleuchtung in 255steln. Zwischen
    /// zwei Stufen linear gemischt, wie das Spiel die Lightmap liest
    /// ([`super::rasterizer::Lightmap::linear`]).
    /// Siehe docs/renderer/cinematic.md, „Licht in HDR“.
    pub fn licht(&self, himmel: [f32; 3], sky: f32, block: f32, schatten: f32) -> [f32; 3] {
        let stufe = |stufen: &[[f32; 3]; 16], wert: f32| -> [f32; 3] {
            let s = (wert / 16.0).clamp(0.0, 15.0);
            let unten = s as usize;
            let oben = (unten + 1).min(15);
            let f = s - unten as f32;
            std::array::from_fn(|c| stufen[unten][c] + (stufen[oben][c] - stufen[unten][c]) * f)
        };
        let (h, b) = (
            stufe(&self.himmel_stufen, sky),
            stufe(&self.block_stufen, block),
        );
        let k = schatten / 255.0;
        std::array::from_fn(|c| (himmel[c] * h[c] + b[c]) * k)
    }

    /// Die Richtung zur Sonne im Blick.
    pub fn sonne(&self) -> [f32; 3] {
        self.sonne
    }

    /// Das Licht der Sonne auf einem Pixel mit `geometrie`, ohne Schatten:
    /// nach dem Winkel zwischen Normale und Sonne, eine Fläche ohne `shade`
    /// wie eine nach oben; abgewandt keines.
    /// Siehe docs/renderer/cinematic.md, „Sonne“.
    pub fn sonnenlicht(&self, geometrie: &Geometrie) -> [f32; 3] {
        let [nx, ny, nz] = match geometrie.shade {
            true => geometrie.normale,
            false => [0.0, 1.0, 0.0],
        };
        let cos = (nx * self.sonne[0] + ny * self.sonne[1] + nz * self.sonne[2]).max(0.0);
        self.sonne_licht.map(|c| c * cos)
    }

    /// Eine Farbe aus HDR, linear, nach sRGB: Weissabgleich und Belichtung,
    /// dann je Kanal die Kurve aus [`Look::kurve`].
    pub fn ton(&self, farbe: [f32; 3]) -> [u8; 3] {
        std::array::from_fn(|c| to_srgb(self.look.kurve(farbe[c] * self.ton[c])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::colors::Colors;
    use crate::render::look::LOOK;

    fn kino(typ: &DimensionType) -> Kino {
        Kino::new(
            LOOK,
            typ,
            &BiomeTable::new(&Colors::default()),
            Kamera::ZWEI_ZU_EINS,
        )
    }

    /// Die Sonne nach dem Winkel: eine Fläche nach oben bekommt den Sinus
    /// der Höhe, eine abgewandte nichts, eine ohne `shade` wie eine nach
    /// oben. Im Nether scheint sie nicht.
    #[test]
    fn sonne_nach_dem_winkel() {
        let kino = kino(&DimensionType::oberwelt());
        let g = |normale, shade| Geometrie {
            tiefe: 0.0,
            normale,
            shade,
            wasser: 0.0,
        };
        let oben = kino.sonnenlicht(&g([0.0, 1.0, 0.0], true));
        let hoch = LOOK.sonne_hoehe.to_radians().sin() * LOOK.sonne;
        for (o, f) in oben.iter().zip(LOOK.sonne_farbe) {
            assert!((o - f * hoch).abs() < 1e-5);
        }
        let weg = kino.sonne().map(|c| -c);
        assert_eq!(kino.sonnenlicht(&g(weg, true)), [0.0; 3]);
        assert_eq!(kino.sonnenlicht(&g(weg, false)), oben);
        let nether = DimensionType::des_spiels("minecraft:the_nether").unwrap();
        let im_nether = super::tests::kino(&nether);
        assert_eq!(im_nether.sonnenlicht(&g([0.0, 1.0, 0.0], true)), [0.0; 3]);
    }

    /// Die Stufen wie in `lightmap.fsh`, getrennt und ohne Begrenzung: In
    /// der Oberwelt gibt Himmelslicht 15 in jedem Kanal die Stärke des
    /// Himmels, Blocklicht 15 im Rot, wo `BlockLightTint` voll ist,
    /// `BlockFactor` mal die Stärke des Blocklichts; Stufe 0 bleibt dunkel.
    /// Dazwischen `getBrightness`.
    #[test]
    fn stufen_wie_lightmap_fsh() {
        let kino = kino(&DimensionType::oberwelt());
        assert_eq!(kino.himmel_stufen[15], [LOOK.himmel; 3]);
        assert_eq!(kino.himmel_stufen[0], [0.0; 3]);
        assert_eq!(kino.block_stufen[15][0], 1.4 * LOOK.block);
        assert_eq!(kino.block_stufen[0], [0.0; 3]);
        let mitte = 8.0 / 15.0 / (4.0 - 3.0 * 8.0 / 15.0) * LOOK.himmel;
        assert!((kino.himmel_stufen[8][1] - mitte).abs() < 1e-6);
        // In der Mitte die Farbe von BlockLightTint #ffd88c, linear.
        let [r, g, b] = kino.block_stufen[7];
        let tint = linear([0xff, 0xd8, 0x8c]);
        assert!((g / r - tint[1]).abs() < 0.01 && (b / r - tint[2]).abs() < 0.01);
    }

    /// Zwischen zwei Stufen linear, wie die Lightmap gelesen wird; ohne
    /// Schatten dunkel, über die oberste Stufe hinaus nicht heller.
    #[test]
    fn licht_zwischen_den_stufen() {
        let kino = kino(&DimensionType::oberwelt());
        let weiss = [1.0; 3];
        let stufe = |s: f32| kino.licht(weiss, s * 16.0, 0.0, 255.0)[0];
        assert!(
            (kino.licht(weiss, 7.5 * 16.0, 0.0, 255.0)[0] - (stufe(7.0) + stufe(8.0)) / 2.0).abs()
                < 1e-6
        );
        assert_eq!(stufe(15.0), stufe(16.0));
        assert_eq!(kino.licht(weiss, 240.0, 240.0, 0.0), [0.0; 3]);
        let halb = kino.licht(weiss, 240.0, 0.0, 127.5);
        assert!((halb[0] - stufe(15.0) / 2.0).abs() < 1e-6);
    }

    /// Ohne Farbe im Biom gilt die des Dimensionstyps: in der Oberwelt
    /// Himmel #78a7ff und Nebel #c0d8ff, gemischt mit dem Anteil aus 0058.
    #[test]
    fn himmel_ohne_biom_vom_dimensionstyp() {
        let kino = kino(&DimensionType::oberwelt());
        let (himmel, nebel) = (linear([0x78, 0xa7, 0xff]), linear([0xc0, 0xd8, 0xff]));
        let soll = Himmelsfarben {
            licht: LOOK.himmelslicht(himmel, nebel),
            himmel,
            nebel,
            wassernebel: linear([0x05, 0x05, 0x33]),
        };
        assert!(!kino.himmel.is_empty());
        for h in &kino.himmel {
            assert_eq!(*h, soll);
        }
    }

    /// Wasser von oben spiegelt wenig, von der Seite viel, bei 0058 aus 2:1
    /// rund 5 %, und zeigt dann den Himmel; flach darüber den Nebel. Wasser
    /// in seiner eigenen blauen Farbe dämpft Rot stärker als Blau.
    #[test]
    fn wasser_spiegelt_und_dampft() {
        let kino = kino(&DimensionType::oberwelt());
        let farben = kino.himmel(0);
        let oben = [0.0, 1.0, 0.0];
        let r = std::f32::consts::FRAC_1_SQRT_2;
        let blick = [-1.0 / 3f32.sqrt(); 3];
        let (steil, himmel) = kino.spiegel(&farben, blick, oben);
        assert!((0.05..0.06).contains(&steil), "{steil}");
        for (h, soll) in himmel.iter().zip(farben.himmel) {
            assert!((h - soll).abs() < 0.002);
        }
        let (flach, nebel) = kino.spiegel(&farben, [r, -1e-3, -r], oben);
        assert!(flach > 0.9, "{flach}");
        for (n, soll) in nebel.iter().zip(farben.nebel) {
            assert!((n - soll).abs() < 0.05);
        }
        let sigma = kino.wasser_dichte(linear([0x3f, 0x76, 0xe4]));
        assert!(sigma[0] > sigma[1] && sigma[1] > sigma[2], "{sigma:?}");
        assert!((sigma[2] - 0.35 / LOOK.wasser_dichte).abs() < 1e-6);
    }

    /// Eine weisse Fläche nach oben im vollen Himmelslicht der Oberwelt,
    /// ohne Sonne: der Weissabgleich nimmt dem Himmel den Stich nur zum
    /// Teil, Blau bleibt vorn, und nichts läuft über.
    #[test]
    fn weisse_flaeche_im_himmelslicht() {
        let kino = kino(&DimensionType::oberwelt());
        let licht = kino.licht(kino.himmel(0).licht, 240.0, 0.0, 255.0);
        let [r, g, b] = kino.ton(licht);
        assert!(r < g && g < b && b < 255, "{:?}", [r, g, b]);
        assert_eq!(kino.ton([0.0; 3]), [0; 3]);
    }
}
