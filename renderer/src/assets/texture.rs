use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use image::RgbaImage;

use super::{find_file, split_id};

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
    pub fn load(&mut self, roots: &[PathBuf], id: &str) -> TextureId {
        if let Some(&existing) = self.ids.get(id) {
            return existing;
        }

        let (namespace, name) = split_id(id);
        let loaded = find_file(roots, namespace, "textures", name, "png")
            .and_then(|path| read_texture(&path));

        let texture = match loaded {
            Some(image) => {
                self.images.push(image);
                self.names.push(id.to_string());
                TextureId(self.images.len() as u32 - 1)
            }
            None => {
                self.missing.insert(id.to_string());
                Textures::MISSING
            }
        };
        self.ids.insert(id.to_string(), texture);
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

/// Lädt eine PNG-Datei und schneidet bei animierten Texturen das erste Bild
/// heraus.
fn read_texture(path: &Path) -> Option<RgbaImage> {
    let image = image::open(path).ok()?.into_rgba8();

    // Animierte Texturen sind senkrechte Streifen; erkennbar an der
    // .mcmeta-Datei daneben. Ohne sie wären hohe Texturen wie die von
    // Truhen nicht von Animationen zu unterscheiden.
    let meta = PathBuf::from(format!("{}.mcmeta", path.display()));
    if meta.is_file() && image.height() > image.width() {
        let size = image.width();
        return Some(image::imageops::crop_imm(&image, 0, 0, size, size).to_image());
    }
    Some(image)
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut textures = Textures::new();
        let id = textures.load(&[PathBuf::from("gibt/es/nicht")], "minecraft:block/nope");
        assert_eq!(id, Textures::MISSING);
        assert!(textures.missing().contains("minecraft:block/nope"));
        // zweiter Aufruf trifft den Cache
        assert_eq!(
            textures.load(&[], "minecraft:block/nope"),
            Textures::MISSING
        );
        assert_eq!(textures.missing().len(), 1);
    }

    #[test]
    fn unbekannte_id_liefert_den_platzhalter() {
        let textures = Textures::new();
        assert_eq!(textures.image(TextureId(999)).dimensions(), (16, 16));
        assert_eq!(textures.name(TextureId(999)), "<unbekannt>");
    }
}
