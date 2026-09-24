use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};
use image::RgbaImage;
use serde_json::Value;

use super::blockstate::{boolean, field, float, int};
use super::{Pack, find_file, parse_json, read_text, split_id};

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
    broken: BTreeMap<String, String>,
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
            broken: BTreeMap::new(),
        }
    }

    /// Platzhalter für nicht auffindbare Texturen.
    pub const MISSING: TextureId = TextureId(0);

    /// Lädt eine Textur oder liefert die bereits geladene.
    ///
    /// Eine fehlende Datei ist kein Fehler: Minecraft zeigt dafür das
    /// magenta-schwarze Karo, und ein halb vollständiges Pack soll einen
    /// Renderlauf nicht abbrechen. Ebenso eine, die der Client verwirft
    /// ([`read_texture`]). Die Namen sammelt [`Textures::missing`], die
    /// verworfenen mit Grund [`Textures::broken`]. Ohne Namensraum gilt
    /// `minecraft`, wie im Client: `block/stone` ist dieselbe Textur wie
    /// `minecraft:block/stone`.
    pub fn load(&mut self, packs: &[Pack], id: &str) -> TextureId {
        let (namespace, name) = split_id(id);
        let id = format!("{namespace}:{name}");
        if let Some(&existing) = self.ids.get(&id) {
            return existing;
        }

        let loaded = find_file(packs, namespace, "textures", name, "png")
            .map(|(layer, path)| read_texture(packs, layer, namespace, name, path));

        let texture = match loaded {
            Some(Ok(image)) => {
                self.images.push(image);
                self.names.push(id.clone());
                TextureId(self.images.len() as u32 - 1)
            }
            Some(Err(grund)) => {
                self.missing.insert(id.clone());
                self.broken.insert(id.clone(), format!("{grund:#}"));
                Textures::MISSING
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

    /// Texturnamen, die das magenta-schwarze Karo zeigen: ohne Datei oder
    /// mit einer, die der Client verwirft.
    pub fn missing(&self) -> &BTreeSet<String> {
        &self.missing
    }

    /// Die verworfenen unter [`Textures::missing`], je Name mit dem Grund.
    pub fn broken(&self) -> &BTreeMap<String, String> {
        &self.broken
    }

    /// Anzahl geladener Texturen, den Platzhalter eingerechnet.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

/// Lädt eine PNG-Datei und schneidet bei animierten Texturen das Bild
/// heraus, das der Client zuerst zeigt. Wie `SpriteResourceLoader` wird
/// eine Textur zur Missing-Textur, wenn ihre `.mcmeta` nicht zu lesen ist
/// oder die Bildgrösse nicht zur Animation passt; dann `Err` mit dem Grund.
fn read_texture(
    packs: &[Pack],
    png_layer: usize,
    namespace: &str,
    name: &str,
    path: &Path,
) -> Result<RgbaImage> {
    let image = image::open(path)
        .with_context(|| format!("{} lesen", path.display()))?
        .into_rgba8();
    match animation(packs, png_layer, namespace, name)? {
        Some(animation) => erstes_bild(&image, &animation),
        None => Ok(image),
    }
}

/// Sucht die `.mcmeta`-Datei im Packstapel und liest sie.
///
/// Minecraft nimmt Metadaten aus derselben oder einer höher priorisierten
/// Schicht als die PNG-Datei — ein Overlay darf also allein die `.mcmeta`
/// mitbringen. Gepaart wird über den aufgelisteten Namen, wie in
/// `FallbackResourceManager.listResources`. Ohne `animation` ist die
/// Textur statisch, auch wenn die Datei existiert: 48 der Vanilla-mcmeta
/// enthalten nur `texture`-Flags.
fn animation(
    packs: &[Pack],
    png_layer: usize,
    namespace: &str,
    name: &str,
) -> Result<Option<Animation>> {
    let Some((_, meta)) = find_file(
        &packs[png_layer..],
        namespace,
        "textures",
        name,
        "png.mcmeta",
    ) else {
        return Ok(None);
    };
    mcmeta(&read_text(meta)?).with_context(|| format!("{} lesen", meta.display()))
}

/// Die Angaben aus `animation`, die das erste Bild bestimmen.
#[derive(Debug)]
struct Animation {
    width: Option<u32>,
    height: Option<u32>,
    /// Die Nummern aus `frames`, falls es die Liste gibt.
    frames: Option<Vec<u32>>,
}

/// Eine `.mcmeta` wie `ResourceMetadata.fromJsonStream`: ein Objekt, streng
/// gelesen wie `GsonHelper.parse`. Die Abschnitte `animation` und
/// `texture` liest je ein Codec (`getSection`). Ein Abschnitt `null` ist
/// kein fehlender, er geht an den Codec und scheitert. Andere Abschnitte
/// liest der Block-Atlas nicht.
fn mcmeta(text: &str) -> Result<Option<Animation>> {
    let json = parse_json(text, false)?;
    ensure!(json.is_object(), "kein Objekt");
    if let Some(textur) = json.get("texture") {
        texture_section(textur).context("texture")?;
    }
    json.get("animation")
        .map(|animation| animation_section(animation).context("animation"))
        .transpose()
}

/// `TextureMetadataSection.CODEC`: alles darf fehlen, und was dasteht,
/// muss passen.
fn texture_section(json: &Value) -> Result<()> {
    ensure!(json.is_object(), "kein Objekt");
    for name in ["blur", "clamp"] {
        if let Some(wert) = field(json, name) {
            boolean(wert).with_context(|| name.to_string())?;
        }
    }
    if let Some(wert) = field(json, "mipmap_strategy") {
        ensure!(
            matches!(
                wert.as_str(),
                Some("auto" | "mean" | "cutout" | "strict_cutout" | "dark_cutout")
            ),
            "mipmap_strategy {wert}"
        );
    }
    if let Some(wert) = field(json, "alpha_cutoff_bias") {
        float(wert).context("alpha_cutoff_bias")?;
    }
    Ok(())
}

/// `AnimationMetadataSection.CODEC`: `width`, `height` und `frametime`
/// sind positiv (`POSITIVE_INT`), `interpolate` ein Wahrheitswert.
fn animation_section(json: &Value) -> Result<Animation> {
    ensure!(json.is_object(), "kein Objekt");
    let positiv = |name: &str| -> Result<Option<u32>> {
        field(json, name)
            .map(|wert| positive(wert).with_context(|| name.to_string()))
            .transpose()
    };
    let width = positiv("width")?;
    let height = positiv("height")?;
    positiv("frametime")?;
    if let Some(wert) = field(json, "interpolate") {
        boolean(wert).context("interpolate")?;
    }
    let frames = field(json, "frames")
        .map(|liste| {
            liste
                .as_array()
                .ok_or_else(|| anyhow!("frames ist keine Liste"))?
                .iter()
                .map(|bild| frame(bild).context("frames"))
                .collect::<Result<Vec<u32>>>()
        })
        .transpose()?;
    Ok(Animation {
        width,
        height,
        frames,
    })
}

/// Ein Eintrag in `frames` (`AnimationFrame.CODEC`): eine Nummer ab 0 oder
/// ein Objekt mit `index` und eigener Anzeigedauer `time`.
fn frame(json: &Value) -> Result<u32> {
    let index = match json {
        Value::Object(_) => {
            if let Some(time) = field(json, "time") {
                positive(time).context("time")?;
            }
            int(field(json, "index").ok_or_else(|| anyhow!("index fehlt"))?)?
        }
        _ => int(json)?,
    };
    u32::try_from(index).map_err(|_| anyhow!("Bild {index}"))
}

/// `ExtraCodecs.POSITIVE_INT`.
fn positive(json: &Value) -> Result<u32> {
    let wert = int(json)?;
    match u32::try_from(wert) {
        Ok(wert) if wert >= 1 => Ok(wert),
        _ => bail!("{wert} ist nicht positiv"),
    }
}

/// Das Bild einer Animation, das der Client zuerst zeigt, oder `Err`, wenn
/// er die Textur verwirft.
///
/// Die Bildgrösse steht in `width` und `height`, fehlt eine, gilt die
/// andere, fehlen beide, die kürzere Seite (`calculateFrameSize`). Teilt
/// sie das Bild nicht auf, verwirft `SpriteResourceLoader` die Textur.
/// `SpriteContents` streicht Bilder aus `frames`, die es nicht gibt; ohne
/// `frames` zählen alle. Zeigt es danach mindestens zwei, beginnt es mit
/// dem ersten. Sonst ist die Textur statisch, und ist das Bild dann
/// grösser als eines, scheitert im Client das Hochladen des Atlas
/// (`CommandEncoder.writeToTexture`); der Renderer verwirft sie.
fn erstes_bild(image: &RgbaImage, animation: &Animation) -> Result<RgbaImage> {
    let (breite, hoehe) = image.dimensions();
    let (b, h) = match (animation.width, animation.height) {
        (Some(b), Some(h)) => (b, h),
        (Some(b), None) => (b, b),
        (None, Some(h)) => (h, h),
        (None, None) => (breite.min(hoehe), breite.min(hoehe)),
    };
    ensure!(
        b > 0 && breite.is_multiple_of(b) && hoehe.is_multiple_of(h),
        "Bildgrösse {breite}x{hoehe} ist kein Vielfaches von {b}x{h}"
    );
    let spalten = breite / b;
    let gesamt = spalten * (hoehe / h);
    let gueltig: Vec<u32> = match &animation.frames {
        None => (0..gesamt.min(2)).collect(),
        Some(frames) => frames
            .iter()
            .copied()
            .filter(|&i| i < gesamt)
            .take(2)
            .collect(),
    };
    // `frames` gibt die Abspielreihenfolge an; das erste Bild darin ist nicht
    // zwingend Nummer 0. Vanilla nutzt das in fire_0 und soul_fire_0.
    let index = match gueltig.as_slice() {
        [erstes, _] => *erstes,
        _ if (breite, hoehe) == (b, h) => return Ok(image.clone()),
        _ => bail!("nur ein Bild der Animation, aber das Bild ist {breite}x{hoehe}"),
    };
    Ok(
        image::imageops::crop_imm(image, (index % spalten) * b, (index / spalten) * h, b, h)
            .to_image(),
    )
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

    /// Jede 16 Pixel hohe Zeile bekommt ihre Nummer als Rotwert.
    fn streifen(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |_, y| {
            image::Rgba([(y / 16) as u8, 0, 0, 255])
        })
    }

    fn animation(json: &str) -> Animation {
        mcmeta(json).unwrap().unwrap()
    }

    fn erstes(image: &RgbaImage, json: &str) -> Result<RgbaImage> {
        erstes_bild(image, &animation(json))
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
        assert!(textures.broken().is_empty());
    }

    #[test]
    fn unbekannte_id_liefert_den_platzhalter() {
        let textures = Textures::new();
        assert_eq!(textures.image(TextureId(999)).dimensions(), (16, 16));
        assert_eq!(textures.name(TextureId(999)), "<unbekannt>");
    }

    #[test]
    fn senkrechter_streifen_ohne_angaben() {
        let frame = erstes(&streifen(16, 48), r#"{"animation": {}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
        assert_eq!(frame.get_pixel(0, 0).0[0], 0);
    }

    /// Vanilla-fire_0 beginnt mit Bild 16, nicht mit 0.
    #[test]
    fn erstes_deklariertes_bild_gewinnt() {
        let frame = erstes(
            &streifen(16, 512),
            r#"{"animation": {"frames": [16, 17, 0]}}"#,
        )
        .unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
        assert_eq!(frame.get_pixel(0, 0).0[0], 16);
    }

    #[test]
    fn frames_als_objekte() {
        let frame = erstes(
            &streifen(16, 512),
            r#"{"animation": {"frames": [{"index": 3, "time": 2}, 0]}}"#,
        )
        .unwrap();
        assert_eq!(frame.get_pixel(0, 0).0[0], 3);
    }

    /// Wie `calculateFrameSize`: fehlt eine Seite, gilt die andere, fehlen
    /// beide, die kürzere des Bildes. Ein waagerechter Streifen läuft so
    /// von links nach rechts.
    #[test]
    fn bildgroesse_wie_im_client() {
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"height": 8}}"#).unwrap();
        assert_eq!(frame.dimensions(), (8, 8));
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"width": 8}}"#).unwrap();
        assert_eq!(frame.dimensions(), (8, 8));
        let frame = erstes(&streifen(48, 16), r#"{"animation": {}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
        let frame = erstes(
            &streifen(32, 16),
            r#"{"animation": {"width": 16, "height": 16}}"#,
        )
        .unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
    }

    /// `"width": 16.0` ist für `Codec.INT` 16, wie `intValue`.
    #[test]
    fn kommazahl_als_bildgroesse() {
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"width": 16.0}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
    }

    /// Bilder, die es nicht gibt, streicht der Client. Bleibt eines, ist die
    /// Textur statisch; ist das Bild grösser als eines, scheitert im Client
    /// der Atlas, und der Renderer verwirft die Textur. Ein Bild, das die
    /// Bildgrösse nicht teilt, verwirft schon `SpriteResourceLoader`.
    #[test]
    fn unpassende_bilder_verwerfen_die_textur() {
        let frame = erstes(
            &streifen(16, 48),
            r#"{"animation": {"frames": [42, 2, 1]}}"#,
        )
        .unwrap();
        assert_eq!(frame.get_pixel(0, 0).0[0], 2);
        for json in [
            r#"{"animation": {"width": 999}}"#,
            r#"{"animation": {"frames": [3]}}"#,
            r#"{"animation": {"frames": [42, 0]}}"#,
            r#"{"animation": {"frames": []}}"#,
        ] {
            assert!(erstes(&streifen(16, 48), json).is_err(), "{json}");
        }
        let einzeln = erstes(&streifen(16, 16), r#"{"animation": {"frames": [0]}}"#).unwrap();
        assert_eq!(einzeln.dimensions(), (16, 16));
    }

    /// 48 der Vanilla-mcmeta enthalten nur `texture`-Flags. Solche Texturen
    /// sind statisch und dürfen nicht zugeschnitten werden.
    #[test]
    fn mcmeta_ohne_animation_ist_keine_animation() {
        assert!(mcmeta(r#"{"texture": {"blur": true}}"#).unwrap().is_none());
    }

    /// Was die Codecs ablehnen, macht die Textur im Client zur
    /// Missing-Textur, belegt per javap an 26.2 samt DFU 10.0.21. Ein
    /// doppelter Schlüssel nimmt wie in Gson den letzten Wert.
    #[test]
    fn mcmeta_wie_im_client() {
        for json in [
            r#"{"animation": {"frametime": 0}}"#,
            r#"{"animation": {"width": -16}}"#,
            r#"{"animation": {"width": "16"}}"#,
            r#"{"animation": {"interpolate": 1}}"#,
            r#"{"animation": {"frames": [-1]}}"#,
            r#"{"animation": {"frames": [{"time": 2}]}}"#,
            r#"{"animation": {"frames": [{"index": 0, "time": 0}]}}"#,
            r#"{"animation": {"frames": [null]}}"#,
            r#"{"animation": {"frames": 0}}"#,
            r#"{"animation": null}"#,
            r#"{"animation": []}"#,
            r#"{"texture": {"blur": 1}}"#,
            r#"{"texture": {"mipmap_strategy": "linear"}}"#,
            r#"{"texture": {"alpha_cutoff_bias": "0.5"}}"#,
            r#"{"texture": null}"#,
            r#"[]"#,
            r#"null"#,
            r#""#,
        ] {
            assert!(mcmeta(json).is_err(), "{json}");
        }
        for json in [
            r#"{"animation": {"width": null, "frames": [{"index": 1, "time": null}]}}"#,
            r#"{"texture": {"blur": true, "clamp": false, "mipmap_strategy": "dark_cutout", "alpha_cutoff_bias": 0.5}}"#,
            r#"{"villager": 5, "gui": null}"#,
            r#"{"animation": {}} ["dahinter"]"#,
        ] {
            assert!(mcmeta(json).is_ok(), "{json}");
        }
        let doppelt = animation(r#"{"animation": {"height": 4, "height": 8}}"#);
        assert_eq!(doppelt.height, Some(8));
    }
}
