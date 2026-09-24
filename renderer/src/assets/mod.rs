pub mod baker;
pub mod blockstate;
pub mod colors;
pub mod fluid;
pub mod model;
pub mod pack;
pub mod texture;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::world::BlockState;
pub use baker::{BakedModel, Quad, bake};
pub use blockstate::{BlockStateDef, Definition, ModelRef};
pub use colors::{Colors, Tint, Tints};
use model::ModelFile;
pub use model::{Element, ElementFace, Face, ResolvedModel, Rotation};
pub use pack::Pack;
pub use texture::{TextureId, Textures};

/// Das fertige Modell einer Blockstate: gebacken und um die Flüssigkeit
/// ergänzt, die kein Modell-JSON beschreibt. Bei Alternativen die erste.
pub fn model_of(assets: &mut Assets, state: &BlockState) -> Result<BakedModel> {
    Ok(models_of(assets, state)?.swap_remove(0).1)
}

/// Alle Alternativen einer Blockstate mit Gewicht, fertig gebacken.
///
/// Jeder Pfad, der ein Sprite baut, geht hier durch — sonst hätte der eine
/// Wasser und der andere nicht.
pub fn models_of(assets: &mut Assets, state: &BlockState) -> Result<Vec<(u32, BakedModel)>> {
    assets
        .alternatives(state)?
        .into_iter()
        .map(|(weight, variants)| {
            let mut model = bake(&variants);
            fluid::add(&mut model, state, assets);
            Ok((weight, model))
        })
        .collect()
}

/// Der Verweis, unter dem ein fehlendes Modell als Missing-Würfel steht.
pub const MISSING_MODEL: &str = "minecraft:builtin/missing";

/// Ein Modell, das für eine konkrete Blockstate gilt, mit der Drehung aus der
/// Blockstate-Datei.
#[derive(Debug, Clone)]
pub struct ResolvedVariant {
    /// Der Modellverweis aus der Blockstate-Datei, für Diagnoseausgaben.
    pub model_id: String,
    pub model: Arc<ResolvedModel>,
    /// Drehung um die X-Achse in Grad, Vielfache von 90.
    pub x: i32,
    /// Drehung um die Y-Achse in Grad, Vielfache von 90.
    pub y: i32,
    /// Drehung um die Z-Achse in Grad; seit Minecraft 1.21.11 erlaubt.
    pub z: i32,
    /// Texturen mitdrehen statt der Drehung folgen zu lassen.
    pub uvlock: bool,
}

/// Ein aufgelöster Asset-Baum aus einem oder mehreren Wurzelverzeichnissen.
///
/// Spätere Wurzeln überschreiben frühere, so wie Minecraft Resourcepacks
/// stapelt: Modelle und Texturen Datei für Datei, Blockstates Zustand für
/// Zustand. Ein Overlay-Pack, das nur 39 Blockstates mitbringt,
/// funktioniert damit über einer vollständigen Vanilla-Basis, und eines,
/// das in einer Datei nur einen Teil der Zustände nennt, auch. Jede Wurzel
/// liest der Renderer wie der Client, siehe [`Pack`].
pub struct Assets {
    packs: Vec<Pack>,
    textures: Textures,
    blockstates: HashMap<String, Arc<BlockStateStack>>,
    models: HashMap<String, Arc<ResolvedModel>>,
    /// Modelle, deren Parent fehlt oder kaputt ist, mit dem Grund.
    parent_problems: HashMap<String, String>,
    colors: Colors,
    skipped: BTreeMap<String, String>,
    broken: BTreeMap<String, String>,
    unchecked: BTreeMap<String, String>,
}

/// Die Blockstate-Dateien eines Blocks aus allen Wurzeln, die oberste
/// zuerst. Eine kaputte steht mit ihrem Fehler da.
struct BlockStateStack {
    files: Vec<std::result::Result<BlockStateDef, String>>,
    /// Die Definition aus 26.2, falls es den Block dort gibt.
    definition: Option<&'static Definition>,
}

impl Assets {
    pub fn open(roots: Vec<PathBuf>) -> Result<Assets> {
        if roots.is_empty() {
            bail!("kein Asset-Verzeichnis angegeben");
        }
        let mut packs = Vec::with_capacity(roots.len());
        for root in &roots {
            if !root.is_dir() {
                bail!("Asset-Verzeichnis {} existiert nicht", root.display());
            }
            packs.push(Pack::open(root, &pack::ASSETS)?);
        }
        Ok(Assets {
            colors: Colors::load(&packs),
            packs,
            textures: Textures::new(),
            blockstates: HashMap::new(),
            models: HashMap::new(),
            parent_problems: HashMap::new(),
            skipped: BTreeMap::new(),
            broken: BTreeMap::new(),
            unchecked: BTreeMap::new(),
        })
    }

    /// Blockstates, die ganz oder teilweise den Missing-Würfel zeichnen,
    /// weil ein Modell fehlt oder kaputt ist oder keine Variante passt,
    /// und solche, deren Modell ein Parent fehlt — je Blockstate mit dem
    /// ersten Grund.
    pub fn skipped(&self) -> &BTreeMap<String, String> {
        &self.skipped
    }

    /// Blockstate-Dateien, die der Client verwerfen würde, je Pfad mit dem
    /// Grund. Für ihre Zustände gilt die Datei eines tieferen Packs.
    pub fn broken(&self) -> &BTreeMap<String, String> {
        &self.broken
    }

    /// Blockstate-Dateien, deren Multipart-Bedingungen etwas fragen, das
    /// die Definition aus 26.2 nicht kennt, je Pfad mit dem Unbekannten.
    /// Dort vergleicht der Renderer den Text ([`BlockStateDef::instantiate`]).
    pub fn unchecked(&self) -> &BTreeMap<String, String> {
        &self.unchecked
    }

    /// Colormaps und Biome für die Färbung von Gras, Laub und Wasser.
    pub fn colors(&self) -> &Colors {
        &self.colors
    }

    /// Liest Biomdefinitionen aus einer Datenwurzel — das `data/` aus dem
    /// Client-JAR oder ein Datenpaket.
    pub fn load_biomes(&mut self, dir: &Path) -> Result<usize> {
        self.colors.load_biomes(dir)
    }

    /// Lädt eine Textur nach Namen. Flüssigkeiten brauchen ihre Textur,
    /// ohne dass ein Modell sie nennt.
    pub fn texture(&mut self, id: &str) -> TextureId {
        self.textures.load(&self.packs, id)
    }

    pub fn textures(&self) -> &Textures {
        &self.textures
    }

    fn find(&self, namespace: &str, kind: &str, path: &str, extension: &str) -> Option<&Path> {
        find_file(&self.packs, namespace, kind, path, extension).map(|(_, path)| path)
    }

    /// Alle Blockstate-Namen, die in irgendeiner Wurzel definiert sind.
    pub fn block_names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = self
            .packs
            .iter()
            .flat_map(Pack::files)
            .filter_map(|(name, _)| {
                let (namespace, rest) = name.split_once('/')?;
                let block = rest.strip_prefix("blockstates/")?.strip_suffix(".json")?;
                (!block.contains('/')).then(|| format!("{namespace}:{block}"))
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        Ok(names)
    }

    /// Die Blockstate-Dateien eines Blocks aus allen Wurzeln, gelesen wie
    /// im Client ([`BlockStateDef::read`]) und gegen die Definition aus
    /// 26.2 instanziiert, falls es den Block dort gibt. Eine kaputte Datei
    /// steht mit ihrem Fehler da und landet in [`Assets::broken`].
    fn blockstate_stack(&mut self, block: &str) -> Result<Arc<BlockStateStack>> {
        if let Some(stack) = self.blockstates.get(block) {
            return Ok(Arc::clone(stack));
        }
        let (namespace, name) = split_id(block);
        let mut stack = BlockStateStack {
            files: Vec::new(),
            definition: Definition::of(block),
        };
        for pack in self.packs.iter().rev() {
            let Some(path) = pack.listed(namespace, "blockstates", name, "json") else {
                continue;
            };
            let def = read_text(path)
                .and_then(|text| BlockStateDef::read(&text))
                .with_context(|| format!("{} lesen", path.display()));
            match def {
                Ok(mut def) => {
                    if let Some(definition) = stack.definition {
                        let unbekannt: Vec<String> =
                            def.instantiate(definition).into_iter().collect();
                        if !unbekannt.is_empty() {
                            self.unchecked
                                .insert(path.display().to_string(), unbekannt.join(", "));
                        }
                    }
                    stack.files.push(Ok(def));
                }
                Err(error) => {
                    self.broken
                        .insert(path.display().to_string(), format!("{error:#}"));
                    stack.files.push(Err(format!("{error:#}")));
                }
            }
        }
        if stack.files.is_empty() {
            bail!("keine Blockstate-Datei für {block}");
        }
        let stack = Arc::new(stack);
        self.blockstates
            .insert(block.to_string(), Arc::clone(&stack));
        Ok(stack)
    }

    /// Modelle der ersten Alternative — bei `multipart` können es mehrere
    /// sein, bei `variants` genau eines.
    pub fn variants(&mut self, state: &BlockState) -> Result<Vec<ResolvedVariant>> {
        Ok(self.alternatives(state)?.swap_remove(0).1)
    }

    /// Die Modellverweise einer Blockstate mit Gewicht, wie die
    /// Blockstate-Dateien sie nennen.
    ///
    /// Die Packs stapeln sich je Zustand wie in
    /// `loadBlockStateDefinitionStack`: die oberste Datei, die den Zustand
    /// kennt, gewinnt, und eine kaputte fällt aus. Kennt ihn keine, gilt wie
    /// im Client der Missing-Würfel: `ModelManager` füllt jede Blockstate
    /// ohne Modell damit auf. Was fehlt, steht in [`Assets::skipped`].
    pub fn alternative_refs(&mut self, state: &BlockState) -> Result<Vec<(u32, Vec<ModelRef>)>> {
        let stack = self.blockstate_stack(state.name())?;
        let index = stack.definition.and_then(|d| d.index(state));
        let mut grund = None;
        for def in &stack.files {
            match def {
                Ok(def) => {
                    if let Some(refs) = def.alternatives(state, index) {
                        return Ok(refs);
                    }
                }
                Err(error) => {
                    grund.get_or_insert_with(|| error.clone());
                }
            }
        }
        let grund =
            grund.unwrap_or_else(|| "passt auf keine Variante der Blockstate-Datei".to_string());
        self.skip(state, grund);
        Ok(vec![(1, vec![ModelRef::missing()])])
    }

    /// Alle Alternativen einer Blockstate mit ihrem Gewicht, die Modelle
    /// aufgelöst.
    ///
    /// Ein Modell, das fehlt oder kaputt ist, wird zum Missing-Würfel, mit
    /// der Drehung seines Eintrags — wie im Client, der jeden Verweis für
    /// sich auflöst. Bei `multipart` trifft das nur den kaputten Teil, und
    /// eine Alternative behält ihr Gewicht: fiele sie weg, würfelte
    /// `nextInt` an den meisten Positionen anders als das Spiel. Ein Pack
    /// mit einem Tippfehler bricht so keinen Lauf ab. Fehlt einem Modell
    /// nur der Parent, steht die Blockstate ebenso in [`Assets::skipped`].
    pub fn alternatives(&mut self, state: &BlockState) -> Result<Vec<(u32, Vec<ResolvedVariant>)>> {
        let mut out = Vec::new();
        for (weight, refs) in self.alternative_refs(state)? {
            let mut variants = Vec::with_capacity(refs.len());
            for r in refs {
                let (model_id, model) = match self.model(&r.model) {
                    Ok(model) => {
                        if let Some(why) = self.parent_problems.get(&r.model).cloned() {
                            self.skip(state, why);
                        }
                        (r.model, model)
                    }
                    Err(error) => {
                        self.skip(state, format!("{error:#}"));
                        (MISSING_MODEL.to_string(), self.model(MISSING_MODEL)?)
                    }
                };
                variants.push(ResolvedVariant {
                    model_id,
                    model,
                    x: r.x,
                    y: r.y,
                    z: r.z,
                    uvlock: r.uvlock,
                });
            }
            out.push((weight, variants));
        }
        Ok(out)
    }

    /// Merkt sich den ersten Grund, aus dem eine Blockstate den
    /// Missing-Würfel zeichnet.
    fn skip(&mut self, state: &BlockState, why: String) {
        self.skipped.entry(state.to_string()).or_insert(why);
    }

    /// Überträgt den Grund auf eine Blockstate derselben Familie: die
    /// Modelle löst nur ihr erstes Mitglied auf.
    pub fn skip_like(&mut self, state: &BlockState, like: &BlockState) {
        if let Some(why) = self.skipped.get(&like.to_string()).cloned() {
            self.skip(state, why);
        }
    }

    /// Modell mit aufgelöster `parent`-Kette und aufgelösten Texturen.
    ///
    /// Fehlt das Modell, ist es kaputt oder hängt es in einem Zyklus, ist
    /// das ein Fehler, und der Verweis wird zum Missing-Würfel. Fehlt ein
    /// Parent oder ist er kaputt, setzt der Client das Missing-Modell an
    /// seine Stelle ("Missing block model"): die eigenen Elemente des
    /// Kindes bleiben, auch leere, sonst erbt es den Missing-Würfel. Den
    /// Grund merkt sich der Renderer für [`Assets::skipped`]. Den Parent
    /// `builtin/missing` kennt der Client, er ist kein Fehler.
    pub fn model(&mut self, id: &str) -> Result<Arc<ResolvedModel>> {
        if let Some(model) = self.models.get(id) {
            return Ok(Arc::clone(model));
        }

        // parent-Kette einsammeln: das Kind gewinnt bei Texturen, die
        // erstbeste Definition gewinnt bei elements.
        let mut textures: HashMap<String, model::Slot> = HashMap::new();
        let mut elements = None;
        let mut current = Some(id.to_string());
        let mut seen = Vec::new();

        while let Some(model_id) = current {
            if seen.contains(&model_id) {
                bail!("Modell {id}: parent-Zyklus über {model_id}");
            }
            seen.push(model_id.clone());

            let (namespace, name) = split_id(&model_id);
            // Das Itemmodell, das der Client aus `layer0` erzeugt; eine
            // Blockgeometrie hat es nicht. Sonst kennt 26.2 nur noch
            // `builtin/missing`.
            if (namespace, name) == ("minecraft", "builtin/generated") {
                break;
            }
            let raw = if model_id == MISSING_MODEL {
                ModelFile::missing()
            } else {
                match self.read_model(namespace, name) {
                    Ok(raw) => raw,
                    Err(error) if seen.len() == 1 => {
                        return Err(error.context(format!("Modell {model_id}")));
                    }
                    Err(error) => {
                        self.parent_problems.insert(
                            id.to_string(),
                            format!("Parent {model_id} von {id}: {error:#}"),
                        );
                        ModelFile::missing()
                    }
                }
            };

            for (key, slot) in raw.textures {
                textures.entry(key).or_insert(slot);
            }
            if elements.is_none() {
                elements = raw.elements;
            }
            current = raw.parent;
        }

        let model = Arc::new(ResolvedModel::build(
            elements.unwrap_or_default(),
            &textures,
            &mut self.textures,
            &self.packs,
            id,
        )?);
        self.models.insert(id.to_string(), Arc::clone(&model));
        Ok(model)
    }

    /// Eine Modelldatei, wie `CuboidModel` sie liest ([`ModelFile::read`]).
    fn read_model(&self, namespace: &str, name: &str) -> Result<ModelFile> {
        let path = self
            .find(namespace, "models", name, "json")
            .ok_or_else(|| anyhow!("nicht gefunden"))?;
        ModelFile::read(&read_json(path)?).with_context(|| format!("{} lesen", path.display()))
    }
}

/// Erste Datei, die von hinten nach vorne in den Wurzeln gefunden wird —
/// die zuletzt angegebene Wurzel gewinnt. Gefunden wird, was der Client
/// beim Auflisten findet ([`Pack::listed`]).
///
/// Liefert zusätzlich den Index der Wurzel, damit zusammengehörige Dateien
/// wie `.png` und `.png.mcmeta` in derselben oder einer höheren Schicht
/// gesucht werden können.
fn find_file<'a>(
    packs: &'a [Pack],
    namespace: &str,
    kind: &str,
    path: &str,
    extension: &str,
) -> Option<(usize, &'a Path)> {
    packs.iter().enumerate().rev().find_map(|(layer, pack)| {
        pack.listed(namespace, kind, path, extension)
            .map(|file| (layer, file))
    })
}

/// `minecraft:block/stone` -> `("minecraft", "block/stone")`. Ohne Namensraum,
/// auch mit `:` vorn, gilt `minecraft`.
pub fn split_id(id: &str) -> (&str, &str) {
    match id.split_once(':') {
        Some(("", path)) => ("minecraft", path),
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", id),
    }
}

/// Der Text einer Datei, wie der Client ihn liest: kaputtes UTF-8 als
/// U+FFFD (`InputStreamReader`), ein Byte-Order-Mark vorn übersprungen
/// (`JsonReader`).
fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("{} lesen", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// Liest ein Modell so wie Minecraft (`GsonHelper.fromJson`): alles hinter
/// dem ersten Dokument wird ignoriert. In Packs kommen aneinandergehängte
/// Blockbench-Exporte vor.
fn read_json(path: &Path) -> Result<serde_json::Value> {
    parse_json(&read_text(path)?, false).with_context(|| format!("{} lesen", path.display()))
}

/// Liest JSON so streng wie Gson im Modus `STRICT`, den der Client für
/// Blockstates, Modelle, `.mcmeta` und Biome setzt. Mit `ganz` darf hinter
/// dem ersten Dokument nichts mehr stehen, wie bei `StrictJsonParser`;
/// sonst liest es nur das erste, wie `GsonHelper`. Eine Zahl ab 1024
/// Zeichen lehnt schon der Tokenizer ab: so lang ist sein Puffer
/// (`JsonReader.peekNumber`), und nur im Modus `LENIENT` ginge es weiter.
/// Gezählt wird im Text, also auch bei einem Schlüssel, den ein späterer
/// gleichen Namens überschreibt.
fn parse_json(text: &str, ganz: bool) -> Result<serde_json::Value> {
    let (json, ende) = if ganz {
        let json = serde_json::from_str(text).context("kein gültiges JSON")?;
        (json, text.len())
    } else {
        let mut stream = serde_json::Deserializer::from_str(text).into_iter();
        let json = stream
            .next()
            .ok_or_else(|| anyhow!("leer"))?
            .context("kein gültiges JSON")?;
        (json, stream.byte_offset())
    };
    let laenge = laengste_zahl(&text[..ende]);
    ensure!(
        laenge < 1024,
        "eine Zahl mit {laenge} Zeichen, Gson liest höchstens 1023"
    );
    Ok(json)
}

/// Die längste Zahl in gültigem JSON: ausserhalb von Zeichenketten jede
/// Folge ab `-` oder einer Ziffer aus Ziffern, `-`, `+`, `.`, `e` und `E`.
fn laengste_zahl(text: &str) -> usize {
    let mut laengste = 0;
    let mut zeichen = text.bytes().peekable();
    while let Some(b) = zeichen.next() {
        match b {
            b'"' => {
                while let Some(b) = zeichen.next() {
                    match b {
                        b'\\' => {
                            zeichen.next();
                        }
                        b'"' => break,
                        _ => {}
                    }
                }
            }
            b'-' | b'0'..=b'9' => {
                let mut laenge = 1;
                while zeichen
                    .next_if(|b| matches!(b, b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E'))
                    .is_some()
                {
                    laenge += 1;
                }
                laengste = laengste.max(laenge);
            }
            _ => {}
        }
    }
    laengste
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wie `InputStreamReader` und `JsonReader`: kaputtes UTF-8 wird zu
    /// U+FFFD, genau ein Byte-Order-Mark vorn fällt weg.
    #[test]
    fn text_wie_im_client() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.json");
        std::fs::write(&file, b"\xef\xbb\xbfa\xffb").unwrap();
        assert_eq!(read_text(&file).unwrap(), "a\u{fffd}b");
        std::fs::write(&file, b"\xef\xbb\xbf\xef\xbb\xbfa").unwrap();
        assert_eq!(read_text(&file).unwrap(), "\u{feff}a");
    }

    /// Gson liest eine Zahl bis 1023 Zeichen, eine längere lehnt der
    /// Tokenizer ab, auch unter einem Schlüssel, den ein späterer gleichen
    /// Namens überschreibt, und auch in einem ersten Dokument, hinter dem
    /// noch etwas steht. Was dahinter steht, liest `GsonHelper` nicht.
    #[test]
    fn zahlen_bis_1023_zeichen() {
        let zahl = |laenge: usize| format!("1{}", "0".repeat(laenge - 1));
        for ganz in [true, false] {
            assert!(parse_json(&format!("[{}]", zahl(1023)), ganz).is_ok());
            assert!(parse_json(&format!("[{}]", zahl(1024)), ganz).is_err());
            let doppelt = format!(r#"{{"a": {}, "a": 1}}"#, zahl(1024));
            assert!(parse_json(&doppelt, ganz).is_err());
            let text = format!(r#"{{"a": "{}"}}"#, zahl(2000));
            assert!(parse_json(&text, ganz).is_ok(), "in einer Zeichenkette");
        }
        assert!(parse_json(&format!("{{}} [{}]", zahl(1024)), false).is_ok());
        assert!(parse_json("{} []", true).is_err());
    }

    /// Auch ein Modell liest der Renderer so: eine Zahl ab 1024 Zeichen
    /// macht die Datei kaputt, was hinter dem ersten Dokument steht, zählt
    /// nicht.
    #[test]
    fn modell_mit_zu_langer_zahl() {
        let dir = tempfile::tempdir().unwrap();
        let datei = dir.path().join("m.json");
        let zahl = format!("1{}", "0".repeat(1023));
        std::fs::write(&datei, format!(r#"{{"x": {zahl}}}"#)).unwrap();
        assert!(read_json(&datei).is_err());
        std::fs::write(&datei, format!(r#"{{}} {{"x": {zahl}}}"#)).unwrap();
        assert!(read_json(&datei).is_ok());
    }

    #[test]
    fn namensraum_abtrennen() {
        assert_eq!(
            split_id("minecraft:block/stone"),
            ("minecraft", "block/stone")
        );
        assert_eq!(split_id("block/stone"), ("minecraft", "block/stone"));
        assert_eq!(split_id(":block/stone"), ("minecraft", "block/stone"));
        assert_eq!(split_id("terranova:block/x"), ("terranova", "block/x"));
    }
}
