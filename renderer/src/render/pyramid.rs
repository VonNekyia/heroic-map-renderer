//! Die Zoomstufen über der gerenderten Kachelebene.
//!
//! Jede gröbere Stufe entsteht aus vier Kacheln der feineren, auf die halbe
//! Kantenlänge gestaucht. Gerendert wird nur die feinste Stufe — alles
//! darüber ist Bildverkleinerung und kostet keinen Weltzugriff.

use std::collections::BTreeSet;

use image::{Rgba, RgbaImage};
use std::sync::LazyLock;

use serde::Serialize;

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
        let (qx, qy) = child.quadrant();
        let klein = shrink(image);
        for (x, y, pixel) in klein.enumerate_pixels() {
            out.put_pixel(qx * half + x, qy * half + y, *pixel);
        }
    }
    out
}

/// Halbiert die Kantenlänge eines Bildes.
///
/// Gemittelt wird mit vormultipliziertem Alpha. Geradeaus gemittelt zögen
/// durchsichtige Pixel ihre Farbe in die Nachbarn, und jede Kante gegen
/// Luft bekäme einen dunklen Saum — auf einer Karte voller Blattwerk und
/// Zäune wäre das überall zu sehen.
///
/// Gemittelt wird ausserdem in linearem Licht, nicht in sRGB-Werten: die
/// sind gammakodiert, und ihr Mittel ist zu dunkel. Halb Schwarz, halb
/// Weiss ergibt so 188 statt 128 — kontrastreiche Texturen fallen beim
/// Herauszoomen sonst zusammen, und jede Stufe verdunkelt weiter.
pub fn shrink(image: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(image.width() / 2, image.height() / 2);
    for (x, y, ziel) in out.enumerate_pixels_mut() {
        let mut farbe = [0.0f32; 3];
        let mut alpha = 0u32;
        for dy in 0..2 {
            for dx in 0..2 {
                let pixel = image.get_pixel(2 * x + dx, 2 * y + dy).0;
                let a = pixel[3] as u32;
                alpha += a;
                for (summe, &wert) in farbe.iter_mut().zip(&pixel[..3]) {
                    *summe += LINEAR[wert as usize] * a as f32;
                }
            }
        }
        // Ohne Deckung gibt es keine Farbe zu mitteln, und das Pixel ist
        // ohnehin durchsichtig.
        if alpha == 0 {
            *ziel = Rgba([0, 0, 0, 0]);
            continue;
        }
        let mittel = |summe: f32| to_srgb(summe / alpha as f32);
        *ziel = Rgba([
            mittel(farbe[0]),
            mittel(farbe[1]),
            mittel(farbe[2]),
            alpha.div_ceil(4) as u8,
        ]);
    }
    out
}

/// sRGB-Wert nach linearem Licht, als Tabelle: die Pyramide läuft über
/// jedes Pixel jeder Stufe.
static LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// Lineares Licht zurück nach sRGB.
fn to_srgb(linear: f32) -> u8 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Was das Frontend über die Karte wissen muss.
///
/// Die Projektion selbst steht nicht drin: sie hängt allein an `scale`,
/// und die Formel gehört in den Renderer, nicht in eine Datei.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
