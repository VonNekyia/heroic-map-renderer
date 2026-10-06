//! Die Kopien einer Blatttextur für eigene Laubfarben: „hell“ für Bit 24 und
//! „grau“ für die Sorten, die das Spiel nicht tönt. Je Kopie die Farben, die
//! getauscht werden, aus `hell.txt` und `grau.txt`, die Blüten, die ungetönt
//! bleiben, aus `blueten.txt`, und der Tausch je Texel wie
//! `PalettedPermutations` im Client.
//! Siehe docs/benutzung/laubfarben.md, „Wirkung“.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use image::RgbaImage;

/// Welche Kopie einer Blatttextur.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kopie {
    /// Für Laub, das das Spiel nicht tönt, mit eigener Farbe ohne Bit 24.
    Grau,
    /// Für Laub mit Bit 24.
    Hell,
}

/// Die Farben einer Textur, alt zu neu.
pub(crate) type Tausch = HashMap<[u8; 3], [u8; 3]>;

/// Je Textur, etwa `minecraft:block/oak_leaves`, die Paare einer Zeile.
fn tabelle(text: &'static str) -> HashMap<&'static str, Tausch> {
    text.lines()
        .filter_map(|zeile| {
            let mut teile = zeile.split(' ');
            let textur = teile.next()?;
            let farben = teile
                .filter_map(|paar| {
                    let (alt, neu) = paar.split_once('>')?;
                    Some((rgb(alt)?, rgb(neu)?))
                })
                .collect();
            Some((textur, farben))
        })
        .collect()
}

/// Erzeugt mit `laubtabellen.py`, siehe docs/entwicklung/tabellen.md.
static HELL: LazyLock<HashMap<&'static str, Tausch>> =
    LazyLock::new(|| tabelle(include_str!("hell.txt")));
static GRAU: LazyLock<HashMap<&'static str, Tausch>> =
    LazyLock::new(|| tabelle(include_str!("grau.txt")));
static BLUETEN: LazyLock<HashMap<&'static str, HashSet<[u8; 3]>>> = LazyLock::new(|| {
    include_str!("blueten.txt")
        .lines()
        .filter_map(|zeile| {
            let mut teile = zeile.split(' ');
            let textur = teile.next()?;
            Some((textur, teile.filter_map(rgb).collect()))
        })
        .collect()
});

/// `rrggbb` als Farbe.
fn rgb(hex: &str) -> Option<[u8; 3]> {
    let wert = u32::from_str_radix(hex, 16).ok()?;
    Some([(wert >> 16) as u8, (wert >> 8) as u8, wert as u8])
}

/// Die Farben, die diese Kopie der Textur tauscht; `None` für jede andere
/// Textur.
pub(crate) fn tausch(kopie: Kopie, textur: &str) -> Option<&'static Tausch> {
    match kopie {
        Kopie::Hell => HELL.get(textur),
        Kopie::Grau => GRAU.get(textur),
    }
}

/// Die Farben einer Textur, die ungetönt in einer eigenen Ebene bleiben.
pub(crate) fn blueten(textur: &str) -> Option<&'static HashSet<[u8; 3]>> {
    BLUETEN.get(textur)
}

/// Tauscht jede deckende Farbe, die die Tabelle nennt; ein Texel ohne
/// Deckung und eine Farbe, die sie nicht nennt, bleiben, die Deckung auch,
/// denn die neuen Farben sind ganz deckend. Die Blüten nimmt es heraus und
/// gibt sie, falls es welche gibt, als zweites Bild allein.
pub(crate) fn kopie(
    bild: &RgbaImage,
    tausch: &Tausch,
    blueten: Option<&HashSet<[u8; 3]>>,
) -> (RgbaImage, Option<RgbaImage>) {
    let mut kopie = bild.clone();
    let mut nur_blueten = RgbaImage::new(bild.width(), bild.height());
    let mut mit_blueten = false;
    for (x, y, texel) in kopie.enumerate_pixels_mut() {
        let [r, g, b, a] = texel.0;
        if a == 0 {
            continue;
        }
        if blueten.is_some_and(|blueten| blueten.contains(&[r, g, b])) {
            nur_blueten.put_pixel(x, y, *texel);
            texel.0 = [r, g, b, 0];
            mit_blueten = true;
        } else if let Some(&[r, g, b]) = tausch.get(&[r, g, b]) {
            texel.0 = [r, g, b, a];
        }
    }
    (kopie, mit_blueten.then_some(nur_blueten))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sieben getönte Sorten für hell, sieben ungetönte für grau und hell,
    /// die Blüten der blühenden Azalee; Stichproben aus den Tabellen.
    #[test]
    fn tabellen_fuer_vierzehn_blattsorten() {
        assert_eq!((HELL.len(), GRAU.len(), BLUETEN.len()), (14, 7, 1));
        let eiche = tausch(Kopie::Hell, "minecraft:block/oak_leaves").unwrap();
        assert_eq!(eiche.len(), 4);
        assert_eq!(eiche[&[0x68, 0x64, 0x68]], [0xa8, 0xa4, 0xa8]);
        let dschungel = tausch(Kopie::Hell, "minecraft:block/jungle_leaves").unwrap();
        assert_eq!(dschungel[&[0xec, 0xdc, 0xa5]], [0xff, 0xf2, 0xc4]);
        let kirsche = tausch(Kopie::Grau, "minecraft:block/cherry_leaves").unwrap();
        assert_eq!(kirsche.len(), 14);
        assert_eq!(kirsche[&[0xf5, 0xda, 0xef]], [0xbc, 0xbc, 0xbc]);
        let pappel = tausch(Kopie::Hell, "minecraft:block/yellow_poplar_leaves").unwrap();
        assert_eq!(pappel[&[0xab, 0x59, 0x25]], [0xa8, 0xa8, 0xa8]);
        assert!(tausch(Kopie::Grau, "minecraft:block/oak_leaves").is_none());
        let bluete = blueten("minecraft:block/flowering_azalea_leaves").unwrap();
        assert_eq!(bluete.len(), 3);
        assert!(bluete.contains(&[0xba, 0x62, 0xce]));
        assert!(tausch(Kopie::Hell, "minecraft:block/stone").is_none());
    }

    /// Eine genannte Farbe wird getauscht, mit ihrer Deckung; eine andere und
    /// ein Loch bleiben, wie `PalettedPermutations` sie lässt.
    #[test]
    fn tauscht_nur_genannte_farben() {
        let eiche = tausch(Kopie::Hell, "minecraft:block/oak_leaves").unwrap();
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
        let (hell, blueten) = kopie(&bild, eiche, None);
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
        assert!(blueten.is_none());
    }

    /// Die Blüten der blühenden Azalee gehen aus der grauen Kopie heraus in
    /// ein Bild für sich, unverändert; die Blätter werden grau.
    #[test]
    fn blueten_in_eigener_ebene() {
        let name = "minecraft:block/flowering_azalea_leaves";
        let grau = tausch(Kopie::Grau, name).unwrap();
        let bild =
            RgbaImage::from_raw(2, 1, vec![0x70, 0x92, 0x2d, 255, 0xba, 0x62, 0xce, 255]).unwrap();
        let (kopie, blueten) = kopie(&bild, grau, blueten(name));
        assert_eq!(kopie.get_pixel(0, 0).0, [0xbc, 0xbc, 0xbc, 255]);
        assert_eq!(kopie.get_pixel(1, 0).0[3], 0);
        let blueten = blueten.unwrap();
        assert_eq!(blueten.get_pixel(0, 0).0[3], 0);
        assert_eq!(blueten.get_pixel(1, 0).0, [0xba, 0x62, 0xce, 255]);
    }
}
