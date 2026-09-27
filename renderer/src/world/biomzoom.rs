//! Welches Biom ein Block trägt: `BiomeManager.getBiome` aus 26.2.
//!
//! Gespeichert sind Biome in Zellen von 4×4×4 Blöcken, je Viertelposition
//! eines. Das Spiel gibt jedem Block das Biom einer der acht
//! Viertelpositionen um ihn, gewürfelt aus dem Seed der Welt, damit die
//! Grenzen nicht auf dem Raster verlaufen.
//! Siehe docs/renderer/biomfarben.md, „Biom je Block“.

/// `BiomeManager.obfuscateSeed`: Guavas `Hashing.sha256().hashLong(seed)
/// .asLong()`, also SHA-256 über die acht Bytes des Seeds, Little Endian,
/// und die ersten acht Bytes davon, ebenso gelesen. Damit würfelt der
/// Zoom; der Client bekommt vom Server nur diesen Wert.
pub fn obfuscate_seed(seed: i64) -> i64 {
    let hash = sha256(&seed.to_le_bytes());
    i64::from_le_bytes(hash[..8].try_into().expect("acht Bytes"))
}

/// Die Viertelposition, deren Biom der Block `[x, y, z]` trägt:
/// `BiomeManager.getBiome` mit dem Wert aus [`obfuscate_seed`]. Von den
/// acht Ecken der Zelle um den um zwei Blöcke verschobenen Block gewinnt die
/// mit dem kleinsten verwackelten Abstand, bei Gleichstand die frühere.
pub fn zoom(zoom_seed: i64, [x, y, z]: [i32; 3]) -> [i32; 3] {
    let (i, j, k) = (x.wrapping_sub(2), y.wrapping_sub(2), z.wrapping_sub(2));
    let (l, m, n) = (i >> 2, j >> 2, k >> 2);
    let (d, e, f) = (
        (i & 3) as f64 / 4.0,
        (j & 3) as f64 / 4.0,
        (k & 3) as f64 / 4.0,
    );
    let mut beste = 0;
    let mut abstand = f64::INFINITY;
    for p in 0..8 {
        let [ex, ey, ez] = [p & 4 != 0, p & 2 != 0, p & 1 != 0];
        let v = fiddled_distance(
            zoom_seed,
            [l + ex as i32, m + ey as i32, n + ez as i32],
            [
                if ex { d - 1.0 } else { d },
                if ey { e - 1.0 } else { e },
                if ez { f - 1.0 } else { f },
            ],
        );
        if abstand > v {
            beste = p;
            abstand = v;
        }
    }
    [l + (beste >> 2 & 1), m + (beste >> 1 & 1), n + (beste & 1)]
}

/// `BiomeManager.getFiddledDistance`: der Abstand zur Ecke `q`, jede Achse
/// um bis zu 0,45 verwackelt. Die Summanden in der Reihenfolge des Spiels,
/// Gleitkomma rundet sonst anders.
fn fiddled_distance(seed: i64, [i, j, k]: [i32; 3], [d, e, f]: [f64; 3]) -> f64 {
    let mut m = seed;
    for c in [i, j, k, i, j, k] {
        m = lcg(m, c as i64);
    }
    let g = fiddle(m);
    m = lcg(m, seed);
    let h = fiddle(m);
    m = lcg(m, seed);
    let n = fiddle(m);
    (f + n) * (f + n) + (e + h) * (e + h) + (d + g) * (d + g)
}

/// `LinearCongruentialGenerator.next`.
fn lcg(l: i64, m: i64) -> i64 {
    l.wrapping_mul(
        l.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    )
    .wrapping_add(m)
}

/// `BiomeManager.getFiddle`: aus Bit 24 bis 33 ein Versatz von -0,45 bis
/// knapp 0,45.
fn fiddle(l: i64) -> f64 {
    ((l >> 24).rem_euclid(1024) as f64 / 1024.0 - 0.5) * 0.9
}

/// SHA-256 nach FIPS 180-4, für [`obfuscate_seed`]: acht Bytes je Lauf,
/// eine neue Abhängigkeit lohnt dafür nicht.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (t, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[t] = u32::from_be_bytes(*word);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ w[t - 15] >> 3;
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ w[t - 2] >> 10;
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for t in 0..64 {
            let [a, b, c, d, e, f, g, hh] = v;
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (h, v) in h.iter_mut().zip(v) {
            *h = h.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (bytes, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *bytes = word.to_be_bytes();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Prüfwerte aus FIPS 180-2, Anhang B.1 und B.2.
    #[test]
    fn sha256_wie_im_standard() {
        let hex = |bytes: [u8; 32]| bytes.map(|b| format!("{b:02x}")).concat();
        assert_eq!(
            hex(sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// `BiomeManager.obfuscateSeed` aus dem Server-JAR 26.2, ausgegeben von
    /// `Biomwerte.java` neben dieser Datei. Alle Seeds hier und in
    /// `zoom_wie_im_spiel` sind Testwerte, auch der lange; keiner gehört zu
    /// einer Welt.
    #[test]
    fn seed_wie_im_spiel() {
        for (seed, soll) in [
            (0, 8794265229978523055),
            (1, -6467378160175308932),
            (-1, 6759447113877070610),
            (12345, 293737985876514017),
            (i64::MIN, 6374347445474471398),
            (i64::MAX, 7179146226492139882),
            (-4172144997902289642, 2159143436479834350),
        ] {
            assert_eq!(obfuscate_seed(seed), soll, "Seed {seed}");
        }
    }

    /// Welche Ecke `getBiome` im Server-JAR 26.2 wählt, aus `Biomwerte.java`
    /// neben dieser Datei: je Seed für jeden Block mit x und z von -6 bis 5
    /// und y von -3 bis 2 eine Ziffer wie `p` im Spiel, 4 für x + 1, 2 für
    /// y + 1, 1 für z + 1, gezählt von der Zelle um den um zwei verschobenen
    /// Block; x läuft innen, dann z, eine Zeile je y. Dazu Blöcke am Rand der
    /// Welt mit der ganzen Viertelposition.
    #[test]
    fn zoom_wie_im_spiel() {
        #[rustfmt::skip]
        const WUERFEL: [(i64, [&str; 6]); 2] = [
            (
                12345,
                [
                "226622662222226622362222226733332227337733373337222622062226222600062226322700062226337733773337226622662266006622662266033522762666333311773666",
                "004400040000004400040000004400140007111511151115000400040004000000440004100511440004115511551115004400440044004400040044111511140444111111551444",
                "000400040000000400040007000400040777111401151777000000440066000022440004100611440004115511151115004400040044004400040044215511140444111511151445",
                "000000040022000400040277000400007777111400057777000622240666000622240666106622240066116611151115004400040004204400040044255511110445315511113355",
                "000622262226200622262227200622272777200622273777006622262666006622262666166622273266166633373377206622262266226622262366225511163366335511173335",
                "004400040004004400040000004400051155004400751155004400640044044400770004044400773004144411551155044400440044004400440144114400451144115511551114",
                ],
            ),
            (
                -4172144997902289642,
                [
                "206622262266007622262266117722773777117711773777066600662666066400440366336404440333333733443333222622262222222732262222337733362227337733373337",
                "004400040044155400040044155510551555155515551555044404440444044404440444114511111111111511111111000400000000000500000000110511040055115511151155",
                "024400240044155500220044155512551555155515551555044404440444044421440444116611110114111511111111000400000000000000000000000510000055111511751155",
                "222422220066155522222666155512222566155515551555244422440444226622440666216621113666111611113333000400002220002200002220022200002555222737773555",
                "222622222266225622222266155522222266355733252777226622246666226622246666226622113366336621113333222200062222222200062222222220062227222737773555",
                "000400040004000400040004115400040004115511041155004400040444004400040444004400051114114400051111000403440000000403340000000403330077104413351775",
                ],
            ),
        ];
        for (seed, zeilen) in WUERFEL {
            let zoom_seed = obfuscate_seed(seed);
            for (y, zeile) in (-3..=2).zip(zeilen) {
                let mut ziffern = zeile.bytes().map(|b| (b - b'0') as i32);
                for z in -6..=5 {
                    for x in -6..=5 {
                        let p = ziffern.next().expect("144 Ziffern");
                        let soll = [
                            ((x - 2) >> 2) + (p >> 2 & 1),
                            ((y - 2) >> 2) + (p >> 1 & 1),
                            ((z - 2) >> 2) + (p & 1),
                        ];
                        assert_eq!(
                            zoom(zoom_seed, [x, y, z]),
                            soll,
                            "Seed {seed}, ({x}, {y}, {z})"
                        );
                    }
                }
            }
        }
        const WEIT: [(i64, [i32; 3], [i32; 3]); 10] = [
            (12345, [29999999, 319, -29999999], [7500000, 79, -7500000]),
            (12345, [-30000000, -64, 30000000], [-7500001, -17, 7500000]),
            (12345, [-1, -64, -1], [-1, -17, -1]),
            (12345, [1000000, 70, -2000000], [250000, 17, -500001]),
            (12345, [-123457, 63, 98765], [-30865, 15, 24691]),
            (
                -4172144997902289642,
                [29999999, 319, -29999999],
                [7499999, 79, -7500001],
            ),
            (
                -4172144997902289642,
                [-30000000, -64, 30000000],
                [-7500000, -17, 7499999],
            ),
            (-4172144997902289642, [-1, -64, -1], [-1, -17, -1]),
            (
                -4172144997902289642,
                [1000000, 70, -2000000],
                [249999, 17, -500001],
            ),
            (
                -4172144997902289642,
                [-123457, 63, 98765],
                [-30865, 15, 24691],
            ),
        ];
        for (seed, block, soll) in WEIT {
            assert_eq!(
                zoom(obfuscate_seed(seed), block),
                soll,
                "Seed {seed}, {block:?}"
            );
        }
    }
}
