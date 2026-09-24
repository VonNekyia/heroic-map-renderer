pub mod baker;
pub mod blockstate;
pub mod colors;
pub mod fluid;
pub mod model;
pub mod texture;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};

use crate::world::BlockState;
pub use baker::{BakedModel, Quad, bake};
pub use blockstate::{BlockStateDef, ModelRef};
pub use colors::{Colors, Tint, Tints};
pub use model::{Element, ElementFace, Face, ResolvedModel, Rotation};
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

/// Wie tief die `parent`-Kette eines Modells verfolgt wird, bevor ein Zyklus
/// angenommen wird. Vanilla-Ketten sind höchstens vier Glieder lang.
const MAX_PARENT_DEPTH: usize = 16;

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
/// das in einer Datei nur einen Teil der Zustände nennt, auch.
pub struct Assets {
    roots: Vec<PathBuf>,
    textures: Textures,
    blockstates: HashMap<String, Arc<BlockStateStack>>,
    models: HashMap<String, Arc<ResolvedModel>>,
    colors: Colors,
    skipped: BTreeMap<String, String>,
    broken: BTreeMap<String, String>,
}

/// Die Blockstate-Dateien eines Blocks aus allen Wurzeln, die oberste
/// zuerst. Eine kaputte steht mit ihrem Fehler da.
type BlockStateStack = Vec<std::result::Result<BlockStateDef, String>>;

impl Assets {
    pub fn open(roots: Vec<PathBuf>) -> Result<Assets> {
        if roots.is_empty() {
            bail!("kein Asset-Verzeichnis angegeben");
        }
        for root in &roots {
            if !root.is_dir() {
                bail!("Asset-Verzeichnis {} existiert nicht", root.display());
            }
        }
        Ok(Assets {
            colors: Colors::load(&roots),
            roots,
            textures: Textures::new(),
            blockstates: HashMap::new(),
            models: HashMap::new(),
            skipped: BTreeMap::new(),
            broken: BTreeMap::new(),
        })
    }

    /// Blockstates, die ganz oder teilweise den Missing-Würfel zeichnen,
    /// weil ein Modell fehlt oder kaputt ist oder keine Variante passt —
    /// je Blockstate mit dem ersten Grund.
    pub fn skipped(&self) -> &BTreeMap<String, String> {
        &self.skipped
    }

    /// Blockstate-Dateien, die der Client verwerfen würde, je Pfad mit dem
    /// Grund. Für ihre Zustände gilt die Datei eines tieferen Packs.
    pub fn broken(&self) -> &BTreeMap<String, String> {
        &self.broken
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
        self.textures.load(&self.roots, id)
    }

    pub fn textures(&self) -> &Textures {
        &self.textures
    }

    fn find(&self, namespace: &str, kind: &str, path: &str, extension: &str) -> Option<PathBuf> {
        find_file(&self.roots, namespace, kind, path, extension).map(|(_, path)| path)
    }

    /// Alle Blockstate-Namen, die in irgendeiner Wurzel definiert sind.
    pub fn block_names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for root in &self.roots {
            let Ok(namespaces) = std::fs::read_dir(root) else {
                continue;
            };
            for namespace in namespaces.flatten() {
                let dir = namespace.path().join("blockstates");
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                let namespace = namespace.file_name().to_string_lossy().into_owned();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "json") {
                        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                        names.push(format!("{namespace}:{stem}"));
                    }
                }
            }
        }
        names.sort_unstable();
        names.dedup();
        Ok(names)
    }

    /// Die Blockstate-Dateien eines Blocks aus allen Wurzeln. Streng
    /// gelesen wie im Client (`StrictJsonParser`), eine kaputte Datei steht
    /// mit ihrem Fehler da und landet in [`Assets::broken`].
    fn blockstate_stack(&mut self, block: &str) -> Result<Arc<BlockStateStack>> {
        if let Some(stack) = self.blockstates.get(block) {
            return Ok(Arc::clone(stack));
        }
        let (namespace, name) = split_id(block);
        let mut stack = Vec::new();
        for root in self.roots.iter().rev() {
            let Some(path) = file_in(root, namespace, "blockstates", name, "json") else {
                continue;
            };
            let def = read_json_strict(&path).and_then(|json| {
                BlockStateDef::parse(&json).with_context(|| format!("{} lesen", path.display()))
            });
            if let Err(error) = &def {
                self.broken
                    .insert(path.display().to_string(), format!("{error:#}"));
            }
            stack.push(def.map_err(|error| format!("{error:#}")));
        }
        if stack.is_empty() {
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
        let mut grund = None;
        for def in stack.iter() {
            match def {
                Ok(def) => {
                    if let Some(refs) = def.alternatives(state) {
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
    /// mit einem Tippfehler bricht so keinen Lauf ab.
    pub fn alternatives(&mut self, state: &BlockState) -> Result<Vec<(u32, Vec<ResolvedVariant>)>> {
        let mut out = Vec::new();
        for (weight, refs) in self.alternative_refs(state)? {
            let mut variants = Vec::with_capacity(refs.len());
            for r in refs {
                let (model_id, model) = if r.model == MISSING_MODEL {
                    (r.model, Arc::new(ResolvedModel::missing()))
                } else {
                    match self.model(&r.model) {
                        Ok(model) => (r.model, model),
                        Err(error) => {
                            self.skip(state, format!("{error:#}"));
                            (
                                MISSING_MODEL.to_string(),
                                Arc::new(ResolvedModel::missing()),
                            )
                        }
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
    pub fn model(&mut self, id: &str) -> Result<Arc<ResolvedModel>> {
        if let Some(model) = self.models.get(id) {
            return Ok(Arc::clone(model));
        }

        // parent-Kette einsammeln: das Kind gewinnt bei Texturen, die
        // erstbeste Definition gewinnt bei elements.
        let mut textures: HashMap<String, model::TextureValue> = HashMap::new();
        let mut elements = None;
        let mut current = Some(id.to_string());
        let mut seen = Vec::new();

        while let Some(model_id) = current {
            if seen.contains(&model_id) {
                bail!("Modell {id}: parent-Zyklus über {model_id}");
            }
            if seen.len() >= MAX_PARENT_DEPTH {
                bail!("Modell {id}: parent-Kette tiefer als {MAX_PARENT_DEPTH}");
            }
            seen.push(model_id.clone());

            let (namespace, name) = split_id(&model_id);
            // builtin/... hat keine Datei; solche Blöcke (Truhen, Banner)
            // rendert Minecraft über Entity-Modelle, die V1 nicht kennt.
            // Fehlt ein Parent, setzt der Client an seine Stelle das
            // Missing-Modell ("Missing block model"): die eigenen Elemente
            // des Kindes bleiben, sonst erbt es den Missing-Würfel.
            let Some(path) = self.find(namespace, "models", name, "json") else {
                if name.starts_with("builtin/") || (seen.len() > 1 && elements.is_some()) {
                    break;
                }
                bail!("Modell {model_id} nicht gefunden (verlangt von {id})");
            };

            let json = read_json(&path)?;
            let raw: model::ModelJson = serde_json::from_value(json)
                .with_context(|| format!("{} lesen", path.display()))?;

            for (key, value) in raw.textures {
                textures.entry(key).or_insert(value);
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
            &self.roots,
            id,
        )?);
        self.models.insert(id.to_string(), Arc::clone(&model));
        Ok(model)
    }
}

/// Erste Datei, die von hinten nach vorne in den Wurzeln gefunden wird —
/// die zuletzt angegebene Wurzel gewinnt.
///
/// Liefert zusätzlich den Index der Wurzel, damit zusammengehörige Dateien
/// wie `.png` und `.png.mcmeta` in derselben oder einer höheren Schicht
/// gesucht werden können.
fn find_file(
    roots: &[PathBuf],
    namespace: &str,
    kind: &str,
    path: &str,
    extension: &str,
) -> Option<(usize, PathBuf)> {
    roots.iter().enumerate().rev().find_map(|(layer, root)| {
        file_in(root, namespace, kind, path, extension).map(|file| (layer, file))
    })
}

/// Die Datei in genau dieser Wurzel, falls es sie gibt.
fn file_in(
    root: &Path,
    namespace: &str,
    kind: &str,
    path: &str,
    extension: &str,
) -> Option<PathBuf> {
    let mut file = path
        .split('/')
        .fold(root.join(namespace).join(kind), |acc, part| acc.join(part));
    file.as_mut_os_string().push(".");
    file.as_mut_os_string().push(extension);
    file.is_file().then_some(file)
}

/// `minecraft:block/stone` -> `("minecraft", "block/stone")`. Ohne Namensraum
/// gilt `minecraft`.
pub fn split_id(id: &str) -> (&str, &str) {
    match id.split_once(':') {
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", id),
    }
}

/// Liest ein Modell so wie Minecraft (`GsonHelper.fromJson`): alles hinter
/// dem ersten Dokument wird ignoriert. In Packs kommen aneinandergehängte
/// Blockbench-Exporte vor.
fn read_json(path: &Path) -> Result<serde_json::Value> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} lesen", path.display()))?;
    let mut stream = serde_json::Deserializer::from_str(&text).into_iter::<serde_json::Value>();
    stream
        .next()
        .ok_or_else(|| anyhow!("{} ist leer", path.display()))?
        .with_context(|| format!("{} ist kein gültiges JSON", path.display()))
}

/// Liest eine Blockstate-Datei so streng wie der Client
/// (`StrictJsonParser`): nach dem ersten Dokument darf nichts mehr kommen.
fn read_json_strict(path: &Path) -> Result<serde_json::Value> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} lesen", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} ist kein gültiges JSON", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namensraum_abtrennen() {
        assert_eq!(
            split_id("minecraft:block/stone"),
            ("minecraft", "block/stone")
        );
        assert_eq!(split_id("block/stone"), ("minecraft", "block/stone"));
        assert_eq!(split_id("terranova:block/x"), ("terranova", "block/x"));
    }
}
