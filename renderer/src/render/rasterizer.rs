use std::sync::LazyLock;

use image::{Rgba, RgbaImage};

use crate::assets::baker::{BakedModel, Quad};
use crate::assets::blockstate::Leuchten;
use crate::assets::{Face, Textures, Tints, fluid};

use super::Projection;
use super::pyramid::{LINEAR, to_srgb};

/// Abtastpunkte je Pixelkante für die Textur.
///
/// Die Geometrie wird nur im Pixelmittelpunkt geprüft: jeder Pixel gehört
/// genau einer Fläche, und benachbarte Flächen stossen nahtlos aneinander.
/// Geglättete Kanten trügen Teildeckung im Alpha, und beim Zusammensetzen
/// der Sprites könnte niemand mehr unterscheiden, ob zwei Nachbarflächen
/// dasselbe Pixel teilen oder ob eine durch die andere scheint — ein
/// geschlossenes Wasserbecken bekäme an jeder Blockgrenze eine hellere
/// Naht, ein Boden aus deckenden Blöcken dunkle Linien. Die Textur dagegen
/// wird über den Pixel gemittelt, und zwar so dicht, dass jeder Texel
/// erfasst wird: eine Seitenfläche ist `scale / 2` Pixel breit für
/// sechzehn Texel, also liegen `32 / scale` Texel unter jedem Pixel.
/// Mindestens zwei Abtastpunkte, damit auch bei scale 32 die Mitte
/// zwischen zwei Texeln stimmt; höchstens sechzehn, mehr Texel hat eine
/// Textur nicht.
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
/// Blickrichtung und hat im Bild keine Fläche. Der Baker dreht in f32, und
/// eine solche Fläche behält eine Normale, deren Summe um 1e-7 ihrer Länge
/// neben null liegt, mal davor, mal dahinter. Davor legte sie einen
/// Streifen von 2e-7 Pixeln Breite auf die Kante ihres Nachbarn, und ein
/// Pixelmittelpunkt genau darauf bekam beide. Echte Drehungen liegen weit
/// darüber: um eine Achse in Schritten von 22,5 Grad, dazu Vielfache von
/// 90, ist die kleinste Summe ungleich null 0,54 der Länge.
const EDGE_ON: f32 = 1e-4;

/// Helligkeit je Flächenrichtung, wie Minecraft sie verwendet. Ohne diese
/// Abstufung sieht ein isometrischer Würfel flach aus.
const SHADE_TOP: f32 = 1.0;
const SHADE_BOTTOM: f32 = 0.5;
const SHADE_NORTH_SOUTH: f32 = 0.8;
const SHADE_EAST_WEST: f32 = 0.6;

/// Volles Himmelslicht, am Tag unter freiem Himmel. So hell zeichnet der
/// Renderer jede Fläche, siehe [`brightness`].
pub const FULL_LIGHT: u8 = 15;

/// Himmelslicht direkt unter einer Wasseroberfläche: Der Block der
/// Oberfläche selbst nimmt eine Stufe, `LiquidBlock.propagatesSkylightDown`
/// ist falsch.
pub const LIGHT_UNDER_SURFACE: u8 = 14;

/// Helligkeit einer Fläche im Himmelslicht `light` (0 bis 15), am Tag in
/// der Oberwelt, so wie `shaders/core/lightmap.fsh` in 26.2 sie rechnet:
/// `get_brightness(l / 15) = l / (4 - 3 l)` mal `SkyFactor` 1 und die
/// weisse `SkyLightColor`, dazu die `ambient_light_color` `#0a0a0a` der
/// Oberwelt, auf 1 begrenzt. Danach steht sie halb zwischen diesem Wert und
/// `notGamma`, denn `options.gamma` ist im Spiel 0,5. Licht 15 gibt 1, also
/// so hell, wie der Renderer jede Fläche zeichnet.
pub fn brightness(light: u8) -> f32 {
    let level = light.min(FULL_LIGHT) as f32 / 15.0;
    let sky = level / (4.0 - 3.0 * level);
    let color = (10.0 / 255.0 + sky).min(1.0);
    let rest = 1.0 - color;
    let not_gamma = 1.0 - rest * rest * rest * rest;
    color + (not_gamma - color) * 0.5
}

/// [`brightness`] in 255steln, für [`darken`]: 255 bei vollem Licht.
pub fn light_factor(light: u8) -> u32 {
    (brightness(light) * 255.0).round() as u32
}

/// `BlockFactor` aus `LightmapRenderStateExtractor.extract`: 1,4 und ein
/// Flackern, das `tick` zufällig um 0 laufen lässt. Hier ohne Flackern.
const BLOCK_FACTOR: f32 = 1.4;

/// `visual/block_light_tint` der Oberwelt: der Standard `#FFD88C` aus
/// `EnvironmentAttributes`, denn `overworld.json` setzt keinen.
const BLOCK_LIGHT_TINT: [f32; 3] = [1.0, 216.0 / 255.0, 140.0 / 255.0];

/// Helligkeit je Farbkanal im Himmelslicht `sky` und im Blocklicht
/// `block`, wie `lightmap.fsh` sie rechnet: zum Himmelslicht aus
/// [`brightness`] kommt `get_brightness(block / 15)` mal [`BLOCK_FACTOR`]
/// in der Farbe [`BLOCK_LIGHT_TINT`], die zur vollen Stufe hin fast weiss
/// wird (`mix` mit `0,9 · (2 l − 1)²`). `notGamma` hebt alle Kanäle mit dem
/// hellsten. Ohne Blocklicht ist das [`brightness`] in jedem Kanal.
pub fn brightness_rgb(sky: u8, block: u8) -> [f32; 3] {
    if block == 0 {
        return [brightness(sky); 3];
    }
    let level = |l: u8| l.min(FULL_LIGHT) as f32 / 15.0;
    let get_brightness = |l: f32| l / (4.0 - 3.0 * l);
    let b = level(block);
    let (sky_brightness, block_brightness) =
        (get_brightness(level(sky)), get_brightness(b) * BLOCK_FACTOR);
    let mix = 0.9 * (2.0 * b - 1.0) * (2.0 * b - 1.0);
    let color = BLOCK_LIGHT_TINT.map(|tint| {
        let block_color = tint + (1.0 - tint) * mix;
        (10.0 / 255.0 + sky_brightness + block_color * block_brightness).min(1.0)
    });
    let max = color.iter().fold(0.0f32, |a, &c| a.max(c));
    let rest = 1.0 - max;
    let scaled = 1.0 - rest * rest * rest * rest;
    color.map(|c| c + (c * (scaled / max) - c) * 0.5)
}

/// In welchem Licht das Spiel einen Block zeichnet: Himmels- und
/// Blocklicht, je 0 bis 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Light {
    pub sky: u8,
    pub block: u8,
}

impl Light {
    /// Voller Tag unter freiem Himmel: so hell zeichnet der Renderer jedes
    /// Sprite.
    pub const FULL: Light = Light {
        sky: FULL_LIGHT,
        block: 0,
    };

    pub fn sky(sky: u8) -> Light {
        Light { sky, block: 0 }
    }

    /// [`brightness_rgb`] in 255steln, für [`darken`].
    pub fn factors(self) -> [u32; 3] {
        static FAKTOREN: LazyLock<[[[u32; 3]; 16]; 16]> = LazyLock::new(|| {
            std::array::from_fn(|sky| {
                std::array::from_fn(|block| {
                    brightness_rgb(sky as u8, block as u8).map(|c| (c * 255.0).round() as u32)
                })
            })
        });
        FAKTOREN[self.sky.min(FULL_LIGHT) as usize][self.block.min(FULL_LIGHT) as usize]
    }
}

/// Ein Pixel im Licht mit den Faktoren aus [`Light::factors`]: jeder
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

/// Die Seiten, die die Kamera sieht, in der Reihenfolge der Nummern in
/// [`Sprite::ao`]: Seite `i` hat dort die Nummer `i + 1`.
pub const AO_FACES: [Face; 3] = [Face::Up, Face::South, Face::East];

/// Die Ecken einer Seite aus [`AO_FACES`] in der Reihenfolge von `FaceInfo`
/// in 26.2, in den beiden Koordinaten der Seite: oben `(x, z)`, Süden
/// `(x, y)`, Osten `(z, y)`. `FaceBakery.recalculateWinding` bringt jedes
/// gebackene Viereck in diese Reihenfolge, und das Spiel zeichnet es als
/// die Dreiecke 0-1-2 und 2-3-0 (`RenderSystem.sharedSequentialQuad`).
const FACE_INFO: [[[f32; 2]; 4]; 3] = [
    [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]],
];

/// Die Seite aus [`AO_FACES`], als die das Spiel ein Viereck mit den vier
/// Werten je Ecke weich beleuchtet: eben auf dem Rand des Würfels
/// (`faceCubic` in `BlockModelLighter.prepareQuadShape`) und über die ganze
/// Seite (nicht `facePartial`, bis auf 1e-4). Flüssigkeiten zeichnet das
/// Spiel ohne weiche Beleuchtung. Eben heisst hier bis auf 1e-4: Der Baker
/// dreht über sin und cos und trifft die Ebene nur fast.
fn ao_face(quad: &Quad) -> Option<usize> {
    if quad.fluid.is_some() {
        return None;
    }
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
    let voll = |axis: usize| min(axis) < 1e-4 && max(axis) > 0.9999;
    let rand = |axis: usize| max(axis) - min(axis) < 1e-4 && min(axis) > 0.9999;
    let n = quad.normal();
    // Je Seite die Achse ihrer Normalen und die beiden in der Seite.
    let (face, _, [s, t]) = [(0, 1, [0, 2]), (1, 2, [0, 1]), (2, 0, [2, 1])]
        .into_iter()
        .find(|&(_, axis, _)| n[axis] > 0.0 && rand(axis))?;
    (voll(s) && voll(t)).then_some(face)
}

/// Die Koordinaten eines Punkts auf einer Seite aus [`AO_FACES`], wie in
/// [`FACE_INFO`].
fn face_coords(face: usize, [x, y, z]: [f32; 3]) -> [f32; 2] {
    match face {
        0 => [x, z],
        1 => [x, y],
        _ => [z, y],
    }
}

/// Die Anteile der vier Ecken aus [`FACE_INFO`] an einem Punkt der Seite,
/// in 255steln und zusammen genau 255: baryzentrisch in dem der beiden
/// Dreiecke des Spiels, in dem der Punkt liegt. So verläuft die
/// Helligkeit der Ecken im Spiel über die Fläche.
fn corner_weights(face: usize, p: [f32; 2]) -> [u32; 4] {
    let e = FACE_INFO[face];
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
/// fehlt auf 255, und die Nummer der Seite aus [`AO_FACES`].
fn ao_word(face: usize, w: [u32; 4]) -> u32 {
    w[0] | w[1] << 8 | w[2] << 16 | (face as u32 + 1) << 24
}

/// Die Helligkeit der weichen Beleuchtung an einem Pixel in 255steln:
/// der Eintrag der AO-Karte gegen die Werte der Ecken seiner Seite, wie
/// [`ChunkCache::ao_at`](super::metatile) sie je Block liefert, vier Bytes
/// je Seite in der Reihenfolge von [`FACE_INFO`]. 255 ohne Seite. Dieselbe
/// Rechnung steht im Shader.
pub fn ao_factor(word: u32, corners: [u32; 3]) -> u32 {
    let face = word >> 24;
    if face == 0 {
        return 255;
    }
    let c = corners[face as usize - 1];
    let [w0, w1, w2] = [word & 255, word >> 8 & 255, word >> 16 & 255];
    let w3 = 255 - w0 - w1 - w2;
    (w0 * (c & 255) + w1 * (c >> 8 & 255) + w2 * (c >> 16 & 255) + w3 * (c >> 24) + 127) / 255
}

/// Die Werte der Ecken für einen Block ohne weiche Beleuchtung: überall
/// 255, jeder Pixel bleibt, wie er ist.
pub const NO_AO: [u32; 3] = [u32::MAX; 3];

/// Licht aus [`light_factor`] und weiche Beleuchtung aus [`ao_factor`]
/// zusammen als Faktor für [`darken`]. Bei vollem Licht bleibt der Wert der
/// weichen Beleuchtung, wie er ist.
pub fn with_ao(factor: u32, ao: u32) -> u32 {
    (factor * ao + 127) / 255
}

/// Das fertig gerasterte Bild einer Blockstate.
pub struct Sprite {
    pub image: RgbaImage,
    /// Pixelposition der linken oberen Ecke, relativ zum projizierten
    /// Blockursprung.
    pub offset: (i32, i32),
    /// Je Pixel, wie die weiche Beleuchtung des Spiels ihn abdunkelt, siehe
    /// [`ao_factor`]; nur für Modelle, die das Spiel weich beleuchtet und
    /// die nur aus vollen Seiten bestehen.
    pub ao: Option<Vec<u32>>,
}

/// Rastert ein gebackenes Modell in ein Sprite.
///
/// Da die Kamera fest steht, sieht jede Blockstate immer gleich aus. Das
/// Sprite entsteht deshalb einmal und wird im Renderpfad nur noch kopiert.
/// `leuchten` sagt, wie hell der Block selbst leuchtet: Was er unter
/// seiner eigenen Wasseroberfläche trägt, liegt im Licht direkt unter ihr
/// und in seinem eigenen Blocklicht, siehe `Canvas::into_image`.
pub fn render(
    model: &BakedModel,
    textures: &Textures,
    projection: &Projection,
    tints: Tints,
    leuchten: Leuchten,
) -> Option<Sprite> {
    let mut projected: Vec<ProjectedQuad> = model
        .quads
        .iter()
        .filter(|quad| faces_camera(quad))
        .map(|quad| ProjectedQuad::new(quad, projection))
        .collect();

    // Jede Fläche legt je Pixel ein Fragment ab, gemischt wird erst am
    // Schluss: je Pixel von hinten nach vorne, nach der Tiefe an genau
    // diesem Pixel. Ein durchsichtiges Texel liegt so immer über dem, was
    // dahinter liegt — Wasser über einem Zaunpfosten, Glas über dem Block
    // dahinter —, egal in welcher Reihenfolge die Flächen kommen. Und eine
    // Fläche, deren Textur am Pixel nur zum Teil deckt, weil sie über den
    // Pixel gemittelt ist (die Kante eines Weizenhalms), verdeckt die
    // Fläche dahinter nicht mehr ganz.
    //
    // Die Reihenfolge der Flächen zählt nur noch bei gleicher Tiefe: dann
    // gewinnt die spätere. Sortiert wird deshalb stabil nach der
    // vordersten Ecke — deckungsgleiche Flächen behalten ihre
    // Modellreihenfolge, und der Grasblock legt sein Overlay so auf den
    // Grundwürfel.
    projected.sort_by(|a, b| a.depth.total_cmp(&b.depth));

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

    // Weich beleuchtet wird hier nur, was ganz aus vollen Seiten besteht:
    // Dann gehört jeder Pixel genau einer Seite, und die Werte ihrer Ecken
    // reichen. Treppen, Platten und alles mit Teilflächen fehlen noch.
    let ao = model.ambient_occlusion && projected.iter().all(|q| q.ao_face.is_some());
    if !ao {
        for quad in &mut projected {
            quad.ao_face = None;
        }
    }

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

    let unter = match leuchten {
        Leuchten::Voll => Light {
            sky: FULL_LIGHT,
            block: FULL_LIGHT,
        },
        Leuchten::Stufe(block) => Light {
            sky: LIGHT_UNDER_SURFACE,
            block,
        },
    };
    let (image, ao) = canvas.into_image(unter, ao);
    Some(Sprite {
        image,
        offset: (min_x, min_y),
        ao,
    })
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
    /// Als welche Seite das Viereck weich beleuchtet wird, siehe [`ao_face`].
    ao_face: Option<usize>,
}

impl<'a> ProjectedQuad<'a> {
    fn new(quad: &'a Quad, projection: &Projection) -> ProjectedQuad<'a> {
        let screen = quad.corners.map(|corner| {
            let (x, y) = projection.project(corner);
            (x, y, Projection::depth(corner))
        });
        ProjectedQuad {
            quad,
            screen,
            depth: screen.iter().map(|&(_, _, d)| d).fold(f32::MIN, f32::max),
            shade: shade_factor(quad),
            ao_face: ao_face(quad),
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
        // Blockinnere (`FluidRenderer`); hier rückt sie stattdessen
        // in der Tiefe nach hinten. Die Seiten eines gefluteten Blocks
        // liegen genau auf der Blockgrenze, wo auch der Wasserwürfel
        // endet, und bei gleicher Tiefe gewann das Wasser: ein Film auf
        // jeder gefluteten Platte, Treppe und Falltür.
        let behind = if self.quad.fluid.is_some() {
            FLUID_BEHIND
        } else {
            0.0
        };
        let ao_face = self.ao_face;
        let vertices: [Vertex; 4] = std::array::from_fn(|i| {
            let (x, y, depth) = self.screen[i];
            let [s, t] = ao_face.map_or([0.0; 2], |f| face_coords(f, self.quad.corners[i]));
            Vertex {
                x: x - min_x as f32,
                y: y - min_y as f32,
                depth: depth - behind,
                u: self.quad.uvs[i][0],
                v: self.quad.uvs[i][1],
                s,
                t,
            }
        });
        // Die Texturmittelung tastet knapp neben dem Pixelmittelpunkt ab,
        // am Rand also knapp ausserhalb der Fläche. Dort bleibt sie im
        // Ausschnitt, den die Fläche aus der Textur nimmt — eine Tür soll
        // nicht ihre Rückseite an die Kante mischen.
        let mut bounds = [[f32::MAX, f32::MAX], [f32::MIN, f32::MIN]];
        for [u, v] in self.quad.uvs {
            bounds[0] = [bounds[0][0].min(u), bounds[0][1].min(v)];
            bounds[1] = [bounds[1][0].max(u), bounds[1][1].max(v)];
        }

        let tint = match self.quad.tint_index {
            None => None,
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
                    surface: self.quad.fluid.is_some_and(|(_, face)| face == Face::Up),
                    order,
                    ao_face,
                },
                samples,
            );
        }
    }
}

/// True, wenn die Fläche der Kamera zugewandt ist.
///
/// Die Kamera blickt entlang (-1, -1, -1); eine Fläche ist also sichtbar,
/// wenn ihre Normale eine Komponente in Richtung (1, 1, 1) hat. Ohne diese
/// Prüfung gewinnen abgewandte Flächen den Tiefentest, wenn sie mit einer
/// sichtbaren zusammenfallen — beim Seerosenblatt liegen `down` und `up` in
/// derselben Ebene. Eine Fläche parallel zur Blickrichtung zählt nicht,
/// siehe `EDGE_ON`.
pub(crate) fn faces_camera(quad: &Quad) -> bool {
    let n = quad.normal();
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n[0] + n[1] + n[2] > EDGE_ON * length
}

/// Helligkeit nach der Richtung, in die die Fläche am stärksten zeigt.
fn shade_factor(quad: &Quad) -> f32 {
    if !quad.shade {
        return SHADE_TOP;
    }
    let n = quad.normal();
    let [ax, ay, az] = [n[0].abs(), n[1].abs(), n[2].abs()];
    if ay >= ax && ay >= az {
        if n[1] >= 0.0 { SHADE_TOP } else { SHADE_BOTTOM }
    } else if az >= ax {
        SHADE_NORTH_SOUTH
    } else {
        SHADE_EAST_WEST
    }
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
}

/// Wie ein Texel zur Farbe wird: Helligkeit der Fläche, Färbung, ob sie
/// die Oberseite einer Flüssigkeit ist, als welche Seite sie weich
/// beleuchtet wird — und der Rang der Fläche, der bei gleicher Tiefe
/// entscheidet.
#[derive(Clone, Copy)]
struct Shading {
    shade: f32,
    tint: Option<[f32; 3]>,
    surface: bool,
    order: u32,
    ao_face: Option<usize>,
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
    /// Von der Oberseite einer Flüssigkeit: Was im Sprite dahinter liegt,
    /// liegt unter Wasser.
    surface: bool,
    /// Eintrag der AO-Karte, 0 ohne weiche Beleuchtung.
    ao: u32,
}

/// Die Fragmente eines Sprites, gemischt erst in `into_image`.
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
    /// dem Dreieck, für das die Kante oben oder links liegt. Zwei Dreiecke
    /// mit gemeinsamer Kante — die Hälften einer Fläche, zwei Flächen eines
    /// Würfels — bekommen ihn so genau einmal. Ohne die Regel mischte ein
    /// durchsichtiges Texel auf einer solchen Kante doppelt.
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
            surface,
            order,
            ao_face,
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
                let Some(texel) = filtered(&v, area, px, py, sample, inside, samples) else {
                    continue;
                };
                if texel[3] == 255 {
                    self.front[index] = self.front[index].max(depth);
                }
                let ao = ao_face.map_or(0, |face| {
                    let s = w[0] * v[0].s + w[1] * v[1].s + w[2] * v[2].s;
                    let t = w[0] * v[0].t + w[1] * v[1].t + w[2] * v[2].t;
                    ao_word(face, corner_weights(face, [s, t]))
                });
                self.fragments.push(Fragment {
                    pixel: index as u32,
                    depth,
                    order,
                    color: shaded(texel, shade, tint),
                    surface,
                    ao,
                });
            }
        }
    }

    /// Mischt je Pixel die Fragmente von hinten nach vorne. Was unter der
    /// eigenen Oberfläche liegt, im Licht `unter`. Mit `ao` dazu die
    /// AO-Karte aus dem vordersten Fragment je Pixel.
    fn into_image(mut self, unter: Light, ao: bool) -> (RgbaImage, Option<Vec<u32>>) {
        let unter = unter.factors();
        self.fragments.sort_unstable_by(|a, b| {
            a.pixel
                .cmp(&b.pixel)
                .then(a.depth.total_cmp(&b.depth))
                .then(a.order.cmp(&b.order))
        });
        let mut image = RgbaImage::new(self.width, self.height);
        let mut map = ao.then(|| vec![0u32; (self.width * self.height) as usize]);
        for pixel in self.fragments.chunk_by(|a, b| a.pixel == b.pixel) {
            let mut color = [0u8; 4];
            for fragment in pixel {
                // Was ein gefluteter Block unter seiner eigenen Oberfläche
                // trägt, ein Zaunpfosten etwa, liegt im Licht direkt unter
                // ihr, eine Laterne oder Meeresgurke dazu in ihrem eigenen
                // Blocklicht. Wo das Sprite nichts dahinter hat, bleibt die
                // Oberfläche, wie sie ist: Was dort durchscheint, zeichnet
                // der Renderlauf in seinem eigenen Licht.
                if fragment.surface && color[3] != 0 {
                    color = darken(color, unter);
                }
                color = over(fragment.color, color);
            }
            let index = pixel[0].pixel;
            image.put_pixel(index % self.width, index / self.width, Rgba(color));
            if let Some(map) = &mut map {
                map[index as usize] = pixel[pixel.len() - 1].ao;
            }
        }
        (image, map)
    }
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
// aber die Rundung der uv. Gemessen kostet der ganze Sprite-Bau der
// Testwelt über alle vier Stufen rund 11 s, der Vollrender rund 11 min.
/// Mittelwert der Texel unter einem Pixel, mit vormultipliziertem Alpha —
/// sonst zögen durchsichtige Texel ihre Farbe in die Nachbarn — und in
/// linearem Licht wie die Pyramide: das Mittel von sRGB-Werten ist zu
/// dunkel, halb Schwarz und halb Weiss gäbe 128 statt 188. Gezählt werden
/// nur Abtastpunkte innerhalb der Fläche; liegt keiner darin, weil die
/// Fläche schmaler ist als ein Pixel, gilt der Mittelpunkt. `None`, wenn
/// kein Texel deckt.
fn filtered(
    v: &[Vertex; 3],
    area: f32,
    px: f32,
    py: f32,
    sample: &impl Fn(f32, f32) -> [u8; 4],
    inside: &impl Fn(f32, f32) -> bool,
    n: u32,
) -> Option<[u8; 4]> {
    // Summe der vormultiplizierten Farben in linearem Licht, Summe der
    // Alphas, Anzahl — und ob alle Abtastpunkte dasselbe Texel trafen.
    // Bei scale 32 ist das je nach Textur bei einem Zehntel bis gut einem
    // Drittel der Pixel so, und dann ist das Texel selbst das Mittel, ohne
    // Umweg über lineares Licht.
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
    for sy in 0..n {
        for sx in 0..n {
            let dx = (sx as f32 + 0.5) / n as f32 - 0.5;
            let dy = (sy as f32 + 0.5) / n as f32 - 0.5;
            let (u, vv) = uv(px + dx, py + dy);
            if inside(u, vv) {
                add(&mut acc, sample(u, vv));
            }
        }
    }
    if acc.2 == 0 {
        let (u, vv) = uv(px, py);
        add(&mut acc, sample(u, vv));
    }
    let (sum, alpha, count) = acc;
    if alpha <= 0.0 {
        return None;
    }
    if let Some(Some(texel)) = einzig {
        return Some(texel);
    }
    Some([
        to_srgb(sum[0] / alpha),
        to_srgb(sum[1] / alpha),
        to_srgb(sum[2] / alpha),
        (alpha / count as f32 * 255.0).round() as u8,
    ])
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
/// Rechnung steht im Shader (`gpu.wgsl`). Gleitkomma würde dort je
/// Grafikkarte anders runden; so liefert jede Karte dasselbe Byte wie die
/// CPU. Gegenüber der Gleitkommafassung weicht das Ergebnis höchstens um
/// 1 ab, und nur dort, wo Gleitkomma selbst daneben lag.
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
        assert_eq!(brightness_rgb(0, 15), [1.0; 3]);
        assert_eq!(brightness_rgb(15, 15), [1.0; 3]);
        let warm = brightness_rgb(5, 6);
        for (ist, soll) in warm.iter().zip([0.58609, 0.53676, 0.44063]) {
            assert!((ist - soll).abs() < 1e-4, "{warm:?}");
        }
        assert_eq!(brightness_rgb(7, 0), [brightness(7); 3]);
        assert_eq!(Light::sky(14).factors(), [light_factor(14); 3]);
        assert_eq!(Light { sky: 0, block: 15 }.factors(), [255; 3]);
    }

    /// Ein voller Würfel, den das Spiel weich beleuchtet, bekommt eine
    /// AO-Karte: Jeder Pixel mit Farbe liegt auf einer der drei Seiten, und
    /// alle drei kommen vor. Ohne `ambientocclusion`, mit einer Oberseite in
    /// halber Höhe und als obere Platte, deren Oberseite voll ist, ihre
    /// Seiten aber nicht, gibt es keine.
    #[test]
    fn ao_karte_nur_fuer_volle_wuerfel() {
        let kasten = |from: [f32; 3], to: [f32; 3], ambient_occlusion| BakedModel {
            quads: crate::assets::baker::box_quads(from, to, Textures::MISSING, None, None)
                .collect(),
            ambient_occlusion,
        };
        let wuerfel = |to: [f32; 3], ambient_occlusion| kasten([0.0; 3], to, ambient_occlusion);
        let textures = Textures::new();
        let projection = Projection::new(32);
        let bild = |model: &BakedModel| {
            render(
                model,
                &textures,
                &projection,
                Tints::default(),
                Leuchten::Stufe(0),
            )
        };
        let sprite = bild(&wuerfel([16.0; 3], true)).unwrap();
        let karte = sprite.ao.as_ref().expect("AO-Karte");
        for (i, p) in sprite.image.pixels().enumerate() {
            assert_eq!(p.0[3] != 0, karte[i] >> 24 != 0, "Pixel {i}");
        }
        let seiten: std::collections::BTreeSet<u32> = karte.iter().map(|w| w >> 24).collect();
        assert_eq!(seiten, [0, 1, 2, 3].into());
        assert!(bild(&wuerfel([16.0; 3], false)).unwrap().ao.is_none());
        assert!(
            bild(&wuerfel([16.0, 8.0, 16.0], true))
                .unwrap()
                .ao
                .is_none()
        );
        assert!(
            bild(&kasten([0.0, 8.0, 0.0], [16.0; 3], true))
                .unwrap()
                .ao
                .is_none(),
            "obere Platte"
        );
    }

    /// Die Anteile der Ecken an einem Punkt der Oberseite: an einer Ecke nur
    /// sie, sonst baryzentrisch im Dreieck 0-1-2 oder 2-3-0, zusammen immer
    /// 255. Mit den Ecken einer Innenecke liegt die Mitte zwischen der
    /// dunklen und der hellen Ecke.
    #[test]
    fn anteile_der_ecken() {
        assert_eq!(corner_weights(0, [0.0, 0.0]), [255, 0, 0, 0]);
        assert_eq!(corner_weights(0, [0.0, 1.0]), [0, 255, 0, 0]);
        assert_eq!(corner_weights(0, [1.0, 1.0]), [0, 0, 255, 0]);
        assert_eq!(corner_weights(0, [1.0, 0.0]), [0, 0, 0, 255]);
        assert_eq!(corner_weights(0, [0.25, 0.75]), [64, 127, 64, 0]);
        assert_eq!(corner_weights(0, [0.75, 0.25]), [64, 0, 64, 127]);
        for face in 0..3 {
            for i in 0..=10 {
                for j in 0..=10 {
                    let w = corner_weights(face, [i as f32 / 10.0, j as f32 / 10.0]);
                    assert_eq!(w.iter().sum::<u32>(), 255, "Seite {face}, {i}, {j}");
                }
            }
        }
        let innenecke = [u32::from_le_bytes([102, 153, 255, 153]), NO_AO[1], NO_AO[2]];
        let an = |p| ao_factor(ao_word(0, corner_weights(0, p)), innenecke);
        assert_eq!(an([0.0, 0.0]), 102);
        assert_eq!(an([0.5, 0.5]), 178);
        assert_eq!(an([1.0, 1.0]), 255);
        assert_eq!(ao_factor(0, innenecke), 255, "Pixel ohne Seite");
        assert_eq!(with_ao(255, 178), 178);
        assert_eq!(with_ao(light_factor(0), 255), light_factor(0));
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
            true,
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
                Leuchten::Stufe(0),
            )
            .is_some()
        );
    }

    fn quad(corners: [[f32; 3]; 4], shade: bool) -> Quad {
        Quad {
            corners,
            uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture: crate::assets::Textures::MISSING,
            tint_index: None,
            shade,
            force_translucent: false,
            fluid: None,
        }
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
            true,
        );
        assert_eq!(shade_factor(&oben), SHADE_TOP);

        let unten = quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            true,
        );
        assert_eq!(shade_factor(&unten), SHADE_BOTTOM);
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
            true,
        );
        assert_eq!(shade_factor(&nord), SHADE_NORTH_SOUTH);

        let ost = quad(
            [
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
            ],
            true,
        );
        assert_eq!(shade_factor(&ost), SHADE_EAST_WEST);
    }

    #[test]
    fn shade_false_bleibt_hell() {
        let nord = quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            false,
        );
        assert_eq!(shade_factor(&nord), SHADE_TOP);
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
                Leuchten::Stufe(0),
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
                true,
            ),
            quad(
                [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                true,
            ),
            quad(
                [
                    [1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                ],
                true,
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
            Leuchten::Stufe(0),
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
}
