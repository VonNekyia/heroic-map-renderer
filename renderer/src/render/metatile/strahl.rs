//! Der Strahl zur Sonne durch die Welt im Chunk-Cache, für Cinematic.
//! Siehe docs/renderer/cinematic.md, „Schatten“.

use std::collections::BTreeMap;
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
    /// Volle deckende Würfel ([`Sonnenform::wuerfel`]) ausser oberen
    /// Hälften: Hier endet jeder Strahl, ohne Test.
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
    /// Die höchste [`Saeule::decke`] der Chunks, die ein Strahl von hier
    /// bis zur Weite erreichen kann, sobald gebraucht: Darüber ist er frei.
    horizont: Option<i32>,
    /// Je Section, in die Modelle aus Nachbarn ragen, diese Zellen.
    ueber: Tabelle<i8, Box<[u16; 256]>>,
    /// Je Section ihre [`Bits`], sobald ein Strahl sie betritt; `None` ohne
    /// Arbeit.
    sections: Tabelle<i8, Option<Rc<Bits>>>,
    /// Die Bits „frei zur Sonne“, sobald ein Strahl hier beginnt.
    frei: Option<Rc<Frei>>,
}

/// Die Bits „frei zur Sonne“ eines Chunks: die unterste Lage und je Spalte
/// `z * 16 + x` im Blick ein Wort, Bit `m` für die Lage `unterste + m`.
/// `None` ohne Block in Reichweite. Sie hängen nicht am scale, ein Cache
/// der nativen Stufen behält sie im Vorrat.
pub(super) type Frei = Option<(i32, Box<[u128; 256]>)>;

/// Wie viele Lagen unter dem Horizont die Bits „frei zur Sonne“ einer Spalte
/// abdecken.
const LAGEN: i32 = 128;

/// Ein Versatz zu einer Spalte, deren Zellen das Prisma zur Sonne einer
/// Startzelle in den Lagen `k0 .. k0 + n` über ihr berühren kann, siehe
/// [`versaetze`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Versatz {
    pub i: i32,
    pub j: i32,
    pub k0: u32,
    pub n: u32,
}

/// Die Versätze des Prismas zur Sonne `d` im Blick für die Bits „frei zur
/// Sonne“: Das Prisma einer Zelle sind alle Punkte `p + t·d` mit `p` in ihr,
/// `t ≥ 0` und `t` höchstens `weite`. In der Lage `k` über ihr ist `t` von
/// `max(0, k − 1) / d_y` bis `(k + 1) / d_y`; je Achse berührt es die
/// Zellen, deren geschlossener Würfel diese Spanne schneidet, nur vorwärts
/// wie der Gang. Um 10⁻⁶ weiter, so liegt jede Zelle, die der Gang in f64
/// betritt, darin. Bis [`LAGEN`] Lagen über der Zelle.
/// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
pub(crate) fn versaetze(d: [f32; 3], weite: f32) -> Vec<Versatz> {
    const EPS: f64 = 1e-6;
    let d = ohne_null(d).map(f64::from);
    if d[1] <= 0.0 {
        return Vec::new();
    }
    let k_max = ((f64::from(weite) * d[1]).floor() as i32 + 1).min(LAGEN - 1);
    let spanne = |c: f64, k: i32| -> (i32, i32) {
        let a = c.abs() / d[1];
        let (nah, fern) = (a * f64::from((k - 1).max(0)), a * f64::from(k + 1));
        if c > 0.0 {
            (
                ((nah - EPS).ceil() as i32 - 1).max(0),
                (1.0 + fern + EPS).floor() as i32,
            )
        } else {
            (
                (-fern - EPS).ceil() as i32 - 1,
                ((1.0 - nah + EPS).floor() as i32).min(0),
            )
        }
    };
    // Je Spalte die Lagen, kleinste, grösste und wie viele.
    let mut je: BTreeMap<(i32, i32), (i32, i32, i32)> = BTreeMap::new();
    for k in 0..=k_max {
        let (x0, x1) = spanne(d[0], k);
        let (z0, z1) = spanne(d[2], k);
        for j in z0..=z1 {
            for i in x0..=x1 {
                let e = je.entry((i, j)).or_insert((k, k, 0));
                (e.0, e.1, e.2) = (e.0.min(k), e.1.max(k), e.2 + 1);
            }
        }
    }
    je.into_iter()
        .map(|((i, j), (von, bis, anzahl))| {
            // Die Spannen wachsen mit k in Richtung der Sonne, also sind die
            // Lagen jeder Spalte ein Lauf.
            debug_assert_eq!(anzahl, bis - von + 1, "Lagen von ({i}, {j}) mit Lücke");
            Versatz {
                i,
                j,
                k0: von as u32,
                n: (bis - von + 1) as u32,
            }
        })
        .collect()
}

/// Die Blöcke eines Chunks, deren Modell für die Sonne aus dem Würfel ragt,
/// im Blick: für die Zellen [`Bits::ueber`] der Nachbarn.
pub(super) fn ragende(loaded: &Loaded, sprites: &super::SpriteSet) -> Rc<[[i32; 3]]> {
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
    out.into()
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

/// Die Richtung zur Sonne ohne eine Komponente 0, für [`Gang`] und
/// [`super::super::sonne::Sonnenform`].
pub(crate) fn ohne_null(d: [f32; 3]) -> [f32; 3] {
    d.map(|c| if c.abs() < 1e-9 { 1e-9 } else { c })
}

impl ChunkCache<'_> {
    /// Wie viel Sonne am Punkt `p0` im Blick ankommt: 0 hinter einer
    /// deckenden Stelle, sonst [`Look::pflanzen`] je Bodenpflanze auf dem
    /// Strahl, ausser der, auf der er beginnt (`eigen`, der Block des Draws).
    ///
    /// Ist die Startzelle frei zur Sonne ([`ChunkCache::frei_zur_sonne`]),
    /// kommt alles an, ohne Gang. Sonst geht der schnelle Gang
    /// ([`ChunkCache::sonne_im_gang`]) bis zur ersten freien Zelle.
    ///
    /// [`Look::pflanzen`]: super::super::look::Look::pflanzen
    pub fn sonne(&mut self, p0: [f64; 3], eigen: [i32; 3]) -> Result<f32> {
        if self.frei_zur_sonne(p0)? {
            // Jeder Test im Debug-Build schickt den Strahl auch durch den
            // vollen Gang.
            debug_assert_eq!(self.gang(p0, eigen, false)?, 1.0, "frei: {p0:?}");
            return Ok(1.0);
        }
        self.sonne_im_gang(p0, eigen)
    }

    /// Ob ein Strahl von `p0` nach den Bits „frei zur Sonne“ nichts trifft:
    /// Keine Zelle, die das Prisma seiner Startzelle unter dem Horizont
    /// berühren kann, hat Arbeit für den Gang. Hinreichend, nicht nötig;
    /// wo es `false` sagt, entscheidet der Gang.
    /// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
    pub fn frei_zur_sonne(&mut self, p0: [f64; 3]) -> Result<bool> {
        if self
            .sprites
            .kino()
            .expect("Cinematic")
            .versaetze()
            .is_empty()
        {
            return Ok(false);
        }
        let c = [boden(p0[0]), boden(p0[1]), boden(p0[2])];
        let key = (c[0] >> 4, c[2] >> 4);
        let (slot, _) = self.saeule(key)?;
        self.frei_in(slot, key, c)
    }

    /// Ob die Zelle `c` im Chunk `key` im Slot `slot` frei zur Sonne ist,
    /// wie [`ChunkCache::frei_zur_sonne`]; die Säule muss stehen.
    fn frei_in(&mut self, slot: usize, key: (i32, i32), c: [i32; 3]) -> Result<bool> {
        if self
            .sprites
            .kino()
            .expect("Cinematic")
            .versaetze()
            .is_empty()
        {
            return Ok(false);
        }
        let Some((unterste, wort)) =
            self.frei_bits(slot, key, ((c[2] & 15) * 16 + (c[0] & 15)) as usize)?
        else {
            return Ok(false);
        };
        let m = c[1] - unterste;
        Ok((0..LAGEN).contains(&m) && wort >> m & 1 != 0)
    }

    /// Das Wort der Bits „frei zur Sonne“ der Spalte `col` im Chunk `key`
    /// im Slot `i`, mit seiner untersten Lage; beim ersten Mal für den
    /// ganzen Chunk gerechnet. `None` ohne Block in Reichweite.
    fn frei_bits(&mut self, i: usize, key: (i32, i32), col: usize) -> Result<Option<(i32, u128)>> {
        let Some(loaded) = self.slots[i].loaded.as_ref() else {
            return Ok(None);
        };
        let wort = |frei: &Frei| frei.as_ref().map(|(unterste, w)| (*unterste, w[col]));
        if let Some(frei) = loaded.sonne.as_ref().and_then(|s| s.frei.as_ref()) {
            return Ok(wort(frei));
        }
        let gemerkt = self.vorrat.as_ref().and_then(|v| v.chunks.get(&key));
        if let Some(frei) = gemerkt.and_then(|g| g.frei.clone()) {
            let antwort = wort(&frei);
            if let Some(s) = self.slots[i].loaded.as_mut().and_then(|l| l.sonne.as_mut()) {
                s.frei = Some(frei);
            }
            return Ok(antwort);
        }
        let sprites = self.sprites;
        let kino = sprites.kino().expect("Cinematic");
        let d = ohne_null(kino.sonne());
        let weite = f64::from(kino.look().sonne_weite);
        let horizont = self.horizont(i, key, d, weite)?;
        let frei = if horizont == i32::MIN {
            None
        } else {
            let unterste = horizont + 1 - LAGEN;
            let versaetze = kino.versaetze();
            let (i0, i1) = versaetze
                .iter()
                .fold((0, 0), |(a, b), v| (a.min(v.i), b.max(v.i)));
            let (j0, j1) = versaetze
                .iter()
                .fold((0, 0), |(a, b), v| (a.min(v.j), b.max(v.j)));
            // Die Arbeit jeder Spalte, die ein Versatz erreicht, in den
            // Lagen ab `unterste`; darüber hat keine Zelle in Reichweite
            // Arbeit.
            let (x0, z0) = (key.0 * 16 + i0, key.1 * 16 + j0);
            let breite = (16 + i1 - i0) as usize;
            let mut arbeit = vec![0u128; breite * (16 + j1 - j0) as usize];
            for cz in z0 >> 4..=(key.1 * 16 + 15 + j1) >> 4 {
                for cx in x0 >> 4..=(key.0 * 16 + 15 + i1) >> 4 {
                    let (slot, decke) = self.saeule((cx, cz))?;
                    if decke < unterste {
                        continue;
                    }
                    for sy in unterste >> 4..=decke.min(horizont) >> 4 {
                        let Some(bits) = self.sonnen_bits(slot, sy)? else {
                            continue;
                        };
                        let versatz = sy * 16 - unterste;
                        for col in 0..256 {
                            let (x, z) = (cx * 16 + (col & 15) as i32, cz * 16 + (col >> 4) as i32);
                            if x < x0
                                || z < z0
                                || x >= x0 + breite as i32
                                || z > key.1 * 16 + 15 + j1
                            {
                                continue;
                            }
                            let w = u128::from(bits.arbeit[col]);
                            let w = if versatz >= 0 {
                                w << versatz
                            } else {
                                w >> -versatz
                            };
                            arbeit[(z - z0) as usize * breite + (x - x0) as usize] |= w;
                        }
                    }
                }
            }
            let mut woerter = Box::new([0u128; 256]);
            for (col, wort) in woerter.iter_mut().enumerate() {
                let (x, z) = (
                    key.0 * 16 + (col & 15) as i32,
                    key.1 * 16 + (col >> 4) as i32,
                );
                let mut gesperrt = 0u128;
                for v in versaetze {
                    let w = arbeit[(z + v.j - z0) as usize * breite + (x + v.i - x0) as usize];
                    let mut breit = w;
                    for s in 1..v.n {
                        breit |= w >> s;
                    }
                    gesperrt |= breit >> v.k0;
                }
                *wort = !gesperrt;
            }
            Some((unterste, woerter))
        };
        let frei = Rc::new(frei);
        let antwort = wort(&frei);
        if let Some(g) = self.vorrat.as_mut().and_then(|v| v.chunks.get_mut(&key)) {
            g.frei = Some(Rc::clone(&frei));
        }
        if let Some(s) = self.slots[i].loaded.as_mut().and_then(|l| l.sonne.as_mut()) {
            s.frei = Some(frei);
        }
        Ok(antwort)
    }

    /// [`ChunkCache::sonne`] ohne die Bits „frei zur Sonne“ der Startzelle,
    /// als schneller Gang: über der Decke eines Chunks, durch eine Section
    /// ohne Arbeit und durch einen Würfel aus 4 × 4 × 4 Zellen ohne Arbeit
    /// springt er hinaus; in einen vollen deckenden Würfel tritt er ohne
    /// Test; sonst prüft er die Zelle wie [`ChunkCache::sonne_bezug`].
    /// Dasselbe Ergebnis, denn ein Block, den der Strahl nicht trifft,
    /// ändert nichts, gleich ob er geprüft wird. In der ersten Zelle nach
    /// dem Start, die frei zur Sonne ist, endet er mit dem Licht, das er bis
    /// dahin hat: Der Rest des Strahls liegt in ihrem Prisma.
    /// Siehe docs/renderer/cinematic.md, „Frei zur Sonne“.
    pub fn sonne_im_gang(&mut self, p0: [f64; 3], eigen: [i32; 3]) -> Result<f32> {
        self.gang(p0, eigen, true)
    }

    /// [`ChunkCache::sonne_im_gang`], mit `frueh` bis zur ersten freien
    /// Zelle, ohne bis zur Weite.
    fn gang(&mut self, p0: [f64; 3], eigen: [i32; 3], frueh: bool) -> Result<f32> {
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
        let mut start = true;
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
            if frueh && !start && self.frei_in(slot, key, c)? {
                // Jeder Test im Debug-Build schickt den Strahl auch durch den
                // vollen Gang.
                debug_assert_eq!(self.gang(p0, eigen, false)?, licht, "frei ab {c:?}: {p0:?}");
                return Ok(licht);
            }
            start = false;
            let basis = [key.0 * 16, c[1] & !15, key.1 * 16];
            if d64[1] > 0.0 && c[1] > decke {
                if c[1] > self.horizont(slot, key, d, weite)? {
                    return Ok(licht);
                }
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
                    cache.wirkung(b, family, p0, eigen)
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
        let mut ueber: Tabelle<i8, Box<[u16; 256]>> = Tabelle::default();
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
                    let liste = Rc::clone(nachbar.ragende.as_ref().expect("eben gesetzt"));
                    for &b in liste.iter() {
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
            horizont: None,
            ueber,
            sections: Tabelle::default(),
            frei: None,
        }));
        Ok((i, decke))
    }

    /// [`Saeule::horizont`] des Chunks `key` im Slot `i`, beim ersten Mal
    /// gerechnet; ohne Chunk gibt es keine Säule, die ihn hält, dann jedes
    /// Mal. Siehe docs/renderer/cinematic.md, „Der schnelle Gang“.
    fn horizont(&mut self, i: usize, key: (i32, i32), d: [f32; 3], weite: f64) -> Result<i32> {
        let gemerkt = self.slots[i].loaded.as_ref().and_then(|l| l.sonne.as_ref());
        if let Some(h) = gemerkt.and_then(|s| s.horizont) {
            return Ok(h);
        }
        // ⌊·⌋ + 1 statt ⌈·⌉: auch bei einer ganzen Zahl ein Chunk mehr, für
        // die Rundung des Gangs in f64 an einer Chunkgrenze.
        let reicht = |c: f32| (weite * f64::from(c.abs()) / 16.0).floor() as i32 + 1;
        let schritt = |c: f32| if c > 0.0 { 1 } else { -1 };
        let mut h = i32::MIN;
        for j in 0..=reicht(d[2]) {
            for k in 0..=reicht(d[0]) {
                let (_, decke) =
                    self.saeule((key.0 + k * schritt(d[0]), key.1 + j * schritt(d[2])))?;
                h = h.max(decke);
            }
        }
        if let Some(s) = self.slots[i].loaded.as_mut().and_then(|l| l.sonne.as_mut()) {
            s.horizont = Some(h);
        }
        Ok(h)
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
            // Je Paletteneintrag: Bit 0 Arbeit, Bit 1 voller Würfel. Eine
            // obere Hälfte geht durch den Test, auch als voller Würfel: Über
            // ihrer Bodenpflanze bewirkt sie nichts, siehe `ChunkCache::wirkung`.
            let art: Vec<u8> = loaded.families[s]
                .iter()
                .map(|index| {
                    let family = index.map(|i| sprites.family(i));
                    match family.and_then(|f| f.sonne.as_ref().map(|form| (f, form))) {
                        Some((f, form)) if form.wuerfel && !f.obere_haelfte() => 3,
                        Some((_, form)) if !form.leer => 1,
                        _ => 0,
                    }
                })
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
                        match self.wirkung(b, family, p0, eigen)? {
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
    /// Weite entgegenstellt. Ist der Block `eigen`, auf dem der Strahl
    /// beginnt, eine Bodenpflanze, bewirken er und seine obere Hälfte
    /// nichts, auch wenn die selbst keine Bodenpflanze ist, wie die Blüte
    /// der Sonnenblume.
    pub(super) fn wirkung(
        &mut self,
        b: [i32; 3],
        family: &Family,
        p0: [f64; 3],
        eigen: [i32; 3],
    ) -> Result<Wirkung> {
        let sprites = self.sprites;
        let weite = f64::from(sprites.kino().expect("Cinematic").look().sonne_weite);
        let Some(form) = family.sonne.as_ref().filter(|form| !form.leer) else {
            return Ok(Wirkung::Nichts);
        };
        let oben = b == [eigen[0], eigen[1] + 1, eigen[2]] && family.obere_haelfte();
        if (b == eigen && form.pflanze)
            || (oben
                && self
                    .family_at(eigen[0], eigen[1], eigen[2])?
                    .and_then(|f| f.sonne.as_ref())
                    .is_some_and(|f| f.pflanze))
        {
            return Ok(Wirkung::Nichts);
        }
        // Gewürfelt wird in der Welt.
        let [wx, wz] = self.richtung.in_die_welt([b[0], b[2]]);
        let Some(wahl) = family.wahl([wx, b[1], wz]) else {
            return Ok(Wirkung::Nichts);
        };
        // Lava wie im Spiel, siehe docs/renderer/cinematic.md, „Schatten“.
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
        let o = std::array::from_fn(|k| p0[k] - f64::from(b[k]));
        Ok(
            if form.trifft(wahl, voll, weg, sprites.masken(), o, weite) {
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
