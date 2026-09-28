use std::sync::LazyLock;

use image::{Rgba, RgbaImage};

use crate::assets::baker::{BakedModel, Quad};
use crate::assets::blockentity::Entity;
use crate::assets::blockstate::Leuchten;
use crate::assets::{Face, Textures, Tints, fluid};

use super::Projection;
use super::pyramid::{LINEAR, to_srgb};

/// Abtastpunkte je Pixelkante für die Textur. Die Geometrie wird nur im
/// Pixelmittelpunkt geprüft, die Textur über den Pixel gemittelt, so
/// dicht, dass jeder Texel zählt: eine Seitenfläche ist `scale / 2` Pixel
/// breit für sechzehn Texel, also liegen `32 / scale` Texel unter jedem
/// Pixel. Mindestens zwei, damit auch bei scale 32 die Mitte zwischen zwei
/// Texeln stimmt; höchstens sechzehn, mehr Texel hat eine Textur nicht.
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
/// der Oberwelt, so wie `shaders/core/lightmap.fsh` in 26.2 sie rechnet.
/// Licht 15 gibt 1, also so hell, wie der Renderer jede Fläche zeichnet.
/// Siehe docs/renderer/wasser-und-licht.md, „Helligkeit wie im Spiel“.
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
/// [`brightness`] kommt das Blocklicht mit [`BLOCK_FACTOR`] in der Farbe
/// [`BLOCK_LIGHT_TINT`]. Ohne Blocklicht ist das [`brightness`] in jedem
/// Kanal.
/// Siehe docs/renderer/wasser-und-licht.md, „Blocklicht“.
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
/// `(x, y)`, Osten `(z, y)`. Das Spiel zeichnet das Viereck als die
/// Dreiecke 0-1-2 und 2-3-0.
/// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
const FACE_INFO: [[[f32; 2]; 4]; 3] = [
    [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]],
];

/// Die Seite aus [`AO_FACES`], als die das Spiel ein Viereck mit den vier
/// Werten je Ecke weich beleuchtet: eben auf dem Rand des Würfels und über
/// die ganze Seite, beides bis auf 1e-4, denn der Baker dreht über sin und
/// cos. Flüssigkeiten bekommen keine, Flächen aus Blockentity-Modellen auch
/// nicht: Das Spiel zeichnet sie im Licht der Entities, siehe
/// [`entity_light`].
/// Siehe docs/renderer/weiche-beleuchtung.md, „Die Regeln des Spiels“.
fn ao_face(quad: &Quad) -> Option<usize> {
    if quad.fluid.is_some() || quad.entity.is_some() {
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
    /// Je Pixel zwei Wörter, die Tönungskarte: der Anteil, der die Farbe des
    /// Blocks aus dem Biom trägt, und der, der die des Wassers trägt, je
    /// Kanal ein Byte, Rot im untersten. `image` hält den Rest; zusammen
    /// setzt [`tinted`] sie beim Zeichnen. Nur für Sprites mit Flächen,
    /// deren Farbe vom Biom kommt.
    /// Siehe docs/renderer/biomfarben.md, „Tönung beim Zeichnen“.
    pub tint: Option<Vec<u32>>,
}

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

/// Eine Farbe gepackt wie die Tönungskarte, Rot im untersten Byte.
pub fn pack(tint: [u8; 3]) -> u32 {
    tint[0] as u32 | (tint[1] as u32) << 8 | (tint[2] as u32) << 16
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
        .filter_map(|quad| Some(ProjectedQuad::new(quad, projection, seite(quad)?)))
        .collect();

    // Jede Fläche legt je Pixel ein Fragment ab, gemischt wird erst am
    // Schluss: je Pixel von hinten nach vorne, nach der Tiefe an genau
    // diesem Pixel, egal in welcher Reihenfolge die Flächen kommen. Bei
    // gleicher Tiefe gewinnt die spätere; sortiert wird deshalb stabil nach
    // der vordersten Ecke, und deckungsgleiche Flächen behalten ihre
    // Modellreihenfolge.
    // Siehe docs/renderer/naehte.md, „Fragmente je Pixel“.
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
        tint: None,
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
    /// `rueckseite`: Die Kamera sieht die Fläche von hinten, siehe [`seite`].
    fn new(quad: &'a Quad, projection: &Projection, rueckseite: bool) -> ProjectedQuad<'a> {
        let screen = quad.corners.map(|corner| {
            let (x, y) = projection.project(corner);
            (x, y, Projection::depth(corner))
        });
        ProjectedQuad {
            quad,
            screen,
            depth: screen.iter().map(|&(_, _, d)| d).fold(f32::MIN, f32::max),
            shade: shade_factor(quad, rueckseite),
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
        // Die Schicht wie im Spiel. Eine Fläche aus einem Blockentity-Modell
        // bringt ihre mit. Sonst (`FaceBakery.computeMaterialTransparency`
        // und `ChunkSectionLayer.byTransparency` in 26.2): mit
        // `force_translucent` durchscheinend, sonst nach dem Ausschnitt der
        // Textur. Flüssigkeiten gehen dort nicht durch den FaceBakery. Eine
        // deckende Fläche deckt ausgeschnitten wie gemischt ganz.
        let deckung = match self.quad.entity {
            Some(Entity { schicht, .. }) if schicht.gemischt => Deckung::Gemischt {
                schwelle: schwelle(schicht.alpha),
            },
            Some(Entity { schicht, .. }) => Deckung::Ausgeschnitten {
                fuellung: None,
                schwelle: schwelle(schicht.alpha),
            },
            None if self.quad.force_translucent
                || self.quad.fluid.is_some()
                || textures.durchscheinend(self.quad.texture, bounds[0], bounds[1]) =>
            {
                Deckung::Gemischt {
                    schwelle: schwelle(Some(ALPHA_CUTOUT_TRANSLUCENT)),
                }
            }
            None => Deckung::Ausgeschnitten {
                fuellung: textures
                    .fuellung(self.quad.texture)
                    .map(|farbe| farbe.map(|c| LINEAR[c as usize])),
                schwelle: schwelle(Some(ALPHA_CUTOUT_CUTOUT)),
            },
        };

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
                    surface: self.quad.fluid.is_some_and(|(_, face)| face == Face::Up),
                    order,
                    ao_face,
                    deckung,
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
    zur_kamera(quad.normal())
}

/// Zeigt die Normale zur Kamera, siehe [`faces_camera`]?
fn zur_kamera(n: [f32; 3]) -> bool {
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n[0] + n[1] + n[2] > EDGE_ON * length
}

/// Welche Seite einer Fläche die Kamera sieht: `Some(false)` die Vorderseite,
/// `Some(true)` die Rückseite, die nur eine Schicht ohne Culling zeichnet
/// (`RenderPipeline.isCull`), `None` keine.
fn seite(quad: &Quad) -> Option<bool> {
    let n = quad.normal();
    if zur_kamera(n) {
        return Some(false);
    }
    let beidseitig = quad.entity.is_some_and(|e| e.schicht.beidseitig);
    (beidseitig && zur_kamera(n.map(|a| -a))).then_some(true)
}

/// Die Richtungen des Lichts für Entity-Modelle in der Oberwelt, wie
/// `Lighting.updateLevel` in 26.2 sie setzt: `DIFFUSE_LIGHT_0` und
/// `DIFFUSE_LIGHT_1` vor dem Normieren.
const ENTITY_LICHT: [[f32; 3]; 2] = [[0.2, 1.0, -0.7], [-0.2, 1.0, 0.7]];

/// Wie hell eine Fläche aus einem Blockentity-Modell ist:
/// `minecraft_mix_light` in `shaders/include/light.glsl`, 0,6 je Richtung
/// und 0,4 Umgebung. Oben 1, nach Norden und Süden 0,74, nach Osten und
/// Westen 0,50, unten 0,4.
/// Siehe docs/renderer/blockentities.md, „Licht“.
fn entity_light(n: [f32; 3]) -> f32 {
    let laenge = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let summe: f32 = ENTITY_LICHT
        .iter()
        .map(|l| ((l[0] * n[0] + l[1] * n[1] + l[2] * n[2]) / (laenge(*l) * laenge(n))).max(0.0))
        .sum();
    (summe * 0.6 + 0.4).min(1.0)
}

/// Helligkeit nach der Richtung, in die die Fläche am stärksten zeigt. Eine
/// Fläche aus einem Blockentity-Modell liegt im Licht der Entities, von
/// hinten mit `PER_FACE_LIGHTING` im Licht der umgekehrten Normalen.
fn shade_factor(quad: &Quad, rueckseite: bool) -> f32 {
    if let Some(entity) = quad.entity {
        let n = quad.normal();
        let umgekehrt = rueckseite && entity.schicht.je_seite;
        return entity_light(if umgekehrt { n.map(|a| -a) } else { n });
    }
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
/// beleuchtet wird, wie sie mit Löchern deckt — und der Rang der Fläche,
/// der bei gleicher Tiefe entscheidet.
#[derive(Clone, Copy)]
struct Shading {
    shade: f32,
    tint: Option<[f32; 3]>,
    surface: bool,
    order: u32,
    ao_face: Option<usize>,
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
            surface,
            order,
            ao_face,
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
            true,
        );
        anpassen(&mut oben);
        let model = BakedModel {
            quads: vec![oben],
            ambient_occlusion: false,
        };
        let sprite = render(
            &model,
            textures,
            &Projection::new(32),
            Tints::default(),
            Leuchten::Stufe(0),
        )
        .unwrap();
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
                true,
            );
            oben.texture = textur;
            let model = BakedModel {
                quads: vec![oben],
                ambient_occlusion: false,
            };
            render(
                &model,
                &textures,
                &Projection::new(32),
                Tints::default(),
                Leuchten::Stufe(0),
            )
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

    /// Der Alpha-Test von `cutout_terrain`: unter der Hälfte der
    /// Abtastpunkte verworfen, darüber ganz deckend in der Farbe der
    /// deckenden. Bei genau der Hälfte entscheidet das Texel in der
    /// Pixelmitte. Mit der Füllung aus `dark_cutout` zählen die Löcher in
    /// ihrer Farbe mit.
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
            let ist = entity_light(normale);
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
            true,
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
                Leuchten::Stufe(0),
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
                Leuchten::Stufe(0),
            )
        };
        assert_eq!(seite(&unterseite()), None, "Blockmodell");
        assert!(bild(unterseite()).is_none());
        let mit_culling = aus_entity(unterseite(), schicht(false, true));
        assert_eq!(seite(&mit_culling), None);
        assert!(bild(mit_culling).is_none());

        let je_seite = aus_entity(unterseite(), schicht(true, true));
        assert_eq!(seite(&je_seite), Some(true));
        assert_eq!(shade_factor(&je_seite, true), 1.0, "Licht der Oberseite");
        let einseitig_beleuchtet = aus_entity(unterseite(), schicht(true, false));
        assert_eq!(shade_factor(&einseitig_beleuchtet, true), 0.4);
        assert!(bild(je_seite).is_some());

        let oben = quad(
            [
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
            true,
        );
        assert_eq!(seite(&aus_entity(oben, schicht(true, true))), Some(false));
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
                    true,
                ),
                schicht,
            );
            oben.texture = textur;
            let model = BakedModel {
                quads: vec![oben],
                ambient_occlusion: false,
            };
            let sprite = render(
                &model,
                textures,
                &Projection::new(32),
                Tints::default(),
                Leuchten::Stufe(0),
            )
            .unwrap();
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

    fn quad(corners: [[f32; 3]; 4], shade: bool) -> Quad {
        Quad {
            corners,
            uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture: crate::assets::Textures::MISSING,
            tint_index: None,
            shade,
            force_translucent: false,
            fluid: None,
            entity: None,
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
        assert_eq!(shade_factor(&oben, false), SHADE_TOP);

        let unten = quad(
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            true,
        );
        assert_eq!(shade_factor(&unten, false), SHADE_BOTTOM);
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
        assert_eq!(shade_factor(&nord, false), SHADE_NORTH_SOUTH);

        let ost = quad(
            [
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
            ],
            true,
        );
        assert_eq!(shade_factor(&ost, false), SHADE_EAST_WEST);
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
        assert_eq!(shade_factor(&nord, false), SHADE_TOP);
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
