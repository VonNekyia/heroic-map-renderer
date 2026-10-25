//! Das Relief von oben, `--relief`: Hänge hell oder dunkel nach dem
//! Gefälle der Oberfläche, wahlweise mit Schatten, nur `top-north` in
//! Richtung `s` mit der Karte.
//! Siehe docs/renderer/relief.md.

use anyhow::Result;
use image::RgbaImage;

use super::{ChunkCache, ScreenRect};

/// Was `--relief` zeichnet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relief {
    /// `slope`: jede Spalte heller oder dunkler nach dem Gefälle zu ihren
    /// Nachbarn.
    Gefaelle,
    /// `shadows`: dazu Schatten, wo eine höhere Spalte im Nordwesten die
    /// Sonne verdeckt.
    Schatten,
}

/// So viel heller je Block Gefälle zum Licht hin, so viel dunkler davon weg.
const JE_BLOCK: f32 = 0.12;
/// Höchstens so viel heller oder dunkler durch das Gefälle.
const HOECHSTENS: f32 = 0.24;
/// So viele Blöcke diagonal nach Nordwesten sucht der Schatten eine höhere
/// Spalte.
const REICHWEITE: i32 = 32;
/// So viel Licht behält eine Spalte im Schatten.
const IM_SCHATTEN: f32 = 0.75;

impl Relief {
    pub fn parse(text: &str) -> Result<Relief, String> {
        match text {
            "slope" => Ok(Relief::Gefaelle),
            "shadows" => Ok(Relief::Schatten),
            _ => Err(format!("unbekanntes Relief {text}: slope oder shadows")),
        }
    }

    /// Der Name wie in `--relief` und `map.json`.
    pub fn name(self) -> &'static str {
        match self {
            Relief::Gefaelle => "slope",
            Relief::Schatten => "shadows",
        }
    }

    /// Wie viele Chunks nach Osten und Süden eine Änderung das Relief
    /// anderer Spalten ändert: so weit, wie der Schatten sucht,
    /// [`REICHWEITE`]. Auch mit `slope`, denn die Schicht trägt den Schatten.
    /// Siehe docs/benutzung/updates.md, „Wo ein Update zeichnet“.
    pub fn chunks_nach_suedosten(self) -> i32 {
        (REICHWEITE + 15) / 16
    }
}

/// Der Code einer Spalte in der Schicht: die Stufe des Gefälles `n` in
/// −4…4, um 4 verschoben, dazu 9, wenn sie im Schatten liegt. 4 heisst
/// unverändert.
/// Siehe docs/renderer/relief.md, „Die Schicht“.
fn code([west, ost, nord, sued]: [i32; 4], schatten: bool) -> u8 {
    let n = (ost - west + sued - nord).clamp(-4, 4);
    (n + 4) as u8 + 9 * u8::from(schatten)
}

/// Der Code, der nichts ändert.
pub const UNVERAENDERT: u8 = 4;

/// Die Helligkeit einer Spalte aus den Höhen ihrer Nachbarn im Westen,
/// Osten, Norden und Süden: Steigt die Oberfläche nach Südosten, schaut sie
/// zum Licht aus Nordwesten und wird heller.
/// Siehe docs/renderer/relief.md, „Gefälle“.
fn gefaelle([west, ost, nord, sued]: [i32; 4]) -> f32 {
    let steigt = (ost - west + sued - nord) as f32 / 2.0;
    (1.0 + JE_BLOCK * steigt).clamp(1.0 - HOECHSTENS, 1.0 + HOECHSTENS)
}

/// Ob eine Spalte der Höhe `h` im Schatten liegt: ob `k` Blöcke diagonal
/// nach Nordwesten, für ein `k` bis [`REICHWEITE`], eine Spalte über den
/// Strahl zur Sonne unter 45° ragt.
/// Siehe docs/renderer/relief.md, „Schatten“.
fn im_schatten(h: i32, nach_nordwesten: impl Fn(i32) -> Option<i32>) -> bool {
    (1..=REICHWEITE).any(|k| {
        nach_nordwesten(k)
            .is_some_and(|hk| f64::from(hk - h) > f64::from(k) * std::f64::consts::SQRT_2)
    })
}

impl ChunkCache<'_> {
    /// Die Oberfläche des Chunks `cx`, `cz` aus dem Cache, geladen falls
    /// nötig; `None` ohne Chunk. In Richtung `s` ist der Blick die Welt.
    fn oberflaeche(&mut self, cx: i32, cz: i32) -> Result<Option<[Option<i32>; 256]>> {
        let i = self.slot((cx, cz))?;
        Ok(self.slots[i].loaded.as_ref().map(|l| l.chunk.surface()))
    }
}

/// Legt das Relief auf das Bild des Ausschnitts `rect`: je Block der
/// Oberfläche ein Faktor auf Rot, Grün und Blau seiner Pixel, Alpha bleibt.
/// Die Höhen kommen aus `WORLD_SURFACE` der Chunks, ein Pixel hängt so nur
/// an der Welt, nicht am Ausschnitt. Liefert je Pixel des Bilds den Code
/// der Schicht ([`code`]), mit Schatten, auch wenn `relief` keinen zeichnet,
/// und den Rest: die Farbe vor dem Relief an jedem Pixel, den es bei 255
/// abschneidet, mit Alpha 255, sonst durchsichtig; `None` ohne solche.
/// Siehe docs/renderer/relief.md, „Höhen“.
pub(super) fn zeichne(
    chunks: &mut ChunkCache,
    bild: &mut RgbaImage,
    rect: ScreenRect,
    relief: Relief,
) -> Result<(Vec<u8>, Option<RgbaImage>)> {
    let p = chunks.sprites.projection();
    let o = p.project_block([0, 0, 0]);
    let (sx, sz) = (
        p.project_block([1, 0, 0]).0 - o.0,
        p.project_block([0, 0, 1]).1 - o.1,
    );
    let block = |v: i32, null: f64, je: f64| ((f64::from(v) - null) / je).floor() as i32;
    let (bx0, bz0) = (block(rect.x, o.0, sx), block(rect.y, o.1, sz));
    let (bx1, bz1) = (
        block(rect.right() - 1, o.0, sx),
        block(rect.bottom() - 1, o.1, sz),
    );
    // Die Höhen samt Rand: ein Block nach Osten und Süden, nach Nordwesten so
    // weit, wie der Schatten sucht.
    let r = REICHWEITE;
    let (gx0, gz0, gx1, gz1) = (bx0 - r, bz0 - r, bx1 + 1, bz1 + 1);
    let breite = (gx1 - gx0 + 1) as usize;
    let mut hoehen = vec![None; breite * (gz1 - gz0 + 1) as usize];
    for cz in gz0.div_euclid(16)..=gz1.div_euclid(16) {
        for cx in gx0.div_euclid(16)..=gx1.div_euclid(16) {
            let Some(oberflaeche) = chunks.oberflaeche(cx, cz)? else {
                continue;
            };
            for (i, h) in oberflaeche.into_iter().enumerate() {
                let (x, z) = (cx * 16 + (i % 16) as i32, cz * 16 + (i / 16) as i32);
                if (gx0..=gx1).contains(&x) && (gz0..=gz1).contains(&z) {
                    hoehen[(z - gz0) as usize * breite + (x - gx0) as usize] = h;
                }
            }
        }
    }
    let an = |x: i32, z: i32| hoehen[(z - gz0) as usize * breite + (x - gx0) as usize];
    let (w, h) = (bild.width() as i32, bild.height() as i32);
    let mut codes = vec![UNVERAENDERT; (w * h) as usize];
    let mut rest: Option<RgbaImage> = None;
    for z in bz0..=bz1 {
        for x in bx0..=bx1 {
            let Some(h0) = an(x, z) else {
                continue;
            };
            // Ohne Nachbarn gilt er als gleich hoch.
            let um = |dx: i32, dz: i32| an(x + dx, z + dz).unwrap_or(h0);
            let nachbarn = [um(-1, 0), um(1, 0), um(0, -1), um(0, 1)];
            let schatten = im_schatten(h0, |k| an(x - k, z - k));
            let mut faktor = gefaelle(nachbarn);
            if relief == Relief::Schatten && schatten {
                faktor *= IM_SCHATTEN;
            }
            let px0 = (o.0 + f64::from(x) * sx).round() as i32 - rect.x;
            let pz0 = (o.1 + f64::from(z) * sz).round() as i32 - rect.y;
            let code = code(nachbarn, schatten);
            let (spalten, zeilen) = (
                px0.max(0)..(px0 + sx as i32).min(w),
                pz0.max(0)..(pz0 + sz as i32).min(h),
            );
            if code != UNVERAENDERT && !spalten.is_empty() {
                for py in zeilen.clone() {
                    let zeile = (py * w) as usize;
                    codes[zeile + spalten.start as usize..zeile + spalten.end as usize].fill(code);
                }
            }
            if faktor == 1.0 {
                continue;
            }
            for py in zeilen {
                for px in spalten.clone() {
                    let pixel = bild.get_pixel_mut(px as u32, py as u32);
                    if pixel[3] == 0 {
                        continue;
                    }
                    let [r, g, b, _] = pixel.0;
                    let mut geschnitten = false;
                    for c in &mut pixel.0[..3] {
                        let neu = (f32::from(*c) * faktor).round();
                        geschnitten |= neu > 255.0;
                        *c = neu.min(255.0) as u8;
                    }
                    if geschnitten {
                        rest.get_or_insert_with(|| RgbaImage::new(w as u32, h as u32))
                            .put_pixel(px as u32, py as u32, image::Rgba([r, g, b, 255]));
                    }
                }
            }
        }
    }
    Ok((codes, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_und_parse_passen() {
        for relief in [Relief::Gefaelle, Relief::Schatten] {
            assert_eq!(Relief::parse(relief.name()), Ok(relief));
        }
        assert!(Relief::parse("none").is_err());
    }

    /// Zum Licht aus Nordwesten heller, davon weg dunkler, eben gleich,
    /// gekappt.
    #[test]
    fn gefaelle_zum_licht_hell() {
        assert_eq!(gefaelle([64, 64, 64, 64]), 1.0);
        // Nach Osten einen Block höher: Die Spalte schaut nach Westen.
        assert!((gefaelle([64, 65, 64, 64]) - 1.06).abs() < 1e-6);
        assert!((gefaelle([64, 64, 65, 64]) - 0.94).abs() < 1e-6);
        assert_eq!(gefaelle([60, 70, 60, 70]), 1.0 + HOECHSTENS);
        assert_eq!(gefaelle([70, 60, 70, 60]), 1.0 - HOECHSTENS);
    }

    /// Der Code: eben 4, die Stufe des Gefälles zwischen 0 und 8 begrenzt
    /// wie der Faktor, im Schatten 9 dazu.
    #[test]
    fn code_der_schicht() {
        assert_eq!(code([64, 64, 64, 64], false), UNVERAENDERT);
        assert_eq!(code([64, 65, 64, 64], false), 5);
        assert_eq!(code([64, 64, 65, 64], false), 3);
        assert_eq!(code([60, 70, 60, 70], false), 8);
        assert_eq!(code([70, 60, 70, 60], false), 0);
        assert_eq!(code([64, 64, 64, 64], true), 13);
        assert_eq!(code([70, 60, 70, 60], true), 9);
        // Wo der Code begrenzt, ist es auch der Faktor.
        assert_eq!(gefaelle([64, 68, 64, 64]), gefaelle([60, 70, 60, 70]));
    }

    /// Schatten nur, wo eine Spalte im Nordwesten über 45° ragt, bis
    /// [`REICHWEITE`] Blöcke weit.
    #[test]
    fn schatten_unter_45_grad() {
        let turm = |hoch: i32, bei: i32| move |k: i32| Some(if k == bei { hoch } else { 64 });
        // 3 Blöcke diagonal sind 4,24 Blöcke weit.
        assert!(im_schatten(64, turm(69, 3)));
        assert!(!im_schatten(64, turm(68, 3)));
        assert!(im_schatten(64, turm(64 + 46, REICHWEITE)));
        assert!(!im_schatten(64, turm(400, REICHWEITE + 1)));
        assert!(!im_schatten(64, |_| None));
    }
}
