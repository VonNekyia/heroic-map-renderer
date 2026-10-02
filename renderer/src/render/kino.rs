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
    /// Himmel und Nebel des Dimensionstyps, für Biome ohne eigene Farbe.
    vorgabe: (Tint, Tint),
    /// Je Biom das Himmelslicht in seinen Farben, siehe
    /// [`Look::himmelslicht`].
    himmel: Vec<[f32; 3]>,
    /// Weissabgleich mal Belichtung, je Kanal.
    ton: [f32; 3],
    /// Die Richtung zur Sonne im Blick, siehe [`Look::sonne_im_blick`].
    sonne: [f32; 3],
    /// Das Licht der Sonne auf einer Fläche, die genau zu ihr zeigt: ihre
    /// Farbe mal ihrer Stärke; 0, wo der Dimensionstyp kein Himmelslicht
    /// zeigt (`sky_light_factor` 0).
    sonne_licht: [f32; 3],
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
            vorgabe: (typ.sky_color, typ.fog_color),
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
        let (himmel, nebel) = self.vorgabe;
        self.himmel = biomes
            .himmel()
            .map(|h| {
                self.look.himmelslicht(
                    linear(h.himmel.unwrap_or(himmel)),
                    linear(h.nebel.unwrap_or(nebel)),
                )
            })
            .collect();
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    /// Das Himmelslicht im Biom `biome`, linear, mit der Stärke 1.
    pub fn himmel(&self, biome: u16) -> [f32; 3] {
        self.himmel[biome as usize]
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
        let soll = LOOK.himmelslicht(linear([0x78, 0xa7, 0xff]), linear([0xc0, 0xd8, 0xff]));
        assert!(!kino.himmel.is_empty());
        for h in &kino.himmel {
            assert_eq!(*h, soll);
        }
    }

    /// Eine weisse Fläche nach oben im vollen Himmelslicht der Oberwelt,
    /// ohne Sonne: der Weissabgleich nimmt dem Himmel den Stich nur zum
    /// Teil, Blau bleibt vorn, und nichts läuft über.
    #[test]
    fn weisse_flaeche_im_himmelslicht() {
        let kino = kino(&DimensionType::oberwelt());
        let licht = kino.licht(kino.himmel(0), 240.0, 0.0, 255.0);
        let [r, g, b] = kino.ton(licht);
        assert!(r < g && g < b && b < 255, "{:?}", [r, g, b]);
        assert_eq!(kino.ton([0.0; 3]), [0; 3]);
    }
}
