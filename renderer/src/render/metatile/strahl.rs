//! Der Strahl zur Sonne durch die Welt im Chunk-Cache, für Cinematic.
//! Siehe docs/renderer/cinematic.md, „Schatten“.

use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Result;

use crate::assets::Face;
use crate::assets::blockstate::seite;
use crate::assets::fluid::Fluid;

use super::super::sprites::Family;
use super::{ChunkCache, Loaded, PRESENT, Tabelle, spalten_im_blick};

/// Was der schnelle Gang von einer Section weiss, je Spalte `z * 16 + x` im
/// Blick ein Wort, Bit `y`.
pub(super) struct Bits {
    /// Zellen, die er prüft: ein Block mit Dreiecken, oder eine Zelle, in
    /// die das Modell eines Nachbarn ragt.
    arbeit: [u16; 256],
    /// Volle deckende Würfel ([`Sonnenform::wuerfel`]): Hier endet jeder
    /// Strahl, ohne Test.
    ///
    /// [`Sonnenform::wuerfel`]: super::super::sonne::Sonnenform::wuerfel
    wuerfel: [u16; 256],
    /// Zellen, in die das Modell eines Nachbarn ragt.
    ueber: [u16; 256],
    /// Je Würfel aus 4 × 4 × 4 Zellen ein Bit, ob dort Arbeit ist:
    /// `x / 4 + 4 · (z / 4) + 16 · (y / 4)`.
    bricks: u64,
}

/// Eine Section im Blick: Chunk in x, Section in y, Chunk in z.
type SectionKey = (i32, i32, i32);

/// Was der schnelle Gang von einem Chunk weiss, sobald ein Strahl ihn
/// betritt.
pub(super) struct Saeule {
    /// Über dieser Zelle hält im Chunk nichts den Strahl auf, `i32::MIN`
    /// ohne Block.
    decke: i32,
    /// Je Section, in die Modelle aus Nachbarn ragen, diese Zellen.
    ueber: HashMap<i8, Box<[u16; 256]>>,
    /// Je Section ihre [`Bits`], sobald ein Strahl sie betritt; `None` ohne
    /// Arbeit.
    sections: Tabelle<i8, Option<Rc<Bits>>>,
}

/// Die Blöcke eines Chunks, deren Modell für die Sonne aus dem Würfel ragt,
/// im Blick: für die Zellen [`Bits::ueber`] der Nachbarn.
pub(super) fn ragende(loaded: &Loaded, sprites: &super::SpriteSet) -> Vec<[i32; 3]> {
    let richtung = sprites.projection().richtung();
    let ragt = |index: &Option<u32>| {
        index.is_some_and(|i| {
            sprites
                .family(i)
                .sonne
                .as_ref()
                .is_some_and(|form| !form.leer && form.zellen != [[0; 3]; 2])
        })
    };
    let mut out = Vec::new();
    for (section, families) in loaded.chunk.sections().iter().zip(&loaded.families) {
        if !families.iter().any(ragt) {
            continue;
        }
        let y0 = i32::from(section.y) * 16;
        section.blocks().for_each_index(4096, |i, p| {
            if families.get(p).is_some_and(ragt) {
                let (x, z) = (
                    loaded.chunk.x * 16 + (i & 15) as i32,
                    loaded.chunk.z * 16 + (i >> 4 & 15) as i32,
                );
                let [bx, bz] = richtung.in_den_blick([x, z]);
                out.push([bx, y0 + (i >> 8) as i32, bz]);
            }
        });
    }
    out
}

/// Was ein Block auf dem Strahl zur Sonne bewirkt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Wirkung {
    Nichts,
    /// Eine Bodenpflanze, die er trifft: Sie dämpft.
    Daempft,
    Deckt,
}

/// Ein Gang Zelle für Zelle durch das Blockgitter im Blick (Amanatides und
/// Woo): `zelle` ist die, in die der Strahl bei `t` eintritt.
pub(super) struct Gang {
    pub zelle: [i32; 3],
    pub t: f64,
    schritt: [i32; 3],
    naechste: [f64; 3],
    delta: [f64; 3],
    /// `1 / d` je Achse.
    kehr: [f64; 3],
}

/// `x.floor() as i32` ohne Aufruf der libm, gleich für |x| < 2³¹.
fn boden(x: f64) -> i32 {
    let i = x as i32;
    i - i32::from(f64::from(i) > x)
}

impl Gang {
    /// Ab `p0` in Richtung `d`, ohne eine Komponente 0.
    pub(super) fn new(p0: [f64; 3], d: [f64; 3]) -> Gang {
        let zelle = [boden(p0[0]), boden(p0[1]), boden(p0[2])];
        let richtung = |c: f64| if c > 0.0 { 1 } else { -1 };
        let schritt = [richtung(d[0]), richtung(d[1]), richtung(d[2])];
        let naechste = |k: usize| {
            let grenze = zelle[k] + i32::from(schritt[k] > 0);
            (f64::from(grenze) - p0[k]) / d[k]
        };
        let kehr = [1.0 / d[0], 1.0 / d[1], 1.0 / d[2]];
        Gang {
            zelle,
            t: 0.0,
            schritt,
            naechste: [naechste(0), naechste(1), naechste(2)],
            delta: [kehr[0].abs(), kehr[1].abs(), kehr[2].abs()],
            kehr,
        }
    }

    /// Aus dem Kasten `lo` bis `hi` (ausschliesslich) hinaus, ohne die
    /// Zellen dazwischen: in die Zelle hinter der Grenze, die der Strahl
    /// zuerst erreicht. Achsen ohne `achsen` begrenzen den Kasten nicht.
    fn springe(
        &mut self,
        p0: [f64; 3],
        d: [f64; 3],
        lo: [i32; 3],
        hi: [i32; 3],
        achsen: [bool; 3],
    ) {
        let aus = |k: usize| {
            if !achsen[k] {
                return f64::INFINITY;
            }
            let grenze = if self.schritt[k] > 0 { hi[k] } else { lo[k] };
            (f64::from(grenze) - p0[k]) * self.kehr[k]
        };
        let t_aus = [aus(0), aus(1), aus(2)];
        let k = if t_aus[0] <= t_aus[1] && t_aus[0] <= t_aus[2] {
            0
        } else if t_aus[1] <= t_aus[2] {
            1
        } else {
            2
        };
        let t = t_aus[k].max(self.t);
        for j in 0..3 {
            self.zelle[j] = if j == k {
                if self.schritt[k] > 0 {
                    hi[k]
                } else {
                    lo[k] - 1
                }
            } else {
                let c = boden(p0[j] + t * d[j]);
                if achsen[j] {
                    c.clamp(lo[j], hi[j] - 1)
                } else {
                    c
                }
            };
            let grenze = self.zelle[j] + i32::from(self.schritt[j] > 0);
            self.naechste[j] = (f64::from(grenze) - p0[j]) * self.kehr[j];
        }
        self.t = t;
    }

    /// In die nächste Zelle, über die Grenze, die der Strahl zuerst erreicht.
    pub(super) fn weiter(&mut self) {
        let n = self.naechste;
        let k = if n[0] <= n[1] && n[0] <= n[2] {
            0
        } else if n[1] <= n[2] {
            1
        } else {
            2
        };
        self.t = n[k];
        self.zelle[k] += self.schritt[k];
        self.naechste[k] += self.delta[k];
    }
}

/// Die Richtung zur Sonne ohne eine Komponente 0, für [`Gang`].
pub(super) fn ohne_null(d: [f32; 3]) -> [f32; 3] {
    d.map(|c| if c.abs() < 1e-9 { 1e-9 } else { c })
}

impl ChunkCache<'_> {
    /// Wie viel Sonne am Punkt `p0` im Blick ankommt: 0 hinter einer
    /// deckenden Stelle, sonst [`Look::pflanzen`] je Bodenpflanze auf dem
    /// Strahl, ausser der, auf der er beginnt (`eigen`, der Block des Draws).
    ///
    /// [`Look::pflanzen`]: super::super::look::Look::pflanzen
    pub fn sonne(&mut self, p0: [f64; 3], eigen: [i32; 3]) -> Result<f32> {
        self.sonne_gang(p0, eigen)
    }

    /// [`ChunkCache::sonne`] als schneller Gang: über der Decke eines
    /// Chunks, durch eine Section ohne Arbeit und durch einen Würfel aus
    /// 4 × 4 × 4 Zellen ohne Arbeit springt er hinaus; in einen vollen
    /// deckenden Würfel tritt er ohne Test; sonst prüft er die Zelle wie
    /// [`ChunkCache::sonne_bezug`]. Dasselbe Ergebnis, denn ein Block, den
    /// der Strahl nicht trifft, ändert nichts, gleich ob er geprüft wird.
    pub(super) fn sonne_gang(&mut self, p0: [f64; 3], eigen: [i32; 3]) -> Result<f32> {
        let sprites = self.sprites;
        let look = sprites.kino().expect("Cinematic").look();
        let d = ohne_null(sprites.kino().expect("Cinematic").sonne());
        let d64 = d.map(f64::from);
        let weite = f64::from(look.sonne_weite);
        let mut getestet: Vec<[i32; 3]> = Vec::new();
        let mut licht = 1.0;
        let mut gang = Gang::new(p0, d64);
        let mut chunk: Option<((i32, i32), usize, i32)> = None;
        let mut section: Option<(SectionKey, Option<Rc<Bits>>)> = None;
        while gang.t <= weite {
            let c = gang.zelle;
            let key = (c[0] >> 4, c[2] >> 4);
            let (slot, decke) = match chunk {
                Some((k, slot, decke)) if k == key => (slot, decke),
                _ => {
                    let (slot, decke) = self.saeule(key)?;
                    chunk = Some((key, slot, decke));
                    (slot, decke)
                }
            };
            let basis = [key.0 * 16, c[1] & !15, key.1 * 16];
            if d64[1] > 0.0 && c[1] > decke {
                gang.springe(
                    p0,
                    d64,
                    basis,
                    [basis[0] + 16, 0, basis[2] + 16],
                    [true, false, true],
                );
                continue;
            }
            let skey = (key.0, c[1] >> 4, key.1);
            if !section.as_ref().is_some_and(|(k, _)| *k == skey) {
                section = Some((skey, self.sonnen_bits(slot, c[1] >> 4)?));
            }
            let Some(bits) = section.as_ref().and_then(|(_, bits)| bits.as_deref()) else {
                gang.springe(p0, d64, basis, basis.map(|b| b + 16), [true; 3]);
                continue;
            };
            let r = [c[0] - basis[0], c[1] - basis[1], c[2] - basis[2]];
            if bits.bricks >> ((r[0] >> 2) + 4 * (r[2] >> 2) + 16 * (r[1] >> 2)) & 1 == 0 {
                let lo = [c[0] & !3, c[1] & !3, c[2] & !3];
                gang.springe(p0, d64, lo, lo.map(|b| b + 4), [true; 3]);
                continue;
            }
            let (col, bit) = ((r[2] * 16 + r[0]) as usize, 1u16 << r[1]);
            if bits.wuerfel[col] & bit != 0 {
                return Ok(0.0);
            }
            if bits.arbeit[col] & bit != 0 {
                let mut pruefe = |cache: &mut Self, b: [i32; 3]| -> Result<Wirkung> {
                    let Some((family, _)) = cache.block_at(b[0], b[1], b[2])? else {
                        return Ok(Wirkung::Nichts);
                    };
                    let Some(form) = &family.sonne else {
                        return Ok(Wirkung::Nichts);
                    };
                    let r = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
                    if (0..3).any(|k| r[k] < form.zellen[0][k] || r[k] > form.zellen[1][k]) {
                        return Ok(Wirkung::Nichts);
                    }
                    // Ein Modell, das ragt, prüft der Gang einmal je Strahl.
                    if form.zellen != [[0; 3]; 2] {
                        if getestet.contains(&b) {
                            return Ok(Wirkung::Nichts);
                        }
                        getestet.push(b);
                    }
                    cache.wirkung(b, family, p0, d, eigen)
                };
                let [lo, hi] = match bits.ueber[col] & bit {
                    0 => [[0; 3]; 2],
                    _ => sprites.sonne_reich(),
                };
                for oy in lo[1]..=hi[1] {
                    for oz in lo[2]..=hi[2] {
                        for ox in lo[0]..=hi[0] {
                            match pruefe(self, [c[0] - ox, c[1] - oy, c[2] - oz])? {
                                Wirkung::Deckt => return Ok(0.0),
                                Wirkung::Daempft => licht *= look.pflanzen,
                                Wirkung::Nichts => {}
                            }
                        }
                    }
                }
            }
            gang.weiter();
        }
        Ok(licht)
    }

    /// Die Säule des Chunks `key` im Blick, beim ersten Mal angelegt; gibt
    /// seinen Slot und ihre Decke, `i32::MIN` ohne Chunk. Dafür werden die
    /// Chunks rundum geladen: Aus ihnen können Modelle hineinragen.
    fn saeule(&mut self, key: (i32, i32)) -> Result<(usize, i32)> {
        let i = self.slot(key)?;
        match self.slots[i].loaded.as_ref() {
            None => return Ok((i, i32::MIN)),
            Some(Loaded { sonne: Some(s), .. }) => return Ok((i, s.decke)),
            Some(_) => {}
        }
        let sprites = self.sprites;
        let [lo, hi] = sprites.sonne_reich();
        let mut ueber: HashMap<i8, Box<[u16; 256]>> = HashMap::new();
        let mut decke = i32::MIN;
        if [lo, hi] != [[0; 3]; 2] {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let j = self.slot((key.0 + dx, key.1 + dz))?;
                    let Some(nachbar) = self.slots[j].loaded.as_mut() else {
                        continue;
                    };
                    if nachbar.ragende.is_none() {
                        nachbar.ragende = Some(ragende(nachbar, sprites));
                    }
                    let liste = nachbar.ragende.clone().unwrap_or_default();
                    for b in liste {
                        let Some((family, _)) = self.block_at(b[0], b[1], b[2])? else {
                            continue;
                        };
                        let Some(form) = &family.sonne else {
                            continue;
                        };
                        let [von, bis] = form.zellen;
                        for y in b[1] + von[1]..=b[1] + bis[1] {
                            for z in b[2] + von[2]..=b[2] + bis[2] {
                                for x in b[0] + von[0]..=b[0] + bis[0] {
                                    if [x, y, z] == b || (x >> 4, z >> 4) != key {
                                        continue;
                                    }
                                    let Ok(sy) = i8::try_from(y >> 4) else {
                                        continue;
                                    };
                                    let woerter =
                                        ueber.entry(sy).or_insert_with(|| Box::new([0; 256]));
                                    woerter[((z & 15) * 16 + (x & 15)) as usize] |= 1 << (y & 15);
                                    decke = decke.max(y);
                                }
                            }
                        }
                    }
                }
            }
        }
        let i = self.slot(key)?;
        let loaded = self.slots[i].loaded.as_mut().expect("eben geladen");
        for (section, masks) in loaded.chunk.sections().iter().zip(&loaded.masks).rev() {
            let oder = masks
                .as_ref()
                .map_or(0, |m| m.bits[PRESENT].iter().fold(0, |a, &w| a | w));
            if oder != 0 {
                decke = decke.max(i32::from(section.y) * 16 + 15 - oder.leading_zeros() as i32);
                break;
            }
        }
        loaded.sonne = Some(Box::new(Saeule {
            decke,
            ueber,
            sections: Tabelle::default(),
        }));
        Ok((i, decke))
    }

    /// Die [`Bits`] der Section `sy` im Chunk im Slot `i`, beim ersten Mal
    /// gerechnet; `None` ohne Arbeit. Die Säule muss stehen.
    fn sonnen_bits(&mut self, i: usize, sy: i32) -> Result<Option<Rc<Bits>>> {
        let sprites = self.sprites;
        let Some(loaded) = self.slots[i].loaded.as_mut() else {
            return Ok(None);
        };
        let Ok(sy) = i8::try_from(sy) else {
            return Ok(None);
        };
        let saeule = loaded.sonne.as_mut().expect("die Säule steht");
        if let Some(bits) = saeule.sections.get(&sy) {
            return Ok(bits.clone());
        }
        let mut bits = Bits {
            arbeit: [0; 256],
            wuerfel: [0; 256],
            ueber: saeule.ueber.get(&sy).map_or([0; 256], |w| **w),
            bricks: 0,
        };
        if let Some(s) = loaded.chunk.section_index(sy) {
            let section = &loaded.chunk.sections()[s];
            // Je Paletteneintrag: Bit 0 Arbeit, Bit 1 voller Würfel.
            let art: Vec<u8> = loaded.families[s]
                .iter()
                .map(
                    |index| match index.and_then(|i| sprites.family(i).sonne.as_ref()) {
                        Some(form) if form.wuerfel => 3,
                        Some(form) if !form.leer => 1,
                        _ => 0,
                    },
                )
                .collect();
            if art.iter().any(|&a| a != 0) {
                let blick = spalten_im_blick(sprites.projection().richtung());
                let spalte = |col: usize| blick.as_ref().map_or(col, |b| b[col] as usize);
                section.blocks().for_each_index(4096, |i, p| {
                    let a = art.get(p).copied().unwrap_or(0);
                    if a != 0 {
                        let (col, bit) = (spalte(i & 255), 1u16 << (i >> 8));
                        bits.arbeit[col] |= bit;
                        if a & 2 != 0 {
                            bits.wuerfel[col] |= bit;
                        }
                    }
                });
            }
        }
        for col in 0..256 {
            bits.arbeit[col] |= bits.ueber[col];
            let mut w = bits.arbeit[col];
            while w != 0 {
                let y = w.trailing_zeros() as usize;
                w &= w - 1;
                bits.bricks |= 1 << ((col & 15) / 4 + 4 * (col / 64) + 16 * (y / 4));
            }
        }
        let bits = (bits.bricks != 0).then(|| Rc::new(bits));
        let saeule = self.slots[i]
            .loaded
            .as_mut()
            .and_then(|l| l.sonne.as_mut())
            .expect("die Säule steht");
        saeule.sections.insert(sy, bits.clone());
        Ok(bits)
    }

    /// [`ChunkCache::sonne`] als langsamer Bezug: Zelle für Zelle bis zur
    /// Weite, je Zelle jeder Block, dessen Modell in sie ragt, mit dem Test
    /// seiner Flächen. Jeder Block einmal je Strahl.
    pub fn sonne_bezug(&mut self, p0: [f64; 3], eigen: [i32; 3]) -> Result<f32> {
        let sprites = self.sprites;
        let look = sprites.kino().expect("Cinematic").look();
        let d = ohne_null(sprites.kino().expect("Cinematic").sonne());
        let [lo, hi] = sprites.sonne_reich();
        let mut getestet: Vec<[i32; 3]> = Vec::new();
        let mut licht = 1.0;
        let mut gang = Gang::new(p0, d.map(f64::from));
        while gang.t <= f64::from(look.sonne_weite) {
            let c = gang.zelle;
            for oy in lo[1]..=hi[1] {
                for oz in lo[2]..=hi[2] {
                    for ox in lo[0]..=hi[0] {
                        let b = [c[0] - ox, c[1] - oy, c[2] - oz];
                        if getestet.contains(&b) {
                            continue;
                        }
                        let Some((family, _)) = self.block_at(b[0], b[1], b[2])? else {
                            continue;
                        };
                        let Some(form) = &family.sonne else {
                            continue;
                        };
                        let r = [ox, oy, oz];
                        if (0..3).any(|k| r[k] < form.zellen[0][k] || r[k] > form.zellen[1][k]) {
                            continue;
                        }
                        getestet.push(b);
                        match self.wirkung(b, family, p0, d, eigen)? {
                            Wirkung::Deckt => return Ok(0.0),
                            Wirkung::Daempft => licht *= look.pflanzen,
                            Wirkung::Nichts => {}
                        }
                    }
                }
            }
            gang.weiter();
        }
        Ok(licht)
    }

    /// Was der Block `b` im Blick mit `family` dem Strahl `p0 + t·d` bis zur
    /// Weite entgegenstellt. Die Bodenpflanze `eigen`, auf der der Strahl
    /// beginnt, und ihre obere Hälfte bewirken nichts.
    pub(super) fn wirkung(
        &mut self,
        b: [i32; 3],
        family: &Family,
        p0: [f64; 3],
        d: [f32; 3],
        eigen: [i32; 3],
    ) -> Result<Wirkung> {
        let sprites = self.sprites;
        let weite = sprites.kino().expect("Cinematic").look().sonne_weite;
        let Some(form) = family.sonne.as_ref().filter(|form| !form.leer) else {
            return Ok(Wirkung::Nichts);
        };
        if form.pflanze
            && (b == eigen || (b == [eigen[0], eigen[1] + 1, eigen[2]] && family.obere_haelfte()))
        {
            return Ok(Wirkung::Nichts);
        }
        // Gewürfelt wird in der Welt.
        let [wx, wz] = self.richtung.in_die_welt([b[0], b[2]]);
        let Some(wahl) = family.wahl([wx, b[1], wz]) else {
            return Ok(Wirkung::Nichts);
        };
        // Lava reicht unter derselben bis zur Kante, und ihre Flächen
        // entfallen zu derselben und vor einer vollen Seite, wie
        // `LiquidBlockRenderer.shouldRenderFace`.
        let (mut voll, mut weg) = (false, 0u8);
        if let Some((art @ Fluid::Lava, _)) = family.fluid {
            let gleich =
                |f: Option<&Family>| f.is_some_and(|f| f.fluid.is_some_and(|(k, _)| k == art));
            voll = gleich(self.family_at(b[0], b[1] + 1, b[2])?);
            for face in [
                Face::Down,
                Face::Up,
                Face::North,
                Face::South,
                Face::West,
                Face::East,
            ] {
                let [dx, dy, dz] = self.richtung.versatz_in_den_blick(face.versatz());
                let nachbar = self.family_at(b[0] + dx, b[1] + dy, b[2] + dz)?;
                if gleich(nachbar)
                    || nachbar.is_some_and(|n| n.voll & seite(face.gegenueber()) != 0)
                {
                    weg |= seite(face);
                }
            }
        }
        let o = std::array::from_fn(|k| (p0[k] - f64::from(b[k])) as f32);
        Ok(
            if form.trifft(wahl, voll, weg, sprites.masken(), o, d, weite) {
                if form.pflanze {
                    Wirkung::Daempft
                } else {
                    Wirkung::Deckt
                }
            } else {
                Wirkung::Nichts
            },
        )
    }
}
