//! Was Cinematic zum Zeichnen braucht: das Licht des Spiels in HDR, je
//! Stufe der Lightmap und je Biom, und der Ton aus Weissabgleich,
//! Belichtung und Kurve.
//! Siehe docs/renderer/cinematic.md.

use crate::assets::DimensionType;
use crate::assets::colors::Tint;

use super::Kamera;
use super::look::Look;
use super::pyramid::{LINEAR, linear_wert, to_srgb};
use super::rasterizer::{BLOCK_FACTOR, Geometrie, blocklicht_farbe, get_brightness, roh};
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
    /// Himmel, Nebel und Nebel unter Wasser des Dimensionstyps, für Biome
    /// ohne eigene Farbe.
    vorgabe: [Tint; 3],
    /// Je Biom die Farben seines Himmels.
    himmel: Vec<Himmelsfarben>,
    /// Der Weissabgleich `v` je Kanal, siehe [`Look::weissabgleich`].
    weiss: [f32; 3],
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
    /// `temperature` des Bioms, für die Wärme aus [`Look::waerme`].
    pub temperatur: f32,
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
            temperatur: f(self.temperatur, other.temperatur),
        }
    }
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
    /// Biome aus `biomes`, aus der Kamera `kamera`.
    pub fn new(look: Look, typ: &DimensionType, biomes: &BiomeTable, kamera: Kamera) -> Kino {
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
            vorgabe: [typ.sky_color, typ.fog_color, typ.water_fog_color],
            himmel: Vec::new(),
            weiss,
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
            .map(|(h, temperatur)| {
                let (himmel, nebel) = (
                    linear(h.himmel.unwrap_or(himmel)),
                    linear(h.nebel.unwrap_or(nebel)),
                );
                Himmelsfarben {
                    licht: self.look.himmelslicht(himmel, nebel),
                    himmel,
                    nebel,
                    wassernebel: linear(h.wassernebel.unwrap_or(wassernebel)),
                    temperatur,
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

    /// Die Wärme für ein Biom der Temperatur `t`, siehe [`Look::waerme`].
    pub fn waerme(&self, t: f32) -> f32 {
        self.look.waerme(t)
    }

    /// Der Weissabgleich je Kanal mit der Wärme `w`, `1 + (v − 1) · w` wie in
    /// 0058.
    /// Siehe docs/renderer/cinematic.md, „Wärme“.
    fn abgleich(&self, w: f32) -> [f32; 3] {
        self.weiss.map(|v| 1.0 + (v - 1.0) * w)
    }

    /// Eine Farbe aus HDR, linear, nach sRGB: der Weissabgleich mit der
    /// Wärme `w` ([`Kino::abgleich`]), dazu `bloom`, schon abgeglichen, dann
    /// die Belichtung und je Kanal die Kurve aus [`Look::kurve`].
    pub fn ton(&self, farbe: [f32; 3], w: f32, bloom: [f32; 3]) -> [u8; 3] {
        let v = self.abgleich(w);
        std::array::from_fn(|c| {
            to_srgb(
                self.look
                    .kurve((farbe[c] * v[c] + bloom[c]) * self.look.belichtung),
            )
        })
    }

    /// Wie weit der Bloom bei `scale` reicht: der Radius `r` eines der
    /// Kastenfilter aus [`unscharf`], wie im Prototyp aus #89 für die
    /// Gaussglocke mit σ = [`Look::bloom_breite`] · scale; 0 ohne Bloom.
    pub fn bloom_radius(&self, scale: u32) -> usize {
        if self.look.bloom <= 0.0 {
            return 0;
        }
        let sigma = f64::from(self.look.bloom_breite) * f64::from(scale);
        let b = ((4.0 * sigma * sigma + 1.0).sqrt().round() as usize).max(1);
        (b | 1) / 2
    }

    /// Der Bloom eines Bildes der Breite `breite`: das Leuchten `leuchten`,
    /// vormultipliziert, je Pixel abgeglichen mit dessen Wärme `waerme`, mal
    /// [`Look::bloom`], unscharf mit dem Radius aus [`Kino::bloom_radius`].
    /// `None`, wo nichts leuchtet.
    /// Siehe docs/renderer/cinematic.md, „Bloom“.
    pub fn bloom(
        &self,
        leuchten: &[[f32; 3]],
        waerme: &[f32],
        breite: usize,
        scale: u32,
    ) -> Option<Vec<[f32; 3]>> {
        let r = self.bloom_radius(scale);
        if r == 0 || leuchten.iter().all(|l| *l == [0.0; 3]) {
            return None;
        }
        let quelle: Vec<[f32; 3]> = leuchten
            .iter()
            .zip(waerme)
            .map(|(l, &w)| {
                let v = self.abgleich(w);
                std::array::from_fn(|c| l[c] * v[c] * self.look.bloom)
            })
            .collect();
        Some(unscharf(&quelle, breite, r))
    }
}

/// Drei Kastenfilter der Breite `2r + 1` je Achse, erst senkrecht, dann
/// waagrecht, am Rand fortgesetzt: nahe an einer Gaussglocke, wie im
/// Prototyp aus #89. Gerechnet in Festkomma mit 24 Bit nach dem Komma: Die
/// gleitende Summe ist so exakt, und jeder Wert hängt nur an seinem
/// Fenster. Ein Ausschnitt mit Rand gibt dieselben Bits wie das ganze Bild.
fn unscharf(bild: &[[f32; 3]], breite: usize, r: usize) -> Vec<[f32; 3]> {
    const EINS: f64 = (1u64 << 24) as f64;
    let hoehe = bild.len() / breite;
    let mut q: Vec<[i64; 3]> = bild
        .iter()
        .map(|p| p.map(|c| (f64::from(c) * EINS).round() as i64))
        .collect();
    let b = 2 * r as i64 + 1;
    let mut linie = Vec::new();
    // Je Achse: wie viele Werte eine Linie hat, ihr Abstand, wie viele Linien
    // und der Abstand der Linien.
    for (laenge, schritt, linien, versatz) in
        [(hoehe, breite, breite, 1), (breite, 1, hoehe, breite)]
    {
        for _ in 0..3 {
            for l in 0..linien {
                let start = l * versatz;
                linie.clear();
                linie.extend((0..laenge).map(|i| q[start + i * schritt]));
                let at = |i: isize| linie[i.clamp(0, laenge as isize - 1) as usize];
                let mut summe = [0i64; 3];
                for k in -(r as isize)..=r as isize {
                    let v = at(k);
                    summe = std::array::from_fn(|c| summe[c] + v[c]);
                }
                for i in 0..laenge {
                    q[start + i * schritt] = summe.map(|s| (s + b / 2).div_euclid(b));
                    let (rein, raus) = (at((i + r + 1) as isize), at(i as isize - r as isize));
                    summe = std::array::from_fn(|c| summe[c] + rein[c] - raus[c]);
                }
            }
        }
    }
    q.iter()
        .map(|p| p.map(|c| (c as f64 / EINS) as f32))
        .collect()
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
            assert_eq!(andere.weiss, oberwelt.weiss, "{typ}");
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
    /// das Soll in Python gerechnet. Ohne Biomdaten gilt die Temperatur von
    /// plains, 0,8.
    #[test]
    fn himmel_ohne_biom_vom_dimensionstyp() {
        let kino = kino(&DimensionType::oberwelt());
        let soll = Himmelsfarben {
            licht: [0.2726444, 0.4614934, 1.0],
            himmel: linear([0x78, 0xa7, 0xff]),
            nebel: linear([0xc0, 0xd8, 0xff]),
            wassernebel: linear([0x05, 0x05, 0x33]),
            temperatur: 0.8,
        };
        let nah = |a: [f32; 3], b: [f32; 3]| (0..3).all(|c| (a[c] - b[c]).abs() < 1e-5);
        assert!(!kino.himmel.is_empty());
        for h in &kino.himmel {
            assert!(nah(h.licht, soll.licht), "{h:?}");
            assert_eq!(
                (h.himmel, h.nebel, h.wassernebel, h.temperatur),
                (soll.himmel, soll.nebel, soll.wassernebel, soll.temperatur)
            );
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

    /// Der Ton mit Wärme: Weiss in linearem Licht 1, mal Weissabgleich der
    /// Oberwelt und Belichtung 0,25, ohne Wärme und mit 1,5 wie in einer
    /// Savanne; das Soll in Python gerechnet. Die Wärme hebt Rot und senkt
    /// Blau.
    #[test]
    fn ton_mit_waerme() {
        let kino = kino(&DimensionType::oberwelt());
        assert_eq!(kino.ton([1.0; 3], 1.0, [0.0; 3]), [145, 137, 117]);
        assert_eq!(kino.ton([1.0; 3], 1.5, [0.0; 3]), [149, 137, 106]);
    }

    /// Der Rand des Bloom aus 0058: σ = scale/4, Kastenfilter wie im
    /// Prototyp, 3r ist bei scale 32 24 Pixel.
    #[test]
    fn bloom_radius_nach_dem_scale() {
        let kino = kino(&DimensionType::oberwelt());
        for (scale, r) in [(4, 1), (8, 2), (16, 4), (32, 8), (6, 1), (24, 6)] {
            assert_eq!(kino.bloom_radius(scale), r, "scale {scale}");
        }
    }

    /// Die Unschärfe gleicht der des Prototyps: ein Pixel mit (1, 2, 3) in
    /// der Mitte, σ = 2 (Radius 2); die Sollwerte rechnet `unscharf` aus
    /// `look.py` des Prototyps. Die Summe bleibt, nach 3r = 6 ist nichts
    /// mehr.
    #[test]
    fn unscharf_wie_im_prototyp() {
        let mut bild = vec![[0.0f32; 3]; 31 * 31];
        bild[15 * 31 + 15] = [1.0, 2.0, 3.0];
        let aus = unscharf(&bild, 31, 2);
        for ((x, y), soll) in [
            ((15, 15), 0.023104),
            ((18, 15), 0.01216),
            ((15, 12), 0.01216),
            ((21, 15), 0.001216),
            ((22, 15), 0.0),
        ] {
            let ist = aus[y * 31 + x];
            for (c, k) in [1.0, 2.0, 3.0].into_iter().enumerate() {
                assert!((ist[c] - soll * k).abs() < 1e-6, "({x}, {y}): {ist:?}");
            }
        }
        let summe: f32 = aus.iter().map(|p| p[0]).sum();
        assert!((summe - 1.0).abs() < 1e-5, "{summe}");
    }

    /// Ein Ausschnitt mit einem Rand von 3r gibt dieselben Bits wie das
    /// ganze Bild, auch an den Rändern des Ausschnitts.
    #[test]
    fn unscharf_im_ausschnitt_gleich() {
        let (breite, hoehe, r) = (40, 30, 3);
        let bild: Vec<[f32; 3]> = (0..breite * hoehe)
            .map(|i| {
                let f = ((i * 7919) % 101) as f32 / 37.0;
                [f, f * 0.5, (i % 13) as f32]
            })
            .collect();
        let ganz = unscharf(&bild, breite, r);
        let (x0, y0, b, h) = (9, 11, 12, 8);
        let rand = 3 * r;
        let teil: Vec<[f32; 3]> = (y0 - rand..y0 + h + rand)
            .flat_map(|y| (x0 - rand..x0 + b + rand).map(move |x| (x, y)))
            .map(|(x, y)| bild[y * breite + x])
            .collect();
        let klein = unscharf(&teil, b + 2 * rand, r);
        for y in 0..h {
            for x in 0..b {
                let ist = klein[(y + rand) * (b + 2 * rand) + x + rand];
                assert_eq!(ist, ganz[(y0 + y) * breite + x0 + x], "({x}, {y})");
            }
        }
    }

    /// Eine weisse Fläche nach oben im vollen Himmelslicht der Oberwelt,
    /// ohne Sonne: der Weissabgleich nimmt dem Himmel den Stich nur zum
    /// Teil, Blau bleibt vorn, und nichts läuft über.
    #[test]
    fn weisse_flaeche_im_himmelslicht() {
        let kino = kino(&DimensionType::oberwelt());
        let licht = kino.licht(kino.himmel(0).licht, 240.0, 0.0, 255.0);
        let [r, g, b] = kino.ton(licht, 1.0, [0.0; 3]);
        assert!(r < g && g < b && b < 255, "{:?}", [r, g, b]);
        assert_eq!(kino.ton([0.0; 3], 1.0, [0.0; 3]), [0; 3]);
    }
}
