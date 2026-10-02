//! Was Cinematic zum Zeichnen braucht: das Licht des Spiels in HDR, je
//! Stufe der Lightmap und je Biom, und der Ton aus Weissabgleich,
//! Belichtung und Kurve.
//! Siehe docs/renderer/cinematic.md.

use crate::assets::DimensionType;
use crate::assets::colors::Tint;

use super::look::Look;
use super::pyramid::{LINEAR, linear_wert, to_srgb};
use super::rasterizer::{BLOCK_FACTOR, blocklicht_farbe, get_brightness, roh};
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
    /// `lightmap.fsh`, gemischt wie dort und danach linear, mal
    /// [`Look::block`].
    block_stufen: [[f32; 3]; 16],
    /// `AmbientColor` der Dimension, die `lightmap.fsh` jedem Licht
    /// zugrunde legt, roh wie dort.
    umgebung: [f32; 3],
    /// Himmel und Nebel des Dimensionstyps, für Biome ohne eigene Farbe.
    vorgabe: (Tint, Tint),
    /// Je Biom das Himmelslicht in seinen Farben, siehe
    /// [`Look::himmelslicht`].
    himmel: Vec<[f32; 3]>,
    /// Weissabgleich mal Belichtung, je Kanal.
    ton: [f32; 3],
}

/// Eine Farbe in linearem Licht.
fn linear(farbe: Tint) -> [f32; 3] {
    farbe.map(|c| LINEAR[c as usize])
}

/// `getBrightness` einer Stufe von 0 bis 15.
fn helligkeit(stufe: usize) -> f32 {
    get_brightness(stufe as f32 / 15.0)
}

impl Kino {
    /// Der Look `look` in der Dimension vom Typ `typ`, mit den Farben der
    /// Biome aus `biomes`.
    pub fn new(look: Look, typ: &DimensionType, biomes: &BiomeTable) -> Kino {
        let himmel_farbe = linear(typ.sky_light_color);
        let himmel_stufen = std::array::from_fn(|s| {
            let k = helligkeit(s) * typ.sky_light_factor * look.himmel;
            himmel_farbe.map(|c| c * k)
        });
        let block_stufen = std::array::from_fn(|b| {
            let k = helligkeit(b) * BLOCK_FACTOR * look.block;
            blocklicht_farbe(typ.block_light_tint, b as f32 / 15.0).map(|c| linear_wert(c) * k)
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
            umgebung: roh(typ.ambient_light_color),
            vorgabe: (typ.sky_color, typ.fog_color),
            himmel: Vec::new(),
            ton: weiss.map(|v| v * look.belichtung),
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

    /// Das Himmelslicht im Biom `biome`, linear, mit der Stärke 1.
    pub fn himmel(&self, biome: u16) -> [f32; 3] {
        self.himmel[biome as usize]
    }

    /// Das Licht an einer Ecke oder einem Block in HDR je Kanal: `himmel`
    /// die Farbe des Himmelslichts am Block, `sky` und `block` die Stufen in
    /// Sechzehnteln, `schatten` der Schatten der weichen Beleuchtung in
    /// 255steln. Zwischen zwei Stufen linear gemischt, wie das Spiel die
    /// Lightmap liest ([`super::rasterizer::Lightmap::linear`]); darunter die
    /// Umgebungsfarbe.
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
        std::array::from_fn(|c| (self.umgebung[c] + himmel[c] * h[c] + b[c]) * k)
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
        Kino::new(LOOK, typ, &BiomeTable::new(&Colors::default()))
    }

    /// Die Stufen wie in `lightmap.fsh`, getrennt und ohne Begrenzung, je
    /// gegen eine Zahl, in Python aus der Formel des Shaders gerechnet: In
    /// der Oberwelt gibt Himmelslicht 15 in jedem Kanal die Stärke des
    /// Himmels, Stufe 8 `getBrightness` davon. Blocklicht mischt
    /// `BlockLightTint` #ffd88c roh mit Weiss, `0,9 · (2b − 1)²`, und erst
    /// das Ergebnis wird linear, mal 1,4 und der Stärke 1,5. Stufe 0 bleibt
    /// dunkel; im Nether und im Ende, `sky_light_factor` 0, jede Stufe des
    /// Himmels.
    #[test]
    fn stufen_wie_lightmap_fsh() {
        let nah = |ist: [f32; 3], soll: [f32; 3]| {
            assert!(
                (0..3).all(|c| (ist[c] - soll[c]).abs() < 1e-5),
                "{ist:?} statt {soll:?}"
            );
        };
        let kino = kino(&DimensionType::oberwelt());
        nah(kino.himmel_stufen[15], [3.0; 3]);
        nah(kino.himmel_stufen[8], [0.6666667; 3]);
        nah(kino.himmel_stufen[0], [0.0; 3]);
        nah(kino.block_stufen[15], [2.1, 2.027_676, 1.890_965]);
        nah(kino.block_stufen[12], [1.05, 0.819_760_4, 0.463_149_5]);
        nah(kino.block_stufen[0], [0.0; 3]);
        for typ in ["minecraft:the_nether", "minecraft:the_end"] {
            let ohne = super::tests::kino(&DimensionType::des_spiels(typ).unwrap());
            assert!(ohne.himmel_stufen.iter().all(|s| *s == [0.0; 3]), "{typ}");
        }
    }

    /// Die Umgebungsfarbe liegt unter jedem Licht, roh wie in
    /// `lightmap.fsh`: im Nether #302821 ohne Himmels- und Blocklicht, und
    /// der Schatten der weichen Beleuchtung dunkelt sie mit.
    #[test]
    fn umgebung_im_nether() {
        let nether = kino(&DimensionType::des_spiels("minecraft:the_nether").unwrap());
        let soll = [48.0 / 255.0, 40.0 / 255.0, 33.0 / 255.0];
        assert_eq!(nether.licht([1.0; 3], 0.0, 0.0, 255.0), soll);
        assert_eq!(nether.licht([1.0; 3], 240.0, 0.0, 0.0), [0.0; 3]);
    }

    /// Der Weissabgleich ist in jeder Dimension der der Oberwelt.
    #[test]
    fn weissabgleich_wie_in_der_oberwelt() {
        let oberwelt = kino(&DimensionType::oberwelt());
        for typ in ["minecraft:the_nether", "minecraft:the_end"] {
            let andere = super::tests::kino(&DimensionType::des_spiels(typ).unwrap());
            assert_eq!(andere.ton, oberwelt.ton, "{typ}");
        }
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
    /// Himmel #78a7ff und Nebel #c0d8ff, linear zu 0,75 und 0,25 gemischt;
    /// das Soll in Python gerechnet.
    #[test]
    fn himmel_ohne_biom_vom_dimensionstyp() {
        let kino = kino(&DimensionType::oberwelt());
        let soll = [0.2726444, 0.4614934, 1.0];
        assert!(!kino.himmel.is_empty());
        for h in &kino.himmel {
            assert!((0..3).all(|c| (h[c] - soll[c]).abs() < 1e-5), "{h:?}");
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
