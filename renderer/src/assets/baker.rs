use super::fluid::Fluid;
use super::model::{Face, Rotation};
use super::{ResolvedVariant, TextureId};

/// Kantenlänge eines Blocks in Modellkoordinaten.
const BLOCK: f32 = 16.0;
/// Mittelpunkt eines Blocks, um den Blockstate-Drehungen laufen.
const CENTER: f32 = BLOCK / 2.0;

/// Ein Viereck in Blockkoordinaten (0..1), fertig zum Rastern.
#[derive(Debug, Clone)]
pub struct Quad {
    pub corners: [[f32; 3]; 4],
    /// Texturkoordinaten je Ecke, normiert auf 0..1.
    pub uvs: [[f32; 2]; 4],
    pub texture: TextureId,
    pub tint_index: Option<u32>,
    pub shade: bool,
    pub force_translucent: bool,
    /// Gesetzt für die Flächen einer Flüssigkeit, mit ihrer Richtung: die
    /// Fläche entfällt, wenn der Nachbar dort dieselbe Flüssigkeit führt.
    pub fluid: Option<(Fluid, Face)>,
}

impl Quad {
    /// Flächennormale, bestimmt aus den ersten drei Ecken.
    pub fn normal(&self) -> [f32; 3] {
        let [a, b, c, _] = self.corners;
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    }
}

/// Alle Vierecke einer Blockstate, im Blockwürfel 0..1.
#[derive(Debug, Default)]
pub struct BakedModel {
    pub quads: Vec<Quad>,
}

impl BakedModel {
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
    }
}

/// Wandelt die Modelle einer Blockstate in Vierecke um.
///
/// Bei `multipart` liefert der Asset-Layer mehrere Varianten; ihre Vierecke
/// landen alle im selben Modell. Danach spielt das Modell-JSON keine Rolle
/// mehr.
pub fn bake(variants: &[ResolvedVariant]) -> BakedModel {
    let mut quads = Vec::new();

    for variant in variants {
        for element in &variant.model.elements {
            for (face, data) in &element.faces {
                let plane = plane_of(*face, element.from, element.to);

                // Die Geometrie kommt immer aus der Größe des Elements, nie
                // aus `uv`. Ein `uv`, das einen Ausschnitt der Textur wählt,
                // darf die Fläche nicht verschieben — Türen und Zäune geben
                // genau solche Ausschnitte an.
                let extent = default_uv(*face, element.from, element.to);
                let uv = data.uv.unwrap_or(extent);

                // Beide Rechtecke in derselben Reihenfolge ablaufen: links
                // unten, rechts unten, rechts oben, links oben. Von außen
                // betrachtet läuft das gegen den Uhrzeigersinn, damit das
                // Kreuzprodukt der ersten drei Ecken nach außen zeigt —
                // daran hängt die Helligkeit der Fläche.
                let ecken = |r: [f32; 4]| [[r[0], r[3]], [r[2], r[3]], [r[2], r[1]], [r[0], r[1]]];
                let texture_corners = ecken(uv);
                let texture_corners = if variant.uvlock {
                    texture_corners.map(|p| lock_uv(*face, p, variant.x, variant.y, variant.z))
                } else {
                    texture_corners
                };
                let mut corners = ecken(extent).map(|[u, v]| corner(*face, u, v, plane));

                for point in &mut corners {
                    if let Some(rotation) = &element.rotation {
                        *point = rotate_element(*point, rotation);
                    }
                    *point = rotate_variant(*point, variant.x, variant.y, variant.z);
                    for axis in point.iter_mut() {
                        *axis /= BLOCK;
                    }
                }

                // Eine Flächendrehung verschiebt nur, welche Texturecke an
                // welche Geometrieecke kommt.
                let rotated = rotate_texture(texture_corners, data.rotation);

                quads.push(Quad {
                    corners,
                    uvs: rotated.map(|[u, v]| [u / BLOCK, v / BLOCK]),
                    texture: data.texture,
                    tint_index: data.tint_index,
                    shade: element.shade,
                    force_translucent: data.force_translucent,
                    fluid: None,
                });
            }
        }
    }

    BakedModel { quads }
}

/// Die sechs Seiten eines achsenparallelen Kastens.
///
/// Flüssigkeiten haben kein Modell-JSON, ihre Geometrie entsteht im Code.
/// Die Ecken kommen trotzdem aus derselben Zuordnung wie beim Backen, damit
/// Umlaufsinn, Normale und Texturrichtung zusammenpassen.
pub fn box_quads(
    from: [f32; 3],
    to: [f32; 3],
    texture: TextureId,
    tint_index: Option<u32>,
    fluid: Option<Fluid>,
) -> impl Iterator<Item = Quad> {
    [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ]
    .into_iter()
    .map(move |face| {
        let plane = plane_of(face, from, to);
        let extent = default_uv(face, from, to);
        let ecken = |r: [f32; 4]| [[r[0], r[3]], [r[2], r[3]], [r[2], r[1]], [r[0], r[1]]];
        Quad {
            corners: ecken(extent).map(|[u, v]| corner(face, u, v, plane).map(|a| a / BLOCK)),
            uvs: ecken(extent).map(|[u, v]| [u / BLOCK, v / BLOCK]),
            texture,
            tint_index,
            shade: true,
            force_translucent: false,
            fluid: fluid.map(|fluid| (fluid, face)),
        }
    })
}

/// Ebene, in der eine Seite liegt.
fn plane_of(face: Face, from: [f32; 3], to: [f32; 3]) -> f32 {
    match face {
        Face::Down => from[1],
        Face::Up => to[1],
        Face::North => from[2],
        Face::South => to[2],
        Face::West => from[0],
        Face::East => to[0],
    }
}

/// Texturkoordinate auf einer Seite zurück in Modellkoordinaten.
///
/// Von außen betrachtet läuft `u` nach rechts und `v` nach unten. Daraus
/// folgt für jede Seite genau eine Zuordnung; `default_uv` ist die Umkehrung.
fn corner(face: Face, u: f32, v: f32, plane: f32) -> [f32; 3] {
    match face {
        Face::Down => [u, plane, BLOCK - v],
        Face::Up => [u, plane, v],
        Face::North => [BLOCK - u, BLOCK - v, plane],
        Face::South => [u, BLOCK - v, plane],
        Face::West => [plane, BLOCK - v, u],
        Face::East => [plane, BLOCK - v, BLOCK - u],
    }
}

/// Texturkoordinate aus einem Punkt auf der Seite — die Umkehrung von
/// `corner`, die Ebene fällt dabei weg.
fn uv_of(face: Face, [x, y, z]: [f32; 3]) -> [f32; 2] {
    match face {
        Face::Down => [x, BLOCK - z],
        Face::Up => [x, z],
        Face::North => [BLOCK - x, BLOCK - y],
        Face::South => [x, BLOCK - y],
        Face::West => [z, BLOCK - y],
        Face::East => [BLOCK - z, BLOCK - y],
    }
}

/// Auf welche Seite eine Variantendrehung eine Seite abbildet.
fn rotate_face(face: Face, x: i32, y: i32, z: i32) -> Face {
    // Die Normale mitdrehen. Blockstates erlauben nur Vielfache von 90 Grad,
    // also liegt sie danach wieder auf einer Achse.
    let n = match face {
        Face::Down => [0.0, -1.0, 0.0],
        Face::Up => [0.0, 1.0, 0.0],
        Face::North => [0.0, 0.0, -1.0],
        Face::South => [0.0, 0.0, 1.0],
        Face::West => [-1.0, 0.0, 0.0],
        Face::East => [1.0, 0.0, 0.0],
    };
    let n = rotate_z(rotate_y(rotate_x(n, -(x as f32)), -(y as f32)), -(z as f32));

    let achse = if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
        0
    } else if n[1].abs() >= n[2].abs() {
        1
    } else {
        2
    };
    match (achse, n[achse] > 0.0) {
        (0, true) => Face::East,
        (0, false) => Face::West,
        (1, true) => Face::Up,
        (1, false) => Face::Down,
        (_, true) => Face::South,
        (_, false) => Face::North,
    }
}

/// `uvlock`: die Textur bleibt an der Welt ausgerichtet, statt sich mit dem
/// Modell mitzudrehen. 143 Vanilla-Blockstates setzen das Flag, fast alle
/// Treppen, Zäune und Falltüren darunter.
///
/// Die Texturkoordinate wandert dafür auf ihre Seite des Einheitswürfels,
/// dreht sich mit und wird auf der Zielseite wieder zur Texturkoordinate.
/// Dass sie dabei an derselben Geometrieecke bleibt, erledigt der
/// gemeinsame Eckenumlauf in `bake`.
fn lock_uv(face: Face, [u, v]: [f32; 2], x: i32, y: i32, z: i32) -> [f32; 2] {
    let plane = plane_of(face, [0.0; 3], [BLOCK; 3]);
    let punkt = rotate_variant(corner(face, u, v, plane), x, y, z);
    // Vierteldrehungen über sin/cos treffen die ganzen Zahlen knapp daneben.
    uv_of(rotate_face(face, x, y, z), punkt).map(|a| (a * 1000.0).round() / 1000.0)
}

/// Fehlt `uv` im Modell, leitet Minecraft sie aus der Größe des Elements ab.
fn default_uv(face: Face, from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let (fx, fy, fz) = (from[0], from[1], from[2]);
    let (tx, ty, tz) = (to[0], to[1], to[2]);
    match face {
        Face::Down => [fx, BLOCK - tz, tx, BLOCK - fz],
        Face::Up => [fx, fz, tx, tz],
        Face::North => [BLOCK - tx, BLOCK - ty, BLOCK - fx, BLOCK - fy],
        Face::South => [fx, BLOCK - ty, tx, BLOCK - fy],
        Face::West => [fz, BLOCK - ty, tz, BLOCK - fy],
        Face::East => [BLOCK - tz, BLOCK - ty, BLOCK - fz, BLOCK - fy],
    }
}

/// Dreht die Textur auf der Fläche um Vielfache von 90 Grad im Uhrzeigersinn.
fn rotate_texture(corners: [[f32; 2]; 4], degrees: u16) -> [[f32; 2]; 4] {
    let steps = (degrees / 90) as usize % 4;
    let mut out = [[0.0; 2]; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = corners[(i + steps) % 4];
    }
    out
}

/// Drehung eines Elements um seinen eigenen Ursprung.
fn rotate_element(point: [f32; 3], rotation: &Rotation) -> [f32; 3] {
    let origin = rotation.origin;
    let mut v = [
        point[0] - origin[0],
        point[1] - origin[1],
        point[2] - origin[2],
    ];

    // Minecraft skaliert lokal, also vor dem Drehen.
    if rotation.rescale {
        let factors = rescale_factors(rotation.angles);
        for (axis, factor) in v.iter_mut().zip(factors) {
            *axis *= factor;
        }
    }
    v = rotate_all(v, rotation.angles);

    [v[0] + origin[0], v[1] + origin[1], v[2] + origin[2]]
}

/// `rescale` dehnt das gedrehte Element so, dass es seinen ursprünglichen
/// Platz wieder ausfüllt.
///
/// Der Faktor je Achse ist der Kehrwert der größten Komponente der gedrehten
/// Einheitsachse, wie in `CuboidRotation.computeRescale`. Das gilt für eine
/// wie für mehrere Achsen und ist durch Wurzel 3 nach oben begrenzt — die
/// naheliegende Formel `1/cos(winkel)` wächst dagegen bei 90 Grad über alle
/// Grenzen.
fn rescale_factors(angles: [f32; 3]) -> [f32; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|achse| {
        let gedreht = rotate_all(achse, angles);
        let groesste = gedreht[0].abs().max(gedreht[1].abs()).max(gedreht[2].abs());
        if groesste > 1e-6 { 1.0 / groesste } else { 1.0 }
    })
}

/// Dreht um X, dann Y, dann Z — die Reihenfolge, die Minecraft für die
/// mehrachsige Schreibweise verwendet.
fn rotate_all(v: [f32; 3], angles: [f32; 3]) -> [f32; 3] {
    rotate_z(rotate_y(rotate_x(v, angles[0]), angles[1]), angles[2])
}

/// Drehung des ganzen Modells um den Blockmittelpunkt, wie sie in der
/// Blockstate-Datei steht.
///
/// Minecraft dreht dort mit umgekehrtem Vorzeichen (`BlockModelRotation`).
fn rotate_variant(point: [f32; 3], x: i32, y: i32, z: i32) -> [f32; 3] {
    if x == 0 && y == 0 && z == 0 {
        return point;
    }
    let mut v = [point[0] - CENTER, point[1] - CENTER, point[2] - CENTER];
    v = rotate_x(v, -(x as f32));
    v = rotate_y(v, -(y as f32));
    v = rotate_z(v, -(z as f32));
    [v[0] + CENTER, v[1] + CENTER, v[2] + CENTER]
}

fn rotate_x([x, y, z]: [f32; 3], degrees: f32) -> [f32; 3] {
    if degrees == 0.0 {
        return [x, y, z];
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    [x, y * cos - z * sin, y * sin + z * cos]
}

fn rotate_y([x, y, z]: [f32; 3], degrees: f32) -> [f32; 3] {
    if degrees == 0.0 {
        return [x, y, z];
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    [x * cos + z * sin, y, -x * sin + z * cos]
}

fn rotate_z([x, y, z]: [f32; 3], degrees: f32) -> [f32; 3] {
    if degrees == 0.0 {
        return [x, y, z];
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    [x * cos - y * sin, x * sin + y * cos, z]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rund(v: [f32; 3]) -> [f32; 3] {
        v.map(|a| (a * 1000.0).round() / 1000.0)
    }

    /// `corner` und `default_uv` müssen zueinander passen: die abgeleiteten
    /// Texturkoordinaten müssen wieder die Ecken des Elements ergeben.
    #[test]
    fn default_uv_ist_die_umkehrung_von_corner() {
        let from = [2.0, 3.0, 4.0];
        let to = [14.0, 15.0, 12.0];
        for face in [
            Face::Down,
            Face::Up,
            Face::North,
            Face::South,
            Face::West,
            Face::East,
        ] {
            let uv = default_uv(face, from, to);
            let plane = plane_of(face, from, to);
            for [u, v] in [[uv[0], uv[1]], [uv[2], uv[3]]] {
                let p = corner(face, u, v, plane);
                for i in 0..3 {
                    assert!(
                        (p[i] >= from[i] - 0.001) && (p[i] <= to[i] + 0.001),
                        "{face:?}: Ecke {p:?} liegt ausserhalb von {from:?}..{to:?}"
                    );
                }
            }
        }
    }

    /// Von außen gesehen läuft u nach rechts, v nach unten.
    #[test]
    fn texturrichtung_je_seite() {
        // Nordseite zeigt nach -z, u wächst nach Westen
        assert_eq!(corner(Face::North, 0.0, 0.0, 0.0), [16.0, 16.0, 0.0]);
        assert_eq!(corner(Face::North, 16.0, 16.0, 0.0), [0.0, 0.0, 0.0]);
        // Südseite zeigt nach +z, u wächst nach Osten
        assert_eq!(corner(Face::South, 0.0, 0.0, 16.0), [0.0, 16.0, 16.0]);
        // Oberseite: u nach Osten, v nach Süden
        assert_eq!(corner(Face::Up, 0.0, 0.0, 16.0), [0.0, 16.0, 0.0]);
        assert_eq!(corner(Face::Up, 16.0, 16.0, 16.0), [16.0, 16.0, 16.0]);
    }

    /// Regression: die Geometrie kam aus `uv` statt aus der Elementgröße.
    /// Eine Tür gibt für ihre Schmalseite einen Texturausschnitt an, der
    /// nicht der Flächengröße entspricht; die Fläche landete dadurch
    /// ausserhalb des Elements.
    #[test]
    fn uv_verschiebt_die_geometrie_nicht() {
        let from = [0.0, 0.0, 0.0];
        let to = [3.0, 16.0, 16.0];
        let plane = plane_of(Face::North, from, to);

        let extent = default_uv(Face::North, from, to);
        let ecken = [
            corner(Face::North, extent[0], extent[1], plane),
            corner(Face::North, extent[2], extent[3], plane),
        ];
        for punkt in ecken {
            assert!(
                punkt[0] >= from[0] && punkt[0] <= to[0],
                "x={} liegt ausserhalb von 0..3",
                punkt[0]
            );
        }

        // Ein abweichendes uv beschreibt nur die Textur, nicht die Fläche.
        let abweichend = [0.0, 0.0, 3.0, 16.0];
        let verschoben = corner(Face::North, abweichend[0], abweichend[1], plane);
        assert!(
            verschoben[0] > to[0],
            "genau dieser Wert wäre früher als Geometrie genommen worden"
        );
    }

    #[test]
    fn drehungen_sind_rechtshaendig() {
        assert_eq!(rund(rotate_y([1.0, 0.0, 0.0], 90.0)), [0.0, 0.0, -1.0]);
        assert_eq!(rund(rotate_x([0.0, 1.0, 0.0], 90.0)), [0.0, 0.0, 1.0]);
        assert_eq!(rund(rotate_z([1.0, 0.0, 0.0], 90.0)), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn variantendrehung_laeuft_um_den_blockmittelpunkt() {
        // Der Mittelpunkt bleibt liegen
        assert_eq!(
            rund(rotate_variant([8.0, 8.0, 8.0], 0, 90, 0)),
            [8.0, 8.0, 8.0]
        );
        // 90 Grad um Y schiebt die Nordkante nach Osten
        assert_eq!(
            rund(rotate_variant([8.0, 8.0, 0.0], 0, 90, 0)),
            [16.0, 8.0, 8.0]
        );
        // Vier Vierteldrehungen sind die Identität
        let mut p = [3.0, 5.0, 7.0];
        for _ in 0..4 {
            p = rotate_variant(p, 0, 90, 0);
        }
        assert_eq!(rund(p), [3.0, 5.0, 7.0]);
    }

    /// Bei den Winkeln, die in Vanilla und im Pack vorkommen, stimmt der
    /// Faktor mit 1/cos überein — das Bild ändert sich durch die neue
    /// Formel also nicht.
    #[test]
    fn rescale_bei_den_ueblichen_winkeln() {
        for winkel in [22.5f32, -22.5, 45.0, -45.0] {
            let f = rescale_factors([0.0, winkel, 0.0]);
            let erwartet = 1.0 / winkel.to_radians().cos().abs();
            assert!((f[0] - erwartet).abs() < 1e-4, "{winkel}: {f:?}");
            assert!((f[1] - 1.0).abs() < 1e-6, "die Drehachse bleibt");
            assert!((f[2] - erwartet).abs() < 1e-4, "{winkel}: {f:?}");
        }
    }

    /// Regression: `1/cos` läuft bei 90 Grad gegen unendlich und erzeugte
    /// Koordinaten in Millionenhöhe. Der Kehrwert der größten Komponente
    /// bleibt bei 1.
    #[test]
    fn rescale_bleibt_bei_jedem_winkel_begrenzt() {
        assert_eq!(rund(rescale_factors([0.0, 90.0, 0.0])), [1.0, 1.0, 1.0]);

        // 60 Grad: die größte Komponente ist sin, nicht cos
        let f = rescale_factors([0.0, 60.0, 0.0]);
        assert!((f[0] - 1.1547).abs() < 1e-3, "{f:?}");

        // obere Schranke ist Wurzel 3, wenn die Achse auf eine Raumdiagonale
        // zeigt
        for winkel in (0..360).step_by(7) {
            for achse in 0..3 {
                let mut angles = [0.0; 3];
                angles[achse] = winkel as f32;
                for f in rescale_factors(angles) {
                    assert!(
                        f.is_finite() && (1.0..=1.7321).contains(&f),
                        "{winkel}: {f}"
                    );
                }
            }
        }
    }

    /// Minecraft erlaubt `rescale` auch für mehrachsige Drehungen.
    #[test]
    fn rescale_gilt_auch_mehrachsig() {
        for f in rescale_factors([45.0, 45.0, 0.0]) {
            assert!(f.is_finite() && (1.0..=1.7321).contains(&f), "{f}");
        }
    }

    /// Eine Vierteldrehung bildet den Würfel auf sich selbst ab; mit
    /// `rescale` darf er den Einheitswürfel nicht verlassen.
    #[test]
    fn vierteldrehung_mit_rescale_bleibt_im_wuerfel() {
        let rotation = Rotation {
            origin: [8.0, 8.0, 8.0],
            angles: [0.0, 90.0, 0.0],
            rescale: true,
        };
        for ecke in [[0.0, 0.0, 0.0], [16.0, 16.0, 16.0], [0.0, 16.0, 0.0]] {
            let p = rotate_element(ecke, &rotation);
            for a in p {
                assert!((-0.001..=16.001).contains(&a), "{ecke:?} -> {p:?}");
            }
        }
    }

    #[test]
    fn texturdrehung_vertauscht_nur_die_ecken() {
        let c = [[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]];
        assert_eq!(rotate_texture(c, 0), c);
        assert_eq!(rotate_texture(c, 360), c);
        assert_eq!(rotate_texture(c, 90)[0], [16.0, 0.0]);
        assert_eq!(rotate_texture(c, 180)[0], [16.0, 16.0]);
        assert_eq!(rotate_texture(c, 270)[0], [0.0, 16.0]);
    }
}
