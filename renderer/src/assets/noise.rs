//! `Biome.BIOME_INFO_NOISE` aus 26.2: das Rauschen, nach dem der Sumpf je
//! Stelle eines von zwei Grün wählt.
//! Siehe docs/renderer/biomfarben.md, „Sumpfgras“.

use std::sync::LazyLock;

/// Der Zufall von `LegacyRandomSource`, derselbe LCG wie
/// `java.util.Random`.
pub struct JavaRandom(i64);

impl JavaRandom {
    const MULT: i64 = 0x5DEECE66D;
    const MASK: i64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom((seed ^ Self::MULT) & Self::MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = self.0.wrapping_mul(Self::MULT).wrapping_add(0xB) & Self::MASK;
        (self.0 >> (48 - bits)) as i32
    }

    /// `BitRandomSource.nextInt`: bei einer Zweierpotenz die oberen Bits,
    /// sonst der Rest, mit der Verwerfungsschleife gegen die Schieflage am
    /// oberen Ende.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    /// `BitRandomSource.nextDouble`: 53 Bit aus zwei Zügen.
    fn next_double(&mut self) -> f64 {
        let hoch = self.next(26) as i64;
        let tief = self.next(27) as i64;
        ((hoch << 27) + tief) as f64 * 1.1102230246251565E-16
    }
}

/// `SimplexNoise`, nur was die zwei Dimensionen brauchen.
struct Simplex {
    p: [u8; 256],
}

/// `SimplexNoise.GRADIENT`; in zwei Dimensionen zählen die ersten zwölf.
const GRADIENT: [[i32; 3]; 12] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
];

impl Simplex {
    /// Wie der Konstruktor: drei Züge für den Ursprung, der in zwei
    /// Dimensionen nicht zählt, dann die Permutation.
    fn new(random: &mut JavaRandom) -> Simplex {
        for _ in 0..3 {
            random.next_double();
        }
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = random.next_int(256 - i as i32) as usize;
            p.swap(i, i + j);
        }
        Simplex { p }
    }

    fn p(&self, i: i32) -> i32 {
        self.p[(i & 255) as usize] as i32
    }

    /// `SimplexNoise.getValue(double, double)`, in der Reihenfolge des
    /// Spiels gerechnet: schon eine andere Klammerung rundet anders.
    fn value(&self, x: f64, y: f64) -> f64 {
        let sqrt3 = 3.0f64.sqrt();
        let f2 = 0.5 * (sqrt3 - 1.0);
        let g2 = (3.0 - sqrt3) / 6.0;
        let s = (x + y) * f2;
        let i = (x + s).floor() as i32;
        let j = (y + s).floor() as i32;
        let t = (i + j) as f64 * g2;
        let (x0, y0) = (x - (i as f64 - t), y - (j as f64 - t));
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let (x1, y1) = (x0 - i1 as f64 + g2, y0 - j1 as f64 + g2);
        let (x2, y2) = (x0 - 1.0 + 2.0 * g2, y0 - 1.0 + 2.0 * g2);
        let (ii, jj) = (i & 255, j & 255);
        let g0 = self.p(ii + self.p(jj)) % 12;
        let g1 = self.p(ii + i1 + self.p(jj + j1)) % 12;
        let g2i = self.p(ii + 1 + self.p(jj + 1)) % 12;
        let n0 = corner(g0, x0, y0);
        let n1 = corner(g1, x1, y1);
        let n2 = corner(g2i, x2, y2);
        70.0 * (n0 + n1 + n2)
    }
}

/// `SimplexNoise.getCornerNoise3D` mit z = 0 und 0,5 als Radius.
fn corner(gradient: i32, x: f64, y: f64) -> f64 {
    let z = 0.0;
    let t = 0.5 - x * x - y * y - z * z;
    if t < 0.0 {
        return 0.0;
    }
    let t = t * t;
    let g = GRADIENT[gradient as usize];
    t * t * (g[0] as f64 * x + g[1] as f64 * y + g[2] as f64 * z)
}

/// `Biome.BIOME_INFO_NOISE`: `PerlinSimplexNoise` mit der Oktave 0 aus
/// `WorldgenRandom(LegacyRandomSource(2345))`. Mit nur dieser Oktave
/// bleiben Eingabe und Ergebnis unskaliert.
static BIOME_INFO_NOISE: LazyLock<Simplex> =
    LazyLock::new(|| Simplex::new(&mut JavaRandom::new(2345)));

/// Das Rauschen an einer Blockspalte, wie `GrassColorModifier.SWAMP` es
/// abfragt: `getValue(x * 0.0225, z * 0.0225, false)`.
pub fn biome_info(x: i32, z: i32) -> f64 {
    BIOME_INFO_NOISE.value(x as f64 * 0.0225, z as f64 * 0.0225)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Das Rauschen aus dem Server-JAR 26.2 (`Biomwerte.java` in
    /// `renderer/src/world/`), Bit für Bit: an einem Gitter samt Rand der
    /// Welt, x aussen, z innen, und an den beiden Stellen im Quadrat bis
    /// ±1000, die der Grenze -0,1 von unten und von oben am nächsten liegen.
    #[test]
    fn rauschen_wie_im_spiel() {
        const STELLEN: [i32; 11] = [-1000, -517, -64, -1, 0, 1, 7, 100, 333, 1024, 29999999];
        #[rustfmt::skip]
        const WERTE: [f64; 121] = [
            0.14513300962850123, -0.4024369731256682, -0.5209561193414015, -0.7367457707078493,
            -0.7554335101731171, -0.7734674741492291, -0.7759374908899779, 0.4905790677163165,
            0.032803410511454224, 0.07573906380917918, 0.20506010128516786, -0.12725863453999564,
            -0.3321766619684492, -0.22020873921007775, 0.5393238437041655, 0.5392316804497579,
            0.5288087466069408, 0.24614419900871995, 0.05200996580069655, 0.38908526105384755,
            -0.028309463614547666, -0.5482044616191608, 0.5281588519390956, -0.5842974542295621,
            0.017498703061163638, -0.4216286780204209, -0.35247634780204445, -0.2768372875118962,
            0.18744755974385321, -0.06832385562884392, 0.5497927161724108, -0.725412532002875,
            0.4404058407442343, -0.3051261967336909, 0.2913965533646888, -0.42162288324228664,
            -0.09764257491368801, -0.09803943319931249, -0.09764257491368801, -0.08080364691252477,
            0.2987516358450783, -0.003908689331350905, 0.2453457258764083, -2.055561879321135E-10,
            -0.3529483525778754, 0.3268056726834298, -0.35244746882236855, 0.0,
            0.0, 0.0, -4.6507205171445625E-4, 0.3176702748926926,
            -0.03500098579669105, 0.1722178968953271, -0.0899186061329891, -0.3836994822196385,
            0.36032350923117634, -0.2767533993523333, 0.09764257491368813, 0.09803943319931264,
            0.09764257491368816, 0.07975074220422569, 0.33523862147404293, -0.07217926952042031,
            0.09638759983024771, -0.1735367737497629, -0.25687639587291355, 0.5033750346648347,
            0.18862744802309775, 0.5589337329478249, 0.5616969179547614, 0.5595531781317122,
            0.43976034693755595, 0.3874330132909068, -0.40211958392550906, -0.34672439221145107,
            -0.473473126352803, -0.4890866815974015, 0.4461551069185518, 0.08764646275917808,
            -0.554368660661038, -0.5154132269749233, -0.4650154341587946, 0.01138100712084939,
            0.24551569364037704, -0.5291588134592144, 0.3602531922952507, 0.5682600793419086,
            0.032803410511454224, -0.4413036017985991, -0.8730026132051905, -0.25467186300344996,
            -0.29112269260816726, -0.32928528636565213, -0.5471677618933197, -0.24562110943091792,
            -0.05695352274550207, 0.18589248784372564, 0.004505688789570943, -0.3305364919055596,
            -0.19617633679178623, 0.5230807755218776, 0.21598993316176274, 0.14942663615569232,
            0.08439878406195793, -0.1746169031498412, -0.13864950739393203, -0.25464244744337067,
            0.22545957757860158, -0.5718816887675031, -0.08063974555330228, -0.3185673800140849,
            -0.18102937476846334, -2.055561879321135E-10, -0.0899186061329891, -0.1735350202334897,
            -0.46229670552706753, -0.28426807348411176, 0.2175030441877457, -0.33496210250029596,
            0.008884016616539278,
        ];
        for (i, &x) in STELLEN.iter().enumerate() {
            for (j, &z) in STELLEN.iter().enumerate() {
                let soll = WERTE[i * STELLEN.len() + j];
                assert_eq!(biome_info(x, z).to_bits(), soll.to_bits(), "({x}, {z})");
            }
        }
        assert_eq!(biome_info(416, -988), -0.10000017679357995);
        assert_eq!(biome_info(404, 737), -0.09999930523135435);
    }
}
