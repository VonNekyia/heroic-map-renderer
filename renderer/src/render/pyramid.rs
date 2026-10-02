//! Die Zoomstufen über der gerenderten Kachelebene.
//!
//! Jede gröbere Stufe entsteht aus vier Kacheln der feineren, auf die halbe
//! Kantenlänge gestaucht. Gerendert wird nur die feinste Stufe — alles
//! darüber ist Bildverkleinerung und kostet keinen Weltzugriff.

use std::collections::BTreeSet;

use image::{GenericImage, RgbaImage};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::{TILE, TileId};

impl TileId {
    /// Die Kachel, die diese auf der nächstgröberen Stufe enthält.
    ///
    /// Arithmetische Verschiebung und nicht Division: `-1 >> 1` ist `-1`,
    /// `-1 / 2` wäre `0`. Die Pyramide hängt am Blockursprung, und dort
    /// treffen alle vier Vorzeichen aufeinander.
    pub fn parent(&self) -> TileId {
        TileId {
            x: self.x >> 1,
            y: self.y >> 1,
        }
    }

    /// Welchen Viertelbereich der Elternkachel diese füllt.
    pub fn quadrant(&self) -> (u32, u32) {
        ((self.x & 1) as u32, (self.y & 1) as u32)
    }

    /// Die vier Kacheln, aus denen diese auf der feineren Stufe besteht.
    pub fn children(&self) -> [TileId; 4] {
        let (x, y) = (self.x * 2, self.y * 2);
        [
            TileId { x, y },
            TileId { x: x + 1, y },
            TileId { x, y: y + 1 },
            TileId { x: x + 1, y: y + 1 },
        ]
    }
}

/// Wie viele Stufen über dieser Kachelmenge noch etwas zusammenfassen.
///
/// Gestapelt wird, bis das Halbieren nichts mehr ändert. Das passiert
/// spätestens bei den vier Kacheln um den Ursprung: `(0, 0)`, `(0, -1)`,
/// `(-1, 0)` und `(-1, -1)` sind ihre eigenen Eltern.
pub fn depth(tiles: &BTreeSet<TileId>) -> u32 {
    let mut ebene = tiles.clone();
    let mut stufen = 0;
    loop {
        let eltern: BTreeSet<TileId> = ebene.iter().map(TileId::parent).collect();
        if eltern == ebene {
            return stufen;
        }
        ebene = eltern;
        stufen += 1;
    }
}

/// Alle Elternkacheln einer Kachelmenge.
pub fn parents(tiles: &BTreeSet<TileId>) -> BTreeSet<TileId> {
    tiles.iter().map(TileId::parent).collect()
}

/// Setzt Kacheln zu ihrer Elternkachel zusammen.
///
/// Jedes Kind wird auf die halbe Kantenlänge gestaucht und in seinen
/// Viertelbereich gesetzt. Fehlende Kinder bleiben durchsichtig.
pub fn merge(parent: TileId, children: &[(TileId, RgbaImage)]) -> RgbaImage {
    let mut out = RgbaImage::new(TILE, TILE);
    let half = TILE / 2;
    for (child, image) in children {
        debug_assert_eq!(
            child.parent(),
            parent,
            "{child:?} gehört nicht zu {parent:?}"
        );
        assert_eq!(image.dimensions(), (TILE, TILE), "{child:?}");
        let (qx, qy) = child.quadrant();
        halbiere(image, &mut out, qx * half, qy * half);
    }
    out
}

/// Halbiert die Kantenlänge eines Bildes, gemittelt mit vormultipliziertem
/// Alpha und in linearem Licht.
/// Siehe docs/benutzung/zoomstufen.md, „Verkleinern“.
pub fn shrink(image: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(image.width() / 2, image.height() / 2);
    halbiere(image, &mut out, 0, 0);
    out
}

/// Setzt die Viertel aus [`shrink`] zu ihrer Elternkachel zusammen, Byte
/// für Byte wie [`merge`] aus den ganzen Kindern. Fehlende Kinder bleiben
/// durchsichtig.
pub fn aus_vierteln(parent: TileId, viertel: &[(TileId, RgbaImage)]) -> RgbaImage {
    let mut out = RgbaImage::new(TILE, TILE);
    let half = TILE / 2;
    for (child, bild) in viertel {
        debug_assert_eq!(
            child.parent(),
            parent,
            "{child:?} gehört nicht zu {parent:?}"
        );
        let (qx, qy) = child.quadrant();
        out.copy_from(bild, qx * half, qy * half)
            .unwrap_or_else(|e| panic!("{child:?}: {e}"));
    }
    out
}

/// Schreibt `image` auf die halbe Kantenlänge verkleinert nach `ziel`, die
/// linke obere Ecke auf (`x0`, `y0`), direkt über die Bytes.
fn halbiere(image: &RgbaImage, ziel: &mut RgbaImage, x0: u32, y0: u32) {
    let (breite, hoehe) = (image.width() as usize / 2, image.height() as usize / 2);
    let (zeile, ziel_zeile) = (4 * image.width() as usize, 4 * ziel.width() as usize);
    let quelle = image.as_raw();
    let ziel: &mut [u8] = ziel;
    let linear = &*LINEAR;
    for y in 0..hoehe {
        let (oben, _) = quelle[2 * y * zeile..][..8 * breite].as_chunks::<4>();
        let (unten, _) = quelle[(2 * y + 1) * zeile..][..8 * breite].as_chunks::<4>();
        let anfang = (y0 as usize + y) * ziel_zeile + 4 * x0 as usize;
        let (raus, _) = ziel[anfang..][..4 * breite].as_chunks_mut::<4>();
        for (x, raus) in raus.iter_mut().enumerate() {
            let mut farbe = [0.0f32; 3];
            let mut alpha = 0u32;
            // Links oben, rechts oben, links unten, rechts unten: Die
            // Reihenfolge legt die Rundung der Summen fest, bis aufs Bit.
            for pixel in [oben[2 * x], oben[2 * x + 1], unten[2 * x], unten[2 * x + 1]] {
                let a = pixel[3] as u32;
                alpha += a;
                for (summe, &wert) in farbe.iter_mut().zip(&pixel[..3]) {
                    *summe += linear[wert as usize] * a as f32;
                }
            }
            // Ohne Deckung gibt es keine Farbe zu mitteln, und das Pixel ist
            // ohnehin durchsichtig.
            if alpha == 0 {
                *raus = [0; 4];
                continue;
            }
            let mittel = |summe: f32| to_srgb(summe / alpha as f32);
            *raus = [
                mittel(farbe[0]),
                mittel(farbe[1]),
                mittel(farbe[2]),
                alpha.div_ceil(4) as u8,
            ];
        }
    }
}

/// sRGB-Wert nach linearem Licht, als Tabelle: die Pyramide läuft über
/// jedes Pixel jeder Stufe.
pub(crate) static LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// Lineares Licht zurück nach sRGB: statt `powf` je Aufruf eine Tabelle der
/// 255 Schwellen, ab denen der gerundete sRGB-Wert um eins steigt. Gezählt
/// wird, wie viele davon höchstens beim Wert liegen: [`EIMER`] gibt den
/// Anfang, ein oder zwei Vergleiche den Rest.
/// Siehe docs/benutzung/zoomstufen.md, „Verkleinern“.
pub(crate) fn to_srgb(linear: f32) -> u8 {
    // Unter jeder Schwelle: NaN, 0 und alles darunter.
    if linear.is_nan() || linear <= 0.0 {
        return 0;
    }
    // Die höchste Schwelle liegt unter 1.
    if linear >= 1.0 {
        return 255;
    }
    let mut n = EIMER[(linear.to_bits() >> 16) as usize] as usize;
    while n < SRGB_STEPS.len() && SRGB_STEPS[n] <= linear {
        n += 1;
    }
    n as u8
}

/// Je Eimer der oberen 16 Bits eines f32 aus (0, 1): wie viele Schwellen
/// höchstens beim kleinsten Wert des Eimers liegen.
static EIMER: LazyLock<[u8; (1.0f32.to_bits() >> 16) as usize]> = LazyLock::new(|| {
    std::array::from_fn(|eimer| {
        let kleinster = f32::from_bits((eimer as u32) << 16);
        SRGB_STEPS.partition_point(|&step| step <= kleinster) as u8
    })
});

/// Die sRGB-Kurve mit Rundung, wie sie vor der Tabelle je Kanal lief.
fn srgb_curve(linear: f32) -> u8 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Schwelle `i`: der kleinste f32, den die Kurve auf mindestens `i + 1`
/// rundet. Per Bisektion über die Bitmuster aus der Kurve selbst gesucht
/// statt aus der Umkehrformel gerechnet: die Kurve ist in f32 nicht exakt,
/// und die Tabelle soll bitgleich zu ihr sein.
static SRGB_STEPS: LazyLock<[f32; 255]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let ziel = i as u8 + 1;
        let (mut unter, mut ab) = (0.0f32, 1.0f32);
        while unter.next_up() < ab {
            let mitte = f32::from_bits(unter.to_bits().midpoint(ab.to_bits()));
            if srgb_curve(mitte) >= ziel {
                ab = mitte;
            } else {
                unter = mitte;
            }
        }
        ab
    })
});

/// Was das Frontend über die Karte wissen muss.
///
/// Die Projektion steht in `projection`, in Pixeln der feinsten Stufe. Das
/// Frontend rechnet sie für die Koordinaten nach; dass beide gleich
/// rechnen, prüfen beide an `renderer/tests/fixtures/projektion.json`.
/// Siehe docs/benutzung/map-json.md, „Kamera und Projektion“.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    /// Kantenlänge einer Kachel in Pixeln.
    pub tile_size: u32,
    /// Pixel je Block auf der feinsten Stufe.
    pub scale: u32,
    pub min_zoom: u32,
    /// Feinste Stufe. Dort liegen die gerenderten Kacheln, darüber nur
    /// verkleinerte.
    pub max_zoom: u32,
    /// Pfadmuster der Kacheln, relativ zu dieser Datei.
    pub tiles: String,
    /// Belegter Bereich auf der feinsten Stufe, in Pixeln:
    /// `[links, oben, rechts, unten]`.
    pub bounds: [i32; 4],
    /// Wie viele Stufen über der Basis aus der Welt gerendert sind statt
    /// verkleinert, siehe `--native-levels`. Fehlt das Feld, stammt der
    /// Baum aus einem älteren Stand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_levels: Option<u32>,
    /// Mit welchem Radius die Kacheln Biomfarben mischen, siehe
    /// `--biome-blend`. Fehlt das Feld, stammt der Baum aus einem älteren
    /// Stand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biome_blend: Option<u8>,
    /// Zu welcher Welt der Baum gehört, siehe [`world_id`]; `null` bei einer
    /// Welt ohne Kennung. Fehlt das Feld, stammt der Baum aus einem älteren
    /// Stand.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "vorhanden"
    )]
    pub world: Option<Option<String>>,
    /// Pfadmuster der Höhen je Region, relativ zu dieser Datei, siehe
    /// [`super::heights`]. Fehlt das Feld, hat der Baum keine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights: Option<String>,
    /// Kantenlänge einer Zelle der Höhen in Blöcken; steht mit `heights`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights_cell: Option<u32>,
    /// Unterster Block, den der Renderer zeichnet; steht mit `heights`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_y: Option<i32>,
    /// Oberster Block, den der Renderer zeichnet; steht mit `heights`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_y: Option<i32>,
    /// Die Kamera, gekürzt: `"2:1"`, `"4:3"`, `"top"`, `"top-north"`,
    /// `"north-45"`. Fehlt das Feld, stammt der Baum aus einem älteren Stand
    /// und zeigt 2:1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// Wo die Kamera steht: diagonal `"se"`, `"sw"`, `"nw"` oder `"ne"`,
    /// genordet `"s"`, `"w"`, `"n"` oder `"e"`. Fehlt das Feld, gilt die
    /// Vorgabe der Kamera, `"se"` oder `"s"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Die Projektion in Pixeln der feinsten Stufe; steht mit `camera`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection: Option<ProjectionInfo>,
    /// Wie der Baum zeichnet: `"map"` die Karte, `"cinematic"` mit
    /// `--cinematic`. Fehlt das Feld, stammt der Baum aus einem älteren
    /// Stand und zeigt die Karte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
    /// Der Fingerabdruck der Werte, mit denen Cinematic zeichnet, siehe
    /// [`super::look::Look::fingerabdruck`]; steht nur mit `"cinematic"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_hash: Option<String>,
}

/// Die Projektion in `map.json`: `azimuth` `"diagonal"` oder `"north"`,
/// `u` Pixel je Schritt in u (h), `v` je Schritt in v (a), `y` je Block
/// Höhe (b), diagonal mit u = x − z und v = x + z, genordet mit u = x und
/// v = z, siehe [`super::Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionInfo {
    pub azimuth: String,
    pub u: u32,
    pub v: u32,
    pub y: u32,
}

/// Liest ein Feld, das auch `null` sein darf: nur ein fehlendes bleibt
/// `None`.
fn vorhanden<'de, D: serde::Deserializer<'de>>(
    feld: D,
) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(feld).map(Some)
}

impl MapInfo {
    pub fn new(scale: u32, max_zoom: u32, tiles: &BTreeSet<TileId>) -> MapInfo {
        let tile = TILE as i32;
        let links = tiles.iter().map(|t| t.x).min().unwrap_or(0) * tile;
        let oben = tiles.iter().map(|t| t.y).min().unwrap_or(0) * tile;
        let rechts = tiles.iter().map(|t| t.x + 1).max().unwrap_or(0) * tile;
        let unten = tiles.iter().map(|t| t.y + 1).max().unwrap_or(0) * tile;
        MapInfo {
            tile_size: TILE,
            scale,
            min_zoom: 0,
            max_zoom,
            tiles: "{z}/{x}/{y}.webp".to_string(),
            bounds: [links, oben, rechts, unten],
            native_levels: None,
            biome_blend: None,
            world: None,
            heights: None,
            heights_cell: None,
            min_y: None,
            max_y: None,
            camera: None,
            direction: None,
            projection: None,
            look: None,
            look_hash: None,
        }
    }
}

/// Wie oft [`world_id`] SipHash verkettet.
const ROUNDS: u32 = 1 << 20;

/// Die Kennung einer Welt im Kachelbaum: das Salz des Baums und ein Hash
/// ihres Seeds und ihrer Dimension, als `"<salz>-<hash>"` in Hexziffern:
/// SipHash-2-4 mit dem Salz, 2^20-mal verkettet. Von Hand und nicht
/// `DefaultHasher`, dessen Algorithmus sich mit jeder Rust-Version ändern
/// darf.
/// Siehe docs/entscheidungen/0014-kennung-der-welt-ohne-seed.md.
pub fn world_id(seed: i64, dimension: &str, salt: u64) -> String {
    let key = [salt, u64::from_le_bytes(*b"a-render")];
    // Der Seed hat feste Länge, die Nachricht bleibt so eindeutig.
    let message = [&seed.to_le_bytes()[..], dimension.as_bytes()].concat();
    let mut hash = siphash24(key, &message);
    for _ in 1..ROUNDS {
        hash = siphash24(key, &hash.to_le_bytes());
    }
    format!("{salt:016x}-{hash:016x}")
}

/// Das Salz einer Kennung aus `map.json`. `None` bei einem anderen Format;
/// eine solche Kennung passt zu keiner Welt.
pub fn salt_of(id: &str) -> Option<u64> {
    let (salt, hash) = id.split_once('-')?;
    if salt.len() != 16 || hash.len() != 16 {
        return None;
    }
    u64::from_str_radix(salt, 16).ok()
}

/// SipHash-2-4 nach Aumasson und Bernstein.
fn siphash24(key: [u64; 2], message: &[u8]) -> u64 {
    let mut v = [
        key[0] ^ 0x736f_6d65_7073_6575,
        key[1] ^ 0x646f_7261_6e64_6f6d,
        key[0] ^ 0x6c79_6765_6e65_7261,
        key[1] ^ 0x7465_6462_7974_6573,
    ];
    let round = |v: &mut [u64; 4]| {
        v[0] = v[0].wrapping_add(v[1]);
        v[1] = v[1].rotate_left(13) ^ v[0];
        v[0] = v[0].rotate_left(32);
        v[2] = v[2].wrapping_add(v[3]);
        v[3] = v[3].rotate_left(16) ^ v[2];
        v[0] = v[0].wrapping_add(v[3]);
        v[3] = v[3].rotate_left(21) ^ v[0];
        v[2] = v[2].wrapping_add(v[1]);
        v[1] = v[1].rotate_left(17) ^ v[2];
        v[2] = v[2].rotate_left(32);
    };
    let compress = |v: &mut [u64; 4], m: u64| {
        v[3] ^= m;
        round(v);
        round(v);
        v[0] ^= m;
    };
    let (blocks, rest) = message.as_chunks::<8>();
    for block in blocks {
        compress(&mut v, u64::from_le_bytes(*block));
    }
    let mut last = (message.len() as u64) << 56;
    for (i, &byte) in rest.iter().enumerate() {
        last |= (byte as u64) << (8 * i);
    }
    compress(&mut v, last);
    v[2] ^= 0xff;
    for _ in 0..4 {
        round(&mut v);
    }
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use rayon::prelude::*;

    fn menge(tiles: &[(i32, i32)]) -> BTreeSet<TileId> {
        tiles.iter().map(|&(x, y)| TileId { x, y }).collect()
    }

    /// Über dem Ursprung treffen alle vier Vorzeichen aufeinander. Wer dort
    /// mit `/ 2` statt `>> 1` rechnet, faltet zwei Quadranten zusammen.
    #[test]
    fn eltern_falten_ueber_dem_ursprung_richtig() {
        for (x, y, soll) in [
            (0, 0, (0, 0)),
            (1, 1, (0, 0)),
            (-1, -1, (-1, -1)),
            (-2, -2, (-1, -1)),
            (-3, 5, (-2, 2)),
        ] {
            let eltern = TileId { x, y }.parent();
            assert_eq!((eltern.x, eltern.y), soll, "({x}, {y})");
        }
    }

    /// Kinder und Eltern müssen zueinander passen, sonst landen Bilder im
    /// falschen Viertel.
    #[test]
    fn kinder_und_eltern_passen_zusammen() {
        for x in -4..4 {
            for y in -4..4 {
                let eltern = TileId { x, y };
                let kinder = eltern.children();
                assert_eq!(kinder.len(), 4);
                let quadranten: BTreeSet<(u32, u32)> =
                    kinder.iter().map(TileId::quadrant).collect();
                assert_eq!(quadranten.len(), 4, "{eltern:?}: {kinder:?}");
                for kind in kinder {
                    assert_eq!(kind.parent(), eltern, "{kind:?}");
                }
            }
        }
    }

    #[test]
    fn tiefe_endet_am_ursprung() {
        // Die vier Kacheln um den Ursprung sind ihre eigenen Eltern.
        assert_eq!(depth(&menge(&[(0, 0), (-1, 0), (0, -1), (-1, -1)])), 0);
        assert_eq!(depth(&menge(&[(0, 0)])), 0);
        assert_eq!(depth(&menge(&[(0, 0), (1, 0)])), 1);
        assert_eq!(depth(&menge(&[(0, 0), (3, 0)])), 2);
        assert_eq!(depth(&menge(&[(0, 0), (255, 0)])), 8);
    }

    #[test]
    fn tiefe_reicht_aus_um_alles_zusammenzufassen() {
        let tiles = menge(&[(-40, 7), (13, -9), (0, 0), (39, 40)]);
        let mut ebene = tiles.clone();
        for _ in 0..depth(&tiles) {
            ebene = parents(&ebene);
        }
        assert_eq!(parents(&ebene), ebene, "nach {} Stufen", depth(&tiles));
    }

    fn voll(farbe: [u8; 4], kante: u32) -> RgbaImage {
        RgbaImage::from_pixel(kante, kante, Rgba(farbe))
    }

    #[test]
    fn verkleinern_haelt_eine_flaeche_farbe() {
        let klein = shrink(&voll([10, 200, 30, 255], 8));
        assert_eq!(klein.dimensions(), (4, 4));
        assert!(klein.pixels().all(|p| p.0 == [10, 200, 30, 255]));
    }

    /// Durchsichtige Pixel dürfen ihre Farbe nicht in die Nachbarn ziehen.
    #[test]
    fn verkleinern_mittelt_vormultipliziert() {
        let mut bild = RgbaImage::new(2, 2);
        bild.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        // Drei durchsichtige Pixel mit Schwarz darunter: geradeaus
        // gemittelt käme ein dunkles Rot heraus.
        let klein = shrink(&bild);
        assert_eq!(klein.dimensions(), (1, 1));
        let p = klein.get_pixel(0, 0).0;
        assert_eq!(&p[..3], &[255, 0, 0], "Farbe verwässert: {p:?}");
        assert_eq!(p[3], 64, "Alpha ist der Mittelwert");
    }

    /// Jeder sRGB-Wert muss die Reise nach linear und zurück unverändert
    /// überstehen, sonst verfärbt sich eine einfarbige Fläche je Stufe.
    #[test]
    fn srgb_rundreise_ist_verlustfrei() {
        for c in 0..=255u8 {
            assert_eq!(to_srgb(LINEAR[c as usize]), c);
        }
    }

    /// Die Tabelle rundet wie die Kurve: über eine Million Werte zwischen
    /// 0 und 1, dazu die Nachbarn jeder Schwelle.
    #[test]
    fn schwellentabelle_rundet_wie_die_kurve() {
        for i in 0..=1_000_000u32 {
            let x = i as f32 / 1_000_000.0;
            assert_eq!(to_srgb(x), srgb_curve(x), "bei {x}");
        }
        for &step in SRGB_STEPS.iter() {
            for x in [step.next_down(), step, step.next_up()] {
                assert_eq!(to_srgb(x), srgb_curve(x), "an der Schwelle {step}");
            }
        }
        assert_eq!(to_srgb(-1.0), 0);
        assert_eq!(to_srgb(2.0), 255);
        assert_eq!(to_srgb(f32::NAN), 0);
        for c in 0..=255u8 {
            assert_eq!(to_srgb(LINEAR[c as usize]), c);
        }
    }

    /// Die Binärsuche über die Schwellen, wie `to_srgb` vor den Eimern
    /// zählte.
    fn binaersuche(linear: f32) -> u8 {
        SRGB_STEPS.partition_point(|&step| step <= linear) as u8
    }

    /// Die Eimer zählen wie die Binärsuche, an jedem f32 von 0 bis 1 und an
    /// den Rändern: Verkleinert wird aus Mitteln von Werten in [0, 1].
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "eine Milliarde Werte, im Debug-Build zu langsam; läuft in Release"
    )]
    fn eimer_zaehlen_wie_die_binaersuche() {
        (0..=1.0f32.to_bits()).into_par_iter().for_each(|bits| {
            let x = f32::from_bits(bits);
            assert_eq!(to_srgb(x), binaersuche(x), "bei {x} ({bits:#x})");
        });
        let raender = [-0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        for x in raender.into_iter().chain([1.0f32.next_up(), 2.0, f32::MAX]) {
            assert_eq!(to_srgb(x), binaersuche(x), "bei {x}");
        }
    }

    /// Über die Bytes wie früher über `get_pixel`: dieselben Summen in
    /// derselben Reihenfolge, Bit für Bit, an Kindern aus Zufallspixeln mit
    /// Alpha 0, 255 und dazwischen.
    #[test]
    fn zusammensetzen_wie_ueber_die_pixel() {
        let wie_frueher = |bild: &RgbaImage| {
            RgbaImage::from_fn(bild.width() / 2, bild.height() / 2, |x, y| {
                let mut farbe = [0.0f32; 3];
                let mut alpha = 0u32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let pixel = bild.get_pixel(2 * x + dx, 2 * y + dy).0;
                        let a = pixel[3] as u32;
                        alpha += a;
                        for (summe, &wert) in farbe.iter_mut().zip(&pixel[..3]) {
                            *summe += LINEAR[wert as usize] * a as f32;
                        }
                    }
                }
                if alpha == 0 {
                    return Rgba([0; 4]);
                }
                let m = |summe: f32| binaersuche(summe / alpha as f32);
                Rgba([
                    m(farbe[0]),
                    m(farbe[1]),
                    m(farbe[2]),
                    alpha.div_ceil(4) as u8,
                ])
            })
        };
        // xorshift: reproduzierbar ohne weitere Abhängigkeit.
        let mut zustand = 0x2545_f491_4f6c_dd1d_u64;
        let mut zufall = move || {
            zustand ^= zustand << 13;
            zustand ^= zustand >> 7;
            zustand ^= zustand << 17;
            zustand as u8
        };
        let eltern = TileId { x: 3, y: -2 };
        let kinder: Vec<(TileId, RgbaImage)> = eltern
            .children()
            .into_iter()
            .map(|kind| {
                let bild = RgbaImage::from_fn(TILE, TILE, |_, _| {
                    let alpha = match zufall() % 4 {
                        0 => 0,
                        1 => 255,
                        _ => zufall(),
                    };
                    Rgba([zufall(), zufall(), zufall(), alpha])
                });
                (kind, bild)
            })
            .collect();

        let bild = merge(eltern, &kinder);
        let half = TILE / 2;
        for (kind, kinderbild) in &kinder {
            let (qx, qy) = kind.quadrant();
            let klein = wie_frueher(kinderbild);
            assert_eq!(shrink(kinderbild), klein, "{kind:?}");
            for (x, y, pixel) in klein.enumerate_pixels() {
                assert_eq!(
                    bild.get_pixel(qx * half + x, qy * half + y),
                    pixel,
                    "{kind:?} bei ({x}, {y})"
                );
            }
        }
        let ungerade =
            RgbaImage::from_fn(7, 5, |_, _| Rgba([zufall(), zufall(), zufall(), zufall()]));
        assert_eq!(shrink(&ungerade), wie_frueher(&ungerade), "ungerade Kanten");

        // Aus den Vierteln wie aus den ganzen Kindern, auch mit Lücken.
        let viertel: Vec<(TileId, RgbaImage)> = kinder
            .iter()
            .map(|(kind, bild)| (*kind, shrink(bild)))
            .collect();
        assert_eq!(aus_vierteln(eltern, &viertel), bild, "alle vier");
        let (kinder, viertel) = ([&kinder[1], &kinder[2]], [&viertel[1], &viertel[2]]);
        assert_eq!(
            aus_vierteln(eltern, &viertel.map(Clone::clone)),
            merge(eltern, &kinder.map(Clone::clone)),
            "zwei von vier"
        );
    }

    /// Halb Schwarz, halb Weiss: in linearem Licht gemittelt ist das
    /// deutlich heller als der sRGB-Mittelwert 128.
    #[test]
    fn verkleinern_mittelt_in_linearem_licht() {
        let mut bild = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        bild.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        bild.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let p = shrink(&bild).get_pixel(0, 0).0;
        assert_eq!(p, [188, 188, 188, 255]);
    }

    /// Vier Pixel, bei denen jede andere Reihenfolge der Summen in f32 ein
    /// anderes Byte ergibt: links oben, rechts oben, links unten, rechts
    /// unten, wie früher `get_pixel` Zeile für Zeile. Gefunden unter
    /// Zufallspixeln; eine Reihenfolge mit demselben ersten Paar rechnet
    /// gleich.
    #[test]
    fn verkleinern_summiert_in_fester_reihenfolge() {
        let mut bild = RgbaImage::new(2, 2);
        bild.put_pixel(0, 0, Rgba([118, 93, 107, 255]));
        bild.put_pixel(1, 0, Rgba([76, 22, 14, 255]));
        bild.put_pixel(0, 1, Rgba([96, 25, 202, 255]));
        bild.put_pixel(1, 1, Rgba([82, 213, 225, 255]));
        assert_eq!(shrink(&bild).get_pixel(0, 0).0, [95, 123, 165, 255]);
    }

    #[test]
    fn verkleinern_haelt_durchsichtig_durchsichtig() {
        let klein = shrink(&RgbaImage::new(4, 4));
        assert!(klein.pixels().all(|p| p.0[3] == 0));
    }

    #[test]
    fn zusammensetzen_legt_jedes_kind_in_sein_viertel() {
        let eltern = TileId { x: -1, y: 2 };
        let farben = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 0, 255],
        ];
        let kinder: Vec<(TileId, RgbaImage)> = eltern
            .children()
            .into_iter()
            .zip(farben)
            .map(|(kind, farbe)| (kind, voll(farbe, TILE)))
            .collect();

        let bild = merge(eltern, &kinder);
        let half = TILE / 2;
        for ((kind, _), farbe) in kinder.iter().zip(farben) {
            let (qx, qy) = kind.quadrant();
            let probe = bild.get_pixel(qx * half + 3, qy * half + 3).0;
            assert_eq!(probe, farbe, "{kind:?} in Viertel ({qx}, {qy})");
        }
    }

    #[test]
    fn zusammensetzen_laesst_fehlende_kinder_durchsichtig() {
        let eltern = TileId { x: 0, y: 0 };
        let kind = eltern.children()[3];
        let bild = merge(eltern, &[(kind, voll([1, 2, 3, 255], TILE))]);

        assert_eq!(bild.get_pixel(TILE / 2 + 1, TILE / 2 + 1).0[3], 255);
        assert_eq!(bild.get_pixel(1, 1).0[3], 0, "leeres Viertel");
    }

    /// Die Testvektoren aus dem SipHash-Paper: Schlüssel 00..0f, Nachricht
    /// leer und 00..0e. Und die Kennung selbst darf sich nie ändern, sonst
    /// gälte jeder bestehende Baum als fremd.
    #[test]
    fn kennung_ist_siphash_des_seeds() {
        let key = [0x0706_0504_0302_0100, 0x0f0e_0d0c_0b0a_0908];
        assert_eq!(siphash24(key, &[]), 0x726f_db47_dd0e_0e31);
        let message: Vec<u8> = (0..15).collect();
        assert_eq!(siphash24(key, &message), 0xa129_ca61_49be_45e5);
        // Gegengerechnet mit einer eigenen Python-Fassung. Die Werte dürfen
        // sich nie ändern, sonst gälte jeder bestehende Baum als fremd.
        let oberwelt = "minecraft:overworld";
        assert_eq!(
            world_id(0, oberwelt, 0),
            "0000000000000000-21035ca95f557704"
        );
        let salt = 0x0123_4567_89ab_cdef;
        assert_eq!(
            world_id(4_815_162_342, oberwelt, salt),
            "0123456789abcdef-0adf0e2365ce474f"
        );
        assert_eq!(
            world_id(4_815_162_342, "minecraft:the_nether", salt),
            "0123456789abcdef-85810263e60694e1"
        );
        assert_eq!(
            world_id(-1, "minecraft:the_end", u64::MAX),
            "ffffffffffffffff-66708158e556a415"
        );
        assert_eq!(salt_of(&world_id(7, oberwelt, 42)), Some(42));
        assert_eq!(salt_of("56007c963ac3acc6"), None, "ohne Salz");
    }

    #[test]
    fn mapinfo_umschliesst_alle_kacheln() {
        let info = MapInfo::new(16, 3, &menge(&[(-2, 1), (4, -3)]));
        let tile = TILE as i32;
        assert_eq!(info.bounds, [-2 * tile, -3 * tile, 5 * tile, 2 * tile]);
        assert_eq!((info.min_zoom, info.max_zoom), (0, 3));
        assert_eq!(info.tile_size, TILE);
    }

    #[test]
    fn mapinfo_wird_als_camelcase_geschrieben() {
        let json = serde_json::to_string(&MapInfo::new(8, 2, &menge(&[(0, 0)]))).unwrap();
        assert!(json.contains("\"tileSize\":256"), "{json}");
        assert!(json.contains("\"maxZoom\":2"), "{json}");
        assert!(json.contains("{z}/{x}/{y}.webp"), "{json}");
    }
}
