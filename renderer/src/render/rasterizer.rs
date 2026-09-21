use image::{Rgba, RgbaImage};

use crate::assets::Textures;
use crate::assets::baker::{BakedModel, Quad};

use super::Projection;

/// Wie viele Abtastpunkte je Pixelkante beim Rastern verwendet werden.
///
/// Die Oberseite eines Blocks wird auf die halbe Höhe gestaucht; ohne
/// Überabtastung treppen die Kanten sichtbar. Das kostet nur beim Backen,
/// nicht im Renderpfad.
const SUPERSAMPLE: u32 = 2;

/// Obergrenze für die Kantenlänge eines Sprites, in Blockbreiten. Modelle
/// dürfen von -16 bis 32 reichen, also drei Blöcke; alles darüber ist
/// kaputt.
const MAX_SPRITE_BLOCKS: u32 = 8;

/// Helligkeit je Flächenrichtung, wie Minecraft sie verwendet. Ohne diese
/// Abstufung sieht ein isometrischer Würfel flach aus.
const SHADE_TOP: f32 = 1.0;
const SHADE_BOTTOM: f32 = 0.5;
const SHADE_NORTH_SOUTH: f32 = 0.8;
const SHADE_EAST_WEST: f32 = 0.6;

/// Vorläufige Färbung für Flächen mit `tintindex`.
///
/// Richtig wäre die Colormap des Bioms. Bis Schritt 8 die liest, sorgt ein
/// fester Grünton dafür, dass Gras und Laub nicht weiß bleiben.
// ponytail: fester Wert, ersetzt durch die Biom-Colormap in Schritt 8.
const PROVISIONAL_TINT: [f32; 3] = [0.56, 0.74, 0.35];

/// Das fertig gerasterte Bild einer Blockstate.
pub struct Sprite {
    pub image: RgbaImage,
    /// Pixelposition der linken oberen Ecke, relativ zum projizierten
    /// Blockursprung.
    pub offset: (i32, i32),
}

/// Rastert ein gebackenes Modell in ein Sprite.
///
/// Da die Kamera fest steht, sieht jede Blockstate immer gleich aus. Das
/// Sprite entsteht deshalb einmal und wird im Renderpfad nur noch kopiert.
pub fn render(model: &BakedModel, textures: &Textures, projection: &Projection) -> Option<Sprite> {
    let projected: Vec<ProjectedQuad> = model
        .quads
        .iter()
        .filter(|quad| faces_camera(quad))
        .map(|quad| ProjectedQuad::new(quad, projection))
        .collect();

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

    let mut canvas = Canvas::new(width * SUPERSAMPLE, height * SUPERSAMPLE);
    for quad in &projected {
        quad.draw(&mut canvas, textures, min_x, min_y);
    }

    Some(Sprite {
        image: canvas.downsample(width, height),
        offset: (min_x, min_y),
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
    shade: f32,
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
            shade: shade_factor(quad),
        }
    }

    fn draw(&self, canvas: &mut Canvas, textures: &Textures, min_x: i32, min_y: i32) {
        let texture = textures.image(self.quad.texture);
        let (tw, th) = texture.dimensions();
        if tw == 0 || th == 0 {
            return;
        }

        let scale = SUPERSAMPLE as f32;
        let vertices: [Vertex; 4] = std::array::from_fn(|i| {
            let (x, y, depth) = self.screen[i];
            Vertex {
                x: (x - min_x as f32) * scale,
                y: (y - min_y as f32) * scale,
                depth,
                u: self.quad.uvs[i][0],
                v: self.quad.uvs[i][1],
            }
        });

        let tint = self.quad.tint_index.map(|_| PROVISIONAL_TINT);
        for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
            canvas.triangle(
                [vertices[a], vertices[b], vertices[c]],
                |u, v| sample(texture, tw, th, u, v),
                self.shade,
                tint,
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
/// derselben Ebene.
fn faces_camera(quad: &Quad) -> bool {
    let n = quad.normal();
    n[0] + n[1] + n[2] > 0.0
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
}

/// Farb- und Tiefenpuffer in Überabtastung.
struct Canvas {
    width: u32,
    height: u32,
    color: Vec<[u8; 4]>,
    depth: Vec<f32>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Canvas {
        let pixels = (width as usize) * (height as usize);
        Canvas {
            width,
            height,
            color: vec![[0; 4]; pixels],
            depth: vec![f32::NEG_INFINITY; pixels],
        }
    }

    /// Rastert ein Dreieck mit baryzentrischer Interpolation.
    ///
    /// Die Projektion ist orthographisch, deshalb ist lineare Interpolation
    /// exakt — es braucht keine perspektivische Korrektur.
    fn triangle(
        &mut self,
        v: [Vertex; 3],
        sample: impl Fn(f32, f32) -> [u8; 4],
        shade: f32,
        tint: Option<[f32; 3]>,
    ) {
        let area = edge(v[0], v[1], v[2].x, v[2].y);
        if area.abs() < 1e-6 {
            return;
        }

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
                let w0 = edge(v[1], v[2], px, py) / area;
                let w1 = edge(v[2], v[0], px, py) / area;
                let w2 = edge(v[0], v[1], px, py) / area;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }

                let index = (y as usize) * (self.width as usize) + x as usize;
                let depth = w0 * v[0].depth + w1 * v[1].depth + w2 * v[2].depth;
                // Bei gleicher Tiefe gewinnt die später gezeichnete Fläche.
                // Vanilla legt deckungsgleiche Schichten übereinander: der
                // Grasblock hat vier Overlay-Flächen auf dem Grundwürfel.
                // Deren durchsichtige Texel lassen den Grund stehen, weil
                // Alpha 0 vorher übersprungen wird.
                if depth < self.depth[index] {
                    continue;
                }

                let u = w0 * v[0].u + w1 * v[1].u + w2 * v[2].u;
                let vv = w0 * v[0].v + w1 * v[1].v + w2 * v[2].v;
                let texel = sample(u, vv);
                if texel[3] == 0 {
                    continue;
                }

                self.depth[index] = depth;
                self.color[index] = shaded(texel, shade, tint);
            }
        }
    }

    /// Mittelt die Überabtastung zurück auf die Zielgröße. Gerechnet wird
    /// mit vormultipliziertem Alpha, sonst bekommen weiche Kanten einen
    /// dunklen Saum.
    fn downsample(&self, width: u32, height: u32) -> RgbaImage {
        let factor = SUPERSAMPLE;
        RgbaImage::from_fn(width, height, |x, y| {
            let mut sum = [0.0f32; 3];
            let mut alpha = 0.0f32;
            for dy in 0..factor {
                for dx in 0..factor {
                    let index = ((y * factor + dy) as usize) * (self.width as usize)
                        + (x * factor + dx) as usize;
                    let p = self.color[index];
                    let a = p[3] as f32 / 255.0;
                    for c in 0..3 {
                        sum[c] += p[c] as f32 * a;
                    }
                    alpha += a;
                }
            }
            if alpha <= 0.0 {
                return Rgba([0, 0, 0, 0]);
            }
            let samples = (factor * factor) as f32;
            Rgba([
                (sum[0] / alpha).round() as u8,
                (sum[1] / alpha).round() as u8,
                (sum[2] / alpha).round() as u8,
                (alpha / samples * 255.0).round() as u8,
            ])
        })
    }
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

/// Vorzeichenbehaftete Fläche des Dreiecks aus zwei Ecken und einem Punkt.
fn edge(a: Vertex, b: Vertex, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::baker::Quad;

    fn quad(corners: [[f32; 3]; 4], shade: bool) -> Quad {
        Quad {
            corners,
            uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture: crate::assets::Textures::MISSING,
            tint_index: None,
            shade,
            force_translucent: false,
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
        assert!(render(&leer, &Textures::new(), &Projection::default()).is_none());
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
            &BakedModel { quads },
            &Textures::new(),
            &Projection::new(16),
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
