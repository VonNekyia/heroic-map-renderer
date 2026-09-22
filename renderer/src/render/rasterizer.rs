use image::{Rgba, RgbaImage};

use crate::assets::baker::{BakedModel, Quad};
use crate::assets::{Textures, Tints, fluid};

use super::Projection;

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

/// Helligkeit je Flächenrichtung, wie Minecraft sie verwendet. Ohne diese
/// Abstufung sieht ein isometrischer Würfel flach aus.
const SHADE_TOP: f32 = 1.0;
const SHADE_BOTTOM: f32 = 0.5;
const SHADE_NORTH_SOUTH: f32 = 0.8;
const SHADE_EAST_WEST: f32 = 0.6;

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
pub fn render(
    model: &BakedModel,
    textures: &Textures,
    projection: &Projection,
    tints: Tints,
) -> Option<Sprite> {
    let mut projected: Vec<ProjectedQuad> = model
        .quads
        .iter()
        .filter(|quad| faces_camera(quad))
        .map(|quad| ProjectedQuad::new(quad, projection))
        .collect();

    // Von hinten nach vorne, damit durchsichtige Flächen das Richtige
    // untermischen: Wasser über einem Zaunpfosten, Glas über dem, was
    // im selben Block dahinter liegt. Für deckende Flächen ist die
    // Reihenfolge egal, da entscheidet der Tiefenpuffer.
    //
    // Sortiert wird nach der hintersten Ecke, nicht nach der Mitte. Eine
    // Fläche, die eine andere umschliesst — die Wasserhülle um einen
    // Zaunpfosten —, reicht immer mindestens so weit nach hinten und
    // kommt damit nach ihr. Nach der Mitte sortiert käme der Pfosten
    // zuletzt und stünde trocken im Wasser.
    //
    // Stabil, damit deckungsgleiche Flächen ihre Modellreihenfolge
    // behalten: der Grasblock legt sein Overlay so auf den Grundwürfel,
    // und die Wasseroberfläche liegt genauso auf der Stufe einer
    // gefluteten Treppe.
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

    let mut canvas = Canvas::new(width, height);
    let samples = texture_samples(projection.scale());
    for quad in &projected {
        quad.draw(&mut canvas, textures, min_x, min_y, tints, samples);
    }

    Some(Sprite {
        image: canvas.into_image(),
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
    /// Tiefe der hintersten Ecke, nur zum Sortieren.
    depth: f32,
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
            depth: screen.iter().map(|&(_, _, d)| d).fold(f32::MIN, f32::max),
            shade: shade_factor(quad),
        }
    }

    fn draw(
        &self,
        canvas: &mut Canvas,
        textures: &Textures,
        min_x: i32,
        min_y: i32,
        tints: Tints,
        samples: u32,
    ) {
        let texture = textures.image(self.quad.texture);
        let (tw, th) = texture.dimensions();
        if tw == 0 || th == 0 {
            return;
        }

        let vertices: [Vertex; 4] = std::array::from_fn(|i| {
            let (x, y, depth) = self.screen[i];
            Vertex {
                x: x - min_x as f32,
                y: y - min_y as f32,
                depth,
                u: self.quad.uvs[i][0],
                v: self.quad.uvs[i][1],
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
                    layers: self.quad.layers,
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

/// Alpha von `layers` Schichten desselben Texels hintereinander: was eine
/// Schicht durchlässt, lässt die nächste wieder nur zum Teil durch.
fn stacked(mut texel: [u8; 4], layers: u8) -> [u8; 4] {
    if layers > 1 && texel[3] > 0 && texel[3] < 255 {
        let through = (1.0 - texel[3] as f32 / 255.0).powi(layers as i32);
        texel[3] = 255 - (through * 255.0).round() as u8;
    }
    texel
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

/// Wie ein Texel zur Farbe wird: Helligkeit der Fläche, Färbung und die
/// Zahl der Schichten für die Deckkraft.
#[derive(Clone, Copy)]
struct Shading {
    shade: f32,
    tint: Option<[f32; 3]>,
    layers: u8,
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
        sample: &impl Fn(f32, f32) -> [u8; 4],
        inside: &impl Fn(f32, f32) -> bool,
        shading: Shading,
        samples: u32,
    ) {
        let Shading {
            shade,
            tint,
            layers,
        } = shading;
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
                let w = weights(&v, area, px, py);
                if w[0] < 0.0 || w[1] < 0.0 || w[2] < 0.0 {
                    continue;
                }

                let index = (y as usize) * (self.width as usize) + x as usize;
                let depth = w[0] * v[0].depth + w[1] * v[1].depth + w[2] * v[2].depth;
                // Bei gleicher Tiefe gewinnt die später gezeichnete Fläche.
                // Vanilla legt deckungsgleiche Schichten übereinander: der
                // Grasblock hat vier Overlay-Flächen auf dem Grundwürfel.
                // Deren durchsichtige Texel lassen den Grund stehen, weil
                // Alpha 0 vorher übersprungen wird.
                if depth < self.depth[index] {
                    continue;
                }
                let Some(texel) = filtered(&v, area, px, py, sample, inside, samples) else {
                    continue;
                };

                // Die hochgerechnete Deckkraft steht für das Wasser unter
                // dem Block. Sie gilt nur, wo der Block selbst nichts
                // dahinter hat: ein Zaunpfosten an der Oberfläche bleibt
                // sichtbar, egal wie tief das Wasser unter ihm steht.
                let layers = if self.color[index][3] == 0 { layers } else { 1 };

                self.depth[index] = depth;
                // Durchsichtige Texel mischen sich mit dem, was schon da
                // steht; die Flächen kommen dafür von hinten nach vorne.
                self.color[index] = over(
                    shaded(stacked(texel, layers), shade, tint),
                    self.color[index],
                );
            }
        }
    }

    fn into_image(self) -> RgbaImage {
        RgbaImage::from_fn(self.width, self.height, |x, y| {
            Rgba(self.color[(y * self.width + x) as usize])
        })
    }
}

/// Baryzentrische Gewichte eines Punkts.
fn weights(v: &[Vertex; 3], area: f32, px: f32, py: f32) -> [f32; 3] {
    [
        edge(v[1], v[2], px, py) / area,
        edge(v[2], v[0], px, py) / area,
        edge(v[0], v[1], px, py) / area,
    ]
}

/// Mittelwert der Texel unter einem Pixel, mit vormultipliziertem Alpha —
/// sonst zögen durchsichtige Texel ihre Farbe in die Nachbarn. Gezählt
/// werden nur Abtastpunkte innerhalb der Fläche; liegt keiner darin, weil
/// die Fläche schmaler ist als ein Pixel, gilt der Mittelpunkt. `None`,
/// wenn kein Texel deckt.
fn filtered(
    v: &[Vertex; 3],
    area: f32,
    px: f32,
    py: f32,
    sample: &impl Fn(f32, f32) -> [u8; 4],
    inside: &impl Fn(f32, f32) -> bool,
    n: u32,
) -> Option<[u8; 4]> {
    // Summe der vormultiplizierten Farben, Summe der Alphas, Anzahl.
    let mut acc = ([0.0f32; 3], 0.0f32, 0u32);
    fn add(acc: &mut ([f32; 3], f32, u32), texel: [u8; 4]) {
        let a = texel[3] as f32 / 255.0;
        for (sum, &value) in acc.0.iter_mut().zip(&texel[..3]) {
            *sum += value as f32 * a;
        }
        acc.1 += a;
        acc.2 += 1;
    }
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
    Some([
        (sum[0] / alpha).round() as u8,
        (sum[1] / alpha).round() as u8,
        (sum[2] / alpha).round() as u8,
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
pub fn over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    if src[3] == 255 || dst[3] == 0 {
        return src;
    }
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        let value = (src[c] as f32 * sa + dst[c] as f32 * da * (1.0 - sa)) / out_a;
        out[c] = value.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (out_a * 255.0).round() as u8;
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
        };
        assert!(
            render(
                &model,
                &Textures::new(),
                &Projection::new(16),
                Tints::default()
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
            layers: 1,
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
                Tints::default()
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
            &BakedModel { quads },
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
}
