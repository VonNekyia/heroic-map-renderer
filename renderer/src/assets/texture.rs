use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use image::RgbaImage;
use serde::Deserialize;

use super::{Pack, find_file, read_text, split_id};

/// Verweis in die Texturtabelle. Id 0 ist immer der Platzhalter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(pub u32);

/// Alle Texturen, einmal geladen.
///
/// Im Renderpfad gibt es danach keine Dateizugriffe mehr — nur noch Indizes
/// in diese Tabelle.
pub struct Textures {
    ids: HashMap<String, TextureId>,
    images: Vec<RgbaImage>,
    /// Parallel zu `images`, damit Fehlermeldungen den Namen nennen können.
    names: Vec<String>,
    missing: BTreeSet<String>,
}

impl Default for Textures {
    fn default() -> Self {
        Self::new()
    }
}

impl Textures {
    pub fn new() -> Textures {
        Textures {
            ids: HashMap::new(),
            images: vec![placeholder()],
            names: vec!["<fehlende Textur>".to_string()],
            missing: BTreeSet::new(),
        }
    }

    /// Platzhalter für nicht auffindbare Texturen.
    pub const MISSING: TextureId = TextureId(0);

    /// Lädt eine Textur oder liefert die bereits geladene.
    ///
    /// Eine fehlende Datei ist kein Fehler: Minecraft zeigt dafür das
    /// magenta-schwarze Karo, und ein halb vollständiges Pack soll einen
    /// Renderlauf nicht abbrechen. Die Namen sammelt [`Textures::missing`].
    /// Ohne Namensraum gilt `minecraft`, wie im Client: `block/stone` ist
    /// dieselbe Textur wie `minecraft:block/stone`.
    pub fn load(&mut self, packs: &[Pack], id: &str) -> TextureId {
        let (namespace, name) = split_id(id);
        let id = format!("{namespace}:{name}");
        if let Some(&existing) = self.ids.get(&id) {
            return existing;
        }

        let loaded = find_file(packs, namespace, "textures", name, "png")
            .and_then(|(layer, path)| read_texture(packs, layer, namespace, name, path));

        let texture = match loaded {
            Some(image) => {
                self.images.push(image);
                self.names.push(id.clone());
                TextureId(self.images.len() as u32 - 1)
            }
            None => {
                self.missing.insert(id.clone());
                Textures::MISSING
            }
        };
        self.ids.insert(id, texture);
        texture
    }

    pub fn image(&self, id: TextureId) -> &RgbaImage {
        self.images
            .get(id.0 as usize)
            .unwrap_or(&self.images[Textures::MISSING.0 as usize])
    }

    /// Name einer geladenen Textur.
    pub fn name(&self, id: TextureId) -> &str {
        self.names.get(id.0 as usize).map_or("<unbekannt>", |s| s)
    }

    /// Texturnamen, für die keine Datei gefunden wurde.
    pub fn missing(&self) -> &BTreeSet<String> {
        &self.missing
    }

    /// Anzahl geladener Texturen, den Platzhalter eingerechnet.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

/// Lädt eine PNG-Datei und schneidet bei animierten Texturen das erste
/// deklarierte Bild heraus.
fn read_texture(
    packs: &[Pack],
    png_layer: usize,
    namespace: &str,
    name: &str,
    path: &Path,
) -> Option<RgbaImage> {
    let image = image::open(path).ok()?.into_rgba8();
    match animation(packs, png_layer, namespace, name) {
        Some(animation) => Some(first_frame(&image, &animation)),
        None => Some(image),
    }
}

/// Sucht die `.mcmeta`-Datei im Packstapel und liest den `animation`-Teil.
///
/// Minecraft nimmt Metadaten aus derselben oder einer höher priorisierten
/// Schicht als die PNG-Datei — ein Overlay darf also allein die `.mcmeta`
/// mitbringen. Gepaart wird über den aufgelisteten Namen, wie in
/// `FallbackResourceManager.listResources`. Ohne `animation` ist die
/// Textur statisch, auch wenn die Datei existiert: 48 der Vanilla-mcmeta
/// enthalten nur `texture`-Flags.
fn animation(packs: &[Pack], png_layer: usize, namespace: &str, name: &str) -> Option<Animation> {
    let (_, meta) = find_file(
        &packs[png_layer..],
        namespace,
        "textures",
        name,
        "png.mcmeta",
    )?;

    let text = read_text(meta).ok()?;
    serde_json::from_str::<McMeta>(&text).ok()?.animation
}

/// Schneidet das erste deklarierte Bild einer Animation heraus.
fn first_frame(image: &RgbaImage, animation: &Animation) -> RgbaImage {
    let (width, height) = image.dimensions();

    // Ohne Angabe ist ein Bild so breit wie die Textur und ebenso hoch —
    // der senkrechte Streifen, den Minecraft dokumentiert.
    let frame_width = animation.width.unwrap_or(width).clamp(1, width);
    let frame_height = animation.height.unwrap_or(frame_width).clamp(1, height);

    let columns = width / frame_width;
    let rows = height / frame_height;
    if columns == 0 || rows == 0 {
        return image.clone();
    }

    // `frames` gibt die Abspielreihenfolge an; das erste Bild darin ist nicht
    // zwingend Nummer 0. Vanilla nutzt das in fire_0 und soul_fire_0.
    let index = animation.frames.first().map_or(0, Frame::index);
    let index = if index < columns * rows { index } else { 0 };

    image::imageops::crop_imm(
        image,
        (index % columns) * frame_width,
        (index / columns) * frame_height,
        frame_width,
        frame_height,
    )
    .to_image()
}

/// Das magenta-schwarze Karo, das Minecraft für fehlende Texturen zeigt.
fn placeholder() -> RgbaImage {
    RgbaImage::from_fn(16, 16, |x, y| {
        if (x < 8) == (y < 8) {
            image::Rgba([0, 0, 0, 255])
        } else {
            image::Rgba([248, 0, 248, 255])
        }
    })
}

#[derive(Deserialize)]
struct McMeta {
    animation: Option<Animation>,
}

#[derive(Deserialize)]
struct Animation {
    width: Option<u32>,
    height: Option<u32>,
    #[serde(default)]
    frames: Vec<Frame>,
}

/// Ein Eintrag in `frames`: entweder eine Nummer oder ein Objekt mit eigener
/// Anzeigedauer.
#[derive(Deserialize)]
#[serde(untagged)]
enum Frame {
    Index(u32),
    Detailed { index: u32 },
}

impl Frame {
    fn index(&self) -> u32 {
        match self {
            Frame::Index(i) => *i,
            Frame::Detailed { index } => *index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jede 16 Pixel hohe Zeile bekommt ihre Nummer als Rotwert.
    fn streifen(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |_, y| {
            image::Rgba([(y / 16) as u8, 0, 0, 255])
        })
    }

    fn animation(json: &str) -> Animation {
        serde_json::from_str::<McMeta>(json)
            .unwrap()
            .animation
            .unwrap()
    }

    #[test]
    fn platzhalter_ist_id_null() {
        let textures = Textures::new();
        assert_eq!(textures.len(), 1);
        let image = textures.image(Textures::MISSING);
        assert_eq!(image.dimensions(), (16, 16));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(8, 0).0, [248, 0, 248, 255]);
    }

    #[test]
    fn fehlende_textur_wird_gesammelt_statt_zu_scheitern() {
        let leer = tempfile::tempdir().unwrap();
        let packs = [Pack::open(leer.path(), &super::super::pack::ASSETS).unwrap()];
        let mut textures = Textures::new();
        let id = textures.load(&packs, "minecraft:block/nope");
        assert_eq!(id, Textures::MISSING);
        assert!(textures.missing().contains("minecraft:block/nope"));
        // zweiter Aufruf trifft den Cache, auch ohne Namensraum
        assert_eq!(textures.load(&[], "block/nope"), Textures::MISSING);
        assert_eq!(textures.missing().len(), 1);
    }

    #[test]
    fn unbekannte_id_liefert_den_platzhalter() {
        let textures = Textures::new();
        assert_eq!(textures.image(TextureId(999)).dimensions(), (16, 16));
        assert_eq!(textures.name(TextureId(999)), "<unbekannt>");
    }

    #[test]
    fn senkrechter_streifen_ohne_angaben() {
        let frame = first_frame(&streifen(16, 48), &animation(r#"{"animation": {}}"#));
        assert_eq!(frame.dimensions(), (16, 16));
        assert_eq!(frame.get_pixel(0, 0).0[0], 0);
    }

    /// Vanilla-fire_0 beginnt mit Bild 16, nicht mit 0.
    #[test]
    fn erstes_deklariertes_bild_gewinnt() {
        let frame = first_frame(
            &streifen(16, 512),
            &animation(r#"{"animation": {"frames": [16, 17, 0]}}"#),
        );
        assert_eq!(frame.dimensions(), (16, 16));
        assert_eq!(frame.get_pixel(0, 0).0[0], 16);
    }

    #[test]
    fn frames_als_objekte() {
        let frame = first_frame(
            &streifen(16, 512),
            &animation(r#"{"animation": {"frames": [{"index": 3, "time": 2}]}}"#),
        );
        assert_eq!(frame.get_pixel(0, 0).0[0], 3);
    }

    #[test]
    fn eigene_bildgroesse() {
        let frame = first_frame(
            &streifen(16, 32),
            &animation(r#"{"animation": {"height": 8}}"#),
        );
        assert_eq!(frame.dimensions(), (16, 8));
    }

    /// Waagerecht angeordnete Bilder: 32x16 mit 16x16-Bildern.
    #[test]
    fn waagerechte_anordnung() {
        let frame = first_frame(
            &streifen(32, 16),
            &animation(r#"{"animation": {"width": 16, "height": 16}}"#),
        );
        assert_eq!(frame.dimensions(), (16, 16));
    }

    #[test]
    fn unsinnige_angaben_brechen_nicht() {
        let frame = first_frame(
            &streifen(16, 16),
            &animation(r#"{"animation": {"width": 999, "frames": [42]}}"#),
        );
        assert_eq!(frame.dimensions(), (16, 16));
    }

    /// 48 der Vanilla-mcmeta enthalten nur `texture`-Flags. Solche Texturen
    /// sind statisch und dürfen nicht zugeschnitten werden.
    #[test]
    fn mcmeta_ohne_animation_ist_keine_animation() {
        let meta: McMeta = serde_json::from_str(r#"{"texture": {"blur": true}}"#).unwrap();
        assert!(meta.animation.is_none());
    }
}
