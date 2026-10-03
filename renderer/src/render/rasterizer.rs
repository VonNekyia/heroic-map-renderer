use std::collections::BTreeSet;
use std::sync::LazyLock;

use image::{Rgba, RgbaImage};

use crate::assets::baker::{BakedModel, Quad};
use crate::assets::blockentity::Entity;
use crate::assets::{CardinalLight, DimensionType, Face, Textures, Tint, Tints, fluid};

use super::pyramid::{LINEAR, to_srgb};
use super::{Cell, Projection, Richtung};

/// Abtastpunkte je Pixelkante für die Textur. Die Geometrie wird nur im
/// Pixelmittelpunkt geprüft, die Textur über den Pixel gemittelt, so
/// dicht, dass jeder Texel zählt: Diagonal ist eine Seitenfläche
/// `scale / 2` Pixel breit für sechzehn Texel, also liegen `32 / scale`
/// Texel unter jedem Pixel. Genordet ist sie `scale` breit; dort tastet es
/// doppelt so dicht ab wie nötig. Mindestens zwei, damit auch bei scale 32
/// die Mitte zwischen zwei Texeln stimmt; höchstens sechzehn, mehr Texel
/// hat eine Textur nicht.
/// Siehe docs/renderer/naehte.md, „Geometrie im Pixelmittelpunkt“.
fn texture_samples(scale: u32) -> u32 {
    (32 / scale.max(1)).clamp(2, 16)
}

/// Obergrenze für die Kantenlänge eines Sprites, in Blockbreiten. Modelle
/// dürfen von -16 bis 32 reichen, also drei Blöcke; alles darüber ist
/// kaputt.
const MAX_SPRITE_BLOCKS: u32 = 8;

/// Um so viel liegt eine Flüssigkeitsfläche in der Tiefe hinter der
/// Blockfläche an derselben Stelle. Weit über dem Rundungsrauschen der
/// interpolierten Tiefe, weit unter jedem echten Abstand im Modell.
const FLUID_BEHIND: f32 = 1e-3;

/// Bis zu diesem Anteil ihrer Normalen steht eine Fläche parallel zur
/// Blickrichtung und hat im Bild keine Fläche. Der Baker dreht in f32 und
/// lässt einer solchen Fläche eine Normale knapp neben null; echte
/// Drehungen liegen weit darüber.
/// Siehe docs/renderer/naehte.md, „Flächen parallel zur Blickrichtung“.
const EDGE_ON: f32 = 1e-4;

/// Die höchste Stufe des Himmels- und des Blocklichts, am Tag unter freiem
/// Himmel, siehe [`brightness_rgb`].
pub const FULL_LIGHT: u8 = 15;

/// `BlockFactor` aus `LightmapRenderStateExtractor.extract`: 1,4 und ein
/// Flackern, das `tick` zufällig um 0 laufen lässt. Hier ohne Flackern.
pub(crate) const BLOCK_FACTOR: f32 = 1.4;

/// `BrightnessFactor`: `options.gamma` in der Voreinstellung.
const BRIGHTNESS_FACTOR: f32 = 0.5;

/// Helligkeit je Farbkanal im Himmelslicht `sky` und im Blocklicht
/// `block` in einer Dimension vom Typ `typ`, wie `lightmap.fsh` in 26.2 sie
/// rechnet: die Umgebungsfarbe, dazu das Himmelslicht in seiner Farbe mal
/// `SkyFactor` und das Blocklicht mit [`BLOCK_FACTOR`] in einer Farbe
/// zwischen `BlockLightTint` und Weiss, auf 0 bis 1 begrenzt und mit
/// [`BRIGHTNESS_FACTOR`] zu `notGamma` hin gemischt. In der Oberwelt gibt
/// Himmelslicht 15 in jedem Kanal 1.
/// Siehe docs/renderer/wasser-und-licht.md, „Helligkeit wie im Spiel“.
pub fn brightness_rgb(typ: &DimensionType, sky: u8, block: u8) -> [f32; 3] {
    let level = |l: u8| l.min(FULL_LIGHT) as f32 / 15.0;
    let umgebung = roh(typ.ambient_light_color);
    let himmel = roh(typ.sky_light_color);
    let b = level(block);
    let sky_brightness = get_brightness(level(sky)) * typ.sky_light_factor;
    let block_brightness = get_brightness(b) * BLOCK_FACTOR;
    let block_color = blocklicht_farbe(typ.block_light_tint, b);
    let color: [f32; 3] = std::array::from_fn(|c| {
        (umgebung[c] + himmel[c] * sky_brightness + block_color[c] * block_brightness)
            .clamp(0.0, 1.0)
    });
    let max = color.iter().fold(0.0f32, |a, &c| a.max(c));
    if max == 0.0 {
        return color;
    }
    let rest = 1.0 - max;
    let scaled = 1.0 - rest * rest * rest * rest;
    color.map(|c| c + (c * (scaled / max) - c) * BRIGHTNESS_FACTOR)
}

/// `get_brightness` in `lightmap.fsh`: die Helligkeit einer Stufe von 0
/// bis 1.
pub(crate) fn get_brightness(level: f32) -> f32 {
    level / (4.0 - 3.0 * level)
}

/// Eine Farbe aus dem Dimensionstyp, wie das Spiel sie liest: je Kanal
/// c/255 (`ARGB.vector3fFromRGB24`).
pub(crate) fn roh(farbe: Tint) -> [f32; 3] {
    farbe.map(|c| f32::from(c) / 255.0)
}

/// Die Farbe des Blocklichts der Stufe `b` von 0 bis 1 wie in
/// `lightmap.fsh`: `BlockLightTint` roh zu Weiss gemischt mit
/// `0,9 · (2b − 1)²`.
pub(crate) fn blocklicht_farbe(tint: Tint, b: f32) -> [f32; 3] {
    let mix = 0.9 * (2.0 * b - 1.0) * (2.0 * b - 1.0);
    roh(tint).map(|t| t + (1.0 - t) * mix)
}

/// Die Lightmap einer Dimension: [`brightness_rgb`] je Himmels- und
/// Blocklicht in 255steln, für [`darken`].
pub struct Lightmap([[[u32; 3]; 16]; 16]);

impl Lightmap {
    pub fn new(typ: &DimensionType) -> Lightmap {
        Lightmap(std::array::from_fn(|sky| {
            std::array::from_fn(|block| {
                brightness_rgb(typ, sky as u8, block as u8).map(|c| (c * 255.0).round() as u32)
            })
        }))
    }

    /// Die Lightmap der Oberwelt am Tag.
    pub fn oberwelt() -> &'static Lightmap {
        static OBERWELT: LazyLock<Lightmap> =
            LazyLock::new(|| Lightmap::new(&DimensionType::oberwelt()));
        &OBERWELT
    }

    /// Die Helligkeit im Licht `licht` je Farbkanal, Rot zuerst.
    pub fn factors(&self, licht: Light) -> [u32; 3] {
        self.0[licht.sky.min(FULL_LIGHT) as usize][licht.block.min(FULL_LIGHT) as usize]
    }

    /// Die Helligkeit an einer Ecke je Kanal, aus ihrem Licht nach
    /// [`smooth_blend`]: Das Spiel liest die Lightmap je Ecke
    /// (`terrain.vsh`, `sample_lightmap`), linear gefiltert
    /// (`ChunkSectionsToRender`, `FilterMode.LINEAR`), also zwischen den
    /// benachbarten Stufen gemischt, in beiden Lichtern.
    /// Siehe docs/renderer/weiche-beleuchtung.md, „Licht an den Ecken“.
    pub fn linear(&self, licht: u32) -> [u32; 3] {
        let (sky, block) = (licht >> 16 & 255, licht & 255);
        let (s, b) = ((sky >> 4).min(15) as u8, (block >> 4).min(15) as u8);
        let (fs, fb) = (sky & 15, block & 15);
        let stufe = |sky: u8, block: u8| self.factors(Light { sky, block });
        let (t00, t01, t10, t11) = (
            stufe(s, b),
            stufe(s, b + 1),
            stufe(s + 1, b),
            stufe(s + 1, b + 1),
        );
        std::array::from_fn(|c| {
            ((16 - fs) * (16 - fb) * t00[c]
                + (16 - fs) * fb * t01[c]
                + fs * (16 - fb) * t10[c]
                + fs * fb * t11[c]
                + 128)
                / 256
        })
    }
}

/// In welchem Licht das Spiel einen Block zeichnet: Himmels- und
/// Blocklicht, je 0 bis 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Light {
    pub sky: u8,
    pub block: u8,
}

impl Light {
    pub fn sky(sky: u8) -> Light {
        Light { sky, block: 0 }
    }

    /// [`Lightmap::factors`] in der Oberwelt.
    pub fn factors(self) -> [u32; 3] {
        Lightmap::oberwelt().factors(self)
    }

    /// Himmels- und Blocklicht, gepackt wie `LightCoordsUtil.pack` in 26.2:
    /// das Blocklicht ab Bit 4, das Himmelslicht ab Bit 20.
    pub fn packed(self) -> u32 {
        u32::from(self.block.min(FULL_LIGHT)) << 4 | u32::from(self.sky.min(FULL_LIGHT)) << 20
    }

    /// Aus der Packung von [`Light::packed`].
    pub fn from_packed(licht: u32) -> Light {
        Light {
            sky: (licht >> 20 & 15) as u8,
            block: (licht >> 4 & 15) as u8,
        }
    }
}

/// So packt das Spiel ein Block, der voll hell gezeichnet wird
/// (`LightCoordsUtil.FULL_BRIGHT`): Himmels- und Blocklicht 15.
pub const VOLL_HELL: u32 = 0xf0_00f0;

/// `LightCoordsUtil.smoothBlend` in 26.2: das Licht an einer Ecke einer
/// weich beleuchteten Seite aus drei Nachbarn und der Zelle vor der Seite,
/// gepackt wie [`Light::packed`]. Ist die Zelle davor hell genug, Himmels-
/// oder Blocklicht über 2, nimmt ein Nachbar ohne Licht ihres, einer ohne
/// Himmelslicht ihr Himmelslicht. Das Mittel der vier liegt danach in
/// Sechzehnteln einer Stufe vor: das Blocklicht in den Bits 0 bis 7, das
/// Himmelslicht in 16 bis 23.
/// Siehe docs/renderer/weiche-beleuchtung.md, „Licht an den Ecken“.
pub fn smooth_blend(mut a0: u32, mut a1: u32, mut a2: u32, mitte: u32) -> u32 {
    let (sky, block) = (|l: u32| l >> 20 & 15, |l: u32| l >> 4 & 15);
    if sky(mitte) > 2 || block(mitte) > 2 {
        for a in [&mut a0, &mut a1, &mut a2] {
            if *a == 0 {
                *a = mitte;
            } else if sky(*a) == 0 {
                *a |= mitte & 0xff_0000;
            }
        }
    }
    (a0 + a1 + a2 + mitte) >> 2 & 0xff_00ff
}

/// Ein Pixel im Licht mit den Faktoren aus [`Lightmap::factors`]: jeder
/// Farbkanal mal seine Helligkeit, das Alpha bleibt. Ganzzahlig wie
/// [`over`], dieselbe Rechnung steht im Shader (`gpu.wgsl`).
pub fn darken(pixel: [u8; 4], factors: [u32; 3]) -> [u8; 4] {
    let dunkel = |c: u8, f: u32| ((c as u32 * f + 127) / 255) as u8;
    [
        dunkel(pixel[0], factors[0]),
        dunkel(pixel[1], factors[1]),
        dunkel(pixel[2], factors[2]),
        pixel[3],
    ]
}

/// Die Seiten, die eine diagonale schräge Kamera sieht, in der Reihenfolge
/// der Plätze in [`Sprite::ao`]: Eine Fläche auf dem Rand der Seite `i` hat
/// dort den Platz `i`, eine im Innern mit dieser Richtung den Platz `i + 3`,
/// siehe [`ao_face`]. `north-45` sieht Osten nicht, von oben nur die
/// Oberseite.
pub const AO_FACES: [Face; 3] = [Face::Up, Face::South, Face::East];

/// Wie viele Plätze die AO-Karte hat: je Seite aus [`AO_FACES`] einen für
/// Flächen auf dem Rand und einen für Flächen im Innern.
pub const AO_PLAETZE: usize = 6;

/// Die Ecken der Seiten oben und rundum in der Reihenfolge von `FaceInfo`
/// in 26.2, im Würfel 0..1, per javap am 26.2-Client. Das Spiel zeichnet
/// das Viereck als die Dreiecke 0-1-2 und 2-3-0.
/// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
const FACE_INFO: [(Face, [[f32; 3]; 4]); 5] = [
    (
        Face::Up,
        [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
    ),
    (
        Face::North,
        [[1., 1., 0.], [1., 0., 0.], [0., 0., 0.], [0., 1., 0.]],
    ),
    (
        Face::South,
        [[0., 1., 1.], [0., 0., 1.], [1., 0., 1.], [1., 1., 1.]],
    ),
    (
        Face::West,
        [[0., 1., 0.], [0., 0., 0.], [0., 0., 1.], [0., 1., 1.]],
    ),
    (
        Face::East,
        [[1., 1., 1.], [1., 0., 1.], [1., 0., 0.], [1., 1., 0.]],
    ),
];

/// Die Ecken der Seite `seite` aus [`AO_FACES`] im Blick, in ihren beiden
/// Koordinaten ([`face_coords`]): die Ecken der Seite der Welt, die dort
/// liegt, in deren Reihenfolge aus [`FACE_INFO`]. In dieser Reihenfolge
/// legt [`ChunkCache::ecken_at`](super::metatile) die Werte ab.
pub(crate) fn ecken_im_blick(richtung: Richtung, seite: usize) -> [[f32; 2]; 4] {
    let welt = richtung.seite_in_die_welt(AO_FACES[seite]);
    let (_, ecken) = FACE_INFO
        .iter()
        .find(|(face, _)| *face == welt)
        .expect("oben oder rundum");
    ecken.map(|ecke| face_coords(seite, richtung.punkt_in_den_blick(ecke)))
}

/// Die Richtung, die das Spiel einem Viereck gibt
/// (`FaceBakery.findClosestDirection`): die erste in der Reihenfolge von
/// `Direction.values()`, also von [`Face`], mit dem grössten Skalarprodukt
/// mit der Normalen; `None`, wenn keines positiv ist.
fn richtung_des_vierecks(n: [f32; 3]) -> Option<Face> {
    let mut beste = None;
    let mut wert = 0.0;
    for face in [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ] {
        let v = face.versatz();
        let dot = n[0] * v[0] as f32 + n[1] * v[1] as f32 + n[2] * v[2] as f32;
        if dot >= 0.0 && dot > wert {
            (beste, wert) = (Some(face), dot);
        }
    }
    beste
}

/// Der Platz in der AO-Karte, als den das Spiel ein Viereck `quad` im Blick
/// weich beleuchtet, mit `welt` derselben Fläche in der Welt. Die Seite
/// ist die Richtung des Vierecks ([`richtung_des_vierecks`], in der Welt
/// gewählt, dann im Blick), wenn sie eine aus [`AO_FACES`] ist. Platz 0 bis
/// 2 hat eine Fläche, die das Spiel im Licht der Zelle davor zeichnet
/// (`faceCubic` in `BlockModelLighter.prepareQuadShape`): eben, bis auf
/// 1e-4, und auf dem Rand des Würfels, mit voller Kollisionsform des Blocks
/// (`kollision`) auch im Innern. Jede andere, auch eine schräge, hat Platz
/// 3 bis 5: Das Spiel zählt sie ab der eigenen Zelle. Flüssigkeiten
/// bekommen keinen, Flächen aus Blockentity-Modellen auch nicht: Das Spiel
/// zeichnet sie im Licht der Entities, siehe [`entity_light`].
/// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
fn ao_face(quad: &Quad, welt: &Quad, kollision: bool, richtung: Richtung) -> Option<usize> {
    if quad.fluid.is_some() || quad.entity.is_some() {
        return None;
    }
    let seite = richtung.seite_in_den_blick(richtung_des_vierecks(welt.normal())?);
    let platz = AO_FACES.iter().position(|&f| f == seite)?;
    let min = |axis: usize| {
        quad.corners
            .iter()
            .map(|c| c[axis])
            .fold(f32::MAX, f32::min)
    };
    let max = |axis: usize| {
        quad.corners
            .iter()
            .map(|c| c[axis])
            .fold(f32::MIN, f32::max)
    };
    // Die Achse der Normalen der Seite.
    let axis = [1, 2, 0][platz];
    let davor = max(axis) - min(axis) < 1e-4 && (kollision || min(axis) > 0.9999);
    Some(if davor { platz } else { platz + 3 })
}

/// Die Koordinaten eines Punkts auf einer Seite aus [`AO_FACES`]: oben
/// `(x, z)`, Süden `(x, y)`, Osten `(z, y)`.
fn face_coords(face: usize, [x, y, z]: [f32; 3]) -> [f32; 2] {
    match face {
        0 => [x, z],
        1 => [x, y],
        _ => [z, y],
    }
}

/// Die Anteile der vier Ecken `e` einer Seite aus [`ecken_im_blick`] an
/// einem Punkt der Seite, in 255steln und zusammen genau 255:
/// baryzentrisch in dem der beiden Dreiecke des Spiels, in dem der Punkt
/// liegt. So verläuft die Helligkeit der Ecken im Spiel über die Fläche.
fn corner_weights(e: [[f32; 2]; 4], p: [f32; 2]) -> [u32; 4] {
    let bary = |[a, b, c]: [usize; 3]| {
        let (pa, pb, pc) = (e[a], e[b], e[c]);
        let flaeche = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
        let wb = ((p[0] - pa[0]) * (pc[1] - pa[1]) - (p[1] - pa[1]) * (pc[0] - pa[0])) / flaeche;
        let wc = ((pb[0] - pa[0]) * (p[1] - pa[1]) - (pb[1] - pa[1]) * (p[0] - pa[0])) / flaeche;
        [1.0 - wb - wc, wb, wc]
    };
    let mut w = [0.0f32; 4];
    let erstes = bary([0, 1, 2]);
    if erstes.iter().all(|&x| x >= -1e-4) {
        [w[0], w[1], w[2]] = erstes;
    } else {
        [w[2], w[3], w[0]] = bary([2, 3, 0]);
    }
    let mut q = w.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as i32);
    let rest = 255 - q.iter().sum::<i32>();
    let groesste = (0..4).max_by_key(|&i| q[i]).unwrap_or(0);
    q[groesste] += rest;
    q.map(|x| x as u32)
}

/// Ein Eintrag der AO-Karte: die Anteile der Ecken 0 bis 2, der vierten
/// fehlt auf 255, und der Platz aus [`ao_face`] plus 1.
fn ao_word(platz: usize, w: [u32; 4]) -> u32 {
    w[0] | w[1] << 8 | w[2] << 16 | (platz as u32 + 1) << 24
}

/// Das Licht an den Ecken der [`AO_PLAETZE`], die weiche Beleuchtung
/// eingerechnet: je Farbkanal und Platz ein Wort, ein Byte je Ecke in der
/// Reihenfolge von [`ecken_im_blick`] seiner Seite, in 255steln. So liefert
/// es [`ChunkCache::ecken_at`](super::metatile) je Block.
pub type Ecken = [[u32; AO_PLAETZE]; 3];

/// Die Helligkeit eines Kanals an einem Pixel in 255steln: der Eintrag der
/// AO-Karte gegen die Werte der Ecken seines Platzes in diesem Kanal aus
/// [`Ecken`], wie die Grafikkarte die Farbe der Ecken über das Dreieck
/// verlaufen lässt. 255 ohne Platz. Dieselbe Rechnung steht im Shader.
pub fn ecken_faktor(word: u32, corners: [u32; AO_PLAETZE]) -> u32 {
    let face = word >> 24;
    if face == 0 {
        return 255;
    }
    let c = corners[face as usize - 1];
    let [w0, w1, w2] = [word & 255, word >> 8 & 255, word >> 16 & 255];
    let w3 = 255 - w0 - w1 - w2;
    (w0 * (c & 255) + w1 * (c >> 8 & 255) + w2 * (c >> 16 & 255) + w3 * (c >> 24) + 127) / 255
}

/// Das fertig gerasterte Bild einer Blockstate.
pub struct Sprite {
    pub image: RgbaImage,
    /// Pixelposition der linken oberen Ecke, relativ zum projizierten
    /// Blockursprung.
    pub offset: (i32, i32),
    /// Die AO-Karte: je Pixel sein Platz und die Anteile der Ecken seiner
    /// Seite, siehe [`ecken_faktor`] und [`ao_face`]; nur, wenn ein Pixel
    /// einen hat. Mit ihr bekommt jeder Platz das Licht an seinen Ecken,
    /// weich beleuchtet oder nicht, siehe [`Sprite::weich`]. Ein Pixel ohne
    /// Platz liegt im Licht der eigenen Zelle.
    pub ao: Option<Vec<u32>>,
    /// Das Modell erlaubt weiche Beleuchtung (`ambientocclusion`).
    pub weich: bool,
    /// Je Pixel zwei Wörter, die Tönungskarte: der Anteil, der die Farbe des
    /// Blocks aus dem Biom trägt, und der, der die des Wassers trägt, je
    /// Kanal ein Byte, Rot im untersten. `image` hält den Rest; zusammen
    /// setzt [`tinted`] sie beim Zeichnen. Nur für Sprites mit Flächen,
    /// deren Farbe vom Biom kommt.
    /// Siehe docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
    pub tint: Option<Vec<u32>>,
    /// Nur in Sprites für Cinematic: je Pixel Tiefe und Normale seines
    /// vordersten Fragments, siehe [`Geometrie`].
    pub geometrie: Option<Vec<Geometrie>>,
}

/// Was ein Sprite für Cinematic an einem Pixel über die vorderste Fläche
/// weiss, die es dort zeigt.
/// Siehe docs/renderer/cinematic.md, „Sprites für Cinematic“.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Geometrie {
    /// Die Tiefe entlang der Blickachse relativ zum Ursprung des Blocks im
    /// Blick, wie [`Projection::depth`]: grösser heisst näher.
    pub tiefe: f32,
    /// Die Normale der Fläche im Blick, normiert, auf der Seite, die die
    /// Kamera sieht.
    pub normale: [f32; 3],
    /// Die Normale im Blick, nach der die Fläche statt ihrer eigenen Licht
    /// bekommt ([`Quad::shade`], etwa nach oben bei `shade: false`).
    ///
    /// [`Quad::shade`]: crate::assets::Quad::shade
    pub shade: Option<[f32; 3]>,
    /// Ist das vorderste Fragment Wasser, sein Alpha, sonst 0.
    pub wasser: f32,
}

/// Je Rang einer Fläche für Cinematic: ihre Normale im Blick, die Normale
/// aus `shade` und ob sie Wasser ist, siehe [`Geometrie`].
type Normalen = ([f32; 3], Option<[f32; 3]>, bool);

/// Ein Pixel in den Farben seines Blocks: je Kanal der Rest aus dem Bild
/// und die beiden Anteile der Tönungskarte `[block, water]` mal der Farbe
/// des Bioms, `farben` wie die Karte gepackt. Ganzzahlig wie [`over`],
/// dieselbe Rechnung steht im Shader. Rest und Anteile ergeben zusammen
/// höchstens 255, darüber läuft kein Kanal.
pub fn tinted(pixel: [u8; 4], [block, water]: [u32; 2], [b, w]: [u32; 2]) -> [u8; 4] {
    let kanal = |c: usize| {
        let byte = |word: u32| word >> (8 * c) & 255;
        (pixel[c] as u32 + (byte(block) * byte(b) + byte(water) * byte(w) + 127) / 255) as u8
    };
    [kanal(0), kanal(1), kanal(2), pixel[3]]
}

/// Wie [`darken`] über [`tinted`], nur liegt der Anteil des Wassers im
/// Licht `wasser`, der Rest im Licht `licht`, beide aus [`Lightmap::factors`]:
/// Ein gefluteter Block an der Oberfläche zeichnet sein Modell im Licht
/// seiner Zelle, sein Wasser im helleren darüber. Einmal gerundet, dieselbe
/// Rechnung steht im Shader.
pub fn tinted_im_licht(
    pixel: [u8; 4],
    [block, water]: [u32; 2],
    [b, w]: [u32; 2],
    licht: [u32; 3],
    wasser: [u32; 3],
) -> [u8; 4] {
    let kanal = |c: usize| {
        let byte = |word: u32| word >> (8 * c) & 255;
        let rest = (pixel[c] as u32 * 255 + byte(block) * byte(b)) * licht[c];
        let nass = byte(water) * byte(w) * wasser[c];
        ((rest + nass + 32512) / 65025).min(255) as u8
    };
    [kanal(0), kanal(1), kanal(2), pixel[3]]
}

/// Eine Farbe gepackt wie die Tönungskarte, Rot im untersten Byte.
pub fn pack(tint: [u8; 3]) -> u32 {
    tint[0] as u32 | (tint[1] as u32) << 8 | (tint[2] as u32) << 16
}

/// [`render_mit_licht`] im Licht der Oberwelt, ohne volle Kollisionsform.
pub fn render(
    model: &BakedModel,
    textures: &Textures,
    projection: &Projection,
    tints: Tints,
) -> Option<Sprite> {
    render_mit_licht(
        model,
        textures,
        projection,
        tints,
        CardinalLight::Default,
        false,
    )
}

/// Rastert ein gebackenes Modell in ein Sprite.
///
/// Da die Kamera fest steht, sieht jede Blockstate immer gleich aus. Das
/// Sprite entsteht deshalb einmal und wird im Renderpfad nur noch kopiert.
/// Das Licht des Blocks kommt erst beim Zeichnen dazu. `licht` schattiert
/// die Seiten wie der Typ der Dimension, siehe [`shade_factor`];
/// `kollision`: Der Block hat volle Kollisionsform, siehe [`ao_face`].
pub fn render_mit_licht(
    model: &BakedModel,
    textures: &Textures,
    projection: &Projection,
    tints: Tints,
    licht: CardinalLight,
    kollision: bool,
) -> Option<Sprite> {
    rastern(model, textures, projection, tints, licht, kollision, false).map(|raster| raster.ganz())
}

/// Ein gerastertes Modell: seine Fragmente je Pixel, gemischt erst auf
/// Abruf, als ganzes Sprite oder je Würfel, siehe [`Raster::teile`].
pub struct Raster {
    canvas: Canvas,
    offset: (i32, i32),
    ao: bool,
    weich: bool,
    /// Für Cinematic je Rang einer Fläche ihre Normale, `shade` und ob sie
    /// Wasser ist, siehe [`Geometrie`].
    normalen: Option<Vec<Normalen>>,
}

impl Raster {
    /// Das ganze Modell als ein Sprite.
    pub fn ganz(&self) -> Sprite {
        let (image, ao, geometrie) = self
            .canvas
            .mischen(self.ao, self.normalen.as_deref(), |_| true);
        Sprite {
            image,
            offset: self.offset,
            ao: ao.filter(|karte| karte.iter().any(|&w| w >> 24 != 0)),
            weich: self.weich,
            tint: None,
            geometrie,
        }
    }

    /// Die Würfel, in denen Fragmente liegen.
    pub fn zellen(&self) -> BTreeSet<Cell> {
        self.canvas.zellen()
    }

    /// Je Würfel, in dem Fragmente liegen, deren Mischung als eigenes
    /// Sprite, auf seine Pixel zugeschnitten, mit Versatz zum Block des
    /// Modells. Die Pixel aller Teile sind zusammen genau die des ganzen
    /// Modells, mit derselben Füllregel und demselben Mittel der Textur.
    ///
    /// Mit `mit_ao` bekommt jeder Teil eine AO-Karte, auch einer ohne Seite:
    /// Seine Pixel liegen dann im Licht der eigenen Zelle, siehe
    /// [`Sprite::ao`]. Der Aufrufer setzt es, wenn das ganze Modell eine
    /// hat.
    /// Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
    pub fn teile(&self, mit_ao: bool) -> Vec<(Cell, Sprite)> {
        self.canvas
            .zellen()
            .into_iter()
            .filter_map(|zelle| {
                let (image, ao, geometrie) =
                    self.canvas
                        .mischen(mit_ao, self.normalen.as_deref(), |f| f.zelle == zelle);
                let sprite = Sprite {
                    image,
                    offset: self.offset,
                    ao,
                    weich: self.weich,
                    tint: None,
                    geometrie,
                };
                Some((zelle, zuschneiden(&sprite)?))
            })
            .collect()
    }
}

/// Das Sprite auf seine Pixel mit Alpha über 0 zugeschnitten, samt
/// AO-Karte und Geometrie. `None` ohne solche Pixel.
fn zuschneiden(sprite: &Sprite) -> Option<Sprite> {
    let mut umriss: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in sprite.image.enumerate_pixels() {
        if pixel.0[3] == 0 {
            continue;
        }
        umriss = Some(match umriss {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    let (x0, y0, x1, y1) = umriss?;
    let (breite, w) = (sprite.image.width(), x1 - x0 + 1);
    let image = image::imageops::crop_imm(&sprite.image, x0, y0, w, y1 - y0 + 1).to_image();
    fn zuschnitt<T: Copy>(
        karte: &[T],
        breite: u32,
        (x0, y0, x1, y1): (u32, u32, u32, u32),
    ) -> Vec<T> {
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| karte[(y * breite + x) as usize]))
            .collect()
    }
    let umriss = (x0, y0, x1, y1);
    Some(Sprite {
        image,
        offset: (sprite.offset.0 + x0 as i32, sprite.offset.1 + y0 as i32),
        ao: sprite
            .ao
            .as_deref()
            .map(|karte| zuschnitt(karte, breite, umriss)),
        weich: sprite.weich,
        tint: None,
        geometrie: sprite
            .geometrie
            .as_deref()
            .map(|karte| zuschnitt(karte, breite, umriss)),
    })
}

/// Rastert ein gebackenes Modell in seine Fragmente, siehe [`Raster`] und
/// [`render_mit_licht`]. Mit `kino` für Cinematic: ohne Schattierung nach
/// Richtung, dafür mit [`Sprite::geometrie`].
/// Siehe docs/renderer/cinematic.md, „Sprites für Cinematic“.
pub fn rastern(
    model: &BakedModel,
    textures: &Textures,
    projection: &Projection,
    tints: Tints,
    licht: CardinalLight,
    kollision: bool,
    kino: bool,
) -> Option<Raster> {
    // Gerastert wird im Blick, schattiert nach der Richtung in der Welt.
    let richtung = projection.richtung();
    let im_blick: Vec<Quad> = model
        .quads
        .iter()
        .map(|quad| quad_im_blick(quad, richtung))
        .collect();
    let ecken: [[[f32; 2]; 4]; 3] = std::array::from_fn(|s| ecken_im_blick(richtung, s));
    let mut projected: Vec<ProjectedQuad> = im_blick
        .iter()
        .zip(&model.quads)
        .filter_map(|(quad, welt)| {
            let rueckseite = seite(quad, projection)?;
            let shade = if kino {
                1.0
            } else {
                shade_factor(welt, rueckseite, licht)
            };
            Some(ProjectedQuad::new(
                quad,
                projection,
                shade,
                ao_face(quad, welt, kollision, richtung),
                &ecken,
                normale(quad, rueckseite),
            ))
        })
        .collect();

    // Jede Fläche legt je Pixel ein Fragment ab, gemischt wird erst am
    // Schluss: je Pixel von hinten nach vorne, nach der Tiefe an genau
    // diesem Pixel, egal in welcher Reihenfolge die Flächen kommen. Bei
    // gleicher Tiefe gewinnt die spätere; sortiert wird deshalb stabil nach
    // der vordersten Ecke, und deckungsgleiche Flächen behalten ihre
    // Modellreihenfolge.
    // Siehe docs/renderer/naehte.md, „Fragmente je Pixel“.
    projected.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let normalen = kino.then(|| {
        projected
            .iter()
            .map(|q| {
                let wasser = matches!(q.quad.fluid, Some((fluid::Fluid::Water, _)));
                let shade = q
                    .quad
                    .shade
                    .map(|seite| richtung.normale_in_den_blick(seite.versatz().map(|c| c as f32)));
                (q.normale, shade, wasser)
            })
            .collect()
    });

    let (min_x, min_y, max_x, max_y) = bounds(&projected)?;
    let width = (max_x - min_x).max(1) as u32;
    let height = (max_y - min_y).max(1) as u32;

    // Ein Modell aus einem Pack kann beliebige Koordinaten enthalten. Ein
    // Sprite, das um Größenordnungen zu groß ausfällt, würde einen
    // Renderlauf über hunderte Regionen an der Speicheranforderung
    // abbrechen — dann lieber diesen einen Block auslassen.
    let limit = projection.scale() * MAX_SPRITE_BLOCKS;
    if width > limit || height > limit {
        return None;
    }

    let ao = projected.iter().any(|q| q.ao_face.is_some());
    let mut canvas = Canvas::new(width, height);
    let samples = texture_samples(projection.scale());
    // Von vorn nach hinten gerastert: was hinter einer deckenden Fläche
    // liegt, wird dann gar nicht erst abgetastet.
    for (order, quad) in projected.iter().enumerate().rev() {
        quad.draw(
            &mut canvas,
            textures,
            (min_x, min_y),
            tints,
            samples,
            order as u32,
        );
    }

    canvas.sortieren();
    Some(Raster {
        canvas,
        offset: (min_x, min_y),
        ao,
        weich: model.ambient_occlusion,
        normalen,
    })
}

/// Die Normale von `quad`, normiert, auf der Seite, die die Kamera sieht:
/// mit `rueckseite` umgekehrt.
fn normale(quad: &Quad, rueckseite: bool) -> [f32; 3] {
    let n = quad.normal();
    let laenge = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let k = if rueckseite { -1.0 } else { 1.0 } / laenge;
    n.map(|a| a * k)
}

fn bounds(quads: &[ProjectedQuad]) -> Option<(i32, i32, i32, i32)> {
    let points = quads.iter().flat_map(|q| q.screen.iter());
    let mut min = (f32::MAX, f32::MAX);
    let mut max = (f32::MIN, f32::MIN);
    let mut any = false;
    for &(x, y, _) in points {
        any = true;
        min = (min.0.min(x), min.1.min(y));
        max = (max.0.max(x), max.1.max(y));
    }
    any.then(|| {
        (
            min.0.floor() as i32,
            min.1.floor() as i32,
            max.0.ceil() as i32,
            max.1.ceil() as i32,
        )
    })
}

/// Ein Viereck mit Bildschirmkoordinaten und Tiefe je Ecke.
struct ProjectedQuad<'a> {
    quad: &'a Quad,
    /// x, y, Tiefe
    screen: [(f32, f32, f32); 4],
    /// Tiefe der vordersten Ecke, nur zum Sortieren.
    depth: f32,
    shade: f32,
    /// Als welcher Platz das Viereck weich beleuchtet wird, siehe
    /// [`ao_face`], mit den Ecken seiner Seite aus [`ecken_im_blick`].
    ao_face: Option<(usize, [[f32; 2]; 4])>,
    /// Die Normale im Blick, siehe [`normale`].
    normale: [f32; 3],
}

impl<'a> ProjectedQuad<'a> {
    /// `quad` im Blick, `shade` aus [`shade_factor`], `platz` aus
    /// [`ao_face`], `ecken` je Seite aus [`ecken_im_blick`], `normale` aus
    /// [`normale`].
    fn new(
        quad: &'a Quad,
        projection: &Projection,
        shade: f32,
        platz: Option<usize>,
        ecken: &[[[f32; 2]; 4]; 3],
        normale: [f32; 3],
    ) -> ProjectedQuad<'a> {
        let screen = quad.corners.map(|corner| {
            let (x, y) = projection.project(corner);
            (x, y, projection.depth(corner))
        });
        ProjectedQuad {
            quad,
            screen,
            depth: screen.iter().map(|&(_, _, d)| d).fold(f32::MIN, f32::max),
            shade,
            ao_face: platz.map(|platz| (platz, ecken[platz % 3])),
            normale,
        }
    }

    fn draw(
        &self,
        canvas: &mut Canvas,
        textures: &Textures,
        (min_x, min_y): (i32, i32),
        tints: Tints,
        samples: u32,
        order: u32,
    ) {
        let texture = textures.image(self.quad.texture);
        let (tw, th) = texture.dimensions();
        if tw == 0 || th == 0 {
            return;
        }

        // Vanilla rückt jede Flüssigkeitsfläche ein Tausendstel ins
        // Blockinnere; hier rückt sie stattdessen in der Tiefe nach
        // hinten, hinter die Seiten eines gefluteten Blocks.
        // Siehe docs/renderer/wasser-und-licht.md, „Flüssigkeiten als Würfel“.
        let behind = if self.quad.fluid.is_some() {
            FLUID_BEHIND
        } else {
            0.0
        };
        let ao_face = self.ao_face;
        let vertices: [Vertex; 4] = std::array::from_fn(|i| {
            let (x, y, depth) = self.screen[i];
            let [s, t] =
                ao_face.map_or([0.0; 2], |(f, _)| face_coords(f % 3, self.quad.corners[i]));
            Vertex {
                x: x - min_x as f32,
                y: y - min_y as f32,
                depth: depth - behind,
                u: self.quad.uvs[i][0],
                v: self.quad.uvs[i][1],
                s,
                t,
                pos: self.quad.corners[i],
            }
        });
        // Die Texturmittelung tastet knapp neben dem Pixelmittelpunkt ab,
        // am Rand also knapp ausserhalb der Fläche. Dort bleibt sie im
        // Ausschnitt, den die Fläche aus der Textur nimmt — eine Tür soll
        // nicht ihre Rückseite an die Kante mischen.
        let bounds = ausschnitt(self.quad);
        let deckung = deckung(self.quad, textures, bounds);

        // Die Farbe einer Fläche aus einem Blockentity-Modell multipliziert
        // die Textur wie die Farbe des Bioms, bei Bannern die des Farbstoffs.
        let farbe = self.quad.entity.map(|e| e.farbe).filter(|&f| f != [255; 3]);
        let tint = match self.quad.tint_index {
            None => farbe,
            Some(fluid::TINT_INDEX) => tints.water,
            Some(_) => tints.block,
        }
        .map(|tint| tint.map(|c| c as f32 / 255.0));
        // Abtastpunkte ausserhalb des Ausschnitts liegen ausserhalb der
        // Fläche und zählen nicht — sonst zöge bei kleinen Flächen der
        // Randtexel das Mittel zu sich. Knapp unter der Obergrenze bleiben,
        // sonst landet die Abtastung im Texel dahinter; ein Ausschnitt
        // ohne Breite bleibt ein Punkt.
        let hi = [
            (bounds[1][0] - 1e-4).max(bounds[0][0]),
            (bounds[1][1] - 1e-4).max(bounds[0][1]),
        ];
        let inside = |u: f32, v: f32| {
            (bounds[0][0]..=bounds[1][0]).contains(&u) && (bounds[0][1]..=bounds[1][1]).contains(&v)
        };
        let sample = |u: f32, v: f32| {
            sample(
                texture,
                tw,
                th,
                u.clamp(bounds[0][0], hi[0]),
                v.clamp(bounds[0][1], hi[1]),
            )
        };
        for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
            canvas.triangle(
                [vertices[a], vertices[b], vertices[c]],
                &sample,
                &inside,
                Shading {
                    shade: self.shade,
                    tint,
                    order,
                    ao_face,
                    fluessig: self.quad.fluid.is_some(),
                    deckung,
                },
                samples,
            );
        }
    }
}

/// Der Ausschnitt, den eine Fläche aus ihrer Textur nimmt: die kleinsten
/// und die grössten Texturkoordinaten ihrer Ecken.
pub(crate) fn ausschnitt(quad: &Quad) -> [[f32; 2]; 2] {
    let mut bounds = [[f32::MAX, f32::MAX], [f32::MIN, f32::MIN]];
    for [u, v] in quad.uvs {
        bounds[0] = [bounds[0][0].min(u), bounds[0][1].min(v)];
        bounds[1] = [bounds[1][0].max(u), bounds[1][1].max(v)];
    }
    bounds
}

/// Wie eine Fläche deckt, nach der Schicht wie im Spiel. Eine Fläche aus
/// einem Blockentity-Modell bringt ihre mit. Sonst
/// (`FaceBakery.computeMaterialTransparency` und
/// `ChunkSectionLayer.byTransparency` in 26.2): mit `force_translucent`
/// durchscheinend, sonst nach dem Ausschnitt `bounds` der Textur.
/// Flüssigkeiten gehen dort nicht durch den FaceBakery. Eine deckende Fläche
/// deckt ausgeschnitten wie gemischt ganz.
fn deckung(quad: &Quad, textures: &Textures, bounds: [[f32; 2]; 2]) -> Deckung {
    match quad.entity {
        Some(Entity { schicht, .. }) if schicht.gemischt => Deckung::Gemischt {
            schwelle: schwelle(schicht.alpha),
        },
        Some(Entity { schicht, .. }) => Deckung::Ausgeschnitten {
            fuellung: None,
            schwelle: schwelle(schicht.alpha),
        },
        None if quad.force_translucent
            || quad.fluid.is_some()
            || textures.durchscheinend(quad.texture, bounds[0], bounds[1]) =>
        {
            Deckung::Gemischt {
                schwelle: schwelle(Some(ALPHA_CUTOUT_TRANSLUCENT)),
            }
        }
        None => Deckung::Ausgeschnitten {
            fuellung: textures
                .fuellung(quad.texture)
                .map(|farbe| farbe.map(|c| LINEAR[c as usize])),
            schwelle: schwelle(Some(ALPHA_CUTOUT_CUTOUT)),
        },
    }
}

/// Ab welchem Alpha ein Texel der Fläche den Strahl zur Sonne aufhält: in
/// einer gemischten Schicht nur ganz deckend, ausgeschnitten ab der Schwelle
/// ihres Alpha-Tests, denn was der Test stehen lässt, deckt dort ganz.
/// Siehe docs/renderer/cinematic.md, „Schatten“.
pub(crate) fn sonnenschwelle(quad: &Quad, textures: &Textures) -> u8 {
    match deckung(quad, textures, ausschnitt(quad)) {
        Deckung::Gemischt { .. } => 255,
        Deckung::Ausgeschnitten { schwelle, .. } => schwelle,
    }
}

/// True, wenn die Fläche der Kamera zugewandt ist.
///
/// Die Kamera blickt gegen ihre Achse ([`Projection::achse`], bei 2:1
/// (1, 1, 1)); eine Fläche ist also sichtbar, wenn ihre Normale eine
/// Komponente in Richtung der Achse hat. Ohne diese
/// Prüfung gewinnen abgewandte Flächen den Tiefentest, wenn sie mit einer
/// sichtbaren zusammenfallen — beim Seerosenblatt liegen `down` und `up` in
/// derselben Ebene. Eine Fläche parallel zur Blickrichtung zählt nicht,
/// siehe `EDGE_ON`. `quad` liegt in der Welt.
pub(crate) fn faces_camera(quad: &Quad, projection: &Projection) -> bool {
    let richtung = projection.richtung();
    zur_kamera(richtung.normale_in_den_blick(quad.normal()), projection)
}

/// Das Viereck der Welt im Blick aus `richtung`, um die Mitte seines
/// Blocks gedreht. Seine Seiten, `fluid` und `cullface`, bleiben die der
/// Welt.
/// Siehe docs/renderer/richtungen.md, „Im Blick“.
fn quad_im_blick(quad: &Quad, richtung: Richtung) -> Quad {
    Quad {
        corners: quad.corners.map(|ecke| richtung.punkt_in_den_blick(ecke)),
        ..quad.clone()
    }
}

/// Zeigt die Normale zur Kamera, siehe [`faces_camera`]? Von oben steht
/// jede senkrechte Fläche auf der Kante.
fn zur_kamera(n: [f32; 3], projection: &Projection) -> bool {
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let [ax, ay, az] = projection.achse();
    n[0] * ax + n[1] * ay + n[2] * az > EDGE_ON * length
}

/// Welche Seite einer Fläche im Blick die Kamera sieht: `Some(false)` die
/// Vorderseite, `Some(true)` die Rückseite, die nur eine Schicht ohne
/// Culling zeichnet (`RenderPipeline.isCull`), `None` keine.
fn seite(quad: &Quad, projection: &Projection) -> Option<bool> {
    let n = quad.normal();
    if zur_kamera(n, projection) {
        return Some(false);
    }
    let beidseitig = quad.entity.is_some_and(|e| e.schicht.beidseitig);
    (beidseitig && zur_kamera(n.map(|a| -a), projection)).then_some(true)
}

/// Wie hell eine Fläche aus einem Blockentity-Modell ist:
/// `minecraft_mix_light` in `shaders/include/light.glsl`, 0,6 je Richtung
/// und 0,4 Umgebung, mit den Richtungen der Dimension.
/// Siehe docs/renderer/blockentities.md, „Licht“.
fn entity_light(n: [f32; 3], licht: CardinalLight) -> f32 {
    let laenge = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let summe: f32 = licht
        .entity_light()
        .iter()
        .map(|l| ((l[0] * n[0] + l[1] * n[1] + l[2] * n[2]) / (laenge(*l) * laenge(n))).max(0.0))
        .sum();
    (summe * 0.6 + 0.4).min(1.0)
}

/// Helligkeit nach der Richtung, in die die Fläche am stärksten zeigt, in
/// der Schattierung der Dimension (`licht`); mit `shade` wie die Seite der
/// Welt, die es nennt (`BlockModelLighter.getDirectionalBrightness`), bei
/// `shade: false` also wie die Oberseite. Eine Fläche aus einem Blockentity-Modell liegt im Licht der
/// Entities, von hinten mit `PER_FACE_LIGHTING` im Licht der umgekehrten
/// Normalen. Die Seiten einer Flüssigkeit nehmen die Oberseite mal Norden
/// oder Westen.
/// Siehe docs/renderer/dimensionstypen.md, „Schattierung nach Richtung“.
fn shade_factor(quad: &Quad, rueckseite: bool, licht: CardinalLight) -> f32 {
    if let Some(entity) = quad.entity {
        let n = quad.normal();
        let umgekehrt = rueckseite && entity.schicht.je_seite;
        return entity_light(if umgekehrt { n.map(|a| -a) } else { n }, licht);
    }
    if let Some((_, seite)) = quad.fluid {
        return match seite {
            Face::Up | Face::Down => licht.face(seite),
            Face::North | Face::South => licht.face(Face::Up) * licht.face(Face::North),
            Face::West | Face::East => licht.face(Face::Up) * licht.face(Face::West),
        };
    }
    if let Some(seite) = quad.shade {
        return licht.face(seite);
    }
    let n = quad.normal();
    let [ax, ay, az] = [n[0].abs(), n[1].abs(), n[2].abs()];
    licht.face(if ay >= ax && ay >= az {
        if n[1] >= 0.0 { Face::Up } else { Face::Down }
    } else if az >= ax {
        Face::North
    } else {
        Face::East
    })
}

/// Texel an normierten Koordinaten. Außerhalb von 0..1 wird wiederholt —
/// einzelne Modelle geben UV jenseits der Texturgrenzen an.
fn sample(texture: &RgbaImage, tw: u32, th: u32, u: f32, v: f32) -> [u8; 4] {
    let x = ((u * tw as f32).floor() as i64).rem_euclid(tw as i64) as u32;
    let y = ((v * th as f32).floor() as i64).rem_euclid(th as i64) as u32;
    texture.get_pixel(x, y).0
}

#[derive(Clone, Copy)]
struct Vertex {
    x: f32,
    y: f32,
    depth: f32,
    u: f32,
    v: f32,
    /// Lage auf der Seite für die weiche Beleuchtung, siehe [`face_coords`].
    s: f32,
    t: f32,
    /// Lage im Raum, in Blockbreiten vom Ursprung des Blocks, für den
    /// Würfel eines Fragments, siehe [`zellbereich`].
    pos: [f32; 3],
}

/// Wie ein Texel zur Farbe wird: Helligkeit der Fläche, Färbung, ob sie
/// die Oberseite einer Flüssigkeit ist, als welche Seite sie weich
/// beleuchtet wird, wie sie mit Löchern deckt — und der Rang der Fläche,
/// der bei gleicher Tiefe entscheidet.
#[derive(Clone, Copy)]
struct Shading {
    shade: f32,
    tint: Option<[f32; 3]>,
    order: u32,
    ao_face: Option<(usize, [[f32; 2]; 4])>,
    /// Die Fläche gehört einer Flüssigkeit.
    fluessig: bool,
    deckung: Deckung,
}

/// Wie eine Fläche deckt, nach der Schicht, in die das Spiel sie legt.
/// Siehe docs/renderer/naehte.md, „Ausgeschnitten statt gemischt“.
#[derive(Clone, Copy)]
enum Deckung {
    /// TRANSLUCENT und die gemischten Schichten der Blockentities: Das
    /// Mittel der Abtastpunkte deckt so weit, wie sie decken. Ein Texel
    /// unter der `schwelle` verwirft der Alpha-Test vorher.
    Gemischt { schwelle: u8 },
    /// CUTOUT, und SOLID, das so oder so ganz deckt: Der Alpha-Test
    /// verwirft jedes Texel unter der `schwelle`, und was bleibt, deckt
    /// ganz. Bei `dark_cutout` zählen die Löcher mit dieser Farbe mit, in
    /// linearem Licht. Die Schichten der Blockentities ohne Alpha-Test
    /// (`entity_solid`) haben die Schwelle 0: Jedes Texel deckt.
    Ausgeschnitten {
        fuellung: Option<[f32; 3]>,
        schwelle: u8,
    },
}

/// `ALPHA_CUTOUT` in `pipeline/cutout_terrain` in 26.2 (`RenderPipelines`).
const ALPHA_CUTOUT_CUTOUT: f32 = 0.5;
/// `ALPHA_CUTOUT` in `pipeline/translucent_terrain`.
const ALPHA_CUTOUT_TRANSLUCENT: f32 = 0.1;

/// Das kleinste Alpha eines Texels in 255steln, das der Test
/// `color.a < ALPHA_CUTOUT` in `terrain.fsh` und `entity.fsh` stehen lässt,
/// 0 ohne Test: 128 bei 0,5 und 26 bei 0,1.
fn schwelle(alpha: Option<f32>) -> u8 {
    alpha.map_or(0, |a| (a * 255.0).ceil() as u8)
}

/// Was eine Fläche zu einem Pixel beiträgt.
#[derive(Clone, Copy)]
struct Fragment {
    pixel: u32,
    /// Größer heißt näher an der Kamera.
    depth: f32,
    /// Bei gleicher Tiefe liegt der höhere Rang oben.
    order: u32,
    /// Farbe mit Helligkeit und Färbung, Alpha der Textur.
    color: [u8; 4],
    /// Eintrag der AO-Karte, 0 ohne weiche Beleuchtung.
    ao: u32,
    /// Von einer Flüssigkeit, siehe [`Canvas::mischen`].
    fluessig: bool,
    /// Der Würfel, in dem es liegt, relativ zum Block des Modells.
    zelle: Cell,
}

/// Die Fragmente eines Sprites, gemischt erst in [`Canvas::mischen`].
struct Canvas {
    width: u32,
    height: u32,
    fragments: Vec<Fragment>,
    /// Je Pixel die Tiefe des vordersten deckenden Fragments: was dahinter
    /// liegt, ist nie zu sehen.
    front: Vec<f32>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Canvas {
        let pixels = (width as usize) * (height as usize);
        Canvas {
            width,
            height,
            fragments: Vec::with_capacity(pixels * 2),
            front: vec![f32::NEG_INFINITY; pixels],
        }
    }

    /// Rastert ein Dreieck mit baryzentrischer Interpolation.
    ///
    /// Die Projektion ist orthographisch, deshalb ist lineare Interpolation
    /// exakt — es braucht keine perspektivische Korrektur.
    ///
    /// Ein Pixel gehört dazu, wenn sein Mittelpunkt im Dreieck liegt. Liegt
    /// er genau auf einer Kante, entscheidet die Füllregel: er gehört nur
    /// dem Dreieck, für das die Kante oben oder links liegt, und zwei Dreiecke
    /// mit gemeinsamer Kante bekommen ihn genau einmal.
    /// Siehe docs/renderer/naehte.md, „Füllregel“.
    fn triangle(
        &mut self,
        v: [Vertex; 3],
        sample: &impl Fn(f32, f32) -> [u8; 4],
        inside: &impl Fn(f32, f32) -> bool,
        shading: Shading,
        samples: u32,
    ) {
        let Shading {
            shade,
            tint,
            order,
            ao_face,
            fluessig,
            deckung,
        } = shading;
        let area = edge(v[0], v[1], v[2].x, v[2].y);
        if area.abs() < 1e-6 {
            return;
        }
        // Ein Umlaufsinn für alle, damit "oben links" überall dasselbe heisst.
        let (v, area) = if area > 0.0 {
            (v, area)
        } else {
            ([v[0], v[2], v[1]], -area)
        };
        let fuellt = [
            top_left(v[1], v[2]),
            top_left(v[2], v[0]),
            top_left(v[0], v[1]),
        ];
        let bereich: [(i32, i32); 3] = std::array::from_fn(|achse| zellbereich(&v, achse));
        let fest = bereich.iter().all(|&(lo, hi)| lo == hi);

        let min_x = v
            .iter()
            .map(|p| p.x)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as u32;
        let max_x = (v.iter().map(|p| p.x).fold(f32::MIN, f32::max).ceil() as i64)
            .clamp(0, self.width as i64) as u32;
        let min_y = v
            .iter()
            .map(|p| p.y)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as u32;
        let max_y = (v.iter().map(|p| p.y).fold(f32::MIN, f32::max).ceil() as i64)
            .clamp(0, self.height as i64) as u32;

        for y in min_y..max_y {
            for x in min_x..max_x {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let e = [
                    edge(v[1], v[2], px, py),
                    edge(v[2], v[0], px, py),
                    edge(v[0], v[1], px, py),
                ];
                if (0..3).any(|i| e[i] < 0.0 || (e[i] == 0.0 && !fuellt[i])) {
                    continue;
                }

                let index = (y as usize) * (self.width as usize) + x as usize;
                let w = e.map(|e| e / area);
                let depth = w[0] * v[0].depth + w[1] * v[1].depth + w[2] * v[2].depth;
                if depth < self.front[index] {
                    continue;
                }
                // Alpha 0 lässt keine Spur: die vier Overlay-Flächen des
                // Grasblocks liegen deckungsgleich auf dem Grundwürfel, und
                // wo ihre Textur leer ist, bleibt der Grund.
                let Some(texel) = filtered(&v, area, (px, py), sample, inside, samples, deckung)
                else {
                    continue;
                };
                if texel[3] == 255 {
                    self.front[index] = self.front[index].max(depth);
                }
                let ao = ao_face.map_or(0, |(face, ecken)| {
                    let s = w[0] * v[0].s + w[1] * v[1].s + w[2] * v[2].s;
                    let t = w[0] * v[0].t + w[1] * v[1].t + w[2] * v[2].t;
                    ao_word(face, corner_weights(ecken, [s, t]))
                });
                // Der Punkt im Raum von einer Ecke aus, damit eine Achse, auf
                // der alle Ecken gleich liegen, genau bleibt.
                let zelle = if fest {
                    bereich.map(|(lo, _)| lo)
                } else {
                    std::array::from_fn(|a| {
                        let p = v[0].pos[a]
                            + w[1] * (v[1].pos[a] - v[0].pos[a])
                            + w[2] * (v[2].pos[a] - v[0].pos[a]);
                        (p.floor() as i32).clamp(bereich[a].0, bereich[a].1)
                    })
                };
                self.fragments.push(Fragment {
                    pixel: index as u32,
                    depth,
                    order,
                    color: shaded(texel, shade, tint),
                    ao,
                    fluessig,
                    zelle,
                });
            }
        }
    }

    /// Ordnet die Fragmente je Pixel von hinten nach vorne.
    fn sortieren(&mut self) {
        self.fragments.sort_unstable_by(|a, b| {
            a.pixel
                .cmp(&b.pixel)
                .then(a.depth.total_cmp(&b.depth))
                .then(a.order.cmp(&b.order))
        });
    }

    /// Die Würfel, in denen Fragmente liegen.
    fn zellen(&self) -> BTreeSet<Cell> {
        self.fragments.iter().map(|f| f.zelle).collect()
    }

    /// Mischt je Pixel die Fragmente, die `nimm` durchlässt, von hinten nach
    /// vorne, nach [`Canvas::sortieren`]. Mit `ao` dazu die AO-Karte aus dem
    /// vordersten von ihnen je Pixel, das nicht von einer Flüssigkeit ist:
    /// Den Anteil des Wassers beleuchtet die Tönungskarte, der Rest ist,
    /// was durch das Wasser zu sehen ist. Mit `normalen` je Rang einer
    /// Fläche die [`Geometrie`] des vordersten.
    fn mischen(
        &self,
        ao: bool,
        normalen: Option<&[Normalen]>,
        nimm: impl Fn(&Fragment) -> bool,
    ) -> (RgbaImage, Option<Vec<u32>>, Option<Vec<Geometrie>>) {
        let pixel = (self.width * self.height) as usize;
        let mut image = RgbaImage::new(self.width, self.height);
        let mut map = ao.then(|| vec![0u32; pixel]);
        let mut geometrie = normalen.map(|_| vec![Geometrie::default(); pixel]);
        for pixel in self.fragments.chunk_by(|a, b| a.pixel == b.pixel) {
            let mut vorderstes = None;
            let mut fest = None;
            let mut color = [0u8; 4];
            for fragment in pixel.iter().filter(|f| nimm(f)) {
                color = over(fragment.color, color);
                vorderstes = Some(fragment);
                if !fragment.fluessig {
                    fest = Some(fragment);
                }
            }
            let Some(vorderstes) = vorderstes else {
                continue;
            };
            let index = vorderstes.pixel;
            image.put_pixel(index % self.width, index / self.width, Rgba(color));
            if let Some(map) = &mut map {
                map[index as usize] = fest.map_or(0, |f| f.ao);
            }
            if let (Some(geometrie), Some(normalen)) = (&mut geometrie, normalen) {
                let (normale, shade, wasser) = normalen[vorderstes.order as usize];
                geometrie[index as usize] = Geometrie {
                    tiefe: vorderstes.depth,
                    normale,
                    shade,
                    wasser: if wasser {
                        f32::from(vorderstes.color[3]) / 255.0
                    } else {
                        0.0
                    },
                };
            }
        }
        (image, map, geometrie)
    }
}

/// Liegt jede Fläche, die der Rasterizer vom Modell zeichnet, auf einer der
/// drei vorderen Seiten seines Würfels im Blick, bei x, y oder z gleich 1?
/// Dann liegt alles im Würfel hinter jeder von ihnen.
/// Siehe docs/renderer/kamera.md, „Ein Teil im Würfel eines anderen Blocks“.
pub(crate) fn auf_den_vorderseiten(model: &BakedModel, projection: &Projection) -> bool {
    model
        .quads
        .iter()
        .map(|quad| quad_im_blick(quad, projection.richtung()))
        .filter(|quad| seite(quad, projection).is_some())
        .all(|quad| (0..3).any(|achse| quad.corners.iter().all(|c| c[achse] == 1.0)))
}

/// Die Würfel, in denen ein Dreieck liegt, auf einer Achse: von der
/// kleinsten bis zur grössten Ecke. Liegt das ganze Dreieck in einer
/// Würfelebene, gehört es dem Würfel dahinter, von der Kamera aus gesehen.
/// Je Fragment bleibt der Würfel in diesem Bereich: Die Gewichte runden,
/// und ein Fragment an einer eigenen Kante fiele sonst knapp in einen
/// Nachbarwürfel.
/// Siehe docs/renderer/kamera.md, „Sortiert wird nach Würfeln“.
fn zellbereich(v: &[Vertex; 3], achse: usize) -> (i32, i32) {
    let werte = v.map(|p| p.pos[achse]);
    let lo = werte.iter().copied().fold(f32::MAX, f32::min);
    let hi = werte.iter().copied().fold(f32::MIN, f32::max);
    if lo == hi && lo == lo.round() {
        return (lo as i32 - 1, lo as i32 - 1);
    }
    let unten = lo.floor() as i32;
    (unten, (hi.ceil() as i32 - 1).max(unten))
}

/// Füllregel: liegt die Kante von `a` nach `b` oben oder links? Bei dem
/// Umlaufsinn aus `triangle` heisst das: waagrecht nach rechts oder
/// aufwärts.
fn top_left(a: Vertex, b: Vertex) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dy == 0.0 && dx > 0.0) || dy < 0.0
}

/// Baryzentrische Gewichte eines Punkts.
fn weights(v: &[Vertex; 3], area: f32, px: f32, py: f32) -> [f32; 3] {
    [
        edge(v[1], v[2], px, py) / area,
        edge(v[2], v[0], px, py) / area,
        edge(v[0], v[1], px, py) / area,
    ]
}

// ponytail: uv je Abtastpunkt über `weights`, bei scale 4 sind das 192
// Kantenauswertungen je Pixel. Gradienten je Dreieck sparten sie, änderten
// aber die Rundung der uv. Der Sprite-Bau ist ein kleiner Teil eines Laufs,
// siehe docs/benutzung/kosten.md, „Dauer“.
/// Mittelwert der Texel unter einem Pixel, mit vormultipliziertem Alpha und
/// in linearem Licht wie die Pyramide. Gezählt werden nur Abtastpunkte
/// innerhalb der Fläche; liegt keiner darin, weil die Fläche schmaler ist
/// als ein Pixel, gilt der Mittelpunkt. `None`, wenn kein Texel deckt oder
/// der Alpha-Test einer ausgeschnittenen Fläche ihn verwirft, siehe
/// [`Deckung`].
/// Siehe docs/benutzung/zoomstufen.md, „Verkleinern“.
fn filtered(
    v: &[Vertex; 3],
    area: f32,
    (px, py): (f32, f32),
    sample: &impl Fn(f32, f32) -> [u8; 4],
    inside: &impl Fn(f32, f32) -> bool,
    n: u32,
    deckung: Deckung,
) -> Option<[u8; 4]> {
    // Summe der vormultiplizierten Farben in linearem Licht, Summe der
    // Alphas, Anzahl — und ob alle Abtastpunkte dasselbe Texel trafen; dann
    // ist das Texel selbst das Mittel, ohne Umweg über lineares Licht.
    // Siehe docs/renderer/naehte.md, „Textur über den Pixel gemittelt“.
    let mut acc = ([0.0f32; 3], 0.0f32, 0u32);
    let mut einzig: Option<Option<[u8; 4]>> = None;
    let mut add = |acc: &mut ([f32; 3], f32, u32), texel: [u8; 4]| {
        einzig = match einzig {
            None => Some(Some(texel)),
            Some(Some(t)) if t == texel => Some(Some(t)),
            _ => Some(None),
        };
        let a = texel[3] as f32 / 255.0;
        for (sum, &value) in acc.0.iter_mut().zip(&texel[..3]) {
            *sum += LINEAR[value as usize] * a;
        }
        acc.1 += a;
        acc.2 += 1;
    };
    let uv = |x: f32, y: f32| {
        let w = weights(v, area, x, y);
        (
            w[0] * v[0].u + w[1] * v[1].u + w[2] * v[2].u,
            w[0] * v[0].v + w[1] * v[1].v + w[2] * v[2].v,
        )
    };
    // Der Alpha-Test je Texel, vor dem Mitteln: Was er verwirft, deckt
    // nicht, und ausgeschnitten deckt, was bleibt, ganz.
    let getestet = |u: f32, vv: f32| {
        let [r, g, b, a] = sample(u, vv);
        match deckung {
            Deckung::Gemischt { schwelle } if a < schwelle => [r, g, b, 0],
            Deckung::Gemischt { .. } => [r, g, b, a],
            Deckung::Ausgeschnitten { schwelle, .. } => {
                [r, g, b, if a >= schwelle { 255 } else { 0 }]
            }
        }
    };
    for sy in 0..n {
        for sx in 0..n {
            let dx = (sx as f32 + 0.5) / n as f32 - 0.5;
            let dy = (sy as f32 + 0.5) / n as f32 - 0.5;
            let (u, vv) = uv(px + dx, py + dy);
            if inside(u, vv) {
                add(&mut acc, getestet(u, vv));
            }
        }
    }
    if acc.2 == 0 {
        let (u, vv) = uv(px, py);
        add(&mut acc, getestet(u, vv));
    }
    let (sum, alpha, count) = acc;
    if alpha <= 0.0 {
        return None;
    }
    if let Some(Some(texel)) = einzig {
        return Some(texel);
    }
    let Deckung::Ausgeschnitten { fuellung, .. } = deckung else {
        return Some([
            to_srgb(sum[0] / alpha),
            to_srgb(sum[1] / alpha),
            to_srgb(sum[2] / alpha),
            (alpha / count as f32 * 255.0).round() as u8,
        ]);
    };
    // Jeder Abtastpunkt deckt ganz oder gar nicht, `alpha` zählt also die
    // deckenden. Deckt genau die Hälfte, entscheidet wie im Spiel das Texel
    // in der Pixelmitte.
    let haelfte = count as f32 / 2.0;
    let mitte_deckt = || {
        let (u, vv) = uv(px, py);
        getestet(u, vv)[3] != 0
    };
    if alpha < haelfte || (alpha == haelfte && !mitte_deckt()) {
        return None;
    }
    let farbe = |c: usize| match fuellung {
        Some(loch) => to_srgb((sum[c] + (count as f32 - alpha) * loch[c]) / count as f32),
        None => to_srgb(sum[c] / alpha),
    };
    Some([farbe(0), farbe(1), farbe(2), 255])
}

fn shaded(texel: [u8; 4], shade: f32, tint: Option<[f32; 3]>) -> [u8; 4] {
    let tint = tint.unwrap_or([1.0, 1.0, 1.0]);
    let mut out = [0u8; 4];
    for c in 0..3 {
        out[c] = (texel[c] as f32 * shade * tint[c])
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    out[3] = texel[3];
    out
}

/// Quelle über Ziel, beide mit unvormultipliziertem Alpha.
///
/// Ganzzahlig, auf 1/255² erweitert und zum Schluss gerundet — dieselbe
/// Rechnung steht im Shader (`gpu.wgsl`), und jede Karte liefert dasselbe
/// Byte wie die CPU.
/// Siehe docs/benutzung/grafikkarte.md, „Byte für Byte wie die CPU“.
pub fn over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    if src[3] == 255 || dst[3] == 0 {
        return src;
    }
    let sa = src[3] as u32;
    let da = dst[3] as u32 * (255 - sa);
    let a = sa * 255 + da;
    let mut out = [0u8; 4];
    for c in 0..3 {
        out[c] = ((src[c] as u32 * sa * 255 + dst[c] as u32 * da + a / 2) / a) as u8;
    }
    out[3] = ((a + 127) / 255) as u8;
    out
}

/// Vorzeichenbehaftete Fläche des Dreiecks aus zwei Ecken und einem Punkt,
/// immer von der lexikographisch kleineren Ecke aus gerechnet. Zwei
/// Dreiecke mit gemeinsamer Kante bekommen so exakt entgegengesetzte
/// Werte. Von verschiedenen Ecken aus rundet f32 verschieden, und ein
/// Pixel genau auf der Kante fiel bei beiden durch — ein Loch im selben
/// Pixel jedes Kreuzmodells, und je nach libm auch in Türen und Knöpfen.
fn edge(a: Vertex, b: Vertex, px: f32, py: f32) -> f32 {
    if (a.x, a.y) > (b.x, b.y) {
        return -edge(b, a, px, py);
    }
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::baker::Quad;
    use crate::assets::blockentity::Schicht;
    use crate::render::Kamera;

    /// Ob die Kamera eine Fläche sieht, hängt an ihrer Achse: Eine Fläche
    /// nach Süden, 30° nach unten gekippt, sehen 2:1 und 4:3 noch, 1:1 und
    /// von oben nicht mehr (tan 30° liegt zwischen 1/2 und 2/3), `north-45`
    /// bis 45°. Eine senkrechte steht von oben auf der Kante, eine nach Osten
    /// auch bei `north-45`.
    #[test]
    fn zur_kamera_folgt_der_achse() {
        let (sin, cos) = 30f32.to_radians().sin_cos();
        let gekippt = [0.0, -sin, cos];
        let senkrecht = [0.0, 0.0, 1.0];
        let osten = [1.0, 0.0, 0.0];
        for (kamera, sieht_gekippt, sieht_senkrecht, sieht_osten) in [
            ("2:1", true, true, true),
            ("4:3", true, true, true),
            ("1:1", false, true, true),
            ("top", false, false, false),
            ("north-45", true, true, false),
            ("top-north", false, false, false),
        ] {
            let p = Projection::mit_kamera(32, Kamera::parse(kamera).unwrap());
            assert_eq!(zur_kamera(gekippt, &p), sieht_gekippt, "{kamera}, gekippt");
            assert_eq!(
                zur_kamera(senkrecht, &p),
                sieht_senkrecht,
                "{kamera}, senkrecht"
            );
            assert_eq!(zur_kamera(osten, &p), sieht_osten, "{kamera}, Osten");
        }
    }

    /// Die Helligkeit im Himmelslicht `licht` ohne Blocklicht in der
    /// Oberwelt, in jedem Kanal gleich.
    fn brightness(licht: u8) -> f32 {
        brightness_rgb(&DimensionType::oberwelt(), licht, 0)[0]
    }

    /// [`brightness`] in 255steln.
    fn light_factor(licht: u8) -> u32 {
        Light::sky(licht).factors()[0]
    }

    /// Die Helligkeit je Himmelslicht, nach `lightmap.fsh` von Hand
    /// ausgerechnet: Umgebungsfarbe #0a0a0a, `SkyFactor` 1, Helligkeit 0,5.
    #[test]
    fn helligkeit_wie_im_spiel() {
        let erwartet = [
            0.09355, 0.13259, 0.17406, 0.21810, 0.26489, 0.31456, 0.36725, 0.42304, 0.48195,
            0.54391, 0.60878, 0.67642, 0.74707, 0.82231, 0.90794, 1.0,
        ];
        for (licht, b) in erwartet.into_iter().enumerate() {
            let ist = brightness(licht as u8);
            assert!((ist - b).abs() < 1e-5, "Licht {licht}: {ist}, erwartet {b}");
        }
    }

    /// Blocklicht wie `lightmap.fsh`: Bei voller Stufe ist alles hell, und
    /// eine Meeresgurke mit 6 im Himmelslicht 5 färbt warm, von Hand nach
    /// dem Shader gerechnet mit `BlockFactor` 1,4 und dem Standard
    /// `#FFD88C` für `BlockLightTint`. Ohne Blocklicht bleibt es
    /// [`brightness`].
    #[test]
    fn blocklicht_wie_im_spiel() {
        let oberwelt = DimensionType::oberwelt();
        assert_eq!(brightness_rgb(&oberwelt, 0, 15), [1.0; 3]);
        assert_eq!(brightness_rgb(&oberwelt, 15, 15), [1.0; 3]);
        let warm = brightness_rgb(&oberwelt, 5, 6);
        for (ist, soll) in warm.iter().zip([0.58609, 0.53676, 0.44063]) {
            assert!((ist - soll).abs() < 1e-4, "{warm:?}");
        }
        assert_eq!(brightness_rgb(&oberwelt, 7, 0), [brightness(7); 3]);
        assert_eq!(Light::sky(14).factors(), [light_factor(14); 3]);
        assert_eq!(Light { sky: 0, block: 15 }.factors(), [255; 3]);
    }

    /// Die Lightmap nimmt die Werte aus dem Typ der Dimension, von Hand nach
    /// `lightmap.fsh` gerechnet. Im Nether und im Ende ist `SkyFactor` 0:
    /// Himmelslicht ändert nichts, und ohne Blocklicht bleibt die
    /// Umgebungsfarbe, `#302821` und `#3f473f`, zur Hälfte zu `notGamma`
    /// gemischt. Blocklicht 9 färbt im Nether wärmer als in der Oberwelt,
    /// und Blocklicht 15 macht überall alles hell. Ein eigener Typ färbt
    /// Himmels- und Blocklicht in seinen Farben.
    #[test]
    fn lightmap_je_dimension() {
        let typ = |id: &str| DimensionType::des_spiels(id).unwrap();
        let (oberwelt, nether, ende) = (
            Lightmap::oberwelt(),
            Lightmap::new(&typ("minecraft:the_nether")),
            Lightmap::new(&typ("minecraft:the_end")),
        );
        let l = |sky, block| Light { sky, block };
        assert_eq!(oberwelt.factors(l(0, 0)), [24; 3]);
        assert_eq!(oberwelt.factors(l(0, 9)), [167, 145, 101]);
        for sky in [0, 14, 15] {
            assert_eq!(
                nether.factors(l(sky, 0)),
                [96, 80, 66],
                "Nether, Himmel {sky}"
            );
            assert_eq!(
                ende.factors(l(sky, 0)),
                [114, 128, 114],
                "Ende, Himmel {sky}"
            );
            assert_eq!(nether.factors(l(sky, 9)), [196, 166, 119]);
            assert_eq!(ende.factors(l(sky, 9)), [205, 197, 151]);
        }
        for map in [oberwelt, &nether, &ende] {
            assert_eq!(map.factors(l(15, 15)), [255; 3]);
        }
        // Ein eigener Typ: rotes Himmelslicht mit `SkyFactor` 0,5, blaues
        // Blocklicht, ohne Umgebungsfarbe.
        let eigen = Lightmap::new(&DimensionType {
            ambient_light_color: [0, 0, 0],
            sky_light_factor: 0.5,
            sky_light_color: [255, 0, 0],
            block_light_tint: [0, 0, 255],
            ..DimensionType::oberwelt()
        });
        assert_eq!(eigen.factors(l(15, 0)), [183, 0, 0]);
        assert_eq!(eigen.factors(l(0, 6)), [4, 4, 101]);
        assert_eq!(eigen.factors(l(0, 0)), [0; 3]);
    }

    /// Die AO-Karte trägt je Pixel seinen Platz plus 1. Beim vollen Würfel
    /// liegt jeder Pixel mit Farbe auf dem Rand einer der drei Seiten, und
    /// alle drei kommen vor; ohne `ambientocclusion` auch, für das Licht je
    /// Seite, nur weich beleuchtet wird er dann nicht. Bei der oberen Platte
    /// ebenso, ihre Oberseite liegt auf dem Rand. Die der unteren liegt im
    /// Innern, ausser der Block hat volle Kollisionsform. Ein Kasten ganz im
    /// Innern hat nur Plätze im Innern. Zeigt keine Fläche mit Platz einen
    /// Pixel, gibt es keine Karte.
    #[test]
    fn ao_karte_fuer_flaechen_im_licht_davor() {
        let kasten = |from: [f32; 3], to: [f32; 3], ambient_occlusion| BakedModel {
            quads: crate::assets::baker::box_quads(from, to, Textures::MISSING, None, None)
                .collect(),
            ambient_occlusion,
        };
        let wuerfel = |to: [f32; 3], ambient_occlusion| kasten([0.0; 3], to, ambient_occlusion);
        let textures = Textures::new();
        let projection = Projection::new(32);
        let bild = |model: &BakedModel, kollision| {
            let licht = CardinalLight::Default;
            render_mit_licht(
                model,
                &textures,
                &projection,
                Tints::default(),
                licht,
                kollision,
            )
            .unwrap()
        };
        let seiten = |sprite: &Sprite| -> std::collections::BTreeSet<u32> {
            let karte = sprite.ao.as_ref().expect("AO-Karte");
            (sprite.image.pixels().zip(karte))
                .filter(|(p, _)| p.0[3] != 0)
                .map(|(_, w)| w >> 24)
                .collect()
        };
        let sprite = bild(&wuerfel([16.0; 3], true), false);
        let karte = sprite.ao.as_ref().expect("AO-Karte");
        for (i, p) in sprite.image.pixels().enumerate() {
            assert_eq!(p.0[3] != 0, karte[i] >> 24 != 0, "Pixel {i}");
        }
        assert_eq!(seiten(&sprite), [1, 2, 3].into());
        assert!(sprite.weich);
        let flach = bild(&wuerfel([16.0; 3], false), false);
        assert_eq!(flach.ao, sprite.ao);
        assert!(!flach.weich);
        let oben = kasten([0.0, 8.0, 0.0], [16.0; 3], true);
        assert_eq!(
            seiten(&bild(&oben, false)),
            [1, 2, 3].into(),
            "obere Platte"
        );
        let unten = wuerfel([16.0, 8.0, 16.0], true);
        assert_eq!(
            seiten(&bild(&unten, false)),
            [2, 3, 4].into(),
            "untere Platte"
        );
        assert_eq!(
            seiten(&bild(&unten, true)),
            [1, 2, 3].into(),
            "mit Kollision"
        );
        let innen = kasten([4.0, 0.0, 4.0], [12.0, 12.0, 12.0], true);
        assert_eq!(seiten(&bild(&innen, false)), [4, 5, 6].into(), "innen");
        // Flächen einer Flüssigkeit haben keinen Platz, die durchsichtige
        // Oberseite über ihnen zeigt keinen Pixel: keine Karte.
        let mut leer = Textures::new();
        let durchsichtig = leer.einfuegen("leer", RgbaImage::new(16, 16), false);
        let mut deckel = quad(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            None,
        );
        deckel.texture = durchsichtig;
        let nass = innen.quads.iter().map(|q| Quad {
            fluid: Some((fluid::Fluid::Water, Face::Up)),
            ..q.clone()
        });
        let mit_deckel = BakedModel {
            quads: nass.chain([deckel]).collect(),
            ambient_occlusion: true,
        };
        let licht = CardinalLight::Default;
        let sprite = render_mit_licht(
            &mit_deckel,
            &leer,
            &projection,
            Tints::default(),
            licht,
            false,
        );
        assert!(sprite.unwrap().ao.is_none(), "durchsichtiger Deckel");
        // Schräg liegt eine Fläche nicht auf dem Rand, auch mit Kollision:
        // Ihre Normale (0, 1, 1) nimmt oben vor Süden, Platz 3.
        let schraeg = quad(
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            None,
        );
        assert_eq!(
            ao_face(&schraeg, &schraeg, true, Richtung::default()),
            Some(3)
        );
    }

    /// Mit gleichem Licht für beide Anteile gleicht [`tinted_im_licht`] dem
    /// Weg über [`tinted`] und [`darken`] bis auf die Rundung; sonst liegt
    /// der Anteil des Wassers in seinem Licht, der Rest in dem des Blocks.
    #[test]
    fn wasser_im_eigenen_licht() {
        let farben = [pack([0x91, 0xBD, 0x59]), pack([0x3F, 0x76, 0xE4])];
        let anteile = [pack([40, 50, 60]), pack([90, 80, 70])];
        let pixel = [30, 20, 10, 200];
        let [f, g] = [Light::sky(13).factors(), Light::sky(14).factors()];
        let zweimal = darken(tinted(pixel, anteile, farben), f);
        let einmal = tinted_im_licht(pixel, anteile, farben, f, f);
        for c in 0..4 {
            assert!(
                (zweimal[c] as i32 - einmal[c] as i32).abs() <= 1,
                "{zweimal:?} {einmal:?}"
            );
        }
        let nur_wasser = tinted_im_licht([0, 0, 0, 200], [0, anteile[1]], farben, f, g);
        let nur_rest = tinted_im_licht(pixel, [anteile[0], 0], farben, f, g);
        let beide = tinted_im_licht(pixel, anteile, farben, f, g);
        let getrennt = [
            (
                nur_wasser,
                darken(tinted([0, 0, 0, 200], [0, anteile[1]], farben), g),
            ),
            (nur_rest, darken(tinted(pixel, [anteile[0], 0], farben), f)),
        ];
        for (ist, soll) in getrennt {
            for c in 0..4 {
                assert!(
                    (ist[c] as i32 - soll[c] as i32).abs() <= 1,
                    "{ist:?} {soll:?}"
                );
            }
        }
        for c in 0..3 {
            let summe = nur_wasser[c] as i32 + nur_rest[c] as i32;
            assert!((beide[c] as i32 - summe).abs() <= 1, "Kanal {c}");
        }
        assert!(beide[2] > einmal[2], "das Wasser liegt heller");
        assert_eq!(beide[3], 200);
    }

    /// Gepackt wie `LightCoordsUtil.pack`: Blocklicht ab Bit 4, Himmelslicht
    /// ab Bit 20, und zurück.
    #[test]
    fn licht_gepackt_wie_im_spiel() {
        assert_eq!(Light { sky: 15, block: 15 }.packed(), VOLL_HELL);
        assert_eq!(Light { sky: 7, block: 3 }.packed(), 0x70_0030);
        for sky in 0..=15 {
            for block in 0..=15 {
                let licht = Light { sky, block };
                assert_eq!(Light::from_packed(licht.packed()), licht);
            }
        }
    }

    /// `smoothBlend` mittelt die vier Werte in Sechzehnteln einer Stufe. Ist
    /// die Zelle vor der Seite heller als 2, im Himmels- oder im Blocklicht,
    /// nimmt ein Nachbar ohne Licht ihres und einer ohne Himmelslicht ihr
    /// Himmelslicht; sonst zählen beide, wie sie sind.
    #[test]
    fn licht_an_den_ecken_wie_im_spiel() {
        let p = |sky, block| Light { sky, block }.packed();
        // Himmelslicht 57/4 = 14,25, in Sechzehnteln 228.
        assert_eq!(
            smooth_blend(p(15, 0), p(15, 0), p(14, 0), p(13, 0)),
            0xe4_0000
        );
        // Blocklicht (2 + 5 + 0 + 2)/4 = 2,25, in Sechzehnteln 36.
        assert_eq!(smooth_blend(0, p(0, 5), p(15, 0), p(15, 2)), 0xf0_0024);
        // Die Mitte in 2 und 2: Himmelslicht 1, Blocklicht 7/4.
        assert_eq!(smooth_blend(0, p(0, 5), p(2, 0), p(2, 2)), 0x10_001c);
        // Blocklicht 3 allein reicht: Alle drei nehmen es.
        assert_eq!(smooth_blend(0, 0, 0, p(0, 3)), 0x30);
        assert_eq!(smooth_blend(0, 0, 0, p(2, 2)), 0x08_0008);
    }

    /// Auf einer Stufe liefert die linear gefilterte Lightmap genau deren
    /// Wert; zwischen zwei Stufen mischt sie, in der Mitte halb und halb.
    #[test]
    fn lightmap_linear_gefiltert() {
        for sky in 0..=15 {
            for block in 0..=15 {
                let licht = Light { sky, block };
                assert_eq!(
                    Lightmap::oberwelt().linear(licht.packed()),
                    licht.factors(),
                    "{licht:?}"
                );
            }
        }
        let [a, b] = [Light::sky(14).factors(), Light::sky(15).factors()];
        assert_eq!(
            Lightmap::oberwelt().linear(0xe8_0000),
            std::array::from_fn(|c| (a[c] + b[c]).div_ceil(2))
        );
        let [a, b] = [Light { sky: 3, block: 6 }, Light { sky: 3, block: 7 }].map(Light::factors);
        let viertel = Lightmap::oberwelt().linear(0x30_0064);
        assert_eq!(
            viertel,
            std::array::from_fn(|c| (12 * a[c] + 4 * b[c] + 8) / 16)
        );
        assert_ne!(a, b);
    }

    /// Die Anteile der Ecken an einem Punkt der Oberseite: an einer Ecke nur
    /// sie, sonst baryzentrisch im Dreieck 0-1-2 oder 2-3-0, zusammen immer
    /// 255. Mit den Ecken einer Innenecke liegt die Mitte zwischen der
    /// dunklen und der hellen Ecke.
    #[test]
    fn anteile_der_ecken() {
        let oben = ecken_im_blick(Richtung::default(), 0);
        assert_eq!(corner_weights(oben, [0.0, 0.0]), [255, 0, 0, 0]);
        assert_eq!(corner_weights(oben, [0.0, 1.0]), [0, 255, 0, 0]);
        assert_eq!(corner_weights(oben, [1.0, 1.0]), [0, 0, 255, 0]);
        assert_eq!(corner_weights(oben, [1.0, 0.0]), [0, 0, 0, 255]);
        assert_eq!(corner_weights(oben, [0.25, 0.75]), [64, 127, 64, 0]);
        assert_eq!(corner_weights(oben, [0.75, 0.25]), [64, 0, 64, 127]);
        for k in 0..4 {
            let richtung =
                Richtung::parse(["se", "sw", "nw", "ne"][k], Kamera::ZWEI_ZU_EINS).unwrap();
            for face in 0..3 {
                let ecken = ecken_im_blick(richtung, face);
                for i in 0..=10 {
                    for j in 0..=10 {
                        let w = corner_weights(ecken, [i as f32 / 10.0, j as f32 / 10.0]);
                        assert_eq!(w.iter().sum::<u32>(), 255, "Seite {face}, {i}, {j}");
                    }
                }
            }
        }
        let mut innenecke = [u32::MAX; AO_PLAETZE];
        innenecke[0] = u32::from_le_bytes([102, 153, 255, 153]);
        let an = |p| ecken_faktor(ao_word(0, corner_weights(oben, p)), innenecke);
        assert_eq!(an([0.0, 0.0]), 102);
        assert_eq!(an([0.5, 0.5]), 178);
        assert_eq!(an([1.0, 1.0]), 255);
        assert_eq!(ecken_faktor(0, innenecke), 255, "Pixel ohne Seite");
    }

    /// Aus der Vorgabe-Richtung sind die Ecken die von oben, Süden und
    /// Osten aus `FaceInfo`. Aus jeder anderen liegt an derselben Stelle der
    /// Seite die Ecke der Seite der Welt, die dort liegt: aus Nordwesten
    /// zeigt die Seite nach Süden die Ecken des Nordens, deren erste oben
    /// links.
    #[test]
    fn ecken_der_seiten_im_blick() {
        let vorgabe = Richtung::default();
        assert_eq!(
            std::array::from_fn::<_, 3, _>(|s| ecken_im_blick(vorgabe, s)),
            [
                [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
                [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]],
            ]
        );
        let nw = Richtung::parse("nw", Kamera::ZWEI_ZU_EINS).unwrap();
        // Norden: (1, 1, 0), (1, 0, 0), (0, 0, 0), (0, 1, 0), halb gedreht.
        assert_eq!(
            ecken_im_blick(nw, 1),
            [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]
        );
        // Aus Südwesten: oben gedreht, die Diagonale von Ecke 0 nach 2
        // gekippt; im Süden der Westen (0, 1, 0), (0, 0, 0), (0, 0, 1),
        // (0, 1, 1), im Osten der Süden (0, 1, 1), (0, 0, 1), (1, 0, 1),
        // (1, 1, 1).
        let sw = Richtung::parse("sw", Kamera::ZWEI_ZU_EINS).unwrap();
        assert_eq!(
            std::array::from_fn::<_, 3, _>(|s| ecken_im_blick(sw, s)),
            [
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]],
            ]
        );
        for name in ["sw", "nw", "ne"] {
            let richtung = Richtung::parse(name, Kamera::ZWEI_ZU_EINS).unwrap();
            for s in 0..3 {
                let mut ecken = ecken_im_blick(richtung, s).map(|[a, b]| (a as u8, b as u8));
                ecken.sort();
                assert_eq!(ecken, [(0, 0), (0, 1), (1, 0), (1, 1)], "{name}, Seite {s}");
            }
        }
    }

    /// Über dem Grund D im Licht l ergibt die Oberfläche `α · W + (1 − α) ·
    /// b(l) · D`, so wie das Spiel sie über den dunkleren Grund legt.
    #[test]
    fn wasser_zeigt_den_grund_im_licht() {
        let wasser = [60, 100, 220, 180];
        let grund = [150, 110, 60, 255];
        for licht in 0..=15 {
            let ist = over(wasser, darken(grund, Light::sky(licht).factors()));
            let a = 180.0 / 255.0;
            for c in 0..3 {
                let soll = a * wasser[c] as f32 + (1.0 - a) * brightness(licht) * grund[c] as f32;
                assert!(
                    (ist[c] as f32 - soll).abs() <= 1.0,
                    "Licht {licht}: {ist:?}, Kanal {c} {soll}"
                );
            }
            assert_eq!(ist[3], 255);
        }
        // Volles Licht lässt jeden Pixel, wie er ist; das Alpha bleibt immer.
        assert_eq!(light_factor(FULL_LIGHT), 255);
        assert_eq!(darken([1, 2, 3, 4], [255; 3]), [1, 2, 3, 4]);
        assert_eq!(
            darken([200, 100, 50, 77], [light_factor(0); 3]),
            [19, 9, 5, 77]
        );
    }

    #[test]
    fn abtastung_folgt_dem_scale() {
        assert_eq!(texture_samples(64), 2);
        assert_eq!(texture_samples(32), 2);
        assert_eq!(texture_samples(16), 2);
        assert_eq!(texture_samples(8), 4);
        assert_eq!(texture_samples(4), 8);
        assert_eq!(texture_samples(2), 16);
        assert_eq!(texture_samples(1), 16);
    }

    /// Ein Texturausschnitt ohne Breite darf nicht zum Absturz führen —
    /// `clamp` verlangt min <= max. Vanilla hat solche Flächen.
    #[test]
    fn texturausschnitt_ohne_breite() {
        let mut schmal = quad(
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            None,
        );
        schmal.uvs = [[0.5, 0.0], [0.5, 0.0], [0.5, 1.0], [0.5, 1.0]];
        let model = BakedModel {
            quads: vec![schmal],
            ambient_occlusion: false,
        };
        assert!(
            render(
                &model,
                &Textures::new(),
                &Projection::new(16),
                Tints::default(),
            )
            .is_some()
        );
    }

    /// Nadeln und Löcher wie im Fichtenlaub, zu 37,5 % Loch; die Nadeln
    /// mit diesem Alpha.
    fn nadeln(alpha: u8) -> RgbaImage {
        RgbaImage::from_fn(16, 16, |x, y| {
            let loch = (x * 5 + y * 3) % 8 < 3;
            Rgba([100, 150, 100, if loch { 0 } else { alpha }])
        })
    }

    /// Die Alphas eines Sprites aus einer Oberseite bei scale 32. Sie
    /// liegt um 45° gedreht, jeder Pixel trifft zwei bis vier Texel.
    fn alphas(textures: &Textures, anpassen: impl Fn(&mut Quad)) -> std::collections::BTreeSet<u8> {
        let mut oben = quad(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            None,
        );
        anpassen(&mut oben);
        let model = BakedModel {
            quads: vec![oben],
            ambient_occlusion: false,
        };
        let sprite = render(&model, textures, &Projection::new(32), Tints::default()).unwrap();
        sprite.image.pixels().map(|p| p[3]).collect()
    }

    /// Wie in der Schicht CUTOUT des Spiels deckt ein Pixel einer Textur
    /// mit Löchern ganz oder gar nicht. Gemischt wird weiter mit
    /// `force_translucent`, mit halb durchsichtigen Texeln und als
    /// Flüssigkeit.
    #[test]
    fn ausgeschnitten_deckt_ganz_oder_gar_nicht() {
        let mut textures = Textures::new();
        let loecher = textures.einfuegen("loecher", nadeln(255), false);
        let halb = textures.einfuegen("halb", nadeln(128), false);
        let gemischt =
            |alphas: &std::collections::BTreeSet<u8>| alphas.iter().any(|&a| a != 0 && a != 255);

        let ausgeschnitten = alphas(&textures, |q| q.texture = loecher);
        assert_eq!(ausgeschnitten, [0, 255].into());
        // Es zählt nur der Ausschnitt der Fläche: links Löcher, rechts halb.
        let links_loecher = textures.einfuegen(
            "links_loecher",
            RgbaImage::from_fn(16, 16, |x, y| {
                let alpha = match (x < 8, (x * 5 + y * 3) % 8 < 3) {
                    (true, true) => 0,
                    (true, false) => 255,
                    (false, _) => 128,
                };
                Rgba([100, 150, 100, alpha])
            }),
            false,
        );
        let links = alphas(&textures, |q| {
            q.texture = links_loecher;
            q.uvs = [[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [0.0, 1.0]];
        });
        assert_eq!(links, [0, 255].into());
        assert!(gemischt(&alphas(&textures, |q| q.texture = links_loecher)));
        assert!(gemischt(&alphas(&textures, |q| {
            q.texture = loecher;
            q.force_translucent = true;
        })));
        // Gemischt, nicht nur das Alpha der Texel: 128 käme auch aus Pixeln,
        // deren Abtastpunkte alle dasselbe Texel treffen.
        let durchscheinend = alphas(&textures, |q| q.texture = halb);
        assert!(durchscheinend.iter().any(|a| ![0, 128, 255].contains(a)));
        assert!(gemischt(&alphas(&textures, |q| {
            q.texture = loecher;
            q.fluid = Some((crate::assets::fluid::Fluid::Water, Face::Up));
        })));
    }

    /// Mit `dark_cutout` zählen die Löcher in der Farbe aus
    /// `fillEmptyAreasWithDarkColor` mit: dieselben Pixel decken, aber am
    /// Rand der Nadeln dunkler.
    #[test]
    fn dark_cutout_dunkelt_die_raender() {
        let mut textures = Textures::new();
        let hell = textures.einfuegen("hell", nadeln(255), false);
        let dunkel = textures.einfuegen("dunkel", nadeln(255), true);
        let sprite = |textur| {
            let mut oben = quad(
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
                None,
            );
            oben.texture = textur;
            let model = BakedModel {
                quads: vec![oben],
                ambient_occlusion: false,
            };
            render(&model, &textures, &Projection::new(32), Tints::default())
                .unwrap()
                .image
        };
        let (hell, dunkel) = (sprite(hell), sprite(dunkel));
        let mut dunkler = 0;
        for (h, d) in hell.pixels().zip(dunkel.pixels()) {
            assert_eq!(h[3], d[3]);
            assert!(d[1] <= h[1]);
            dunkler += usize::from(d[1] < h[1]);
        }
        assert!(dunkler > 0);
    }

    /// Eine durchscheinende Blockfläche testet wie `translucent` im Spiel
    /// gegen 0,1: Texel mit Alpha 25 fallen weg, mit 26 bleiben sie.
    #[test]
    fn durchscheinende_blockflaeche_testet_gegen_ein_zehntel() {
        let mut textures = Textures::new();
        let mut flach = |name, alpha| {
            let bild = RgbaImage::from_pixel(16, 16, Rgba([100, 150, 100, alpha]));
            textures.einfuegen(name, bild, false)
        };
        let (unter, ueber) = (flach("unter", 25), flach("ueber", 26));
        let sprite = |textur| {
            let mut oben = quad(
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
                None,
            );
            oben.texture = textur;
            oben.force_translucent = true;
            let model = BakedModel {
                quads: vec![oben],
                ambient_occlusion: false,
            };
            render(&model, &textures, &Projection::new(32), Tints::default())
        };
        assert!(sprite(unter).is_none_or(|s| s.image.pixels().all(|p| p[3] == 0)));
        assert!(sprite(ueber).unwrap().image.pixels().any(|p| p[3] == 26));
    }

    /// Der Alpha-Test von `cutout_terrain`: unter der Hälfte der
    /// Abtastpunkte verworfen, darüber ganz deckend in der Farbe der
    /// deckenden. Bei genau der Hälfte entscheidet das Texel in der
    /// Pixelmitte. Mit der Füllung aus `dark_cutout` zählen die Löcher in
    /// ihrer Farbe mit. Getestet wird je Texel, vor dem Mitteln.
    #[test]
    fn alpha_test_wie_cutout_terrain() {
        // Über dem Pixel (0, 0) ist u = x und v = y; abgetastet wird bei
        // 0,25 und 0,75, die Mitte liegt bei 0,5.
        let ecke = |x: f32, y: f32| Vertex {
            x,
            y,
            depth: 0.0,
            u: x,
            v: y,
            s: 0.0,
            t: 0.0,
            pos: [0.0; 3],
        };
        let v = [ecke(-1.0, -1.0), ecke(3.0, -1.0), ecke(-1.0, 3.0)];
        let area = edge(v[0], v[1], v[2].x, v[2].y);
        let innen = |_: f32, _: f32| true;
        let farbe = [200, 100, 50, 255];
        let wo = |deckt: fn(f32, f32) -> bool| {
            move |u: f32, vv: f32| {
                if deckt(u, vv) { farbe } else { [0; 4] }
            }
        };
        let pixel = |sample: &dyn Fn(f32, f32) -> [u8; 4], deckung| {
            filtered(&v, area, (0.5, 0.5), &sample, &innen, 2, deckung)
        };
        let aus = Deckung::Ausgeschnitten {
            fuellung: None,
            schwelle: 128,
        };

        let rechts = wo(|u, _| u > 0.6);
        let gemischt = Deckung::Gemischt { schwelle: 26 };
        assert_eq!(pixel(&rechts, gemischt), Some([200, 100, 50, 128]));
        // Alpha 20 und 200 je zur Hälfte: 20 fällt vor dem Mitteln weg, das
        // gibt 100; nach dem Mitteln bestünde 110 den Test.
        let links_blass = |u: f32, _: f32| [200, 100, 50, if u < 0.5 { 20 } else { 200 }];
        assert_eq!(pixel(&links_blass, gemischt), Some([200, 100, 50, 100]));
        assert_eq!(pixel(&rechts, aus), None);
        assert_eq!(pixel(&wo(|u, _| u < 0.6), aus), Some(farbe));
        let drei = wo(|u, v| u > 0.5 || v > 0.5);
        assert_eq!(pixel(&drei, aus), Some(farbe));
        assert_eq!(pixel(&wo(|u, v| u > 0.5 && v > 0.5), aus), None);

        let dunkel = Deckung::Ausgeschnitten {
            fuellung: Some([LINEAR[40]; 3]),
            schwelle: 128,
        };
        let gemittelt = |c: usize| to_srgb((3.0 * LINEAR[farbe[c] as usize] + LINEAR[40]) / 4.0);
        assert_eq!(
            pixel(&drei, dunkel),
            Some([gemittelt(0), gemittelt(1), gemittelt(2), 255])
        );
    }

    /// Das Licht der Blockentities, von Hand nach `minecraft_mix_light` in
    /// `shaders/include/light.glsl` gerechnet: die Richtungen (0,2, 1, −0,7)
    /// und (−0,2, 1, 0,7) normiert, je 0,6, dazu 0,4 Umgebung, höchstens 1.
    #[test]
    fn licht_der_blockentities_wie_im_spiel() {
        for (normale, soll) in [
            ([0.0, 1.0, 0.0], 1.0),
            ([0.0, 0.0, -1.0], 0.73955),
            ([0.0, 0.0, 1.0], 0.73955),
            ([1.0, 0.0, 0.0], 0.49701),
            ([-1.0, 0.0, 0.0], 0.49701),
            ([0.0, -1.0, 0.0], 0.4),
        ] {
            let ist = entity_light(normale, CardinalLight::Default);
            assert!((ist - soll).abs() < 1e-5, "{normale:?}: {ist}");
        }
    }

    /// Im Nether kommt das zweite Licht von unten, (−0,2, −1, 0,7), wie
    /// `Lighting.updateLevel` es für `CardinalLighting.Type.NETHER` setzt:
    /// oben und unten je 0,885, die Seiten wie in der Oberwelt.
    #[test]
    fn licht_der_blockentities_im_nether() {
        for (normale, soll) in [
            ([0.0, 1.0, 0.0], 0.88507),
            ([0.0, -1.0, 0.0], 0.88507),
            ([0.0, 0.0, -1.0], 0.73955),
            ([0.0, 0.0, 1.0], 0.73955),
            ([1.0, 0.0, 0.0], 0.49701),
            ([-1.0, 0.0, 0.0], 0.49701),
        ] {
            let ist = entity_light(normale, CardinalLight::Nether);
            assert!((ist - soll).abs() < 1e-5, "{normale:?}: {ist}");
        }
    }

    fn schicht(beidseitig: bool, je_seite: bool) -> Schicht {
        Schicht {
            alpha: None,
            beidseitig,
            je_seite,
            gemischt: false,
        }
    }

    fn aus_entity(mut quad: Quad, schicht: Schicht) -> Quad {
        quad.entity = Some(Entity {
            schicht,
            farbe: [255; 3],
        });
        quad
    }

    /// Eine Unterseite, die die Kamera nur von hinten sieht.
    fn unterseite() -> Quad {
        quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            None,
        )
    }

    /// Flächen aus Blockentity-Modellen bekommen keine weiche Beleuchtung,
    /// auch als voller Würfel mit `ambientocclusion`: Das Spiel zeichnet
    /// sie nicht mit `ModelBlockRenderer`.
    #[test]
    fn blockentities_ohne_ao() {
        let wuerfel = |entity: bool| BakedModel {
            quads: crate::assets::baker::box_quads(
                [0.0; 3],
                [16.0; 3],
                Textures::MISSING,
                None,
                None,
            )
            .map(|q| {
                if entity {
                    aus_entity(q, schicht(false, false))
                } else {
                    q
                }
            })
            .collect(),
            ambient_occlusion: true,
        };
        let ao = |model: &BakedModel| {
            render(
                model,
                &Textures::new(),
                &Projection::new(32),
                Tints::default(),
            )
            .unwrap()
            .ao
            .is_some()
        };
        assert!(ao(&wuerfel(false)), "Blockmodell");
        assert!(!ao(&wuerfel(true)), "aus dem Blockentity");
    }

    /// Eine Fläche aus einer Schicht ohne Culling zeigt sich auch von
    /// hinten, mit `PER_FACE_LIGHTING` im Licht der umgekehrten Normalen,
    /// sonst im Licht ihrer Vorderseite. Mit Culling fehlt die Rückseite,
    /// bei Blockmodellen immer.
    #[test]
    fn rueckseiten_wie_im_spiel() {
        let modell = |quad| BakedModel {
            quads: vec![quad],
            ambient_occlusion: false,
        };
        let bild = |quad| {
            render(
                &modell(quad),
                &Textures::new(),
                &Projection::new(16),
                Tints::default(),
            )
        };
        let p = Projection::new(16);
        assert_eq!(seite(&unterseite(), &p), None, "Blockmodell");
        assert!(bild(unterseite()).is_none());
        let mit_culling = aus_entity(unterseite(), schicht(false, true));
        assert_eq!(seite(&mit_culling, &p), None);
        assert!(bild(mit_culling).is_none());

        let je_seite = aus_entity(unterseite(), schicht(true, true));
        assert_eq!(seite(&je_seite, &p), Some(true));
        let licht = CardinalLight::Default;
        assert_eq!(
            shade_factor(&je_seite, true, licht),
            1.0,
            "Licht der Oberseite"
        );
        let einseitig_beleuchtet = aus_entity(unterseite(), schicht(true, false));
        assert_eq!(shade_factor(&einseitig_beleuchtet, true, licht), 0.4);
        assert!(bild(je_seite).is_some());

        let oben = quad(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            None,
        );
        assert_eq!(
            seite(&aus_entity(oben, schicht(true, true)), &p),
            Some(false)
        );
    }

    /// Die Schwellen der Alpha-Tests (`color.a < ALPHA_CUTOUT` verwirft):
    /// 0,5 in `cutout_terrain`, 0,1 in `translucent_terrain` und in den
    /// ausgeschnittenen Schichten der Blockentities, keine in `entity_solid`,
    /// wo auch ein Texel ohne Alpha ganz deckt. Eine gemischte Schicht mit
    /// Schwelle behält, was sie stehen lässt, mit seinem Alpha.
    #[test]
    fn schwellen_wie_im_spiel() {
        assert_eq!(schwelle(Some(ALPHA_CUTOUT_CUTOUT)), 128);
        assert_eq!(schwelle(Some(ALPHA_CUTOUT_TRANSLUCENT)), 26);
        assert_eq!(schwelle(None), 0);

        let mut textures = Textures::new();
        let alphas = |textures: &mut Textures, alpha: u8, schicht: Schicht| {
            let textur = textures.einfuegen(
                &format!("alpha_{alpha}"),
                RgbaImage::from_pixel(16, 16, Rgba([200, 100, 50, alpha])),
                false,
            );
            let mut oben = aus_entity(
                quad(
                    [
                        [0.0, 1.0, 0.0],
                        [0.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0],
                        [1.0, 1.0, 0.0],
                    ],
                    None,
                ),
                schicht,
            );
            oben.texture = textur;
            let model = BakedModel {
                quads: vec![oben],
                ambient_occlusion: false,
            };
            let sprite = render(&model, textures, &Projection::new(32), Tints::default()).unwrap();
            sprite
                .image
                .pixels()
                .map(|p| p[3])
                .collect::<std::collections::BTreeSet<u8>>()
        };
        let cutout = Schicht {
            alpha: Some(0.1),
            ..schicht(false, false)
        };
        let gemischt = Schicht {
            gemischt: true,
            ..cutout
        };
        assert_eq!(alphas(&mut textures, 25, cutout), [0].into());
        assert_eq!(alphas(&mut textures, 26, cutout), [0, 255].into());
        assert_eq!(alphas(&mut textures, 25, gemischt), [0].into());
        assert_eq!(alphas(&mut textures, 26, gemischt), [0, 26].into());
        assert_eq!(
            alphas(&mut textures, 0, schicht(false, false)),
            [0, 255].into()
        );
    }

    fn quad(corners: [[f32; 3]; 4], shade: Option<Face>) -> Quad {
        Quad {
            corners,
            uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture: crate::assets::Textures::MISSING,
            tint_index: None,
            shade,
            force_translucent: false,
            fluid: None,
            entity: None,
            cullface: None,
        }
    }

    /// Eine Fläche genau in einer Würfelebene gehört dem Würfel dahinter,
    /// von der Kamera aus: eine Ostseite bei x = 0 dem westlichen, die
    /// Oberseite bei y = 1 dem eigenen. Eine Fläche quer durch eine Ebene
    /// teilt sich auf beide Würfel.
    #[test]
    fn flaeche_in_einer_wuerfelebene_gehoert_dem_wuerfel_dahinter() {
        use crate::assets::baker::box_quads;
        let zellen = |from: [f32; 3], to: [f32; 3], seite: fn([f32; 3]) -> bool| {
            let quads = box_quads(from, to, Textures::MISSING, None, None)
                .filter(|q| seite(q.normal()))
                .collect();
            let model = BakedModel {
                quads,
                ambient_occlusion: false,
            };
            let projection = Projection::new(16);
            let raster = rastern(
                &model,
                &Textures::new(),
                &projection,
                Tints::default(),
                CardinalLight::Default,
                false,
                false,
            )
            .unwrap();
            raster
                .teile(false)
                .into_iter()
                .map(|(zelle, _)| zelle)
                .collect::<Vec<_>>()
        };
        let osten = |n: [f32; 3]| n[0] > 0.0;
        let oben = |n: [f32; 3]| n[1] > 0.0;
        let sueden = |n: [f32; 3]| n[2] > 0.0;
        assert_eq!(
            zellen([-8.0, 0.0, 0.0], [0.0, 16.0, 16.0], osten),
            [[-1, 0, 0]]
        );
        assert_eq!(zellen([0.0; 3], [16.0; 3], oben), [[0, 0, 0]]);
        assert_eq!(
            zellen([8.0, 0.0, 0.0], [24.0, 16.0, 16.0], sueden),
            [[0, 0, 0], [1, 0, 0]]
        );
    }

    #[test]
    fn oberseite_ist_am_hellsten() {
        let oben = quad(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            None,
        );
        let (oberwelt, nether) = (CardinalLight::Default, CardinalLight::Nether);
        assert_eq!(shade_factor(&oben, false, oberwelt), 1.0);
        assert_eq!(shade_factor(&oben, false, nether), 0.9);

        let unten = quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            None,
        );
        assert_eq!(shade_factor(&unten, false, oberwelt), 0.5);
        assert_eq!(shade_factor(&unten, false, nether), 0.9);
    }

    #[test]
    fn seiten_werden_unterschiedlich_abgedunkelt() {
        let nord = quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            None,
        );
        for licht in [CardinalLight::Default, CardinalLight::Nether] {
            assert_eq!(shade_factor(&nord, false, licht), 0.8);
        }

        let ost = quad(
            [
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
            ],
            None,
        );
        for licht in [CardinalLight::Default, CardinalLight::Nether] {
            assert_eq!(shade_factor(&ost, false, licht), 0.6);
        }
    }

    /// Mit `shade` gilt die Seite der Welt, die es nennt, nicht die der
    /// Fläche (`BlockModelLighter.getDirectionalBrightness`): `shade: false`
    /// heisst oben und bleibt hell, die Stängel der Blumenbeete aus 26.3
    /// liegen nach Norden.
    #[test]
    fn shade_nennt_die_seite() {
        let nord = |shade| {
            quad(
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
                shade,
            )
        };
        assert_eq!(
            shade_factor(&nord(Some(Face::Up)), false, CardinalLight::Default),
            1.0
        );
        assert_eq!(
            shade_factor(&nord(Some(Face::Up)), false, CardinalLight::Nether),
            0.9
        );
        assert_eq!(
            shade_factor(&nord(Some(Face::Down)), false, CardinalLight::Default),
            0.5
        );
        let oben = quad(
            [
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            Some(Face::North),
        );
        assert_eq!(shade_factor(&oben, false, CardinalLight::Default), 0.8);
    }

    /// `FluidRenderer` schattiert die Oberseite mit `up()`, die Unterseite
    /// mit `down()` und die Seiten mit `up()` mal `north()` oder `west()`:
    /// im Nether 0,9 mal 0,8 und 0,9 mal 0,6, in der Oberwelt wie Blöcke.
    #[test]
    fn fluessigkeit_wie_fluid_renderer() {
        let wasser = |seite: Face| Quad {
            fluid: Some((fluid::Fluid::Water, seite)),
            ..quad(
                [
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
                None,
            )
        };
        let (oberwelt, nether) = (CardinalLight::Default, CardinalLight::Nether);
        for (seite, soll_oberwelt, soll_nether) in [
            (Face::Up, 1.0, 0.9),
            (Face::Down, 0.5, 0.9),
            (Face::North, 0.8, 0.9f32 * 0.8),
            (Face::South, 0.8, 0.9f32 * 0.8),
            (Face::West, 0.6, 0.9f32 * 0.6),
            (Face::East, 0.6, 0.9f32 * 0.6),
        ] {
            assert_eq!(
                shade_factor(&wasser(seite), false, oberwelt),
                soll_oberwelt,
                "{seite:?}"
            );
            assert_eq!(
                shade_factor(&wasser(seite), false, nether),
                soll_nether,
                "{seite:?}"
            );
        }
    }

    #[test]
    fn leeres_modell_ergibt_kein_sprite() {
        let leer = BakedModel::default();
        assert!(
            render(
                &leer,
                &Textures::new(),
                &Projection::default(),
                Tints::default(),
            )
            .is_none()
        );
    }

    /// Ein voller Würfel belegt genau scale mal scale Pixel und sitzt
    /// symmetrisch um den Blockursprung.
    #[test]
    fn wuerfel_fuellt_das_sprite() {
        // Oberseite, Südseite, Ostseite — mehr ist von dieser Kamera nicht
        // zu sehen.
        let quads = vec![
            quad(
                [
                    [0.0, 1.0, 0.0],
                    [0.0, 1.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [1.0, 1.0, 0.0],
                ],
                None,
            ),
            quad(
                [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                None,
            ),
            quad(
                [
                    [1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                ],
                None,
            ),
        ];

        let sprite = render(
            &BakedModel {
                quads,
                ambient_occlusion: false,
            },
            &Textures::new(),
            &Projection::new(16),
            Tints::default(),
        )
        .expect("Sprite");
        assert_eq!(sprite.image.dimensions(), (16, 16));
        assert_eq!(sprite.offset, (-8, -8));

        // Die Mitte ist gedeckt, die Ecken des Rechtecks nicht: ein
        // isometrischer Würfel ist ein Sechseck.
        assert_eq!(sprite.image.get_pixel(8, 8).0[3], 255);
        assert_eq!(sprite.image.get_pixel(0, 0).0[3], 0);
        assert_eq!(sprite.image.get_pixel(15, 0).0[3], 0);
    }

    /// Decken sich zwei Flächen eines Modells, trägt die Geometrie des
    /// Pixels die vordere: Ein kleiner Kasten steht auf einer Platte, und an
    /// seiner Südseite liegt die Oberseite der Platte dahinter. Dort stehen
    /// Tiefe und Normale der Südseite, daneben die der Oberseite. Die
    /// Flächen des Kastens haben kein `shade`, das trägt die Geometrie mit.
    #[test]
    fn geometrie_der_vorderen_flaeche() {
        let kasten =
            |from, to| crate::assets::baker::box_quads(from, to, Textures::MISSING, None, None);
        let model = BakedModel {
            quads: kasten([0.0; 3], [16.0, 8.0, 16.0])
                .chain(kasten([0.0, 8.0, 0.0], [8.0, 16.0, 8.0]).map(|q| Quad {
                    shade: Some(Face::Up),
                    ..q
                }))
                .collect(),
            ambient_occlusion: true,
        };
        let projection = Projection::new(16);
        let licht = CardinalLight::Default;
        let textures = Textures::new();
        let sprite = rastern(
            &model,
            &textures,
            &projection,
            Tints::default(),
            licht,
            true,
            true,
        )
        .unwrap()
        .ganz();
        let geometrie = sprite.geometrie.as_ref().unwrap();
        let (h, a, b) = (
            projection.h() as f32,
            projection.a() as f32,
            projection.b() as f32,
        );
        let am = |punkt: [f32; 3]| {
            let (sx, sy) = projection.project(punkt);
            let (x, y) = (
                sx.floor() as i32 - sprite.offset.0,
                sy.floor() as i32 - sprite.offset.1,
            );
            let g = geometrie[(y as u32 * sprite.image.width() + x as u32) as usize];
            (g, (sx.floor() + 0.5, sy.floor() + 0.5))
        };
        // Auf der Südseite des Kastens, z = 0,5: x aus u = x − z, y aus v.
        let (g, (sx, sy)) = am([0.4, 0.6, 0.5]);
        let x = sx / h + 0.5;
        let y = ((x + 0.5) * a - sy) / b;
        assert_eq!(
            (g.normale, g.shade),
            ([0.0, 0.0, 1.0], Some([0.0, 1.0, 0.0]))
        );
        assert!(
            (g.tiefe - projection.depth([x, y, 0.5])).abs() < 1e-4,
            "{g:?}"
        );
        // Auf der Oberseite der Platte, y = 0,5.
        let (g, (sx, sy)) = am([0.75, 0.5, 0.75]);
        let (u, v) = (sx / h, (sy + 0.5 * b) / a);
        assert_eq!((g.normale, g.shade), ([0.0, 1.0, 0.0], None));
        let soll = projection.depth([(v + u) / 2.0, 0.5, (v - u) / 2.0]);
        assert!((g.tiefe - soll).abs() < 1e-4, "{g:?}");
    }

    /// Cinematic rastert dieselben Pixel wie die Karte, ohne Schattierung
    /// nach Richtung: Die Karte zeigt jede Seite in der Farbe der Textur mal
    /// dem Faktor ihrer Richtung, Cinematic die Farbe selbst. Je Pixel steht
    /// die Normale seiner Seite, auf der Oberseite die Tiefe ihrer Ebene an
    /// der Mitte des Pixels.
    #[test]
    fn cinematic_ohne_schattierung_mit_geometrie() {
        let model = BakedModel {
            quads: crate::assets::baker::box_quads(
                [0.0; 3],
                [16.0; 3],
                Textures::MISSING,
                None,
                None,
            )
            .collect(),
            ambient_occlusion: true,
        };
        let textures = Textures::new();
        let projection = Projection::new(16);
        let raster = |kino| {
            let licht = CardinalLight::Default;
            rastern(
                &model,
                &textures,
                &projection,
                Tints::default(),
                licht,
                true,
                kino,
            )
            .unwrap()
            .ganz()
        };
        let (karte, kino) = (raster(false), raster(true));
        assert!(karte.geometrie.is_none());
        let geometrie = kino.geometrie.as_ref().expect("Geometrie");
        assert_eq!(
            (karte.offset, karte.image.dimensions(), &karte.ao),
            (kino.offset, kino.image.dimensions(), &kino.ao)
        );
        let mut oben = 0;
        let pixel = karte.image.enumerate_pixels().zip(kino.image.pixels());
        for (((x, y, k), c), g) in pixel.zip(geometrie) {
            assert_eq!(k.0[3], c.0[3], "({x}, {y})");
            if c.0[3] == 0 {
                continue;
            }
            let faktor = if g.normale == [0.0, 1.0, 0.0] {
                1.0
            } else if g.normale == [0.0, 0.0, 1.0] {
                0.8
            } else if g.normale == [1.0, 0.0, 0.0] {
                0.6
            } else {
                panic!("({x}, {y}): Normale {:?}", g.normale);
            };
            assert_eq!(k.0, shaded(c.0, faktor, None), "({x}, {y})");
            if faktor == 1.0 {
                oben += 1;
                // Auf der Oberseite, y = 1, aus der Mitte des Pixels:
                // u = x − z, v = x + z.
                let sx = (kino.offset.0 + x as i32) as f32 + 0.5;
                let sy = (kino.offset.1 + y as i32) as f32 + 0.5;
                let u = sx / projection.h() as f32;
                let v = (sy + projection.b() as f32) / projection.a() as f32;
                let erwartet = projection.depth([(v + u) / 2.0, 1.0, (v - u) / 2.0]);
                assert!(
                    (g.tiefe - erwartet).abs() < 1e-4,
                    "({x}, {y}): Tiefe {} statt {erwartet}",
                    g.tiefe
                );
            }
        }
        assert!(oben > 0);
    }
}
