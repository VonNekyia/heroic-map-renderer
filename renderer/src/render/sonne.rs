//! Was ein Block dem Strahl zur Sonne in den Weg stellt, für Cinematic: je
//! Alternative die Dreiecke seines Modells im Blick mit dem Alpha-Test ihrer
//! Textur, und der Test eines Strahls dagegen. Den Gang durch die Welt macht
//! der Chunk-Cache.
//! Siehe docs/renderer/cinematic.md, „Schatten“.

use std::collections::HashMap;

use image::RgbaImage;

use crate::assets::blockstate::seite;
use crate::assets::fluid::Fluid;
use crate::assets::{BakedModel, Face, Quad, TextureId, Textures};

use super::Richtung;
use super::rasterizer::{ausschnitt, sonnenschwelle};

/// Welche Texel einer Textur den Strahl zur Sonne aufhalten: die, deren
/// Alpha mindestens die Schwelle aus [`sonnenschwelle`] erreicht.
struct Texelmaske {
    breite: u32,
    hoehe: u32,
    bits: Vec<u64>,
}

impl Texelmaske {
    fn new(bild: &RgbaImage, schwelle: u8) -> Texelmaske {
        let (breite, hoehe) = bild.dimensions();
        let mut bits = vec![0u64; (breite as usize * hoehe as usize).div_ceil(64)];
        for (i, pixel) in bild.pixels().enumerate() {
            if pixel.0[3] >= schwelle {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        Texelmaske {
            breite,
            hoehe,
            bits,
        }
    }

    /// Der Texel an Spalte und Zeile, ausserhalb der Textur wiederholt wie
    /// `sample` im Rasterizer.
    fn texel(&self, x: i64, y: i64) -> bool {
        let x = x.rem_euclid(self.breite as i64) as usize;
        let y = y.rem_euclid(self.hoehe as i64) as usize;
        let i = y * self.breite as usize + x;
        self.bits[i / 64] >> (i % 64) & 1 != 0
    }

    /// Deckt der Texel an den normierten Koordinaten `(u, v)`?
    fn deckt(&self, [u, v]: [f32; 2]) -> bool {
        self.texel(
            (u * self.breite as f32).floor() as i64,
            (v * self.hoehe as f32).floor() as i64,
        )
    }

    /// Deckt jeder Texel, den eine Fläche mit dem Ausschnitt `lo` bis `hi`
    /// abtastet?
    fn ganz(&self, lo: [f32; 2], hi: [f32; 2]) -> bool {
        let spalte = |c: f32, n: u32| (c * n as f32).floor() as i64;
        let (x0, x1) = (spalte(lo[0], self.breite), spalte(hi[0], self.breite));
        let (y0, y1) = (spalte(lo[1], self.hoehe), spalte(hi[1], self.hoehe));
        (y0..=y1).all(|y| (x0..=x1).all(|x| self.texel(x, y)))
    }
}

/// Die Texelmasken eines Laufs, je Textur und Schwelle eine.
#[derive(Default)]
pub struct Masken {
    liste: Vec<Texelmaske>,
    index: HashMap<(TextureId, u8), u32>,
}

impl Masken {
    /// Die Maske für die Fläche `quad`, beim ersten Mal angelegt.
    fn von(&mut self, quad: &Quad, textures: &Textures) -> u32 {
        let schwelle = sonnenschwelle(quad, textures);
        *self
            .index
            .entry((quad.texture, schwelle))
            .or_insert_with(|| {
                self.liste
                    .push(Texelmaske::new(textures.image(quad.texture), schwelle));
                self.liste.len() as u32 - 1
            })
    }
}

/// Ein Dreieck einer Fläche, im Blick relativ zum Ursprung seines Blocks.
struct Dreieck {
    a: [f32; 3],
    e1: [f32; 3],
    e2: [f32; 3],
    /// Die Texturkoordinaten an `a` und ihre Änderung entlang `e1` und `e2`.
    uv: [f32; 2],
    du: [f32; 2],
    dv: [f32; 2],
    /// Der Ausschnitt der Textur, in dem abgetastet wird, wie beim Zeichnen.
    lo: [f32; 2],
    hi: [f32; 2],
    maske: u32,
    /// Die Seite der Welt, wenn das Dreieck zu einer Fläche aus Lava gehört:
    /// Sie entfällt zu einem Nachbarn wie im Spiel, siehe [`Sonnenform::trifft`].
    lava: Option<Face>,
}

/// Die Dreiecke eines Modells für den Strahl zur Sonne und ihre Hülle.
struct Schattenmodell {
    dreiecke: Vec<Dreieck>,
    lo: [f32; 3],
    hi: [f32; 3],
    /// Die Seiten des Würfels, die eine Fläche ganz und deckend belegt, Bits
    /// nach [`seite`] im Blick.
    ganze_seiten: u8,
}

impl Schattenmodell {
    /// Das Modell im Blick aus `richtung`. Flächen aus Wasser halten die
    /// Sonne nie auf und fehlen, ebenso Flächen ohne Textur, die auch das
    /// Zeichnen auslässt.
    fn new(
        model: &BakedModel,
        richtung: Richtung,
        textures: &Textures,
        masken: &mut Masken,
    ) -> Schattenmodell {
        let mut m = Schattenmodell {
            dreiecke: Vec::new(),
            lo: [f32::MAX; 3],
            hi: [f32::MIN; 3],
            ganze_seiten: 0,
        };
        for quad in &model.quads {
            let (w, h) = textures.image(quad.texture).dimensions();
            if matches!(quad.fluid, Some((Fluid::Water, _))) || w == 0 || h == 0 {
                continue;
            }
            let maske = masken.von(quad, textures);
            let c = quad.corners.map(|e| richtung.punkt_in_den_blick(e));
            let [lo, oben] = ausschnitt(quad);
            let hi = [(oben[0] - 1e-4).max(lo[0]), (oben[1] - 1e-4).max(lo[1])];
            if quad.fluid.is_none() && masken.liste[maske as usize].ganz(lo, hi) {
                m.ganze_seiten |= ganze_seite(&c);
            }
            for [i, j, k] in [[0, 1, 2], [0, 2, 3]] {
                m.dreiecke.push(Dreieck {
                    a: c[i],
                    e1: sub(c[j], c[i]),
                    e2: sub(c[k], c[i]),
                    uv: quad.uvs[i],
                    du: [
                        quad.uvs[j][0] - quad.uvs[i][0],
                        quad.uvs[j][1] - quad.uvs[i][1],
                    ],
                    dv: [
                        quad.uvs[k][0] - quad.uvs[i][0],
                        quad.uvs[k][1] - quad.uvs[i][1],
                    ],
                    lo,
                    hi,
                    maske,
                    lava: quad.fluid.map(|(_, face)| face),
                });
            }
            for e in c {
                m.lo = std::array::from_fn(|k| m.lo[k].min(e[k]));
                m.hi = std::array::from_fn(|k| m.hi[k].max(e[k]));
            }
        }
        m
    }

    /// Trifft der Strahl `o + t·d` mit `0 ≤ t ≤ weite` eine deckende Stelle?
    /// Erst gegen die Hülle, dann Dreieck für Dreieck (Möller–Trumbore),
    /// beidseitig; der erste deckende Treffer genügt. `weg`: die Seiten der
    /// Welt, zu denen Flächen aus Lava entfallen.
    fn trifft(&self, weg: u8, masken: &Masken, o: [f32; 3], d: [f32; 3], weite: f32) -> bool {
        let (mut t0, mut t1) = (-1e-3f32, weite + 1e-3);
        for k in 0..3 {
            let inv = 1.0 / d[k];
            let (a, b) = (
                (self.lo[k] - 1e-3 - o[k]) * inv,
                (self.hi[k] + 1e-3 - o[k]) * inv,
            );
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        if t0 > t1 {
            return false;
        }
        self.dreiecke.iter().any(|dr| {
            if dr.lava.is_some_and(|face| weg & seite(face) != 0) {
                return false;
            }
            let p = cross(d, dr.e2);
            let det = dot(dr.e1, p);
            if det.abs() < 1e-12 {
                return false;
            }
            let inv = 1.0 / det;
            let s = sub(o, dr.a);
            let u = dot(s, p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return false;
            }
            let q = cross(s, dr.e1);
            let w = dot(d, q) * inv;
            if w < 0.0 || u + w > 1.0 {
                return false;
            }
            let t = dot(dr.e2, q) * inv;
            if !(0.0..=weite).contains(&t) {
                return false;
            }
            let uv = std::array::from_fn(|c| {
                (dr.uv[c] + u * dr.du[c] + w * dr.dv[c]).clamp(dr.lo[c], dr.hi[c])
            });
            masken.liste[dr.maske as usize].deckt(uv)
        })
    }
}

/// Liegt das Viereck `c` ganz auf einer Seite des Würfels? Dann ihr Bit
/// nach [`seite`], sonst 0.
fn ganze_seite(c: &[[f32; 3]; 4]) -> u8 {
    const EPS: f32 = 1e-6;
    let auf = |x: f32, wert: f32| (x - wert).abs() <= EPS;
    for (k, [unten, oben]) in [
        (0, [Face::West, Face::East]),
        (1, [Face::Down, Face::Up]),
        (2, [Face::North, Face::South]),
    ] {
        for (wert, face) in [(0.0, unten), (1.0, oben)] {
            if !c.iter().all(|e| auf(e[k], wert)) {
                continue;
            }
            // Die vier Ecken der Seite, jede einmal.
            let ecken: Vec<[bool; 2]> = c
                .iter()
                .filter_map(|e| {
                    let andere: Vec<f32> = (0..3).filter(|&j| j != k).map(|j| e[j]).collect();
                    let bit = |x: f32| {
                        if auf(x, 1.0) {
                            Some(true)
                        } else if auf(x, 0.0) {
                            Some(false)
                        } else {
                            None
                        }
                    };
                    Some([bit(andere[0])?, bit(andere[1])?])
                })
                .collect();
            let alle = [[false, false], [false, true], [true, false], [true, true]];
            if alle.iter().all(|e| ecken.contains(e)) {
                return seite(face);
            }
        }
    }
    0
}

/// Was die Blöcke einer Familie dem Strahl zur Sonne in den Weg stellen.
pub struct Sonnenform {
    /// Je Alternative ihr Modell, und für Lava dasselbe mit ihr auf voller
    /// Höhe, wenn dieselbe darüber steht.
    modelle: Vec<(Schattenmodell, Option<Schattenmodell>)>,
    /// Jede Alternative belegt alle sechs Seiten ihres Würfels ganz und
    /// deckend: Jeder Strahl, der den Würfel berührt, trifft.
    pub wuerfel: bool,
    /// Kein Dreieck, wie reines Wasser: Die Sonne geht ohne Test durch.
    pub leer: bool,
    /// Die Würfel um den Block, in die ein Modell ragt, relativ zu ihm im
    /// Blick, von und bis einschliesslich.
    pub zellen: [[i32; 3]; 2],
    /// Eine Bodenpflanze: Sie dämpft den Strahl, statt ihn zu decken.
    pub pflanze: bool,
}

impl Sonnenform {
    /// Die Form aus den Modellen der Alternativen; `voll` gibt zu einem
    /// Modell das mit seiner Flüssigkeit auf voller Höhe.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        models: &[(u32, BakedModel)],
        lava: bool,
        voll: impl Fn(&BakedModel) -> BakedModel,
        richtung: Richtung,
        textures: &Textures,
        masken: &mut Masken,
        pflanze: bool,
    ) -> Sonnenform {
        let modelle: Vec<(Schattenmodell, Option<Schattenmodell>)> = models
            .iter()
            .map(|(_, model)| {
                let eigen = Schattenmodell::new(model, richtung, textures, masken);
                let hoch =
                    lava.then(|| Schattenmodell::new(&voll(model), richtung, textures, masken));
                (eigen, hoch)
            })
            .collect();
        let alle = || {
            modelle
                .iter()
                .flat_map(|(eigen, hoch)| std::iter::once(eigen).chain(hoch))
        };
        let leer = alle().all(|m| m.dreiecke.is_empty());
        let mut zellen = [[0; 3], [0; 3]];
        for m in alle().filter(|m| !m.dreiecke.is_empty()) {
            let von = m.lo.map(|c| (c + 1e-4).floor() as i32);
            let bis: [i32; 3] =
                std::array::from_fn(|k| ((m.hi[k] - 1e-4).ceil() as i32 - 1).max(von[k]));
            zellen = [
                std::array::from_fn(|k| zellen[0][k].min(von[k])),
                std::array::from_fn(|k| zellen[1][k].max(bis[k])),
            ];
        }
        Sonnenform {
            wuerfel: modelle
                .iter()
                .all(|(eigen, _)| eigen.ganze_seiten == 0b11_1111),
            leer,
            zellen,
            pflanze,
            modelle,
        }
    }

    /// Trifft der Strahl `o + t·d` mit `0 ≤ t ≤ weite` eine deckende Stelle
    /// der Alternative `wahl`? `o` liegt relativ zum Ursprung des Blocks im
    /// Blick. `voll`: Dieselbe Flüssigkeit steht darüber. `weg`: die Seiten
    /// der Welt, zu denen Flächen aus Lava entfallen, Bits nach [`seite`].
    #[allow(clippy::too_many_arguments)]
    pub fn trifft(
        &self,
        wahl: usize,
        voll: bool,
        weg: u8,
        masken: &Masken,
        o: [f32; 3],
        d: [f32; 3],
        weite: f32,
    ) -> bool {
        let (eigen, hoch) = &self.modelle[wahl];
        let m = match hoch {
            Some(hoch) if voll => hoch,
            _ => eigen,
        };
        m.trifft(weg, masken, o, d, weite)
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Der Punkt `p` einer achsparallelen Fläche mit Normale `n`, in ihrer
/// Ebene auf die Mitte seines Sechzehntels gerundet; auf anderen Flächen
/// bleibt er.
pub fn texel_mitte(p: [f64; 3], n: [f32; 3]) -> [f64; 3] {
    let Some(k) = (0..3).find(|&k| n[k].abs() > 0.999) else {
        return p;
    };
    std::array::from_fn(|j| {
        if j == k {
            p[j]
        } else {
            ((p[j] * 16.0).floor() + 0.5) / 16.0
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganze_seite_nur_fuer_die_volle_seite() {
        let oben = [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ];
        assert_eq!(ganze_seite(&oben), seite(Face::Up));
        let halb = [
            [0.0, 0.5, 0.0],
            [0.0, 0.5, 1.0],
            [1.0, 0.5, 1.0],
            [1.0, 0.5, 0.0],
        ];
        assert_eq!(ganze_seite(&halb), 0);
        let schmal = [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.5],
            [1.0, 1.0, 0.5],
            [1.0, 1.0, 0.0],
        ];
        assert_eq!(ganze_seite(&schmal), 0);
    }

    #[test]
    fn texel_mitte_rundet_nur_in_der_ebene() {
        let p = texel_mitte([0.03, 1.0, 0.99], [0.0, 1.0, 0.0]);
        assert_eq!(p, [0.03125, 1.0, 0.96875]);
        let schraeg = [0.6, 0.0, 0.8];
        assert_eq!(texel_mitte([0.03, 0.2, 0.99], schraeg), [0.03, 0.2, 0.99]);
    }
}
