use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};
use image::RgbaImage;
use serde_json::Value;

use super::blockstate::{boolean, field, float, int};
use super::laubkopie::{self, Kopie};
use super::pack::ASSETS;
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
    /// Parallel zu `images`: was das Spiel ausser dem ersten Bild braucht,
    /// um eine Fläche in ihre Schicht zu legen.
    eigenschaften: Vec<Eigenschaften>,
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
            eigenschaften: vec![Eigenschaften::default()],
            names: vec!["<fehlende Textur>".to_string()],
            missing: BTreeSet::new(),
            broken: BTreeMap::new(),
        }
    }

    /// Platzhalter für nicht auffindbare Texturen.
    pub const MISSING: TextureId = TextureId(0);

    /// Lädt eine Textur oder liefert die bereits geladene.
    ///
    /// Eine fehlende Datei ist kein Fehler, sie wird zur Missing-Textur, ebenso
    /// eine, die der Client verwirft ([`read_texture`]). Die Namen sammelt
    /// [`Textures::missing`], die verworfenen mit Grund [`Textures::broken`].
    /// Ohne Namensraum gilt `minecraft`, wie im Client: `block/stone` ist
    /// dieselbe Textur wie `minecraft:block/stone`. Wo die Datei liegt, sagt
    /// [`datei`].
    /// Siehe docs/renderer/modelle-und-texturen.md, „`.mcmeta`“.
    pub fn load(&mut self, packs: &[Pack], id: &str) -> TextureId {
        let (namespace, name) = split_id(id);
        let id = format!("{namespace}:{name}");
        if let Some(&existing) = self.ids.get(&id) {
            return existing;
        }

        let loaded = datei(packs, namespace, name, "png")
            .map(|(layer, path)| read_texture(packs, layer, namespace, name, &path));

        let texture = match loaded {
            Some(Ok((image, eigenschaften))) => {
                self.images.push(image);
                self.eigenschaften.push(eigenschaften);
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

    /// Ob der Ausschnitt von `von` bis `bis`, in UV von 0 bis 1, halb
    /// durchsichtige Texel hat, wie `SpriteContents.computeTransparency` in
    /// 26.2 es für die Schicht TRANSLUCENT prüft: in jedem Bild, das die
    /// Animation zeigt, über die Texel von `floor(von)` bis `ceil(bis)`. Für
    /// den vollen Ausschnitt gilt die ganze Datei.
    /// Siehe docs/renderer/naehte.md, „Ausgeschnitten statt gemischt“.
    pub fn durchscheinend(&self, id: TextureId, von: [f32; 2], bis: [f32; 2]) -> bool {
        let Some(eigenschaften) = self.eigenschaften.get(id.0 as usize) else {
            return false;
        };
        if !eigenschaften.ganz || (von == [0.0, 0.0] && bis == [1.0, 1.0]) {
            return eigenschaften.ganz;
        }
        let (breite, hoehe) = self.image(id).dimensions();
        // Das Spiel wirft bei einem Ausschnitt über den Rand hinaus; hier
        // bleibt er in der Textur.
        let texel = |uv: f32, rand: u32, runden: fn(f32) -> f32| {
            (runden(uv * rand as f32).max(0.0) as u32).min(rand)
        };
        let (x0, y0) = (
            texel(von[0], breite, f32::floor),
            texel(von[1], hoehe, f32::floor),
        );
        let (x1, y1) = (
            texel(bis[0], breite, f32::ceil),
            texel(bis[1], hoehe, f32::ceil),
        );
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (y * breite + x) as usize))
            .any(|i| eigenschaften.texel[i])
    }

    /// Die Farbe, die das Spiel bei `"mipmap_strategy": "dark_cutout"` in
    /// jedes Loch schreibt, sonst `None`.
    /// Siehe docs/renderer/naehte.md, „Ausgeschnitten statt gemischt“.
    pub fn fuellung(&self, id: TextureId) -> Option<[u8; 3]> {
        self.eigenschaften.get(id.0 as usize)?.fuellung
    }

    /// Eine Kopie einer Blatttextur für eigene Laubfarben, je Art einmal
    /// angelegt, ohne `dark_cutout` wie die helle, die das Spiel ohne
    /// `.mcmeta` anlegt.
    /// `None` für eine Textur ohne Tabelle dieser Art. Sonst die Kopie, `id`
    /// selbst, wenn sich nichts ändert, etwa in einem Resourcepack, und die
    /// Blüten allein, falls es welche gibt.
    /// Siehe docs/benutzung/laubfarben.md, „Wirkung“.
    pub fn kopie(&mut self, id: TextureId, art: Kopie) -> Option<(TextureId, Option<TextureId>)> {
        let name = self.name(id).to_string();
        let tausch = laubkopie::tausch(art, &name)?;
        let (kopie_name, blueten_name) = (format!("{name}#{art:?}"), format!("{name}#Blueten"));
        if let Some(&kopie) = self.ids.get(&kopie_name) {
            return Some((kopie, self.ids.get(&blueten_name).copied()));
        }
        let (bild, blueten) = laubkopie::kopie(self.image(id), tausch, laubkopie::blueten(&name));
        let kopie = match bild != *self.image(id) {
            true => self.ohne_datei(&kopie_name, bild),
            false => id,
        };
        self.ids.insert(kopie_name, kopie);
        if let Some(blueten) = blueten
            && !self.ids.contains_key(&blueten_name)
        {
            let id = self.ohne_datei(&blueten_name, blueten);
            self.ids.insert(blueten_name.clone(), id);
        }
        Some((kopie, self.ids.get(&blueten_name).copied()))
    }

    /// Nimmt ein Bild ohne Datei auf, statisch und ohne `dark_cutout`.
    fn ohne_datei(&mut self, name: &str, bild: RgbaImage) -> TextureId {
        let groesse = (bild.width(), bild.height());
        self.eigenschaften
            .push(eigenschaften(&bild, groesse, &[(0, 0)], false));
        self.images.push(bild);
        self.names.push(name.to_string());
        TextureId(self.images.len() as u32 - 1)
    }

    /// Nimmt ein Bild als Textur auf, statisch und ohne Datei.
    #[cfg(test)]
    pub(crate) fn einfuegen(&mut self, name: &str, image: RgbaImage, dunkel: bool) -> TextureId {
        let bild = (image.width(), image.height());
        self.eigenschaften
            .push(eigenschaften(&image, bild, &[(0, 0)], dunkel));
        self.images.push(image);
        self.names.push(name.to_string());
        TextureId(self.images.len() as u32 - 1)
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

/// Halb durchsichtig: weder Loch noch deckend, wie `NativeImage
/// .computeTransparency` in 26.2 zählt.
fn halb(alpha: u8) -> bool {
    alpha != 0 && alpha != 255
}

/// Was das Spiel ausser dem ersten Bild von einer Textur braucht.
#[derive(Default)]
struct Eigenschaften {
    /// Ob die ganze Datei halb durchsichtige Texel hat, über alle Bilder
    /// einer Animation (`SpriteContents.transparency`).
    ganz: bool,
    /// Je Texel eines Bildes, zeilenweise: ob es in einem der Bilder, die
    /// die Animation zeigt, halb durchsichtig ist. Leer ohne solche Texel.
    texel: Vec<bool>,
    /// Die Farbe der Löcher bei `dark_cutout`.
    fuellung: Option<[u8; 3]>,
}

/// Die Eigenschaften einer Datei `original`, deren Bilder `bild` gross
/// sind und an `ursprung` beginnen.
fn eigenschaften(
    original: &RgbaImage,
    (breite, hoehe): (u32, u32),
    ursprung: &[(u32, u32)],
    dunkel: bool,
) -> Eigenschaften {
    let ganz = original.pixels().any(|p| halb(p[3]));
    let texel = if ganz {
        (0..hoehe)
            .flat_map(|y| (0..breite).map(move |x| (x, y)))
            .map(|(x, y)| {
                ursprung
                    .iter()
                    .any(|&(ux, uy)| halb(original.get_pixel(ux + x, uy + y)[3]))
            })
            .collect()
    } else {
        Vec::new()
    };
    Eigenschaften {
        ganz,
        texel,
        fuellung: dunkel.then(|| fuellung(original)),
    }
}

/// Wie `TextureUtil.fillEmptyAreasWithDarkColor` in 26.2: drei Viertel des
/// deckenden Texels mit der kleinsten Summe aus Rot, Grün und Blau, bei
/// Gleichstand des ersten, Spalte für Spalte. Ohne deckendes Texel bleibt
/// wie im Spiel Weiss als Ausgang.
fn fuellung(original: &RgbaImage) -> [u8; 3] {
    let (breite, hoehe) = original.dimensions();
    let mut dunkelstes = [255u8; 3];
    let mut kleinste = u32::MAX;
    for x in 0..breite {
        for y in 0..hoehe {
            let [r, g, b, a] = original.get_pixel(x, y).0;
            let summe = r as u32 + g as u32 + b as u32;
            if a != 0 && summe < kleinste {
                kleinste = summe;
                dunkelstes = [r, g, b];
            }
        }
    }
    dunkelstes.map(|c| (3 * c as u32 / 4) as u8)
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
) -> Result<(RgbaImage, Eigenschaften)> {
    let image = image::open(path)
        .with_context(|| format!("{} lesen", path.display()))?
        .into_rgba8();
    let Meta { animation, dunkel } = meta(packs, png_layer, namespace, name)?;
    let Bilder { erstes, ursprung } = match animation {
        Some(animation) => erstes_bild(&image, &animation)?,
        None => Bilder::statisch(),
    };
    let bild = erstes.as_ref().unwrap_or(&image).dimensions();
    let eigenschaften = eigenschaften(&image, bild, &ursprung, dunkel);
    Ok((erstes.unwrap_or(image), eigenschaften))
}

/// Sucht die `.mcmeta`-Datei im Packstapel und liest sie: aus derselben
/// oder einer höheren Schicht als die PNG, gepaart, wie die PNG gefunden
/// wurde ([`datei`]). Ohne `animation` ist die Textur statisch, auch wenn
/// die Datei existiert.
/// Siehe docs/renderer/modelle-und-texturen.md, „`.mcmeta`“.
fn meta(packs: &[Pack], png_layer: usize, namespace: &str, name: &str) -> Result<Meta> {
    let Some((_, meta)) = datei(&packs[png_layer..], namespace, name, "png.mcmeta") else {
        return Ok(Meta::default());
    };
    mcmeta(&read_text(&meta)?).with_context(|| format!("{} lesen", meta.display()))
}

/// Die Datei zur Textur `name` mit der `endung`, `png` oder `png.mcmeta`,
/// und ihre Schicht im Packstapel, die oberste zuerst. Aus einem Ordner
/// eines Atlas nur, was der Client dort auflistet ([`ASSETS`]); jede
/// Textur ausserhalb dieser Ordner öffnet der Renderer direkt.
/// Siehe docs/renderer/packs.md, „Direkt geöffnete Dateien“.
fn datei(packs: &[Pack], namespace: &str, name: &str, endung: &str) -> Option<(usize, PathBuf)> {
    let im_ordner = ASSETS
        .iter()
        .filter_map(|liste| liste.strip_prefix(&["textures"][..]))
        .any(|ordner| {
            name.strip_prefix(&ordner.join("/"))
                .is_some_and(|rest| rest.starts_with('/'))
        });
    if im_ordner {
        return find_file(packs, namespace, "textures", name, endung)
            .map(|(layer, pfad)| (layer, pfad.to_path_buf()));
    }
    let pfad = format!("textures/{name}.{endung}");
    packs
        .iter()
        .enumerate()
        .rev()
        .find_map(|(layer, pack)| pack.resource(namespace, &pfad).map(|datei| (layer, datei)))
}

/// Was der Renderer aus einer `.mcmeta` braucht.
#[derive(Debug, Default)]
struct Meta {
    animation: Option<Animation>,
    /// `"mipmap_strategy": "dark_cutout"`.
    dunkel: bool,
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
fn mcmeta(text: &str) -> Result<Meta> {
    let json = parse_json(text, false)?;
    ensure!(json.is_object(), "kein Objekt");
    let dunkel = match json.get("texture") {
        Some(textur) => texture_section(textur).context("texture")?,
        None => false,
    };
    let animation = json
        .get("animation")
        .map(|animation| animation_section(animation).context("animation"))
        .transpose()?;
    Ok(Meta { animation, dunkel })
}

/// `TextureMetadataSection.CODEC`: alles darf fehlen, und was dasteht,
/// muss passen. Wahr bei `"mipmap_strategy": "dark_cutout"`.
fn texture_section(json: &Value) -> Result<bool> {
    ensure!(json.is_object(), "kein Objekt");
    for name in ["blur", "clamp"] {
        if let Some(wert) = field(json, name) {
            boolean(wert).with_context(|| name.to_string())?;
        }
    }
    let strategie = field(json, "mipmap_strategy").map(|wert| (wert, wert.as_str()));
    if let Some((wert, name)) = strategie {
        ensure!(
            matches!(
                name,
                Some("auto" | "mean" | "cutout" | "strict_cutout" | "dark_cutout")
            ),
            "mipmap_strategy {wert}"
        );
    }
    if let Some(wert) = field(json, "alpha_cutoff_bias") {
        float(wert).context("alpha_cutoff_bias")?;
    }
    Ok(strategie.is_some_and(|(_, name)| name == Some("dark_cutout")))
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

/// Was eine Animation zeigt.
struct Bilder {
    /// Das Bild, das der Client zuerst zeigt; `None`, wenn die Textur
    /// statisch ist.
    erstes: Option<RgbaImage>,
    /// Wo jedes Bild beginnt, das er zeigt.
    ursprung: Vec<(u32, u32)>,
}

impl Bilder {
    fn statisch() -> Bilder {
        Bilder {
            erstes: None,
            ursprung: vec![(0, 0)],
        }
    }
}

/// Die Bilder einer Animation, die der Client zeigt, oder `Err`, wenn er
/// die Textur verwirft: Die Bildgrösse kommt aus `width` und `height` oder
/// aus dem Bild und muss es teilen. Von den Bildern aus `frames`, die es
/// gibt, zeigt er bei mindestens zweien das erste; sonst ist die Textur
/// statisch und darf nicht grösser sein als ein Bild.
/// Siehe docs/renderer/modelle-und-texturen.md, „`.mcmeta`“.
fn erstes_bild(image: &RgbaImage, animation: &Animation) -> Result<Bilder> {
    let (breite, hoehe) = image.dimensions();
    let (b, h) = match (animation.width, animation.height) {
        (Some(b), Some(h)) => (b, h),
        (Some(b), None) => (b, hoehe),
        (None, Some(h)) => (breite, h),
        (None, None) => (breite.min(hoehe), breite.min(hoehe)),
    };
    ensure!(
        b > 0 && breite.is_multiple_of(b) && hoehe.is_multiple_of(h),
        "Bildgrösse {breite}x{hoehe} ist kein Vielfaches von {b}x{h}"
    );
    let spalten = breite / b;
    let gesamt = spalten * (hoehe / h);
    let gueltig: Vec<u32> = match &animation.frames {
        None => (0..gesamt).collect(),
        Some(frames) => frames.iter().copied().filter(|&i| i < gesamt).collect(),
    };
    // `frames` gibt die Abspielreihenfolge an; das erste Bild darin ist nicht
    // zwingend Nummer 0. Vanilla nutzt das in fire_0 und soul_fire_0.
    let index = match gueltig.as_slice() {
        [erstes, _, ..] => *erstes,
        _ if (breite, hoehe) == (b, h) => return Ok(Bilder::statisch()),
        _ => bail!("nur ein Bild der Animation, aber das Bild ist {breite}x{hoehe}"),
    };
    let ursprung = |i: u32| ((i % spalten) * b, (i / spalten) * h);
    let (x, y) = ursprung(index);
    Ok(Bilder {
        erstes: Some(image::imageops::crop_imm(image, x, y, b, h).to_image()),
        ursprung: gueltig.into_iter().map(ursprung).collect(),
    })
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
    /// Die helle Fassung tauscht die Farben der Tabelle, hat kein
    /// `dark_cutout` und entsteht einmal; ohne Farbe aus der Tabelle und für
    /// eine Textur ohne Tabelle bleibt es dieselbe Textur.
    #[test]
    fn helle_fassung_ohne_dark_cutout() {
        use super::{RgbaImage, Textures};
        let mut textures = Textures::new();
        let bild = |rgb: [u8; 3]| {
            RgbaImage::from_fn(2, 1, |x, _| {
                image::Rgba([rgb[0], rgb[1], rgb[2], [255, 0][x as usize]])
            })
        };
        let eiche =
            textures.einfuegen("minecraft:block/oak_leaves", bild([0x68, 0x64, 0x68]), true);
        assert!(textures.fuellung(eiche).is_some());
        let (hell, blueten) = textures.kopie(eiche, Kopie::Hell).unwrap();
        assert_ne!(hell, eiche);
        assert!(blueten.is_none());
        assert_eq!(
            textures.image(hell).get_pixel(0, 0).0,
            [0xa8, 0xa4, 0xa8, 255]
        );
        assert_eq!(textures.fuellung(hell), None);
        assert_eq!(textures.kopie(eiche, Kopie::Hell), Some((hell, None)));
        assert_eq!(textures.kopie(eiche, Kopie::Grau), None);
        let fremd = textures.einfuegen("minecraft:block/birch_leaves", bild([1, 2, 3]), false);
        assert_eq!(textures.kopie(fremd, Kopie::Hell), Some((fremd, None)));
        let stein = textures.einfuegen("minecraft:block/stone", bild([0x68, 0x64, 0x68]), false);
        assert_eq!(textures.kopie(stein, Kopie::Hell), None);
        // Die blühende Azalee: grau ohne Blüten, die Blüten für sich, einmal.
        let azalee =
            RgbaImage::from_raw(2, 1, vec![0x70, 0x92, 0x2d, 255, 0xba, 0x62, 0xce, 255]).unwrap();
        let azalee = textures.einfuegen("minecraft:block/flowering_azalea_leaves", azalee, true);
        let (grau, blueten) = textures.kopie(azalee, Kopie::Grau).unwrap();
        let blueten = blueten.unwrap();
        assert_eq!(
            textures.image(grau).get_pixel(0, 0).0,
            [0xbc, 0xbc, 0xbc, 255]
        );
        assert_eq!(
            textures.image(blueten).get_pixel(1, 0).0,
            [0xba, 0x62, 0xce, 255]
        );
        assert_eq!(
            (textures.fuellung(grau), textures.fuellung(blueten)),
            (None, None)
        );
        let (hell, blueten_hell) = textures.kopie(azalee, Kopie::Hell).unwrap();
        assert_ne!(hell, grau);
        assert_eq!(blueten_hell, Some(blueten));
    }

    use super::*;

    /// Jede 16 Pixel hohe Zeile bekommt ihre Nummer als Rotwert.
    fn streifen(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |_, y| {
            image::Rgba([(y / 16) as u8, 0, 0, 255])
        })
    }

    fn animation(json: &str) -> Animation {
        mcmeta(json).unwrap().animation.unwrap()
    }

    fn erstes(image: &RgbaImage, json: &str) -> Result<RgbaImage> {
        let Bilder { erstes, .. } = erstes_bild(image, &animation(json))?;
        Ok(erstes.unwrap_or_else(|| image.clone()))
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

    /// Wie `calculateFrameSize`, belegt per javap: fehlt eine Seite, gilt
    /// dafür die des Bildes, fehlen beide, für beide die kürzere. Ein
    /// waagerechter Streifen läuft so von links nach rechts.
    #[test]
    fn bildgroesse_wie_im_client() {
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"height": 8}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 8));
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"width": 8}}"#).unwrap();
        assert_eq!(frame.dimensions(), (8, 32));
        let frame = erstes(&streifen(16, 48), r#"{"animation": {"height": 12}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 12));
        let frame = erstes(&streifen(48, 16), r#"{"animation": {}}"#).unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
        let frame = erstes(
            &streifen(32, 16),
            r#"{"animation": {"width": 16, "height": 16}}"#,
        )
        .unwrap();
        assert_eq!(frame.dimensions(), (16, 16));
    }

    /// `"width": 8.0` ist für `Codec.INT` 8, wie `intValue`.
    #[test]
    fn kommazahl_als_bildgroesse() {
        let frame = erstes(&streifen(16, 32), r#"{"animation": {"width": 8.0}}"#).unwrap();
        assert_eq!(frame.dimensions(), (8, 32));
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
            r#"{"animation": {"width": 16, "height": 20}}"#,
            r#"{"animation": {"frames": [3]}}"#,
            r#"{"animation": {"frames": [42, 0]}}"#,
            r#"{"animation": {"frames": []}}"#,
        ] {
            assert!(erstes(&streifen(16, 48), json).is_err(), "{json}");
        }
        let einzeln = erstes(&streifen(16, 16), r#"{"animation": {"frames": [0]}}"#).unwrap();
        assert_eq!(einzeln.dimensions(), (16, 16));
    }

    /// Wie `SpriteContents.computeTransparency`: der Ausschnitt von floor
    /// bis ceil der UV, der volle nach der ganzen Datei. Löcher machen
    /// nichts durchscheinend.
    #[test]
    fn durchscheinend_im_ausschnitt() {
        let mut textures = Textures::new();
        let bild = RgbaImage::from_fn(16, 16, |x, y| {
            let alpha = match (x, y) {
                (0..8, 0) => 0,
                (0..8, 1) => 128,
                _ => 255,
            };
            image::Rgba([90, 90, 90, alpha])
        });
        let id = textures.einfuegen("probe", bild, false);
        assert!(textures.durchscheinend(id, [0.0, 0.0], [1.0, 1.0]));
        assert!(!textures.durchscheinend(id, [0.5, 0.0], [1.0, 1.0]));
        assert!(!textures.durchscheinend(id, [0.0, 0.0], [0.5, 0.05]));
        // floor(0,49 · 16) = 7: die Spalte 7 gehört dazu.
        assert!(textures.durchscheinend(id, [0.49, 0.07], [1.0, 0.1]));
        // ceil(0,07 · 16) = 2: die Zeile 1 gehört dazu.
        assert!(textures.durchscheinend(id, [0.0, 0.0], [0.5, 0.07]));
        assert!(!textures.durchscheinend(Textures::MISSING, [0.2, 0.2], [0.3, 0.3]));
    }

    /// Bei einer Animation zählt jedes Bild, das sie zeigt, auch wenn es
    /// nicht das erste ist; eines, das sie nicht zeigt, zählt nicht. Nur
    /// der volle Ausschnitt nimmt die ganze Datei, auch dieses Bild.
    #[test]
    fn transparenz_ueber_die_bilder_der_animation() {
        let dir = tempfile::tempdir().unwrap();
        let block = dir.path().join("minecraft/textures/block");
        std::fs::create_dir_all(&block).unwrap();
        // Drei Bilder übereinander, gezeigt werden die ersten beiden.
        let streifen = |name: &str, halb: &[(u32, u32)]| {
            let bild = RgbaImage::from_fn(16, 48, |x, y| {
                let alpha = if halb.contains(&(x, y)) { 128 } else { 255 };
                image::Rgba([50, 60, 70, alpha])
            });
            bild.save(block.join(format!("{name}.png"))).unwrap();
            std::fs::write(
                block.join(format!("{name}.png.mcmeta")),
                r#"{"animation": {"frames": [0, 1]}}"#,
            )
            .unwrap();
        };
        streifen("zweites", &[(0, 16)]);
        streifen("verborgen", &[(15, 32)]);
        let packs = [Pack::open(dir.path(), &super::super::pack::ASSETS).unwrap()];
        let mut textures = Textures::new();

        let zweites = textures.load(&packs, "block/zweites");
        assert_eq!(textures.image(zweites).dimensions(), (16, 16));
        assert!(textures.durchscheinend(zweites, [0.0, 0.0], [0.5, 0.5]));
        assert!(!textures.durchscheinend(zweites, [0.5, 0.0], [1.0, 0.5]));

        let verborgen = textures.load(&packs, "block/verborgen");
        assert!(!textures.durchscheinend(verborgen, [0.0, 0.0], [0.99, 1.0]));
        assert!(textures.durchscheinend(verborgen, [0.0, 0.0], [1.0, 1.0]));
    }

    /// Wie `fillEmptyAreasWithDarkColor`: drei Viertel des deckenden
    /// Texels mit der kleinsten Summe, bei Gleichstand das erste, Spalte
    /// für Spalte; nur bei `dark_cutout`.
    #[test]
    fn fuellung_wie_im_spiel() {
        let bild = RgbaImage::from_fn(4, 4, |x, y| {
            image::Rgba(match (x, y) {
                (0, 3) => [30, 20, 10, 255],
                (3, 0) => [10, 20, 30, 255],
                (1, 1) => [0, 0, 0, 0],
                _ => [200, 200, 200, 255],
            })
        });
        let mut textures = Textures::new();
        let dunkel = textures.einfuegen("dunkel", bild.clone(), true);
        assert_eq!(textures.fuellung(dunkel), Some([22, 15, 7]));
        let hell = textures.einfuegen("hell", bild, false);
        assert_eq!(textures.fuellung(hell), None);
        assert_eq!(fuellung(&RgbaImage::new(2, 2)), [191; 3]);
        assert!(
            mcmeta(r#"{"texture": {"mipmap_strategy": "dark_cutout"}}"#)
                .unwrap()
                .dunkel
        );
        for json in [
            r#"{"texture": {"mipmap_strategy": "strict_cutout"}}"#,
            r#"{"texture": {}}"#,
            r#"{}"#,
        ] {
            assert!(!mcmeta(json).unwrap().dunkel, "{json}");
        }
    }

    /// 48 der Vanilla-mcmeta enthalten nur `texture`-Flags. Solche Texturen
    /// sind statisch und dürfen nicht zugeschnitten werden.
    #[test]
    fn mcmeta_ohne_animation_ist_keine_animation() {
        assert!(
            mcmeta(r#"{"texture": {"blur": true}}"#)
                .unwrap()
                .animation
                .is_none()
        );
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
