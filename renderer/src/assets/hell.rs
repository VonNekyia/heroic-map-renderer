//! Die hellere Blatttextur für Bit 24 einer eigenen Laubfarbe: je Textur die
//! Farben, die das Spiel dann tauscht, aus `hell.txt`, und der Tausch je
//! Texel wie `PalettedPermutations` im Client.
//! Siehe docs/benutzung/laubfarben.md, „Wirkung“.

use std::collections::HashMap;
use std::sync::LazyLock;

use image::RgbaImage;

/// Die Farben einer Textur, alt zu hell.
pub(crate) type Tausch = HashMap<[u8; 3], [u8; 3]>;

/// Je Blatttextur, etwa `minecraft:block/oak_leaves`, ihr [`Tausch`].
/// Erzeugt mit `hell.py`, siehe docs/entwicklung/tabellen.md.
static TAUSCH: LazyLock<HashMap<&'static str, Tausch>> = LazyLock::new(|| {
    include_str!("hell.txt")
        .lines()
        .filter_map(|zeile| {
            let mut teile = zeile.split(' ');
            let textur = teile.next()?;
            let farben = teile
                .filter_map(|paar| {
                    let (alt, hell) = paar.split_once('>')?;
                    Some((rgb(alt)?, rgb(hell)?))
                })
                .collect();
            Some((textur, farben))
        })
        .collect()
});

/// `rrggbb` als Farbe.
fn rgb(hex: &str) -> Option<[u8; 3]> {
    let wert = u32::from_str_radix(hex, 16).ok()?;
    Some([(wert >> 16) as u8, (wert >> 8) as u8, wert as u8])
}

/// Die Farben, die das Spiel in dieser Textur tauscht; `None` für jede
/// andere Textur.
pub(crate) fn tausch(textur: &str) -> Option<&'static Tausch> {
    TAUSCH.get(textur)
}

/// Tauscht jede deckende Farbe, die die Tabelle nennt; ein Texel ohne
/// Deckung und eine Farbe, die sie nicht nennt, bleiben. Die Deckung bleibt,
/// denn die hellen Farben sind ganz deckend.
pub(crate) fn hell(bild: &RgbaImage, tausch: &Tausch) -> RgbaImage {
    let mut hell = bild.clone();
    for texel in hell.pixels_mut() {
        let [r, g, b, a] = texel.0;
        if a == 0 {
            continue;
        }
        if let Some(&[r, g, b]) = tausch.get(&[r, g, b]) {
            texel.0 = [r, g, b, a];
        }
    }
    hell
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sieben Blattsorten, mit Stichproben aus der Tabelle.
    #[test]
    fn tabelle_fuer_sieben_blattsorten() {
        assert_eq!(TAUSCH.len(), 7);
        let eiche = tausch("minecraft:block/oak_leaves").unwrap();
        assert_eq!(eiche.len(), 4);
        assert_eq!(eiche[&[0x68, 0x64, 0x68]], [0xa8, 0xa4, 0xa8]);
        let dschungel = tausch("minecraft:block/jungle_leaves").unwrap();
        assert_eq!(dschungel[&[0xec, 0xdc, 0xa5]], [0xff, 0xf2, 0xc4]);
        assert!(tausch("minecraft:block/stone").is_none());
    }

    /// Eine genannte Farbe wird getauscht, mit ihrer Deckung; eine andere und
    /// ein Loch bleiben, wie `PalettedPermutations` sie lässt.
    #[test]
    fn tauscht_nur_genannte_farben() {
        let eiche = tausch("minecraft:block/oak_leaves").unwrap();
        let bild = RgbaImage::from_raw(
            4,
            1,
            vec![
                0x68, 0x64, 0x68, 255, //
                0x68, 0x64, 0x68, 128, //
                0x11, 0x22, 0x33, 255, //
                0x68, 0x64, 0x68, 0, //
            ],
        )
        .unwrap();
        let hell = hell(&bild, eiche);
        let texel: Vec<[u8; 4]> = hell.pixels().map(|p| p.0).collect();
        assert_eq!(
            texel,
            [
                [0xa8, 0xa4, 0xa8, 255],
                [0xa8, 0xa4, 0xa8, 128],
                [0x11, 0x22, 0x33, 255],
                [0x68, 0x64, 0x68, 0],
            ]
        );
    }
}
